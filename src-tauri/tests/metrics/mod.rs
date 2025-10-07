use std::convert::TryFrom;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use fireworks_collaboration_lib::core::config::model::{
    ObservabilityConfig, ObservabilityExportConfig, ObservabilityLayer, ObservabilityRedactIpMode,
};
use fireworks_collaboration_lib::core::metrics::{
    aggregate_enabled, auto_downgrade, build_snapshot, current_layer, encode_prometheus,
    evaluate_alerts_now, force_memory_pressure_check, global_registry,
    init_aggregate_observability_with_provider, init_alerts_observability,
    init_basic_observability, observe_histogram_metric, observe_histogram_with_kind,
    override_layer_guards, parse_snapshot_query, record_counter_metric, resolve_config, set_layer,
    HistogramWindowConfig, ManualTimeProvider, MetricDescriptor, MetricError, MetricRegistry,
    SampleKind, SnapshotQuery, SnapshotQueryError, TimeProvider, WindowRange, ALERTS_FIRED_TOTAL,
    CIRCUIT_BREAKER_RECOVER_TOTAL, CIRCUIT_BREAKER_TRIP_TOTAL, GIT_RETRY_TOTAL, GIT_TASKS_TOTAL,
    GIT_TASK_DURATION_MS, HTTP_STRATEGY_FALLBACK_TOTAL, IP_POOL_AUTO_DISABLE_TOTAL,
    IP_POOL_LATENCY_MS, IP_POOL_REFRESH_TOTAL, IP_POOL_SELECTION_TOTAL,
    METRICS_EXPORT_RATE_LIMITED_TOTAL, METRICS_EXPORT_REQUESTS_TOTAL, METRICS_EXPORT_SERIES_TOTAL,
    METRIC_MEMORY_PRESSURE_TOTAL, OBSERVABILITY_LAYER, PROXY_FALLBACK_TOTAL, TLS_HANDSHAKE_MS,
};
use fireworks_collaboration_lib::core::metrics::{
    configure_tls_sampling, set_runtime_debug_mode, set_runtime_ip_mode, set_runtime_memory_limit,
};
use fireworks_collaboration_lib::events::structured::{
    ensure_fanout_bus, publish_global, Event, MemoryEventBus, MetricAlertState, StrategyEvent,
    TaskEvent,
};
use hyper::body::to_bytes;
use hyper::client::HttpConnector;
use hyper::{Body, Client, Request, StatusCode, Uri};
use once_cell::sync::{Lazy, OnceCell};
use serde_json::Value;
use tempfile::tempdir;
use uuid::Uuid;

fn ensure_metrics_init() {
    let cfg = ObservabilityConfig::default();
    init_basic_observability(&cfg).expect("basic observability should initialize");
}

fn ensure_aggregate_init() -> Arc<ManualTimeProvider> {
    static PROVIDER: Lazy<Arc<ManualTimeProvider>> =
        Lazy::new(|| Arc::new(ManualTimeProvider::new()));
    if !aggregate_enabled() {
        let cfg = ObservabilityConfig::default();
        let provider = PROVIDER.clone();
        let trait_arc: Arc<dyn TimeProvider> = provider.clone();
        init_aggregate_observability_with_provider(&cfg, trait_arc)
            .expect("aggregate observability should initialize");
    }
    PROVIDER.clone()
}

static AGGREGATE_TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn aggregate_lock() -> MutexGuard<'static, ()> {
    AGGREGATE_TEST_GUARD
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

static ALERT_RULES_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let dir = tempdir().expect("alert rules dir");
    // Persist the temp dir and take ownership of the path so it remains for the test lifetime.
    let path = dir.keep();
    path.join("alert-rules.json")
});

static ALERTS_ENGINE_INIT: OnceCell<()> = OnceCell::new();

fn alerts_rules_path() -> &'static PathBuf {
    &ALERT_RULES_PATH
}

const TEST_RUNTIME_IP_METRIC: MetricDescriptor = MetricDescriptor::counter(
    "test_runtime_ip_redaction_total",
    "Runtime IP redaction test metric",
    &["client_ip"],
);

const WAIT_RETRIES: usize = 50;
const WAIT_DELAY_MS: u64 = 10;

fn wait_for_counter(
    registry: &MetricRegistry,
    desc: MetricDescriptor,
    labels: &[(&'static str, &str)],
    target: u64,
) -> u64 {
    let mut value = 0;
    for _ in 0..WAIT_RETRIES {
        value = registry.get_counter(desc, labels).unwrap_or(0);
        if value >= target {
            break;
        }
        thread::sleep(Duration::from_millis(WAIT_DELAY_MS));
    }
    value
}

fn wait_for_histogram(
    registry: &MetricRegistry,
    desc: MetricDescriptor,
    labels: &[(&'static str, &str)],
    target_count: u64,
) -> Option<(u64, f64)> {
    let mut snapshot = None;
    for _ in 0..WAIT_RETRIES {
        snapshot = registry
            .get_histogram(desc, labels)
            .map(|h| (h.count, h.sum));
        match snapshot {
            Some((count, _)) if count >= target_count => break,
            Some(_) | None => {
                thread::sleep(Duration::from_millis(WAIT_DELAY_MS));
            }
        }
    }
    snapshot
}

fn current_git_totals(registry: &MetricRegistry) -> (u64, u64) {
    let mut failed = 0;
    let mut total = 0;
    if let Ok(series) = registry.list_counter_series(GIT_TASKS_TOTAL) {
        for entry in series {
            if entry.labels.len() != 2 {
                continue;
            }
            let kind_label = entry.labels[0].clone();
            let state_label = entry.labels[1].clone();
            let pairs = [
                ("kind", kind_label.as_str()),
                ("state", state_label.as_str()),
            ];
            if let Ok(snapshot) = registry.snapshot_counter_window(
                GIT_TASKS_TOTAL,
                &pairs,
                WindowRange::LastFiveMinutes,
            ) {
                total += snapshot.total;
                if state_label == "failed" {
                    failed += snapshot.total;
                }
            }
        }
    }
    (failed, total)
}

fn ensure_alerts_engine_initialized() {
    ALERTS_ENGINE_INIT.get_or_init(|| {
        ensure_metrics_init();
        let provider = ensure_aggregate_init();
        provider.reset();
        provider.set(Duration::from_secs(600));
        let mut cfg = ObservabilityConfig::default();
        cfg.alerts_enabled = true;
        cfg.alerts.eval_interval_secs = 0;
        cfg.alerts.rules_path = alerts_rules_path().to_string_lossy().to_string();
        init_alerts_observability(&cfg).expect("initialize alerts");
    });
}

#[test]
fn git_task_metrics_increment_on_lifecycle() {
    ensure_metrics_init();
    let registry = global_registry();
    let kind = format!("GitCloneTest_{}", Uuid::new_v4());
    let task_id = Uuid::new_v4().to_string();

    let counter_before = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "completed")],
        )
        .unwrap_or(0);
    let hist_before = registry
        .get_histogram(GIT_TASK_DURATION_MS, &[("kind", kind.as_str())])
        .map(|h| h.count)
        .unwrap_or(0);

    publish_global(Event::Task(TaskEvent::Started {
        id: task_id.clone(),
        kind: kind.clone(),
    }));
    std::thread::sleep(Duration::from_millis(5));
    publish_global(Event::Task(TaskEvent::Completed { id: task_id }));

    let counter_after = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "completed")],
        )
        .unwrap();
    let hist_after = registry
        .get_histogram(GIT_TASK_DURATION_MS, &[("kind", kind.as_str())])
        .map(|h| h.count)
        .unwrap();

    assert_eq!(counter_after, counter_before + 1);
    assert_eq!(hist_after, hist_before + 1);
}

#[test]
fn git_failure_records_retry_totals() {
    ensure_metrics_init();
    let registry = global_registry();
    let kind = format!("GitCloneTestFail_{}", Uuid::new_v4());
    let task_id = Uuid::new_v4().to_string();

    let fail_before = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "failed")],
        )
        .unwrap_or(0);
    let retry_before = registry
        .get_counter(
            GIT_RETRY_TOTAL,
            &[("kind", kind.as_str()), ("category", "network")],
        )
        .unwrap_or(0);

    publish_global(Event::Task(TaskEvent::Started {
        id: task_id.clone(),
        kind: kind.clone(),
    }));
    publish_global(Event::Task(TaskEvent::Failed {
        id: task_id,
        category: "Network".into(),
        code: None,
        message: "simulated failure".into(),
    }));

    let fail_after = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "failed")],
        )
        .unwrap();
    let retry_after = registry
        .get_counter(
            GIT_RETRY_TOTAL,
            &[("kind", kind.as_str()), ("category", "network")],
        )
        .unwrap();

    assert_eq!(fail_after, fail_before + 1);
    assert_eq!(retry_after, retry_before + 1);
}

