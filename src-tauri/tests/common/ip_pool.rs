#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use fireworks_collaboration_lib::core::ip_pool::cache::{
    IpCacheSlot, IpCandidate, IpScoreCache, IpStat,
};
use fireworks_collaboration_lib::core::ip_pool::config::{
    EffectiveIpPoolConfig, IpPoolSourceToggle, UserStaticIp,
};
use fireworks_collaboration_lib::core::ip_pool::history::IpHistoryRecord;
use fireworks_collaboration_lib::core::ip_pool::{IpCacheKey, IpSource};
use fireworks_collaboration_lib::events::structured::{
    clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
};

/// 返回启用运行时的默认配置。
pub fn enabled_config() -> EffectiveIpPoolConfig {
    let mut cfg = EffectiveIpPoolConfig::default();
    cfg.runtime.enabled = true;
    cfg
}

/// 快速构造一个已启用但所有 sources 默认值保持不变的池实例 (带空缓存)。
pub fn enabled_pool() -> fireworks_collaboration_lib::core::ip_pool::IpPool {
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpScoreCache};
    IpPool::with_cache(enabled_config(), IpScoreCache::new())
}

/// 构造一个带可选自定义缓存的启用池，便于合并测试内快速复用。
pub fn enabled_pool_with_cache(
    cache: IpScoreCache,
) -> fireworks_collaboration_lib::core::ip_pool::IpPool {
    use fireworks_collaboration_lib::core::ip_pool::IpPool;
    IpPool::with_cache(enabled_config(), cache)
}

/// 返回启用且指定 history_path 的配置。
pub fn enabled_config_with_history<P: AsRef<std::path::Path>>(path: P) -> EffectiveIpPoolConfig {
    let mut cfg = enabled_config();
    cfg.runtime.history_path = Some(path.as_ref().to_string_lossy().into());
    cfg
}

/// 构造仅启用 `UserStatic` 源的配置，并注入指定 host/ip/port。
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

/// 关闭所有采样来源，常用于测试回退到系统默认逻辑。
pub fn disable_all_sources(cfg: &mut EffectiveIpPoolConfig) {
    cfg.runtime.sources = IpPoolSourceToggle {
        builtin: false,
        dns: false,
        history: false,
        user_static: false,
        fallback: false,
    };
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
        resolver_metadata: stat.resolver_metadata.clone(),
    }
}

/// 构造一个自定义字段的历史记录（无需先构造 IpStat）。
pub fn make_history_record(
    host: &str,
    port: u16,
    ip: [u8; 4],
    sources: Vec<IpSource>,
    latency_ms: u32,
    measured_at_ms: i64,
    expires_at_ms: i64,
) -> IpHistoryRecord {
    use fireworks_collaboration_lib::core::ip_pool::IpCandidate;
    IpHistoryRecord {
        host: host.to_string(),
        port,
        candidate: IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
            port,
            sources.first().cloned().unwrap_or(IpSource::Builtin),
        ),
        sources,
        latency_ms,
        measured_at_epoch_ms: measured_at_ms,
        expires_at_epoch_ms: expires_at_ms,
        resolver_metadata: Vec::new(),
    }
}

/// 便捷构造仅 Builtin 来源的历史记录。
pub fn make_history_record_builtin(
    host: &str,
    port: u16,
    ip: [u8; 4],
    latency_ms: u32,
    measured_at_ms: i64,
    expires_at_ms: i64,
) -> IpHistoryRecord {
    make_history_record(
        host,
        port,
        ip,
        vec![IpSource::Builtin],
        latency_ms,
        measured_at_ms,
        expires_at_ms,
    )
}

/// 同时写入缓存与历史（若历史可用），减少测试中重复样板。
pub fn cache_and_history(
    host: &str,
    port: u16,
    stat: IpStat,
    cache: &IpScoreCache,
    history: Option<&fireworks_collaboration_lib::core::ip_pool::history::IpHistoryStore>,
) {
    cache_best(cache, host, port, stat.clone());
    if let Some(store) = history {
        let record = history_record(host, port, &stat);
        store.upsert(record).expect("upsert history");
    }
}

