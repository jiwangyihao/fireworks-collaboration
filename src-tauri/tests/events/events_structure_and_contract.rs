#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Events Structure & Contract (Roadmap 12.12)
//! ----------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - `events_structured_basic.rs`
//!   - `events_contract_snapshot.rs`
//!   - `events_no_legacy_taskerror.rs`
//!   - `events_task_lifecycle_structured.rs` (仅结构契约/非生命周期特定片段，生命周期用例 12.13 单独聚合)
//! 分区结构：
//!   `section_schema_basic`        -> 基础结构化事件发布 / snapshot / `take_all`
//!   `unified_basic_and_sequence`  -> 基础结构化事件发布 + 最小序列锚点（Started -> `RetryApplied` -> Completed）
//!   `section_legacy_absence`      -> 验证不再出现 legacy `TaskEvent::Failed` code（策略/partial 旧错误码）
//!   `section_contract_snapshot`   -> JSON snapshot（精简抽样，避免冗长）
//!   `section_adaptive_tls_metrics`-> 自适应 TLS 事件与指标观测
//!   `section_tls_fingerprint_log` -> 证书指纹日志与结构化事件
//!   `section_tls_pin_enforcement` -> Pin 校验事件与降级路径
//! 设计说明：
//!   * 保留最小代表性事件集合，替代原多文件重复验证。
//!   * snapshot 采用行拼接字符串（与原 `tests/events_contract_snapshot.rs` 一致模式），但裁剪为核心样本，后续 schema 变更时需明确更新 expected。
//!   * 不覆盖生命周期进度/取消分支（推迟到 12.13）。
//! Cross-ref:
//!   - 12.9 / 12.10 中策略与 retry 事件锚点
//!   - 12.11 中取消/超时 outcome 计划将复用结构化事件枚举
//! Post-audit(v1): 初版聚合采用静态 JSON 对比；后续可考虑引入 insta snapshot 或基于字段子集的宽松匹配以降低微字段变更噪音。

use crate::common::event_assert::expect_subsequence;
use crate::common::test_env::init_test_env;
use fireworks_collaboration_lib::events::structured::{
    Event, EventBus, MemoryEventBus, PolicyEvent, StrategyEvent, TaskEvent, TransportEvent,
};

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// ---------------- unified_basic_and_sequence ----------------
mod section_unified_basic_and_sequence {
    use super::*;
    #[test]
    fn unified_basic_sequence_and_drain() {
        let bus = MemoryEventBus::new();
        // 发布基础事件：Started -> RetryApplied -> Completed
        bus.publish(Event::Task(TaskEvent::Started {
            id: "case1".into(),
            kind: "GitClone".into(),
        }));
        bus.publish(Event::Policy(PolicyEvent::RetryApplied {
            id: "case1".into(),
            code: "retry_strategy_override_applied".into(),
            changed: vec!["max".into()],
        }));
        bus.publish(Event::Task(TaskEvent::Completed { id: "case1".into() }));
        let snap = bus.snapshot();
        assert_eq!(snap.len(), 3, "snapshot length mismatch");
        assert!(
            matches!(snap[0], Event::Task(TaskEvent::Started { .. })),
            "first event should be Task::Started"
        );
        // 序列锚点（粗粒度标签）
        let labels: Vec<String> = snap
            .iter()
            .map(|e| {
                match e {
                    Event::Task(TaskEvent::Started { .. }) => "Task:Started",
                    Event::Task(TaskEvent::Completed { .. }) => "Task:Completed",
                    Event::Policy(PolicyEvent::RetryApplied { .. }) => "Policy:RetryApplied",
                    _ => "Other",
                }
                .to_string()
            })
            .collect();
        expect_subsequence(
            &labels,
            &["Task:Started", "Policy:RetryApplied", "Task:Completed"],
        );
        // take_all drains & 幂等校验
        assert_eq!(bus.take_all().len(), 3, "take_all should drain all events");
        assert!(bus.take_all().is_empty(), "second take_all must be empty");
    }
}

