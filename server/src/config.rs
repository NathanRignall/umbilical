use std::{collections::BTreeMap, env, net::SocketAddr, path::PathBuf};

use serde::Deserialize;

use anyhow::{Context, Result, bail};
use ipnet::IpNet;

/// Limits applied to every guest we authorize. `None` = no limit.
#[derive(Clone, Debug)]
pub struct GuestLimits {
    pub minutes: u32,
    pub data_mbytes: Option<u32>,
    pub rx_kbps: Option<u32>,
    pub tx_kbps: Option<u32>,
}

#[derive(Clone, Debug)]
pub enum Backend {
    /// Talk to a real UniFi Network Application (9.1+).
    UniFi {
        /// e.g. `https://192.168.1.1/proxy/network/integration`
        api_base: String,
        api_key: String,
        insecure_tls: bool,
    },
    /// Pretend every authorization succeeds. For UI work without hardware.
    Mock,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub listen: SocketAddr,
    pub static_dir: PathBuf,
    pub backend: Backend,
    pub limits: GuestLimits,
    /// Where the admin page's grants are saved.
    pub state_dir: PathBuf,
    /// The UniFi site the admin page manages (its `internalReference`, usually `default`).
    pub site: String,
    /// Clients allowed to use the admin page. Empty disables it.
    pub admin_subnets: Vec<IpNet>,
    /// Things guests can be given access to (speakers, TV, ...), by id. Each has a UniFi
    /// firewall policy whose source MAC list we manage.
    pub groups: BTreeMap<String, Group>,
    /// If set, the QR links (/<group>?k=<key>) must carry it; change it to retire printed codes.
    pub qr_key: Option<String>,
    /// Base URL for the printed QR code, e.g. http://communalwomb.com. Defaults to the admin page's own.
    pub public_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen = var("LISTEN_ADDR")
            .unwrap_or_else(|| "0.0.0.0:80".into())
            .parse()
            .context("LISTEN_ADDR")?;
        let static_dir = var("STATIC_DIR").unwrap_or_else(|| "../web/dist".into()).into();

        let backend = if flag("UNIFI_MOCK") {
            Backend::Mock
        } else {
            let api_base = match (var("UNIFI_API_BASE"), var("UNIFI_URL")) {
                (Some(base), _) => base,
                // UniFi OS consoles (UDM, UCG, Cloud Key Gen2+) expose the
                // Network integration API behind the /proxy/network prefix.
                (None, Some(url)) => format!("{}/proxy/network/integration", url.trim_end_matches('/')),
                (None, None) => bail!("set UNIFI_URL (or UNIFI_API_BASE), or UNIFI_MOCK=1"),
            };
            let api_key = var("UNIFI_API_KEY").context("UNIFI_API_KEY is required")?;
            Backend::UniFi {
                api_base: api_base.trim_end_matches('/').to_owned(),
                api_key,
                insecure_tls: flag("UNIFI_INSECURE_TLS"),
            }
        };

        let limits = GuestLimits {
            minutes: num("GUEST_MINUTES")?.unwrap_or(24 * 60),
            data_mbytes: num("GUEST_DATA_LIMIT_MB")?,
            rx_kbps: num("GUEST_RX_KBPS")?,
            tx_kbps: num("GUEST_TX_KBPS")?,
        };

        let admin_subnets = var("ADMIN_ALLOWED_SUBNETS")
            .unwrap_or_default()
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().with_context(|| format!("ADMIN_ALLOWED_SUBNETS: bad subnet {s:?}")))
            .collect::<Result<_>>()?;

        Ok(Self {
            listen,
            static_dir,
            backend,
            limits,
            state_dir: var("STATE_DIR").unwrap_or_else(|| ".".into()).into(),
            site: var("UNIFI_SITE").unwrap_or_else(|| "default".into()),
            admin_subnets,
            groups: groups()?,
            qr_key: var("QR_KEY"),
            public_url: var("PUBLIC_URL").map(|u| u.trim_end_matches('/').to_owned()),
        })
    }
}

/// One thing guests can be given access to.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Shown to people, e.g. "speakers" or "TV". Defaults to the id.
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub emoji: String,
    /// UniFi firewall policy name. Defaults to "umbilical: <id>".
    #[serde(default)]
    pub policy: String,
    /// Guests can turn it on themselves at /<id> (the QR code) for this long. None = admin only.
    #[serde(default)]
    pub self_service_minutes: Option<u32>,
}

/// ACCESS_GROUPS is JSON: {"speakers": {"emoji": "🔊", "selfServiceMinutes": 240}, "tv": {...}}.
/// Unset means a single "speakers" group.
fn groups() -> Result<BTreeMap<String, Group>> {
    let mut groups: BTreeMap<String, Group> = match var("ACCESS_GROUPS") {
        Some(json) => serde_json::from_str(&json).context("ACCESS_GROUPS must be a JSON object of groups")?,
        None => BTreeMap::from([("speakers".to_owned(), Group {
            label: String::new(),
            emoji: "🔊".into(),
            policy: String::new(),
            self_service_minutes: None,
        })]),
    };
    for (id, g) in &mut groups {
        // The id becomes a URL path (/<id>), so keep it simple and clear of the portal's own paths.
        if id.is_empty()
            || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || ["admin", "api", "assets", "guest"].contains(&id.as_str())
        {
            bail!("ACCESS_GROUPS: bad group id {id:?} (use lowercase letters, digits and dashes)");
        }
        if g.label.is_empty() {
            g.label = id.clone();
        }
        if g.policy.is_empty() {
            g.policy = format!("umbilical: {id}");
        }
    }
    Ok(groups)
}

fn var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn flag(key: &str) -> bool {
    matches!(var(key).as_deref(), Some("1" | "true" | "yes" | "on"))
}

fn num(key: &str) -> Result<Option<u32>> {
    var(key).map(|v| v.parse().with_context(|| format!("{key} must be a number"))).transpose()
}
