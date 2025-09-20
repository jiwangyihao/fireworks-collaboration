use std::fs;
// 库名在 Cargo.toml [lib] 中为 fireworks_collaboration_lib
use fireworks_collaboration_lib::events::emitter::peek_captured_events;
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn git_clone_http_override_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    // Build a bare local source repo and then clone from it with override
    let tmp_src = tempfile::tempdir().unwrap();
    // init source git repository by spawning git via library existing helpers (reuse existing helper if present) – fallback: create .git by `git2` directly
    let repo = git2::Repository::init(tmp_src.path()).unwrap();
    // create one commit
    let file_path = tmp_src.path().join("a.txt");
    fs::write(&file_path, "hello").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

    // destination
    let dest = tempfile::tempdir().unwrap();

    // Build registry
    let reg = Arc::new(TaskRegistry::new());
    let override_json = serde_json::json!({"http": {"followRedirects": false, "maxRedirects": 2}});
    let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(override_json.clone()) });
    let app = AppHandle; // non-tauri 占位句柄
    let handle = reg.clone().spawn_git_clone_task_with_opts(Some(app), id, token, tmp_src.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(override_json));

    // Wait for completion (poll snapshot)
    for _ in 0..100 { if let Some(s) = reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(50)).await; }
    handle.await.unwrap();

    let events = peek_captured_events();
    let mut found = false;
    for (topic, payload) in events { if topic=="task://error" && payload.contains("http_strategy_override_applied") { found=true; break; } }
    assert!(found, "expected http_strategy_override_applied event");
    });
}
