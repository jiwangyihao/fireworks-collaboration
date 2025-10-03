//! IP Pool 模块综合测试
//! 合并了 ip_pool/manager_tests.rs, ip_pool/cache_tests.rs, ip_pool/config_tests.rs,
//! ip_pool/circuit_breaker_tests.rs, ip_pool/events_tests.rs,
//! ip_pool/preheat_tests.rs, ip_pool/history_tests.rs

// ============================================================================
// manager_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::ip_pool::config::EffectiveIpPoolConfig;
use fireworks_collaboration_lib::core::ip_pool::manager::IpPool;
use fireworks_collaboration_lib::core::ip_pool::{
    IpCandidate, IpOutcome, IpSelection, IpSource, IpStat,
};
use std::net::{IpAddr, Ipv4Addr};

fn make_stat(addr: [u8; 4], port: u16) -> IpStat {
    let candidate = IpCandidate::new(
        IpAddr::from(Ipv4Addr::from(addr)),
        port,
        IpSource::UserStatic,
    );
    let mut stat = IpStat::with_latency(candidate, 12);
    stat.measured_at_epoch_ms = Some(1);
    stat.expires_at_epoch_ms = Some(i64::MAX - 1);
    stat.sources = vec![IpSource::UserStatic];
    stat
}

#[test]
fn report_candidate_outcome_tracks_per_ip() {
    let mut cfg = EffectiveIpPoolConfig::default();
    cfg.runtime.enabled = true;
    let pool = IpPool::new(cfg);
    let stat = make_stat([127, 0, 0, 2], 443);
    pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Failure);
    pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Success);
    let metrics = pool
        .candidate_outcome_metrics("example.com", 443, stat.candidate.address)
        .expect("candidate metrics present");
    assert_eq!(metrics.success, 1);
    assert_eq!(metrics.failure, 1);
    assert_eq!(metrics.last_sources, vec![IpSource::UserStatic]);
}

#[test]
fn report_outcome_updates_aggregate_counts() {
    let mut cfg = EffectiveIpPoolConfig::default();
    cfg.runtime.enabled = true;
    let pool = IpPool::new(cfg);
    let stat = make_stat([127, 0, 0, 3], 443);
    let selection = IpSelection::from_cached("example.com", 443, stat.clone());
    pool.report_outcome(&selection, IpOutcome::Success);
    let aggregate = pool
        .outcome_metrics("example.com", 443)
        .expect("aggregate metrics present");
    assert_eq!(aggregate.success, 1);
    assert_eq!(aggregate.failure, 0);
    assert!(aggregate.last_outcome_ms > 0);
}

// ============================================================================
// cache_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::ip_pool::cache::{IpCacheKey, IpCacheSlot, IpScoreCache};

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

// ============================================================================
// config_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::ip_pool::config::{
    default_cache_prune_interval_secs, default_cooldown_seconds, default_failure_rate_threshold,
    default_failure_threshold, default_failure_window_seconds, default_max_cache_entries,
    default_max_parallel_probes, default_min_samples_in_window, default_probe_timeout_ms,
    default_score_ttl_seconds, default_singleflight_timeout_ms, IpPoolFileConfig,
    IpPoolRuntimeConfig, PreheatDomain, UserStaticIp,
};
use fireworks_collaboration_lib::core::ip_pool::config::{
    join_ip_config_path, load_or_init_file_at, save_file_at,
};
use std::fs;
use std::sync::{Mutex, OnceLock};

#[test]
fn runtime_defaults_are_disabled() {
    let cfg = IpPoolRuntimeConfig::default();
    assert!(!cfg.enabled);
    assert_eq!(cfg.max_parallel_probes, default_max_parallel_probes());
    assert_eq!(cfg.probe_timeout_ms, default_probe_timeout_ms());
    assert!(cfg.history_path.is_none());
    assert!(cfg.sources.builtin);
    assert!(cfg.sources.dns);
    assert!(cfg.sources.history);
    assert!(cfg.sources.user_static);
    assert!(cfg.sources.fallback);
    assert_eq!(
        cfg.cache_prune_interval_secs,
        default_cache_prune_interval_secs()
    );
    assert_eq!(cfg.max_cache_entries, default_max_cache_entries());
    assert_eq!(
        cfg.singleflight_timeout_ms,
        default_singleflight_timeout_ms()
    );
    assert_eq!(cfg.failure_threshold, default_failure_threshold());
    assert_eq!(cfg.failure_rate_threshold, default_failure_rate_threshold());
    assert_eq!(cfg.failure_window_seconds, default_failure_window_seconds());
    assert_eq!(cfg.min_samples_in_window, default_min_samples_in_window());
    assert_eq!(cfg.cooldown_seconds, default_cooldown_seconds());
    assert!(cfg.circuit_breaker_enabled);
}

