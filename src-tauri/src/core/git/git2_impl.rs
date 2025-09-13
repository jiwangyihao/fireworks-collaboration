use std::path::Path;

use super::{errors::{GitError, ErrorCategory}, service::{GitService, ProgressPayload}};

/// git2-rs 实现骨架（MP0.1）：仅提供可编译的占位实现，不执行真实操作。
pub struct Git2Service;

impl Git2Service {
    pub fn new() -> Self { Self }
}

impl GitService for Git2Service {
    fn clone_blocking<F: FnMut(ProgressPayload)>(
        &self,
        repo: &str,
        dest: &Path,
        should_interrupt: &std::sync::atomic::AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 基础阶段事件（骨架）
        let _ = (repo, dest, should_interrupt);
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitClone".into(),
            phase: "Init".into(),
            percent: 0,
            objects: None,
            bytes: None,
            total_hint: None,
        });
        // MP0.1 阶段仅返回 Ok，后续 MP0.2 接入 git2::build::RepoBuilder 等
        Ok(())
    }

    fn fetch_blocking<F: FnMut(ProgressPayload)>(
        &self,
        _repo_url: &str,
        dest: &Path,
        _should_interrupt: &std::sync::atomic::AtomicBool,
        mut on_progress: F,
    ) -> Result<(), GitError> {
        // 简单目录存在性检查；MP0.1 仅做最小校验
        if !dest.join(".git").exists() {
            return Err(GitError::new(ErrorCategory::Internal, "dest is not a git repository (missing .git)"));
        }
        on_progress(ProgressPayload {
            task_id: uuid::Uuid::nil(),
            kind: "GitFetch".into(),
            phase: "Init".into(),
            percent: 0,
            objects: None,
            bytes: None,
            total_hint: None,
        });
        Ok(())
    }
}
