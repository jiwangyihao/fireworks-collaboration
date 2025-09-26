//! task_wait: 任务等待辅助工具
//! 提供统一的异步等待方法，避免在各测试文件中重复实现轮询逻辑。

use fireworks_collaboration_lib::core::tasks::model::TaskState;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use std::time::Instant;

/// 轮询等待直到任务进入终态（Completed/Failed/Canceled）。
/// 说明：
/// - 最多轮询约 120 次，每次睡眠 ~35ms（总计约 < 5 秒）。
/// - 不返回最终状态；若需状态可扩展带返回值的版本。
#[allow(dead_code)]
pub async fn wait_until_task_done(reg: &TaskRegistry, id: uuid::Uuid) {
    for _ in 0..120u32 {
        if let Some(snap) = reg.snapshot(&id) {
            if matches!(
                snap.state,
                TaskState::Completed | TaskState::Failed | TaskState::Canceled
            ) {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(35)).await;
    }
}

/// 等待给定谓词在超时之前为 true。
#[allow(dead_code)]
pub async fn wait_predicate<F: Fn() -> bool>(pred: F, max_ms: u64, step_ms: u64) -> bool {
    let start = Instant::now();
    while start.elapsed().as_millis() < max_ms as u128 {
        if pred() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(step_ms)).await;
    }
    false
}

/// 等待特定任务达到目标状态，返回是否在超时内达到。
#[allow(dead_code)]
pub async fn wait_task_state(
    reg: &TaskRegistry,
    id: &uuid::Uuid,
    target: TaskState,
    max_ms: u64,
    step_ms: u64,
) -> bool {
    wait_predicate(
        || reg.snapshot(id).map(|s| s.state == target).unwrap_or(false),
        max_ms,
        step_ms,
    )
    .await
}