#[test]
fn strategy_events_emit_metrics() {
    ensure_metrics_init();
    let registry = global_registry();
    let task_id = Uuid::new_v4().to_string();
    let reason = format!("refresh_reason_{}", Uuid::new_v4().simple());

    let tls_before = registry
        .get_histogram(
            TLS_HANDSHAKE_MS,
            &[("sni_strategy", "fake"), ("outcome", "ok")],
        )
        .map(|h| h.count)
        .unwrap_or(0);
    let latency_before = registry
        .get_histogram(IP_POOL_LATENCY_MS, &[("source", "builtin")])
        .map(|h| h.count)
        .unwrap_or(0);
    let refresh_before = registry
        .get_counter(
            IP_POOL_REFRESH_TOTAL,
            &[("reason", reason.as_str()), ("success", "true")],
        )
        .unwrap_or(0);
    let selection_before = registry
        .get_counter(
            IP_POOL_SELECTION_TOTAL,
            &[("strategy", "cached"), ("outcome", "success")],
        )
        .unwrap_or(0);
    let fallback_before = registry
        .get_counter(
            HTTP_STRATEGY_FALLBACK_TOTAL,
            &[("stage", "fallback"), ("from", "fake")],
        )
        .unwrap_or(0);

    publish_global(Event::Strategy(StrategyEvent::AdaptiveTlsTiming {
        id: task_id.clone(),
        kind: "GitClone".into(),
        used_fake_sni: true,
        fallback_stage: "Fake".into(),
        connect_ms: Some(5),
        tls_ms: Some(12),
        first_byte_ms: Some(15),
        total_ms: Some(20),
        cert_fp_changed: false,
        ip_source: Some("Builtin".into()),
        ip_latency_ms: Some(18),
        ip_selection_stage: None,
    }));

    publish_global(Event::Strategy(StrategyEvent::AdaptiveTlsFallback {
        id: task_id.clone(),
        kind: "GitClone".into(),
        from: "Fake".into(),
        to: "Real".into(),
        reason: "Fallback".into(),
        ip_source: None,
        ip_latency_ms: None,
    }));

    publish_global(Event::Strategy(StrategyEvent::IpPoolSelection {
        id: task_id.clone(),
        domain: "example.com".into(),
        port: 443,
        strategy: "Cached".into(),
        source: Some("Builtin".into()),
        latency_ms: Some(18),
        candidates_count: 3,
    }));

    publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
        id: task_id,
        domain: "example.com".into(),
        success: true,
        candidates_count: 2,
        min_latency_ms: Some(15),
        max_latency_ms: Some(25),
        reason: reason.clone(),
    }));

    let tls_after = registry
        .get_histogram(
            TLS_HANDSHAKE_MS,
            &[("sni_strategy", "fake"), ("outcome", "ok")],
        )
        .map(|h| h.count)
        .unwrap();
    let latency_after = registry
        .get_histogram(IP_POOL_LATENCY_MS, &[("source", "builtin")])
        .map(|h| h.count)
        .unwrap();
    let refresh_after = registry
        .get_counter(
            IP_POOL_REFRESH_TOTAL,
            &[("reason", reason.as_str()), ("success", "true")],
        )
        .unwrap();
    let selection_after = registry
        .get_counter(
            IP_POOL_SELECTION_TOTAL,
            &[("strategy", "cached"), ("outcome", "success")],
        )
        .unwrap();
    let fallback_after = registry
        .get_counter(
            HTTP_STRATEGY_FALLBACK_TOTAL,
            &[("stage", "fallback"), ("from", "fake")],
        )
        .unwrap();

    assert!(tls_after >= tls_before + 1);
    assert!(latency_after >= latency_before + 1);
    assert_eq!(refresh_after, refresh_before + 1);
    assert_eq!(selection_after, selection_before + 1);
    assert_eq!(fallback_after, fallback_before + 1);
}

#[test]
fn proxy_and_circuit_events_emit_metrics() {
    ensure_metrics_init();
    let registry = global_registry();

    let proxy_reason = "Proxy Fallback+Reason";
    let proxy_label = "proxy_fallback_reason";
    let auto_disable_reason = "Auto Disable#";
    let auto_disable_label = "auto_disable";
    let circuit_reason = "Circuit Trip!";
    let circuit_label = "circuit_trip";

    let proxy_before = registry
        .get_counter(PROXY_FALLBACK_TOTAL, &[("reason", proxy_label)])
        .unwrap_or(0);
    let auto_disable_before = registry
        .get_counter(
            IP_POOL_AUTO_DISABLE_TOTAL,
            &[("reason", auto_disable_label)],
        )
        .unwrap_or(0);
    let trip_before = registry
        .get_counter(CIRCUIT_BREAKER_TRIP_TOTAL, &[("reason", circuit_label)])
        .unwrap_or(0);
    let recover_before = registry
        .get_counter(CIRCUIT_BREAKER_RECOVER_TOTAL, &[])
        .unwrap_or(0);

    publish_global(Event::Strategy(StrategyEvent::ProxyFallback {
        id: Uuid::new_v4().to_string(),
        reason: proxy_reason.into(),
        failure_count: 3,
        window_seconds: 60,
    }));

    publish_global(Event::Strategy(StrategyEvent::IpPoolAutoDisable {
        reason: auto_disable_reason.into(),
        until_ms: 30_000,
    }));

    publish_global(Event::Strategy(StrategyEvent::IpPoolIpTripped {
        ip: "10.0.0.1".into(),
        reason: circuit_reason.into(),
    }));

    publish_global(Event::Strategy(StrategyEvent::IpPoolIpRecovered {
        ip: "10.0.0.1".into(),
    }));

    let proxy_after = registry
        .get_counter(PROXY_FALLBACK_TOTAL, &[("reason", proxy_label)])
        .unwrap();
    let auto_disable_after = registry
        .get_counter(
            IP_POOL_AUTO_DISABLE_TOTAL,
            &[("reason", auto_disable_label)],
        )
        .unwrap();
    let trip_after = registry
        .get_counter(CIRCUIT_BREAKER_TRIP_TOTAL, &[("reason", circuit_label)])
        .unwrap();
    let recover_after = registry
        .get_counter(CIRCUIT_BREAKER_RECOVER_TOTAL, &[])
        .unwrap();

    assert_eq!(proxy_after, proxy_before + 1);
    assert_eq!(auto_disable_after, auto_disable_before + 1);
    assert_eq!(trip_after, trip_before + 1);
    assert_eq!(recover_after, recover_before + 1);
}

#[test]
fn duplicate_completion_events_do_not_double_count() {
    ensure_metrics_init();
    let registry = global_registry();
    let kind = format!("GitFetchDup_{}", Uuid::new_v4());
    let task_id = Uuid::new_v4().to_string();

    let counter_before = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "completed")],
        )
        .unwrap_or(0);

    publish_global(Event::Task(TaskEvent::Started {
        id: task_id.clone(),
        kind: kind.clone(),
    }));
    publish_global(Event::Task(TaskEvent::Completed {
        id: task_id.clone(),
    }));
    publish_global(Event::Task(TaskEvent::Completed { id: task_id }));

    let counter_after = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "completed")],
        )
        .unwrap();

    assert_eq!(counter_after, counter_before + 1);
}

#[test]
fn tls_timing_failure_records_fail_outcome() {
    ensure_metrics_init();
    configure_tls_sampling(1);
    let registry = global_registry();
    let task_id = Uuid::new_v4().to_string();

    let before = registry
        .get_histogram(
            TLS_HANDSHAKE_MS,
            &[("sni_strategy", "real"), ("outcome", "fail")],
        )
        .map(|h| (h.count, h.sum))
        .unwrap_or((0, 0.0));

    publish_global(Event::Strategy(StrategyEvent::AdaptiveTlsTiming {
        id: task_id,
        kind: "GitFetch".into(),
        used_fake_sni: false,
        fallback_stage: "initial".into(),
        connect_ms: None,
        tls_ms: None,
        first_byte_ms: None,
        total_ms: Some(42),
        cert_fp_changed: false,
        ip_source: None,
        ip_latency_ms: None,
        ip_selection_stage: None,
    }));

    let target = before.0 + 1;
    let after = wait_for_histogram(
        &registry,
        TLS_HANDSHAKE_MS,
        &[("sni_strategy", "real"), ("outcome", "fail")],
        target,
    )
    .expect("tls latency histogram should exist");

    assert_eq!(after.0, target);
    assert!(after.1 >= before.1 + 42.0);
    configure_tls_sampling(5);
}

