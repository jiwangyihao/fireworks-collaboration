use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    sync::RwLock,
};

/// IP 候选来源分类，贯穿配置、缓存与事件输出。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum IpSource {
    Builtin,
    Dns,
    History,
    UserStatic,
    Fallback,
}

/// IP 候选条目，记录来源与端口信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct IpCandidate {
    pub address: IpAddr,
    pub port: u16,
    pub source: IpSource,
}

impl IpCandidate {
    pub fn new(address: IpAddr, port: u16, source: IpSource) -> Self {
        Self {
            address,
            port,
            source,
        }
    }
}

impl Default for IpCandidate {
    fn default() -> Self {
        Self {
            address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: 0,
            source: IpSource::Builtin,
        }
    }
}

/// 单个 IP 的评分信息，预留延迟与生命周期字段，后续阶段可填充。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpStat {
    pub candidate: IpCandidate,
    #[serde(default)]
    pub sources: Vec<IpSource>,
    /// TCP 握手延迟（毫秒）；P4.0 阶段未计算时保持 None。
    #[serde(default)]
    pub latency_ms: Option<u32>,
    /// 评分产生时间（Unix epoch 毫秒）。
    #[serde(default)]
    pub measured_at_epoch_ms: Option<i64>,
    /// 评分过期时间（Unix epoch 毫秒）。
    #[serde(default)]
    pub expires_at_epoch_ms: Option<i64>,
}

impl IpStat {
    pub fn with_latency(candidate: IpCandidate, latency_ms: u32) -> Self {
        let initial_source = candidate.source;
        Self {
            candidate,
            latency_ms: Some(latency_ms),
            measured_at_epoch_ms: None,
            expires_at_epoch_ms: None,
            sources: vec![initial_source],
        }
    }

    pub fn is_expired(&self, now_ms: i64) -> bool {
        match self.expires_at_epoch_ms {
            Some(expires) => expires <= now_ms,
            None => false,
        }
    }
}

/// 缓存键：域名 + 端口。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IpCacheKey {
    pub host: String,
    pub port: u16,
}

impl IpCacheKey {
    pub fn new<S: Into<String>>(host: S, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }
}

/// 缓存槽位：当前最佳候选与备选列表。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpCacheSlot {
    #[serde(default)]
    pub best: Option<IpStat>,
    #[serde(default)]
    pub alternatives: Vec<IpStat>,
}

impl IpCacheSlot {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn with_best(stat: IpStat) -> Self {
        Self {
            best: Some(stat),
            alternatives: Vec::new(),
        }
    }
}

/// 评分缓存，负责并发访问控制。
#[derive(Debug, Default)]
pub struct IpScoreCache {
    inner: RwLock<HashMap<IpCacheKey, IpCacheSlot>>,
}

impl IpScoreCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, host: &str, port: u16) -> Option<IpCacheSlot> {
        let guard = self.inner.read().ok()?;
        guard.get(&IpCacheKey::new(host.to_string(), port)).cloned()
    }

    pub fn insert(&self, key: IpCacheKey, slot: IpCacheSlot) {
        if let Ok(mut guard) = self.inner.write() {
            guard.insert(key, slot);
        }
    }

    pub fn remove(&self, host: &str, port: u16) {
        if let Ok(mut guard) = self.inner.write() {
            guard.remove(&IpCacheKey::new(host.to_string(), port));
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.write() {
            guard.clear();
        }
    }

    pub fn snapshot(&self) -> HashMap<IpCacheKey, IpCacheSlot> {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn cache_insert_and_get_best() {
        let cache = IpScoreCache::new();
        let key = IpCacheKey::new("github.com", 443);
        let stat = IpStat::with_latency(
            IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            42,
        );
        cache.insert(key.clone(), IpCacheSlot::with_best(stat.clone()));
        let fetched = cache.get("github.com", 443).unwrap();
        assert_eq!(fetched.best.as_ref().unwrap().latency_ms, Some(42));
        assert_eq!(
            fetched.best.unwrap().candidate.address,
            stat.candidate.address
        );
        assert_eq!(stat.sources, vec![IpSource::Builtin]);
        // 确保 snapshot 复制，而不是共享引用
        let snapshot = cache.snapshot();
        assert!(snapshot.contains_key(&key));
    }

    #[test]
    fn cache_remove_and_clear() {
        let cache = IpScoreCache::new();
        cache.insert(
            IpCacheKey::new("github.com", 443),
            IpCacheSlot::with_best(IpStat::with_latency(
                IpCandidate::new(
                    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                    443,
                    IpSource::Builtin,
                ),
                10,
            )),
        );
        cache.remove("github.com", 443);
        assert!(cache.get("github.com", 443).is_none());
        cache.insert(
            IpCacheKey::new("github.com", 80),
            IpCacheSlot::with_best(IpStat::with_latency(
                IpCandidate::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80, IpSource::Dns),
                5,
            )),
        );
        cache.clear();
        assert!(cache.get("github.com", 80).is_none());
    }
}
