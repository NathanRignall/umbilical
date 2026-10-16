mod config;
mod unifi;

use std::{sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};

use config::{Backend, Config, GuestLimits};
use unifi::UniFi;

struct AppState {
    unifi: Option<UniFi>,
    limits: GuestLimits,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "unifi_portal=info,tower_http=info".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    let unifi = match &cfg.backend {
        Backend::UniFi { api_base, api_key, insecure_tls } => {
            info!(%api_base, insecure_tls, "using UniFi controller");
            Some(UniFi::new(api_base.clone(), api_key, *insecure_tls)?)
        }
        Backend::Mock => {
            warn!("UNIFI_MOCK is set: authorizations are faked");
            None
        }
    };
    let state = Arc::new(AppState { unifi, limits: cfg.limits.clone() });

    // Every unknown path (notably /guest/s/<site>/) gets the SPA. Captive
    // portal browsers cache aggressively, so tell them not to.
    let index = cfg.static_dir.join("index.html");
    let spa = ServeDir::new(&cfg.static_dir).fallback(ServeFile::new(index));

    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/authorize", post(authorize))
        .with_state(state)
        .fallback_service(spa)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        ))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(cfg.listen).await?;
    info!("portal listening on http://{}", cfg.listen);
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizeRequest {
    site: String,
    client_mac: String,
    #[serde(default)]
    ap_mac: Option<String>,
    #[serde(default)]
    ssid: Option<String>,
    accepted_terms: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizeResponse {
    expires_at: Option<String>,
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<unifi::Error> for ApiError {
    fn from(e: unifi::Error) -> Self {
        warn!(error = %e, "authorization failed");
        match e {
            unifi::Error::ClientNotFound(_) => Self(
                StatusCode::NOT_FOUND,
                "We couldn't find your device on the network. Try reconnecting to the Wi-Fi.".into(),
            ),
            // Don't leak controller details to guests.
            _ => Self(StatusCode::BAD_GATEWAY, "The network is having trouble right now. Please try again.".into()),
        }
    }
}

async fn authorize(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AuthorizeRequest>,
) -> Result<Json<AuthorizeResponse>, ApiError> {
    if !req.accepted_terms {
        return Err(ApiError(StatusCode::BAD_REQUEST, "You need to accept the terms to continue.".into()));
    }
    let mac = normalize_mac(&req.client_mac)
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "Invalid device address.".into()))?;
    info!(%mac, site = %req.site, ap = ?req.ap_mac, ssid = ?req.ssid, "guest accepted terms");

    let Some(unifi) = &state.unifi else {
        let expires = std::time::SystemTime::now() + Duration::from_secs(u64::from(state.limits.minutes) * 60);
        return Ok(Json(AuthorizeResponse {
            expires_at: Some(humantime::format_rfc3339_seconds(expires).to_string()),
        }));
    };

    let site_id = unifi.site_id(&req.site).await?;

    // A guest that just associated can take a moment to show up in the API.
    let mut client = None;
    for attempt in 0..4 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(750)).await;
        }
        if let Some(c) = unifi.find_client(&site_id, &mac).await? {
            client = Some(c);
            break;
        }
    }
    let client = client.ok_or_else(|| unifi::Error::ClientNotFound(mac.clone()))?;

    if !client.access.authorized {
        unifi.authorize_guest(&site_id, &client.id, &state.limits).await?;
    }

    // Best effort: report when access ends. Authorization already succeeded.
    let expires_at = match unifi.client(&site_id, &client.id).await {
        Ok(c) => c.access.authorization.and_then(|a| a.expires_at),
        Err(e) => {
            warn!(error = %e, "could not read back authorization");
            None
        }
    };
    info!(%mac, client_id = %client.id, ?expires_at, "guest authorized");
    Ok(Json(AuthorizeResponse { expires_at }))
}

/// Accepts `aa:bb:cc:dd:ee:ff`, `AA-BB-…`, or bare hex. Returns lowercase colon form.
fn normalize_mac(raw: &str) -> Option<String> {
    let hex: String = raw.chars().filter(|c| !matches!(c, ':' | '-' | '.')).collect();
    if hex.len() != 12 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let hex = hex.to_ascii_lowercase();
    Some((0..6).map(|i| &hex[i * 2..i * 2 + 2]).collect::<Vec<_>>().join(":"))
}

#[cfg(test)]
mod tests {
    use super::normalize_mac;

    #[test]
    fn macs() {
        assert_eq!(normalize_mac("1C:71:25:63:E4:24").as_deref(), Some("1c:71:25:63:e4:24"));
        assert_eq!(normalize_mac("1c-71-25-63-e4-24").as_deref(), Some("1c:71:25:63:e4:24"));
        assert_eq!(normalize_mac("1c712563e424").as_deref(), Some("1c:71:25:63:e4:24"));
        assert_eq!(normalize_mac("1c:71:25:63:e4"), None);
        assert_eq!(normalize_mac("zz:71:25:63:e4:24"), None);
        assert_eq!(normalize_mac("'); drop"), None);
    }
}
