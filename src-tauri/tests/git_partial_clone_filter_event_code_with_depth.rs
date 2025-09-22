#![cfg(not(feature = "tauri-app"))]
//! 验证 clone depth+filter 回退事件 code=partial_filter_fallback 且为 shallow 提示
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, Event, TransportEvent, get_global_memory_bus};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-code-depth-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); for i in 1..=2 { std::fs::write(dir.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); } dir }

#[tokio::test]
async fn clone_depth_filter_fallback_has_shallow_code() {
  let origin = build_origin();
  let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: Some(1), filter: Some("tree:0".into()), strategy_override: None });
  let app = AppHandle;
  let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
  let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), Some(serde_json::json!(1)), Some("tree:0".into()), None);
  let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,60,100).await;
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  let mut found=false; let mut attempts=0; while attempts<20 && !found { if attempts>0 { tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    if let Some(bus)=get_global_memory_bus(){ for e in bus.snapshot(){ if let Event::Transport(TransportEvent::PartialFilterFallback{shallow:true,..})=e { found=true; break; } } }
    attempts+=1;
  }
  assert!(found, "expected structured shallow fallback transport event for depth+filter clone");
}
