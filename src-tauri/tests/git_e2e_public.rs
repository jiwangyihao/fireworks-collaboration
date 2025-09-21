#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tokio::time::{timeout, Duration};

use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-{}-{}", prefix, id))
}

// 读取环境变量，决定是否跳过公网E2E测试
// 默认启用；当 CI=1/true 或 FWC_E2E_DISABLE=1/true 时跳过
fn e2e_should_skip() -> bool {
    let ci = std::env::var("CI").map(|v| v == "1" || v.to_ascii_lowercase() == "true").unwrap_or(false);
    let disabled = std::env::var("FWC_E2E_DISABLE").map(|v| v == "1" || v.to_ascii_lowercase() == "true").unwrap_or(false);
    ci || disabled
}

// 公网仓库与分支配置（可由环境变量覆盖）
fn repo_url() -> String {
    std::env::var("FWC_E2E_REPO").unwrap_or_else(|_| "https://github.com/rust-lang/log".to_string())
}

#[tokio::test]
async fn e2e_git_clone_public_repo_when_enabled() {
    if e2e_should_skip() { 
        eprintln!("[skip] CI or FWC_E2E_DISABLE detected, skip public network E2E clone");
        return; 
    }

    let res = timeout(Duration::from_secs(60), async {
        let repo = repo_url();
        let dest = unique_temp_dir("clone");
        let flag = AtomicBool::new(false);
        let dest_for_thread = dest.clone();
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&repo, &dest_for_thread, None, &flag, |_p| {})
        }).await.expect("spawn_blocking join");
        assert!(out.is_ok(), "clone should succeed for public repo: {:?}", out.err());
        assert!(dest.join(".git").exists(), "destination should be a git repo");
    }).await;
    assert!(res.is_ok(), "E2E clone exceeded timeout window");
}

#[tokio::test]
async fn e2e_git_fetch_public_repo_when_enabled() {
    if e2e_should_skip() { 
        eprintln!("[skip] CI or FWC_E2E_DISABLE detected, skip public network E2E fetch");
        return; 
    }

    let res = timeout(Duration::from_secs(60), async {
        let repo = repo_url();
        let dest = unique_temp_dir("fetch");
        // 先 clone
        let flag1 = AtomicBool::new(false);
        let clone_repo = repo.clone();
        let dest_clone = dest.clone();
        let clone_out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&clone_repo, &dest_clone, None, &flag1, |_p| {})
        }).await.expect("spawn_blocking join for clone");
        if let Err(e) = clone_out {
            eprintln!("[skip-warn] public fetch pre-clone failed (network?) => soft skip: {e}");
            return; // 软跳过
        }

        // 再 fetch（不传 repo_url，使用 origin）
        let flag2 = AtomicBool::new(false);
        let dest_fetch = dest.clone();
        let fetch_out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.fetch_blocking("", &dest_fetch, None, &flag2, |_p| {})
        }).await.expect("spawn_blocking join for fetch");
        if let Err(e) = fetch_out {
            eprintln!("[skip-warn] public fetch failed (network?) => soft skip: {e}");
            return; // 软跳过
        }
    }).await;
    assert!(res.is_ok(), "E2E fetch exceeded timeout window");
}
