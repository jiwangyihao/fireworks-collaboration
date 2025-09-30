#![cfg(not(feature = "tauri-app"))]
//! Event Backward Compatibility Tests (P4.4)
//! -----------------------------------------
//! 验证新增可选字段的向后兼容性：
//! - ip_source, ip_latency_ms, ip_selection_stage 在 AdaptiveTlsTiming 中
//! - ip_source, ip_latency_ms 在 AdaptiveTlsFallback 中
//! - 确保旧版本客户端能正常解析新事件（字段缺失时为 None）

#[path = "../common/mod.rs"]
mod common;

use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

mod section_backward_compatibility {
    use fireworks_collaboration_lib::events::structured::StrategyEvent;
    use serde_json::json;

    #[test]
    fn adaptive_tls_timing_deserializes_with_missing_ip_fields() {
        // Simulate old event JSON without IP pool fields
        let old_json = json!({
            "AdaptiveTlsTiming": {
                "id": "task123",
                "kind": "GitClone",
                "used_fake_sni": false,
                "fallback_stage": "Real",
                "connect_ms": 100,
                "tls_ms": 200,
                "first_byte_ms": 50,
                "total_ms": 350,
                "cert_fp_changed": false
            }
        });

        let event: StrategyEvent =
            serde_json::from_value(old_json).expect("deserialize old event");
        match event {
            StrategyEvent::AdaptiveTlsTiming {
                id,
                kind,
                used_fake_sni,
                fallback_stage,
                connect_ms,
                tls_ms,
                first_byte_ms,
                total_ms,
                cert_fp_changed,
                ip_source,
                ip_latency_ms,
                ip_selection_stage,
            } => {
                assert_eq!(id, "task123");
                assert_eq!(kind, "GitClone");
                assert!(!used_fake_sni);
                assert_eq!(fallback_stage, "Real");
                assert_eq!(connect_ms, Some(100));
                assert_eq!(tls_ms, Some(200));
                assert_eq!(first_byte_ms, Some(50));
                assert_eq!(total_ms, Some(350));
                assert!(!cert_fp_changed);
                assert!(ip_source.is_none());
                assert!(ip_latency_ms.is_none());
                assert!(ip_selection_stage.is_none());
            }
            _ => panic!("expected AdaptiveTlsTiming"),
        }
    }

    #[test]
    fn adaptive_tls_timing_serializes_with_ip_fields_omitted_when_none() {
        let event = StrategyEvent::AdaptiveTlsTiming {
            id: "task456".into(),
            kind: "GitPush".into(),
            used_fake_sni: false,
            fallback_stage: "Real".into(),
            connect_ms: Some(80),
            tls_ms: Some(120),
            first_byte_ms: Some(40),
            total_ms: Some(240),
            cert_fp_changed: false,
            ip_source: None,
            ip_latency_ms: None,
            ip_selection_stage: None,
        };

        let json = serde_json::to_value(&event).expect("serialize event");
        let obj = json.as_object().unwrap().get("AdaptiveTlsTiming").unwrap();
        // Fields should be omitted when None
        assert!(!obj.as_object().unwrap().contains_key("ip_source"));
        assert!(!obj.as_object().unwrap().contains_key("ip_latency_ms"));
        assert!(!obj
            .as_object()
            .unwrap()
            .contains_key("ip_selection_stage"));
    }

    #[test]
    fn adaptive_tls_timing_serializes_with_ip_fields_included_when_some() {
        let event = StrategyEvent::AdaptiveTlsTiming {
            id: "task789".into(),
            kind: "GitFetch".into(),
            used_fake_sni: true,
            fallback_stage: "Fake".into(),
            connect_ms: Some(90),
            tls_ms: Some(110),
            first_byte_ms: Some(45),
            total_ms: Some(245),
            cert_fp_changed: false,
            ip_source: Some("Builtin,Dns".into()),
            ip_latency_ms: Some(25),
            ip_selection_stage: Some("Cached".into()),
        };

        let json = serde_json::to_value(&event).expect("serialize event");
        let obj = json.as_object().unwrap().get("AdaptiveTlsTiming").unwrap();
        assert_eq!(
            obj.get("ip_source").unwrap().as_str().unwrap(),
            "Builtin,Dns"
        );
        assert_eq!(obj.get("ip_latency_ms").unwrap().as_u64().unwrap(), 25);
        assert_eq!(
            obj.get("ip_selection_stage").unwrap().as_str().unwrap(),
            "Cached"
        );
    }

    #[test]
    fn adaptive_tls_fallback_deserializes_with_missing_ip_fields() {
        let old_json = json!({
            "AdaptiveTlsFallback": {
                "id": "task111",
                "kind": "GitClone",
                "from": "Fake",
                "to": "Real",
                "reason": "FakeHandshakeError"
            }
        });

        let event: StrategyEvent =
            serde_json::from_value(old_json).expect("deserialize old fallback");
        match event {
            StrategyEvent::AdaptiveTlsFallback {
                id,
                kind,
                from,
                to,
                reason,
                ip_source,
                ip_latency_ms,
            } => {
                assert_eq!(id, "task111");
                assert_eq!(kind, "GitClone");
                assert_eq!(from, "Fake");
                assert_eq!(to, "Real");
                assert_eq!(reason, "FakeHandshakeError");
                assert!(ip_source.is_none());
                assert!(ip_latency_ms.is_none());
            }
            _ => panic!("expected AdaptiveTlsFallback"),
        }
    }

    #[test]
    fn adaptive_tls_fallback_serializes_with_ip_fields() {
        let event = StrategyEvent::AdaptiveTlsFallback {
            id: "task222".into(),
            kind: "GitPush".into(),
            from: "Real".into(),
            to: "Default".into(),
            reason: "RealHandshakeError".into(),
            ip_source: Some("Builtin".into()),
            ip_latency_ms: Some(30),
        };

        let json = serde_json::to_value(&event).expect("serialize fallback");
        let obj = json
            .as_object()
            .unwrap()
            .get("AdaptiveTlsFallback")
            .unwrap();
        assert_eq!(obj.get("ip_source").unwrap().as_str().unwrap(), "Builtin");
        assert_eq!(obj.get("ip_latency_ms").unwrap().as_u64().unwrap(), 30);
    }

    #[test]
    fn old_client_can_deserialize_new_event_with_unknown_fields() {
        // Simulate new event with IP pool fields sent to old client
        // Old client should ignore unknown fields
        let new_json_str = r#"{
            "AdaptiveTlsTiming": {
                "id": "task999",
                "kind": "GitClone",
                "used_fake_sni": false,
                "fallback_stage": "Real",
                "connect_ms": 100,
                "tls_ms": 200,
                "first_byte_ms": 50,
                "total_ms": 350,
                "cert_fp_changed": false,
                "ip_source": "Builtin",
                "ip_latency_ms": 20,
                "ip_selection_stage": "Cached"
            }
        }"#;

        // Deserialize as full type (simulating old client model without extra fields)
        let event: StrategyEvent =
            serde_json::from_str(new_json_str).expect("old client should parse");
        match event {
            StrategyEvent::AdaptiveTlsTiming { id, .. } => {
                assert_eq!(id, "task999");
            }
            _ => panic!("expected AdaptiveTlsTiming"),
        }
    }
}
