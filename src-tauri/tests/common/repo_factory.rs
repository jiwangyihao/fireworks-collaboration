//! repo_factory: 提供更高层次的测试仓库构造与 HEAD/分支状态工具。
//! 当前仅实现 12.3 阶段所需的最小集合；后续阶段（clone/fetch/push）可扩展。

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use uuid::Uuid;
use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, commit::git_commit, add::git_add, branch::git_branch};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

/// 创建带若干分支的仓库：
/// - 初始化 main 分支，写入并提交一个 base 文件。
/// - 对 `branches` 中的每个名字创建分支（指向当前 HEAD，不自动 checkout）。
#[allow(dead_code)]
pub fn repo_with_branches(branches: &[&str]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("fwc-branches-{}", Uuid::new_v4()));
    let cancel = AtomicBool::new(false);
    git_init(&path, &cancel, |_p: ProgressPayload| {}).expect("init repo");
    // base commit
    std::fs::write(path.join("base.txt"), "base").unwrap();
    git_add(&path, &["base.txt"], &cancel, |_p| {}).unwrap();
    git_commit(&path, "chore: base", None, false, &cancel, |_p| {}).unwrap();
    for b in branches { git_branch(&path, b, false, false, &cancel, |_p| {}).unwrap(); }
    path
}

/// 读取当前 HEAD 所指向的本地分支名；若为分离 HEAD 则返回 None。
#[allow(dead_code)]
pub fn current_branch(repo_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_path).ok()?;
    let head = repo.head().ok()?;
    if head.is_branch() { head.shorthand().map(|s| s.to_string()) } else { None }
}

/// 判断 HEAD 是否处于分离状态。
#[allow(dead_code)]
pub fn is_head_detached(repo_path: &Path) -> bool {
    git2::Repository::open(repo_path).map(|r| r.head().map(|h| !h.is_branch()).unwrap_or(false)).unwrap_or(false)
}

/// 快速创建：指定 (commit_message, file_name, file_content) 序列并线性提交；返回路径。
/// 可用于后续需要多提交历史的 checkout / reset 等场景。
#[allow(dead_code)]
pub fn repo_with_linear_commits(specs: &[(&str, &str, &str)]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("fwc-linear-{}", Uuid::new_v4()));
    let cancel = AtomicBool::new(false);
    git_init(&path, &cancel, |_p: ProgressPayload| {}).expect("init repo");
    for (msg, file, content) in specs {
        std::fs::write(path.join(file), content).unwrap();
        git_add(&path, &[*file], &cancel, |_p| {}).unwrap();
        git_commit(&path, msg, None, false, &cancel, |_p| {}).unwrap();
    }
    path
}
