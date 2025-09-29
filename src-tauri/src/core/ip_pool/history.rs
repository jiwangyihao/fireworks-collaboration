use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use super::{IpCandidate, IpSource};

const IP_HISTORY_FILE_NAME: &str = "ip-history.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IpHistoryRecord {
    pub host: String,
    pub port: u16,
    pub candidate: IpCandidate,
    #[serde(default)]
    pub sources: Vec<IpSource>,
    pub latency_ms: u32,
    pub measured_at_epoch_ms: i64,
    pub expires_at_epoch_ms: i64,
}

impl IpHistoryRecord {
    pub fn key(&self) -> (&str, u16) {
        (&self.host, self.port)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct IpHistoryFile {
    #[serde(default)]
    entries: Vec<IpHistoryRecord>,
}

#[derive(Debug)]
pub struct IpHistoryStore {
    path: Option<PathBuf>,
    inner: Mutex<IpHistoryFile>,
}

impl IpHistoryStore {
    pub fn load_or_init_at(base_dir: &Path) -> Result<Self> {
        let path = Self::join_history_path(base_dir);
        let file = if path.exists() {
            let data =
                fs::read(&path).with_context(|| format!("read ip history: {}", path.display()))?;
            match serde_json::from_slice::<IpHistoryFile>(&data) {
                Ok(parsed) => parsed,
                Err(err) => {
                    tracing::warn!(
                        target = "ip_pool",
                        path = %path.display(),
                        error = %err,
                        "ip history corrupted, resetting"
                    );
                    IpHistoryFile::default()
                }
            }
        } else {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).ok();
            }
            let default = IpHistoryFile::default();
            Self::persist(Some(&path), &default)?;
            default
        };
        Ok(Self {
            path: Some(path),
            inner: Mutex::new(file),
        })
    }

    pub fn load_or_init_from_file(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let file = if path.exists() {
            let data =
                fs::read(path).with_context(|| format!("read ip history: {}", path.display()))?;
            match serde_json::from_slice::<IpHistoryFile>(&data) {
                Ok(parsed) => parsed,
                Err(err) => {
                    tracing::warn!(
                        target = "ip_pool",
                        path = %path.display(),
                        error = %err,
                        "ip history corrupted, resetting"
                    );
                    IpHistoryFile::default()
                }
            }
        } else {
            let default = IpHistoryFile::default();
            Self::persist(Some(path), &default)?;
            default
        };
        Ok(Self {
            path: Some(path.to_path_buf()),
            inner: Mutex::new(file),
        })
    }

    pub fn load_default() -> Result<Self> {
        let base = crate::core::config::loader::base_dir();
        Self::load_or_init_at(&base)
    }

    pub fn in_memory() -> Self {
        Self {
            path: None,
            inner: Mutex::new(IpHistoryFile::default()),
        }
    }

    fn join_history_path(base_dir: &Path) -> PathBuf {
        let mut p = base_dir.to_path_buf();
        p.push("config");
        p.push(IP_HISTORY_FILE_NAME);
        p
    }