// ---------------- section_legacy_absence ----------------
mod section_legacy_absence {
    use super::*;
    #[test]
    fn no_legacy_failed_codes_present() {
        // 仅验证：不再出现旧时代通过 TaskEvent::Failed.code 暴露的策略/partial 错误码。
        let bus = MemoryEventBus::new();
        bus.publish(Event::Strategy(StrategyEvent::Summary {
            id: "L1".into(),
            kind: "GitClone".into(),
            http_follow: true,
            http_max: 3,
            retry_max: 2,
            retry_base_ms: 200,
            retry_factor: 1.2,
            retry_jitter: true,
            applied_codes: vec!["http_strategy_override_applied".into()],
            filter_requested: true,
        }));
        bus.publish(Event::Transport(TransportEvent::PartialFilterFallback {
            id: "L1".into(),
            shallow: false,
            message: "partial_filter_fallback".into(),
        }));
        bus.publish(Event::Strategy(StrategyEvent::AdaptiveTlsRollout {
            id: "L2".into(),
            kind: "GitClone".into(),
            percent_applied: 10,
            sampled: true,
        }));
        let events = bus.snapshot();
        // legacy code 曾以 TaskEvent::Failed 的 code 形式出现，本组不应出现 Task::Failed 里的旧策略 code
        for e in &events {
            if let Event::Task(TaskEvent::Failed { code: Some(c), .. }) = e {
                panic!("unexpected legacy style failed code present: {c}");
            }
        }
    }
}

// ---------------- section_contract_snapshot ----------------
mod section_contract_snapshot {
    use super::*;
    use crate::common::event_assert::expect_optional_tags_subsequence;
    use crate::common::event_assert::structured_ext::{
        assert_unique_event_ids, map_structured_events_to_type_tags, serialize_events_to_json_lines,
    };
    #[test]
    fn contract_core_snapshot() {
        #[allow(dead_code)]
        const SCHEMA_VERSION: u32 = 1; // schema 变更需显式 bump
        let samples = vec![
            Event::Task(TaskEvent::Started {
                id: "id1".into(),
                kind: "GitClone".into(),
            }),
            Event::Task(TaskEvent::Failed {
                id: "id1".into(),
                category: "Protocol".into(),
                code: Some("x".into()),
                message: "m".into(),
            }),
            Event::Policy(PolicyEvent::RetryApplied {
                id: "id2".into(),
                code: "retry_strategy_override_applied".into(),
                changed: vec!["max".into()],
            }),
            Event::Transport(TransportEvent::PartialFilterUnsupported {
                id: "id3".into(),
                requested: "blob:none".into(),
            }),
            Event::Strategy(StrategyEvent::HttpApplied {
                id: "id4".into(),
                follow: true,
                max_redirects: 5,
            }),
            Event::Strategy(StrategyEvent::AdaptiveTlsRollout {
                id: "id5".into(),
                kind: "GitClone".into(),
                percent_applied: 42,
                sampled: true,
            }),
        ];
        let lines = serialize_events_to_json_lines(&samples);
        let joined = lines.join("\n");
        let expected = r#"{"type":"Task","data":{"Started":{"id":"id1","kind":"GitClone"}}}
{"type":"Task","data":{"Failed":{"id":"id1","category":"Protocol","code":"x","message":"m"}}}
{"type":"Policy","data":{"RetryApplied":{"id":"id2","code":"retry_strategy_override_applied","changed":["max"]}}}
{"type":"Transport","data":{"PartialFilterUnsupported":{"id":"id3","requested":"blob:none"}}}
{"type":"Strategy","data":{"HttpApplied":{"id":"id4","follow":true,"max_redirects":5}}}
{"type":"Strategy","data":{"AdaptiveTlsRollout":{"id":"id5","kind":"GitClone","percent_applied":42,"sampled":true}}}"#;
        assert_eq!(
            joined, expected,
            "structured event core contract changed; update expected if intentional"
        );
        let lines_vec: Vec<String> = expected
            .lines()
            .map(|s| s.trim_start().to_string())
            .collect();
        // 结构化样例序列化为行后也可映射出顶层类型标签
        expect_optional_tags_subsequence(&lines_vec, &["Task", "Policy", "Transport", "Strategy"]);
        assert_unique_event_ids(&lines_vec);
        let _mapped = map_structured_events_to_type_tags(&samples);
        // SCHEMA_VERSION >= 1 是常量，已经在编译时检查
    }
}