#[test]
fn ip_selection_failure_records_fail_outcome() {
    ensure_metrics_init();
    let registry = global_registry();
    let task_id = Uuid::new_v4().to_string();

    let before = registry
        .get_counter(
            IP_POOL_SELECTION_TOTAL,
            &[("strategy", "on_demand"), ("outcome", "fail")],
        )
        .unwrap_or(0);

    publish_global(Event::Strategy(StrategyEvent::IpPoolSelection {
        id: task_id,
        domain: "example.com".into(),
        port: 443,
        strategy: "On-Demand".into(),
        source: None,
        latency_ms: None,
        candidates_count: 0,
    }));

    let after = registry
        .get_counter(
            IP_POOL_SELECTION_TOTAL,
            &[("strategy", "on_demand"), ("outcome", "fail")],
        )
        .unwrap();

    assert_eq!(after, before + 1);
}

#[test]
fn duplicate_failure_events_do_not_double_count() {
    ensure_metrics_init();
    let registry = global_registry();
    let kind = format!("GitFetchFailDup_{}", Uuid::new_v4());
    let task_id = Uuid::new_v4().to_string();

    let fail_before = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "failed")],
        )
        .unwrap_or(0);
    let retry_before = registry
        .get_counter(
            GIT_RETRY_TOTAL,
            &[("kind", kind.as_str()), ("category", "network")],
        )
        .unwrap_or(0);

    publish_global(Event::Task(TaskEvent::Started {
        id: task_id.clone(),
        kind: kind.clone(),
    }));
    publish_global(Event::Task(TaskEvent::Failed {
        id: task_id.clone(),
        category: "Network".into(),
        code: Some("ECONNRESET".into()),
        message: "network flake".into(),
    }));
    publish_global(Event::Task(TaskEvent::Failed {
        id: task_id,
        category: "Network".into(),
        code: Some("ECONNRESET".into()),
        message: "network flake".into(),
    }));

    let fail_after = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "failed")],
        )
        .unwrap();
    let retry_after = registry
        .get_counter(
            GIT_RETRY_TOTAL,
            &[("kind", kind.as_str()), ("category", "network")],
        )
        .unwrap();

    assert_eq!(fail_after, fail_before + 1);
    assert_eq!(retry_after, retry_before + 1);
}

#[test]
fn ip_refresh_failure_records_false_outcome() {
    ensure_metrics_init();
    let registry = global_registry();
    let task_id = Uuid::new_v4().to_string();
    let reason = format!("refresh_fail_{}", Uuid::new_v4().simple());

    let before = registry
        .get_counter(
            IP_POOL_REFRESH_TOTAL,
            &[("reason", reason.as_str()), ("success", "false")],
        )
        .unwrap_or(0);

    publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
        id: task_id,
        domain: "example.com".into(),
        success: false,
        candidates_count: 1,
        min_latency_ms: Some(30),
        max_latency_ms: Some(40),
        reason: reason.clone(),
    }));

    let after = registry
        .get_counter(
            IP_POOL_REFRESH_TOTAL,
            &[("reason", reason.as_str()), ("success", "false")],
        )
        .unwrap();

    assert_eq!(after, before + 1);
}

#[test]
fn runtime_tls_sampling_reconfiguration_changes_rate() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    configure_tls_sampling(1);
    let registry = global_registry();
    let strategy = format!("runtime_sample_{}", Uuid::new_v4().simple());
    let labels = [("sni_strategy", strategy.as_str()), ("outcome", "ok")];

    let baseline = registry
        .get_histogram(TLS_HANDSHAKE_MS, &labels)
        .map(|snapshot| snapshot.count)
        .unwrap_or(0);

    for _ in 0..5 {
        observe_histogram_with_kind(TLS_HANDSHAKE_MS, &labels, 25.0, SampleKind::TlsHandshake);
    }

    let after_low_rate = registry
        .get_histogram(TLS_HANDSHAKE_MS, &labels)
        .map(|snapshot| snapshot.count)
        .unwrap_or(0);
    assert_eq!(after_low_rate, baseline + 5);

    configure_tls_sampling(10);
    for _ in 0..30 {
        observe_histogram_with_kind(TLS_HANDSHAKE_MS, &labels, 30.0, SampleKind::TlsHandshake);
    }

    let after_high_rate = registry
        .get_histogram(TLS_HANDSHAKE_MS, &labels)
        .map(|snapshot| snapshot.count)
        .unwrap();
    let recorded = after_high_rate.saturating_sub(after_low_rate);
    assert_eq!(recorded, 3, "expected exactly three samples at rate 10");

    configure_tls_sampling(5);
}

#[test]
fn runtime_ip_redaction_respects_mode_and_debug() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let registry = global_registry();
    if let Err(err) = registry.register(TEST_RUNTIME_IP_METRIC) {
        assert!(
            matches!(err, MetricError::AlreadyRegistered(_)),
            "register test metric failed: {}",
            err
        );
    }

    set_runtime_debug_mode(false);
    set_runtime_ip_mode(ObservabilityRedactIpMode::Mask);

    let masked_before = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.*.*")])
        .unwrap_or(0);

    record_counter_metric(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.113.42")], 1);

    let masked_after = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.*.*")])
        .unwrap();
    assert_eq!(masked_after, masked_before + 1);

    set_runtime_ip_mode(ObservabilityRedactIpMode::Full);
    let full_before = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.113.99")])
        .unwrap_or(0);

    record_counter_metric(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.113.99")], 1);

    let full_after = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.113.99")])
        .unwrap();
    assert_eq!(full_after, full_before + 1);

    let masked_series = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "203.0.*.*")])
        .unwrap();
    assert_eq!(masked_series, masked_after);

    set_runtime_debug_mode(true);
    set_runtime_ip_mode(ObservabilityRedactIpMode::Mask);
    let debug_before = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "198.51.100.7")])
        .unwrap_or(0);

    record_counter_metric(TEST_RUNTIME_IP_METRIC, &[("client_ip", "198.51.100.7")], 1);

    let debug_after = registry
        .get_counter(TEST_RUNTIME_IP_METRIC, &[("client_ip", "198.51.100.7")])
        .unwrap();
    assert_eq!(debug_after, debug_before + 1);

    set_runtime_debug_mode(false);
    set_runtime_ip_mode(ObservabilityRedactIpMode::Mask);
}

#[test]
fn metrics_counter_window_tracks_recent_values() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600)); // start from minute 10

    let registry = global_registry();
    let strategy = format!("cached_{}", Uuid::new_v4().simple());
    let labels = [("strategy", strategy.as_str()), ("outcome", "success")];

    registry
        .incr_counter(IP_POOL_SELECTION_TOTAL, &labels, 1)
        .expect("counter increment");

    provider.advance(Duration::from_secs(60));
    registry
        .incr_counter(IP_POOL_SELECTION_TOTAL, &labels, 2)
        .expect("counter increment");

    provider.advance(Duration::from_secs(60));

    let snapshot = registry
        .snapshot_counter_window(
            IP_POOL_SELECTION_TOTAL,
            &labels,
            WindowRange::LastFiveMinutes,
        )
        .expect("counter window snapshot");

    assert_eq!(snapshot.points.len(), 5);
    let values: Vec<u64> = snapshot.points.iter().map(|p| p.value).collect();
    assert_eq!(values[2], 1);
    assert_eq!(values[3], 2);
    assert_eq!(values[4], 0);
    assert_eq!(snapshot.total, values.iter().sum::<u64>());
}

#[test]
fn metrics_histogram_window_combines_samples() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(3_600)); // start from minute 60

    let registry = global_registry();
    let strategy = format!("fake_{}", Uuid::new_v4().simple());
    let outcome = format!("ok_{}", Uuid::new_v4().simple());
    let labels = [
        ("sni_strategy", strategy.as_str()),
        ("outcome", outcome.as_str()),
    ];

    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 25.0)
        .expect("record sample");
    provider.advance(Duration::from_secs(60));
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 40.0)
        .expect("record sample");
    provider.advance(Duration::from_secs(60));
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 80.0)
        .expect("record sample");

    let snapshot = registry
        .snapshot_histogram_window(
            TLS_HANDSHAKE_MS,
            &labels,
            WindowRange::LastFiveMinutes,
            &[0.5, 0.9],
        )
        .expect("histogram window snapshot");

    assert_eq!(snapshot.points.len(), 5);
    assert_eq!(snapshot.count, 3);
    assert!((snapshot.sum - 145.0).abs() < f64::EPSILON);
    let counts: Vec<u64> = snapshot.points.iter().map(|p| p.count).collect();
    assert_eq!(counts.iter().sum::<u64>(), snapshot.count);
    assert_eq!(counts.iter().filter(|&&v| v > 0).count(), 3);
    let recent: Vec<u64> = counts.iter().rev().take(3).copied().collect();
    assert!(recent.iter().all(|&v| v > 0));
    let bucket_total: u64 = snapshot.buckets.iter().map(|(_, v)| *v).sum();
    assert_eq!(bucket_total, snapshot.count);

    let p50 = snapshot
        .quantiles
        .iter()
        .find(|(q, _)| (*q - 0.5).abs() < f64::EPSILON)
        .map(|(_, v)| *v)
        .expect("p50 present");
    assert!(p50 >= 25.0 && p50 <= 80.0);
    if let Some((_, p90)) = snapshot
        .quantiles
        .iter()
        .find(|(q, _)| (*q - 0.9).abs() < f64::EPSILON)
    {
        assert!(*p90 >= 40.0);
    }
}

