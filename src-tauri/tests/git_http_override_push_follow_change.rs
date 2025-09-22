use std::sync::Arc;
use fireworks_collaboration_lib::events::emitter::{peek_captured_events, AppHandle};
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::tasks::model::TaskState;

#[test]
fn git_push_http_override_follow_change_triggers_event() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
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
        for _ in 0..160 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed|TaskState::Failed) { break; } } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }

    #[derive(serde::Deserialize)] struct Outer { code: Option<String>, #[serde(rename="taskId")] task_id: Option<uuid::Uuid> }
        let events = peek_captured_events();
        let mut count=0;
    for (topic,payload) in events { if topic=="task://error" { if let Ok(o)=serde_json::from_str::<Outer>(&payload) { if o.task_id==Some(id) && o.code.as_deref()==Some("http_strategy_override_applied") { count+=1; } } } }
        assert_eq!(count, 1, "expected exactly one http_strategy_override_applied event (summary not counted)");
    });
}
