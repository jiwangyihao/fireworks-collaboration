#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_test_event_bus, clear_test_event_bus, Event as StructuredEvent, StrategyEvent};
use fireworks_collaboration_lib::core::tasks::registry::test_emit_clone_with_override;
use std::sync::Arc;

fn install_bus() -> MemoryEventBus {
    let bus = MemoryEventBus::new();
    set_test_event_bus(Arc::new(bus.clone()));
    bus
}

#[test]
fn strategy_override_http_tls_summary_structured_events() {
    let bus = install_bus();
    let task_id = uuid::Uuid::new_v4();
    let override_json = serde_json::json!({
        "http": {"follow_redirects": false, "max_redirects": 5},
        "tls": {"insecure_skip_verify": true, "skip_san_whitelist": true},
        "retry": {"max": 3, "baseMs": 500, "factor": 2.0, "jitter": false}
    });
    test_emit_clone_with_override("https://example.com/repo.git", task_id, override_json);
    let evs = bus.snapshot();
    // 期望至少包含：HttpApplied, TlsApplied(含冲突?), RetryApplied(Policy), Summary
    let mut has_http=false; let mut has_tls=false; let mut _has_conflict=false; let mut has_summary=false; let mut has_retry=false;
    for e in &evs {
        match e {
            StructuredEvent::Strategy(StrategyEvent::HttpApplied { id, .. }) => { if id==&task_id.to_string() { has_http=true; } },
            StructuredEvent::Strategy(StrategyEvent::TlsApplied { id, .. }) => { if id==&task_id.to_string() { has_tls=true; } },
            StructuredEvent::Strategy(StrategyEvent::Conflict { id, kind, .. }) => { if id==&task_id.to_string() && (kind=="tls" || kind=="http") { _has_conflict=true; } },
            StructuredEvent::Strategy(StrategyEvent::Summary { id, kind, .. }) => { if id==&task_id.to_string() && kind=="GitClone" { has_summary=true; } },
            StructuredEvent::Policy(fireworks_collaboration_lib::events::structured::PolicyEvent::RetryApplied { id, .. }) => { if id==&task_id.to_string() { has_retry=true; } },
            _ => {}
        }
    }
    assert!(has_http, "missing HttpApplied (check field aliases)");
    assert!(has_tls, "missing TlsApplied (check field aliases)");
    assert!(has_summary, "missing Summary event");
    assert!(has_retry, "missing RetryApplied policy event");
    clear_test_event_bus();
}

#[test]
fn strategy_override_no_change_generates_summary_only() {
    let bus = install_bus();
    let task_id = uuid::Uuid::new_v4();
    // override 与默认值一致（假设默认 follow=true, insecure=false, skip=false, retry 默认）
    let override_json = serde_json::json!({
        "http": {"follow_redirects": true},
        "tls": {"insecure_skip_verify": false, "skip_san_whitelist": false},
        "retry": {}
    });
    test_emit_clone_with_override("https://example.com/repo.git", task_id, override_json);
    let evs = bus.take_all();
    let mut summaries=0;
    for e in &evs { if let StructuredEvent::Strategy(StrategyEvent::Summary { .. }) = e { summaries+=1; } }
    assert!(summaries>=1, "expected at least one summary event");
    clear_test_event_bus();
}
