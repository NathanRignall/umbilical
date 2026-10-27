//! Minimal client for the official UniFi Network integration API (v1).
//! Docs: https://help.ui.com/hc/en-us/articles/31228198640023
//! and the API reference in Network > Settings > Control Plane > Integrations.

use std::{collections::HashMap, time::Duration};

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::config::GuestLimits;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("UniFi site '{0}' not found")]
    SiteNotFound(String),
    #[error("device {0} is not connected to the network")]
    ClientNotFound(String),
    #[error("UniFi API returned {status}: {body}")]
    Api { status: StatusCode, body: String },
    #[error("could not reach UniFi controller: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Site {
    id: String,
    internal_reference: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GuestClient {
    pub name: Option<String>,
    pub mac_address: Option<String>,
    pub ip_address: Option<String>,
    pub connected_at: Option<String>,
    #[serde(default)]
    pub access: Access,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub id: String,
    #[serde(default)]
    pub access: Access,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    #[serde(default)]
    pub authorized: bool,
    pub authorization: Option<Authorization>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Authorization {
    pub authorized_at: Option<String>,
    pub expires_at: Option<String>,
    #[serde(rename = "dataUsageLimitMBytes")]
    pub data_limit_mbytes: Option<u64>,
    pub authorization_method: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionResponse {
    granted_authorization: Option<Authorization>,
}

pub struct UniFi {
    http: Client,
    base: String,
    /// `/guest/s/<name>/` name -> site UUID. Sites basically never change.
    sites: RwLock<HashMap<String, String>>,
}

impl UniFi {
    pub fn new(base: String, api_key: &str, insecure_tls: bool) -> anyhow::Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut key = reqwest::header::HeaderValue::from_str(api_key)?;
        key.set_sensitive(true);
        headers.insert("X-API-KEY", key);
        headers.insert(reqwest::header::ACCEPT, "application/json".parse()?);

        let http = Client::builder()
            .default_headers(headers)
            // Consoles ship with a self-signed certificate.
            .danger_accept_invalid_certs(insecure_tls)
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self { http, base, sites: RwLock::default() })
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str, filter: Option<&str>) -> Result<T, Error> {
        let mut req = self.http.get(format!("{}{path}", self.base));
        if let Some(f) = filter {
            req = req.query(&[("filter", f)]);
        }
        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(api_error(res).await);
        }
        Ok(res.json().await?)
    }

    /// Map the site name from the redirect URL (usually `default`) to its UUID.
    pub async fn site_id(&self, name: &str) -> Result<String, Error> {
        if let Some(id) = self.sites.read().await.get(name) {
            return Ok(id.clone());
        }
        let sites: Page<Site> = self.get("/v1/sites", None).await?;
        let found = sites
            .data
            .iter()
            .find(|s| s.internal_reference.as_deref() == Some(name) || s.name.as_deref() == Some(name))
            // A Cloud Gateway has exactly one site; don't be picky about its name.
            .or_else(|| (sites.data.len() == 1).then(|| &sites.data[0]))
            .ok_or_else(|| Error::SiteNotFound(name.to_owned()))?;
        self.sites.write().await.insert(name.to_owned(), found.id.clone());
        Ok(found.id.clone())
    }

    pub async fn find_client(&self, site_id: &str, mac: &str) -> Result<Option<ClientInfo>, Error> {
        let filter = format!("macAddress.eq('{mac}')");
        let page: Page<ClientInfo> = self.get(&format!("/v1/sites/{site_id}/clients"), Some(&filter)).await?;
        Ok(page.data.into_iter().next())
    }

    pub async fn client(&self, site_id: &str, client_id: &str) -> Result<ClientInfo, Error> {
        self.get(&format!("/v1/sites/{site_id}/clients/{client_id}"), None).await
    }

    /// Returns the authorization the controller says it granted, when the response includes it.
    /// Guests currently connected to the site.
    pub async fn list_guests(&self, site_id: &str) -> Result<Vec<GuestClient>, Error> {
        let mut out = Vec::new();
        let mut offset = 0;
        loop {
            let res = self
                .http
                .get(format!("{}/v1/sites/{site_id}/clients", self.base))
                .query(&[("filter", "access.type.eq('GUEST')")])
                .query(&[("limit", 200), ("offset", offset)])
                .send()
                .await?;
            if !res.status().is_success() {
                return Err(api_error(res).await);
            }
            let page: Page<GuestClient> = res.json().await?;
            let n = page.data.len();
            out.extend(page.data);
            if n < 200 {
                return Ok(out);
            }
            offset += 200;
        }
    }

    /// The firewall policy with this name, as raw JSON (we send most of it back unchanged).
    pub async fn find_policy(&self, site_id: &str, name: &str) -> Result<Option<Value>, Error> {
        let page: Page<Value> = self.get(&format!("/v1/sites/{site_id}/firewall/policies?limit=200"), None).await?;
        Ok(page.data.into_iter().find(|p| p["name"].as_str() == Some(name)))
    }

    /// Point a policy's source at exactly these MACs, disabling it when there are none
    /// (UniFi requires at least one MAC in the filter). Everything else is kept as is.
    pub async fn set_policy_macs(&self, site_id: &str, mut policy: Value, macs: &[String]) -> Result<(), Error> {
        let id = policy["id"].as_str().unwrap_or_default().to_owned();
        if let Some(obj) = policy.as_object_mut() {
            for read_only in ["id", "index", "metadata"] {
                obj.remove(read_only);
            }
        }
        policy["enabled"] = Value::Bool(!macs.is_empty());
        if !macs.is_empty() {
            policy["source"]["trafficFilter"] = json!({
                "type": "MAC_ADDRESS",
                "macAddressFilter": { "macAddresses": macs },
            });
        }
        let res = self
            .http
            .put(format!("{}/v1/sites/{site_id}/firewall/policies/{id}", self.base))
            .json(&policy)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(api_error(res).await);
        }
        Ok(())
    }

    pub async fn authorize_guest(
        &self,
        site_id: &str,
        client_id: &str,
        limits: &GuestLimits,
    ) -> Result<Option<Authorization>, Error> {
        let mut body = json!({
            "action": "AUTHORIZE_GUEST_ACCESS",
            "timeLimitMinutes": limits.minutes,
        });
        for (key, val) in [
            ("dataUsageLimitMBytes", limits.data_mbytes),
            ("rxRateLimitKbps", limits.rx_kbps),
            ("txRateLimitKbps", limits.tx_kbps),
        ] {
            if let Some(v) = val {
                body[key] = v.into();
            }
        }
        let res = self
            .http
            .post(format!("{}/v1/sites/{site_id}/clients/{client_id}/actions", self.base))
            .json(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            return Err(api_error(res).await);
        }
        let text = res.text().await?;
        match serde_json::from_str::<ActionResponse>(&text) {
            Ok(r) => Ok(r.granted_authorization),
            Err(_) => {
                tracing::warn!(body = %text, "unexpected authorize response");
                Ok(None)
            }
        }
    }
}

async fn api_error(res: reqwest::Response) -> Error {
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Error::Api { status, body }
}
