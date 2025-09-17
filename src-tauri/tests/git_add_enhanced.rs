#![cfg(not(feature = "tauri-app"))]
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::default_impl::{init::git_init, add::git_add};
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::service::ProgressPayload;

fn tmp_dir() -> std::path::PathBuf { std::env::temp_dir().join(format!("fwc-add-enh-{}", uuid::Uuid::new_v4())) }
fn cat(err: GitError) -> ErrorCategory { match err { GitError::Categorized { category, .. } => category } }

#[test]
fn git_add_rejects_absolute_and_dedupes() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("a.txt"), "hi").unwrap();
    // absolute path rejection
    let abs = if cfg!(windows) { "C:/Windows" } else { "/etc" };
    let out = git_add(&dest, &[abs], &flag, |_p| {});
    assert!(out.is_err());
    assert_eq!(cat(out.err().unwrap()), ErrorCategory::Protocol);

    // duplicate paths -> still succeeds once
    let out2 = git_add(&dest, &["a.txt", "a.txt"], &flag, |_p| {});
    assert!(out2.is_ok());
}

#[test]
fn git_add_emits_progress_monotonic() {
    let dest = tmp_dir();
    let flag = AtomicBool::new(false);
    git_init(&dest, &flag, |_p| {}).unwrap();
    std::fs::write(dest.join("f1.txt"), "1").unwrap();
    std::fs::write(dest.join("f2.txt"), "2").unwrap();
    let mut percents: Vec<u32> = Vec::new();
    let mut phases: Vec<String> = Vec::new();
    git_add(&dest, &["f1.txt", "f2.txt"], &flag, |p: ProgressPayload| { percents.push(p.percent); phases.push(p.phase); }).unwrap();
    assert!(percents.len() >= 2, "expect at least two progress events");
    for w in percents.windows(2) { assert!(w[1] >= w[0], "percent not monotonic: {:?}", percents); }
    assert!(phases.last().unwrap().contains("Staged"));
}