// ---------------- section_adaptive_tls_metrics ----------------
mod section_adaptive_tls_metrics {
    use fireworks_collaboration_lib::events::structured::{
        publish_global, set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use uuid::Uuid;

    #[test]
    fn adaptive_tls_timing_event_emitted_with_first_byte() {
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        publish_global(Event::Strategy(StrategyEvent::AdaptiveTlsTiming {
            id: Uuid::new_v4().to_string(),
            kind: "GitClone".into(),
            used_fake_sni: true,
            fallback_stage: "Fake".into(),
            connect_ms: Some(10),
            tls_ms: Some(30),
            first_byte_ms: Some(40),
            total_ms: Some(50),
            cert_fp_changed: false,
            ip_source: None,
            ip_latency_ms: None,
            ip_selection_stage: None,
        }));
        let events = bus.snapshot();
        let timing = events.into_iter().find_map(|e| match e {
            Event::Strategy(StrategyEvent::AdaptiveTlsTiming { first_byte_ms, .. }) => {
                first_byte_ms
            }
            _ => None,
        });
        assert_eq!(timing, Some(40));
    }
}

// ---------------- section_adaptive_tls_fallback ----------------
mod section_adaptive_tls_fallback {
    use crate::common::strategy_support::test_emit_adaptive_tls_observability;
    use fireworks_collaboration_lib::core::git::transport::{
        tl_push_fallback_event, tl_take_fallback_events, FallbackEventRecord,
    };
    use fireworks_collaboration_lib::events::structured::{
        set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn metrics_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct MetricsEnvGuard {
        prev: Option<String>,
    }

    impl Drop for MetricsEnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.prev.take() {
                std::env::set_var("FWC_TEST_FORCE_METRICS", prev);
            } else {
                std::env::remove_var("FWC_TEST_FORCE_METRICS");
            }
        }
    }

    fn set_metrics_env(force: Option<bool>) -> MetricsEnvGuard {
        let prev = std::env::var("FWC_TEST_FORCE_METRICS").ok();
        match force {
            Some(true) => std::env::set_var("FWC_TEST_FORCE_METRICS", "1"),
            Some(false) => std::env::set_var("FWC_TEST_FORCE_METRICS", "0"),
            None => std::env::remove_var("FWC_TEST_FORCE_METRICS"),
        }
        MetricsEnvGuard { prev }
    }

    #[test]
    fn fallback_and_auto_disable_events_emitted() {
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        // Ensure clean state
        tl_take_fallback_events();
        tl_push_fallback_event(FallbackEventRecord::Transition {
            from: "Fake",
            to: "Real",
            reason: "FakeHandshakeError".into(),
        });
        tl_push_fallback_event(FallbackEventRecord::AutoDisable {
            enabled: true,
            threshold_pct: 25,
            cooldown_secs: 45,
        });
        test_emit_adaptive_tls_observability(Uuid::nil(), "GitClone");
        let events = bus.take_all();
        let has_transition = events.iter().any(|evt| match evt {
            Event::Strategy(StrategyEvent::AdaptiveTlsFallback {
                from, to, reason, ..
            }) => from == "Fake" && to == "Real" && reason == "FakeHandshakeError",
            _ => false,
        });
        assert!(has_transition, "adaptive fallback transition event missing");
        let has_auto_disable = events.iter().any(|evt| match evt {
            Event::Strategy(StrategyEvent::AdaptiveTlsAutoDisable {
                enabled,
                threshold_pct,
                cooldown_secs,
                ..
            }) => *enabled && *threshold_pct == 25 && *cooldown_secs == 45,
            _ => false,
        });
        assert!(has_auto_disable, "adaptive TLS auto-disable event missing");
    }

    #[test]
    fn fallback_events_emit_when_metrics_disabled() {
        let _lock = metrics_env_lock().lock().unwrap();
        let _env_guard = set_metrics_env(Some(false));

        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        tl_take_fallback_events();
        tl_push_fallback_event(FallbackEventRecord::Transition {
            from: "Fake",
            to: "Real",
            reason: "ForcedDisable".into(),
        });
        tl_push_fallback_event(FallbackEventRecord::AutoDisable {
            enabled: true,
            threshold_pct: 30,
            cooldown_secs: 120,
        });
        test_emit_adaptive_tls_observability(Uuid::nil(), "GitClone");
        let events = bus.take_all();
        let has_transition = events.iter().any(|evt| match evt {
            Event::Strategy(StrategyEvent::AdaptiveTlsFallback { reason, .. }) => {
                reason == "ForcedDisable"
            }
            _ => false,
        });
        assert!(
            has_transition,
            "fallback event missing when metrics disabled"
        );
        let has_auto_disable = events.iter().any(|evt| match evt {
            Event::Strategy(StrategyEvent::AdaptiveTlsAutoDisable {
                enabled,
                threshold_pct,
                cooldown_secs,
                ..
            }) => *enabled && *threshold_pct == 30 && *cooldown_secs == 120,
            _ => false,
        });
        assert!(
            has_auto_disable,
            "auto-disable event missing when metrics disabled"
        );
        let has_timing = events.iter().any(|evt| {
            matches!(
                evt,
                Event::Strategy(StrategyEvent::AdaptiveTlsTiming { .. })
            )
        });
        assert!(
            !has_timing,
            "timing event should not emit when metrics forcibly disabled"
        );
    }
}

