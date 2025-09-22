#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, Event, TaskEvent, PolicyEvent, EventBus};

#[test]
fn structured_event_bus_basic_flow() {
    let bus = MemoryEventBus::new();
    // publish two events
    bus.publish(Event::Task(TaskEvent::Started { id: "abc".into(), kind: "GitClone".into() }));
    bus.publish(Event::Policy(PolicyEvent::RetryApplied { id: "abc".into(), code: "retry_strategy_override_applied".to_string(), changed: vec!["max".to_string(), "factor".to_string()] }));

    let snap = bus.snapshot();
    assert_eq!(snap.len(), 2, "snapshot should contain 2 events");
    // verify ordering & content
    match &snap[0] { Event::Task(TaskEvent::Started { id, kind }) => { assert_eq!(id, "abc"); assert_eq!(kind, "GitClone"); }, _ => panic!("unexpected first event") }
    match &snap[1] { Event::Policy(PolicyEvent::RetryApplied { id, code, changed }) => { assert_eq!(id, "abc"); assert_eq!(code, "retry_strategy_override_applied"); assert!(changed.contains(&"max".to_string())); }, _ => panic!("unexpected second event") }

    // take_all should empty
    let taken = bus.take_all();
    assert_eq!(taken.len(), 2);
    assert!(bus.take_all().is_empty());
}
