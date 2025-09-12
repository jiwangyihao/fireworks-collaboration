#![cfg(not(feature = "tauri-app"))]
use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use tokio::time::{sleep, Duration};

async fn wait_state<F: Fn() -> bool>(cond: F, total_ms: u64) -> bool {
    let mut left = total_ms;
    while left > 0 {
        if cond() { return true; }
        sleep(Duration::from_millis(15)).await;
        left = left.saturating_sub(15);
    }
    false
}

#[tokio::test]
async fn test_list_contains_created_tasks() {
    let reg = Arc::new(TaskRegistry::new());
    let mut ids = vec![];
    for i in 0..3 { let (id, token) = reg.create(TaskKind::Sleep { ms: 50 + i * 10 }); reg.clone().spawn_sleep_task(None, id, token, 50 + i * 10); ids.push(id);}
    assert_eq!(reg.list().len(), 3, "list should contain all tasks");
}

#[tokio::test]
async fn test_immediate_cancel_before_completion() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 400 });
    reg.clone().spawn_sleep_task(None, id, token.clone(), 400);
    // 等待进入运行状态
    let running = wait_state(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Running)).unwrap_or(false), 500).await;
    assert!(running);
    token.cancel();
    let canceled = wait_state(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Canceled)).unwrap_or(false), 1000).await;
    assert!(canceled, "task should cancel");
}

#[tokio::test]
async fn test_high_parallel_short_tasks() {
    let reg = Arc::new(TaskRegistry::new());
    let mut ids = vec![];
    for _ in 0..20 { let (id, token) = reg.create(TaskKind::Sleep { ms: 90 }); reg.clone().spawn_sleep_task(None, id, token, 90); ids.push(id); }
    let all_completed = wait_state(|| ids.iter().all(|id| reg.snapshot(id).map(|s| matches!(s.state, TaskState::Completed)).unwrap_or(false)), 3000).await;
    assert!(all_completed, "all short tasks should complete in parallel");
}

#[tokio::test]
async fn test_partial_cancel_mixture() {
    let reg = Arc::new(TaskRegistry::new());
    let mut cancel_tokens = vec![]; let mut ids = vec![];
    for i in 0..10 { let (id, token) = reg.create(TaskKind::Sleep { ms: 300 }); reg.clone().spawn_sleep_task(None, id, token.clone(), 300); if i % 2 == 0 { cancel_tokens.push(token.clone()); } ids.push(id); }
    // 部分取消
    for t in cancel_tokens { t.cancel(); }
    let done = wait_state(|| ids.iter().all(|id| reg.snapshot(id).map(|s| matches!(s.state, TaskState::Completed | TaskState::Canceled)).unwrap_or(false)), 2000).await;
    assert!(done, "all tasks should end in completed or canceled");
}