// ---------------- section_tls_fingerprint_log ----------------
mod section_tls_fingerprint_log {
    use fireworks_collaboration_lib::core::config::{loader, model::AppConfig};
    use fireworks_collaboration_lib::core::git::transport::record_certificate;
    use fireworks_collaboration_lib::events::structured::{
        set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use rcgen::generate_simple_self_signed;
    use rustls::Certificate;
    use serde_json::Value;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    fn enable_cert_fp_logging_with_limit(max_bytes: usize) {
        let mut cfg = AppConfig::default();
        cfg.tls.cert_fp_log_enabled = true;
        cfg.tls.cert_fp_max_bytes = max_bytes as u64;
        loader::save(&cfg).expect("save cfg");
    }

    fn fp_log_guard() -> &'static Mutex<()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    fn read_log_entries(path: &Path) -> Vec<Value> {
        if !path.exists() {
            return Vec::new();
        }
        let raw = std::fs::read_to_string(path).expect("read log");
        raw.lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .collect()
    }

    #[test]
    fn fingerprint_changed_event_once() {
        let _lock = fp_log_guard().lock().unwrap();
        enable_cert_fp_logging_with_limit(1024);
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let cert_a = Certificate(vec![0x30, 0x82, 0x01, 0x0a, 0x01, 0xA1, 0xB2]);
        let cert_b = Certificate(vec![0x30, 0x82, 0x01, 0x0a, 0x02, 0xA1, 0xB2]);
        let host = "example.test";
        let r1 = record_certificate(host, &[cert_a.clone()]);
        assert!(
            r1.is_some() && r1.unwrap().0,
            "first should be changed=true"
        );
        let r2 = record_certificate(host, &[cert_a.clone()]);
        assert!(r2.is_some() && !r2.unwrap().0, "same cert no change");
        let r3 = record_certificate(host, &[cert_b.clone()]);
        assert!(
            r3.is_some() && r3.unwrap().0,
            "different cert triggers change"
        );
        let events = bus.snapshot();
        let count = events
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    Event::Strategy(StrategyEvent::CertFingerprintChanged { .. })
                )
            })
            .count();
        assert_eq!(count, 2, "two change events (initial + different)");
    }

    #[test]
    fn cert_fingerprint_changed_base64_length() {
        let _lock = fp_log_guard().lock().unwrap();
        enable_cert_fp_logging_with_limit(1024);
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let host = format!("b64.{}.test", uuid::Uuid::new_v4());
        let _ = record_certificate(
            &host,
            &[Certificate(vec![0x30, 0x82, 0xAA, 0xBB, 0xCC, 0xDD])],
        );
        let events = bus.snapshot();
        let mut found = false;
        for e in events.iter() {
            if let Event::Strategy(StrategyEvent::CertFingerprintChanged {
                spki_sha256,
                cert_sha256,
                host: h,
                ..
            }) = e
            {
                if h == &host {
                    assert_eq!(
                        spki_sha256.len(),
                        43,
                        "spki length must be 43 for SHA256 base64url no pad"
                    );
                    assert_eq!(
                        cert_sha256.len(),
                        43,
                        "cert length must be 43 for SHA256 base64url no pad"
                    );
                    found = true;
                }
            }
        }
        assert!(found, "expected fingerprint change event for b64.test");
    }

    #[test]
    fn fingerprint_lru_eviction_triggers_rechange() {
        let _lock = fp_log_guard().lock().unwrap();
        enable_cert_fp_logging_with_limit(1024);
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let prefix = format!("pfx-{}", uuid::Uuid::new_v4());
        let max = 512usize;
        for i in 0..(max * 2 + 1) {
            let host = format!("{prefix}-{i}.lru.test");
            let der = vec![0x30, 0x82, (i & 0xFF) as u8, 0x01, 0x02, 0x03];
            let _ = record_certificate(&host, &[Certificate(der)]);
        }
        let first = format!("{prefix}-0.lru.test");
        let r = record_certificate(
            &first,
            &[Certificate(vec![0x30, 0x82, 0x00, 0x01, 0x02, 0x03])],
        );
        assert!(
            r.is_some() && r.unwrap().0,
            "evicted host should appear as changed again"
        );
        let events = bus.snapshot();
        assert!(events.iter().any(|e| matches!(e, Event::Strategy(StrategyEvent::CertFingerprintChanged { host, .. }) if *host == first)),
            "expected change event for reinserted first host");
    }

    #[test]
    fn cert_fp_log_rotation_small_limit() {
        let _lock = fp_log_guard().lock().unwrap();
        enable_cert_fp_logging_with_limit(1);
        let host = "rotate.test";
        let base = loader::base_dir();
        let log = base.join("cert-fp.log");
        let rotated = base.join("cert-fp.log.1");
        let _ = std::fs::remove_file(&log);
        let _ = std::fs::remove_file(&rotated);
        for i in 0..10 {
            let der = vec![0x30, 0x81, i as u8, 0x01, 0x02, 0x03];
            let _ = record_certificate(host, &[Certificate(der)]);
        }
        assert!(log.exists(), "primary log should exist");
        assert!(
            rotated.exists(),
            "rotated log .1 should exist due to tiny size limit"
        );
    }

    #[test]
    fn fingerprint_logs_include_spki_source_exact_and_fallback() {
        let _lock = fp_log_guard().lock().unwrap();
        enable_cert_fp_logging_with_limit(1024);
        let base = loader::base_dir();
        let log = base.join("cert-fp.log");
        let rotated = base.join("cert-fp.log.1");
        let _ = std::fs::remove_file(&log);
        let _ = std::fs::remove_file(&rotated);

        let host_exact = format!("exact-{}.test", uuid::Uuid::new_v4());
        let cert = generate_simple_self_signed(vec![host_exact.clone()]).unwrap();
        let der = cert.serialize_der().unwrap();
        let _ = record_certificate(&host_exact, &[Certificate(der)]);

        let entries = read_log_entries(&log);
        let exact_entry = entries
            .iter()
            .find(|v| {
                v.get("host")
                    .and_then(|h| h.as_str())
                    .map(|s| s == host_exact)
                    .unwrap_or(false)
            })
            .expect("exact entry");
        assert_eq!(
            exact_entry.get("spkiSource").and_then(|v| v.as_str()),
            Some("exact")
        );

        let host_fallback = format!("fallback-{}.test", uuid::Uuid::new_v4());
        let _ = record_certificate(&host_fallback, &[Certificate(Vec::new())]);
        let entries = read_log_entries(&log);
        let fallback_entry = entries
            .iter()
            .find(|v| {
                v.get("host")
                    .and_then(|h| h.as_str())
                    .map(|s| s == host_fallback)
                    .unwrap_or(false)
            })
            .expect("fallback entry");
        assert_eq!(
            fallback_entry.get("spkiSource").and_then(|v| v.as_str()),
            Some("fallback")
        );
    }

    #[test]
    fn cert_fp_log_disabled_suppresses_record() {
        let _lock = fp_log_guard().lock().unwrap();
        let mut cfg = AppConfig::default();
        cfg.tls.cert_fp_log_enabled = false;
        loader::save(&cfg).unwrap();
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let res = record_certificate("nolog.test", &[Certificate(vec![0x30, 0x01, 0x02])]);
        assert!(res.is_none(), "record should early exit when disabled");
        assert!(bus.snapshot().is_empty(), "no events when log disabled");
    }
}

