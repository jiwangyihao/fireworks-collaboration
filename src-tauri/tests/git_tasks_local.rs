#![cfg(all(not(feature = "tauri-app"), feature = "git-impl-git2"))]

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-gitlocal-test-{}", id))
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
async fn registry_clone_local_repo_completes() {
    // 准备一个最小本地仓库
    let src = unique_temp_dir();
    std::fs::create_dir_all(&src).unwrap();
    let status = Command::new("git").args(["init", "--quiet", src.to_string_lossy().as_ref()]).status().expect("git init");
    assert!(status.success(), "git init should succeed");
    let run = |args: &[&str]| {
        let st = Command::new("git").current_dir(&src).args(args).status().unwrap();
        assert!(st.success(), "git {:?} should succeed", args);
    };
    run(&["config", "user.email", "you@example.com"]);
    run(&["config", "user.name", "You"]);
    std::fs::write(src.join("one.txt"), "1").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "init"]);

    // 启动注册表任务进行克隆
    let reg = Arc::new(TaskRegistry::new());
    let dest = unique_temp_dir().to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitClone { repo: src.to_string_lossy().to_string(), dest: dest.clone() });
    let handle = reg.clone().spawn_git_clone_task(None, id, token, src.to_string_lossy().to_string(), dest.clone());

    // 等待完成
    let completed = wait_for_state(&reg, id, TaskState::Completed, 10_000).await;
    assert!(completed, "local clone task should complete");
    let _ = handle.await;
}
