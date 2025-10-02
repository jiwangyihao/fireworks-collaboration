// 从 src/events/structured.rs 迁移的测试
use fireworks_collaboration_lib::events::structured::{
    Event, EventBus, MemoryEventBus, PolicyEvent, TaskEvent,
};

#[test]
fn memory_event_bus_basic() {
    let bus = MemoryEventBus::new();
    bus.publish(Event::Task(TaskEvent::Started {
        id: "1".into(),
        kind: "GitClone".into(),
    }));
    bus.publish(Event::Policy(PolicyEvent::RetryApplied {
        id: "1".into(),
        code: "retry_strategy_override_applied".to_string(),
        changed: vec!["max".to_string()],
    }));
    let snapshot = bus.snapshot();
    assert_eq!(snapshot.len(), 2);
    // take_all should clear
    let taken = bus.take_all();
    assert_eq!(taken.len(), 2);
    assert!(bus.take_all().is_empty());
}
