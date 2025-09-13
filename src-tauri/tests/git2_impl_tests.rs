#![cfg(all(not(feature = "tauri-app"), feature = "git-impl-git2"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::service::GitService;
use fireworks_collaboration_lib::core::git::git2_impl::Git2Service;

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-git2-test-{}", id))
}

#[test]
fn clone_reports_initial_negotiating_progress() {
    let service = Git2Service::new();
    let dest = unique_temp_dir();
    let flag = AtomicBool::new(true); // 立刻取消，避免真实网络
    let mut saw_negotiating = false;
    let _ = service.clone_blocking(
        "https://invalid-host.invalid/repo.git",
        &dest,
        &flag,
        |p| {
            if p.phase == "Negotiating" {
                saw_negotiating = true;
            }
        },
    );
    assert!(saw_negotiating, "should emit Negotiating phase at start");
}

#[test]
fn clone_cancel_flag_results_in_cancel_error() {
    let service = Git2Service::new();
    let dest = unique_temp_dir();
    let flag = AtomicBool::new(true); // 一开始就取消
    let out = service.clone_blocking(
        "https://example.com/any.git",
        &dest,
        &flag,
        |_p| {},
    );
    // 由于我们在 transfer_progress 中进行取消检查，可能在真正开始下载前返回 Cancel
    assert!(matches!(out, Err(e) if matches!(e, fireworks_collaboration_lib::core::git::errors::GitError::Categorized { category: fireworks_collaboration_lib::core::git::errors::ErrorCategory::Cancel, .. })), "cancel should map to Cancel category");
}

#[test]
fn clone_invalid_local_path_fails_quick() {
    let service = Git2Service::new();
    let dest = unique_temp_dir();
    // 使用明显不存在的本地路径（非 URL），git2 会尝试按本地路径处理并失败
    let repo = PathBuf::from("C:/this-path-should-not-exist-xyz/repo");
    let flag = AtomicBool::new(false);
    let out = service.clone_blocking(
        repo.to_string_lossy().as_ref(),
        &dest,
        &flag,
        |_p| {},
    );
    assert!(out.is_err(), "invalid local path should fail fast");
}

#[test]
fn clone_from_local_repo_succeeds_and_completes_with_valid_progress() {
    use std::process::Command;
    // 1) 在临时目录创建一个裸仓库作为源（或普通仓库也可）
    let work = unique_temp_dir();
    std::fs::create_dir_all(&work).unwrap();
    // git init
    let status = Command::new("git").args(["init", "--quiet", work.to_string_lossy().as_ref()]).status().expect("git init");
    assert!(status.success(), "git init should succeed");
    // 在仓库中创建一个文件并提交
    let run = |args: &[&str]| {
        let st = Command::new("git").current_dir(&work).args(args).status().unwrap();
        assert!(st.success(), "git {:?} should succeed", args);
    };
    run(&["config", "user.email", "you@example.com"]);
    run(&["config", "user.name", "You"]);
    std::fs::write(work.join("README.md"), "hello").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "init"]);

    // 2) 使用 Git2Service 执行克隆（本地路径，无需网络）
    let service = Git2Service::new();
    let dest = unique_temp_dir();
    let flag = AtomicBool::new(false);
    let mut completed = false;
    let mut last_percent = 0;
    let mut percents: Vec<u32> = vec![];
    let mut phases: Vec<String> = vec![];
    let out = service.clone_blocking(
        work.to_string_lossy().as_ref(),
        &dest,
        &flag,
        |p| {
            last_percent = p.percent;
            percents.push(p.percent);
            phases.push(p.phase.clone());
            if p.phase == "Completed" { completed = true; }
        },
    );
    assert!(out.is_ok(), "local clone should succeed");
    assert!(completed, "should emit Completed phase");
    assert_eq!(last_percent, 100, "final percent should be 100");
    assert!(percents.iter().all(|p| *p <= 100), "all percents should be <= 100");
    // 至少出现 Negotiating 与 Checkout 阶段（Receiving 可能很快但通常会出现）
    assert!(phases.iter().any(|ph| ph == "Negotiating"), "should see Negotiating phase");
    assert!(phases.iter().any(|ph| ph == "Checkout"), "should see Checkout phase");
    // 3) 目标目录应已成为 git 仓库
    assert!(dest.join(".git").exists(), "dest should contain .git");
}
