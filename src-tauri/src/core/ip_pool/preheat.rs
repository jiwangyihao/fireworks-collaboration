use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration as StdDuration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use tokio::{
    net::{lookup_host, TcpStream},
    sync::{Notify, Semaphore},
    time::{sleep, Duration, Instant},
};

use super::{
    cache::{IpCacheKey, IpCacheSlot, IpScoreCache, IpStat},
    config::{EffectiveIpPoolConfig, IpPoolRuntimeConfig, PreheatDomain, UserStaticIp},
    history::{IpHistoryRecord, IpHistoryStore},
    IpCandidate, IpSource,
};
use ipnet::IpNet;

/// 判断 IP 是否在名单（支持单 IP 和 CIDR）
fn is_ip_in_list(ip: IpAddr, list: &[String]) -> bool {
    for entry in list {
        if let Ok(net) = entry.parse::<IpNet>() {
            if net.contains(&ip) {
                return true;
            }
        } else if let Ok(addr) = entry.parse::<IpAddr>() {
            if addr == ip {
                return true;
            }
        }
    }
    false
}

/// 最小 TTL，避免出现 0 秒导致刷新频繁自旋。
const MIN_TTL_SECS: u64 = 30;
/// 兜底超时时长毫秒，避免运行时配置给出异常值。
const MAX_PROBE_TIMEOUT_MS: u64 = 10_000;
/// 失败时最大退避倍数，避免无限放大刷新间隔。
const FAILURE_BACKOFF_MULT_MAX: u64 = 6;

#[derive(Debug)]
pub struct PreheatService {
    stop_flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
    thread: Option<thread::JoinHandle<()>>,
}

impl PreheatService {
    pub fn spawn(
        config: Arc<EffectiveIpPoolConfig>,
        cache: Arc<IpScoreCache>,
        history: Arc<IpHistoryStore>,
    ) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());
        let thread_flag = stop_flag.clone();
        let thread_notify = notify.clone();
        let thread_config = config.clone();
        let thread_cache = cache.clone();
        let thread_history = history.clone();
        let handle = thread::Builder::new()
            .name("ip-pool-preheat".into())
            .spawn(move || {
                match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt.block_on(async move {
                        run_preheat_loop(
                            thread_config,
                            thread_cache,
                            thread_history,
                            thread_flag,
                            thread_notify,
                        )
                        .await;
                    }),
                    Err(err) => {
                        tracing::error!(
                            target = "ip_pool",
                            error = %err,
                            "failed to build preheat runtime"
                        );
                    }
                }
            })
            .context("spawn preheat thread")?;

        Ok(Self {
            stop_flag,
            notify,
            thread: Some(handle),
        })
    }

    pub fn request_refresh(&self) {
        self.notify.notify_waiters();
    }
}

impl Drop for PreheatService {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.notify.notify_waiters();
        if let Some(handle) = self.thread.take() {
            if let Err(err) = handle.join() {
                tracing::warn!(
                    target = "ip_pool",
                    "failed to join preheat thread: {:?}",
                    err
                );
            }
        }
    }
}