#[test]
fn metrics_histogram_raw_samples_respect_window() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();

    let registry = global_registry();
    registry.enable_histogram_window(
        TLS_HANDSHAKE_MS,
        HistogramWindowConfig::enable_raw_samples(Duration::from_secs(300), 4),
    );

    provider.set(Duration::from_secs(600)); // minute 10
    let strategy = format!("sampled_{}", Uuid::new_v4().simple());
    let labels = [("sni_strategy", strategy.as_str()), ("outcome", "ok_raw")];

    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 20.0)
        .expect("record sample");
    provider.advance(Duration::from_secs(60));
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 30.0)
        .expect("record sample");
    provider.advance(Duration::from_secs(240));
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 40.0)
        .expect("record sample");
    provider.advance(Duration::from_secs(120));
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 75.0)
        .expect("record sample");

    let snapshot = registry
        .snapshot_histogram_window(TLS_HANDSHAKE_MS, &labels, WindowRange::LastFiveMinutes, &[])
        .expect("histogram window snapshot");

    let raw_values: Vec<f64> = snapshot.raw_samples.iter().map(|s| s.value).collect();
    let raw_offsets: Vec<u64> = snapshot
        .raw_samples
        .iter()
        .map(|s| s.offset_seconds)
        .collect();
    assert_eq!(raw_values, vec![40.0, 75.0]);
    assert_eq!(raw_offsets.len(), 2);
    assert_eq!(raw_offsets[1].saturating_sub(raw_offsets[0]), 2 * 60);
    assert_eq!(snapshot.count, 2);
    registry.enable_histogram_window(TLS_HANDSHAKE_MS, HistogramWindowConfig::default());
}

#[test]
fn metrics_histogram_raw_sample_capacity_updates() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(60 * 20));

    let registry = global_registry();
    let base_config = HistogramWindowConfig::enable_raw_samples(Duration::from_secs(600), 3);
    registry.enable_histogram_window(TLS_HANDSHAKE_MS, base_config);

    let strategy = format!("raw_cap_{}", Uuid::new_v4().simple());
    let outcome = format!("ok_{}", Uuid::new_v4().simple());
    let labels = [
        ("sni_strategy", strategy.as_str()),
        ("outcome", outcome.as_str()),
    ];

    for sample in [10.0, 20.0, 30.0] {
        registry
            .observe_histogram(TLS_HANDSHAKE_MS, &labels, sample)
            .expect("record sample");
        provider.advance(Duration::from_secs(60));
    }

    let snapshot = registry
        .snapshot_histogram_window(TLS_HANDSHAKE_MS, &labels, WindowRange::LastFiveMinutes, &[])
        .expect("histogram window snapshot");
    assert_eq!(snapshot.raw_samples.len(), 3);
    let initial_values: Vec<f64> = snapshot.raw_samples.iter().map(|s| s.value).collect();
    assert_eq!(initial_values, vec![10.0, 20.0, 30.0]);
    assert!(snapshot
        .raw_samples
        .windows(2)
        .all(|pair| pair[1].offset_seconds > pair[0].offset_seconds));

    registry.enable_histogram_window(
        TLS_HANDSHAKE_MS,
        HistogramWindowConfig::enable_raw_samples(Duration::from_secs(600), 2),
    );
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 40.0)
        .expect("record sample");
    provider.advance(Duration::from_secs(60));

    let snapshot = registry
        .snapshot_histogram_window(TLS_HANDSHAKE_MS, &labels, WindowRange::LastFiveMinutes, &[])
        .expect("histogram window snapshot");
    assert_eq!(snapshot.raw_samples.len(), 2);
    let trimmed: Vec<f64> = snapshot.raw_samples.iter().map(|s| s.value).collect();
    assert_eq!(trimmed, vec![30.0, 40.0]);
    assert!(snapshot.raw_samples.windows(2).all(|pair| pair[1]
        .offset_seconds
        .saturating_sub(pair[0].offset_seconds)
        == 60));

    registry.enable_histogram_window(TLS_HANDSHAKE_MS, HistogramWindowConfig::default());
    provider.advance(Duration::from_secs(60));
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 50.0)
        .expect("record sample");

    let snapshot = registry
        .snapshot_histogram_window(TLS_HANDSHAKE_MS, &labels, WindowRange::LastFiveMinutes, &[])
        .expect("histogram window snapshot");
    assert!(snapshot.raw_samples.is_empty());
    let window_total: u64 = snapshot.points.iter().map(|p| p.count).sum();
    assert_eq!(snapshot.count, window_total);
    assert_eq!(snapshot.count, 4);
}

#[test]
fn runtime_memory_pressure_disables_raw_samples() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600));

    let registry = global_registry();
    registry.enable_histogram_window(
        TLS_HANDSHAKE_MS,
        HistogramWindowConfig::enable_raw_samples(Duration::from_secs(300), 16),
    );

    let labels = [("sni_strategy", "memory-pressure"), ("outcome", "ok")];
    let baseline = registry
        .get_counter(METRIC_MEMORY_PRESSURE_TOTAL, &[])
        .unwrap_or(0);

    set_runtime_memory_limit(1);
    for value in [12.0, 24.0, 48.0, 96.0] {
        observe_histogram_metric(TLS_HANDSHAKE_MS, &labels, value);
    }
    force_memory_pressure_check();

    let after = registry
        .get_counter(METRIC_MEMORY_PRESSURE_TOTAL, &[])
        .unwrap_or(0);
    assert!(
        after >= baseline + 1,
        "memory pressure counter should advance"
    );

    let snapshot = registry
        .snapshot_histogram_window(TLS_HANDSHAKE_MS, &labels, WindowRange::LastFiveMinutes, &[])
        .expect("histogram snapshot");
    assert!(
        snapshot.raw_samples.is_empty(),
        "raw samples should be disabled after memory pressure"
    );

    set_runtime_memory_limit(8_000_000);
    registry.enable_histogram_window(TLS_HANDSHAKE_MS, HistogramWindowConfig::default());
}

#[test]
fn observability_layer_manual_transition_updates_gauge() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    override_layer_guards(Duration::from_secs(0), Duration::from_secs(0));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));
    memory_bus.take_all();

    let initial = current_layer();
    let target = if initial == ObservabilityLayer::Basic {
        ObservabilityLayer::Aggregate
    } else {
        ObservabilityLayer::Basic
    };

    let changed = set_layer(target, Some("manual-test"));
    assert!(changed, "expected layer change to occur");

    std::thread::sleep(Duration::from_millis(10));
    let events = memory_bus.take_all();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Strategy(StrategyEvent::ObservabilityLayerChanged {
            to,
            initiator,
            reason,
            ..
        }) if to == target.as_str()
            && initiator == "manual"
            && reason.as_deref() == Some("manual-test")
    )));

    let layer_value = global_registry()
        .get_gauge(OBSERVABILITY_LAYER, &[])
        .expect("layer gauge should exist");
    assert_eq!(layer_value, target.as_u8() as u64);

    let _ = set_layer(initial, Some("restore"));
    memory_bus.take_all();
}

#[test]
fn observability_layer_auto_downgrade_on_memory_pressure() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    override_layer_guards(Duration::from_secs(0), Duration::from_secs(0));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));
    memory_bus.take_all();

    let _ = set_layer(ObservabilityLayer::Optimize, Some("prepare"));
    memory_bus.take_all();

    let registry = global_registry();
    registry.enable_histogram_window(
        TLS_HANDSHAKE_MS,
        HistogramWindowConfig::enable_raw_samples(Duration::from_secs(120), 8),
    );

    set_runtime_memory_limit(1);
    let labels = [("sni_strategy", "layer-auto"), ("outcome", "ok")];
    for sample in [12.0, 18.0, 24.0, 30.0] {
        observe_histogram_metric(TLS_HANDSHAKE_MS, &labels, sample);
    }
    force_memory_pressure_check();
    set_runtime_memory_limit(8_000_000);

    let downgraded = current_layer();
    assert!(
        downgraded < ObservabilityLayer::Optimize,
        "layer should downgrade after memory pressure"
    );

    let gauge_value = global_registry()
        .get_gauge(OBSERVABILITY_LAYER, &[])
        .expect("layer gauge present");
    assert_eq!(gauge_value, downgraded.as_u8() as u64);

    let events = memory_bus.take_all();
    assert!(events.iter().any(|event| matches!(
        event,
        Event::Strategy(StrategyEvent::ObservabilityLayerChanged {
            initiator,
            reason,
            ..
        }) if initiator == "auto-downgrade"
            && reason.as_deref() == Some("memory_pressure")
    )));

    registry.enable_histogram_window(TLS_HANDSHAKE_MS, HistogramWindowConfig::default());
    let _ = set_layer(ObservabilityLayer::Optimize, Some("restore"));
    memory_bus.take_all();
}

