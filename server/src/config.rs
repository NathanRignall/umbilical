use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};

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

        Ok(Self { listen, static_dir, backend, limits })
    }
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