async fn run_preheat_loop(
    config: Arc<EffectiveIpPoolConfig>,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
    stop: Arc<AtomicBool>,
    notify: Arc<Notify>,
) {
    // 全局回退参数
    let preheat_failure_threshold: u32 = 5; // 连续失败阈值
    let auto_disable_cooldown_ms: i64 = 5 * 60 * 1000; // 5分钟
    // 允许通过 config.runtime 读取自定义阈值，后续可扩展

    // 尝试获取全局 IpPool 实例（假设通过 OnceCell/全局变量注入，或通过 config 传递）
    let ip_pool = crate::core::IP_POOL_GLOBAL.get().and_then(|p| p.upgrade());

    if !config.runtime.enabled {
        tracing::debug!(target = "ip_pool", "ip pool disabled; preheat loop idle");
        wait_for_shutdown(stop, notify).await;
        return;
    }

    if config.file.preheat_domains.is_empty() {
        tracing::info!(
            target = "ip_pool",
            "no preheat domains configured; preheat loop idle"
        );
        wait_for_shutdown(stop, notify).await;
        return;
    }

    let ttl_secs = config.file.score_ttl_seconds.max(MIN_TTL_SECS);
    let base_instant = Instant::now();
    let mut schedules: Vec<DomainSchedule> = config
        .file
        .preheat_domains
        .iter()
        .cloned()
        .map(|domain| DomainSchedule::new(domain, ttl_secs, base_instant))
        .collect();

    tracing::info!(
        target = "ip_pool",
        domains = schedules.len(),
        ttl_secs,
        "starting ip pool preheat loop"
    );


    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // 检查全局 auto-disabled 状态，若处于禁用期则 sleep 并跳过
        if let Some(pool) = ip_pool.as_ref() {
            if let Some(until) = pool.auto_disabled_until() {
                let now = current_epoch_ms();
                if now < until {
                    tracing::warn!(target = "ip_pool", until, "ip pool is auto-disabled, preheat loop sleeping");
                    sleep(Duration::from_millis((until - now).max(1000) as u64)).await;
                    continue;
                } else {
                    // 冷却期已过，自动恢复
                    pool.clear_auto_disabled();
                    crate::core::ip_pool::events::emit_ip_pool_auto_enable();
                }
            }
        }

        let (index, due) = match next_due_schedule(&schedules) {
            Some(entry) => entry,
            None => {
                tracing::warn!(target = "ip_pool", "preheat scheduler empty; stopping loop");
                break;
            }
        };

        let now = Instant::now();
        if due <= now {
            let domain = schedules[index].domain.clone();
            let result = preheat_domain(&domain, config.as_ref(), cache.clone(), history.clone()).await;
            match result {
                Ok(_) => {
                    let instant = Instant::now();
                    schedules[index].mark_success(instant);
                    tracing::debug!(
                        target = "ip_pool",
                        host = domain.host.as_str(),
                        ports = ?domain.ports,
                        next_refresh_secs = schedules[index].ttl().as_secs(),
                        "preheat domain refreshed"
                    );
                }
                Err(err) => {
                    let instant = Instant::now();
                    schedules[index].mark_failure(instant);
                    tracing::warn!(
                        target = "ip_pool",
                        host = domain.host.as_str(),
                        ports = ?domain.ports,
                        error = %err,
                        failure_streak = schedules[index].failure_streak(),
                        backoff_secs = schedules[index].current_backoff().as_secs(),
                        "preheat domain failed; backing off"
                    );
                }
            }

            // 检查所有域名的 failure_streak 是否都超过阈值，若是则触发全局 auto-disable
            let all_failed = schedules.iter().all(|s| s.failure_streak() >= preheat_failure_threshold);
            if all_failed {
                if let Some(pool) = ip_pool.as_ref() {
                    let until = current_epoch_ms() + auto_disable_cooldown_ms;
                    pool.set_auto_disabled(auto_disable_cooldown_ms);
                    crate::core::ip_pool::events::emit_ip_pool_auto_disable(
                        "preheat consecutive failures", until
                    );
                }
                // 进入冷却期，sleep 一段时间
                sleep(Duration::from_millis(auto_disable_cooldown_ms as u64)).await;
                continue;
            }

            if stop.load(Ordering::Relaxed) {
                break;
            }
            continue;
        }

        let sleep_duration = due - now;
        tokio::select! {
            _ = sleep(sleep_duration) => {}
            _ = notify.notified() => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let refresh_instant = Instant::now();
                for schedule in &mut schedules {
                    schedule.force_refresh(refresh_instant);
                }
                tracing::debug!(target = "ip_pool", "preheat refresh requested");
            }
        }
    }

    tracing::info!(target = "ip_pool", "preheat loop stopped");
}

async fn wait_for_shutdown(stop: Arc<AtomicBool>, notify: Arc<Notify>) {
    while !stop.load(Ordering::Relaxed) {
        notify.notified().await;
    }
}

