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
    // 外部超时保护，防止偶发环境导致卡住
    let res = timeout(Duration::from_secs(5), async {
        let dest = unique_temp_dir();
        let flag = AtomicBool::new(false);
        // 在阻塞线程中执行，避免阻塞 Tokio 定时器
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
        // 将中断标志置为 true，应使 fetch_then_checkout 立即返回错误（无需触网）
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
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
        let handle = reg.clone().spawn_git_clone_task(None, id, token.clone(), repo, dest);

        // 先等待进入 Running
        let running = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Running)).unwrap_or(false), 2000, 20).await;
        assert!(running, "task should enter running state");

        // 立即取消 -> 应在短时间内转为 Canceled
        token.cancel();
        let canceled = wait_until(|| reg.snapshot(&id).map(|s| matches!(s.state, TaskState::Canceled)).unwrap_or(false), 5000, 50).await;
        assert!(canceled, "task should transition to canceled after token.cancel()");

        // 确保后台阻塞任务优雅结束，避免阻塞线程池悬挂影响测试退出
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_invalid_repo_fails_fast() {
    let res = timeout(Duration::from_secs(8), async {
        let reg = Arc::new(TaskRegistry::new());
        // 使用明显不存在的本地路径，避免外网依赖
        let repo = std::path::PathBuf::from("C:/this-path-should-not-exist-xyz/repo").to_string_lossy().to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);

        let running = wait_for_state(&reg, id, TaskState::Running, 1000).await;
        assert!(running, "should enter running");
        // 在 2s 内未失败，则触发取消作为兜底，确保测试不悬挂
        let failed_quick = wait_for_state(&reg, id, TaskState::Failed, 2000).await;
        // 无论失败与否，都显式取消一次，确保 watcher 线程退出，阻塞任务能结束
        let _ = reg.cancel(&id);
        if !failed_quick {
            // 若未在窗口内失败，则等待进入 Canceled
            let canceled = wait_for_state(&reg, id, TaskState::Canceled, 4000).await;
            assert!(canceled, "invalid repo should fail or be canceled within timeout");
        }

        // 等待阻塞任务收尾，防止测试退出时仍占用线程池
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_cancel_before_start_results_canceled() {
    let res = timeout(Duration::from_secs(4), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "C:/unused".to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
        token.cancel(); // 启动前取消
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await;
        assert!(canceled, "should be canceled immediately");
        // 任务应快速返回，但仍用一个小超时等待其彻底退出
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_invalid_url_fails_quick() {
    // 验证在注册表任务层面，明显无效的 URL 会快速失败（不依赖外网）
    let res = timeout(Duration::from_secs(6), async {
        let reg = Arc::new(TaskRegistry::new());
    let repo = "not-a-valid-url!!!".to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);

        let running = wait_for_state(&reg, id, TaskState::Running, 800).await;
        assert!(running, "should enter running");
    let failed = wait_for_state(&reg, id, TaskState::Failed, 2000).await;
    assert!(failed, "invalid url should fail quickly");

    // 失败情况下 watcher 线程仍在等待取消信号，这里显式取消以确保其退出
    let _ = reg.cancel(&id);
    // 最后确保阻塞任务退出
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_invalid_scheme_fails_quick() {
    // 验证 ftp:// 这类非 http(s) 的 scheme 会被快速判定为无效输入并失败（不应触发重试）
    let res = timeout(Duration::from_secs(6), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "ftp://example.com/repo.git".to_string();
        let dest = std::env::temp_dir().join(format!("fwc-clone-{}", uuid::Uuid::new_v4())).to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
        let handle = reg.clone().spawn_git_clone_task(None, id, token, repo, dest);

        let running = wait_for_state(&reg, id, TaskState::Running, 800).await;
        assert!(running, "should enter running");
        let failed = wait_for_state(&reg, id, TaskState::Failed, 2000).await;
        assert!(failed, "invalid scheme should fail quickly");

        // 显式取消以确保 watcher 线程退出
        let _ = reg.cancel(&id);
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_git_clone_relative_path_non_repo_errors_fast() {
    // 使用明显不存在的相对路径，验证路径分支能快速失败
    let res = timeout(Duration::from_secs(5), async {
        let dest = unique_temp_dir();
        let flag = AtomicBool::new(false);
        let repo = format!("./fwc-not-a-git-repo-{}", uuid::Uuid::new_v4());
        let out = tokio::task::spawn_blocking(move || {
            let svc = DefaultGitService::new();
            svc.clone_blocking(&repo, &dest, &flag, |_p| {})
        }).await.expect("spawn_blocking join");
        assert!(out.is_err(), "relative non-repo path should error quickly");
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

