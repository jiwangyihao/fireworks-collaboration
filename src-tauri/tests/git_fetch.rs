#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::time::{sleep, Duration, timeout};

use fireworks_collaboration_lib::core::git::fetch::fetch_blocking;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-gitfetch-test-{}", id))
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
async fn test_git_fetch_non_repo_dest_errors_fast() {
    let res = timeout(Duration::from_secs(5), async {
        let dest = unique_temp_dir();
        let flag = AtomicBool::new(false);
        let out = tokio::task::spawn_blocking(move || {
            // repo_url 传空串，等价于 `git fetch` 使用默认远程，但由于 dest 不是仓库，应立刻报错
            fetch_blocking("", &dest, &flag)
        }).await.expect("spawn_blocking join");
        assert!(out.is_err(), "non-repo dest should error quickly");
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_git_fetch_cancel_before_start_results_canceled() {
    let res = timeout(Duration::from_secs(4), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "".to_string(); // 使用默认远程逻辑
        let dest = unique_temp_dir().to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitFetch { repo: repo.clone(), dest: dest.clone() });
        token.cancel(); // 启动前取消
    let handle = reg.clone().spawn_git_fetch_task(None, id, token, repo, dest, None);
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await;
        assert!(canceled, "should be canceled immediately");
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[tokio::test]
async fn test_registry_git_fetch_invalid_dest_fails_quick() {
    let res = timeout(Duration::from_secs(6), async {
        let reg = Arc::new(TaskRegistry::new());
        let repo = "".to_string(); // 默认远程逻辑
        let dest = unique_temp_dir().to_string_lossy().to_string();
        let (id, token) = reg.create(TaskKind::GitFetch { repo: repo.clone(), dest: dest.clone() });
    let handle = reg.clone().spawn_git_fetch_task(None, id, token, repo, dest, None);

        // 任务可能非常快地从 Pending -> Failed，未必能观测到 Running，这里允许两者其一先出现
        let mut saw_running_or_failed = false;
        for _ in 0..100 { // 2s 内每 20ms 轮询一次
            if let Some(s) = reg.snapshot(&id) {
                if matches!(s.state, TaskState::Running | TaskState::Failed) { saw_running_or_failed = true; break; }
            }
            sleep(Duration::from_millis(20)).await;
        }
        assert!(saw_running_or_failed, "should enter running or fail quickly");

        // 非仓库路径应快速失败
        let failed = wait_for_state(&reg, id, TaskState::Failed, 2000).await;
        assert!(failed, "invalid dest should fail quickly");

        // 显式取消以结束 watcher 线程，然后等待阻塞任务结束
        let _ = reg.cancel(&id);
        let _ = timeout(Duration::from_secs(2), async { let _ = handle.await; }).await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}