fn next_due_schedule(schedules: &[DomainSchedule]) -> Option<(usize, Instant)> {
    schedules
        .iter()
        .enumerate()
        .min_by_key(|(_, schedule)| schedule.next_due())
        .map(|(idx, schedule)| (idx, schedule.next_due()))
}

#[derive(Debug, Clone)]
struct DomainSchedule {
    domain: PreheatDomain,
    ttl: Duration,
    min_backoff: Duration,
    max_backoff: Duration,
    current_backoff: Duration,
    failure_streak: u32,
    next_due: Instant,
}

impl DomainSchedule {
    fn new(domain: PreheatDomain, ttl_secs: u64, now: Instant) -> Self {
        let ttl_secs = ttl_secs.max(MIN_TTL_SECS);
        let ttl = Duration::from_secs(ttl_secs);
        let max_backoff_secs = ttl_secs
            .saturating_mul(FAILURE_BACKOFF_MULT_MAX)
            .max(ttl_secs);
        let max_backoff = Duration::from_secs(max_backoff_secs);
        Self {
            domain,
            ttl,
            min_backoff: ttl,
            max_backoff,
            current_backoff: ttl,
            failure_streak: 0,
            next_due: now,
        }
    }

    fn mark_success(&mut self, now: Instant) {
        self.failure_streak = 0;
        self.current_backoff = self.min_backoff;
        self.next_due = now + self.ttl;
    }

    fn mark_failure(&mut self, now: Instant) {
        self.failure_streak = self.failure_streak.saturating_add(1);
        let doubled = self
            .current_backoff
            .checked_mul(2)
            .unwrap_or(self.max_backoff);
        self.current_backoff = if doubled > self.max_backoff {
            self.max_backoff
        } else {
            doubled
        };
        self.next_due = now + self.current_backoff;
    }

    fn force_refresh(&mut self, now: Instant) {
        self.failure_streak = 0;
        self.current_backoff = self.min_backoff;
        self.next_due = now;
    }

    fn ttl(&self) -> Duration {
        self.ttl
    }

    fn current_backoff(&self) -> Duration {
        self.current_backoff
    }

    fn failure_streak(&self) -> u32 {
        self.failure_streak
    }

    fn next_due(&self) -> Instant {
        self.next_due
    }
}

