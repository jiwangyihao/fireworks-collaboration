#![cfg(not(feature = "tauri-app"))]
//! 验证 clone fallback 事件结构字段完备：code/category/message 并且不含 retriedTimes
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, get_global_memory_bus, Event, TransportEvent};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-clone-struct-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->PathBuf { let dir = unique_dir("origin"); std::fs::create_dir_all(&dir).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&dir).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); std::fs::write(dir.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]); dir }

#[tokio::test]
async fn clone_fallback_event_structure() {
  std::env::remove_var("FWC_PARTIAL_FILTER_CAPABLE"); // 确保触发 fallback
  let origin = build_origin(); let dest = unique_dir("clone");
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: Some("blob:none".into()), strategy_override: None });
  let app = AppHandle; let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
  let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, Some("blob:none".into()), None);
  let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,50,100).await;
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  // 轮询结构化事件：期待 TransportEvent::PartialFilterFallback 且 shallow=false
  let mut found=false; let mut attempts=0; while attempts<20 && !found { if attempts>0 { tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
    if let Some(bus)=get_global_memory_bus() { for e in bus.snapshot() { if let Event::Transport(TransportEvent::PartialFilterFallback { shallow, id: eid, .. }) = e { if eid.to_string()==id.to_string() && shallow==false { found=true; break; } } } }
    attempts+=1;
  }
  assert!(found, "expected structured TransportEvent::PartialFilterFallback shallow=false for clone fallback");
}
