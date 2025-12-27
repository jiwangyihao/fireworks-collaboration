//! Events system unit tests
//!
//! Tests for MemoryEventBus, FanoutEventBus, and Event types.

use std::sync::Arc;

use fireworks_collaboration_lib::events::structured::{
    Event, EventBus, FanoutEventBus, MemoryEventBus, TaskEvent,
};

// ============ MemoryEventBus Tests ============

#[test]
fn test_memory_event_bus_new() {
    let bus = MemoryEventBus::new();
    let events = bus.take_all();
    assert!(events.is_empty());
}

#[test]
fn test_memory_event_bus_publish_single() {
    let bus = MemoryEventBus::new();
    let event = Event::Task(TaskEvent::Started {
        id: "task-1".to_string(),
        kind: "clone".to_string(),
    });
    bus.publish(event.clone());

    let events = bus.take_all();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], event);
}

#[test]
fn test_memory_event_bus_publish_multiple() {
    let bus = MemoryEventBus::new();
    bus.publish(Event::Task(TaskEvent::Started {
        id: "task-1".to_string(),
        kind: "clone".to_string(),
    }));
    bus.publish(Event::Task(TaskEvent::Started {
        id: "task-2".to_string(),
        kind: "fetch".to_string(),
    }));
    bus.publish(Event::Task(TaskEvent::Started {
        id: "task-3".to_string(),
        kind: "push".to_string(),
    }));

    let events = bus.take_all();
    assert_eq!(events.len(), 3);
}

#[test]
fn test_memory_event_bus_take_all_clears() {
    let bus = MemoryEventBus::new();
    bus.publish(Event::Task(TaskEvent::Started {
        id: "task-1".to_string(),
        kind: "clone".to_string(),
    }));

    let first = bus.take_all();
    assert_eq!(first.len(), 1);

    let second = bus.take_all();
    assert!(second.is_empty());
}

#[test]
fn test_memory_event_bus_snapshot_preserves() {
    let bus = MemoryEventBus::new();
    bus.publish(Event::Task(TaskEvent::Started {
        id: "task-1".to_string(),
        kind: "clone".to_string(),
    }));

    let snapshot1 = bus.snapshot();
    assert_eq!(snapshot1.len(), 1);

    let snapshot2 = bus.snapshot();
    assert_eq!(snapshot2.len(), 1);

    // take_all should still have the event
    let events = bus.take_all();
    assert_eq!(events.len(), 1);
}

// ============ FanoutEventBus Tests ============

#[test]
fn test_fanout_event_bus_new() {
    let _fanout = FanoutEventBus::new();
    // FanoutEventBus created successfully
}

#[test]
fn test_fanout_event_bus_register_memory() {
    let fanout = FanoutEventBus::new();
    let memory = Arc::new(MemoryEventBus::new());
    fanout.register(memory);
    // No error means registration succeeded
}

#[test]
fn test_fanout_event_bus_publish_to_memory() {
    let fanout = FanoutEventBus::new();
    let memory = Arc::new(MemoryEventBus::new());
    fanout.register(memory.clone());

    fanout.publish(Event::Task(TaskEvent::Started {
        id: "test".to_string(),
        kind: "clone".to_string(),
    }));

    // Check memory bus received the event
    let events = memory.take_all();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_fanout_event_bus_publish_to_multiple() {
    let fanout = FanoutEventBus::new();
    let memory1 = Arc::new(MemoryEventBus::new());
    let memory2 = Arc::new(MemoryEventBus::new());

    fanout.register(memory1.clone());
    fanout.register(memory2.clone());

    fanout.publish(Event::Task(TaskEvent::Started {
        id: "shared".to_string(),
        kind: "clone".to_string(),
    }));

    // Both should receive the event
    assert_eq!(memory1.take_all().len(), 1);
    assert_eq!(memory2.take_all().len(), 1);
}

// ============ Event/TaskEvent Tests ============

#[test]
fn test_task_event_started() {
    let event = TaskEvent::Started {
        id: "task-123".to_string(),
        kind: "clone".to_string(),
    };
    if let TaskEvent::Started { id, kind } = event {
        assert_eq!(id, "task-123");
        assert_eq!(kind, "clone");
    } else {
        panic!("Expected TaskEvent::Started");
    }
}

#[test]
fn test_task_event_completed() {
    let event = TaskEvent::Completed {
        id: "task-456".to_string(),
    };
    if let TaskEvent::Completed { id } = event {
        assert_eq!(id, "task-456");
    } else {
        panic!("Expected TaskEvent::Completed");
    }
}

#[test]
fn test_task_event_canceled() {
    let event = TaskEvent::Canceled {
        id: "task-canceled".to_string(),
    };
    if let TaskEvent::Canceled { id } = event {
        assert_eq!(id, "task-canceled");
    } else {
        panic!("Expected TaskEvent::Canceled");
    }
}

#[test]
fn test_task_event_failed() {
    let event = TaskEvent::Failed {
        id: "task-789".to_string(),
        category: "network".to_string(),
        code: Some("ECONNREFUSED".to_string()),
        message: "Connection refused".to_string(),
    };
    if let TaskEvent::Failed {
        id,
        category,
        code,
        message,
    } = event
    {
        assert_eq!(id, "task-789");
        assert_eq!(category, "network");
        assert_eq!(code, Some("ECONNREFUSED".to_string()));
        assert_eq!(message, "Connection refused");
    } else {
        panic!("Expected TaskEvent::Failed");
    }
}

#[test]
fn test_event_wraps_task_event() {
    let task_event = TaskEvent::Completed {
        id: "wrapped".to_string(),
    };
    let event = Event::Task(task_event.clone());

    if let Event::Task(inner) = event {
        assert_eq!(inner, task_event);
    } else {
        panic!("Expected Event::Task");
    }
}

#[test]
fn test_event_equality() {
    let event1 = Event::Task(TaskEvent::Completed {
        id: "same".to_string(),
    });
    let event2 = Event::Task(TaskEvent::Completed {
        id: "same".to_string(),
    });
    let event3 = Event::Task(TaskEvent::Completed {
        id: "different".to_string(),
    });

    assert_eq!(event1, event2);
    assert_ne!(event1, event3);
}
