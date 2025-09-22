use fireworks_collaboration_lib::events::structured::{MemoryEventBus, Event, TaskEvent, EventBusAny};
use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
use std::sync::Arc;

#[tokio::test]
async fn lifecycle_sleep_completed() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::Sleep { ms: 30 });
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let handle = reg.spawn_sleep_task(None, id, token, 30);
    // 轮询等待 Started 事件出现
    let mut spins=0; while spins<20 { if bus.snapshot().iter().any(|e| matches!(e, Event::Task(TaskEvent::Started { id: sid, .. }) if sid==&id.to_string())) { break; } tokio::time::sleep(std::time::Duration::from_millis(5)).await; spins+=1; }
    handle.await.unwrap();
    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut canceled=false; let mut failed=false;
    for e in events { if let Event::Task(te) = e { match te { TaskEvent::Started { id: sid, .. } if sid==id.to_string() => started=true, TaskEvent::Completed { id: sid } if sid==id.to_string()=>completed=true, TaskEvent::Canceled { id: sid } if sid==id.to_string()=>canceled=true, TaskEvent::Failed { id: sid, .. } if sid==id.to_string()=>failed=true, _=>{} } } }
    assert!(started, "should have Started event");
    assert!(completed, "should have Completed event");
    assert!(!canceled, "should not be Canceled");
    assert!(!failed, "should not be Failed");
    // no thread-local bus to clear
}

#[tokio::test]
async fn lifecycle_sleep_canceled() {
    let reg = Arc::new(TaskRegistry::new());
    let (id, token) = reg.create(fireworks_collaboration_lib::core::tasks::model::TaskKind::Sleep { ms: 200 });
    let bus = MemoryEventBus::new();
    reg.inject_structured_bus(Arc::new(bus.clone()) as Arc<dyn EventBusAny>);
    let handle = reg.spawn_sleep_task(None, id, token.clone(), 200);
    // 等待进入 Running
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    token.cancel();
    // 等待 Started 捕获
    let mut spins=0; while spins<20 { if bus.snapshot().iter().any(|e| matches!(e, Event::Task(TaskEvent::Started { id: sid, .. }) if sid==&id.to_string())) { break; } tokio::time::sleep(std::time::Duration::from_millis(5)).await; spins+=1; }
    token.cancel();
    handle.await.unwrap();
    let events = bus.snapshot();
    let mut started=false; let mut completed=false; let mut canceled=false; let mut failed=false;
    for e in events { if let Event::Task(te) = e { match te { TaskEvent::Started { id: sid, .. } if sid==id.to_string() => started=true, TaskEvent::Completed { id: sid } if sid==id.to_string()=>completed=true, TaskEvent::Canceled { id: sid } if sid==id.to_string()=>canceled=true, TaskEvent::Failed { id: sid, .. } if sid==id.to_string()=>failed=true, _=>{} } } }
    assert!(started, "should have Started event");
    assert!(canceled, "should have Canceled event");
    assert!(!completed, "should not be Completed");
    assert!(!failed, "should not be Failed");
    // no thread-local bus to clear
}
