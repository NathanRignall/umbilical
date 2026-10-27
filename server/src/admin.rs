//! Extra access for guests (speakers, TV, ...): the admin page's API, which lists guests and
//! grants or revokes access per guest and group, and the self-service QR code endpoint.
//! The admin API only answers clients in ADMIN_ALLOWED_SUBNETS (see `guard`).

use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

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

use crate::{AppState, config::Group, grants, normalize_mac};

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

/// The group named in the URL, or a 404.
fn group<'a>(state: &'a AppState, id: &str) -> Result<&'a Group, AdminError> {
    state.groups.get(id).ok_or_else(|| AdminError(StatusCode::NOT_FOUND, "That isn't available here.".into()))
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
    /// group id -> until (Unix seconds; `null` = until revoked). Only groups they have.
    access: BTreeMap<String, Option<u64>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    id: String,
    label: String,
    emoji: String,
    policy: String,
    /// The QR code's link and how long it grants, when self-service is on for this group.
    self_service: Option<SelfService>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfService {
    minutes: u32,
    /// Absolute when PUBLIC_URL is set, else a path the page joins to its own origin.
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestList {
    guests: Vec<Guest>,
    groups: Vec<GroupInfo>,
}

pub async fn list(State(state): State<Arc<AppState>>) -> Result<Json<GuestList>, AdminError> {
    let grants = state.grants.snapshot().await;
    let access_of = |mac: &str| -> BTreeMap<String, Option<u64>> {
        grants
            .iter()
            .filter(|(id, _)| state.groups.contains_key(*id))
            .filter_map(|(id, members)| members.get(mac).map(|until| (id.clone(), *until)))
            .collect()
    };
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
            Some(Guest {
                name: c.name,
                ip: c.ip_address,
                connected_at: c.connected_at,
                joined: c.access.authorized,
                access: access_of(&mac),
                mac,
            })
        })
        .collect();
    // Granted devices that aren't connected right now still get a row, so they can be revoked.
    let granted: BTreeSet<&String> = grants.values().flat_map(|m| m.keys()).collect();
    for mac in granted {
        if !guests.iter().any(|g| &g.mac == mac) {
            guests.push(Guest {
                name: None,
                mac: mac.clone(),
                ip: None,
                connected_at: None,
                joined: false,
                access: access_of(mac),
            });
        }
    }
    guests.retain(|g| g.ip.is_some() || !g.access.is_empty());

    let groups = state
        .groups
        .iter()
        .map(|(id, g)| GroupInfo {
            id: id.clone(),
            label: g.label.clone(),
            emoji: g.emoji.clone(),
            policy: g.policy.clone(),
            self_service: g.self_service_minutes.map(|minutes| {
                let path = match &state.qr_key {
                    Some(k) => format!("/{id}?k={k}"),
                    None => format!("/{id}"),
                };
                SelfService { minutes, url: format!("{}{path}", state.public_url.as_deref().unwrap_or("")) }
            }),
        })
        .collect();
    Ok(Json(GuestList { guests, groups }))
}

#[derive(Deserialize)]
pub struct GrantRequest {
    /// `null` = until revoked.
    minutes: Option<u32>,
}

