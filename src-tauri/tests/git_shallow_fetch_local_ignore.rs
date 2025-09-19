#![cfg(not(feature = "tauri-app"))]
//! Local fetch depth ignore test
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::process::Command;
use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_dir(p: &str) -> PathBuf { std::env::temp_dir().join(format!("fwc-shallow-fetch-local-{}-{}", p, uuid::Uuid::new_v4())) }

fn build_repo_with_commits(n: u32) -> PathBuf {
    let dir = unique_dir("repo");
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| { let st = Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success(), "git {:?} failed", args); };
    run(&["init", "--quiet"]);
    run(&["config", "user.email", "a@example.com"]);
    run(&["config", "user.name", "A"]);
    for i in 1..=n { std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap(); run(&["add", "."]); run(&["commit", "-m", &format!("c{}", i)]); }
    dir
}

#[test]
fn local_fetch_depth_ignored() {
    // Prepare origin with several commits
    let origin = build_repo_with_commits(3);
    // Clone full repo first
    let dest = unique_dir("clone");
    let flag_clone = AtomicBool::new(false);
    let svc = DefaultGitService::new();
    svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, None, &flag_clone, |_p| {}).expect("full clone");

    // Perform fetch with depth=1 referencing local path (should be ignored silently per design) using explicit remote URL path
    let flag_fetch = AtomicBool::new(false);
    let fetch_res = svc.fetch_blocking(origin.to_string_lossy().as_ref(), &dest, Some(1), &flag_fetch, |_p| {});
    assert!(fetch_res.is_ok(), "local fetch with depth should succeed");

    // Validate no .git/shallow created
    let shallow_file = dest.join(".git").join("shallow");
    assert!(!shallow_file.exists(), "local fetch should ignore depth and not create shallow file");
}
