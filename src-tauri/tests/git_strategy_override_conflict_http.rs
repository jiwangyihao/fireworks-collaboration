use std::fs; use std::sync::Arc; use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind}; use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events}; use fireworks_collaboration_lib::tasks::model::TaskState; 

#[test]
fn http_conflict_follow_false_max_positive() {
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async {
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
    for _ in 0..80 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
    handle.await.unwrap();
    let events = peek_captured_events();
    let mut conflict_evt=0; let mut applied_evt=0; let mut conflict_msg_ok=false;
    for (topic,p) in events { if topic=="task://error" && p.contains(&id.to_string()) { if p.contains("http_strategy_override_applied") { applied_evt+=1; } if p.contains("strategy_override_conflict") { conflict_evt+=1; if p.contains("force maxRedirects=0") { conflict_msg_ok=true; } } } }
    assert_eq!(applied_evt,1,"http applied event once");
    assert_eq!(conflict_evt,1,"conflict event once");
    assert!(conflict_msg_ok, "conflict message should mention force maxRedirects=0");
  });
}