async fn preheat_domain(
    domain: &PreheatDomain,
    config: &EffectiveIpPoolConfig,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
) -> Result<()> {
    let host = domain.host.as_str();
    tracing::debug!(target = "ip_pool", host, ports = ?domain.ports, "preheat domain");
    for &port in &domain.ports {
        let candidates = collect_candidates(host, port, config, history.clone()).await;
        if candidates.is_empty() {
            tracing::warn!(target = "ip_pool", host, port, "no candidates collected");
            // Emit failure event
            {
                use crate::core::ip_pool::events::emit_ip_pool_refresh;
                use uuid::Uuid;
                let task_id = Uuid::new_v4();
                emit_ip_pool_refresh(task_id, host, false, &[], "no_candidates".to_string());
            }
            continue;
        }

        let stats = measure_candidates(
            host,
            port,
            candidates,
            &config.runtime,
            config.file.score_ttl_seconds,
        )
        .await;

        if stats.is_empty() {
            tracing::warn!(target = "ip_pool", host, port, "all probes failed");
            cache.remove(host, port);
            // Emit failure event
            {
                use crate::core::ip_pool::events::emit_ip_pool_refresh;
                use uuid::Uuid;
                let task_id = Uuid::new_v4();
                emit_ip_pool_refresh(task_id, host, false, &[], "all_probes_failed".to_string());
            }
            continue;
        }

        update_cache_and_history(host, port, stats.clone(), cache.clone(), history.clone())?;

        // Emit IP pool refresh event for observability
        {
            use crate::core::ip_pool::events::emit_ip_pool_refresh;
            use uuid::Uuid;
            let task_id = Uuid::new_v4();
            emit_ip_pool_refresh(task_id, host, true, &stats, "preheat".to_string());
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(super) struct AggregatedCandidate {
    candidate: IpCandidate,
    sources: HashSet<IpSource>,
}

impl AggregatedCandidate {
    fn new(ip: IpAddr, port: u16, source: IpSource) -> Self {
        let mut sources = HashSet::new();
        sources.insert(source);
        Self {
            candidate: IpCandidate::new(ip, port, source),
            sources,
        }
    }

    fn merge_source(&mut self, source: IpSource) {
        self.sources.insert(source);
    }

    fn to_stat(&self, latency_ms: u32, ttl_secs: u64) -> IpStat {
        let now_ms = current_epoch_ms();
        let expires = now_ms + (ttl_secs as i64 * 1000);
        let mut stat = IpStat::with_latency(self.candidate.clone(), latency_ms);
        stat.sources = self.sources.iter().copied().collect();
        stat.measured_at_epoch_ms = Some(now_ms);
        stat.expires_at_epoch_ms = Some(expires);
        stat
    }
}

pub(super) async fn collect_candidates(
    host: &str,
    port: u16,
    config: &EffectiveIpPoolConfig,
    history: Arc<IpHistoryStore>,
) -> Vec<AggregatedCandidate> {
    let mut map: HashMap<IpAddr, AggregatedCandidate> = HashMap::new();
    let toggles = &config.runtime.sources;

    if toggles.builtin {
        for ip in builtin_lookup(host) {
            map.entry(ip)
                .and_modify(|entry| entry.merge_source(IpSource::Builtin))
                .or_insert_with(|| AggregatedCandidate::new(ip, port, IpSource::Builtin));
        }
    }

    if toggles.user_static {
        for entry in user_static_lookup(host, port, &config.file.user_static) {
            map.entry(entry)
                .and_modify(|agg| agg.merge_source(IpSource::UserStatic))
                .or_insert_with(|| AggregatedCandidate::new(entry, port, IpSource::UserStatic));
        }
    }

    if toggles.history {
        if let Some(record) = history.get_fresh(host, port, current_epoch_ms()) {
            let ip = record.candidate.address;
            map.entry(ip)
                .and_modify(|agg| {
                    for src in record.sources.iter().copied() {
                        agg.merge_source(src);
                    }
                    agg.merge_source(IpSource::History);
                })
                .or_insert_with(|| AggregatedCandidate::new(ip, port, IpSource::History));
        }
    }

    if toggles.dns {
        if let Ok(addrs) = resolve_dns(host, port).await {
            for ip in addrs {
                map.entry(ip)
                    .and_modify(|agg| agg.merge_source(IpSource::Dns))
                    .or_insert_with(|| AggregatedCandidate::new(ip, port, IpSource::Dns));
            }
        }
    }

    if toggles.fallback {
        for ip in fallback_lookup(host) {
            map.entry(ip)
                .and_modify(|agg| agg.merge_source(IpSource::Fallback))
                .or_insert_with(|| AggregatedCandidate::new(ip, port, IpSource::Fallback));
        }
    }

    let mut candidates: Vec<AggregatedCandidate> = map.into_values().collect();
    let whitelist = &config.file.whitelist;
    let blacklist = &config.file.blacklist;
    // 白名单优先
    if !whitelist.is_empty() {
        candidates.retain(|c| {
            let mut allowed = false;
            for cidr in whitelist {
                if is_ip_in_list(c.candidate.address, &[cidr.clone()]) {
                    allowed = true;
                    crate::core::ip_pool::events::emit_ip_pool_cidr_filter(c.candidate.address, "whitelist", cidr);
                    break;
                }
            }
            allowed
        });
    }
    // 黑名单过滤
    if !blacklist.is_empty() {
        let before = candidates.len();
        candidates.retain(|c| {
            let mut blocked = false;
            for cidr in blacklist {
                if is_ip_in_list(c.candidate.address, &[cidr.clone()]) {
                    blocked = true;
                    crate::core::ip_pool::events::emit_ip_pool_cidr_filter(c.candidate.address, "blacklist", cidr);
                    break;
                }
            }
            !blocked
        });
        let after = candidates.len();
        if before != after {
            tracing::info!(target = "ip_pool", removed = before - after, "candidates removed by blacklist");
        }
    }
    candidates
}

pub(super) async fn measure_candidates(
    host: &str,
    _port: u16,
    candidates: Vec<AggregatedCandidate>,
    runtime_cfg: &IpPoolRuntimeConfig,
    ttl_secs: u64,
) -> Vec<IpStat> {
    let max_parallel = runtime_cfg.max_parallel_probes.max(1);
    let timeout = runtime_cfg
        .probe_timeout_ms
        .min(MAX_PROBE_TIMEOUT_MS)
        .max(100);
    let semaphore = Arc::new(Semaphore::new(max_parallel));
    let mut handles = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        let permit = semaphore.clone();
        let timeout_ms = timeout;
        let host = host.to_string();
        handles.push(tokio::spawn(async move {
            let _permit = permit
                .acquire_owned()
                .await
                .map_err(|_| anyhow!("semaphore closed"))?;
            let latency = probe_latency(
                candidate.candidate.address,
                candidate.candidate.port,
                timeout_ms,
            )
            .await?;
            tracing::debug!(
                target = "ip_pool",
                host = host.as_str(),
                port = candidate.candidate.port,
                ip = %candidate.candidate.address,
                latency_ms = latency,
                "probe success"
            );
            Ok::<IpStat, anyhow::Error>(candidate.to_stat(latency, ttl_secs))
        }));
    }

    let mut stats: Vec<IpStat> = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(stat)) => stats.push(stat),
            Ok(Err(err)) => tracing::debug!(target = "ip_pool", error = %err, "probe failed"),
            Err(err) => tracing::debug!(target = "ip_pool", error = %err, "probe join error"),
        }
    }

    stats.sort_by_key(|stat| stat.latency_ms.unwrap_or(u32::MAX));
    stats
}

