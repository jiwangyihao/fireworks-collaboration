use std::fs; use std::sync::Arc; use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind}; use fireworks_collaboration_lib::events::emitter::AppHandle; use fireworks_collaboration_lib::tasks::model::TaskState; 
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_conflict_kind, assert_applied_code};

#[test]
fn tls_conflict_insecure_and_skip_san() {
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async {
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let tmp_src = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp_src.path()).unwrap();
    let fp = tmp_src.path().join("a.txt"); fs::write(&fp, "hi").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
    let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    let dest = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    let override_json = serde_json::json!({"tls": {"insecureSkipVerify": true, "skipSanWhitelist": true}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
    for _ in 0..80 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
    handle.await.unwrap();
    assert_applied_code(&id.to_string(), "tls_strategy_override_applied");
    // 冲突消息包含规范化 skipSanWhitelist=false
    assert_conflict_kind(&id.to_string(), "tls", Some("skipSanWhitelist=false"));
  });
}