#[test]
fn file_defaults_are_empty_preheat() {
    let cfg = IpPoolFileConfig::default();
    assert!(cfg.preheat_domains.is_empty());
    assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
    assert!(cfg.user_static.is_empty());
    assert!(cfg.blacklist.is_empty());
    assert!(cfg.whitelist.is_empty());
}

#[test]
fn deserializes_with_defaults() {
    let json = r#"{
        "runtime": {
            "enabled": true,
            "maxParallelProbes": 8,
            "historyPath": "custom/ip-history.json"
        },
        "file": {
            "preheatDomains": [{"host": "github.com", "ports": [443, 80]}]
        }
    }"#;
    let cfg: EffectiveIpPoolConfig = serde_json::from_str(json).unwrap();
    assert!(cfg.runtime.enabled);
    assert_eq!(cfg.runtime.max_parallel_probes, 8);
    assert_eq!(cfg.runtime.probe_timeout_ms, default_probe_timeout_ms());
    assert_eq!(cfg.file.score_ttl_seconds, default_score_ttl_seconds());
    assert_eq!(cfg.file.preheat_domains.len(), 1);
    let domain = &cfg.file.preheat_domains[0];
    assert_eq!(domain.host, "github.com");
    assert_eq!(domain.ports, vec![443, 80]);
    assert!(cfg.file.user_static.is_empty());
    assert_eq!(
        cfg.runtime.cache_prune_interval_secs,
        default_cache_prune_interval_secs()
    );
    assert_eq!(cfg.runtime.max_cache_entries, default_max_cache_entries());
    assert_eq!(
        cfg.runtime.singleflight_timeout_ms,
        default_singleflight_timeout_ms()
    );
}

#[test]
fn load_or_init_file_creates_default() {
    let guard = test_guard().lock().unwrap();
    let temp_dir = std::env::temp_dir().join(format!("fwc-ip-pool-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    let cfg = load_or_init_file_at(&temp_dir).expect("create default ip config");
    assert!(cfg.preheat_domains.is_empty());
    assert_eq!(cfg.score_ttl_seconds, default_score_ttl_seconds());
    let path = join_ip_config_path(&temp_dir);
    assert!(path.exists());
    fs::remove_dir_all(&temp_dir).ok();
    drop(guard);
}

#[test]
fn save_file_persists_changes() {
    let guard = test_guard().lock().unwrap();
    let temp_dir = std::env::temp_dir().join(format!("fwc-ip-pool-save-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    let mut cfg = IpPoolFileConfig::default();
    cfg.preheat_domains.push(PreheatDomain::new("github.com"));
    cfg.score_ttl_seconds = 120;
    cfg.user_static.push(UserStaticIp {
        host: "github.com".into(),
        ip: "140.82.112.3".into(),
        ports: vec![443],
    });
    save_file_at(&cfg, &temp_dir).expect("save ip config");
    let loaded = load_or_init_file_at(&temp_dir).expect("load ip config");
    assert_eq!(loaded.preheat_domains.len(), 1);
    assert_eq!(loaded.score_ttl_seconds, 120);
    assert_eq!(loaded.user_static.len(), 1);
    fs::remove_dir_all(&temp_dir).ok();
    drop(guard);
}

fn test_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

// ============================================================================
// circuit_breaker_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::ip_pool::circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitState,
};

#[test]
fn consecutive_failures_trigger_circuit_open() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 3,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    // 前两次失败不应触发
    breaker.record_failure(ip);
    assert!(!breaker.is_tripped(ip));
    breaker.record_failure(ip);
    assert!(!breaker.is_tripped(ip));

    // 第三次失败触发熔断
    breaker.record_failure(ip);
    assert!(breaker.is_tripped(ip));

    let stats = breaker.get_stats(ip).unwrap();
    assert_eq!(stats.state, CircuitState::Cooldown);
    assert_eq!(stats.consecutive_failures, 3);
}

#[test]
fn success_resets_consecutive_failures() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 3,
        failure_rate_threshold: 0.9, // 高阈值，避免失败率触发
        min_samples_in_window: 10,   // 高样本数要求
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    breaker.record_failure(ip);
    breaker.record_failure(ip);
    breaker.record_success(ip); // 重置连续失败计数
    breaker.record_failure(ip);
    breaker.record_failure(ip);

    // 不应触发，因为中间有成功，连续失败被重置
    assert!(!breaker.is_tripped(ip));
}

#[test]
fn failure_rate_triggers_circuit_open() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 100, // 不通过连续失败触发
        failure_rate_threshold: 0.5,
        min_samples_in_window: 5,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    // 2 成功 + 3 失败 = 5 样本，失败率 60%
    breaker.record_success(ip);
    breaker.record_success(ip);
    breaker.record_failure(ip);
    breaker.record_failure(ip);
    breaker.record_failure(ip);

    // 应触发熔断
    assert!(breaker.is_tripped(ip));
}

