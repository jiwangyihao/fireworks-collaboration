use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, Event, TaskEvent, EventBusAny};

fn make_local_repo() -> tempfile::TempDir {
    let td = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(td.path()).unwrap();
    // one commit
    std::fs::write(td.path().join("a.txt"), b"hello").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("a.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    // 归还 tempdir；repo drop 后仍保留磁盘内容供克隆
    td
}

#[tokio::test]
async fn git_clone_lifecycle_started_completed() {
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);

    let src_td = make_local_repo();
    let dest_td = tempfile::tempdir().unwrap();

    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitClone { repo: src_td.path().to_string_lossy().into(), dest: dest_td.path().to_string_lossy().into(), depth: None, filter: None, strategy_override: None });
    let handle = reg.spawn_git_clone_task_with_opts(None, id, token, src_td.path().to_string_lossy().into(), dest_td.path().to_string_lossy().into(), None, None, None);
    handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut failed=false; let mut canceled=false;
    for e in events { if let Event::Task(t) = e { match t { TaskEvent::Started { id: sid, kind } if sid==id.to_string() && kind=="GitClone" => started=true, TaskEvent::Completed { id: sid } if sid==id.to_string()=>completed=true, TaskEvent::Failed { id: sid, .. } if sid==id.to_string()=>failed=true, TaskEvent::Canceled { id:sid } if sid==id.to_string()=>canceled=true, _=>{} } } }
    assert!(started, "clone should emit Started");
    assert!(completed, "clone should emit Completed");
    assert!(!failed && !canceled, "clone should not fail or cancel");
}

#[tokio::test]
async fn git_fetch_lifecycle_cancel() {
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);

    let src_td = make_local_repo();
    let dest_td = tempfile::tempdir().unwrap();

    // 先克隆一次作为 fetch 基础
    {
        let (cid, ctoken) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitClone { repo: src_td.path().to_string_lossy().into(), dest: dest_td.path().to_string_lossy().into(), depth: None, filter: None, strategy_override: None });
        let h = reg.spawn_git_clone_task_with_opts(None, cid, ctoken, src_td.path().to_string_lossy().into(), dest_td.path().to_string_lossy().into(), None, None, None);
        h.await.unwrap();
    }

    // 触发 fetch 并立即取消
    let (fid, ftoken) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitFetch { repo: src_td.path().to_string_lossy().into(), dest: dest_td.path().to_string_lossy().into(), depth: None, filter: None, strategy_override: None });
    let fetch_handle = reg.spawn_git_fetch_task_with_opts(None, fid, ftoken.clone(), src_td.path().to_string_lossy().into(), dest_td.path().to_string_lossy().into(), None, None, None, None);
    // 立即取消
    ftoken.cancel();
    fetch_handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut canceled=false; let mut completed=false; let mut failed=false;
    for e in events { if let Event::Task(t) = e { match t { TaskEvent::Started { id: sid, kind } if sid==fid.to_string() && kind=="GitFetch" => started=true, TaskEvent::Canceled { id:sid } if sid==fid.to_string()=>canceled=true, TaskEvent::Completed { id:sid } if sid==fid.to_string()=>completed=true, TaskEvent::Failed { id:sid, .. } if sid==fid.to_string()=>failed=true, _=>{} } } }
    assert!(started, "fetch should emit Started");
    assert!(canceled, "fetch should emit Canceled when token canceled");
    assert!(!completed && !failed, "fetch should neither complete nor fail when canceled early");
}