// ---------------- section_tls_pin_enforcement ----------------
mod section_tls_pin_enforcement {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use fireworks_collaboration_lib::core::tls::spki::compute_spki_sha256_b64;
    use fireworks_collaboration_lib::core::tls::verifier::RealHostCertVerifier;
    use fireworks_collaboration_lib::events::structured::{
        set_test_event_bus, Event, MemoryEventBus, StrategyEvent,
    };
    use rcgen::generate_simple_self_signed;
    use rustls::client::{ServerCertVerified, ServerCertVerifier};
    use rustls::{Certificate, ServerName};
    use std::sync::Arc;

    struct AlwaysOkVerifier;

    impl ServerCertVerifier for AlwaysOkVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &Certificate,
            _intermediates: &[Certificate],
            _server_name: &ServerName,
            _scts: &mut dyn Iterator<Item = &[u8]>,
            _ocsp_response: &[u8],
            _now: std::time::SystemTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
    }

    #[test]
    fn pin_mismatch_emits_event_and_counts_verify() {
        let bus = Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let host = format!("pin-mismatch-{}.test", uuid::Uuid::new_v4());
        let cert = generate_simple_self_signed(vec![host.clone()]).unwrap();
        let der = cert.serialize_der().unwrap();
        let leaf = Certificate(der.clone());
        let (spki_sha256, _) = compute_spki_sha256_b64(&leaf);

        let pins = vec![URL_SAFE_NO_PAD.encode([0u8; 32])];
        let verifier = RealHostCertVerifier::new(
            Arc::new(AlwaysOkVerifier),
            Some(host.clone()),
            true,
            pins,
        );

        let mut scts = std::iter::empty::<&[u8]>();
        let err = verifier
            .verify_server_cert(
                &leaf,
                &[],
                &ServerName::try_from("fake.sni.example").unwrap(),
                &mut scts,
                &[],
                std::time::SystemTime::now(),
            )
            .expect_err("pin mismatch should fail");
        assert!(format!("{err}").to_ascii_lowercase().contains("pin"));

        let events = bus.snapshot();
        let mismatch_events: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                Event::Strategy(StrategyEvent::CertFpPinMismatch {
                    host,
                    spki_sha256: s,
                    pin_count,
                    ..
                }) => Some((host.clone(), s.clone(), *pin_count)),
                _ => None,
            })
            .collect();
        assert_eq!(mismatch_events.len(), 1, "expect one pin mismatch event");
        let (event_host, event_spki, pin_count) = &mismatch_events[0];
        assert_eq!(event_host, &host);
        assert_eq!(event_spki, &spki_sha256);
        assert_eq!(*pin_count, 1);
    }

    #[test]
    fn pin_match_allows_connection_without_mismatch_event() {
        let bus = Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let host = format!("pin-match-{}.test", uuid::Uuid::new_v4());
        let cert = generate_simple_self_signed(vec![host.clone()]).unwrap();
        let der = cert.serialize_der().unwrap();
        let leaf = Certificate(der.clone());
        let (pin, _) = compute_spki_sha256_b64(&leaf);

        let verifier = RealHostCertVerifier::new(
            Arc::new(AlwaysOkVerifier),
            Some(host.clone()),
            true,
            vec![pin],
        );

        let mut scts = std::iter::empty::<&[u8]>();
        let result = verifier.verify_server_cert(
            &leaf,
            &[],
            &ServerName::try_from("fake.sni.example").unwrap(),
            &mut scts,
            &[],
            std::time::SystemTime::now(),
        );
        assert!(result.is_ok(), "pin match should succeed");

        let events = bus.snapshot();
        assert!(
            events
                .iter()
                .all(|e| !matches!(e, Event::Strategy(StrategyEvent::CertFpPinMismatch { .. }))),
            "pin match should not emit mismatch events"
        );
    }

    #[test]
    fn invalid_pins_disable_enforcement() {
        let bus = Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let host = format!("pin-invalid-{}.test", uuid::Uuid::new_v4());
        let cert = generate_simple_self_signed(vec![host.clone()]).unwrap();
        let der = cert.serialize_der().unwrap();
        let leaf = Certificate(der);

        let verifier = RealHostCertVerifier::new(
            Arc::new(AlwaysOkVerifier),
            Some(host.clone()),
            true,
            vec!["not-base64!!".into()],
        );

        let mut scts = std::iter::empty::<&[u8]>();
        let result = verifier.verify_server_cert(
            &leaf,
            &[],
            &ServerName::try_from("fake.sni.example").unwrap(),
            &mut scts,
            &[],
            std::time::SystemTime::now(),
        );
        assert!(result.is_ok(), "invalid pins should disable enforcement");
        assert!(bus.snapshot().is_empty(), "no events when pins invalid");
    }
}
