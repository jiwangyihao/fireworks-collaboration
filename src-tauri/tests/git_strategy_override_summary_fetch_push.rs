#![cfg(not(feature = "tauri-app"))]
//! 验证 GitFetch / GitPush 也产生 strategy_override_summary 事件，并包含 appliedCodes。

use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, Event, StrategyEvent, get_global_memory_bus};
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::TaskKind;
mod support;

use std::sync::{OnceLock, Mutex, MutexGuard};

fn capture_guard() -> MutexGuard<'static, ()> {
    static G: OnceLock<Mutex<()>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(dir.path()).unwrap();
    std::fs::write(dir.path().join("a.txt"), "one").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("a.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    dir
}

#[tokio::test]
async fn fetch_summary_event_and_applied_codes() {
    let _g = capture_guard();
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let origin_dir = init_repo();
    // 创建工作仓库并添加远程
    let work_dir = tempfile::tempdir().unwrap();
    let work_repo = git2::Repository::init(work_dir.path()).unwrap();
    work_repo.remote("origin", origin_dir.path().to_string_lossy().as_ref()).unwrap();

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let repo_url = origin_dir.path().to_string_lossy().to_string();
    let dest_path = work_dir.path().to_string_lossy().to_string();
    // fetch TaskKind 需要 repo+dest
    let (id, token) = reg.create(TaskKind::GitFetch { repo: repo_url.clone(), dest: dest_path.clone(), depth: None, filter: None, strategy_override: None });
    let override_json = serde_json::json!({"retry": {"max": 4}});
    let handle = reg.spawn_git_fetch_task_with_opts(Some(AppHandle {}), id, token.clone(), repo_url, dest_path, None, None, None, Some(override_json));
    let _ = handle.await;

    // 轮询等待结构化 summary 事件出现
    let mut found=false; let mut attempts=0; while attempts<30 && !found { if attempts>0 { tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        if let Some(bus)=get_global_memory_bus(){ for e in bus.snapshot(){ if let Event::Strategy(StrategyEvent::Summary{ id: sid, applied_codes, .. })=e { if sid==id.to_string() && applied_codes.iter().any(|c| c=="retry_strategy_override_applied") { found=true; break; } } } }
        attempts+=1;
    }
    assert!(found, "expected structured StrategyEvent::Summary with retry code");
}

// 验证 push 任务会同时产生独立 HttpApplied/TlsApplied 事件与汇总 Summary（包含 appliedCodes）。
#[tokio::test]
async fn push_summary_event_with_independent_applied_events() {
    let _g = capture_guard();
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    // 初始化一个包含提交的仓库并克隆到另一个目录作为推送来源
    let origin_dir = init_repo();
    let src_clone = tempfile::tempdir().unwrap();
    // 简单用 system git 克隆太重；这里直接再次 init 并添加 remote 指向 origin，然后 push 默认分支
    // 创建源仓库（带不同内容便于 push）
    let src_repo = git2::Repository::init(src_clone.path()).unwrap();
    std::fs::write(src_clone.path().join("b.txt"), "two").unwrap();
    let mut idx = src_repo.index().unwrap(); idx.add_path(std::path::Path::new("b.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = src_repo.find_tree(tree_id).unwrap(); let sig = src_repo.signature().unwrap(); src_repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    src_repo.remote("origin", origin_dir.path().to_string_lossy().as_ref()).unwrap();

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (pid, ptoken) = reg.create(TaskKind::GitPush { dest: src_clone.path().to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: None, username: None, password: None, strategy_override: None });
    let override_json = serde_json::json!({"http": {"followRedirects": false}, "tls": {"insecureSkipVerify": true}});
    let handle = reg.spawn_git_push_task(Some(AppHandle {}), pid, ptoken.clone(), src_clone.path().to_string_lossy().to_string(), Some("origin".into()), None, None, None, Some(override_json));
    let _ = handle.await;
    let mut found_summary=false; let mut attempts=0; let mut summary_http=false; let mut summary_tls=false; let mut saw_http_event=false; let mut saw_tls_event=false;
    while attempts<30 && !(found_summary && saw_http_event && saw_tls_event) { if attempts>0 { tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
        if let Some(bus)=get_global_memory_bus(){ for e in bus.snapshot(){ match e { Event::Strategy(StrategyEvent::Summary { id: sid, applied_codes, .. }) if sid==pid.to_string() => {
                    summary_http = applied_codes.iter().any(|c| c=="http_strategy_override_applied");
                    summary_tls = applied_codes.iter().any(|c| c=="tls_strategy_override_applied");
                    found_summary = true;
                }, Event::Strategy(StrategyEvent::HttpApplied{ id: sid, .. }) if sid==pid.to_string() => { saw_http_event=true; },
                Event::Strategy(StrategyEvent::TlsApplied{ id: sid, .. }) if sid==pid.to_string() => { saw_tls_event=true; },
                _=>{} } } }
        attempts+=1;
    }
    assert!(found_summary && summary_http && summary_tls, "expected summary with http+tls applied codes");
    assert!(saw_http_event && saw_tls_event, "expected independent HttpApplied & TlsApplied events");
}

#[tokio::test]
async fn fetch_summary_event_no_override() {
    let _g = capture_guard();
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let origin_dir = init_repo();
    // 创建工作仓库并添加远程
    let work_dir = tempfile::tempdir().unwrap();
    let work_repo = git2::Repository::init(work_dir.path()).unwrap();
    work_repo.remote("origin", origin_dir.path().to_string_lossy().as_ref()).unwrap();

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let repo_url = origin_dir.path().to_string_lossy().to_string();
    let dest_path = work_dir.path().to_string_lossy().to_string();
    let (id, token) = reg.create(TaskKind::GitFetch { repo: repo_url.clone(), dest: dest_path.clone(), depth: None, filter: None, strategy_override: None });
    let handle = reg.spawn_git_fetch_task_with_opts(Some(AppHandle {}), id, token.clone(), repo_url, dest_path, None, None, None, None);
    let _ = handle.await;
    let mut found=false; let mut attempts=0; let mut empty_codes=false; while attempts<30 && !found { if attempts>0 { tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
        if let Some(bus)=get_global_memory_bus(){ for e in bus.snapshot(){ if let Event::Strategy(StrategyEvent::Summary { id: sid, applied_codes, .. })=e { if sid==id.to_string() { empty_codes = applied_codes.is_empty(); found=true; break; } } } }
        attempts+=1;
    }
    assert!(found && empty_codes, "expected summary with empty appliedCodes");
}

#[tokio::test]
async fn push_summary_event_no_override() {
    let _g = capture_guard();
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let origin_dir = init_repo();
    let src_clone = tempfile::tempdir().unwrap();
    let src_repo = git2::Repository::init(src_clone.path()).unwrap();
    std::fs::write(src_clone.path().join("c.txt"), "three").unwrap();
    let mut idx = src_repo.index().unwrap(); idx.add_path(std::path::Path::new("c.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = src_repo.find_tree(tree_id).unwrap(); let sig = src_repo.signature().unwrap(); src_repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    src_repo.remote("origin", origin_dir.path().to_string_lossy().as_ref()).unwrap();

    let reg = std::sync::Arc::new(TaskRegistry::new());
    let (pid, ptoken) = reg.create(TaskKind::GitPush { dest: src_clone.path().to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: None, username: None, password: None, strategy_override: None });
    let handle = reg.spawn_git_push_task(Some(AppHandle {}), pid, ptoken.clone(), src_clone.path().to_string_lossy().to_string(), Some("origin".into()), None, None, None, None);
    let _ = handle.await;
    let mut found=false; let mut attempts=0; let mut empty_codes=false; while attempts<30 && !found { if attempts>0 { tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
        if let Some(bus)=get_global_memory_bus(){ for e in bus.snapshot(){ if let Event::Strategy(StrategyEvent::Summary { id: sid, applied_codes, .. })=e { if sid==pid.to_string() { empty_codes = applied_codes.is_empty(); found=true; break; } } } }
        attempts+=1;
    }
    assert!(found && empty_codes, "expected push summary with empty appliedCodes");
}
