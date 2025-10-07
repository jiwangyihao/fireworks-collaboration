//! `repo_factory`: 高层测试仓库构造 / 结构描述 / 分支 & 历史工具。
//! 改进版：
//!  * 统一内部初始化与提交逻辑（去重）
//!  * 提供 `RepoBuilder` 构建多分支 + 线性/追加提交
//!  * 提供 `RepoDescriptor` 便于测试输出/快照 (branches, commits)
//!  * 保持向后兼容：原有 `repo_with_branches` / `repo_with_linear_commits` API 未删除
//! 未来扩展：标签创建、复杂拓扑（分叉/合并）、基于对象计数的 shallow 验证支撑。

use fireworks_collaboration_lib::core::git::default_impl::{
    add::git_add, branch::git_branch, commit::git_commit, init::git_init,
};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use uuid::Uuid;

// ---- 内部通用 Helper ----
fn init_empty_repo(path: &Path) {
    let cancel = AtomicBool::new(false);
    git_init(path, &cancel, |_p: ProgressPayload| {}).expect("init repo");
    if let Ok(repo) = git2::Repository::open(path) {
        if let Ok(mut cfg) = repo.config() {
            let _ = cfg.set_bool("core.autocrlf", false);
            let _ = cfg.set_bool("core.safecrlf", false);
            let _ = cfg.set_str("core.eol", "lf");
            // CI 环境下常缺失 user.name/user.email，导致后续 git_commit 失败：
            // error: signature: config value 'user.name' was not found
            // 仅在未显式配置时注入测试默认值，避免影响本地已有个性化设置。
            let name_missing = cfg.get_entry("user.name").is_err();
            if name_missing {
                let _ = cfg.set_str("user.name", "Test User");
            }
            let email_missing = cfg.get_entry("user.email").is_err();
            if email_missing {
                let _ = cfg.set_str("user.email", "test@example.com");
            }
        }
    }
}

fn commit_file(repo: &Path, file: &str, content: &str, msg: &str) {
    let cancel = AtomicBool::new(false);
    std::fs::write(repo.join(file), content).expect("write file");
    git_add(repo, &[file], &cancel, |_p| {}).expect("git add");
    git_commit(repo, msg, None, false, &cancel, |_p| {}).expect("git commit");
}

fn create_temp(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fwc-{prefix}-{}", Uuid::new_v4()))
}

/// 仓库结构描述（最小摘要，可用于断言或调试输出）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDescriptor {
    pub path: PathBuf,
    pub branches: Vec<String>,
    pub commit_count: usize,
}
// describe() 方法已移除（未被引用，保持结构最小化）。

/// 测试仓库构建器：链式构造多分支 + 提交。
#[derive(Debug, Default)]
pub struct RepoBuilder {
    base_commits: Vec<(String, String, String)>, // (file, content, msg)
    branches: Vec<String>,
    additional_commits: Vec<(String, String, String)>,
}
impl RepoBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    /// 初始线性提交（依次创建）。
    pub fn with_base_commit<S: Into<String>>(mut self, file: S, content: S, msg: S) -> Self {
        let (file, content, msg) = (file.into(), content.into(), msg.into());
        self.base_commits.push((file, content, msg));
        self
    }
    /// 在构建后追加的额外线性提交（构造完成后按顺序追加）。
    pub fn with_commit<S: Into<String>>(mut self, file: S, content: S, msg: S) -> Self {
        let (file, content, msg) = (file.into(), content.into(), msg.into());
        self.additional_commits.push((file, content, msg));
        self
    }
    /// 添加需要创建的分支（指向最终 HEAD，不自动 checkout）。
    pub fn with_branch<S: Into<String>>(mut self, name: S) -> Self {
        self.branches.push(name.into());
        self
    }
    pub fn build(self) -> RepoDescriptor {
        let path = create_temp("repo-bld");
        init_empty_repo(&path);
        for (file, content, msg) in &self.base_commits {
            commit_file(&path, file, content, msg);
        }
        for (file, content, msg) in &self.additional_commits {
            commit_file(&path, file, content, msg);
        }
        let cancel = AtomicBool::new(false);
        for b in &self.branches {
            git_branch(&path, b, false, false, &cancel, |_p| {}).expect("git branch");
        }
        let branches = list_local_branches(&path);
        let commit_count = rev_count(&path) as usize;
        RepoDescriptor {
            path,
            branches,
            commit_count,
        }
    }
}

/// 读取当前仓库 HEAD 可达提交数量（利用 git log rev-list）。
pub fn rev_count(path: &Path) -> u32 {
    let repo = git2::Repository::open(path).expect("open repo for rev_count");
    let mut revwalk = repo.revwalk().expect("revwalk");
    revwalk.push_head().expect("push head");
    let mut c = 0u32;
    for _ in revwalk {
        c += 1;
    }
    c
}

fn list_local_branches(path: &Path) -> Vec<String> {
    let repo = git2::Repository::open(path).expect("open repo");
    let mut out = Vec::new();
    let branches = repo
        .branches(Some(git2::BranchType::Local))
        .expect("branches");
    for b in branches.flatten() {
        if let Some(name) = b.0.name().ok().flatten() {
            out.push(name.to_string());
        }
    }
    out.sort();
    out
}

// (branches_from_slice removed; builder now takes owned Strings directly)

// ---- HEAD / 分支状态工具：保持原有函数，仅微调实现 (无需改) ----

#[cfg(test)]
mod tests_repo_factory {
    use super::*;

    #[test]
    fn builder_basic_branches_and_commits() {
        let desc = RepoBuilder::new()
            .with_base_commit("a.txt", "A", "feat: a")
            .with_commit("b.txt", "B", "feat: b")
            .with_branch("dev")
            .with_branch("release")
            .build();
        assert!(desc.commit_count >= 2, "expect >=2 commits");
        assert!(desc.branches.iter().any(|b| b == "dev"));
        assert!(desc.branches.iter().any(|b| b == "release"));
    }
}
