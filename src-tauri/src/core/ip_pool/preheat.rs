use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration as StdDuration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};

use hyper::{Body, Request, Version};
use rustls::ServerName;
use tokio::{
    net::TcpStream,
    sync::{Notify, Semaphore},
    task::JoinSet,
    time::{sleep, timeout, Duration, Instant},
};
use tokio_rustls::TlsConnector;

use super::{
    cache::{IpCacheKey, IpCacheSlot, IpScoreCache, IpStat},
    config::{
        DnsRuntimeConfig, EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig,
        PreheatDomain, ProbeMethod, UserStaticIp,
    },
    dns::{self, DnsResolvedIp},
    history::{IpHistoryRecord, IpHistoryStore},
    IpCandidate, IpSource,
};
use crate::core::config::model::TlsCfg;
use crate::core::tls::verifier::create_client_config_with_expected_name;
use ipnet::IpNet;

pub type ResolverFn = Arc<
    dyn Fn(&str, u16, &DnsRuntimeConfig) -> BoxFuture<'static, Result<Vec<DnsResolvedIp>>>
        + Send
        + Sync,
>;

pub type ProberFn = Arc<
    dyn Fn(IpAddr, u16, &str, &str, &str, u64, ProbeMethod) -> BoxFuture<'static, Result<u32>>
        + Send
        + Sync,
>;

use futures::future::BoxFuture;

pub fn default_dns_resolver() -> ResolverFn {
    Arc::new(|host, port, cfg| {
        let host = host.to_string();
        let cfg = cfg.clone();
        Box::pin(async move { dns::resolve(&host, port, &cfg).await })
    })
}

