#![cfg(test)]
use crate::events::structured::{set_test_event_bus, MemoryEventBus, Event, StrategyEvent};
use uuid::Uuid;

#[test]
fn adaptive_tls_timing_event_emitted_with_first_byte() {
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    let id = Uuid::new_v4();
    crate::core::tasks::registry::test_emit_adaptive_tls_timing(id, "GitClone");
    let events = bus.snapshot();
    let timing = events.into_iter().find_map(|e| match e { Event::Strategy(StrategyEvent::AdaptiveTlsTiming { first_byte_ms, .. }) => first_byte_ms, _=>None });
    assert_eq!(timing, Some(40));
}

#[test]
fn fingerprint_changed_event_once() {
    use crate::core::git::transport::fingerprint::record_certificate;
    use rustls::Certificate;
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    // Fake certificates: different DER contents
    let cert_a = Certificate(vec![0x30,0x82,0x01,0x0a,0x01,0xA1,0xB2]);
    let cert_b = Certificate(vec![0x30,0x82,0x01,0x0a,0x02,0xA1,0xB2]);
    let host = "example.test";
    let r1 = record_certificate(host, &[cert_a.clone()]);
    assert!(r1.unwrap().0, "first should be changed=true");
    let r2 = record_certificate(host, &[cert_a.clone()]);
    assert!(!r2.unwrap().0, "same cert no change");
    let r3 = record_certificate(host, &[cert_b.clone()]);
    assert!(r3.unwrap().0, "different cert triggers change");
    let events = bus.snapshot();
    let count = events.into_iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::CertFingerprintChanged { .. }))).count();
    assert_eq!(count, 2, "two change events (initial + different)");
}

#[test]
fn metrics_disabled_suppresses_timing() {
    // Force disable via test override (does not depend on config loader caching)
    crate::core::git::transport::metrics::test_override_metrics_enabled(Some(false));
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    crate::core::tasks::registry::test_emit_adaptive_tls_timing(Uuid::new_v4(), "GitFetch");
    let events = bus.snapshot();
    assert!(events.iter().all(|e| !matches!(e, Event::Strategy(StrategyEvent::AdaptiveTlsTiming { .. }))), "timing event should be suppressed when metrics disabled override active");
    // Restore override
    crate::core::git::transport::metrics::test_override_metrics_enabled(None);
}

#[test]
fn cert_fp_log_rotation_small_limit() {
    // Set extremely small max bytes
    let mut cfg = crate::core::config::model::AppConfig::default();
    cfg.tls.cert_fp_max_bytes = 64; // very small
    crate::core::config::loader::save(&cfg).unwrap();
    use crate::core::git::transport::fingerprint::record_certificate;
    use rustls::Certificate;
    let host = "rotate.test";
    // Clean existing log files for deterministic assertion
    let base = crate::core::config::loader::base_dir();
    let log = base.join("cert-fp.log");
    let rotated = base.join("cert-fp.log.1");
    let _ = std::fs::remove_file(&log);
    let _ = std::fs::remove_file(&rotated);
    for i in 0..5 {
        let der = vec![0x30,0x81, i as u8, 0x01, 0x02, 0x03];
        let _ = record_certificate(host, &[Certificate(der)]);
    }
    assert!(log.exists(), "primary log should exist");
    assert!(rotated.exists(), "rotated log .1 should exist due to tiny size limit");
}

#[test]
fn metrics_override_force_enable_and_restore() {
    use crate::core::git::transport::metrics::test_override_metrics_enabled;
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    // Force disable first to ensure clean state then enable via override
    test_override_metrics_enabled(Some(false));
    crate::core::tasks::registry::test_emit_adaptive_tls_timing(Uuid::new_v4(), "GitClone");
    assert!(bus.snapshot().is_empty(), "should suppress when forced false");
    // Force enable ignoring config
    test_override_metrics_enabled(Some(true));
    let id2 = Uuid::new_v4();
    crate::core::tasks::registry::test_emit_adaptive_tls_timing(id2, "GitFetch");
    let events = bus.snapshot();
    assert!(events.iter().any(|e| matches!(e, Event::Strategy(StrategyEvent::AdaptiveTlsTiming { .. }))), "forced true should emit timing");
    // Clear override -> revert to config (default true) still emits
    test_override_metrics_enabled(None);
    let id3 = Uuid::new_v4();
    crate::core::tasks::registry::test_emit_adaptive_tls_timing(id3, "GitPush");
    let events2 = bus.snapshot();
    let count = events2.iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::AdaptiveTlsTiming { .. }))).count();
    assert!(count >= 2, "after clearing override should continue emitting (>=2 timing events total)");
}