/// 当前 Unix 毫秒时间戳。
pub fn epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as i64
}

/// Helper guard that installs a fresh `MemoryEventBus` for the duration of a test.
/// Automatically clears the global bus when dropped to avoid cross-test leakage.
pub struct TestEventBus {
    bus: Arc<MemoryEventBus>,
}

impl TestEventBus {
    /// Installs a new in-memory event bus and returns a guard for interacting with it.
    pub fn install() -> Self {
        let bus = Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        Self { bus }
    }

    /// Returns a clone of the underlying bus for advanced scenarios.
    pub fn handle(&self) -> Arc<MemoryEventBus> {
        self.bus.clone()
    }

    /// Returns a snapshot of all events currently captured by the bus.
    pub fn snapshot(&self) -> Vec<Event> {
        self.bus.snapshot()
    }

    /// Returns only the strategy events captured by the bus.
    pub fn strategy_events(&self) -> Vec<StrategyEvent> {
        self.snapshot()
            .into_iter()
            .filter_map(|event| match event {
                Event::Strategy(evt) => Some(evt),
                _ => None,
            })
            .collect()
    }
}

impl Drop for TestEventBus {
    fn drop(&mut self) {
        clear_test_event_bus();
    }
}

/// Convenience constructor to install a temporary memory event bus.
pub fn install_test_event_bus() -> TestEventBus {
    TestEventBus::install()
}

/// 断言仅产生一个 StrategyEvent，并应用调用方提供的检查逻辑。
/// 用于大量事件测试中减少重复样板代码。
pub fn expect_single_strategy_event<F>(events: Vec<StrategyEvent>, assert_fn: F)
where
    F: FnOnce(&StrategyEvent),
{
    assert_eq!(
        events.len(),
        1,
        "expected a single strategy event, got {}",
        events.len()
    );
    assert_fn(&events[0]);
}

/// 统计符合条件的 StrategyEvent 数量并断言等于期望。
pub fn expect_strategy_event_count<F>(events: Vec<StrategyEvent>, matcher: F, expected: usize)
where
    F: Fn(&StrategyEvent) -> bool,
{
    let count = events.iter().filter(|e| matcher(e)).count();
    assert_eq!(
        count, expected,
        "strategy event count mismatch: expected {expected}, got {count}"
    );
}

// ================= TCP Listener 测试辅助 =================
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// 绑定一个本地临时端口并返回 (listener, local_addr)。
pub async fn bind_ephemeral() -> (TcpListener, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral listener");
    let addr = listener.local_addr().expect("listener local addr");
    (listener, addr)
}

/// 启动一个只接受一次连接的任务，并对计数器自增。
pub fn spawn_single_accept(listener: TcpListener, counter: Arc<AtomicUsize>) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            drop(stream);
        }
    })
}

/// 创建只启用 user_static 的池并绑定一个临时端口，返回 (pool, addr, counter, accept_handle)。
pub async fn make_user_static_pool_with_listener(
    host: &str,
) -> (
    fireworks_collaboration_lib::core::ip_pool::IpPool,
    std::net::SocketAddr,
    Arc<AtomicUsize>,
    JoinHandle<()>,
) {
    use fireworks_collaboration_lib::core::ip_pool::config::EffectiveIpPoolConfig;
    use fireworks_collaboration_lib::core::ip_pool::{IpPool, IpScoreCache};
    let (listener, addr) = bind_ephemeral().await;
    let counter = Arc::new(AtomicUsize::new(0));
    let accept = spawn_single_accept(listener, counter.clone());
    let mut cfg: EffectiveIpPoolConfig = enabled_config();
    cfg.runtime.sources.builtin = false;
    cfg.runtime.sources.dns = false;
    cfg.runtime.sources.history = false;
    cfg.runtime.sources.user_static = true;
    cfg.runtime.sources.fallback = false;
    cfg.file.user_static.push(
        fireworks_collaboration_lib::core::ip_pool::config::UserStaticIp {
            host: host.to_string(),
            ip: addr.ip().to_string(),
            ports: vec![addr.port()],
        },
    );
    let pool = IpPool::with_cache(cfg, IpScoreCache::new());
    (pool, addr, counter, accept)
}
