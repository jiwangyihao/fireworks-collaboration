//! task_wait: 任务等待辅助工具
//! 提供统一的异步等待方法，避免在各测试文件中重复实现轮询逻辑。

use fireworks_collaboration_lib::core::tasks::model::TaskState;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;

/// 轮询等待直到任务进入终态（Completed/Failed/Canceled）。
/// 说明：
/// - 最多轮询约 120 次，每次睡眠 ~35ms（总计约 < 5 秒）。
/// - 不返回最终状态；若需状态可扩展带返回值的版本。
#[allow(dead_code)]
pub async fn wait_until_task_done(reg: &TaskRegistry, id: uuid::Uuid) {
    for _ in 0..120u32 {
        if let Some(snap) = reg.snapshot(&id) {
            if matches!(snap.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(35)).await;
    }
}