#[test]
fn manual_reset_clears_circuit_state() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 2,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    breaker.record_failure(ip);
    breaker.record_failure(ip);
    assert!(breaker.is_tripped(ip));

    breaker.reset_ip(ip);
    assert!(!breaker.is_tripped(ip));
}

#[test]
fn disabled_breaker_never_trips() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: false,
        consecutive_failure_threshold: 1,
        ..Default::default()
    });

    let ip = "192.0.2.1".parse().unwrap();

    breaker.record_failure(ip);
    breaker.record_failure(ip);
    breaker.record_failure(ip);

    assert!(!breaker.is_tripped(ip));
}

#[test]
fn get_tripped_ips_returns_only_tripped() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        enabled: true,
        consecutive_failure_threshold: 2,
        ..Default::default()
    });

    let ip1: IpAddr = "192.0.2.1".parse().unwrap();
    let ip2: IpAddr = "192.0.2.2".parse().unwrap();
    let ip3: IpAddr = "192.0.2.3".parse().unwrap();

    breaker.record_failure(ip1);
    breaker.record_failure(ip1); // ip1 熔断

    breaker.record_failure(ip2);
    breaker.record_success(ip2); // ip2 正常

    breaker.record_failure(ip3);
    breaker.record_failure(ip3); // ip3 熔断

    let tripped = breaker.get_tripped_ips();
    assert_eq!(tripped.len(), 2);
    assert!(tripped.contains(&ip1));
    assert!(tripped.contains(&ip3));
    assert!(!tripped.contains(&ip2));
}

// ============================================================================
// events_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::ip_pool::events::{
    emit_ip_pool_refresh, emit_ip_pool_selection,
};
use fireworks_collaboration_lib::core::ip_pool::IpSelectionStrategy;
use fireworks_collaboration_lib::events::structured::{
    clear_test_event_bus, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[test]
fn emit_ip_pool_selection_publishes_event() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));

    let task_id = Uuid::new_v4();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let stat = IpStat {
        candidate: IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
            443,
            IpSource::Builtin,
        ),
        sources: vec![IpSource::Builtin, IpSource::Dns],
        latency_ms: Some(42),
        measured_at_epoch_ms: Some(now_ms),
        expires_at_epoch_ms: Some(now_ms + 300_000),
    };

    emit_ip_pool_selection(
        task_id,
        "github.com",
        443,
        IpSelectionStrategy::Cached,
        Some(&stat),
        3,
    );

    let events = bus.snapshot();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolSelection {
            id,
            domain,
            port,
            strategy,
            source,
            latency_ms,
            candidates_count,
        }) => {
            assert_eq!(id, &task_id.to_string());
            assert_eq!(domain, "github.com");
            assert_eq!(*port, 443);
            assert_eq!(strategy, "Cached");
            assert!(source.is_some());
            assert_eq!(*latency_ms, Some(42));
            assert_eq!(*candidates_count, 3);
        }
        _ => panic!("expected IpPoolSelection event"),
    }
    clear_test_event_bus();
}

#[test]
fn emit_ip_pool_refresh_publishes_event() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));

    let task_id = Uuid::new_v4();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let candidates = vec![
        IpStat {
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: Some(10),
            measured_at_epoch_ms: Some(now_ms),
            expires_at_epoch_ms: Some(now_ms + 300_000),
        },
        IpStat {
            candidate: IpCandidate::new(IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)), 443, IpSource::Dns),
            sources: vec![IpSource::Dns],
            latency_ms: Some(20),
            measured_at_epoch_ms: Some(now_ms),
            expires_at_epoch_ms: Some(now_ms + 300_000),
        },
    ];

    emit_ip_pool_refresh(
        task_id,
        "github.com",
        true,
        &candidates,
        "preheat".to_string(),
    );

    let events = bus.snapshot();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolRefresh {
            id,
            domain,
            success,
            candidates_count,
            min_latency_ms,
            max_latency_ms,
            reason,
        }) => {
            assert_eq!(id, &task_id.to_string());
            assert_eq!(domain, "github.com");
            assert!(success);
            assert_eq!(*candidates_count, 2);
            assert_eq!(*min_latency_ms, Some(10));
            assert_eq!(*max_latency_ms, Some(20));
            assert_eq!(reason, "preheat");
        }
        _ => panic!("expected IpPoolRefresh event"),
    }
    clear_test_event_bus();
}