pub(super) async fn probe_latency(ip: IpAddr, port: u16, timeout_ms: u64) -> Result<u32> {
    let addr = SocketAddr::new(ip, port);
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();
    let stream = tokio::time::timeout(timeout, TcpStream::connect(addr)).await??;
    let elapsed = start.elapsed();
    drop(stream);
    Ok(elapsed.as_millis().min(u128::from(u32::MAX)) as u32)
}

pub(super) fn update_cache_and_history(
    host: &str,
    port: u16,
    stats: Vec<IpStat>,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
) -> Result<()> {
    let best = stats
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("empty stats"))?;
    let alternatives = if stats.len() > 1 {
        stats[1..].to_vec()
    } else {
        Vec::new()
    };
    let slot = IpCacheSlot {
        best: Some(best.clone()),
        alternatives,
    };
    cache.insert(IpCacheKey::new(host.to_string(), port), slot);

    let record = IpHistoryRecord {
        host: host.to_string(),
        port,
        candidate: best.candidate.clone(),
        sources: best.sources.clone(),
        latency_ms: best.latency_ms.unwrap_or_default(),
        measured_at_epoch_ms: best.measured_at_epoch_ms.unwrap_or_else(current_epoch_ms),
        expires_at_epoch_ms: best.expires_at_epoch_ms.unwrap_or_else(current_epoch_ms),
    };
    history.upsert(record)?;
    Ok(())
}

fn builtin_lookup(host: &str) -> Vec<IpAddr> {
    BUILTIN_IPS
        .iter()
        .find(|(h, _)| h.eq_ignore_ascii_case(host))
        .map(|(_, ips)| ips.iter().filter_map(|ip| ip.parse().ok()).collect())
        .unwrap_or_default()
}

