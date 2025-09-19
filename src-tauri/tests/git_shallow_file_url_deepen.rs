#![cfg(not(feature = "tauri-app"))]
//! Shallow clone and deepen using a file:// URL form (exercise code path that treats non-local looking URL scheme)
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use fireworks_collaboration_lib::core::git::DefaultGitService;
use fireworks_collaboration_lib::core::git::service::GitService;

fn unique_dir(p:&str)->PathBuf{ std::env::temp_dir().join(format!("fwc-file-url-{}-{}", p, uuid::Uuid::new_v4())) }

fn init_origin_with_commits(n:u32)->PathBuf{
    let dir = unique_dir("origin");
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args:&[&str]|{ let st=Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success(), "git {:?} failed", args); };
    run(&["init","--quiet"]);
    run(&["config","user.email","a@example.com"]);
    run(&["config","user.name","A"]);
    for i in 1..=n { std::fs::write(dir.join(format!("f{}.txt", i)), format!("{}", i)).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
    dir
}

fn rev_count(repo:&PathBuf)->u32{ let out=Command::new("git").current_dir(repo).args(["rev-list","--count","HEAD"]).output().unwrap(); assert!(out.status.success(),"rev-list failed"); String::from_utf8_lossy(&out.stdout).trim().parse().unwrap() }

#[test]
#[ignore]
fn shallow_then_full_deepen_via_file_url(){
    // 忽略：当前实现不支持 file:// scheme；保留测试骨架以便未来支持时补充。
    let origin = init_origin_with_commits(4); // unreachable when ignored, kept for future enable
    // Build file:// URL (ensure absolute path)
    let origin_abs = origin.canonicalize().unwrap();
    let file_url = if cfg!(windows) { format!("file:///{}", origin_abs.to_string_lossy().replace('\\', "/")) } else { format!("file://{}", origin_abs.to_string_lossy()) };
    let dest = unique_dir("clone");
    let svc = DefaultGitService::new();
    let flag = AtomicBool::new(false);
    // Shallow depth=1
    let r1 = svc.clone_blocking(&file_url, &dest, Some(1), &flag, |_p| {});
    assert!(r1.is_ok(), "shallow file URL clone should succeed: {:?}", r1.err());
    let c1 = rev_count(&dest); assert!(c1 >=1 && c1 <=4, "c1 bounds");
    // Deepen to 2
    svc.fetch_blocking(&file_url, &dest, Some(2), &flag, |_p| {}).expect("deepen 2");
    let c2 = rev_count(&dest); assert!(c2 >= c1 && c2 <=4, "c2 monotonic");
    // Fetch with smaller depth=1 should not shrink (ignored or non-decreasing)
    svc.fetch_blocking(&file_url, &dest, Some(1), &flag, |_p| {}).expect("redundant small depth fetch");
    let c2b = rev_count(&dest); assert!(c2b >= c2, "smaller depth should not reduce history");
    // Full fetch (None) to reach max (may already be full depending on libgit2 local optimization)
    svc.fetch_blocking(&file_url, &dest, None, &flag, |_p| {}).expect("full fetch");
    let c_full = rev_count(&dest); assert!(c_full >= c2b && c_full <=4, "full deepen non-decreasing");
}
