#![cfg(not(feature = "tauri-app"))]
//! 基线：无 filter/无 depth 情况不应产生 fallback 提示
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus, get_global_memory_bus, Event, TransportEvent};

fn unique_dir(label:&str)->PathBuf { std::env::temp_dir().join(format!("fwc-partial-base-{}-{}", label, uuid::Uuid::new_v4())) }

fn build_origin()->PathBuf {
	let dir = unique_dir("origin");
	std::fs::create_dir_all(&dir).unwrap();
	let run = |args:&[&str]|{ let st=Command::new("git").current_dir(&dir).args(args).status().unwrap(); assert!(st.success()); };
	run(&["init","--quiet"]); run(&["config","user.email","a@example.com"]); run(&["config","user.name","A"]);
	for i in 1..=2 { std::fs::write(dir.join(format!("f{}.txt", i)), i.to_string()).unwrap(); run(&["add","."]); run(&["commit","-m", &format!("c{}", i)]); }
	dir
}

#[tokio::test]
async fn no_filter_no_fallback_event() {
	let origin = build_origin();
	let dest = unique_dir("clone");
	let registry = Arc::new(TaskRegistry::new());
	let (id, token) = registry.create(TaskKind::GitClone { repo: origin.to_string_lossy().to_string(), dest: dest.to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
	let app = AppHandle;
	let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
	let handle = registry.spawn_git_clone_task_with_opts(Some(app), id, token, origin.to_string_lossy().to_string(), dest.to_string_lossy().to_string(), None, None, None);
	let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&registry,&id,50,160).await; // 50ms*160≈8s
	let snap = registry.snapshot(&id).unwrap(); assert_eq!(snap.state, TaskState::Completed, "clone should complete");
	handle.await.unwrap();
	// 结构化断言：不应出现 TransportEvent::PartialFilterFallback
	let mut attempts=0; let mut seen=false; while attempts<6 { if let Some(bus)=get_global_memory_bus() { if bus.snapshot().iter().any(|e| matches!(e, Event::Transport(TransportEvent::PartialFilterFallback { .. }))) { seen=true; break; } } if attempts<5 { tokio::time::sleep(std::time::Duration::from_millis(40)).await; } attempts+=1; }
	assert!(!seen, "should not emit structured fallback event when no filter provided");
}
