#![cfg(not(feature = "tauri-app"))]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::process::Command;

use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_dir(p: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fwc-shallow-local-{}-{}", p, uuid::Uuid::new_v4()))
}

// 创建一个包含多次提交的本地仓库（普通非裸）
fn build_source_repo() -> PathBuf {
    let src = unique_dir("src");
    std::fs::create_dir_all(&src).unwrap();
    let run = |args: &[&str]| {
        let st = Command::new("git").current_dir(&src).args(args).status().unwrap();
        assert!(st.success(), "git {:?} failed", args);
    };
    run(&["init", "--quiet"]);
    run(&["config", "user.email", "you@example.com"]);
    run(&["config", "user.name", "You"]);
    for i in 1..=3 { // 3 次提交
        std::fs::write(src.join(format!("f{}.txt", i)), format!("{}", i)).unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", &format!("c{}", i)]);
    }
    src
}

#[test]
fn local_clone_depth_ignored_and_not_shallow() {
    let src = build_source_repo();
    // 准备目标目录
    let dest = unique_dir("dest");
    let flag = AtomicBool::new(false);
    let svc = DefaultGitService::new();
    // 指定 depth=1（应被忽略）
    let out = svc.clone_blocking(src.to_string_lossy().as_ref(), &dest, Some(1), &flag, |_p| {});
    assert!(out.is_ok(), "local clone with depth should succeed");
    // 不应生成 .git/shallow
    let shallow_file = dest.join(".git").join("shallow");
    assert!(!shallow_file.exists(), "local clone should ignore depth and not create shallow file");
    // 提交历史应 >=3（验证确实不是被裁剪）
    let log_out = Command::new("git").current_dir(&dest).args(["rev-list", "--count", "HEAD"]).output().unwrap();
    assert!(log_out.status.success(), "git rev-list should succeed");
    let count: u32 = String::from_utf8_lossy(&log_out.stdout).trim().parse().unwrap();
    assert!(count >= 3, "expected full history (>=3 commits), got {}", count);
}
