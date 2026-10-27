//! Who has been given extra access (speakers) from the admin page, and until when.
//! Saved as JSON so a restart doesn't forget, and so the firewall rule can be rebuilt.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

/// Unix seconds when the grant ends; `None` means until revoked.
pub type Until = Option<u64>;

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    speakers: BTreeMap<String, Until>,
}

pub struct Grants {
    path: PathBuf,
    inner: Mutex<Saved>,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Grants {
    pub async fn load(dir: PathBuf) -> Self {
        let path = dir.join("grants.json");
        let saved = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                warn!(error = %e, path = %path.display(), "ignoring unreadable grants file");
                Saved::default()
            }),
            Err(_) => Saved::default(),
        };
        Self { path, inner: Mutex::new(saved) }
    }

    async fn save(&self, saved: &Saved) {
        let json = serde_json::to_vec_pretty(saved).expect("grants serialize");
        if let Err(e) = tokio::fs::write(&self.path, json).await {
            warn!(error = %e, path = %self.path.display(), "could not save grants");
        }
    }

    pub async fn snapshot(&self) -> BTreeMap<String, Until> {
        self.inner.lock().await.speakers.clone()
    }

    pub async fn macs(&self) -> Vec<String> {
        self.inner.lock().await.speakers.keys().cloned().collect()
    }

    pub async fn grant(&self, mac: String, minutes: Option<u32>) {
        let mut s = self.inner.lock().await;
        s.speakers.insert(mac, minutes.map(|m| now() + u64::from(m) * 60));
        self.save(&s).await;
    }

    /// Returns whether anything was removed.
    pub async fn revoke(&self, mac: &str) -> bool {
        let mut s = self.inner.lock().await;
        let removed = s.speakers.remove(mac).is_some();
        if removed {
            self.save(&s).await;
        }
        removed
    }

    /// Drops expired grants; returns the MACs that were removed.
    pub async fn expire(&self, at: u64) -> Vec<String> {
        let mut s = self.inner.lock().await;
        let gone: Vec<String> =
            s.speakers.iter().filter(|(_, until)| until.is_some_and(|t| t <= at)).map(|(m, _)| m.clone()).collect();
        if !gone.is_empty() {
            for m in &gone {
                s.speakers.remove(m);
            }
            self.save(&s).await;
        }
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn grant_expire_revoke_persist() {
        let dir = std::env::temp_dir().join(format!("umbilical-grants-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = Grants::load(dir.clone()).await;
        g.grant("aa:aa:aa:aa:aa:aa".into(), Some(60)).await;
        g.grant("bb:bb:bb:bb:bb:bb".into(), None).await;
        assert_eq!(g.macs().await.len(), 2);

        // Nothing has expired yet; an hour and a bit later only the timed one has.
        assert!(g.expire(now()).await.is_empty());
        assert_eq!(g.expire(now() + 3601).await, vec!["aa:aa:aa:aa:aa:aa".to_string()]);

        // Survives a reload.
        let g2 = Grants::load(dir.clone()).await;
        assert_eq!(g2.macs().await, vec!["bb:bb:bb:bb:bb:bb".to_string()]);
        assert!(g2.revoke("bb:bb:bb:bb:bb:bb").await);
        assert!(!g2.revoke("bb:bb:bb:bb:bb:bb").await);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
