use fireworks_collaboration_lib::core::ip_pool::config::{EffectiveIpPoolConfig, PreheatDomain};
use fireworks_collaboration_lib::core::ip_pool::history::{IpHistoryRecord, IpHistoryStore};
use fireworks_collaboration_lib::core::ip_pool::preheat::{
    builtin_lookup, collect_candidates, current_epoch_ms, next_due_schedule, probe_latency,
    update_cache_and_history, user_static_lookup, AggregatedCandidate, DomainSchedule,
};
use fireworks_collaboration_lib::core::ip_pool::{
    cache::IpScoreCache, IpCandidate, IpSource, UserStaticIp,
};
use std::net::IpAddr;
use std::sync::Arc;
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
