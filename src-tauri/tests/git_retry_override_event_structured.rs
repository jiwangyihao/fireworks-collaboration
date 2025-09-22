#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_global_event_bus, publish_global, Event as StructuredEvent, PolicyEvent};
use fireworks_collaboration_lib::core::tasks::retry::{RetryPlan, compute_retry_diff};

// 该测试不直接走 TaskRegistry 路径（后续 Phase 可接入），先验证 diff + 结构化事件发布机制。

#[test]
fn retry_override_diff_and_event_structured_changed() {
    let bus = MemoryEventBus::new();
    set_global_event_bus(std::sync::Arc::new(bus.clone())).ok();
    let base = RetryPlan { max: 6, base_ms: 300, factor: 1.5, jitter: true };
    let newp = RetryPlan { max: 3, base_ms: 500, factor: 2.0, jitter: false };
    let (diff, changed) = compute_retry_diff(&base, &newp);
    assert!(changed);
    publish_global(StructuredEvent::Policy(PolicyEvent::RetryApplied { id: "t1".into(), code: "retry_strategy_override_applied".into(), changed: diff.changed.iter().map(|s| s.to_string()).collect() }));
    let events = bus.take_all();
    assert_eq!(events.len(), 1);
    match &events[0] { StructuredEvent::Policy(PolicyEvent::RetryApplied { id, code, changed }) => {
        assert_eq!(id, "t1");
        assert_eq!(code, "retry_strategy_override_applied");
        assert!(changed.contains(&"max".to_string()));
        assert!(changed.contains(&"baseMs".to_string()));
        assert!(changed.contains(&"factor".to_string()));
        assert!(changed.contains(&"jitter".to_string()));
    }, _ => panic!("unexpected event variant") }
}

#[test]
fn retry_override_diff_and_event_structured_unchanged() {
    let bus = MemoryEventBus::new();
    // 不重置全局总线避免 set 失败，这里只做 diff 验证，不发事件
    let base = RetryPlan { max: 6, base_ms: 300, factor: 1.5, jitter: true };
    let newp = base.clone();
    let (diff, changed) = compute_retry_diff(&base, &newp);
    assert!(!changed);
    assert!(diff.changed.is_empty());
    assert!(bus.take_all().is_empty());
}
