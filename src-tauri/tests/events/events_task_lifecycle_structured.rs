//! Task Lifecycle Structured Event Tests
//!
//! Covers strict state transitions and event contract for task lifecycle:
//! - Created -> Started -> Completed
//! - Created -> Started -> Canceled
//! - Created -> Started -> Failed
//! - Progress Event emission

use crate::common::event_assert::expect_subsequence;
use crate::common::test_env::init_test_env;
use fireworks_collaboration_lib::events::structured::{Event, EventBus, MemoryEventBus, TaskEvent};

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

mod section_lifecycle_transitions {
    use super::*;

    #[test]
    fn lifecycle_success_flow() {
        let bus = MemoryEventBus::new();
        let task_id = "task-success-1";

        // Simulate Task Runner lifecycle
        bus.publish(Event::Task(TaskEvent::Created {
            id: task_id.into(),
            kind: "GitClone".into(),
        }));
        bus.publish(Event::Task(TaskEvent::Started {
            id: task_id.into(),
            kind: "GitClone".into(),
        }));
        bus.publish(Event::Task(TaskEvent::Progress {
            id: task_id.into(),
            message: "Cloning...".into(),
            increment: 0.5,
        }));
        bus.publish(Event::Task(TaskEvent::Completed { id: task_id.into() }));

        let events = bus.snapshot();
        let labels: Vec<String> = events
            .iter()
            .map(|e| {
                match e {
                    Event::Task(TaskEvent::Created { .. }) => "Created",
                    Event::Task(TaskEvent::Started { .. }) => "Started",
                    Event::Task(TaskEvent::Progress { .. }) => "Progress",
                    Event::Task(TaskEvent::Completed { .. }) => "Completed",
                    _ => "Other",
                }
                .to_string()
            })
            .collect();

        expect_subsequence(&labels, &["Created", "Started", "Progress", "Completed"]);
    }

    #[test]
    fn lifecycle_cancellation_flow() {
        let bus = MemoryEventBus::new();
        let task_id = "task-cancel-1";

        bus.publish(Event::Task(TaskEvent::Created {
            id: task_id.into(),
            kind: "GitFetch".into(),
        }));
        bus.publish(Event::Task(TaskEvent::Started {
            id: task_id.into(),
            kind: "GitFetch".into(),
        }));
        bus.publish(Event::Task(TaskEvent::Canceled { id: task_id.into() }));

        let events = bus.snapshot();
        let labels: Vec<String> = events
            .iter()
            .map(|e| {
                match e {
                    Event::Task(TaskEvent::Created { .. }) => "Created",
                    Event::Task(TaskEvent::Started { .. }) => "Started",
                    Event::Task(TaskEvent::Canceled { .. }) => "Canceled",
                    _ => "Other",
                }
                .to_string()
            })
            .collect();

        expect_subsequence(&labels, &["Created", "Started", "Canceled"]);
    }

    #[test]
    fn lifecycle_failure_flow() {
        let bus = MemoryEventBus::new();
        let task_id = "task-fail-1";

        bus.publish(Event::Task(TaskEvent::Created {
            id: task_id.into(),
            kind: "GitPush".into(),
        }));
        bus.publish(Event::Task(TaskEvent::Started {
            id: task_id.into(),
            kind: "GitPush".into(),
        }));
        bus.publish(Event::Task(TaskEvent::Failed {
            id: task_id.into(),
            category: "Network".into(),
            code: Some("connect_timeout".into()),
            message: "Connection timed out".into(),
        }));

        let events = bus.snapshot();

        // Verify failure details
        let failure = events
            .iter()
            .find_map(|e| match e {
                Event::Task(TaskEvent::Failed {
                    category,
                    code,
                    message,
                    ..
                }) => Some((category, code, message)),
                _ => None,
            })
            .expect("should have failed event");

        assert_eq!(failure.0, "Network");
        assert_eq!(failure.1.as_deref(), Some("connect_timeout"));
        assert_eq!(failure.2, "Connection timed out");

        let labels: Vec<String> = events
            .iter()
            .map(|e| {
                match e {
                    Event::Task(TaskEvent::Created { .. }) => "Created",
                    Event::Task(TaskEvent::Started { .. }) => "Started",
                    Event::Task(TaskEvent::Failed { .. }) => "Failed",
                    _ => "Other",
                }
                .to_string()
            })
            .collect();

        expect_subsequence(&labels, &["Created", "Started", "Failed"]);
    }
}
