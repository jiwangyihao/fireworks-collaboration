use std::fs; use std::sync::Arc; use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind}; use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events}; use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn no_conflict_http_tls_override() {
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async {
    let tmp_src = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp_src.path()).unwrap();
    let fp = tmp_src.path().join("z.txt"); fs::write(&fp, "nc").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("z.txt")).unwrap(); index.write().unwrap();
    let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

    let dest = tempfile::tempdir().unwrap();
    // already normalized values: follow=true + max=5 (no conflict), insecure=true + skipSan=false (no conflict)
    let override_json = serde_json::json!({
      "http": {"followRedirects": true, "maxRedirects": 5},
      "tls": {"insecureSkipVerify": true, "skipSanWhitelist": false}
    });
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle; let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));
    for _ in 0..100 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
    handle.await.unwrap();
    let ev = peek_captured_events();
    let mut conflict_found=false; let mut http_applied=0; let mut tls_applied=0;
    for (topic,p) in ev { if topic=="task://error" && p.contains(&id.to_string()) { if p.contains("strategy_override_conflict") { conflict_found=true; } if p.contains("http_strategy_override_applied") { http_applied+=1; } if p.contains("tls_strategy_override_applied") { tls_applied+=1; } } }
  assert_eq!(http_applied,0, "http values unchanged -> no http applied event");
  assert_eq!(tls_applied,1, "tls insecure changed -> one tls applied event");
  assert!(!conflict_found, "no conflict expected for normalized inputs");
  });
}
