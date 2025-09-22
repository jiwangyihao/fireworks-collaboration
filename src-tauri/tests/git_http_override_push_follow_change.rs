use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
// TaskState 间接通过等待 helper 使用
use fireworks_collaboration_lib::tests_support::wait::wait_task_terminal;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tests_support::event_assert::assert_applied_code;

#[test]
fn git_push_http_override_follow_change_triggers_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // init repo with commits to push
        let local = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(local.path()).unwrap();
        std::fs::write(local.path().join("a.txt"), "v1").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig,"c1", &tree, &[]).unwrap();

        // bare remote repo
        let remote = tempfile::tempdir().unwrap();
        git2::Repository::init_bare(remote.path()).unwrap();
        // add remote manually via git2
        repo.remote("origin", remote.path().to_string_lossy().as_ref()).unwrap();

        // create registry and run push with only follow changed
        let reg = Arc::new(TaskRegistry::new());
        let override_json = serde_json::json!({"http": {"followRedirects": false}});
        let (id, token) = reg.create(TaskKind::GitPush { dest: local.path().to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: None, username: None, password: None, strategy_override: Some(override_json.clone()) });
        let app = AppHandle; reg.clone().spawn_git_push_task(Some(app), id, token, local.path().to_string_lossy().to_string(), Some("origin".into()), None, None, None, Some(override_json));

        // wait
    let _ = wait_task_terminal(&reg, &id, 25, 160).await;
        assert_applied_code(&id.to_string(), "http_strategy_override_applied");
    });
}
