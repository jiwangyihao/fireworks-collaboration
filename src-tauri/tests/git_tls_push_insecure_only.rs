//! Push task with TLS insecureSkipVerify only override (P2.3d extra)
use std::sync::Arc;
use fireworks_collaboration_lib::tasks::{TaskRegistry, TaskKind};
use fireworks_collaboration_lib::events::emitter::AppHandle;
use fireworks_collaboration_lib::tests_support::event_assert::assert_tls_applied;
use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
use fireworks_collaboration_lib::tasks::model::TaskState;

async fn wait_done(reg:&TaskRegistry, id:uuid::Uuid){ for _ in 0..120 { if let Some(s)=reg.snapshot(&id) { if matches!(s.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) { break; } } tokio::time::sleep(std::time::Duration::from_millis(35)).await; } }

fn init_repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(tmp.path()).unwrap();
    let f = tmp.path().join("f.txt"); std::fs::write(&f, "1").unwrap();
    let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
    tmp
}

#[test]
fn push_tls_insecure_only_event_once() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
    let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
    let src = init_repo(); let src_path = src.path().to_string_lossy().to_string();
        let reg = Arc::new(TaskRegistry::new()); let app = AppHandle;
        let work = tempfile::tempdir().unwrap();
        let (id_clone, tk_clone) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let hc = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id_clone, tk_clone, src_path.clone(), work.path().to_string_lossy().to_string(), None, None, None);
    wait_done(&reg, id_clone).await; hc.await.unwrap();

        let over = serde_json::json!({"tls": {"insecureSkipVerify": true}});
        let (id_push, tk_push) = reg.create(TaskKind::GitPush { dest: work.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(over.clone()) });
        let hp = reg.clone().spawn_git_push_task(Some(app.clone()), id_push, tk_push, work.path().to_string_lossy().to_string(), None, None, None, None, Some(over));
        wait_done(&reg, id_push).await; hp.await.unwrap();
        assert_tls_applied(&id_push.to_string(), true);
    });
}
