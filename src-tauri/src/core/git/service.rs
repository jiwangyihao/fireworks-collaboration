use std::path::Path;
use uuid::Uuid;

/// 统一进度负载：与前端事件字段保持兼容
#[derive(Debug, Clone)]
pub struct ProgressPayload {
    pub task_id: Uuid,
    pub kind: String,      // GitClone | GitFetch | GitPush
    pub phase: String,     // Negotiating | Receiving | Checkout | ...
    pub percent: u32,
    pub objects: Option<u64>,
    pub bytes: Option<u64>,
    pub total_hint: Option<u64>,
}

/// Git 服务统一抽象（后续便于从 gix 迁移到 git2）
pub trait GitService {
    /// Clone 仓库到目标目录（阻塞调用）
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        should_interrupt: &std::sync::atomic::AtomicBool,
        on_progress: F,
    ) -> Result<(), crate::core::git::errors::GitError>;

    /// Fetch 已有仓库（可选传入远程名或 URL），阻塞调用
    fn fetch_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo_url: &str,
        dest: &Path,
        should_interrupt: &std::sync::atomic::AtomicBool,
        on_progress: F,
    ) -> Result<(), crate::core::git::errors::GitError>;
}