fn user_static_lookup(host: &str, port: u16, entries: &[UserStaticIp]) -> Vec<IpAddr> {
    entries
        .iter()
        .filter(|entry| entry.host.eq_ignore_ascii_case(host) && entry.ports.contains(&port))
        .filter_map(|entry| entry.ip.parse().ok())
        .collect()
}

fn fallback_lookup(host: &str) -> Vec<IpAddr> {
    FALLBACK_IPS
        .iter()
        .find(|(h, _)| h.eq_ignore_ascii_case(host))
        .map(|(_, ips)| ips.iter().filter_map(|ip| ip.parse().ok()).collect())
        .unwrap_or_default()
}

async fn resolve_dns(host: &str, port: u16) -> Result<Vec<IpAddr>> {
    let mut addrs = Vec::new();
    let iter = lookup_host((host, port)).await?;
    for addr in iter {
        addrs.push(addr.ip());
    }
    addrs.sort();
    addrs.dedup();
    Ok(addrs)
}

pub(super) fn current_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| StdDuration::from_secs(0))
        .as_millis() as i64
}

const BUILTIN_IPS: &[(&str, &[&str])] = &[
    (
        "github.com",
        &["140.82.112.3", "140.82.113.3", "140.82.114.3"],
    ),
    (
        "codeload.github.com",
        &["140.82.112.9", "140.82.113.9", "140.82.114.9"],
    ),
    (
        "githubusercontent.com",
        &["185.199.108.133", "185.199.110.133", "185.199.111.133"],
    ),
    ("api.github.com", &["140.82.113.6", "140.82.114.6"]),
];

