#![cfg(not(feature = "tauri-app"))]
//! 验证仅 filter (无 depth) 情况下的回退事件：应出现 fallback=full 提示
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, get_global_memory_bus, Event, TransportEvent};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-only-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin()->PathBuf {
    let dir = unique_dir("origin");
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args:&[&str]|{ let st=Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success()); };
    run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
    for i in 1..=2 { std::fs::write(dir.join(format!("f{}.txt", i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
    dir
}

#[tokio::test]
async fn filter_only_fallback_full_message() {
    let origin = build_origin();
    let dest = unique_dir("clone");
    let registry = Arc::new(TaskRegistry::new());
    let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
    let app = AppHandle; // non-tauri capture
    // 设置全局事件总线用于结构化断言
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, Some("blob:none".into()), None);
    // 等待任务完成
    let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,50,160).await;
    let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "clone should complete");
    handle.await.unwrap();
    // 轮询结构化事件：期待 shallow=false（full 回退）
    let mut found=false; let mut attempts=0; while attempts<20 && !found { if let Some(bus)=get_global_memory_bus() { for e in bus.snapshot() { if let Event::Transport(TransportEvent::PartialFilterFallback { shallow, id: eid, .. }) = e { if eid.to_string()==id.to_string() && shallow==false { found=true; break; } } } } if !found { tokio::time::sleep(std::time::Duration::from_millis(50)).await; } attempts+=1; }
    assert!(found, "expected structured fallback (shallow=false) for filter-only clone");
}
