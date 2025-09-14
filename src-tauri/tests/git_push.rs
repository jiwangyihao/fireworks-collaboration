#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::process::Command;

use fireworks_collaboration_lib::core::git::service::GitService;
use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::TaskState;
use tokio::time::{sleep, Duration, timeout};

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-git2-push-{}", id))
}

fn run_in(dir: &PathBuf, args: &[&str]) {
    let st = Command::new("git").current_dir(dir).args(args).status().unwrap();
    assert!(st.success(), "git {:?} in {:?} should succeed", args, dir);
}

#[test]
fn push_blocking_success_local() {
    // 1) 创建裸远程仓库
    let remote = unique_temp_dir();
    std::fs::create_dir_all(&remote).unwrap();
    let st = Command::new("git").args(["init", "--bare", "--quiet", remote.to_string_lossy().as_ref()]).status().expect("git init --bare");
    assert!(st.success(), "init bare remote should succeed");

    // 2) 创建工作仓库并设置 origin 指向裸仓库
    let work = unique_temp_dir();
    std::fs::create_dir_all(&work).unwrap();
    run_in(&work, &["init", "--quiet"]);
    run_in(&work, &["config", "user.email", "you@example.com"]);
    run_in(&work, &["config", "user.name", "You"]);
    run_in(&work, &["remote", "add", "origin", remote.to_string_lossy().as_ref()]);
    std::fs::write(work.join("a.txt"), "1").unwrap();
    run_in(&work, &["add", "."]);
    run_in(&work, &["commit", "-m", "c1"]);

    // 3) 调用 push_blocking 推送 master 到远程
    let service = DefaultGitService::new();
    let flag = AtomicBool::new(false);
    let res = service.push_blocking(
        &work,
        Some("origin"),
        Some(&["refs/heads/master:refs/heads/master"][..]),
        None,
        &flag,
        |_p| {},
    );
    assert!(res.is_ok(), "local push should succeed");

    // 4) 校验远程的 HEAD 分支引用存在
    let out = Command::new("git").current_dir(&remote).args(["show-ref", "refs/heads/master"]).output().unwrap();
    assert!(out.status.success(), "remote should contain refs/heads/master after push");
}

#[test]
fn push_cancel_flag_results_in_cancel_error() {
    // 准备一个工作仓库与裸远程
    let remote = unique_temp_dir();
    std::fs::create_dir_all(&remote).unwrap();
    let st = Command::new("git").args(["init", "--bare", "--quiet", remote.to_string_lossy().as_ref()]).status().expect("git init --bare");
    assert!(st.success());

    let work = unique_temp_dir();
    std::fs::create_dir_all(&work).unwrap();
    run_in(&work, &["init", "--quiet"]);
    run_in(&work, &["config", "user.email", "you@example.com"]);
    run_in(&work, &["config", "user.name", "You"]);
    run_in(&work, &["remote", "add", "origin", remote.to_string_lossy().as_ref()]);
    std::fs::write(work.join("b.txt"), "1").unwrap();
    run_in(&work, &["add", "."]);
    run_in(&work, &["commit", "-m", "c1"]);

    let service = DefaultGitService::new();
    let flag = AtomicBool::new(true); // 立即取消
    let out = service.push_blocking(
        &work,
        Some("origin"),
        Some(&["refs/heads/master:refs/heads/master"][..]),
        None,
        &flag,
        |_p| {},
    );
    assert!(matches!(out, Err(e) if matches!(e, fireworks_collaboration_lib::core::git::errors::GitError::Categorized { category: fireworks_collaboration_lib::core::git::errors::ErrorCategory::Cancel, .. })), "cancel should map to Cancel category");
}

