#![cfg(not(feature = "tauri-app"))]

#[path = "common/mod.rs"]
mod common;

use crate::common::fixtures::create_empty_dir;
use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

use fireworks_collaboration_lib::core::config::loader::testing::{
    clear_global_base_dir, override_global_base_dir,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::git::transport::metrics::{
    tl_reset as tl_metrics_reset, tl_snapshot,
};
use fireworks_collaboration_lib::core::git::transport::testing::{
    auto_disable_guard, classify_and_count_fallback, inject_fake_failure, inject_real_failure,
    reset_auto_disable, reset_fallback_counters, reset_injected_failures,
    snapshot_fallback_counters, TestSubtransport,
};
use fireworks_collaboration_lib::core::git::transport::{
    is_fake_disabled, tl_take_fallback_events, AutoDisableConfig, FallbackEventRecord,
};
use fireworks_collaboration_lib::core::ip_pool::config::{
    EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig, IpPoolSourceToggle, UserStaticIp,
};
use fireworks_collaboration_lib::core::ip_pool::global::testing::reset_global_pool;
use fireworks_collaboration_lib::core::ip_pool::global::{obtain_global_pool, set_global_pool};
use fireworks_collaboration_lib::core::ip_pool::{
    IpCacheKey, IpCacheSlot, IpCandidate, IpPool, IpSource, IpStat,
};
use rcgen::generate_simple_self_signed;
use rustls::{
    Certificate as RustlsCertificate, PrivateKey as RustlsPrivateKey,
    ServerConfig as RustlsServerConfig, ServerConnection,
};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

fn counter_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

fn spawn_tls_server(ip: &str, port: u16) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
    let cert = generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let server_config = RustlsServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(
            vec![RustlsCertificate(cert_der)],
            RustlsPrivateKey(priv_key),
        )
        .unwrap();
    let config = Arc::new(server_config);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let socket = format!("{}:{}", ip, port);
    let handle = thread::spawn(move || {
        let listener = TcpListener::bind(&socket).expect("bind tls server");
        listener.set_nonblocking(true).expect("set nonblocking");
        while !stop_clone.load(AtomicOrdering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    stream.set_nonblocking(false).ok();
                    if let Ok(mut conn) = ServerConnection::new(Arc::clone(&config)) {
                        let _ = conn.complete_io(&mut stream);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    if stop_clone.load(AtomicOrdering::Relaxed) {
                        break;
                    }
                    panic!("tls server accept failed: {e}");
                }
            }
        }
    });
    (stop, handle)
}

fn spawn_fail_server(ip: &str, port: u16) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let socket = format!("{}:{}", ip, port);
    let handle = thread::spawn(move || {
        let listener = TcpListener::bind(&socket).expect("bind fail server");
        listener.set_nonblocking(true).expect("set nonblocking");
        while !stop_clone.load(AtomicOrdering::Relaxed) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    let _ = stream.shutdown(Shutdown::Both);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(e) => {
                    if stop_clone.load(AtomicOrdering::Relaxed) {
                        break;
                    }
                    panic!("fail server accept failed: {e}");
                }
            }
        }
    });
    (stop, handle)
}

fn pick_free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("pick free port")
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn classify_pin_error_as_verify() {
    let _lock = counter_guard().lock().unwrap();
    reset_fallback_counters();
    let category = classify_and_count_fallback("cert_fp_pin_mismatch");
    assert_eq!(category, "Verify");
    let (tls_total, verify_total) = snapshot_fallback_counters();
    assert_eq!(tls_total, 0);
    assert_eq!(verify_total, 1);
}

#[test]
fn classify_tls_error_falls_back_to_tls() {
    let _lock = counter_guard().lock().unwrap();
    reset_fallback_counters();
    let category = classify_and_count_fallback("handshake failure");
    assert_eq!(category, "Tls");
    let (tls_total, verify_total) = snapshot_fallback_counters();
    assert_eq!(tls_total, 1);
    assert_eq!(verify_total, 0);
}

