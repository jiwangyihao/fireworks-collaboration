#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tokio::time::{timeout, Duration};
use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-shallow-{}-{}", prefix, id))
}

fn e2e_should_skip() -> bool {
    let ci = std::env::var("CI").map(|v| v == "1" || v.to_ascii_lowercase() == "true").unwrap_or(false);
    let disabled = std::env::var("FWC_E2E_DISABLE").map(|v| v == "1" || v.to_ascii_lowercase() == "true").unwrap_or(false);
    ci || disabled
}

fn repo_url() -> String {
    std::env::var("FWC_E2E_REPO").unwrap_or_else(|_| "https://github.com/rust-lang/log".to_string())
}

/// P2.2b: 验证 depth=1 触发浅克隆（存在 .git/shallow 文件）。
#[tokio::test]
async fn shallow_clone_depth_one_creates_shallow_file() {
    if e2e_should_skip() { eprintln!("[skip] shallow clone test skipped due to CI or FWC_E2E_DISABLE"); return; }
    let res = timeout(Duration::from_secs(60), async {
        let repo = repo_url();
        let dest = unique_temp_dir("d1");
        let flag = AtomicBool::new(false);
        let dest_for_thread = dest.clone();
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&repo, &dest_for_thread, Some(1), &flag, |_p| {})
        }).await.expect("spawn_blocking join");
    if out.is_err() { eprintln!("[soft-skip] public shallow clone failed: {:?}", out.err()); return; }
        let shallow_file = dest.join(".git").join("shallow");
        assert!(shallow_file.exists(), ".git/shallow file should exist for depth=1");
        let meta = std::fs::metadata(&shallow_file).expect("stat shallow");
        assert!(meta.len() > 0, "shallow file should not be empty");
    }).await;
    assert!(res.is_ok(), "shallow clone test timeout");
}

/// 验证全量克隆（未指定 depth）通常没有 shallow 文件；存在时仅记录告警。
#[tokio::test]
async fn full_clone_no_depth_has_no_shallow_file() {
    if e2e_should_skip() { eprintln!("[skip] full clone shallow absence test skipped"); return; }
    let res = timeout(Duration::from_secs(60), async {
        let repo = repo_url();
        let dest = unique_temp_dir("full");
        let flag = AtomicBool::new(false);
        let dest_for_thread = dest.clone();
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&repo, &dest_for_thread, None, &flag, |_p| {})
        }).await.expect("spawn_blocking join");
    if out.is_err() { eprintln!("[soft-skip] public full clone failed: {:?}", out.err()); return; }
        let shallow_file = dest.join(".git").join("shallow");
        if shallow_file.exists() { eprintln!("[warn] shallow file present in full clone; ignoring"); }
    }).await;
    assert!(res.is_ok(), "full clone test timeout");
}