#[tokio::test]
async fn registry_push_local_completes() {
    // 裸远程
    let remote = unique_temp_dir();
    std::fs::create_dir_all(&remote).unwrap();
    let st = Command::new("git").args(["init", "--bare", "--quiet", remote.to_string_lossy().as_ref()]).status().expect("git init --bare");
    assert!(st.success());

    // 工作仓库
    let work = unique_temp_dir();
    std::fs::create_dir_all(&work).unwrap();
    run_in(&work, &["init", "--quiet"]);
    run_in(&work, &["config", "user.email", "you@example.com"]);
    run_in(&work, &["config", "user.name", "You"]);
    run_in(&work, &["remote", "add", "origin", remote.to_string_lossy().as_ref()]);
    std::fs::write(work.join("c.txt"), "1").unwrap();
    run_in(&work, &["add", "."]);
    run_in(&work, &["commit", "-m", "c1"]);

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitPush { dest: work.to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: Some(vec!["refs/heads/master:refs/heads/master".into()]), username: None, password: None });
    let handle = reg.spawn_git_push_task(None, id, token, work.to_string_lossy().to_string(), Some("origin".into()), Some(vec!["refs/heads/master:refs/heads/master".into()]), None, None);

    // 简单等待完成（最多 3s）
    let start = std::time::Instant::now();
    loop {
        if let Some(snap) = reg.snapshot(&id) {
            if matches!(snap.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) {
                break;
            }
        }
        if start.elapsed() > std::time::Duration::from_secs(3) { break; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let snap = reg.snapshot(&id).expect("snapshot");
    assert_eq!(snap.state, TaskState::Completed, "push task should complete");
    handle.await.unwrap();
}

#[tokio::test]
async fn registry_push_cancel_before_start_results_canceled() {
    let res = timeout(Duration::from_secs(4), async {
        // 裸远程
        let remote = unique_temp_dir();
        std::fs::create_dir_all(&remote).unwrap();
        let st = Command::new("git").args(["init", "--bare", "--quiet", remote.to_string_lossy().as_ref()]).status().expect("git init --bare");
        assert!(st.success());

        // 工作仓库
        let work = unique_temp_dir();
        std::fs::create_dir_all(&work).unwrap();
        run_in(&work, &["init", "--quiet"]);
        run_in(&work, &["config", "user.email", "you@example.com"]);
        run_in(&work, &["config", "user.name", "You"]);
        run_in(&work, &["remote", "add", "origin", remote.to_string_lossy().as_ref()]);
        std::fs::write(work.join("d.txt"), "1").unwrap();
        run_in(&work, &["add", "."]);
        run_in(&work, &["commit", "-m", "c1"]);

        let reg = std::sync::Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitPush { dest: work.to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: Some(vec!["refs/heads/master:refs/heads/master".into()]), username: None, password: None });
        token.cancel();
        let handle = reg.spawn_git_push_task(None, id, token, work.to_string_lossy().to_string(), Some("origin".into()), Some(vec!["refs/heads/master:refs/heads/master".into()]), None, None);
        // 等待取消状态
        let start = std::time::Instant::now();
        loop {
            if let Some(snap) = reg.snapshot(&id) {
                if snap.state == TaskState::Canceled { break; }
            }
            if start.elapsed() > Duration::from_millis(800) { break; }
            sleep(Duration::from_millis(20)).await;
        }
        let snap = reg.snapshot(&id).expect("snapshot");
        assert_eq!(snap.state, TaskState::Canceled, "push task should be canceled before start");
        let _ = handle.await;
    }).await;
    assert!(res.is_ok(), "test exceeded timeout window");
}

#[test]
fn push_invalid_dest_fails_quick() {
    // 目标不是 git 仓库，push 应快速失败
    let dest = unique_temp_dir();
    // dest 不创建 .git
    let service = DefaultGitService::new();
    let flag = AtomicBool::new(false);
    let out = service.push_blocking(&dest, Some("origin"), Some(&["refs/heads/main:refs/heads/main"][..]), None, &flag, |_p| {});
    assert!(out.is_err(), "non-repo dest should fail quickly");
}

#[test]
fn push_emits_preupload_and_completed_phases() {
    // 搭建本地仓库与远程，收集阶段事件
    let remote = unique_temp_dir();
    std::fs::create_dir_all(&remote).unwrap();
    let st = Command::new("git").args(["init", "--bare", "--quiet", remote.to_string_lossy().as_ref()]).status().expect("git init --bare");
    assert!(st.success());

    let work = unique_temp_dir();
    std::fs::create_dir_all(&work).unwrap();
    run_in(&work, &["init", "--quiet"]);
    run_in(&work, &["config", "user.email", "you@example.com"]);
    run_in(&work, &["config", "user.name", "You"]);
    run_in(&work, &["remote", "add", "origin", remote.to_string_lossy().as_ref()]);
    std::fs::write(work.join("e.txt"), "1").unwrap();
    run_in(&work, &["add", "."]);
    run_in(&work, &["commit", "-m", "c1"]);

    let service = DefaultGitService::new();
    let flag = AtomicBool::new(false);
    let mut phases: Vec<String> = vec![];
    let res = service.push_blocking(&work, Some("origin"), Some(&["refs/heads/master:refs/heads/master"][..]), None, &flag, |p| { phases.push(p.phase); });
    assert!(res.is_ok(), "push should succeed");
    assert!(phases.iter().any(|ph| ph == "PreUpload"), "should emit PreUpload phase");
    assert!(phases.iter().any(|ph| ph == "Completed"), "should emit Completed phase");
}