#[test]
fn emit_ip_pool_refresh_handles_empty_candidates() {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));

    let task_id = Uuid::new_v4();
    emit_ip_pool_refresh(
        task_id,
        "example.com",
        false,
        &[],
        "sampling_failed".to_string(),
    );

    let events = bus.snapshot();
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::Strategy(StrategyEvent::IpPoolRefresh {
            success,
            candidates_count,
            min_latency_ms,
            max_latency_ms,
            ..
        }) => {
            assert!(!success);
            assert_eq!(*candidates_count, 0);
            assert!(min_latency_ms.is_none());
            assert!(max_latency_ms.is_none());
        }
        _ => panic!("expected IpPoolRefresh event"),
    }
    clear_test_event_bus();
}

// ============================================================================
// preheat_tests.rs 的测试
// ============================================================================

use fireworks_collaboration_lib::core::ip_pool::history::{IpHistoryRecord, IpHistoryStore};
use fireworks_collaboration_lib::core::ip_pool::preheat::{
    builtin_lookup, collect_candidates, current_epoch_ms, next_due_schedule, probe_latency,
    update_cache_and_history, user_static_lookup, AggregatedCandidate, DomainSchedule,
};
use tokio::runtime::Builder;
use tokio::time::{Duration as TokioDuration, Instant};

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
    let result = rt.block_on(async { probe_latency("203.0.113.1".parse().unwrap(), 9, 200).await });
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
    let candidates =
        rt.block_on(async { collect_candidates("github.com", 443, &cfg, history.clone()).await });
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
    let candidates =
        rt.block_on(async { collect_candidates("expired.test", 443, &cfg, history.clone()).await });
    assert!(candidates.is_empty());
    assert!(history.get("expired.test", 443).is_none());
}

// ============================================================================
// history_tests.rs 的测试
// ============================================================================

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

#[test]
fn enforce_capacity_removes_oldest_entries() {
    let store = IpHistoryStore::in_memory();
    for i in 0..5 {
        let record = IpHistoryRecord {
            host: format!("host{}.com", i),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, i as u8 + 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10 + i as u32,
            measured_at_epoch_ms: (i + 1) as i64 * 1000,
            expires_at_epoch_ms: 100_000,
        };
        store.upsert(record).expect("write history");
    }
    let removed = store.enforce_capacity(3).expect("enforce capacity");
    assert_eq!(removed, 2);
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.len(), 3);
    // Should keep the 3 newest (host2, host3, host4)
    assert!(snapshot.iter().all(|e| e.host.starts_with("host")
        && (e.host == "host2.com" || e.host == "host3.com" || e.host == "host4.com")));
}

#[test]
fn prune_and_enforce_removes_expired_and_old_entries() {
    let store = IpHistoryStore::in_memory();
    // Add 2 expired entries
    for i in 0..2 {
        let record = IpHistoryRecord {
            host: format!("expired{}.com", i),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, i as u8 + 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10,
            measured_at_epoch_ms: 1000,
            expires_at_epoch_ms: 5000,
        };
        store.upsert(record).expect("write history");
    }
    // Add 4 valid entries
    for i in 0..4 {
        let record = IpHistoryRecord {
            host: format!("valid{}.com", i),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(2, 2, 2, i as u8 + 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 20,
            measured_at_epoch_ms: (i + 1) as i64 * 2000 + 10000,
            expires_at_epoch_ms: 100_000,
        };
        store.upsert(record).expect("write history");
    }
    let (expired, capacity_pruned) = store
        .prune_and_enforce(10_000, 3)
        .expect("prune and enforce");
    assert_eq!(expired, 2);
    assert_eq!(capacity_pruned, 1);
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.len(), 3);
    // Should only have valid entries, and the 3 newest
    assert!(snapshot.iter().all(|e| e.host.starts_with("valid")));
}
