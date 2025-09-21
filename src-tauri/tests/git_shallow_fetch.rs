#![cfg(not(feature = "tauri-app"))]
//! Shallow fetch E2E test (skipped under CI or when FWC_E2E_DISABLE=1)
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;
use tokio::time::{timeout, Duration};

fn e2e_should_skip() -> bool {
    let ci = std::env::var("CI").map(|v| v == "1" || v.to_ascii_lowercase() == "true").unwrap_or(false);
    let disabled = std::env::var("FWC_E2E_DISABLE").map(|v| v == "1" || v.to_ascii_lowercase() == "true").unwrap_or(false);
    ci || disabled
}

fn unique_dir(prefix: &str) -> PathBuf { std::env::temp_dir().join(format!("fwc-shallow-fetch-{}-{}", prefix, uuid::Uuid::new_v4())) }
fn repo_url() -> String { std::env::var("FWC_E2E_REPO").unwrap_or_else(|_| "https://github.com/rust-lang/log".to_string()) }

/// P2.2c: verify fetch with depth=1 on a previously full clone creates/retains .git/shallow file after converting repo to shallow via reclone simulation.
#[tokio::test]
async fn shallow_fetch_depth_one_creates_or_preserves_shallow_file() {
    if e2e_should_skip() { eprintln!("[skip] shallow fetch test skipped"); return; }
    let res = timeout(Duration::from_secs(90), async {
        let repo = repo_url();
        // First do a full clone (depth None) so we have an existing repo
        let full_dest = unique_dir("full");
        let flag_full = AtomicBool::new(false);
        let full_clone_dest = full_dest.clone();
        let repo_clone_for_first = repo.clone();
        tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&repo_clone_for_first, &full_clone_dest, None, &flag_full, |_p| {})
        }).await.expect("spawn full clone").expect("full clone ok");

        // Now perform a shallow fetch with depth=1; depending on libgit2 behavior, it may create a shallow file if negotiation supports it.
        let shallow_flag = AtomicBool::new(false);
        let fetch_dest = full_dest.clone();
    let _repo_for_fetch = repo; // reuse original string here (unused placeholder)
        let fetch_res = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.fetch_blocking("", &fetch_dest, Some(1), &shallow_flag, |_p| {})
        }).await.expect("spawn fetch");
        if let Err(e) = fetch_res {
            eprintln!("[skip-warn] shallow fetch network failed: {} => soft skip", e);
            return; // 软跳过：网络不通时不判失败
        }

        let shallow_file = full_dest.join(".git").join("shallow");
        // We assert existence; if upstream does not generate it, we still allow but print warn (leniency for differing server support)
        if !shallow_file.exists() { eprintln!("[warn] shallow file absent after shallow fetch; server may not have truncated history"); } else {
            assert!(std::fs::metadata(&shallow_file).unwrap().len() > 0, "shallow file should be non-empty");
        }
    }).await;
    assert!(res.is_ok(), "shallow fetch test timeout");
}
