use std::fs; use std::sync::Arc; use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind}; use fireworks_collaboration_lib::events::emitter::{AppHandle, peek_captured_events}; use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn combo_conflict_and_ignored_http_tls() {
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async {
    // build repo
    let tmp_src = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp_src.path()).unwrap();
    let fp = tmp_src.path().join("a.txt"); fs::write(&fp, "combo").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
    let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

    let dest = tempfile::tempdir().unwrap();
    // override 包含： http 冲突 (follow=false max=7) + tls 冲突 (insecure=true skipSan=true) + 未知字段 top & nested
    let override_json = serde_json::json!({
      "http": {"followRedirects": false, "maxRedirects": 7, "AAA": 1},
      "tls": {"insecureSkipVerify": true, "skipSanWhitelist": true, "BBB": 2},
      "retry": {"max": 3},
      "extraTop": {"x": 1}
    });
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

    for _ in 0..100 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
    handle.await.unwrap();

    let ev = peek_captured_events();
    let mut http_conflict=0; let mut tls_conflict=0; let mut ignored=0; let mut http_applied=0; let mut tls_applied=0;
    for (topic,p) in ev { if topic=="task://error" && p.contains(&id.to_string()) {
        if p.contains("http_strategy_override_applied") { http_applied+=1; }
        if p.contains("tls_strategy_override_applied") { tls_applied+=1; }
        if p.contains("strategy_override_conflict") && p.contains("http conflict") { http_conflict+=1; }
        if p.contains("strategy_override_conflict") && p.contains("tls conflict") { tls_conflict+=1; }
        if p.contains("strategy_override_ignored_fields") { ignored+=1; }
    }}
    assert_eq!(http_applied,1); assert_eq!(tls_applied,1);
    assert_eq!(http_conflict,1, "expect one http conflict event");
    assert_eq!(tls_conflict,1, "expect one tls conflict event");
    assert_eq!(ignored,1, "expect one ignored fields event");
  });
}
