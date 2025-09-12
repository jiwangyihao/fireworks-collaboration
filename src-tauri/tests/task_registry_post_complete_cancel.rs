#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn cancel_after_completion_returns_true_and_keeps_completed_state() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 80 });
    reg.clone().spawn_sleep_task(None, id, token, 80);
    // wait until completed
    for _ in 0..80 { if let Some(s)=reg.snapshot(&id){ if matches!(s.state, TaskState::Completed){ break; } } sleep(Duration::from_millis(10)).await; }
    let before = reg.snapshot(&id).expect("snapshot");
    assert!(matches!(before.state, TaskState::Completed));
    // cancel should still return true
    assert!(reg.cancel(&id));
    // state should remain Completed (not mutated to Canceled post factum)
    let after = reg.snapshot(&id).expect("snapshot");
    assert!(matches!(after.state, TaskState::Completed));
}

