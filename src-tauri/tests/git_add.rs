#![cfg(not(feature = "tauri-app"))]
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, add::git_add};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

fn tmp_dir() -> std::path::PathBuf { std::env::temp_dir().join(format!("fwc-add-{}", uuid::Uuid::new_v4())) }
fn assert_cat(err: GitError, want: ErrorCategory) { match err { GitError::Categorized { category, .. } => assert_eq!(category, want, "unexpected category") } }

#[test]
fn git_add_success_files_and_dir() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    // create files & dir
    std::fs::write(dest.join("a.txt"), "hello").unwrap();
    std::fs::create_dir_all(dest.join("dir/sub")).unwrap();
    std::fs::write(dest.join("dir/sub/b.txt"), "world").unwrap();
    let mut phases = Vec::new();
    git_add(&dest, &["a.txt", "dir"], &flag, |p: ProgressPayload| { phases.push(p.phase); }).unwrap();
    // verify staged entries exist in index
    let repo = git2::Repository::open(&dest).unwrap();
    let idx = repo.index().unwrap();
    assert!(idx.get_path(std::path::Path::new("a.txt"), 0).is_some());
    assert!(idx.get_path(std::path::Path::new("dir/sub/b.txt"), 0).is_some());
}

#[test]
fn git_add_rejects_empty_list() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    let out = git_add(&dest, &[], &flag, |_p| {});
    assert!(out.is_err());
    assert_cat(out.err().unwrap(), ErrorCategory::Protocol);
}

#[test]
fn git_add_rejects_outside_path() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    let outside = std::env::temp_dir().join("outside-file.txt");
    std::fs::write(&outside, "x").unwrap();
    // try to add with relative path containing .. to escape
    let out = git_add(&dest, &["../outside-file.txt"], &flag, |_p| {});
    assert!(out.is_err());
    assert_cat(out.err().unwrap(), ErrorCategory::Protocol);
}

#[test]
fn git_add_cancelled() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(true); // already canceled
    let _ = git_init(&dest, &flag, |_p| {}); // returns Cancel
    // Now attempt add with cancel flag
    let out = git_add(&dest, &["a.txt"], &flag, |_p| {});
    assert!(out.is_err());
    assert_cat(out.err().unwrap(), ErrorCategory::Cancel);
}