#[test]
fn fallback_transition_emits_events_and_triggers_auto_disable() {
    let _auto_guard = auto_disable_guard().lock().unwrap();
    let _lock = counter_guard().lock().unwrap();
    reset_fallback_counters();
    reset_auto_disable();
    reset_injected_failures();
    let cfg = AppConfig::default();
    let sub = TestSubtransport::new(cfg.clone());
    let mut auto_disable_seen = false;
    for i in 0..5 {
        inject_fake_failure(format!("fake-fail-{i}"));
        inject_real_failure(format!("real-fail-{i}"));
        let res = sub.connect_tls_with_fallback("github.com", 443);
        assert!(res.is_err(), "expected injected failure on attempt {i}");
        let events = tl_take_fallback_events();
        assert!(
            events.iter().any(|evt| matches!(
                evt,
                FallbackEventRecord::Transition { from, to, reason }
                    if *from == "Fake" && *to == "Real" && reason == "FakeHandshakeError"
            )),
            "missing Fake->Real transition on attempt {i}"
        );
        if events.iter().any(|evt| {
            matches!(
                evt,
                FallbackEventRecord::AutoDisable {
                    enabled: true,
                    threshold_pct,
                    cooldown_secs,
                }
                if *threshold_pct == cfg.http.auto_disable_fake_threshold_pct
                    && *cooldown_secs == cfg.http.auto_disable_fake_cooldown_sec as u32
            )
        }) {
            auto_disable_seen = true;
        }
    }
    assert!(auto_disable_seen, "auto-disable event not observed");
    let auto_cfg = AutoDisableConfig::from_http_cfg(&cfg.http);
    assert!(
        is_fake_disabled(&auto_cfg),
        "fake SNI should be disabled after trigger"
    );
    reset_auto_disable();
}

