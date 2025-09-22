use std::fs; use std::sync::Arc; use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind}; use fireworks_collaboration_lib::events::emitter::AppHandle; 
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::{assert_conflict_kind, assert_applied_code};

#[test]
fn http_conflict_follow_false_max_positive() {
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async {
    // 安装全局 MemoryEventBus 以捕获结构化事件
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let tmp_src = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp_src.path()).unwrap();
    let fp = tmp_src.path().join("a.txt"); fs::write(&fp, "hi").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
    let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    let dest = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    let override_json = serde_json::json!({"http": {"followRedirects": false, "maxRedirects": 5}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
  let _ = fireworks_collaboration_lib::tests_support::wait::wait_task_terminal(&reg, &id, 40, 80).await;
    handle.await.unwrap();
    // 结构化事件断言：Summary 应含 http_strategy_override_applied 代码；Conflict(kind=http) 存在且 message 含 force maxRedirects=0
    assert_applied_code(&id.to_string(), "http_strategy_override_applied");
    assert_conflict_kind(&id.to_string(), "http", Some("maxRedirects=0"));
  });
}
