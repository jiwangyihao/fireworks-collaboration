use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::{peek_captured_events, AppHandle};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn git_fetch_http_override_only_max_triggers_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // create source repo with one commit
        let src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(src.path()).unwrap();
        std::fs::write(src.path().join("a.txt"), "one").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("a.txt")).unwrap(); index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();

        // clone to a working repo first so that subsequent fetch has a remote reference
        let work = tempfile::tempdir().unwrap();
        {
            let reg = Arc::new(TaskRegistry::new());
            let (cid, ctoken) = reg.create(TaskKind::GitClone { repo: src.path().to_string_lossy().to_string(), dest: work.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
            let app = AppHandle; reg.clone().spawn_git_clone_task_with_opts(Some(app), cid, ctoken, src.path().to_string_lossy().to_string(), work.path().to_string_lossy().to_string(), None, None, None);
            for _ in 0..120 { if let Some(s)=reg.snapshot(&cid) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }
        }

        // now run fetch with only maxRedirects changed (global max=5 -> override=2)
        let reg = Arc::new(TaskRegistry::new());
        let override_json = serde_json::json!({"http": {"maxRedirects": 2}});
    let (id, token) = reg.create(TaskKind::Sleep { ms: 5 });
    let app = AppHandle; reg.clone().spawn_git_fetch_task_with_opts(Some(app), id, token, src.path().to_string_lossy().to_string(), work.path().to_string_lossy().to_string(), None, None, None, Some(override_json));
        for _ in 0..160 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }

        let events = peek_captured_events();
        let mut found=false; let mut count=0;
    for (topic,payload) in events { if topic=="task://error" && payload.contains("\"code\":\"http_strategy_override_applied\"") && payload.contains(&id.to_string()) { found=true; count+=1; } }
        assert!(found, "expected override event for fetch only max change");
        assert_eq!(count, 1, "expected single event emission");
    });
}
