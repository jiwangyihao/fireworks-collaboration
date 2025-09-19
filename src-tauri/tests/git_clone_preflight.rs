#![cfg(not(feature = "tauri-app"))]

use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::service::GitService;
use fireworks_collaboration_lib::core::git::{DefaultGitService, errors::{GitError, ErrorCategory}};

fn assert_category(err: GitError, want: ErrorCategory) {
    match err { GitError::Categorized { category, .. } => assert_eq!(category, want, "unexpected category"), }
}

#[test]
fn clone_fails_fast_when_source_path_missing() {
    let svc = DefaultGitService::new();
    let repo = "C:/this/path/should/not/exist/for/test"; // looks like a local path
    let dest = std::env::temp_dir().join(format!("fwc-preflight-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest).unwrap();
    let flag = AtomicBool::new(false);
    let out = svc.clone_blocking(repo, &dest, None, &flag, |_p| {});
    assert!(out.is_err());
    assert_category(out.err().unwrap(), ErrorCategory::Internal);
}

#[test]
fn clone_fails_fast_on_unsupported_url_scheme() {
    let svc = DefaultGitService::new();
    let repo = "ftp://example.com/repo.git"; // unsupported scheme
    let dest = std::env::temp_dir().join(format!("fwc-preflight-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest).unwrap();
    let flag = AtomicBool::new(false);
    let out = svc.clone_blocking(repo, &dest, None, &flag, |_p| {});
    assert!(out.is_err());
    assert_category(out.err().unwrap(), ErrorCategory::Internal);
}

#[test]
fn clone_fails_fast_on_invalid_repo_string() {
    let svc = DefaultGitService::new();
    let repo = "mailto:abc"; // 不像路径、也不是 http(s)/scp-like → 立即判定为无效输入
    let dest = std::env::temp_dir().join(format!("fwc-preflight-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest).unwrap();
    let flag = AtomicBool::new(false);
    let out = svc.clone_blocking(repo, &dest, None, &flag, |_p| {});
    assert!(out.is_err());
    assert_category(out.err().unwrap(), ErrorCategory::Internal);
}