pub async fn grant(
    State(state): State<Arc<AppState>>,
    Path((id, mac)): Path<(String, String)>,
    Json(req): Json<GrantRequest>,
) -> Result<StatusCode, AdminError> {
    group(&state, &id)?;
    let mac = normalize_mac(&mac).ok_or_else(|| AdminError(StatusCode::BAD_REQUEST, "bad MAC".into()))?;
    state.grants.grant(&id, mac.clone(), req.minutes).await;
    info!(group = %id, %mac, minutes = ?req.minutes, "access granted");
    sync(&state, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke(
    State(state): State<Arc<AppState>>,
    Path((id, mac)): Path<(String, String)>,
) -> Result<StatusCode, AdminError> {
    group(&state, &id)?;
    let mac = normalize_mac(&mac).ok_or_else(|| AdminError(StatusCode::BAD_REQUEST, "bad MAC".into()))?;
    if state.grants.revoke(&id, &mac).await {
        info!(group = %id, %mac, "access revoked");
    }
    sync(&state, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Make one group's UniFi firewall policy source list match its saved grants.
pub async fn sync(state: &AppState, id: &str) -> Result<(), AdminError> {
    let g = group(state, id)?;
    let _one_at_a_time = state.sync_lock.lock().await;
    let macs = state.grants.macs(id).await;
    let Some(unifi) = &state.unifi else {
        info!(group = %id, ?macs, "access policy (mock)");
        return Ok(());
    };
    let site = unifi.site_id(&state.site).await?;
    let policy = unifi.find_policy(&site, &g.policy).await?.ok_or_else(|| {
        AdminError(
            StatusCode::BAD_GATEWAY,
            format!("No firewall policy named \"{}\" in UniFi. Create it first (see README).", g.policy),
        )
    })?;
    unifi.set_policy_macs(&site, policy, &macs).await?;
    info!(group = %id, ?macs, policy = %g.policy, "access policy updated");
    Ok(())
}

/// Bring UniFi in line at startup, then expire grants as they run out.
pub async fn run_expiry(state: Arc<AppState>) {
    // Grants come from the admin page and/or QR codes; with neither, there's nothing to do.
    let self_service = state.groups.values().any(|g| g.self_service_minutes.is_some());
    if state.admin_subnets.is_empty() && !self_service {
        return;
    }
    // Groups whose UniFi policy still needs updating; retried every tick until it works.
    let mut pending: BTreeSet<String> = state.groups.keys().cloned().collect();
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    loop {
        tick.tick().await;
        for (id, mac) in state.grants.expire(grants::now()).await {
            info!(group = %id, %mac, "access expired");
            pending.insert(id);
        }
        for id in std::mem::take(&mut pending) {
            if let Err(e) = sync(&state, &id).await {
                warn!(group = %id, error = %e.1, "access sync failed; will retry");
                pending.insert(id);
            }
        }
    }
}

#[derive(Deserialize)]
pub struct SelfServiceRequest {
    #[serde(default)]
    k: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfServiceResponse {
    /// Unix seconds; null = until revoked.
    until: Option<u64>,
    label: String,
    emoji: String,
}

/// /<group> (the QR code): the guest's own device gets that access for a while.
/// The device is found by the request's IP, so this only works from the guest Wi-Fi.
pub async fn self_service(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
    Json(req): Json<SelfServiceRequest>,
) -> Result<Json<SelfServiceResponse>, AdminError> {
    let g = group(&state, &id)?;
    let Some(minutes) = g.self_service_minutes else {
        return Err(AdminError(StatusCode::NOT_FOUND, "That isn't available here.".into()));
    };
    if state.qr_key.is_some() && req.k != state.qr_key {
        warn!(group = %id, ip = %peer.ip(), "self-service: wrong or missing QR key");
        return Err(AdminError(
            StatusCode::FORBIDDEN,
            format!("This code doesn't work any more. Scan the one by the {}.", g.label),
        ));
    }
    let ip = peer.ip().to_string();
    let mac = match &state.unifi {
        Some(u) => {
            let site = u.site_id(&state.site).await?;
            let guest = u.guest_by_ip(&site, &ip).await?.ok_or_else(|| {
                AdminError(StatusCode::CONFLICT, "Connect to the womb Wi-Fi first, then scan again.".into())
            })?;
            if !guest.access.authorized {
                return Err(AdminError(StatusCode::CONFLICT, "Join the Wi-Fi first, then scan again.".into()));
            }
            guest.mac_address.as_deref().and_then(normalize_mac)
        }
        // Mock mode: pretend every visitor is the same guest.
        None => Some("02:00:00:00:00:01".to_owned()),
    }
    .ok_or_else(|| AdminError(StatusCode::BAD_GATEWAY, "Couldn't identify your device.".into()))?;

    let until = state.grants.extend(&id, mac.clone(), minutes).await;
    info!(group = %id, %ip, %mac, ?until, "self-service access");
    sync(&state, &id).await?;
    Ok(Json(SelfServiceResponse { until, label: g.label.clone(), emoji: g.emoji.clone() }))
}
