//! Who has been given extra access (speakers, TV, ...) and until when, per access group.
//! Saved as JSON so a restart doesn't forget, and so the firewall rules can be rebuilt.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::warn;

/// Unix seconds when the grant ends; `None` means until revoked.
pub type Until = Option<u64>;
/// MAC -> until.
pub type Members = BTreeMap<String, Until>;

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// group id -> members.
    groups: BTreeMap<String, Members>,
}

impl Saved {
    /// Reads the current format, or the older speakers-only one (`{"speakers": {mac: until}}`).
    fn parse(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let v: Value = serde_json::from_slice(bytes)?;
        if v.get("groups").is_some() {
            return serde_json::from_value(v);
        }
        let speakers: Members = serde_json::from_value(v.get("speakers").cloned().unwrap_or_default())?;
        Ok(Self { groups: BTreeMap::from([("speakers".to_owned(), speakers)]) })
    }
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
            Ok(bytes) => Saved::parse(&bytes).unwrap_or_else(|e| {
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

    /// group id -> members.
    pub async fn snapshot(&self) -> BTreeMap<String, Members> {
        self.inner.lock().await.groups.clone()
    }

    pub async fn macs(&self, group: &str) -> Vec<String> {
        self.inner.lock().await.groups.get(group).map(|m| m.keys().cloned().collect()).unwrap_or_default()
    }

    pub async fn grant(&self, group: &str, mac: String, minutes: Option<u32>) {
        let mut s = self.inner.lock().await;
        s.groups.entry(group.to_owned()).or_default().insert(mac, minutes.map(|m| now() + u64::from(m) * 60));
        self.save(&s).await;
    }

    /// Like `grant`, but never shortens an existing grant. Returns the resulting end time.
    pub async fn extend(&self, group: &str, mac: String, minutes: u32) -> Until {
        let mut s = self.inner.lock().await;
        let members = s.groups.entry(group.to_owned()).or_default();
        let new = now() + u64::from(minutes) * 60;
        let until = match members.get(&mac) {
            Some(None) => None,
            Some(Some(t)) => Some((*t).max(new)),
            None => Some(new),
        };
        members.insert(mac, until);
        self.save(&s).await;
        until
    }

    /// Returns whether anything was removed.
    pub async fn revoke(&self, group: &str, mac: &str) -> bool {
        let mut s = self.inner.lock().await;
        let removed = s.groups.get_mut(group).is_some_and(|m| m.remove(mac).is_some());
        if removed {
            self.save(&s).await;
        }
        removed
    }

    /// Drops expired grants; returns (group, MAC) for each one removed.
    pub async fn expire(&self, at: u64) -> Vec<(String, String)> {
        let mut s = self.inner.lock().await;
        let mut gone = Vec::new();
        for (group, members) in &mut s.groups {
            members.retain(|mac, until| {
                let keep = !until.is_some_and(|t| t <= at);
                if !keep {
                    gone.push((group.clone(), mac.clone()));
                }
                keep
            });
        }
        if !gone.is_empty() {
            self.save(&s).await;
        }
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("umbilical-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn grant_expire_revoke_persist() {
        let dir = tmp("grants");
        let g = Grants::load(dir.clone()).await;
        g.grant("speakers", "aa:aa:aa:aa:aa:aa".into(), Some(60)).await;
        g.grant("speakers", "bb:bb:bb:bb:bb:bb".into(), None).await;
        g.grant("tv", "aa:aa:aa:aa:aa:aa".into(), None).await;
        assert_eq!(g.macs("speakers").await.len(), 2);
        assert_eq!(g.macs("tv").await, vec!["aa:aa:aa:aa:aa:aa".to_string()]);

        // Nothing has expired yet; an hour and a bit later only the timed speakers one has.
        assert!(g.expire(now()).await.is_empty());
        assert_eq!(g.expire(now() + 3601).await, vec![("speakers".to_string(), "aa:aa:aa:aa:aa:aa".to_string())]);

        // Survives a reload; groups stay separate.
        let g2 = Grants::load(dir.clone()).await;
        assert_eq!(g2.macs("speakers").await, vec!["bb:bb:bb:bb:bb:bb".to_string()]);
        assert_eq!(g2.macs("tv").await.len(), 1);
        assert!(g2.revoke("speakers", "bb:bb:bb:bb:bb:bb").await);
        assert!(!g2.revoke("speakers", "bb:bb:bb:bb:bb:bb").await);
        assert!(!g2.revoke("nope", "bb:bb:bb:bb:bb:bb").await);

        // extend never shortens: a forever grant stays forever, a longer one stays longer.
        g2.grant("speakers", "cc:cc:cc:cc:cc:cc".into(), None).await;
        assert_eq!(g2.extend("speakers", "cc:cc:cc:cc:cc:cc".into(), 10).await, None);
        g2.grant("speakers", "dd:dd:dd:dd:dd:dd".into(), Some(600)).await;
        let long = g2.snapshot().await["speakers"]["dd:dd:dd:dd:dd:dd"];
        assert_eq!(g2.extend("speakers", "dd:dd:dd:dd:dd:dd".into(), 10).await, long);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn migrates_speakers_only_file() {
        let dir = tmp("migrate");
        std::fs::write(dir.join("grants.json"), r#"{"speakers": {"aa:aa:aa:aa:aa:aa": null}}"#).unwrap();
        let g = Grants::load(dir.clone()).await;
        assert_eq!(g.macs("speakers").await, vec!["aa:aa:aa:aa:aa:aa".to_string()]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
