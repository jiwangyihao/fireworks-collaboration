#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::service::GitService;
use fireworks_collaboration_lib::core::git::DefaultGitService;

fn unique_temp_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let id = uuid::Uuid::new_v4().to_string();
    base.join(format!("fwc-git2-test-{}", id))
}

#[test]
fn clone_reports_initial_negotiating_progress() {
    let service = DefaultGitService::new();
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
    let service = DefaultGitService::new();
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
    let service = DefaultGitService::new();
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

    // 2) 使用 DefaultGitService 执行克隆（本地路径，无需网络）
    let service = DefaultGitService::new();
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

#[test]
fn fetch_cancel_flag_results_in_cancel_error() {
    use std::process::Command;
    // 准备一个本地目标仓库（空 repo，设置 origin 指向自身以满足远程存在，但我们会立即取消）
    let target = unique_temp_dir();
    std::fs::create_dir_all(&target).unwrap();
    let run_in = |dir: &PathBuf, args: &[&str]| {
        let st = Command::new("git").current_dir(dir).args(args).status().unwrap();
        assert!(st.success(), "git {:?} in {:?} should succeed", args, dir);
    };
    run_in(&target, &["init", "--quiet"]);
    run_in(&target, &["remote", "add", "origin", target.to_string_lossy().as_ref()]);

    let service = DefaultGitService::new();
    let flag = AtomicBool::new(true); // 立即取消
    let out = service.fetch_blocking(
        "", // 使用默认远程
        &target,
        &flag,
        |_p| {},
    );
    assert!(matches!(out, Err(e) if matches!(e, fireworks_collaboration_lib::core::git::errors::GitError::Categorized { category: fireworks_collaboration_lib::core::git::errors::ErrorCategory::Cancel, .. })), "cancel should map to Cancel category");
}

#[test]
fn fetch_updates_remote_tracking_refs() {
    use std::process::Command;
    // 1) 创建源仓库并提交一条记录
    let src = unique_temp_dir();
    std::fs::create_dir_all(&src).unwrap();
    let run_src = |args: &[&str]| {
        let st = Command::new("git").current_dir(&src).args(args).status().unwrap();
        assert!(st.success(), "git {:?} (src) should succeed", args);
    };
    run_src(&["init", "--quiet"]);
    run_src(&["config", "user.email", "you@example.com"]);
    run_src(&["config", "user.name", "You"]);
    std::fs::write(src.join("f.txt"), "1").unwrap();
    run_src(&["add", "."]);
    run_src(&["commit", "-m", "c1"]);

    // 2) 使用 git 克隆到目标（确保配置了默认 refspec 与 origin 远程）
    let dst = unique_temp_dir();
    let st = Command::new("git").args(["clone", "--quiet", src.to_string_lossy().as_ref(), dst.to_string_lossy().as_ref()]).status().expect("git clone");
    assert!(st.success(), "initial clone should succeed");

    // 3) 在源仓库新增一次提交
    std::fs::write(src.join("f.txt"), "2").unwrap();
    run_src(&["add", "."]);
    run_src(&["commit", "-m", "c2"]);
    let src_head = {
        let out = Command::new("git").current_dir(&src).args(["rev-parse", "HEAD"]).output().unwrap();
        assert!(out.status.success(), "get src HEAD");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };

    // 4) 在目标仓库调用 DefaultGitService::fetch_blocking
    let service = DefaultGitService::new();
    let flag = AtomicBool::new(false);
    let got = service.fetch_blocking("", &dst, &flag, |_p| {});
    assert!(got.is_ok(), "fetch should succeed");

    // 5) 验证远程跟踪分支已更新到源 HEAD
    let dst_remote_head = {
        let out = Command::new("git").current_dir(&dst).args(["rev-parse", "refs/remotes/origin/master"]).output().unwrap();
        assert!(out.status.success(), "get dst remote head");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert_eq!(dst_remote_head, src_head, "remote-tracking ref should match source HEAD after fetch");
}
