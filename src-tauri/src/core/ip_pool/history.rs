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

    pub fn join_history_path(base_dir: &Path) -> PathBuf {
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

    /// Prune entries based on capacity limit (LRU-like based on measured_at).
    /// Returns the number of entries removed.
    pub fn enforce_capacity(&self, max_entries: usize) -> Result<usize> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("ip history poisoned"))?;

        if guard.entries.len() <= max_entries {
            return Ok(0);
        }

        // Sort by measured_at (oldest first)
        guard.entries.sort_by_key(|e| e.measured_at_epoch_ms);
        let to_remove = guard.entries.len() - max_entries;
        guard.entries.drain(0..to_remove);

        Self::persist(self.path.as_deref(), &guard)?;
        tracing::info!(
            target = "ip_pool",
            removed = to_remove,
            remaining = guard.entries.len(),
            "enforced ip history capacity limit"
        );
        Ok(to_remove)
    }

    /// Prune expired entries and enforce capacity limit.
    /// Returns (expired_count, capacity_pruned_count).
    pub fn prune_and_enforce(
        &self,
        now_epoch_ms: i64,
        max_entries: usize,
    ) -> Result<(usize, usize)> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow!("ip history poisoned"))?;

        let before = guard.entries.len();
        guard
            .entries
            .retain(|e| e.expires_at_epoch_ms > now_epoch_ms);
        let expired = before - guard.entries.len();

        let capacity_pruned = if guard.entries.len() > max_entries {
            guard.entries.sort_by_key(|e| e.measured_at_epoch_ms);
            let to_remove = guard.entries.len() - max_entries;
            guard.entries.drain(0..to_remove);
            to_remove
        } else {
            0
        };

        if expired > 0 || capacity_pruned > 0 {
            Self::persist(self.path.as_deref(), &guard)?;
            tracing::debug!(
                target = "ip_pool",
                expired = expired,
                capacity_pruned = capacity_pruned,
                remaining = guard.entries.len(),
                "pruned ip history"
            );
        }

        Ok((expired, capacity_pruned))
    }

    fn persist(path: Option<&Path>, file: &IpHistoryFile) -> Result<()> {
        if let Some(path) = path {
            let json = serde_json::to_string_pretty(file).context("serialize ip history")?;
            // Check file size before writing (warn if > 1MB)
            if json.len() > 1_048_576 {
                tracing::warn!(
                    target = "ip_pool",
                    path = %path.display(),
                    size_bytes = json.len(),
                    entries = file.entries.len(),
                    "ip history file size exceeds 1MB; consider reducing max entries"
                );
            }
            fs::write(path, json).with_context(|| format!("write ip history: {}", path.display()))
        } else {
            Ok(())
        }
    }
}
