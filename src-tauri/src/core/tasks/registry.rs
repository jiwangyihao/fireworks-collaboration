use std::{collections::HashMap, sync::{Arc, Mutex}, time::SystemTime};
use tokio::{time::{sleep, Duration}, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::events::emitter::{emit_all, AppHandle};
use super::model::{TaskMeta, TaskKind, TaskState, TaskSnapshot, TaskStateEvent, TaskProgressEvent};

const EV_STATE: &str = "task://state";
const EV_PROGRESS: &str = "task://progress";

#[derive(Debug)]
pub struct TaskRegistry {
    inner: Mutex<HashMap<Uuid, TaskMeta>>,
}

impl TaskRegistry {
    pub fn new() -> Self { Self { inner: Mutex::new(HashMap::new()) } }

    pub fn create(&self, kind: TaskKind) -> (Uuid, CancellationToken) {
        let id = Uuid::new_v4();
        let token = CancellationToken::new();
        let meta = TaskMeta { id, kind, state: TaskState::Pending, created_at: SystemTime::now(), cancel_token: token.clone(), fail_reason: None };
        self.inner.lock().unwrap().insert(id, meta);
        (id, token)
    }

    pub fn list(&self) -> Vec<TaskSnapshot> { self.inner.lock().unwrap().values().map(TaskSnapshot::from).collect() }

    pub fn snapshot(&self, id: &Uuid) -> Option<TaskSnapshot> { self.inner.lock().unwrap().get(id).map(TaskSnapshot::from) }

    pub fn cancel(&self, id: &Uuid) -> bool { self.inner.lock().unwrap().get(id).map(|m| { m.cancel_token.cancel(); true }).unwrap_or(false) }

    fn with_meta<F: FnOnce(&mut TaskMeta)>(&self, id: &Uuid, f: F) -> Option<TaskMeta> {
        let mut guard = self.inner.lock().unwrap();
        if let Some(m) = guard.get_mut(id) { f(m); Some(m.clone()) } else { None }
    }

    fn emit_state(&self, app:&AppHandle, id:&Uuid) { if let Some(m) = self.inner.lock().unwrap().get(id) { let evt = TaskStateEvent::new(m); emit_all(app, EV_STATE, &evt); } }

    fn set_state_emit(&self, app:&AppHandle, id:&Uuid, s:TaskState){ if self.with_meta(id, |m| m.state = s).is_some(){ self.emit_state(app, id);} }

    fn set_state_noemit(&self, id:&Uuid, s:TaskState){ let _ = self.with_meta(id, |m| m.state = s); }

    pub fn spawn_sleep_task(self: &Arc<Self>, app: Option<AppHandle>, id: Uuid, token: CancellationToken, total_ms: u64) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            match &app { Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running), None => this.set_state_noemit(&id, TaskState::Running) }
            let step = 50u64; // 更细颗粒度便于测试
            let mut elapsed = 0u64;
            while elapsed < total_ms {
                if token.is_cancelled(){ match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Canceled), None=> this.set_state_noemit(&id,TaskState::Canceled)}; return; }
                sleep(Duration::from_millis(step)).await;
                elapsed += step;
                if let Some(app_ref) = &app {
                    let percent = ((elapsed.min(total_ms) as f64 / total_ms as f64) * 100.0) as u32;
                    let prog = TaskProgressEvent { task_id: id, kind: "Sleep".into(), phase: "Running".into(), percent };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                }
            }
            match &app { Some(app_ref)=> this.set_state_emit(app_ref,&id,TaskState::Completed), None=> this.set_state_noemit(&id,TaskState::Completed) }
        })
    }
}

pub type SharedTaskRegistry = Arc<TaskRegistry>;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    async fn wait_for_state(reg:&TaskRegistry, id:Uuid, target:TaskState, max_ms:u64) -> bool {
        let mut waited = 0u64;
        while waited < max_ms {
            if let Some(s) = reg.snapshot(&id) { if s.state == target { return true; } }
            sleep(Duration::from_millis(20)).await; waited += 20;
        }
        false
    }

    #[tokio::test]
    async fn test_create_initial_pending() {
        let reg = TaskRegistry::new();
        let (id, _t) = reg.create(TaskKind::Sleep { ms: 100 });
        let snap = reg.snapshot(&id).expect("snapshot");
        assert!(matches!(snap.state, TaskState::Pending));
    }

    #[tokio::test]
    async fn test_sleep_task_completes() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 120 });
        let handle = reg.spawn_sleep_task(None, id, token, 120);
        // 等待完成
        let ok = wait_for_state(&reg, id, TaskState::Completed, 1500).await;
        assert!(ok, "task should complete");
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cancel_sleep_task() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 500 });
        let handle = reg.spawn_sleep_task(None, id, token.clone(), 500);
        // 取消前先确认进入 running
        let entered = wait_for_state(&reg, id, TaskState::Running, 500).await; assert!(entered, "should enter running");
        token.cancel();
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await; assert!(canceled, "should cancel");
        handle.await.unwrap();
    }
}