    pub fn upsert(&self, record: IpHistoryRecord) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("ip history poisoned"))?;
        if let Some(existing) = guard
            .entries
            .iter_mut()
            .find(|item| item.host == record.host && item.port == record.port)
        {
            *existing = record;
        } else {
            guard.entries.push(record);
        }
        Self::persist(self.path.as_deref(), &guard)
    }

    pub fn get(&self, host: &str, port: u16) -> Option<IpHistoryRecord> {
        self.inner.lock().ok().and_then(|guard| {
            guard
                .entries
                .iter()
                .find(|e| e.host == host && e.port == port)
                .cloned()
        })
    }

    pub fn get_fresh(&self, host: &str, port: u16, now_epoch_ms: i64) -> Option<IpHistoryRecord> {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => return None,
        };

        if let Some(idx) = guard
            .entries
            .iter()
            .position(|entry| entry.host == host && entry.port == port)
        {
            let expires_at = guard.entries[idx].expires_at_epoch_ms;
            if expires_at > now_epoch_ms {
                return Some(guard.entries[idx].clone());
            }

            guard.entries.remove(idx);
            if let Err(err) = Self::persist(self.path.as_deref(), &guard) {
                tracing::warn!(
                    target = "ip_pool",
                    error = %err,
                    host,
                    port,
                    "failed to persist ip history after pruning expired entry"
                );
            }
        }

        None
    }

    pub fn snapshot(&self) -> Option<Vec<IpHistoryRecord>> {
        self.inner.lock().ok().map(|guard| guard.entries.clone())
    }

    pub fn clear(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("ip history poisoned"))?;
        guard.entries.clear();
        Self::persist(self.path.as_deref(), &guard)
    }

    pub fn remove(&self, host: &str, port: u16) -> Result<bool> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("ip history poisoned"))?;
        if let Some(idx) = guard
            .entries
            .iter()
            .position(|entry| entry.host == host && entry.port == port)
        {
            guard.entries.remove(idx);
            Self::persist(self.path.as_deref(), &guard)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn persist(path: Option<&Path>, file: &IpHistoryFile) -> Result<()> {
        if let Some(path) = path {
            let json = serde_json::to_string_pretty(file).context("serialize ip history")?;
            fs::write(path, json).with_context(|| format!("write ip history: {}", path.display()))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ip_pool::{IpCandidate, IpSource};
    use std::net::{IpAddr, Ipv4Addr};
    use uuid::Uuid;

    #[test]
    fn load_or_init_creates_file() {
        let dir = std::env::temp_dir().join(format!("ip-history-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = IpHistoryStore::load_or_init_at(&dir).expect("load history");
        let path = IpHistoryStore::join_history_path(&dir);
        assert!(path.exists());
        assert!(store.snapshot().unwrap().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn upsert_and_get_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ip-history-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let store = IpHistoryStore::load_or_init_at(&dir).expect("load history");
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin, IpSource::Dns],
            latency_ms: 32,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 2,
        };
        store.upsert(record.clone()).expect("write history");
        let fetched = store.get("github.com", 443).expect("history entry");
        assert_eq!(fetched.latency_ms, 32);
        assert_eq!(fetched.sources, vec![IpSource::Builtin, IpSource::Dns]);
        let snapshot = store.snapshot().unwrap();
        assert_eq!(snapshot.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_or_init_from_file_creates_parent_dirs() {
        let base = std::env::temp_dir().join(format!("ip-history-file-{}", Uuid::new_v4()));
        let path = base.join("nested").join("custom-history.json");
        let store = IpHistoryStore::load_or_init_from_file(&path).expect("load history file");
        assert!(path.exists());
        assert!(store.snapshot().unwrap().is_empty());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn get_fresh_evicts_expired_records() {
        let store = IpHistoryStore::in_memory();
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 5,
        };
        store.upsert(record).expect("write history");
        let missing = store.get_fresh("github.com", 443, 10);
        assert!(missing.is_none());
        assert!(store.snapshot().unwrap().is_empty());
    }

    #[test]
    fn get_fresh_returns_valid_record() {
        let store = IpHistoryStore::in_memory();
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin, IpSource::History],
            latency_ms: 8,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 10_000,
        };
        store.upsert(record.clone()).expect("write history");
        let fetched = store
            .get_fresh("github.com", 443, 5_000)
            .expect("fresh record");
        assert_eq!(fetched.latency_ms, record.latency_ms);
        assert_eq!(fetched.sources, record.sources);
        assert_eq!(store.snapshot().unwrap().len(), 1);
    }

    #[test]
    fn remove_clears_matching_entry() {
        let store = IpHistoryStore::in_memory();
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 10_000,
        };
        store.upsert(record).expect("write history");
        assert!(store.remove("github.com", 443).expect("remove entry"));
        assert!(store.get("github.com", 443).is_none());
        assert!(!store.remove("github.com", 443).expect("idempotent remove"));
    }
}
