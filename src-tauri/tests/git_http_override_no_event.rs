use fireworks_collaboration_lib::events::emitter::peek_captured_events;
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn git_clone_http_override_no_event_when_same_values() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    // source repo
    let src = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(src.path()).unwrap();
    let fp = src.path().join("a.txt");
    std::fs::write(&fp, "hi").unwrap();
    let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
    let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig,"c1", &tree, &[]).unwrap();
    let dest = tempfile::tempdir().unwrap();

    let reg = Arc::new(TaskRegistry::new());
    // global defaults follow=true max=5 ; override with same values -> no event
    let override_json = serde_json::json!({"http":{"followRedirects": true, "maxRedirects":5}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle;
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

    for _ in 0..100 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(40)).await; }
    handle.await.unwrap();

    let events = peek_captured_events();
    let mut found=false;
    for (topic,payload) in events { if topic=="task://error" && payload.contains("http_strategy_override_applied") { found=true; break; } }
    assert!(!found, "should not emit http_strategy_override_applied when values unchanged");
    });
}
