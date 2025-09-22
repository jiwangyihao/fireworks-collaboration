use std::sync::Arc;
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, Event, TaskEvent, EventBusAny};
use fireworks_collaboration_lib::events::emitter::AppHandle;

fn init_local_with_remote() -> (tempfile::TempDir, tempfile::TempDir) {
    // remote bare repo
    let remote = tempfile::tempdir().unwrap();
    std::process::Command::new("git").args(["init","--bare","--quiet", remote.path().to_string_lossy().as_ref()]).status().unwrap();
    // work repo
    let work = tempfile::tempdir().unwrap();
    std::process::Command::new("git").args(["init","--quiet"]).current_dir(work.path()).status().unwrap();
    std::process::Command::new("git").args(["config","user.email","you@example.com"]).current_dir(work.path()).status().unwrap();
    std::process::Command::new("git").args(["config","user.name","You"]).current_dir(work.path()).status().unwrap();
    std::process::Command::new("git").args(["remote","add","origin", remote.path().to_string_lossy().as_ref()]).current_dir(work.path()).status().unwrap();
    std::fs::write(work.path().join("a.txt"), b"1").unwrap();
    std::process::Command::new("git").args(["add","."]).current_dir(work.path()).status().unwrap();
    std::process::Command::new("git").args(["commit","-m","c1"]).current_dir(work.path()).status().unwrap();
    (work, remote)
}

#[tokio::test]
async fn git_push_lifecycle_started_completed() {
    let (work, _remote) = init_local_with_remote();
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitPush { dest: work.path().to_string_lossy().into(), remote: Some("origin".into()), refspecs: Some(vec!["refs/heads/master:refs/heads/master".into()]), username: None, password: None, strategy_override: None });
    let handle = reg.spawn_git_push_task(None, id, token, work.path().to_string_lossy().into(), Some("origin".into()), Some(vec!["refs/heads/master:refs/heads/master".into()]), None, None, None);
    handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut failed=false; let mut canceled=false;
    for e in events { if let Event::Task(t) = e { match t { TaskEvent::Started { id: sid, kind } if sid==id.to_string() && kind=="GitPush" => started=true, TaskEvent::Completed { id: sid } if sid==id.to_string()=>completed=true, TaskEvent::Failed { id: sid, .. } if sid==id.to_string()=>failed=true, TaskEvent::Canceled { id:sid } if sid==id.to_string()=>canceled=true, _=>{} } } }
    assert!(started, "push should emit Started");
    assert!(completed, "push should emit Completed");
    assert!(!failed && !canceled, "push success should not fail or cancel");
}

#[tokio::test]
async fn git_push_lifecycle_failed_invalid_repo() {
    // create empty dir (no git init) so push fails fast
    let empty = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitPush { dest: empty.path().to_string_lossy().into(), remote: None, refspecs: None, username: None, password: None, strategy_override: None });
    let handle = reg.spawn_git_push_task(Some(AppHandle), id, token, empty.path().to_string_lossy().into(), None, None, None, None, None);
    handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut failed=false; let mut canceled=false;
    for e in events { if let Event::Task(t) = e { match t { TaskEvent::Started { id: sid, kind } if sid==id.to_string() && kind=="GitPush" => started=true, TaskEvent::Completed { id: sid } if sid==id.to_string()=>completed=true, TaskEvent::Failed { id: sid, .. } if sid==id.to_string()=>failed=true, TaskEvent::Canceled { id:sid } if sid==id.to_string()=>canceled=true, _=>{} } } }
    assert!(started, "push should emit Started even on failure");
    assert!(failed, "push invalid repo should emit Failed");
    assert!(!completed && !canceled, "invalid repo should not complete or cancel");
}

#[tokio::test]
async fn git_push_lifecycle_failed_invalid_repo_no_apphandle() {
    // 与上一个测试不同：不提供 AppHandle，验证 fallback failed 事件仍出现
    let empty = tempfile::tempdir().unwrap();
    let reg = Arc::new(TaskRegistry::new());
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::GitPush { dest: empty.path().to_string_lossy().into(), remote: None, refspecs: None, username: None, password: None, strategy_override: None });
    let handle = reg.spawn_git_push_task(None, id, token, empty.path().to_string_lossy().into(), None, None, None, None, None);
    handle.await.unwrap();

    let events = bus.snapshot();
    let mut started=false; let mut failed=false; let mut duplicate_failed=0;
    for e in events { if let Event::Task(t)=e { match t { TaskEvent::Started { id:sid, kind } if sid==id.to_string() && kind=="GitPush" => started=true,
        TaskEvent::Failed { id:sid, .. } if sid==id.to_string() => { failed=true; duplicate_failed+=1; }, _=>{} } } }
    assert!(started, "push should emit Started");
    assert!(failed, "push should emit Failed without AppHandle");
    assert!(duplicate_failed==1, "failed should only emit once, got {}", duplicate_failed);
}
