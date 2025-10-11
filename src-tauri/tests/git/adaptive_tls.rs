#![cfg(not(feature = "tauri-app"))]

use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::git::transport::metrics::{
    tl_reset as tl_metrics_reset, tl_snapshot,
};
use fireworks_collaboration_lib::core::git::transport::testing::{
    auto_disable_guard, classify_and_count_fallback, reset_auto_disable, reset_fallback_counters,
    snapshot_fallback_counters,
};
use fireworks_collaboration_lib::core::git::transport::{
    is_fake_disabled, record_fake_attempt, tl_push_fallback_event, tl_take_fallback_events,
    AutoDisableConfig, AutoDisableEvent, FallbackEventRecord,
};
use std::sync::{Mutex, OnceLock};

fn counter_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
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
    tl_metrics_reset();

    let cfg = AppConfig::default();
    let auto_cfg = AutoDisableConfig::from_http_cfg(&cfg.http);

    let mut auto_disable_seen = false;
    for _ in 0..8 {
        if let Some(evt) = record_fake_attempt(&auto_cfg, true) {
            match evt {
                AutoDisableEvent::Triggered {
                    threshold_pct,
                    cooldown_secs,
                } => {
                    tl_push_fallback_event(FallbackEventRecord::AutoDisable {
                        enabled: true,
                        threshold_pct,
                        cooldown_secs,
                    });
                    auto_disable_seen = true;
                    break;
                }
                AutoDisableEvent::Recovered => {
                    tl_push_fallback_event(FallbackEventRecord::AutoDisable {
                        enabled: false,
                        threshold_pct: 0,
                        cooldown_secs: 0,
                    });
                }
            }
        }
    }

    let events = tl_take_fallback_events();
    assert!(auto_disable_seen, "auto-disable event not observed");
    assert!(is_fake_disabled(&auto_cfg));
    assert!(events
        .iter()
        .any(|e| matches!(e, FallbackEventRecord::AutoDisable { enabled: true, .. })));

    reset_auto_disable();
    reset_fallback_counters();
    tl_metrics_reset();
}

#[test]
fn fallback_events_consumed_after_snapshot() {
    let _lock = counter_guard().lock().unwrap();
    reset_fallback_counters();
    reset_auto_disable();
    tl_metrics_reset();

    // simulate fallback events being recorded
    tl_push_fallback_event(FallbackEventRecord::Transition {
        from: "Fake",
        to: "Real",
        reason: "FakeHandshakeError".into(),
    });
    tl_push_fallback_event(FallbackEventRecord::AutoDisable {
        enabled: true,
        threshold_pct: 50,
        cooldown_secs: 30,
    });
    let first = tl_take_fallback_events();
    assert!(
        !first.is_empty(),
        "initial snapshot should contain fallback records"
    );
    let second = tl_take_fallback_events();
    assert!(
        second.is_empty(),
        "subsequent snapshot should drain previously seen records"
    );

    reset_fallback_counters();
    reset_auto_disable();
    tl_metrics_reset();
}

#[test]
fn tls_metrics_snapshot_resets_after_reset() {
    let _lock = counter_guard().lock().unwrap();
    tl_metrics_reset();
    let snap = tl_snapshot();
    assert!(snap.ip_latency_ms.is_none());
    assert!(snap.ip_strategy.is_none());
}