pub fn default_latency_prober() -> ProberFn {
    Arc::new(|ip, port, host, sni, path, timeout, method| {
        let host = host.to_string();
        let sni = sni.to_string();
        let path = path.to_string();
        Box::pin(
            async move { probe_latency_impl(ip, port, &host, &sni, &path, timeout, method).await },
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateChannel {
    Builtin,
    UserStatic,
    History,
    Dns,
    Fallback,
}

impl CandidateChannel {
    fn label(self) -> &'static str {
        match self {
            CandidateChannel::Builtin => "builtin",
            CandidateChannel::UserStatic => "user_static",
            CandidateChannel::History => "history",
            CandidateChannel::Dns => "dns",
            CandidateChannel::Fallback => "fallback",
        }
    }
}

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
/// 快速路径等待窗口上限（毫秒）。
const FAST_WAIT_DEFAULT_MS: u64 = 1_200;
/// 快速路径等待窗口下限（毫秒）。
const FAST_WAIT_MIN_MS: u64 = 300;
/// 触发快速结束的延迟阈值（毫秒）。
const FAST_LATENCY_THRESHOLD_MS: u32 = 200;

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
        dns_resolver: Option<ResolverFn>,
        latency_prober: Option<ProberFn>,
    ) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Notify::new());
        let thread_flag = stop_flag.clone();
        let thread_notify = notify.clone();
        let thread_config = config.clone();
        let thread_cache = cache.clone();
        let thread_history = history.clone();

        let resolver = dns_resolver.unwrap_or_else(default_dns_resolver);
        let prober = latency_prober.unwrap_or_else(default_latency_prober);

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
                            resolver,
                            prober,
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
    resolver: ResolverFn,
    prober: ProberFn,
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

    let preheat_domains = resolve_preheat_domains(&config.file);
    if preheat_domains.is_empty() {
        tracing::info!(
            target = "ip_pool",
            "no preheat domains configured; preheat loop idle"
        );
        wait_for_shutdown(stop, notify).await;
        return;
    }

    let ttl_secs = config.file.score_ttl_seconds.max(MIN_TTL_SECS);
    let base_instant = Instant::now();
    let mut schedules: Vec<DomainSchedule> = preheat_domains
        .into_iter()
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
                    tracing::warn!(
                        target = "ip_pool",
                        until,
                        "ip pool is auto-disabled, preheat loop sleeping"
                    );
                    sleep(Duration::from_millis((until - now).max(1000) as u64)).await;
                    continue;
                } else {
                    // 冷却期已过，自动恢复
                    if pool.clear_auto_disabled() {
                        tracing::info!(
                            target = "ip_pool",
                            "ip pool auto-disable cooldown elapsed; re-enabled"
                        );
                    }
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
            let result = preheat_domain(
                &domain,
                config.as_ref(),
                cache.clone(),
                history.clone(),
                resolver.clone(),
                prober.clone(),
            )
            .await;
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
            let all_failed = schedules
                .iter()
                .all(|s| s.failure_streak() >= preheat_failure_threshold);
            if all_failed {
                if let Some(pool) = ip_pool.as_ref() {
                    pool.set_auto_disabled(
                        "preheat consecutive failures",
                        auto_disable_cooldown_ms,
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

pub fn next_due_schedule(schedules: &[DomainSchedule]) -> Option<(usize, Instant)> {
    schedules
        .iter()
        .enumerate()
        .min_by_key(|(_, schedule)| schedule.next_due())
        .map(|(idx, schedule)| (idx, schedule.next_due()))
}

#[derive(Debug, Clone)]
pub struct DomainSchedule {
    pub domain: PreheatDomain,
    ttl: Duration,
    min_backoff: Duration,
    max_backoff: Duration,
    current_backoff: Duration,
    failure_streak: u32,
    next_due: Instant,
}

impl DomainSchedule {
    pub fn new(domain: PreheatDomain, ttl_secs: u64, now: Instant) -> Self {
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

    pub fn mark_success(&mut self, now: Instant) {
        self.failure_streak = 0;
        self.current_backoff = self.min_backoff;
        self.next_due = now + self.ttl;
    }

    pub fn mark_failure(&mut self, now: Instant) {
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

    pub fn force_refresh(&mut self, now: Instant) {
        self.failure_streak = 0;
        self.current_backoff = self.min_backoff;
        self.next_due = now;
    }

    fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn current_backoff(&self) -> Duration {
        self.current_backoff
    }

    pub fn failure_streak(&self) -> u32 {
        self.failure_streak
    }

    pub fn next_due(&self) -> Instant {
        self.next_due
    }
}

async fn preheat_domain(
    domain: &PreheatDomain,
    config: &EffectiveIpPoolConfig,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
    resolver: ResolverFn,
    prober: ProberFn,
) -> Result<()> {
    let host = domain.host.as_str();
    tracing::debug!(target = "ip_pool", host, ports = ?domain.ports, "preheat domain");
    for &port in &domain.ports {
        let toggles = &config.runtime.sources;
        let runtime_cfg = config.runtime.clone();
        let file_cfg = config.file.clone();
        let ttl_secs = file_cfg.score_ttl_seconds;
        let whitelist = file_cfg.whitelist.clone();
        let blacklist = file_cfg.blacklist.clone();

        let mut candidate_map: HashMap<IpAddr, AggregatedCandidate> = HashMap::new();
        let mut join_set: JoinSet<(CandidateChannel, Result<Vec<AggregatedCandidate>>)> =
            JoinSet::new();
        let mut task_count = 0usize;
        let host_owned = host.to_string();

        if toggles.builtin {
            let host_clone = host_owned.clone();
            join_set.spawn(async move {
                (
                    CandidateChannel::Builtin,
                    gather_builtin_candidates(host_clone.as_str(), port),
                )
            });
            task_count += 1;
        }

        if toggles.user_static {
            let host_clone = host_owned.clone();
            let entries = file_cfg.user_static.clone();
            join_set.spawn(async move {
                (
                    CandidateChannel::UserStatic,
                    gather_user_static_candidates(host_clone.as_str(), port, &entries),
                )
            });
            task_count += 1;
        }

        if toggles.history {
            let host_clone = host_owned.clone();
            let history_clone = history.clone();
            join_set.spawn(async move {
                (
                    CandidateChannel::History,
                    gather_history_candidates(host_clone.as_str(), port, history_clone),
                )
            });
            task_count += 1;
        }

        if toggles.dns {
            let host_clone = host_owned.clone();
            let dns_cfg = runtime_cfg.dns.clone();
            let resolver_clone = resolver.clone();
            join_set.spawn(async move {
                (
                    CandidateChannel::Dns,
                    gather_dns_candidates(host_clone.as_str(), port, &dns_cfg, resolver_clone)
                        .await,
                )
            });
            task_count += 1;
        }

        if toggles.fallback {
            let host_clone = host_owned.clone();
            join_set.spawn(async move {
                (
                    CandidateChannel::Fallback,
                    gather_fallback_candidates(host_clone.as_str(), port),
                )
            });
            task_count += 1;
        }

        if task_count == 0 {
            tracing::warn!(
                target = "ip_pool",
                host,
                port,
                "no candidate sources enabled for preheat"
            );
            continue;
        }

        let fast_wait_ms = runtime_cfg
            .probe_timeout_ms
            .min(FAST_WAIT_DEFAULT_MS)
            .max(FAST_WAIT_MIN_MS);
        let wait_deadline = Instant::now() + Duration::from_millis(fast_wait_ms);
        let mut waiting = true;
        let mut fast_path_hit = false;
        let mut timeout_triggered = false;
        let mut measured_once = false;

        while !join_set.is_empty() {
            let join_item = if waiting {
                let now = Instant::now();
                if now >= wait_deadline {
                    waiting = false;
                    timeout_triggered = true;
                    None
                } else {
                    match timeout(wait_deadline - now, join_set.join_next()).await {
                        Ok(item) => item,
                        Err(_) => {
                            waiting = false;
                            timeout_triggered = true;
                            None
                        }
                    }
                }
            } else {
                join_set.join_next().await
            };

            let Some(join_result) = join_item else {
                break;
            };

            match join_result {
                Ok((channel, Ok(batch))) => {
                    if batch.is_empty() {
                        tracing::debug!(
                            target = "ip_pool",
                            host,
                            port,
                            channel = channel.label(),
                            "candidate channel returned no entries"
                        );
                        continue;
                    }

                    tracing::debug!(
                        target = "ip_pool",
                        host,
                        port,
                        channel = channel.label(),
                        count = batch.len(),
                        "collected candidates"
                    );

                    let merged = merge_candidate_map(&mut candidate_map, batch);
                    let filtered = apply_cidr_filters(&mut candidate_map, &whitelist, &blacklist);

                    if merged || filtered {
                        let snapshot: Vec<AggregatedCandidate> =
                            candidate_map.values().cloned().collect();
                        match measure_and_update_candidates(
                            host,
                            port,
                            snapshot,
                            &runtime_cfg,
                            ttl_secs,
                            cache.clone(),
                            history.clone(),
                            true,
                            prober.clone(),
                        )
                        .await
                        {
                            Ok(stats) => {
                                measured_once = true;
                                use crate::core::ip_pool::events::emit_ip_pool_refresh;
                                use uuid::Uuid;
                                let task_id = Uuid::new_v4();
                                if stats.is_empty() {
                                    emit_ip_pool_refresh(
                                        task_id,
                                        host,
                                        false,
                                        &[],
                                        "all_probes_failed".to_string(),
                                    );
                                } else {
                                    emit_ip_pool_refresh(
                                        task_id,
                                        host,
                                        true,
                                        &stats,
                                        "preheat".to_string(),
                                    );

                                    if waiting {
                                        if let Some(latency) =
                                            stats.first().and_then(|s| s.latency_ms)
                                        {
                                            if latency < FAST_LATENCY_THRESHOLD_MS {
                                                fast_path_hit = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                tracing::warn!(
                                    target = "ip_pool",
                                    host,
                                    port,
                                    error = %err,
                                    "failed to measure candidates"
                                );
                            }
                        }
                    }
                }
                Ok((channel, Err(err))) => {
                    tracing::warn!(
                        target = "ip_pool",
                        host,
                        port,
                        channel = channel.label(),
                        error = %err,
                        "candidate channel failed"
                    );
                }
                Err(join_err) => {
                    tracing::debug!(
                        target = "ip_pool",
                        host,
                        port,
                        error = %join_err,
                        "candidate channel join error"
                    );
                }
            }
        }

        if !measured_once && candidate_map.is_empty() {
            tracing::warn!(target = "ip_pool", host, port, "no candidates collected");
            use crate::core::ip_pool::events::emit_ip_pool_refresh;
            use uuid::Uuid;
            let task_id = Uuid::new_v4();
            emit_ip_pool_refresh(task_id, host, false, &[], "no_candidates".to_string());
        }

        let pending = !join_set.is_empty();
        if pending && (fast_path_hit || timeout_triggered) {
            let host_bg = host_owned.clone();
            let runtime_bg = runtime_cfg.clone();
            let whitelist_bg = whitelist.clone();
            let blacklist_bg = blacklist.clone();
            let cache_bg = cache.clone();
            let history_bg = history.clone();
            let prober_bg = prober.clone();
            tokio::spawn(async move {
                drain_remaining_candidates(
                    join_set,
                    candidate_map,
                    host_bg,
                    port,
                    runtime_bg,
                    ttl_secs,
                    whitelist_bg,
                    blacklist_bg,
                    cache_bg,
                    history_bg,
                    prober_bg,
                )
                .await;
            });
            continue;
        }

        // Ensure pending tasks are fully consumed when we decide not to hand them off.
        while let Some(result) = join_set.join_next().await {
            if let Ok((channel, Err(err))) = result {
                tracing::warn!(
                    target = "ip_pool",
                    host,
                    port,
                    channel = channel.label(),
                    error = %err,
                    "candidate channel failed during finalization"
                );
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AggregatedCandidate {
    pub candidate: IpCandidate,
    pub sources: HashSet<IpSource>,
    pub resolver_tags: HashSet<String>,
}

impl AggregatedCandidate {
    pub fn new(ip: IpAddr, port: u16, source: IpSource) -> Self {
        let mut sources = HashSet::new();
        sources.insert(source);
        Self {
            candidate: IpCandidate::new(ip, port, source),
            sources,
            resolver_tags: HashSet::new(),
        }
    }

    fn merge_source(&mut self, source: IpSource) {
        self.sources.insert(source);
    }

    fn merge_resolver_tag<S: Into<String>>(&mut self, tag: S) {
        self.resolver_tags.insert(tag.into());
    }

    fn merge_from(&mut self, mut other: AggregatedCandidate) -> bool {
        let mut changed = false;
        for source in other.sources.drain() {
            if self.sources.insert(source) {
                changed = true;
            }
        }
        for tag in other.resolver_tags.drain() {
            if self.resolver_tags.insert(tag) {
                changed = true;
            }
        }
        changed
    }

    pub fn to_stat(&self, latency_ms: u32, ttl_secs: u64) -> IpStat {
        let now_ms = current_epoch_ms();
        let expires = now_ms + (ttl_secs as i64 * 1000);
        let mut stat = IpStat::with_latency(self.candidate.clone(), latency_ms);
        stat.sources = self.sources.iter().copied().collect();
        if !self.resolver_tags.is_empty() {
            let mut tags: Vec<String> = self.resolver_tags.iter().cloned().collect();
            tags.sort();
            stat.resolver_metadata = tags;
        }
        stat.measured_at_epoch_ms = Some(now_ms);
        stat.expires_at_epoch_ms = Some(expires);
        stat
    }
}

fn merge_candidate_map(
    map: &mut HashMap<IpAddr, AggregatedCandidate>,
    incoming: Vec<AggregatedCandidate>,
) -> bool {
    let mut changed = false;
    for candidate in incoming {
        let address = candidate.candidate.address;
        match map.entry(address) {
            Entry::Vacant(entry) => {
                entry.insert(candidate);
                changed = true;
            }
            Entry::Occupied(mut entry) => {
                if entry.get_mut().merge_from(candidate) {
                    changed = true;
                }
            }
        }
    }
    changed
}

fn apply_cidr_filters(
    map: &mut HashMap<IpAddr, AggregatedCandidate>,
    whitelist: &[String],
    blacklist: &[String],
) -> bool {
    let mut changed = false;

    if !whitelist.is_empty() {
        let mut allowed: HashSet<IpAddr> = HashSet::new();
        for (&ip, _) in map.iter() {
            for cidr in whitelist {
                if is_ip_in_list(ip, &[cidr.clone()]) {
                    crate::core::ip_pool::events::emit_ip_pool_cidr_filter(ip, "whitelist", cidr);
                    allowed.insert(ip);
                    break;
                }
            }
        }

        let before = map.len();
        map.retain(|ip, _| allowed.contains(ip));
        if map.len() != before {
            changed = true;
        }
    }

    if !blacklist.is_empty() {
        let mut to_remove: Vec<IpAddr> = Vec::new();
        for (&ip, _) in map.iter() {
            for cidr in blacklist {
                if is_ip_in_list(ip, &[cidr.clone()]) {
                    crate::core::ip_pool::events::emit_ip_pool_cidr_filter(ip, "blacklist", cidr);
                    to_remove.push(ip);
                    break;
                }
            }
        }

        if !to_remove.is_empty() {
            let mut removed = 0_usize;
            for ip in to_remove {
                if map.remove(&ip).is_some() {
                    removed += 1;
                }
            }
            if removed > 0 {
                tracing::info!(
                    target = "ip_pool",
                    removed,
                    "candidates removed by blacklist"
                );
                changed = true;
            }
        }
    }

    changed
}

fn gather_builtin_candidates(host: &str, port: u16) -> Result<Vec<AggregatedCandidate>> {
    let candidates = builtin_lookup(host)
        .into_iter()
        .map(|ip| AggregatedCandidate::new(ip, port, IpSource::Builtin))
        .collect();
    Ok(candidates)
}

fn gather_user_static_candidates(
    host: &str,
    port: u16,
    entries: &[UserStaticIp],
) -> Result<Vec<AggregatedCandidate>> {
    let candidates = user_static_lookup(host, port, entries)
        .into_iter()
        .map(|ip| AggregatedCandidate::new(ip, port, IpSource::UserStatic))
        .collect();
    Ok(candidates)
}

fn gather_history_candidates(
    host: &str,
    port: u16,
    history: Arc<IpHistoryStore>,
) -> Result<Vec<AggregatedCandidate>> {
    let mut items = Vec::new();
    if let Some(record) = history.get_fresh(host, port, current_epoch_ms()) {
        let mut candidate =
            AggregatedCandidate::new(record.candidate.address, port, IpSource::History);
        for src in record.sources {
            candidate.merge_source(src);
        }
        for tag in record.resolver_metadata {
            candidate.merge_resolver_tag(tag);
        }
        items.push(candidate);
    }
    Ok(items)
}

async fn gather_dns_candidates(
    host: &str,
    port: u16,
    runtime_dns: &DnsRuntimeConfig,
    resolver: ResolverFn,
) -> Result<Vec<AggregatedCandidate>> {
    let mut items = Vec::new();
    for DnsResolvedIp { ip, label } in resolver(host, port, runtime_dns).await? {
        let mut candidate = AggregatedCandidate::new(ip, port, IpSource::Dns);
        if let Some(tag) = label {
            candidate.merge_resolver_tag(tag);
        }
        items.push(candidate);
    }
    Ok(items)
}

fn gather_fallback_candidates(host: &str, port: u16) -> Result<Vec<AggregatedCandidate>> {
    let candidates = fallback_lookup(host)
        .into_iter()
        .map(|ip| AggregatedCandidate::new(ip, port, IpSource::Fallback))
        .collect();
    Ok(candidates)
}

async fn measure_and_update_candidates(
    host: &str,
    port: u16,
    candidates: Vec<AggregatedCandidate>,
    runtime_cfg: &IpPoolRuntimeConfig,
    ttl_secs: u64,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
    with_progress: bool,
    prober: ProberFn,
) -> Result<Vec<IpStat>> {
    if candidates.is_empty() {
        cache.remove(host, port);
        return Ok(Vec::new());
    }

    let stats = if with_progress {
        let cache_progress = cache.clone();
        let history_progress = history.clone();
        let host_owned = host.to_string();
        let mut progress = move |current: &[IpStat]| -> Result<()> {
            if current.is_empty() {
                return Ok(());
            }
            let snapshot: Vec<IpStat> = current.iter().cloned().collect();
            update_cache_and_history(
                host_owned.as_str(),
                port,
                snapshot,
                cache_progress.clone(),
                history_progress.clone(),
            )
        };

        measure_candidates(
            host,
            port,
            candidates,
            runtime_cfg,
            ttl_secs,
            Some(&mut progress),
            prober,
        )
        .await
    } else {
        measure_candidates(host, port, candidates, runtime_cfg, ttl_secs, None, prober).await
    };

    if stats.is_empty() {
        cache.remove(host, port);
        return Ok(stats);
    }

    update_cache_and_history(host, port, stats.clone(), cache, history)?;
    Ok(stats)
}

async fn drain_remaining_candidates(
    mut join_set: JoinSet<(CandidateChannel, Result<Vec<AggregatedCandidate>>)>,
    mut candidates: HashMap<IpAddr, AggregatedCandidate>,
    host: String,
    port: u16,
    runtime_cfg: IpPoolRuntimeConfig,
    ttl_secs: u64,
    whitelist: Vec<String>,
    blacklist: Vec<String>,
    cache: Arc<IpScoreCache>,
    history: Arc<IpHistoryStore>,
    prober: ProberFn,
) {
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((channel, Ok(batch))) => {
                if batch.is_empty() {
                    tracing::debug!(
                        target = "ip_pool",
                        host = host.as_str(),
                        port,
                        channel = channel.label(),
                        "candidate channel returned no entries"
                    );
                    continue;
                }

                tracing::debug!(
                    target = "ip_pool",
                    host = host.as_str(),
                    port,
                    channel = channel.label(),
                    count = batch.len(),
                    "received additional candidates"
                );

                let merged = merge_candidate_map(&mut candidates, batch);
                let filtered = apply_cidr_filters(&mut candidates, &whitelist, &blacklist);

                if merged || filtered {
                    let snapshot: Vec<AggregatedCandidate> = candidates.values().cloned().collect();
                    match measure_and_update_candidates(
                        host.as_str(),
                        port,
                        snapshot,
                        &runtime_cfg,
                        ttl_secs,
                        cache.clone(),
                        history.clone(),
                        false,
                        prober.clone(),
                    )
                    .await
                    {
                        Ok(stats) => {
                            use crate::core::ip_pool::events::emit_ip_pool_refresh;
                            use uuid::Uuid;
                            let task_id = Uuid::new_v4();
                            if stats.is_empty() {
                                emit_ip_pool_refresh(
                                    task_id,
                                    host.as_str(),
                                    false,
                                    &[],
                                    "preheat_background_all_failed".to_string(),
                                );
                            } else {
                                emit_ip_pool_refresh(
                                    task_id,
                                    host.as_str(),
                                    true,
                                    &stats,
                                    "preheat_background".to_string(),
                                );
                            }
                        }
                        Err(err) => {
                            tracing::warn!(
                                target = "ip_pool",
                                host = host.as_str(),
                                port,
                                error = %err,
                                "failed to refresh candidates during background collection"
                            );
                        }
                    }
                }
            }
            Ok((channel, Err(err))) => {
                tracing::warn!(
                    target = "ip_pool",
                    host = host.as_str(),
                    port,
                    channel = channel.label(),
                    error = %err,
                    "candidate channel failed during background collection"
                );
            }
            Err(join_err) => {
                tracing::debug!(
                    target = "ip_pool",
                    host = host.as_str(),
                    port,
                    error = %join_err,
                    "candidate channel join error during background collection"
                );
            }
        }
    }
}

pub async fn collect_candidates(
    host: &str,
    port: u16,
    config: &EffectiveIpPoolConfig,
    history: Arc<IpHistoryStore>,
) -> Vec<AggregatedCandidate> {
    let mut map: HashMap<IpAddr, AggregatedCandidate> = HashMap::new();
    let toggles = &config.runtime.sources;

    if toggles.builtin {
        if let Ok(batch) = gather_builtin_candidates(host, port) {
            merge_candidate_map(&mut map, batch);
        }
    }

    if toggles.user_static {
        if let Ok(batch) = gather_user_static_candidates(host, port, &config.file.user_static) {
            merge_candidate_map(&mut map, batch);
        }
    }

    if toggles.history {
        if let Ok(batch) = gather_history_candidates(host, port, history.clone()) {
            merge_candidate_map(&mut map, batch);
        }
    }

    if toggles.dns {
        let resolver = default_dns_resolver();
        match gather_dns_candidates(host, port, &config.runtime.dns, resolver).await {
            Ok(batch) => {
                merge_candidate_map(&mut map, batch);
            }
            Err(err) => {
                tracing::debug!(
                    target = "ip_pool",
                    host,
                    port,
                    error = %err,
                    "dns candidate collection failed"
                );
            }
        }
    }

    if toggles.fallback {
        if let Ok(batch) = gather_fallback_candidates(host, port) {
            merge_candidate_map(&mut map, batch);
        }
    }

    apply_cidr_filters(&mut map, &config.file.whitelist, &config.file.blacklist);
    map.into_values().collect()
}

pub(super) async fn measure_candidates(
    host: &str,
    port: u16,
    candidates: Vec<AggregatedCandidate>,
    runtime_cfg: &IpPoolRuntimeConfig,
    ttl_secs: u64,
    mut progress: Option<&mut (dyn FnMut(&[IpStat]) -> Result<()> + Send)>,
    prober: ProberFn,
) -> Vec<IpStat> {
    let max_parallel = runtime_cfg.max_parallel_probes.max(1);
    let timeout = runtime_cfg
        .probe_timeout_ms
        .clamp(100, MAX_PROBE_TIMEOUT_MS);
    let semaphore = Arc::new(Semaphore::new(max_parallel));
    let mut join_set = JoinSet::new();

    // 获取探测配置
    let probe_method = runtime_cfg.probe_method;
    let probe_path = runtime_cfg.probe_path.clone();

    for candidate in candidates {
        let permit = semaphore.clone();
        let timeout_ms = timeout;
        let host_label = host.to_string();
        let host_for_probe = host.to_string();
        let sni_host = host.to_string(); // 对于延迟测试，使用真实 host 作为 SNI
        let path = probe_path.clone();
        let method = probe_method;
        let prober_task = prober.clone();
        join_set.spawn(async move {
            let _permit = permit
                .acquire_owned()
                .await
                .map_err(|_| anyhow!("semaphore closed"))?;
            let latency = prober_task(
                candidate.candidate.address,
                candidate.candidate.port,
                &host_for_probe,
                &sni_host,
                &path,
                timeout_ms,
                method,
            )
            .await?;
            tracing::debug!(
                target = "ip_pool",
                host = host_label.as_str(),
                port = candidate.candidate.port,
                ip = %candidate.candidate.address,
                latency_ms = latency,
                method = ?method,
                "probe success"
            );
            Ok::<IpStat, anyhow::Error>(candidate.to_stat(latency, ttl_secs))
        });
    }

    let mut stats: Vec<IpStat> = Vec::new();
    let mut last_notified: Option<(u32, IpAddr)> = None;

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(stat)) => {
                stats.push(stat);
                stats.sort_by_key(|item| item.latency_ms.unwrap_or(u32::MAX));

                if let Some(callback) = progress.as_mut() {
                    if let Some(best) = stats.first() {
                        let current = (best.latency_ms.unwrap_or(u32::MAX), best.candidate.address);
                        let should_notify = match last_notified {
                            None => true,
                            Some(prev) => current.0 < prev.0 || current.1 != prev.1,
                        };
                        if should_notify {
                            if let Err(err) = callback(&stats) {
                                tracing::warn!(
                                    target = "ip_pool",
                                    host,
                                    port,
                                    error = %err,
                                    "failed to apply preheat progress update"
                                );
                            } else {
                                last_notified = Some(current);
                            }
                        }
                    }
                }
            }
            Ok(Err(err)) => tracing::debug!(
                target = "ip_pool",
                host,
                port,
                error = %err,
                "probe failed"
            ),
            Err(err) => tracing::debug!(
                target = "ip_pool",
                host,
                port,
                error = %err,
                "probe join error"
            ),
        }
    }

    stats.sort_by_key(|item| item.latency_ms.unwrap_or(u32::MAX));
    stats
}

/// TCP 握手延迟测试（传统方式，TUN 模式下可能返回本地延迟）
pub async fn probe_latency_tcp(ip: IpAddr, port: u16, timeout_ms: u64) -> Result<u32> {
    let addr = SocketAddr::new(ip, port);
    let timeout = Duration::from_millis(timeout_ms);
    let start = Instant::now();
    let stream = tokio::time::timeout(timeout, TcpStream::connect(addr)).await??;
    let elapsed = start.elapsed();
    drop(stream);
    Ok(elapsed.as_millis().min(u128::from(u32::MAX)) as u32)
}

/// HTTPing 延迟测试：使用 HTTPS HEAD 请求测量应用层延迟
///
/// 此方法通过发送实际的 HTTP 请求来测量延迟，可以在 TUN 模式下获得真实的延迟值，
/// 而不会被代理软件的本地 TCP 握手劫持。
///
/// # Arguments
/// * `ip` - 目标 IP 地址
/// * `port` - 目标端口（通常是 443）
/// * `host` - 用于 Host 头和 TLS 证书验证的主机名
/// * `sni_host` - 用于 TLS SNI 的主机名（可以是伪装的 SNI）
/// * `path` - HTTP 请求路径（默认 "/"）
/// * `timeout_ms` - 超时时间（毫秒）
pub async fn probe_latency_http(
    ip: IpAddr,
    port: u16,
    host: &str,
    sni_host: &str,
    path: &str,
    timeout_ms: u64,
) -> Result<u32> {
    let addr = SocketAddr::new(ip, port);
    let timeout_duration = Duration::from_millis(timeout_ms);

    // 1. 建立 TCP 连接
    let tcp_stream = timeout(timeout_duration, TcpStream::connect(addr))
        .await
        .context("tcp connect timeout")?
        .context("tcp connect failed")?;

    // 2. 配置 TLS，使用指定的 SNI 但验证真实主机名的证书
    let tls_cfg = TlsCfg {
        spki_pins: Vec::new(),
        metrics_enabled: true,
        cert_fp_log_enabled: false, // 探测时不需要记录证书指纹
        cert_fp_max_bytes: 0,
    };
    let tls_config = Arc::new(create_client_config_with_expected_name(&tls_cfg, host));
    let connector = TlsConnector::from(tls_config);

    let server_name = ServerName::try_from(sni_host)
        .map_err(|_| anyhow!("invalid sni hostname: {}", sni_host))?;

    // 3. TLS 握手
    let tls_stream = timeout(timeout_duration, connector.connect(server_name, tcp_stream))
        .await
        .context("tls handshake timeout")?
        .context("tls handshake failed")?;

    // 4. HTTP/1.1 握手
    let (mut sender, conn) = timeout(timeout_duration, hyper::client::conn::handshake(tls_stream))
        .await
        .context("http handshake timeout")?
        .context("http handshake failed")?;

    // 后台驱动连接
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::trace!(target = "ip_pool", "httping conn ended: {:?}", e);
        }
    });

    // 5. 发送 HEAD 请求并计时
    let uri = format!("https://{}{}", host, path);
    let req = Request::builder()
        .method("HEAD")
        .uri(&uri)
        .version(Version::HTTP_11)
        .header("Host", host)
        .header("User-Agent", "fireworks-httping/1.0")
        .header("Connection", "close")
        .body(Body::empty())
        .context("build http request")?;

    let start = Instant::now();
    let _resp = timeout(timeout_duration, sender.send_request(req))
        .await
        .context("http request timeout")?
        .context("http request failed")?;
    let elapsed = start.elapsed();

    Ok(elapsed.as_millis().min(u128::from(u32::MAX)) as u32)
}

/// 根据配置选择的探测方法测量延迟
pub async fn probe_latency_impl(
    ip: IpAddr,
    port: u16,
    host: &str,
    sni_host: &str,
    path: &str,
    timeout_ms: u64,
    method: ProbeMethod,
) -> Result<u32> {
    match method {
        ProbeMethod::Http => probe_latency_http(ip, port, host, sni_host, path, timeout_ms).await,
        ProbeMethod::Tcp => probe_latency_tcp(ip, port, timeout_ms).await,
    }
}

pub fn update_cache_and_history(
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
        resolver_metadata: best.resolver_metadata.clone(),
    };
    history.upsert(record)?;
    Ok(())
}

pub(crate) fn resolve_preheat_domains(file_cfg: &IpPoolFileConfig) -> Vec<PreheatDomain> {
    let mut disabled: HashSet<String> = HashSet::new();
    for entry in &file_cfg.disabled_builtin_preheat {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        disabled.insert(trimmed.to_ascii_lowercase());
    }

    let mut domains: Vec<PreheatDomain> = Vec::new();
    let mut index_map: HashMap<String, usize> = HashMap::new();

    for (pattern, _) in BUILTIN_IPS.iter() {
        for host in expand_builtin_pattern(pattern) {
            let key = host.to_ascii_lowercase();
            if disabled.contains(&key) || index_map.contains_key(&key) {
                continue;
            }
            let domain = PreheatDomain::new(host);
            index_map.insert(key, domains.len());
            domains.push(domain);
        }
    }

    for custom in &file_cfg.preheat_domains {
        let key = custom.host.trim().to_ascii_lowercase();
        if key.is_empty() {
            continue;
        }
        if let Some(idx) = index_map.get(&key).copied() {
            domains[idx] = custom.clone();
        } else {
            index_map.insert(key, domains.len());
            domains.push(custom.clone());
        }
    }

    domains
}

fn expand_builtin_pattern(pattern: &str) -> Vec<String> {
    if let Some(stripped) = pattern.strip_prefix("*.") {
        if stripped.is_empty() {
            return Vec::new();
        }
        return vec![stripped.to_string()];
    }

    if pattern.starts_with('^') {
        if pattern == r"^(analytics|ghcc)\.githubassets\.com$" {
            return vec![
                "analytics.githubassets.com".to_string(),
                "ghcc.githubassets.com".to_string(),
            ];
        }
        return Vec::new();
    }

    vec![pattern.to_string()]
}

pub fn builtin_lookup(host: &str) -> Vec<IpAddr> {
    BUILTIN_IPS
        .iter()
        .find(|(h, _)| h.eq_ignore_ascii_case(host))
        .map(|(_, ips)| ips.iter().filter_map(|ip| ip.parse().ok()).collect())
        .unwrap_or_default()
}

pub fn user_static_lookup(host: &str, port: u16, entries: &[UserStaticIp]) -> Vec<IpAddr> {
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

pub fn current_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| StdDuration::from_secs(0))
        .as_millis() as i64
}

pub(crate) const BUILTIN_IPS: &[(&str, &[&str])] = &[
    (
        "github.com",
        &[
            "4.237.22.38",
            "20.200.245.247",
            "20.201.28.151",
            "20.205.243.166",
            "20.26.156.215",
            "20.27.177.113",
            "20.87.245.0",
            "140.82.112.3",
            "140.82.113.3",
            "140.82.114.3",
            "140.82.114.4",
            "140.82.116.3",
            "140.82.116.4",
            "140.82.121.3",
            "140.82.121.4",
        ],
    ),
    (
        "gist.github.com",
        &[
            "4.237.22.38",
            "20.200.245.247",
            "20.205.243.166",
            "20.27.177.113",
            "140.82.116.3",
            "140.82.116.4",
        ],
    ),
    (
        "github.dev",
        &[
            "20.43.185.14",
            "20.99.227.183",
            "51.137.3.17",
            "52.224.38.193",
        ],
    ),
    (
        "codeload.github.com",
        &[
            "20.26.156.216",
            "20.27.177.114",
            "20.87.245.7",
            "20.200.245.246",
            "20.201.28.149",
            "20.205.243.165",
            "20.248.137.55",
            "140.82.112.9",
            "140.82.113.9",
            "140.82.114.9",
            "140.82.114.10",
            "140.82.116.10",
            "140.82.121.9",
        ],
    ),
    (
        "api.github.com",
        &[
            "20.26.156.210",
            "20.27.177.116",
            "20.87.245.6",
            "20.200.245.245",
            "20.201.28.148",
            "20.205.243.168",
            "20.248.137.49",
            "140.82.112.5",
            "140.82.113.6",
            "140.82.114.6",
            "140.82.116.6",
            "140.82.121.6",
        ],
    ),
    (
        "githubusercontent.com",
        &["185.199.108.133", "185.199.110.133", "185.199.111.133"],
    ),
    (
        "*.githubusercontent.com",
        &["146.75.92.133", "199.232.88.133", "199.232.144.133"],
    ),
    (
        "viewscreen.githubusercontent.com",
        &[
            "140.82.112.21",
            "140.82.112.22",
            "140.82.113.21",
            "140.82.113.22",
            "140.82.114.21",
            "140.82.114.22",
        ],
    ),
    (
        "github.io",
        &[
            "185.199.108.153",
            "185.199.109.153",
            "185.199.110.153",
            "185.199.111.153",
        ],
    ),
    (
        "*.githubassets.com",
        &[
            "185.199.108.154",
            "185.199.109.154",
            "185.199.110.154",
            "185.199.111.154",
        ],
    ),
    (
        r"^(analytics|ghcc)\.githubassets\.com$",
        &[
            "185.199.108.153",
            "185.199.109.153",
            "185.199.110.153",
            "185.199.111.153",
        ],
    ),
];

const FALLBACK_IPS: &[(&str, &[&str])] = &[("github.com", &["20.205.243.166"])];
