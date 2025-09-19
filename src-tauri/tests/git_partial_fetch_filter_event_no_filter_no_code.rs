#![cfg(not(feature = "tauri-app"))]
//! 验证无 filter 情况下不应出现 partial_filter_fallback code
use std::path::PathBuf; use std::process::Command; use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events, drain_captured_events};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-fetch-nocode-{}-{}", label, uuid::Uuid::new_v4())) }
fn build_origin()->(PathBuf, PathBuf) { let origin = unique_dir("origin"); std::fs::create_dir_all(&origin).unwrap(); let run=|a:&[&str]|{assert!(Command::new("git").current_dir(&origin).args(a).status().unwrap().success());}; run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]); for i in 1..=2 { std::fs::write(origin.join(format!("f{}.txt",i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); } let work = unique_dir("work"); assert!(Command::new("git").args(["clone", origin.to_string_lossy().as_ref(), work.to_string_lossy().as_ref()]).status().unwrap().success()); std::fs::write(origin.join("new.txt"),"x").unwrap(); run(&["add","new.txt"]); run(&["commit","-m","c3"]); (work, origin) }

#[tokio::test]
async fn fetch_no_filter_has_no_fallback_code() {
  let (work,_origin) = build_origin();
  let registry = Arc::new(TaskRegistry::new());
  let (id, token) = registry.create(TaskKind::GitFetch { repo: "origin".into(), dest: work.to_string_lossy().to_string(), depth: Some(2), filter: None, strategy_override: None });
  let app = AppHandle; let handle = registry.spawn_git_fetch_task_with_opts(Some(app), id, token, "origin".into(), work.to_string_lossy().to_string(), None, Some(serde_json::json!(2)), None, None);
  let mut waited=0; while waited<6000 { if let Some(s)=registry.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed|TaskState::Canceled){ break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; waited+=50; }
  let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed);
  handle.await.unwrap();
  let mut attempts=0; let mut seen=false; while attempts<8 { let evs=peek_captured_events(); for (topic,json) in &evs { if topic=="task://error" && json.contains("partial_filter_fallback") { seen=true; break; } } if seen { break; } tokio::time::sleep(std::time::Duration::from_millis(40)).await; attempts+=1; }
  let _ = drain_captured_events();
  assert!(!seen, "should not see partial_filter_fallback code when filter absent");
}
