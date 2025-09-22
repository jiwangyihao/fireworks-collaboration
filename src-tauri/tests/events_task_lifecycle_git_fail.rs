use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, Event, TaskEvent, EventBusAny};
use fireworks_collaboration_lib::events::emitter::AppHandle;

// 构造一个有效的本地 git 仓库（供 fetch 失败场景中的 repo 源）
fn make_local_repo() -> tempfile::TempDir {
    let td = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(td.path()).unwrap();
    std::fs::write(td.path().join("a.txt"), b"hello").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("a.txt")).unwrap();
    idx.write().unwrap();
    let tree_id = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = repo.signature().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
    td
}

#[tokio::test]
async fn git_clone_lifecycle_failed_invalid_repo_with_apphandle() {
    // 使用一个不存在的路径作为 repo 源，触发 clone 失败
    let invalid_repo_path = "Z:/definitely/not/exist/clone-src"; // 在多数环境下不存在
    let dest_td = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitClone { repo: invalid_repo_path.into(), dest: dest_td.path().to_string_lossy().into(), depth: None, filter: None, strategy_override: None });
    let handle = reg.spawn_git_clone_task_with_opts(Some(AppHandle), id, token, invalid_repo_path.into(), dest_td.path().to_string_lossy().into(), None, None, None);
    handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut failed_cnt=0; let mut canceled=false;
    for e in events { if let Event::Task(t)=e { match t { TaskEvent::Started { id:sid, kind } if sid==id.to_string() && kind=="GitClone" => started=true,
        TaskEvent::Completed { id:sid } if sid==id.to_string()=>completed=true,
        TaskEvent::Failed { id:sid, .. } if sid==id.to_string()=>{ failed_cnt+=1; },
        TaskEvent::Canceled { id:sid } if sid==id.to_string()=>canceled=true,
        _=>{} } } }
    assert!(started, "clone should emit Started even on failure");
    assert_eq!(failed_cnt, 1, "clone failed should emit exactly one Failed event");
    assert!(!completed && !canceled, "clone invalid repo should not complete nor cancel");
}

#[tokio::test]
async fn git_fetch_lifecycle_failed_invalid_dest_with_apphandle() {
    // 准备一个源 repo，但 dest 目录保持空目录未 init，使 fetch 必然失败
    let src_repo = make_local_repo();
    let empty_dest = tempfile::tempdir().unwrap(); // 未 init
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitFetch { repo: src_repo.path().to_string_lossy().into(), dest: empty_dest.path().to_string_lossy().into(), depth: None, filter: None, strategy_override: None });
    let handle = reg.spawn_git_fetch_task_with_opts(Some(AppHandle), id, token, src_repo.path().to_string_lossy().into(), empty_dest.path().to_string_lossy().into(), None, None, None, None);
    handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut failed_cnt=0; let mut canceled=false;
    for e in events { if let Event::Task(t)=e { match t { TaskEvent::Started { id:sid, kind } if sid==id.to_string() && kind=="GitFetch" => started=true,
        TaskEvent::Completed { id:sid } if sid==id.to_string()=>completed=true,
        TaskEvent::Failed { id:sid, .. } if sid==id.to_string()=>{ failed_cnt+=1; },
        TaskEvent::Canceled { id:sid } if sid==id.to_string()=>canceled=true,
        _=>{} } } }
    assert!(started, "fetch should emit Started even on failure");
    assert_eq!(failed_cnt, 1, "fetch failed should emit exactly one Failed event");
    assert!(!completed && !canceled, "fetch invalid dest should not complete nor cancel");
}
