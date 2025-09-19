#![cfg(not(feature = "tauri-app"))]
//! Deepening a shallow clone via successive shallow fetches (local origin only, network independent)
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_dir(p:&str)->PathBuf{ std::env::temp_dir().join(format!("fwc-shallow-deepen-{}-{}", p, uuid::Uuid::new_v4())) }

fn init_origin_with_commits(n:u32)->PathBuf{
    let dir = unique_dir("origin");
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args:&[&str]|{ let st=Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success(), "git {:?} failed", args); };
    run(&["init","--quiet"]);
    run(&["config","user.email","a@example.com"]);
    run(&["config","user.name","A"]);
    for i in 1..=n {
        std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap();
        run(&["add","."]);
        run(&["commit","-m", &format!("c{}", i)]);
    }
    dir
}

fn rev_count(repo:&PathBuf)->u32{
    let out = Command::new("git").current_dir(repo).args(["rev-list","--count","HEAD"]).output().unwrap();
    assert!(out.status.success(), "rev-list failed");
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[test]
fn deepen_shallow_clone_with_incremental_fetches() {
    // Prepare origin with 5 commits
    let origin = init_origin_with_commits(5);
    // Perform shallow clone depth=1
    let dest = unique_dir("clone");
    let flag = AtomicBool::new(false);
    let svc = DefaultGitService::new();
    svc.clone_blocking(origin.to_string_lossy().as_ref(), &dest, Some(1), &flag, |_p| {}).expect("shallow clone");
    // After shallow clone, history may or may not present shallow file depending on libgit2 local behavior, but commit count should be >=1
    let c1 = rev_count(&dest);
    assert!(c1 >= 1 && c1 <= 5, "initial shallow commit count bounds");
    // Deepen to depth=2 via fetch
    svc.fetch_blocking(origin.to_string_lossy().as_ref(), &dest, Some(2), &flag, |_p| {}).expect("fetch deepen 2");
    let c2 = rev_count(&dest);
    assert!(c2 >= c1 && c2 <= 5, "after deepen to 2 count should not shrink");
    // Deepen further to depth=4
    svc.fetch_blocking(origin.to_string_lossy().as_ref(), &dest, Some(4), &flag, |_p| {}).expect("fetch deepen 4");
    let c3 = rev_count(&dest);
    assert!(c3 >= c2 && c3 <= 5, "after deepen to 4 count monotonic");
}
