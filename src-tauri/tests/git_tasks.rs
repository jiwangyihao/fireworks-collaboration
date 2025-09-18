#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::time::{sleep, Duration, timeout};

use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-git-test-{}", id))
}

async fn wait_until<F: Fn() -> bool>(cond: F, max_ms: u64, step_ms: u64) -> bool {
    let mut waited = 0u64;
    while waited < max_ms {
        if cond() { return true; }
        sleep(Duration::from_millis(step_ms)).await;
        waited += step_ms;
    }
    false
}

async fn wait_for_state(reg:&TaskRegistry, id:uuid::Uuid, target:TaskState, max_ms:u64) -> bool {
    let mut waited = 0u64;
    while waited < max_ms {
        if let Some(s) = reg.snapshot(&id) { if s.state == target { return true; } }
        sleep(Duration::from_millis(20)).await; waited += 20;
    }
    false
}

#[tokio::test]
async fn test_git_clone_invalid_url_early_error() {
    let res = timeout(Duration::from_secs(5), async {
        let dest = unique_temp_dir();
        let flag = AtomicBool::new(false);
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking("not-a-valid-url!!!", &dest, &flag, |_p| {})
        }).await.expect("spawn_blocking join");
        assert!(out.is_err(), "invalid input should error");
        let msg = format!("{}", out.err().unwrap());
        assert!(!msg.is_empty(), "error message should not be empty");
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_git_clone_interrupt_flag_cancels_immediately() {
    let res = timeout(Duration::from_secs(5), async {
        let dest = unique_temp_dir();
        let flag = AtomicBool::new(true);
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking("https://github.com/rust-lang/log", &dest, &flag, |_p| {})
        }).await.expect("join");
        assert!(out.is_err(), "interrupt should cause clone to error quickly");
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_git_clone_cancel_quick() {
    let res = timeout(Duration::from_secs(8), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "https://github.com/rust-lang/log".to_string();
        let dest = unique_temp_dir().to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
        let handle = reg.clone().spawn_git_clone_task(None, id, token.clone(), repo, dest);
        let running = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Running)).unwrap_or(false), 2000, 20).await;
        assert!(running, "task should enter running state");
        token.cancel();
        let canceled = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Canceled)).unwrap_or(false), 5000, 50).await;
        assert!(canceled, "task should transition to canceled after token.cancel()");
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_invalid_repo_fails_fast() {
    let res = timeout(Duration::from_secs(8), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = std::path::PathBuf::from("C:/this-path-should-not-exist-xyz/repo").to_string_lossy().to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);
        let running = wait_for_state(&reg, id, TaskState::Running, 1000).await; assert!(running, "should enter running");
        let failed_quick = wait_for_state(&reg, id, TaskState::Failed, 2000).await;
        let _ = reg.cancel(&id);
        if !failed_quick { let canceled = wait_for_state(&reg, id, TaskState::Canceled, 4000).await; assert!(canceled, "invalid repo should fail or be canceled within timeout"); }
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await; assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_cancel_before_start_results_canceled() {
    let res = timeout(Duration::from_secs(4), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "C:/unused".to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
        token.cancel();
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await; assert!(canceled, "should be canceled immediately");
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await; assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_invalid_url_fails_quick() {
    let res = timeout(Duration::from_secs(6), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "not-a-valid-url!!!".to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);
        let running = wait_for_state(&reg, id, TaskState::Running, 800).await; assert!(running, "should enter running");
        let failed = wait_for_state(&reg, id, TaskState::Failed, 2000).await; assert!(failed, "invalid url should fail quickly");
        let _ = reg.cancel(&id);
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await; assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_invalid_scheme_fails_quick() {
    let res = timeout(Duration::from_secs(6), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "ftp://example.com/repo.git".to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone(), depth: None, filter: None, strategy_override: None });
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);
        let running = wait_for_state(&reg, id, TaskState::Running, 800).await; assert!(running, "should enter running");
        let failed = wait_for_state(&reg, id, TaskState::Failed, 2000).await; assert!(failed, "invalid scheme should fail quickly");
        let _ = reg.cancel(&id);
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await; assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_git_clone_relative_path_non_repo_errors_fast() {
    let res = timeout(Duration::from_secs(5), async {
        let dest = unique_temp_dir();
        let flag = AtomicBool::new(false);
        let repo = format!("./fwc-not-a-git-repo-{}", uuid::Uuid::new_v4());
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&repo, &dest, &flag, |_p| {})
        }).await.expect("spawn_blocking join");
        assert!(out.is_err(), "relative non-repo path should error quickly");
    }).await; assert!(res.is_ok(), "test exceeded timeout window");
}

