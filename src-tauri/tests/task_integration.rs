#![cfg(not(feature = "tauri-app"))]
use std::sync::Arc;
use std::time::Instant;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use tokio::time::{sleep, Duration};

// 等待状态达到目标，超时返回 false
async fn wait_until<F: Fn() -> bool>(cond: F, max_ms: u64, step_ms: u64) -> bool {
    let start = Instant::now();
    while start.elapsed().as_millis() < max_ms as u128 {
        if cond() { return true; }
        sleep(Duration::from_millis(step_ms)).await;
    }
    false
}

#[tokio::test]
async fn test_sleep_task_complete_integration() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 150 });
    reg.clone().spawn_sleep_task(None, id, token, 150);
    let ok = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Completed)).unwrap_or(false), 2_000, 30).await;
    assert!(ok, "sleep task should complete within timeout");
}

#[tokio::test]
async fn test_sleep_task_cancel_integration() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::Sleep { ms: 1_000 });
    reg.clone().spawn_sleep_task(None, id, token.clone(), 1_000);
    // 等待进入 Running
    let running = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Running)).unwrap_or(false), 1_000, 20).await;
    assert!(running, "task should enter running state");
    token.cancel();
    let canceled = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Canceled)).unwrap_or(false), 1_000, 30).await;
    assert!(canceled, "task should transition to canceled after token.cancel()");
}

#[tokio::test]
async fn test_multi_tasks_parallel() {
    let reg = Arc::new(TaskRegistry::new());
    let mut ids = vec![];
    for _ in 0..5 { // 启动 5 个并行短任务
        let (id, token) = reg.create(TaskKind::Sleep { ms: 120 });
        reg.clone().spawn_sleep_task(None, id, token, 120);
        ids.push(id);
    }
    // 等待全部完成
    let all_done = wait_until(|| {
        ids.iter().all(|id| reg.snapshot(id).map(|s| matches!(s.state, TaskState::Completed)).unwrap_or(false))
    }, 3_000, 40).await;
    assert!(all_done, "all parallel tasks should complete");
}
