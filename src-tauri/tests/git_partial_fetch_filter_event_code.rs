#![cfg(not(feature = "tauri-app"))]
//! 验证 fetch filter 回退事件包含结构化 code 字段 partial_filter_fallback
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-code-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin()->(PathBuf, PathBuf) {
  let origin = unique_dir("origin"); std::fs::create_dir_all(&origin).unwrap();
  let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&origin).args(a).status().unwrap().success());};
  run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
  std::fs::write(origin.join("f.txt"),"1").unwrap(); run(&["add","."]); run(&["commit","-m","c1"]);
  let work = unique_dir("work"); assert!(Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap().success());
  // 添加一个新提交以确保 fetch 有变化
  std::fs::write(origin.join("f2.txt"),"2").unwrap(); run(&["add","f2.txt"]); run(&["commit","-m","c2"]);
  (work, origin)
}

#[tokio::test]
async fn fetch_filter_fallback_has_code() {
  let (work, origin) = build_origin();
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: None, filter: Some("tree:0".into()), strategy_override: None });
  let app = AppHandle; let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, None, Some("tree:0".into()), None);
  let mut waited=0; while waited<6000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  let mut found=false; let mut attempts=0; while attempts<20 && !found { let evs=peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && json.contains("partial_filter_fallback") && json.contains("fallback=full") { found=true; break; } } if !found { tokio::time::sleep(std::time::Duration::from_millis(40)).await; } attempts+=1; }
  assert!(found, "expected fetch fallback error with code partial_filter_fallback");
}
