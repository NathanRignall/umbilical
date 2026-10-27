//! The admin page's API: list guests, and give or take away speaker access per guest.
//! Only answers clients in ADMIN_ALLOWED_SUBNETS (see `guard`).

use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{ConnectInfo, Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::{AppState, grants, normalize_mac};

/// 404 the admin page and API for anyone outside the allowed subnets (guests never see it).
pub async fn guard(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == "/admin" || path.starts_with("/admin/") || path.starts_with("/api/admin/") {
        let ip = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip());
        let allowed = ip.is_some_and(|ip| state.admin_subnets.iter().any(|n| n.contains(&ip)));
        if !allowed {
            warn!(?ip, path, "admin request refused");
            return StatusCode::NOT_FOUND.into_response();
        }
    }
    next.run(req).await
}

pub struct AdminError(StatusCode, String);

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<crate::unifi::Error> for AdminError {
    fn from(e: crate::unifi::Error) -> Self {
        warn!(error = %e, "admin UniFi call failed");
        Self(StatusCode::BAD_GATEWAY, e.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Guest {
    name: Option<String>,
    mac: String,
    ip: Option<String>,
    connected_at: Option<String>,
    /// Joined through the portal (UniFi guest authorization).
    joined: bool,
    /// Unix seconds; `null` with `speakers: true` means until revoked.
    speakers_until: Option<u64>,
    speakers: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestList {
    guests: Vec<Guest>,
    policy: String,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<GuestList>, AdminError> {
    let grants = state.grants.snapshot().await;
    let clients = match &state.unifi {
        Some(u) => {
            let site = u.site_id(&state.site).await?;
            u.list_guests(&site).await?
        }
        None => Vec::new(),
    };

    let mut guests: Vec<Guest> = clients
        .into_iter()
        .filter_map(|c| {
            let mac = normalize_mac(c.mac_address.as_deref()?)?;
            let until = grants.get(&mac).copied();
            Some(Guest {
                name: c.name,
                ip: c.ip_address,
                connected_at: c.connected_at,
                joined: c.access.authorized,
                speakers: until.is_some(),
                speakers_until: until.flatten(),
                mac,
            })
        })
        .collect();
    // Granted devices that aren't connected right now still get a row, so they can be revoked.
    for (mac, until) in &grants {
        if !guests.iter().any(|g| &g.mac == mac) {
            guests.push(Guest {
                name: None,
                mac: mac.clone(),
                ip: None,
                connected_at: None,
                joined: false,
                speakers: true,
                speakers_until: *until,
            });
        }
    }
    Ok(Json(GuestList { guests, policy: state.speakers_policy.clone() }))
}

#[derive(Deserialize)]
pub struct GrantRequest {
    /// `null` = until revoked.
    minutes: Option<u32>,
}

pub async fn grant(
    State(state): State<Arc<AppState>>,
    Path(mac): Path<String>,
    Json(req): Json<GrantRequest>,
) -> Result<StatusCode, AdminError> {
    let mac = normalize_mac(&mac).ok_or_else(|| AdminError(StatusCode::BAD_REQUEST, "bad MAC".into()))?;
    state.grants.grant(mac.clone(), req.minutes).await;
    info!(%mac, minutes = ?req.minutes, "speakers granted");
    sync(&state).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke(State(state): State<Arc<AppState>>, Path(mac): Path<String>) -> Result<StatusCode, AdminError> {
    let mac = normalize_mac(&mac).ok_or_else(|| AdminError(StatusCode::BAD_REQUEST, "bad MAC".into()))?;
    if state.grants.revoke(&mac).await {
        info!(%mac, "speakers revoked");
    }
    sync(&state).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Make the UniFi firewall policy's source list match the saved grants.
pub async fn sync(state: &AppState) -> Result<(), AdminError> {
    let _one_at_a_time = state.sync_lock.lock().await;
    let macs = state.grants.macs().await;
    let Some(unifi) = &state.unifi else {
        info!(?macs, "speakers policy (mock)");
        return Ok(());
    };
    let site = unifi.site_id(&state.site).await?;
    let policy = unifi.find_policy(&site, &state.speakers_policy).await?.ok_or_else(|| {
        AdminError(
            StatusCode::BAD_GATEWAY,
            format!("No firewall policy named \"{}\" in UniFi. Create it first (see README).", state.speakers_policy),
        )
    })?;
    unifi.set_policy_macs(&site, policy, &macs).await?;
    info!(?macs, policy = %state.speakers_policy, "speakers policy updated");
    Ok(())
}

/// Bring UniFi in line at startup, then expire grants as they run out.
pub async fn run_expiry(state: Arc<AppState>) {
    if state.admin_subnets.is_empty() {
        return;
    }
    if let Err(e) = sync(&state).await {
        warn!(error = %e.1, "initial speakers sync failed");
    }
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    // Set when UniFi still needs updating, so a failed sync is retried on the next tick.
    let mut pending = false;
    loop {
        tick.tick().await;
        let gone = state.grants.expire(grants::now()).await;
        if !gone.is_empty() {
            info!(?gone, "speakers access expired");
            pending = true;
        }
        if pending {
            match sync(&state).await {
                Ok(()) => pending = false,
                Err(e) => warn!(error = %e.1, "speakers sync failed; will retry"),
            }
        }
    }
}
