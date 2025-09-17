#![cfg(not(feature = "tauri-app"))]
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::init::git_init;
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

fn tmp_dir() -> std::path::PathBuf { std::env::temp_dir().join(format!("fwc-init-{}", uuid::Uuid::new_v4())) }

fn assert_cat(err: GitError, want: ErrorCategory) { match err { GitError::Categorized { category, .. } => assert_eq!(category, want, "unexpected category") } }

#[test]
fn git_init_success_and_idempotent() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(false);
    let mut phases = Vec::new();
    git_init(&dest, &flag, |p: ProgressPayload| { phases.push(p.phase); }).expect("init ok");
    assert!(dest.join(".git").exists(), ".git should exist after init");
    // second time should be idempotent
    git_init(&dest, &flag, |_p| {}).expect("idempotent");
}

#[test]
fn git_init_fails_when_path_is_file() {
    let dest = tmp_dir();
    std::fs::create_dir_all(&dest).unwrap();
    let file_path = dest.join("a.txt");
    std::fs::write(&file_path, "hi").unwrap();
    // Call init on the file path directly
    let flag = AtomicBool::new(false);
    let out = git_init(&file_path, &flag, |_p| {});
    assert!(out.is_err());
    assert_cat(out.err().unwrap(), ErrorCategory::Protocol);
}

#[test]
fn git_init_cancel_before() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(true); // already canceled
    let out = git_init(&dest, &flag, |_p| {});
    assert!(out.is_err());
    assert_cat(out.err().unwrap(), ErrorCategory::Cancel);
}
