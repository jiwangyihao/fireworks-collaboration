#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicBool;

use fireworks_collaboration_lib::core::git::{DefaultGitService, errors::{GitError, ErrorCategory}};
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    base.join(format!("fwc-{}-{}", prefix, uuid::Uuid::new_v4()))
}

fn assert_category(err: GitError, want: ErrorCategory) {
    match err { GitError::Categorized { category, .. } => assert_eq!(category, want, "unexpected category"), }
}

#[test]
fn clone_cancel_quick_returns_cancel() {
    let svc = DefaultGitService::new();
    let repo = "https://example.com/owner/repo.git"; // 仅用于通过初步形态校验，不会实际访问网络（立即取消）
    let dest = unique_temp_dir("clone-cancel");
    let flag = AtomicBool::new(true); // 立即取消
    let out = svc.clone_blocking(repo, &dest, &flag, |_p| {});
    assert!(out.is_err());
    assert_category(out.err().unwrap(), ErrorCategory::Cancel);
}

#[test]
fn fetch_missing_git_dir_fails_fast() {
    let svc = DefaultGitService::new();
    let dest = unique_temp_dir("fetch-missing-git");
    std::fs::create_dir_all(&dest).unwrap(); // 但不创建 .git
    let flag = AtomicBool::new(false);
    let out = svc.fetch_blocking("", &dest, &flag, |_p| {});
    assert!(out.is_err());
    assert_category(out.err().unwrap(), ErrorCategory::Internal);
}

#[test]
fn fetch_cancel_quick_returns_cancel() {
    // 创建一个本地空仓库（仅用于通过 .git 存在性校验）
    let repo = unique_temp_dir("fetch-cancel");
    std::fs::create_dir_all(&repo).unwrap();
    let st = Command::new("git").current_dir(&repo).args(["init", "--quiet"]).status().expect("git init");
    assert!(st.success());

    let svc = DefaultGitService::new();
    let flag = AtomicBool::new(true); // 立即取消
    let out = svc.fetch_blocking("", &repo, &flag, |_p| {});
    assert!(out.is_err());
    assert_category(out.err().unwrap(), ErrorCategory::Cancel);
}