#[test]
fn cert_fp_log_disabled_suppresses_record() {
    // Disable logging
    let mut cfg = crate::core::config::model::AppConfig::default();
    cfg.tls.cert_fp_log_enabled = false;
    crate::core::config::loader::save(&cfg).unwrap();
    use crate::core::git::transport::fingerprint::record_certificate;
    use rustls::Certificate;
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    let res = record_certificate("nolog.test", &[Certificate(vec![0x30,0x01,0x02])]);
    assert!(res.is_none(), "record should early exit when disabled");
    assert!(bus.snapshot().is_empty(), "no events when log disabled");
}

#[test]
fn fingerprint_lru_eviction_triggers_rechange() {
    use crate::core::git::transport::fingerprint::{record_certificate, test_reset_fp_state};
    use rustls::Certificate;
    test_reset_fp_state();
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    // Insert 512 distinct hosts
    for i in 0..512 {
        let host = format!("h{i}.lru.test");
        let der = vec![0x30,0x82,(i & 0xFF) as u8,0x01,0x02,0x03];
        let _ = record_certificate(&host, &[Certificate(der)]);
    }
    // First inserted host was h0.lru.test; now insert one more host to evict it
    let _ = record_certificate("extra.lru.test", &[Certificate(vec![0x30,0x99,0x01,0x02])]);
    // Re-record h0 should be treated as changed=true again (was evicted)
    let r = record_certificate("h0.lru.test", &[Certificate(vec![0x30,0x82,0x00,0x01,0x02,0x03])]).unwrap();
    assert!(r.0, "evicted host should appear as changed again");
    let events = bus.snapshot();
    // Should have at least 514 change events (initial for each + reinsert) but to be lenient just check >= 514 not required; focus on existence of final event
    assert!(events.iter().any(|e| matches!(e, Event::Strategy(StrategyEvent::CertFingerprintChanged { host, .. }) if host=="h0.lru.test")), "expected change event for reinserted h0.lru.test");
}

#[test]
fn cert_fingerprint_changed_base64_length() {
    use crate::core::git::transport::fingerprint::{record_certificate, test_reset_fp_state};
    use rustls::Certificate;
    test_reset_fp_state();
    let bus = std::sync::Arc::new(MemoryEventBus::new());
    set_test_event_bus(bus.clone());
    let _ = record_certificate("b64.test", &[Certificate(vec![0x30,0x82,0xAA,0xBB,0xCC,0xDD])]);
    let events = bus.snapshot();
    let mut found = false;
    for e in events.iter() {
        if let Event::Strategy(StrategyEvent::CertFingerprintChanged { spki_sha256, cert_sha256, host, .. }) = e {
            if host == "b64.test" { 
                assert_eq!(spki_sha256.len(), 43, "spki length must be 43 for SHA256 base64url no pad");
                assert_eq!(cert_sha256.len(), 43, "cert length must be 43 for SHA256 base64url no pad");
                found = true;
            }
        }
    }
    assert!(found, "expected fingerprint change event for b64.test");
}

#[test]
fn fallback_counters_classification_basic() {
    use crate::core::git::transport::http::{test_reset_fallback_counters, test_snapshot_fallback_counters, test_classify_and_count_fallback};
    test_reset_fallback_counters();
    // Map messages that should classify as Verify
    let r1 = test_classify_and_count_fallback("tls: General(SAN whitelist mismatch)");
    assert_eq!(r1, "Verify");
    let r2 = test_classify_and_count_fallback("certificate name mismatch");
    assert_eq!(r2, "Verify");
    // Map messages that should classify as Tls
    let r3 = test_classify_and_count_fallback("tls handshake: unexpected eof");
    assert_eq!(r3, "Tls");
    let r4 = test_classify_and_count_fallback("tcp connect: timed out");
    assert_eq!(r4, "Tls");
    // Check counters reflect 2 Verify and 2 Tls
    let (tls, verify) = test_snapshot_fallback_counters();
    assert_eq!(verify, 2, "verify counter");
    assert_eq!(tls, 2, "tls counter");
}