#[test]
fn observability_resolve_config_limits_component_flags() {
    let mut cfg = ObservabilityConfig::default();
    cfg.layer = ObservabilityLayer::Optimize;
    let resolved = resolve_config(&cfg);
    assert_eq!(resolved.effective_layer, ObservabilityLayer::Optimize);
    assert!(resolved.optimize_enabled);

    cfg.aggregate_enabled = false;
    let resolved = resolve_config(&cfg);
    assert_eq!(resolved.effective_layer, ObservabilityLayer::Basic);
    assert!(!resolved.aggregate_enabled);
    assert_eq!(resolved.max_allowed_layer, ObservabilityLayer::Basic);

    cfg.aggregate_enabled = true;
    cfg.export_enabled = false;
    let resolved = resolve_config(&cfg);
    assert_eq!(resolved.effective_layer, ObservabilityLayer::Aggregate);
    assert!(!resolved.export_enabled);
    assert_eq!(resolved.max_allowed_layer, ObservabilityLayer::Aggregate);
}

#[test]
fn observability_reinitialize_clamps_layer_to_config() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    override_layer_guards(Duration::from_secs(0), Duration::from_secs(0));

    let registry = global_registry();
    let _ = set_layer(ObservabilityLayer::Optimize, Some("prep"));
    let mut cfg = ObservabilityConfig::default();
    cfg.layer = ObservabilityLayer::Optimize;
    cfg.export_enabled = false;

    init_basic_observability(&cfg).expect("reinitialize with constrained config");

    let current = current_layer();
    assert_eq!(current, ObservabilityLayer::Aggregate);
    let gauge = registry
        .get_gauge(OBSERVABILITY_LAYER, &[])
        .expect("layer gauge present");
    assert_eq!(gauge, ObservabilityLayer::Aggregate.as_u8() as u64);

    init_basic_observability(&ObservabilityConfig::default())
        .expect("restore full observability config");
    let _ = set_layer(ObservabilityLayer::Optimize, Some("restore"));
}

#[test]
fn observability_auto_downgrade_respects_residency_and_cooldown() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    override_layer_guards(Duration::from_millis(100), Duration::from_millis(200));

    let _ = set_layer(ObservabilityLayer::Basic, Some("prep-basic"));
    let _ = set_layer(ObservabilityLayer::Optimize, Some("prep-optimize"));

    assert!(
        auto_downgrade("guard-check").is_none(),
        "min residency should block downgrade"
    );

    std::thread::sleep(Duration::from_millis(120));
    let first = auto_downgrade("guard-check");
    assert_eq!(first, Some(ObservabilityLayer::Alerts));
    assert_eq!(current_layer(), ObservabilityLayer::Alerts);

    assert!(
        auto_downgrade("guard-check").is_none(),
        "cooldown should block immediate downgrade"
    );

    std::thread::sleep(Duration::from_millis(220));
    let second = auto_downgrade("guard-check");
    assert_eq!(second, Some(ObservabilityLayer::Ui));
    assert_eq!(current_layer(), ObservabilityLayer::Ui);

    override_layer_guards(Duration::from_secs(0), Duration::from_secs(0));
    let _ = set_layer(ObservabilityLayer::Optimize, Some("restore"));
}

#[test]
fn metrics_counter_window_returns_error_when_series_missing() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();

    let registry = global_registry();
    let strategy = format!("missing_series_{}", Uuid::new_v4().simple());
    let labels = [("strategy", strategy.as_str()), ("outcome", "success")];

    let err = registry
        .snapshot_counter_window(
            IP_POOL_SELECTION_TOTAL,
            &labels,
            WindowRange::LastFiveMinutes,
        )
        .expect_err("series should be missing");
    assert!(
        matches!(err, MetricError::SeriesNotFound { metric } if metric == "ip_pool_selection_total")
    );
}

#[test]
fn metrics_histogram_invalid_quantile_rejected() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();

    let registry = global_registry();
    let strategy = format!("invalid_q_{}", Uuid::new_v4().simple());
    let outcome = format!("ok_{}", Uuid::new_v4().simple());
    let labels = [
        ("sni_strategy", strategy.as_str()),
        ("outcome", outcome.as_str()),
    ];

    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &labels, 42.0)
        .expect("record sample");

    let err = registry
        .snapshot_histogram_window(
            TLS_HANDSHAKE_MS,
            &labels,
            WindowRange::LastFiveMinutes,
            &[1.25],
        )
        .expect_err("invalid quantile should be rejected");
    assert!(matches!(err, MetricError::InvalidQuantile(q) if (q - 1.25).abs() < f64::EPSILON));

    let snapshot = registry
        .snapshot_histogram_window(
            TLS_HANDSHAKE_MS,
            &labels,
            WindowRange::LastFiveMinutes,
            &[0.5],
        )
        .expect("valid quantile accepted");
    assert_eq!(snapshot.count, 1);
}

#[test]
fn metrics_counter_window_captures_last_day() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();

    let registry = global_registry();
    let strategy = format!("day_span_{}", Uuid::new_v4().simple());
    let labels = [("strategy", strategy.as_str()), ("outcome", "success")];

    registry
        .incr_counter(IP_POOL_SELECTION_TOTAL, &labels, 5)
        .expect("initial increment");
    provider.set(Duration::from_secs(12 * 3_600));
    registry
        .incr_counter(IP_POOL_SELECTION_TOTAL, &labels, 7)
        .expect("mid increment");
    provider.set(Duration::from_secs(26 * 3_600));
    registry
        .incr_counter(IP_POOL_SELECTION_TOTAL, &labels, 11)
        .expect("recent increment");

    let snapshot = registry
        .snapshot_counter_window(IP_POOL_SELECTION_TOTAL, &labels, WindowRange::LastDay)
        .expect("counter window snapshot");

    assert_eq!(snapshot.points.len(), 24);
    let total: u64 = snapshot.points.iter().map(|p| p.value).sum();
    assert_eq!(total, 18);
    assert_eq!(snapshot.points.last().map(|p| p.value), Some(11));
    assert!(snapshot.points.iter().any(|p| p.value == 7));
    assert!(!snapshot.points.iter().any(|p| p.value == 5));
}

#[test]
fn metrics_series_listing_reports_last_update() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(300));

    let registry = global_registry();
    let strategy = format!("series_counter_{}", Uuid::new_v4().simple());
    let labels = [("strategy", strategy.as_str()), ("outcome", "success")];

    registry
        .incr_counter(IP_POOL_SELECTION_TOTAL, &labels, 3)
        .expect("counter increment");

    provider.advance(Duration::from_secs(60));
    let outcome = format!("series_hist_{}", Uuid::new_v4().simple());
    let hist_labels = [
        ("sni_strategy", strategy.as_str()),
        ("outcome", outcome.as_str()),
    ];
    registry
        .observe_histogram(TLS_HANDSHAKE_MS, &hist_labels, 55.0)
        .expect("record histogram");

    let counter_series = registry
        .list_counter_series(IP_POOL_SELECTION_TOTAL)
        .expect("counter series");
    let counter_desc = counter_series
        .into_iter()
        .find(|desc| desc.labels == vec![strategy.clone(), "success".into()])
        .expect("counter series present");
    assert!(counter_desc
        .last_updated_seconds
        .map(|secs| secs >= 300)
        .unwrap_or(false));

    let hist_series = registry
        .list_histogram_series(TLS_HANDSHAKE_MS)
        .expect("histogram series");
    let hist_desc = hist_series
        .into_iter()
        .find(|desc| desc.labels == vec![strategy.clone(), outcome.clone()])
        .expect("histogram series present");
    assert!(hist_desc
        .last_updated_seconds
        .map(|secs| secs >= 360)
        .unwrap_or(false));
}

