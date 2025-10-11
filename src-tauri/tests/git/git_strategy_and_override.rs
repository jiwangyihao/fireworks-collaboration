#![cfg(not(feature = "tauri-app"))]
//! Git strategy override tests focused on HTTP and retry overrides after TLS removal.

use super::common::{
    event_assert::{expect_optional_tags_subsequence, expect_subsequence},
    git_scenarios::GitOp,
    http_override_stub::{
        http_override_cases, run_http_override, FollowMode, HttpOverrideCase, IdempotentFlag,
        MaxEventsCase,
    },
    test_env::init_test_env,
};

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

mod section_http_basic {
    use super::*;

    #[test]
    fn http_override_applies_for_all_cases() {
        for case in http_override_cases() {
            let out = run_http_override(&case);
            assert!(out.applied, "override not applied for {case}");
            assert!(!out.events.is_empty());
        }
    }
}

mod section_http_limits {
    use super::*;

    #[test]
    fn http_follow_chain_respects_maximum() {
        for case in http_override_cases() {
            let output = run_http_override(&case);
            if matches!(case.follow, FollowMode::Follow) {
                match case.max_events {
                    MaxEventsCase::Some(limit) => {
                        assert!(
                            output.follow_chain.len() as u32 <= limit.max(1),
                            "follow chain exceeds max for {case}"
                        );
                    }
                    MaxEventsCase::None => {
                        assert_eq!(
                            output.follow_chain.len(),
                            2,
                            "default follow hops should match stub expectation for {case}"
                        );
                    }
                }
            } else {
                assert!(
                    output.follow_chain.is_empty(),
                    "follow chain should be empty when follow == none"
                );
            }
        }
    }
}

mod section_http_invalid_max {
    use super::*;

    #[test]
    fn http_invalid_max_zero_is_rejected() {
        let case = HttpOverrideCase {
            op: GitOp::Clone,
            follow: FollowMode::None,
            idempotent: IdempotentFlag::No,
            max_events: MaxEventsCase::Some(0),
        };
        let output = run_http_override(&case);
        assert!(
            !output.applied,
            "invalid max should not be applied for {case}"
        );
        expect_subsequence(&output.events, &["http:override:invalid_max"]);
        expect_optional_tags_subsequence(&output.events, &["http"]);
    }
}

mod section_http_events {
    use super::*;

    #[test]
    fn http_event_sequence_contains_expected_tags() {
        let case = http_override_cases()
            .into_iter()
            .find(|c| matches!(c.follow, FollowMode::Follow))
            .expect("follow case");
        let output = run_http_override(&case);
        expect_subsequence(
            &output.events,
            &["http:override:start", "http:override", "http:override:applied"],
        );
        expect_optional_tags_subsequence(&output.events, &["http", "http"]);
    }
}

mod section_strategy_summary {
    use crate::common::strategy_support::test_emit_clone_with_override;
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_test_event_bus, Event as StructuredEvent, MemoryEventBus,
        PolicyEvent, StrategyEvent,
    };
    use serde_json::json;
    use std::sync::Arc;
    use uuid::Uuid;

    fn install_test_bus() -> MemoryEventBus {
        let bus = MemoryEventBus::new();
        set_test_event_bus(Arc::new(bus.clone()));
        bus
    }

    #[test]
    fn strategy_override_http_retry_summary_structured_events() {
        let bus = install_test_bus();
        let task_id = Uuid::new_v4();
        let override_json = json!({
            "http": {"follow_redirects": false, "max_redirects": 5},
            "retry": {"max": 3, "baseMs": 500, "factor": 2.0, "jitter": false}
        });

        test_emit_clone_with_override("https://example.com/repo.git", task_id, override_json);

        let mut has_http = false;
        let mut has_summary = false;
        let mut has_retry = false;
        let mut summary_codes: Vec<String> = Vec::new();
        let task_id_str = task_id.to_string();

        for event in bus.snapshot() {
            match event {
                StructuredEvent::Strategy(StrategyEvent::HttpApplied { ref id, .. })
                    if id == &task_id_str =>
                {
                    has_http = true;
                }
                StructuredEvent::Strategy(StrategyEvent::Summary {
                    ref id,
                    applied_codes,
                    kind,
                    ..
                }) if id == &task_id_str && kind == "GitClone" => {
                    has_summary = true;
                    summary_codes = applied_codes.clone();
                }
                StructuredEvent::Policy(PolicyEvent::RetryApplied { ref id, .. })
                    if id == &task_id_str =>
                {
                    has_retry = true;
                }
                _ => {}
            }
        }

        assert!(has_http, "missing http applied event");
        assert!(has_summary, "missing strategy summary");
        assert!(has_retry, "missing retry applied event");
        assert!(
            summary_codes.contains(&"http_strategy_override_applied".to_string()),
            "summary missing http applied code"
        );
        assert!(
            summary_codes.contains(&"retry_strategy_override_applied".to_string()),
            "summary missing retry applied code"
        );

        clear_test_event_bus();
    }

    #[test]
    fn strategy_override_no_change_generates_summary_only() {
        let bus = install_test_bus();
        let task_id = Uuid::new_v4();
        let override_json = json!({
            "http": {"follow_redirects": true},
            "retry": {}
        });

        test_emit_clone_with_override("https://example.com/repo.git", task_id, override_json);

        let mut has_http = false;
        let mut summary_codes: Option<Vec<String>> = None;

        for event in bus.take_all() {
            match event {
                StructuredEvent::Strategy(StrategyEvent::HttpApplied { .. }) => {
                    has_http = true;
                }
                StructuredEvent::Strategy(StrategyEvent::Summary {
                    applied_codes, ..
                }) => {
                    summary_codes = Some(applied_codes.clone());
                }
                _ => {}
            }
        }

        assert!(!has_http, "http applied event should not fire when nothing changes");
        let codes = summary_codes.expect("expected summary event");
        assert!(
            codes.is_empty(),
            "summary should not contain applied codes when nothing changed: {codes:?}"
        );

        clear_test_event_bus();
    }
}
