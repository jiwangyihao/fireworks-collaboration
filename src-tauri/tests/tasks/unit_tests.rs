//! Tasks 模块单元测试
//!
//! 从 registry.rs 迁移的异步任务测试

use fireworks_collaboration_lib::core::tasks::{
    model::{TaskKind, TaskState},
    registry::TaskRegistry,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

async fn wait_for_state(reg: &TaskRegistry, id: Uuid, target: TaskState, max_ms: u64) -> bool {
    let mut waited = 0u64;
    while waited < max_ms {
        if let Some(s) = reg.snapshot(&id) {
            if s.state == target {
                return true;
            }
        }
        sleep(Duration::from_millis(20)).await;
        waited += 20;
    }
    false
}

#[tokio::test]
async fn test_create_initial_pending() {
    let reg = TaskRegistry::new();
    let (id, _t) = reg.create(TaskKind::Sleep { ms: 100 });
    let snap = reg.snapshot(&id).expect("snapshot");
    assert!(matches!(snap.state, TaskState::Pending));
}

#[tokio::test]
async fn test_sleep_task_completes() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 120 });
    let handle = reg.spawn_sleep_task(None, id, token, 120);
    let ok = wait_for_state(&reg, id, TaskState::Completed, 1500).await;
    assert!(ok, "task should complete");
    handle.await.unwrap();
}

#[tokio::test]
async fn test_cancel_sleep_task() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 500 });
    let handle = reg.spawn_sleep_task(None, id, token.clone(), 500);
    let entered = wait_for_state(&reg, id, TaskState::Running, 500).await;
    assert!(entered, "should enter running");
    token.cancel();
    let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await;
    assert!(canceled, "should cancel");
    handle.await.unwrap();
}