#[test]
fn prometheus_encoder_outputs_metrics() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let registry = global_registry();
    let layer_value = current_layer().as_u8();
    let kind = format!("PrometheusTest_{}", Uuid::new_v4());
    let labels = [("kind", kind.as_str()), ("state", "completed")];

    registry
        .incr_counter(GIT_TASKS_TOTAL, &labels, 2)
        .expect("counter increment");
    registry
        .observe_histogram(
            TLS_HANDSHAKE_MS,
            &[("sni_strategy", "fake"), ("outcome", "ok")],
            42.0,
        )
        .expect("histogram sample");

    let output = encode_prometheus(&registry);
    assert!(output.contains("# HELP git_tasks_total"));
    assert!(output.contains(&format!(
        "git_tasks_total{{kind=\"{}\",state=\"completed\"}}",
        kind
    )));
    assert!(output.contains("tls_handshake_ms_bucket"));
    assert!(output.contains("tls_handshake_ms_sum"));
    assert!(output.contains(&format!("observability_layer {}", layer_value)));
}

#[test]
fn snapshot_builder_filters_metrics() {
    let _guard = aggregate_lock();
    ensure_metrics_init();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(120));

    let registry = global_registry();
    let kind = format!("SnapshotCounter_{}", Uuid::new_v4().simple());
    let counter_labels = [("kind", kind.as_str()), ("state", "completed")];

    registry
        .incr_counter(GIT_TASKS_TOTAL, &counter_labels, 1)
        .expect("counter increment");
    provider.advance(Duration::from_secs(60));
    registry
        .observe_histogram(
            TLS_HANDSHAKE_MS,
            &[("sni_strategy", "fake"), ("outcome", "ok")],
            30.0,
        )
        .expect("histogram sample");

    let query = SnapshotQuery {
        names: vec!["git_tasks_total".into(), "tls_handshake_ms".into()],
        range: Some(WindowRange::LastFiveMinutes),
        quantiles: vec![0.5],
        max_series: None,
    };

    let snapshot = build_snapshot(&registry, &query, 0);
    assert!(snapshot
        .series
        .iter()
        .any(|series| series.name == "git_tasks_total"));
    let hist = snapshot
        .series
        .iter()
        .find(|series| series.name == "tls_handshake_ms")
        .expect("histogram series present");
    assert!(hist.count.unwrap_or(0) >= 1);
    if let Some(quantiles) = &hist.quantiles {
        assert!(quantiles.contains_key("p50"));
    }
}

#[test]
fn snapshot_query_rejects_invalid_params() {
    let uri: Uri = "http://localhost/metrics/snapshot?range=15m"
        .parse()
        .unwrap();
    let err = parse_snapshot_query(&uri).expect_err("invalid range should error");
    assert!(matches!(err, SnapshotQueryError::InvalidRange(_)));

    let uri: Uri = "http://localhost/metrics/snapshot?quantiles=p200"
        .parse()
        .unwrap();
    let err = parse_snapshot_query(&uri).expect_err("invalid quantile should error");
    assert!(matches!(err, SnapshotQueryError::InvalidQuantile(_)));

    let uri: Uri = "http://localhost/metrics/snapshot?maxSeries=abc"
        .parse()
        .unwrap();
    let err = parse_snapshot_query(&uri).expect_err("invalid maxSeries should error");
    assert!(matches!(err, SnapshotQueryError::InvalidMaxSeries(_)));
}

#[test]
fn alerts_engine_triggers_and_resolves() {
    let _guard = aggregate_lock();
    ensure_alerts_engine_initialized();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));

    // 使用唯一 kind 标签限制统计范围
    let kind = format!("GitAlert_{}", Uuid::new_v4());
    // 规则表达式需符合 Prometheus 语法: label value 必须使用双引号
    let rule_spec = format!(
        r#"[{{
  "id": "git_fail_rate",
  "expr": "git_tasks_total{{kind=\"{kind}\",state=\"failed\"}}/git_tasks_total{{kind=\"{kind}\"}} > 0.3",
  "severity": "warn",
  "window": "5m"
}}]"#
    );
    fs::write(alerts_rules_path(), rule_spec).expect("write rule file");
    memory_bus.take_all();
    evaluate_alerts_now();
    memory_bus.take_all();

    let registry = global_registry();
    // 记录本测试专属 kind 初始值（初次应为 0）
    let failed_before = registry
        .get_counter(GIT_TASKS_TOTAL, &[("kind", kind.as_str()), ("state", "failed")])
        .unwrap_or(0);
    let completed_before = registry
        .get_counter(GIT_TASKS_TOTAL, &[("kind", kind.as_str()), ("state", "completed")])
        .unwrap_or(0);

    // 简化策略：直接注入失败 3、成功 2，形成 3/(3+2)=0.6 > 0.3，避免全局指标干扰复杂计算
    let f: u64 = 3; // failures
    let s: u64 = 2; // successes

    for idx in 0..f {
        let id = format!("{kind}-fail-{idx}");
        publish_global(Event::Task(TaskEvent::Started { id: id.clone(), kind: kind.clone() }));
        publish_global(Event::Task(TaskEvent::Failed {
            id,
            category: "Network".into(),
            code: None,
            message: "fail".into(),
        }));
    }
    for idx in 0..s {
        let id = format!("{kind}-ok-{idx}");
        publish_global(Event::Task(TaskEvent::Started { id: id.clone(), kind: kind.clone() }));
        publish_global(Event::Task(TaskEvent::Completed { id }));
    }

    let warn_before = registry
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);
    wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "failed")],
        failed_before + 4,
    );
    wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "completed")],
        completed_before + 2,
    );

    evaluate_alerts_now();

    let events_first = memory_bus.take_all();
    let warn_after = registry
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);
    assert!(
        warn_after >= warn_before + 1,
        "expected at least one firing alert"
    );

    let firing_states: Vec<MetricAlertState> = events_first
        .into_iter()
        .filter_map(|event| match event {
            Event::Strategy(StrategyEvent::MetricAlert { state, .. }) => Some(state),
            _ => None,
        })
        .collect();
    assert!(firing_states
        .iter()
        .any(|state| matches!(state, MetricAlertState::Firing)));

    // 现在添加一批成功事件用于让告警进入 Resolved 状态
    let resolve_successes = (f * 2).max(6); // 足够数量拉低失败率
    for idx in 0..resolve_successes {
        let id = format!("{kind}-resolve-{idx}");
        publish_global(Event::Task(TaskEvent::Started { id: id.clone(), kind: kind.clone() }));
        publish_global(Event::Task(TaskEvent::Completed { id }));
    }

    wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "completed")],
        completed_before + resolve_successes,
    );

    evaluate_alerts_now();

    let events_second = memory_bus.take_all();
    let states: Vec<MetricAlertState> = events_second
        .into_iter()
        .filter_map(|event| match event {
            Event::Strategy(StrategyEvent::MetricAlert { state, .. }) => Some(state),
            _ => None,
        })
        .collect();
    assert!(states
        .iter()
        .any(|state| matches!(state, MetricAlertState::Resolved)));
}

