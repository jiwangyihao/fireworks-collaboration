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
    /// depth: 可选浅克隆深度（P2.2b）。None 表示全量克隆。
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        depth: Option<u32>,
        should_interrupt: &std::sync::atomic::AtomicBool,
        on_progress: F,
    ) -> Result<(), crate::core::git::errors::GitError>;

    /// Fetch 已有仓库（可选传入远程名或 URL），阻塞调用
    /// Fetch 已有仓库（可选传入远程名或 URL），阻塞调用
    /// depth: 可选浅拉取深度（P2.2c）。None 表示全量 fetch。
    fn fetch_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo_url: &str,
        dest: &Path,
        depth: Option<u32>,
        should_interrupt: &std::sync::atomic::AtomicBool,
        on_progress: F,
    ) -> Result<(), crate::core::git::errors::GitError>;

    /// Push 到远程（HTTPS 基础）。
    ///
    /// 输入：
    /// - dest: 本地仓库路径
    /// - remote: 远程名（如 "origin"），为空则默认 "origin"
    /// - refspecs: 需要推送的 refspec 列表（如 ["refs/heads/main:refs/heads/main"]），为空则使用默认推送配置
    /// - creds: 可选用户名与密码/令牌（若仅提供 token，可将 username 置为 Some("x-access-token") 以兼容 GitHub）
    fn push_blocking<F: FnMut(ProgressPayload)>(
        &self,
        dest: &Path,
        remote: Option<&str>,
        refspecs: Option<&[&str]>,
        creds: Option<(&str, &str)>,
        should_interrupt: &std::sync::atomic::AtomicBool,
        on_progress: F,
    ) -> Result<(), crate::core::git::errors::GitError>;
}
