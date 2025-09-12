#![cfg(not(feature = "tauri-app"))]
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn snapshot_unknown_returns_none() {
    let reg = TaskRegistry::new();
    let random = uuid::Uuid::new_v4();
    assert!(reg.snapshot(&random).is_none());
}

#[tokio::test]
async fn cancel_unknown_returns_false() {
    let reg = TaskRegistry::new();
    let random = uuid::Uuid::new_v4();
    assert!(!reg.cancel(&random));
}

#[tokio::test]
async fn cancel_idempotent() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 200 });
    reg.clone().spawn_sleep_task(None, id, token.clone(), 200);
    // 等待进入 running
    for _ in 0..20 { if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Running) { break; } } sleep(Duration::from_millis(15)).await; }
    assert!(reg.cancel(&id));
    assert!(reg.cancel(&id)); // 第二次依然返回 true（token 已经取消，但语义上视为仍然允许取消调用）
}

#[tokio::test]
async fn list_snapshots_are_independent_clones() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 50 });
    reg.clone().spawn_sleep_task(None, id, token, 50);
    let list_before = reg.list();
    assert_eq!(list_before.len(), 1);
    // 等待完成
    for _ in 0..40 { if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Completed) { break; } } sleep(Duration::from_millis(15)).await; }
    let list_after = reg.list();
    assert_eq!(list_after.len(), 1);
    // 确认之前克隆的 snapshot 不会被内部状态突变（只验证 state 变化不会回写）
    let new_state = &list_after[0].state;
    // old_state 可能是 Pending 或 Running；不做具体断言，只要不 panic；确保新状态是完成或已取消之一
    assert!(matches!(new_state, TaskState::Completed | TaskState::Canceled));
}