#[test]
fn alerts_engine_uses_builtin_rules_when_file_missing() {
    let _guard = aggregate_lock();
    ensure_alerts_engine_initialized();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));

    if alerts_rules_path().exists() {
        let _ = fs::remove_file(alerts_rules_path());
    }
    memory_bus.take_all();
    evaluate_alerts_now();
    memory_bus.take_all();

    let registry = global_registry();
    let warn_before = registry
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);

    let kind = format!("GitClone_{}", Uuid::new_v4());
    let (mut window_fail_before, mut window_total_before) = current_git_totals(&registry);
    let mut balance_completions: u64 = 0;
    if window_fail_before > 0 {
        let required_total = window_fail_before.saturating_mul(20).saturating_add(1);
        if window_total_before < required_total {
            balance_completions = required_total - window_total_before;
            let needed =
                usize::try_from(balance_completions).expect("balance completions should fit usize");
            for idx in 0..needed {
                let id = format!("{kind}-balance-{idx}");
                publish_global(Event::Task(TaskEvent::Started {
                    id: id.clone(),
                    kind: kind.clone(),
                }));
                publish_global(Event::Task(TaskEvent::Completed { id }));
            }
            provider.advance(Duration::from_secs(60));
            evaluate_alerts_now();
            memory_bus.take_all();
            let (fail_after_balance, total_after_balance) = current_git_totals(&registry);
            window_fail_before = fail_after_balance;
            window_total_before = total_after_balance;
        }
    }
    let balanced_completed = wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "completed")],
        balance_completions,
    );
    let failed_before = registry
        .get_counter(
            GIT_TASKS_TOTAL,
            &[("kind", kind.as_str()), ("state", "failed")],
        )
        .unwrap_or(0);
    for idx in 0..10 {
        let id = format!("{kind}-warmup-{idx}");
        publish_global(Event::Task(TaskEvent::Started {
            id: id.clone(),
            kind: kind.clone(),
        }));
        publish_global(Event::Task(TaskEvent::Completed { id }));
    }

    let completed_before = wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "completed")],
        balanced_completed + 10,
    );

    provider.advance(Duration::from_secs(60));
    evaluate_alerts_now();
    memory_bus.take_all();

    for idx in 0..200 {
        let id = format!("{kind}-{idx}");
        publish_global(Event::Task(TaskEvent::Started {
            id: id.clone(),
            kind: kind.clone(),
        }));
        if idx < 180 {
            publish_global(Event::Task(TaskEvent::Failed {
                id,
                category: "Network".into(),
                code: None,
                message: "fail".into(),
            }));
        } else {
            publish_global(Event::Task(TaskEvent::Completed { id }));
        }
    }

    let failed = wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "failed")],
        failed_before + 180,
    );
    let completed = wait_for_counter(
        &registry,
        GIT_TASKS_TOTAL,
        &[("kind", kind.as_str()), ("state", "completed")],
        completed_before + 20,
    );
    assert_eq!(
        failed,
        failed_before + 180,
        "expected failed count to match workload"
    );
    assert_eq!(
        completed,
        completed_before + 20,
        "expected completed count to match workload",
    );

    provider.advance(Duration::from_secs(60));
    evaluate_alerts_now();

    let events = memory_bus.take_all();
    let warn_after = registry
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);
    let (window_fail_after, window_total_after) = current_git_totals(&registry);
    assert!(
        warn_after >= warn_before + 1,
        "expected builtin rule to fire; warn_before={warn_before} warn_after={warn_after} fail_before={window_fail_before} total_before={window_total_before} fail_after={window_fail_after} total_after={window_total_after} balance_completions={balance_completions} events={events:?}"
    );

    assert!(events.iter().any(|event| matches!(
        event,
        Event::Strategy(StrategyEvent::MetricAlert {
            rule_id,
            state,
            ..
        }) if rule_id == "git_fail_rate"
            && matches!(state, MetricAlertState::Firing | MetricAlertState::Active)
    )));
}

#[test]
fn alerts_engine_respects_min_repeat_interval() {
    let _guard = aggregate_lock();
    ensure_alerts_engine_initialized();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));

    let rule_spec = r#"[
        {
            "id": "repeat_interval_rule",
            "expr": "ip_pool_refresh_total{reason=repeat_test,success=false} > 0",
            "severity": "warn",
            "window": "5m"
        }
    ]"#;
    fs::write(alerts_rules_path(), rule_spec).expect("write repeat rule");
    memory_bus.take_all();
    evaluate_alerts_now();
    memory_bus.take_all();

    publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
        id: "repeat-1".into(),
        domain: "example.com".into(),
        success: false,
        candidates_count: 0,
        min_latency_ms: None,
        max_latency_ms: None,
        reason: "repeat_test".into(),
    }));

    evaluate_alerts_now();
    let first_events = memory_bus.take_all();
    assert!(first_events.iter().any(|event| matches!(
        event,
        Event::Strategy(StrategyEvent::MetricAlert { rule_id, .. })
            if rule_id == "repeat_interval_rule"
    )));

    evaluate_alerts_now();
    let second_events = memory_bus.take_all();
    assert!(second_events.iter().all(|event| !matches!(
        event,
        Event::Strategy(StrategyEvent::MetricAlert { rule_id, .. })
            if rule_id == "repeat_interval_rule"
    )));
}

#[test]
fn alerts_engine_division_by_zero_skips_rule() {
    let _guard = aggregate_lock();
    ensure_alerts_engine_initialized();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));

    let rule_spec = r#"[
        {
            "id": "div_zero_rule",
            "expr": "ip_pool_refresh_total{reason=div_zero,success=true}/git_tasks_total{kind=div_zero_kind} > 0.5",
            "severity": "warn",
            "window": "5m"
        }
    ]"#;
    fs::write(alerts_rules_path(), rule_spec).expect("write div zero rule");
    memory_bus.take_all();
    evaluate_alerts_now();
    memory_bus.take_all();

    publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
        id: "div-zero".into(),
        domain: "example.com".into(),
        success: true,
        candidates_count: 1,
        min_latency_ms: Some(10),
        max_latency_ms: Some(15),
        reason: "div_zero".into(),
    }));

    let warn_before = global_registry()
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);

    evaluate_alerts_now();
    let events = memory_bus.take_all();
    let warn_after = global_registry()
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);

    let unexpected_alert = events.iter().find(|event| {
        matches!(
            event,
            Event::Strategy(StrategyEvent::MetricAlert { rule_id, .. })
                if rule_id == "div_zero_rule"
        )
    });
    assert!(unexpected_alert.is_none(), "div_zero_rule should not fire");
    assert_eq!(
        warn_before, warn_after,
        "division by zero rule should be skipped"
    );
}