const FALLBACK_IPS: &[(&str, &[&str])] = &[("github.com", &["20.205.243.166"])];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ip_pool::history::IpHistoryStore;
    use tokio::{
        runtime::Builder,
        time::{Duration as TokioDuration, Instant},
    };

    fn test_runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread().enable_all().build().unwrap()
    }

    #[test]
    fn builtin_lookup_returns_known_ips() {
        let ips = builtin_lookup("github.com");
        assert!(!ips.is_empty());
        assert!(ips.iter().any(|ip| ip.is_ipv4()));
    }

    #[test]
    fn user_static_lookup_filters_by_port() {
        let entries = vec![UserStaticIp {
            host: "example.com".into(),
            ip: "1.1.1.1".into(),
            ports: vec![80],
        }];
        assert!(user_static_lookup("example.com", 443, &entries).is_empty());
        let hits = user_static_lookup("example.com", 80, &entries);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn collect_candidates_prefers_configured_sources() {
        let rt = test_runtime();
        let history = Arc::new(IpHistoryStore::in_memory());
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        cfg.runtime.sources.builtin = true;
        cfg.file
            .preheat_domains
            .push(PreheatDomain::new("github.com"));
        let candidates =
            rt.block_on(async { collect_candidates("github.com", 443, &cfg, history).await });
        assert!(!candidates.is_empty());
    }

    #[test]
    fn probe_latency_times_out_reasonably() {
        let rt = test_runtime();
        let result =
            rt.block_on(async { probe_latency("203.0.113.1".parse().unwrap(), 9, 200).await });
        assert!(result.is_err());
    }

    #[test]
    fn update_cache_and_history_writes_best_entry() {
        let cache = Arc::new(IpScoreCache::new());
        let history = Arc::new(IpHistoryStore::in_memory());
        let stat = AggregatedCandidate::new("1.1.1.1".parse().unwrap(), 443, IpSource::Builtin)
            .to_stat(10, 60);
        update_cache_and_history(
            "github.com",
            443,
            vec![stat],
            cache.clone(),
            history.clone(),
        )
        .unwrap();
        assert!(cache.get("github.com", 443).is_some());
        assert!(history.get("github.com", 443).is_some());
    }

    #[test]
    fn domain_schedule_backoff_caps_after_retries() {
        let now = Instant::now();
        let mut schedule = DomainSchedule::new(PreheatDomain::new("example.com"), 120, now);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 240);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 480);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 720);
        schedule.mark_failure(now);
        assert_eq!(schedule.current_backoff().as_secs(), 720);
        schedule.mark_success(now);
        assert_eq!(schedule.current_backoff().as_secs(), 120);
    }

    #[test]
    fn domain_schedule_force_refresh_resets_state() {
        let now = Instant::now();
        let mut schedule = DomainSchedule::new(PreheatDomain::new("refresh.com"), 300, now);
        schedule.mark_failure(now);
        assert!(schedule.failure_streak() > 0);
        let later = now + TokioDuration::from_secs(5);
        schedule.force_refresh(later);
        assert_eq!(schedule.failure_streak(), 0);
        assert_eq!(schedule.current_backoff().as_secs(), 300);
        assert_eq!(schedule.next_due(), later);
    }

    #[test]
    fn next_due_schedule_selects_earliest_entry() {
        let base = Instant::now();
        let mut first = DomainSchedule::new(PreheatDomain::new("early.com"), 120, base);
        first.mark_success(base);
        let mut second = DomainSchedule::new(PreheatDomain::new("now.com"), 120, base);
        second.force_refresh(base);
        let schedules = vec![first.clone(), second.clone()];
        let (idx, due) = next_due_schedule(&schedules).expect("schedule entry");
        assert_eq!(schedules[idx].domain.host, "now.com");
        assert_eq!(due, schedules[idx].next_due());
        assert!(due <= first.next_due());
    }

    #[test]
    fn collect_candidates_merges_sources_from_history() {
        let rt = test_runtime();
        let history = Arc::new(IpHistoryStore::in_memory());
        let ip: IpAddr = "140.82.112.3".parse().unwrap();
        let future_expire = current_epoch_ms() + 60_000;
        let record = IpHistoryRecord {
            host: "github.com".into(),
            port: 443,
            candidate: IpCandidate::new(ip, 443, IpSource::History),
            sources: vec![IpSource::History, IpSource::UserStatic],
            latency_ms: 12,
            measured_at_epoch_ms: future_expire - 60_000,
            expires_at_epoch_ms: future_expire,
        };
        history.upsert(record).unwrap();
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        cfg.runtime.sources.builtin = true;
        cfg.runtime.sources.history = true;
        cfg.runtime.sources.dns = false;
        cfg.runtime.sources.user_static = false;
        cfg.runtime.sources.fallback = false;
        let candidates = rt
            .block_on(async { collect_candidates("github.com", 443, &cfg, history.clone()).await });
        let merged = candidates
            .into_iter()
            .find(|candidate| candidate.candidate.address == ip)
            .expect("candidate merged from history and builtin");
        assert!(merged.sources.contains(&IpSource::History));
        assert!(merged.sources.contains(&IpSource::Builtin));
        assert!(merged.sources.contains(&IpSource::UserStatic));
        assert!(history.get("github.com", 443).is_some());
    }

    #[test]
    fn collect_candidates_skips_expired_history_entries() {
        let rt = test_runtime();
        let history = Arc::new(IpHistoryStore::in_memory());
        let mut cfg = EffectiveIpPoolConfig::default();
        cfg.runtime.enabled = true;
        cfg.runtime.sources.builtin = false;
        cfg.runtime.sources.dns = false;
        cfg.runtime.sources.user_static = false;
        cfg.runtime.sources.fallback = false;
        cfg.runtime.sources.history = true;
        let record = IpHistoryRecord {
            host: "expired.test".into(),
            port: 443,
            candidate: IpCandidate::new("1.1.1.1".parse().unwrap(), 443, IpSource::History),
            sources: vec![IpSource::History],
            latency_ms: 10,
            measured_at_epoch_ms: 1,
            expires_at_epoch_ms: 2,
        };
        history.upsert(record).unwrap();
        let candidates = rt.block_on(async {
            collect_candidates("expired.test", 443, &cfg, history.clone()).await
        });
        assert!(candidates.is_empty());
        assert!(history.get("expired.test", 443).is_none());
    }
}
