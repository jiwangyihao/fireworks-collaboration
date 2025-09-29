use std::net::{IpAddr, Ipv4Addr};
use std::time::{SystemTime, UNIX_EPOCH};

use fireworks_collaboration_lib::core::ip_pool::cache::{
    IpCacheSlot, IpCandidate, IpScoreCache, IpStat,
};
use fireworks_collaboration_lib::core::ip_pool::config::{
    EffectiveIpPoolConfig, IpPoolSourceToggle, UserStaticIp,
};
use fireworks_collaboration_lib::core::ip_pool::history::IpHistoryRecord;
use fireworks_collaboration_lib::core::ip_pool::{IpCacheKey, IpSource};

/// 返回启用运行时的默认配置。
pub fn enabled_config() -> EffectiveIpPoolConfig {
    let mut cfg = EffectiveIpPoolConfig::default();
    cfg.runtime.enabled = true;
    cfg
}

/// 构造仅启用 UserStatic 源的配置，并注入指定 host/ip/port。
pub fn user_static_only_config(host: &str, address: IpAddr, port: u16) -> EffectiveIpPoolConfig {
    let mut cfg = enabled_config();
    cfg.runtime.sources = IpPoolSourceToggle {
        builtin: false,
        dns: false,
        history: false,
        user_static: true,
        fallback: false,
    };
    cfg.file.user_static.push(UserStaticIp {
        host: host.to_string(),
        ip: address.to_string(),
        ports: vec![port],
    });
    cfg
}

/// 便捷创建 IPv4 候选。
pub fn candidate_v4(octets: [u8; 4], port: u16, source: IpSource) -> IpCandidate {
    IpCandidate::new(IpAddr::V4(Ipv4Addr::from(octets)), port, source)
}

/// 基于候选构造带延迟数据的评分记录，并可额外设置时间字段。
pub fn stat_with_latency(
    mut stat: IpStat,
    measured_at_ms: Option<i64>,
    expires_at_ms: Option<i64>,
) -> IpStat {
    stat.measured_at_epoch_ms = measured_at_ms;
    stat.expires_at_epoch_ms = expires_at_ms;
    stat
}

/// 创建带延迟的评分记录。
pub fn make_latency_stat(
    octets: [u8; 4],
    port: u16,
    latency_ms: u32,
    source: IpSource,
    measured_at_ms: Option<i64>,
    expires_at_ms: Option<i64>,
) -> IpStat {
    let candidate = candidate_v4(octets, port, source);
    let stat = IpStat::with_latency(candidate, latency_ms);
    stat_with_latency(stat, measured_at_ms, expires_at_ms)
}

/// 插入单条 best 评分到缓存。
pub fn cache_best(cache: &IpScoreCache, host: &str, port: u16, stat: IpStat) {
    cache.insert(IpCacheKey::new(host, port), IpCacheSlot::with_best(stat));
}

/// 基于评分生成历史记录，便于保持字段一致。
pub fn history_record(host: &str, port: u16, stat: &IpStat) -> IpHistoryRecord {
    IpHistoryRecord {
        host: host.to_string(),
        port,
        candidate: stat.candidate.clone(),
        sources: stat.sources.clone(),
        latency_ms: stat.latency_ms.unwrap_or_default(),
        measured_at_epoch_ms: stat.measured_at_epoch_ms.unwrap_or_default(),
        expires_at_epoch_ms: stat.expires_at_epoch_ms.unwrap_or_default(),
    }
}

/// 当前 Unix 毫秒时间戳。
pub fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as i64
}