#[test]
fn alerts_engine_hot_reload_applies_new_rules() {
    let _guard = aggregate_lock();
    ensure_alerts_engine_initialized();
    let provider = ensure_aggregate_init();
    provider.reset();
    provider.set(Duration::from_secs(600));

    let fanout = ensure_fanout_bus().expect("fanout bus available");
    let memory_bus = MemoryEventBus::new();
    fanout.register(Arc::new(memory_bus.clone()));

    let initial_spec = r#"[
        {
            "id": "hot_reload_rule",
            "expr": "ip_pool_refresh_total{reason=hot_reload,success=false} > 5",
            "severity": "warn",
            "window": "5m"
        }
    ]"#;
    fs::write(alerts_rules_path(), initial_spec).expect("write initial rule");
    memory_bus.take_all();
    evaluate_alerts_now();
    memory_bus.take_all();

    for idx in 0..3 {
        publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
            id: format!("hot-reload-{idx}"),
            domain: "example.com".into(),
            success: false,
            candidates_count: 0,
            min_latency_ms: None,
            max_latency_ms: None,
            reason: "hot_reload".into(),
        }));
    }

    evaluate_alerts_now();
    let baseline_events = memory_bus.take_all();
    let baseline_alert = baseline_events.iter().find(|event| {
        matches!(
            event,
            Event::Strategy(StrategyEvent::MetricAlert { rule_id, .. })
                if rule_id == "hot_reload_rule"
        )
    });
    assert!(
        baseline_alert.is_none(),
        "hot_reload_rule should not fire before reload"
    );

    let warn_before = global_registry()
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);

    let updated_spec = r#"[
        {
            "id": "hot_reload_rule",
            "expr": "ip_pool_refresh_total{reason=hot_reload,success=false} > 0",
            "severity": "warn",
            "window": "5m"
        }
    ]"#;
    fs::write(alerts_rules_path(), updated_spec).expect("write updated rule");
    memory_bus.take_all();
    evaluate_alerts_now();
    let updated_events = memory_bus.take_all();
    let warn_after = global_registry()
        .get_counter(ALERTS_FIRED_TOTAL, &[("severity", "warn")])
        .unwrap_or(0);

    assert!(
        warn_after >= warn_before + 1,
        "hot reload should trigger alert"
    );
    assert!(updated_events.iter().any(|event| matches!(
        event,
        Event::Strategy(StrategyEvent::MetricAlert {
            rule_id,
            state,
            ..
        }) if rule_id == "hot_reload_rule"
            && matches!(state, MetricAlertState::Firing | MetricAlertState::Active)
    )));

    for idx in 0..6 {
        publish_global(Event::Strategy(StrategyEvent::IpPoolRefresh {
            id: format!("hot-reload-resolve-{idx}"),
            domain: "example.com".into(),
            success: true,
            candidates_count: 1,
            min_latency_ms: Some(15),
            max_latency_ms: Some(20),
            reason: "hot_reload".into(),
        }));
    }
    evaluate_alerts_now();
    memory_bus.take_all();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_http_server_enforces_auth() {
    ensure_metrics_init();
    let cfg = ObservabilityExportConfig {
        auth_token: Some("secret-token".into()),
        rate_limit_qps: 1,
        max_series_per_snapshot: 50,
        bind_address: "127.0.0.1:0".into(),
    };

    let handle = fireworks_collaboration_lib::core::metrics::start_http_server(&cfg)
        .expect("start http server");
    let addr = handle.local_addr;
    let client: Client<HttpConnector, Body> = Client::new();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let uri = format!("http://{}/metrics", addr).parse::<Uri>().unwrap();
    let resp = client
        .request(
            Request::builder()
                .method("GET")
                .uri(uri.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request without auth");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = client
        .request(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("Authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("authorized request");
    assert_eq!(resp.status(), StatusCode::OK);

    let res_second = client
        .request(
            Request::builder()
                .method("GET")
                .uri(format!("http://{}/metrics", addr).parse::<Uri>().unwrap())
                .header("Authorization", "Bearer secret-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("second authorized request");
    assert!(
        res_second.status() == StatusCode::OK
            || res_second.status() == StatusCode::TOO_MANY_REQUESTS
    );

    handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_http_server_applies_rate_limit() {
    ensure_metrics_init();
    let registry = global_registry();
    let cfg = ObservabilityExportConfig {
        auth_token: None,
        rate_limit_qps: 1,
        max_series_per_snapshot: 50,
        bind_address: "127.0.0.1:0".into(),
    };

    let handle = fireworks_collaboration_lib::core::metrics::start_http_server(&cfg)
        .expect("start http server");
    let addr = handle.local_addr;
    let client: Client<HttpConnector, Body> = Client::new();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let uri: Uri = format!("http://{}/metrics", addr).parse().unwrap();
    let baseline_rate_limited = registry
        .get_counter(METRICS_EXPORT_RATE_LIMITED_TOTAL, &[])
        .unwrap_or(0);
    let baseline_success = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "success")])
        .unwrap_or(0);

    let mut success_count = 0;
    let mut rate_limited_count = 0;

    for _ in 0..3 {
        let response = client
            .request(
                Request::builder()
                    .method("GET")
                    .uri(uri.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("metrics request");
        let status = response.status();
        let _ = to_bytes(response.into_body()).await.expect("drain body");
        match status {
            StatusCode::OK => success_count += 1,
            StatusCode::TOO_MANY_REQUESTS => rate_limited_count += 1,
            other => panic!("unexpected status: {}", other),
        }
    }

    assert!(
        rate_limited_count >= 1,
        "expected at least one rate-limited response"
    );

    let updated_rate_limited = registry
        .get_counter(METRICS_EXPORT_RATE_LIMITED_TOTAL, &[])
        .unwrap_or(0);
    assert!(
        updated_rate_limited >= baseline_rate_limited + rate_limited_count,
        "rate limited counter did not advance"
    );

    let updated_success = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "success")])
        .unwrap_or(0);
    assert!(
        updated_success >= baseline_success + success_count as u64,
        "success counter did not advance"
    );

    handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_snapshot_endpoint_respects_limits() {
    ensure_metrics_init();
    let registry = global_registry();
    let kind = format!("snapshot_limit_{}", Uuid::new_v4().simple());
    let counter_labels = [("kind", kind.as_str()), ("state", "completed")];
    registry
        .incr_counter(GIT_TASKS_TOTAL, &counter_labels, 1)
        .expect("counter increment");
    registry
        .observe_histogram(
            TLS_HANDSHAKE_MS,
            &[("sni_strategy", "fake"), ("outcome", "ok")],
            45.0,
        )
        .expect("histogram sample");

    let cfg = ObservabilityExportConfig {
        auth_token: None,
        rate_limit_qps: 5,
        max_series_per_snapshot: 1,
        bind_address: "127.0.0.1:0".into(),
    };

    let handle = fireworks_collaboration_lib::core::metrics::start_http_server(&cfg)
        .expect("start http server");
    let addr = handle.local_addr;
    let client: Client<HttpConnector, Body> = Client::new();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let baseline_series = registry
        .get_counter(METRICS_EXPORT_SERIES_TOTAL, &[("endpoint", "snapshot")])
        .unwrap_or(0);

    let uri: Uri = format!(
        "http://{}/metrics/snapshot?names=git_tasks_total,tls_handshake_ms&range=1m&quantiles=p50",
        addr
    )
    .parse()
    .unwrap();

    let response = client
        .request(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("snapshot request");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body()).await.expect("read body");
    let payload: Value = serde_json::from_slice(&body).expect("parse snapshot json");
    let series = payload
        .get("series")
        .and_then(|value| value.as_array())
        .expect("series array");
    assert_eq!(series.len(), 1, "max_series limit should cap results");

    let entry = series[0]
        .as_object()
        .expect("series entry should be object");
    assert_eq!(
        entry.get("name").and_then(Value::as_str),
        Some("git_tasks_total")
    );
    assert_eq!(entry.get("type").and_then(Value::as_str), Some("counter"));
    let total_value = entry
        .get("value")
        .and_then(Value::as_u64)
        .expect("counter value present");
    assert!(total_value >= 1, "counter value should be at least one");

    let updated_series = registry
        .get_counter(METRICS_EXPORT_SERIES_TOTAL, &[("endpoint", "snapshot")])
        .unwrap_or(0);
    assert!(
        updated_series >= baseline_series + 1,
        "series counter did not advance"
    );

    handle.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_http_server_records_request_statuses() {
    ensure_metrics_init();
    let registry = global_registry();

    let series_kind = format!("status_counter_{}", Uuid::new_v4().simple());
    let counter_labels = [("kind", series_kind.as_str()), ("state", "completed")];
    registry
        .incr_counter(GIT_TASKS_TOTAL, &counter_labels, 4)
        .expect("counter increment");

    let cfg = ObservabilityExportConfig {
        auth_token: Some("status-token".into()),
        rate_limit_qps: 5,
        max_series_per_snapshot: 100,
        bind_address: "127.0.0.1:0".into(),
    };

    let handle = fireworks_collaboration_lib::core::metrics::start_http_server(&cfg)
        .expect("start http server");
    let addr = handle.local_addr;
    let client: Client<HttpConnector, Body> = Client::new();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let baseline_unauthorized = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "unauthorized")])
        .unwrap_or(0);
    let baseline_method_not_allowed = registry
        .get_counter(
            METRICS_EXPORT_REQUESTS_TOTAL,
            &[("status", "method_not_allowed")],
        )
        .unwrap_or(0);
    let baseline_bad_request = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "bad_request")])
        .unwrap_or(0);
    let baseline_success = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "success")])
        .unwrap_or(0);
    let baseline_prom_series = registry
        .get_counter(METRICS_EXPORT_SERIES_TOTAL, &[("endpoint", "prometheus")])
        .unwrap_or(0);

    let metrics_uri: Uri = format!("http://{}/metrics", addr).parse().unwrap();
    let snapshot_bad_uri: Uri = format!("http://{}/metrics/snapshot?range=invalid", addr)
        .parse()
        .unwrap();

    let unauthorized_resp = client
        .request(
            Request::builder()
                .method("GET")
                .uri(metrics_uri.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("unauthorized request");
    assert_eq!(unauthorized_resp.status(), StatusCode::UNAUTHORIZED);
    let _ = to_bytes(unauthorized_resp.into_body())
        .await
        .expect("drain body");

    let method_resp = client
        .request(
            Request::builder()
                .method("POST")
                .uri(metrics_uri.clone())
                .header("Authorization", "Bearer status-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("method not allowed request");
    assert_eq!(method_resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    let _ = to_bytes(method_resp.into_body()).await.expect("drain body");

    let bad_resp = client
        .request(
            Request::builder()
                .method("GET")
                .uri(snapshot_bad_uri)
                .header("Authorization", "Bearer status-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("bad snapshot request");
    assert_eq!(bad_resp.status(), StatusCode::BAD_REQUEST);
    let _ = to_bytes(bad_resp.into_body()).await.expect("drain body");

    let success_resp = client
        .request(
            Request::builder()
                .method("GET")
                .uri(metrics_uri)
                .header("Authorization", "Bearer status-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("authorized metrics request");
    assert_eq!(success_resp.status(), StatusCode::OK);
    let _ = to_bytes(success_resp.into_body())
        .await
        .expect("drain body");

    let unauthorized_after = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "unauthorized")])
        .unwrap_or(0);
    assert!(
        unauthorized_after >= baseline_unauthorized + 1,
        "unauthorized counter did not advance"
    );

    let method_after = registry
        .get_counter(
            METRICS_EXPORT_REQUESTS_TOTAL,
            &[("status", "method_not_allowed")],
        )
        .unwrap_or(0);
    assert!(
        method_after >= baseline_method_not_allowed + 1,
        "method_not_allowed counter did not advance"
    );

    let bad_after = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "bad_request")])
        .unwrap_or(0);
    assert!(
        bad_after >= baseline_bad_request + 1,
        "bad_request counter did not advance"
    );

    let success_after = registry
        .get_counter(METRICS_EXPORT_REQUESTS_TOTAL, &[("status", "success")])
        .unwrap_or(0);
    assert!(
        success_after >= baseline_success + 1,
        "success counter did not advance"
    );

    let prom_series_after = registry
        .get_counter(METRICS_EXPORT_SERIES_TOTAL, &[("endpoint", "prometheus")])
        .unwrap_or(0);
    assert!(
        prom_series_after >= baseline_prom_series + 1,
        "prometheus series counter did not advance"
    );

    handle.shutdown().await;
}