#[test]
fn ip_pool_candidate_successfully_used() {
    let _guard = counter_guard().lock().unwrap();
    tl_metrics_reset();
    reset_global_pool();

    let temp_dir = create_empty_dir();
    override_global_base_dir(&temp_dir);

    let port = pick_free_port();
    let (stop_tls, tls_handle) = spawn_tls_server("127.0.0.1", port);

    let runtime = IpPoolRuntimeConfig {
        enabled: true,
        sources: IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: true,
            fallback: false,
        },
        max_parallel_probes: 2,
        probe_timeout_ms: 1_000,
        history_path: None,
        cache_prune_interval_secs: 60,
        max_cache_entries: 16,
        singleflight_timeout_ms: 5_000,
    };
    let file = IpPoolFileConfig {
        preheat_domains: Vec::new(),
        score_ttl_seconds: 120,
        user_static: vec![UserStaticIp {
            host: "localhost".into(),
            ip: "127.0.0.1".into(),
            ports: vec![port],
        }],
    };
    let effective = EffectiveIpPoolConfig::from_parts(runtime, file);
    let pool = IpPool::new(effective);
    set_global_pool(Arc::new(Mutex::new(pool)));

    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = false;
    cfg.tls.san_whitelist = vec!["localhost".into()];
    cfg.tls.insecure_skip_verify = true;

    let sub = TestSubtransport::new(cfg);
    let res = sub.connect_tls_with_fallback("localhost", port);
    assert!(res.is_ok(), "expected TLS connection via ip pool candidate");

    let global_pool = obtain_global_pool();
    let guard = global_pool.lock().unwrap();
    let metrics = guard
        .outcome_metrics("localhost", port)
        .expect("metrics available");
    assert_eq!(metrics.success, 1);
    assert_eq!(metrics.failure, 0);
    assert!(
        metrics.last_outcome_ms > 0,
        "last outcome timestamp missing"
    );

    let candidate_metrics = guard
        .candidate_outcome_metrics("localhost", port, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
        .expect("candidate metrics available");
    assert_eq!(candidate_metrics.success, 1);
    assert_eq!(candidate_metrics.failure, 0);
    assert!(
        candidate_metrics.last_outcome_ms > 0,
        "candidate last outcome timestamp missing"
    );
    assert_eq!(candidate_metrics.last_sources, vec![IpSource::UserStatic]);
    drop(guard);

    let snap = tl_snapshot();
    assert_eq!(snap.ip_strategy, Some("Cached"));
    assert_eq!(snap.ip_source.as_deref(), Some("UserStatic"));
    assert!(snap.ip_latency_ms.is_some());

    stop_tls.store(true, AtomicOrdering::Relaxed);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
    tls_handle.join().unwrap();

    reset_global_pool();
    clear_global_base_dir();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn ip_pool_candidate_exhaustion_falls_back_to_system() {
    let _guard = counter_guard().lock().unwrap();
    tl_metrics_reset();
    reset_global_pool();

    let temp_dir = create_empty_dir();
    override_global_base_dir(&temp_dir);

    let port = pick_free_port();
    let (stop_tls, tls_handle) = spawn_tls_server("127.0.0.1", port);
    let (stop_fail_a, fail_handle_a) = spawn_fail_server("127.0.0.2", port);
    let (stop_fail_b, fail_handle_b) = spawn_fail_server("127.0.0.3", port);

    let runtime = IpPoolRuntimeConfig {
        enabled: true,
        sources: IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: true,
            fallback: false,
        },
        max_parallel_probes: 2,
        probe_timeout_ms: 500,
        history_path: None,
        cache_prune_interval_secs: 60,
        max_cache_entries: 16,
        singleflight_timeout_ms: 3_000,
    };
    let file = IpPoolFileConfig {
        preheat_domains: Vec::new(),
        score_ttl_seconds: 30,
        user_static: vec![
            UserStaticIp {
                host: "localhost".into(),
                ip: "127.0.0.2".into(),
                ports: vec![port],
            },
            UserStaticIp {
                host: "localhost".into(),
                ip: "127.0.0.3".into(),
                ports: vec![port],
            },
        ],
    };
    let effective = EffectiveIpPoolConfig::from_parts(runtime, file);
    let pool = IpPool::new(effective);
    set_global_pool(Arc::new(Mutex::new(pool)));

    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = false;
    cfg.tls.san_whitelist = vec!["localhost".into()];
    cfg.tls.insecure_skip_verify = true;

    let sub = TestSubtransport::new(cfg);
    let res = sub.connect_tls_with_fallback("localhost", port);
    assert!(
        res.is_ok(),
        "expected fallback to system connection to succeed"
    );

    let global_pool = obtain_global_pool();
    let guard = global_pool.lock().unwrap();
    let metrics = guard
        .outcome_metrics("localhost", port)
        .expect("metrics available");
    assert_eq!(metrics.success, 0);
    assert_eq!(metrics.failure, 1);
    assert!(
        metrics.last_outcome_ms > 0,
        "last outcome timestamp missing"
    );

    let fail_metrics_a = guard
        .candidate_outcome_metrics("localhost", port, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)))
        .expect("first failure candidate metrics available");
    assert_eq!(fail_metrics_a.success, 0);
    assert!(
        fail_metrics_a.failure >= 1,
        "expected at least one recorded failure for first candidate"
    );
    assert!(
        fail_metrics_a.last_outcome_ms > 0,
        "first candidate missing last outcome timestamp"
    );
    assert_eq!(fail_metrics_a.last_sources, vec![IpSource::UserStatic]);

    let fail_metrics_b = guard
        .candidate_outcome_metrics("localhost", port, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 3)))
        .expect("second failure candidate metrics available");
    assert_eq!(fail_metrics_b.success, 0);
    assert!(
        fail_metrics_b.failure >= 1,
        "expected at least one recorded failure for second candidate"
    );
    assert!(
        fail_metrics_b.last_outcome_ms > 0,
        "second candidate missing last outcome timestamp"
    );
    assert_eq!(fail_metrics_b.last_sources, vec![IpSource::UserStatic]);
    drop(guard);

    let snap = tl_snapshot();
    assert_eq!(snap.ip_strategy, Some("Cached"));
    assert!(snap.ip_source.is_none());
    assert!(snap.ip_latency_ms.is_none());

    stop_fail_a.store(true, AtomicOrdering::Relaxed);
    stop_fail_b.store(true, AtomicOrdering::Relaxed);
    let addr_a = SocketAddr::from(([127, 0, 0, 2], port));
    let addr_b = SocketAddr::from(([127, 0, 0, 3], port));
    let _ = TcpStream::connect_timeout(&addr_a, Duration::from_millis(200));
    let _ = TcpStream::connect_timeout(&addr_b, Duration::from_millis(200));
    fail_handle_a.join().unwrap();
    fail_handle_b.join().unwrap();

    stop_tls.store(true, AtomicOrdering::Relaxed);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
    tls_handle.join().unwrap();

    reset_global_pool();
    clear_global_base_dir();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn ip_pool_second_candidate_recovers_after_failure() {
    let _guard = counter_guard().lock().unwrap();
    tl_metrics_reset();
    reset_global_pool();

    let temp_dir = create_empty_dir();
    override_global_base_dir(&temp_dir);

    let port = pick_free_port();
    let (stop_tls, tls_handle) = spawn_tls_server("127.0.0.1", port);
    let (stop_fail, fail_handle) = spawn_fail_server("127.0.0.2", port);

    let runtime = IpPoolRuntimeConfig {
        enabled: true,
        sources: IpPoolSourceToggle {
            builtin: false,
            dns: false,
            history: false,
            user_static: true,
            fallback: false,
        },
        max_parallel_probes: 2,
        probe_timeout_ms: 1_000,
        history_path: None,
        cache_prune_interval_secs: 60,
        max_cache_entries: 16,
        singleflight_timeout_ms: 5_000,
    };
    let file = IpPoolFileConfig {
        preheat_domains: Vec::new(),
        score_ttl_seconds: 120,
        user_static: vec![],
    };
    let pool = IpPool::new(EffectiveIpPoolConfig::from_parts(runtime, file));

    let mut fail_stat = IpStat::with_latency(
        IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            port,
            IpSource::UserStatic,
        ),
        5,
    );
    fail_stat.measured_at_epoch_ms = Some(1);
    fail_stat.expires_at_epoch_ms = Some(i64::MAX - 1);
    fail_stat.sources = vec![IpSource::UserStatic];

    let mut success_stat = IpStat::with_latency(
        IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port,
            IpSource::UserStatic,
        ),
        10,
    );
    success_stat.measured_at_epoch_ms = Some(1);
    success_stat.expires_at_epoch_ms = Some(i64::MAX - 1);
    success_stat.sources = vec![IpSource::UserStatic];

    pool.cache().insert(
        IpCacheKey::new("localhost", port),
        IpCacheSlot {
            best: Some(fail_stat.clone()),
            alternatives: vec![success_stat.clone()],
        },
    );

    set_global_pool(Arc::new(Mutex::new(pool)));

    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = false;
    cfg.tls.san_whitelist = vec!["localhost".into()];
    cfg.tls.insecure_skip_verify = true;

    let sub = TestSubtransport::new(cfg);
    let res = sub.connect_tls_with_fallback("localhost", port);
    assert!(
        res.is_ok(),
        "expected success after trying alternate candidate"
    );

    let global_pool = obtain_global_pool();
    let guard = global_pool.lock().unwrap();
    let aggregate = guard
        .outcome_metrics("localhost", port)
        .expect("aggregate metrics present");
    assert_eq!(aggregate.success, 1);
    assert_eq!(aggregate.failure, 0);
    let fail_metrics = guard
        .candidate_outcome_metrics("localhost", port, fail_stat.candidate.address)
        .expect("failure candidate metrics present");
    assert!(
        fail_metrics.failure >= 1,
        "expected failure recorded for first candidate"
    );
    let success_metrics = guard
        .candidate_outcome_metrics("localhost", port, success_stat.candidate.address)
        .expect("success candidate metrics present");
    assert_eq!(success_metrics.success, 1);
    drop(guard);

    let snap = tl_snapshot();
    assert_eq!(snap.ip_strategy, Some("Cached"));
    assert_eq!(snap.ip_source.as_deref(), Some("UserStatic"));
    assert!(snap.ip_latency_ms.is_some());

    stop_fail.store(true, AtomicOrdering::Relaxed);
    let fail_addr = SocketAddr::from(([127, 0, 0, 2], port));
    let _ = TcpStream::connect_timeout(&fail_addr, Duration::from_millis(200));
    fail_handle.join().unwrap();

    stop_tls.store(true, AtomicOrdering::Relaxed);
    let tls_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let _ = TcpStream::connect_timeout(&tls_addr, Duration::from_millis(200));
    tls_handle.join().unwrap();

    reset_global_pool();
    clear_global_base_dir();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn ip_pool_disabled_bypasses_candidates() {
    let _guard = counter_guard().lock().unwrap();
    tl_metrics_reset();
    reset_global_pool();

    let temp_dir = create_empty_dir();
    override_global_base_dir(&temp_dir);

    let port = pick_free_port();
    let (stop_tls, tls_handle) = spawn_tls_server("127.0.0.1", port);

    let mut runtime = IpPoolRuntimeConfig::default();
    runtime.enabled = false;
    runtime.sources.user_static = true;
    let mut file = IpPoolFileConfig::default();
    file.user_static.push(UserStaticIp {
        host: "localhost".into(),
        ip: "127.0.0.1".into(),
        ports: vec![port],
    });
    let pool = IpPool::new(EffectiveIpPoolConfig::from_parts(runtime, file));
    set_global_pool(Arc::new(Mutex::new(pool)));

    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = false;
    cfg.tls.san_whitelist = vec!["localhost".into()];
    cfg.tls.insecure_skip_verify = true;

    let sub = TestSubtransport::new(cfg);
    let res = sub.connect_tls_with_fallback("localhost", port);
    assert!(
        res.is_ok(),
        "expected system dns path to succeed when pool disabled"
    );

    let global_pool = obtain_global_pool();
    let guard = global_pool.lock().unwrap();
    assert!(
        guard.outcome_metrics("localhost", port).is_none(),
        "disabled pool should not record aggregate outcomes"
    );
    assert!(
        guard
            .candidate_outcome_metrics("localhost", port, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),)
            .is_none(),
        "candidate metrics should not exist when pool disabled"
    );
    drop(guard);

    let snap = tl_snapshot();
    assert_eq!(snap.ip_strategy, Some("SystemDefault"));
    assert!(snap.ip_source.is_none());
    assert!(snap.ip_latency_ms.is_none());

    stop_tls.store(true, AtomicOrdering::Relaxed);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
    tls_handle.join().unwrap();

    reset_global_pool();
    clear_global_base_dir();
    let _ = std::fs::remove_dir_all(&temp_dir);
}
