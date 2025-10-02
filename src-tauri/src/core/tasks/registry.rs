use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use tokio::{
    task::JoinHandle,
    time::{sleep, Duration},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::events::emitter::{emit_all, AppHandle};
use crate::events::structured::publish_global;

use crate::core::tasks::model::{
    LifecycleFlags, TaskErrorEvent, TaskKind, TaskMeta, TaskProgressEvent, TaskSnapshot, TaskState,
    TaskStateEvent,
};

pub(in crate::core::tasks) const EV_STATE: &str = "task://state";
pub(in crate::core::tasks) const EV_PROGRESS: &str = "task://progress";
pub(in crate::core::tasks) const EV_ERROR: &str = "task://error";

pub struct TaskRegistry {
    pub(in crate::core::tasks) inner: Mutex<HashMap<Uuid, TaskMeta>>,
    pub(in crate::core::tasks) structured_bus: Mutex<Option<Arc<dyn crate::events::structured::EventBusAny>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            structured_bus: Mutex::new(None),
        }
    }

    /// 测试/调用方可注入专用结构化事件总线（绕过全局/线程局部限制，便于捕获跨线程任务生命周期事件）
    pub fn inject_structured_bus(&self, bus: Arc<dyn crate::events::structured::EventBusAny>) {
        *self.structured_bus.lock().unwrap() = Some(bus);
    }

    pub(in crate::core::tasks) fn publish_structured(&self, evt: crate::events::structured::Event) {
        if let Some(bus) = self.structured_bus.lock().unwrap().as_ref() {
            bus.publish(evt.clone());
        }
        publish_global(evt);
    }

    pub fn create(&self, kind: TaskKind) -> (Uuid, CancellationToken) {
        let id = Uuid::new_v4();
        let token = CancellationToken::new();
        let meta = TaskMeta {
            id,
            kind,
            state: TaskState::Pending,
            created_at: SystemTime::now(),
            cancel_token: token.clone(),
            fail_reason: None,
            lifecycle_flags: LifecycleFlags::default(),
        };
        self.inner.lock().unwrap().insert(id, meta);
        (id, token)
    }

    pub fn list(&self) -> Vec<TaskSnapshot> {
        self.inner
            .lock()
            .unwrap()
            .values()
            .map(TaskSnapshot::from)
            .collect()
    }

    pub fn snapshot(&self, id: &Uuid) -> Option<TaskSnapshot> {
        self.inner.lock().unwrap().get(id).map(TaskSnapshot::from)
    }

    pub fn cancel(&self, id: &Uuid) -> bool {
        self.inner
            .lock()
            .unwrap()
            .get(id)
            .map(|m| {
                m.cancel_token.cancel();
                true
            })
            .unwrap_or(false)
    }

    pub(in crate::core::tasks) fn with_meta<F: FnOnce(&mut TaskMeta)>(&self, id: &Uuid, f: F) -> Option<TaskMeta> {
        let mut guard = self.inner.lock().unwrap();
        if let Some(m) = guard.get_mut(id) {
            f(m);
            Some(m.clone())
        } else {
            None
        }
    }

    pub(in crate::core::tasks) fn emit_state(&self, app: &AppHandle, id: &Uuid) {
        if let Some(m) = self.inner.lock().unwrap().get(id) {
            let evt = TaskStateEvent::new(m);
            emit_all(app, EV_STATE, &evt);
        }
    }

    pub(in crate::core::tasks) fn set_state_emit(&self, app: &AppHandle, id: &Uuid, state: TaskState) {
        if self.with_meta(id, |m| m.state = state).is_some() {
            self.emit_state(app, id);
        }
    }

    pub(in crate::core::tasks) fn set_state_noemit(&self, id: &Uuid, state: TaskState) {
        let _ = self.with_meta(id, |m| m.state = state);
    }

    pub(in crate::core::tasks) fn emit_error_structured(&self, evt: &TaskErrorEvent) {
        use crate::events::structured::{
            Event as StructuredEvent, TaskEvent as StructuredTaskEvent,
        };
        self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Failed {
            id: evt.task_id.to_string(),
            category: evt.category.clone(),
            code: evt.code.clone(),
            message: evt.message.clone(),
        }));
        let _ = self.with_meta(&evt.task_id, |m| {
            m.fail_reason = Some(evt.message.clone());
            if !m.lifecycle_flags.failed {
                m.lifecycle_flags.failed = true;
            }
        });
    }

    /// 幂等生命周期事件发布
    pub(in crate::core::tasks) fn publish_lifecycle_started(&self, id: &Uuid, kind: &str) {
        use crate::events::structured::{
            Event as StructuredEvent, TaskEvent as StructuredTaskEvent,
        };
        let mut should = false;
        let kind_owned = kind.to_string();
        if self
            .with_meta(id, |m| {
                if !m.lifecycle_flags.started {
                    m.lifecycle_flags.started = true;
                    should = true;
                }
            })
            .is_some()
            && should
        {
            self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Started {
                id: id.to_string(),
                kind: kind_owned,
            }));
        }
    }

    pub(in crate::core::tasks) fn publish_lifecycle_completed(&self, id: &Uuid) {
        use crate::events::structured::{
            Event as StructuredEvent, TaskEvent as StructuredTaskEvent,
        };
        let mut should = false;
        if self
            .with_meta(id, |m| {
                if !m.lifecycle_flags.completed {
                    m.lifecycle_flags.completed = true;
                    should = true;
                }
            })
            .is_some()
            && should
        {
            self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Completed {
                id: id.to_string(),
            }));
        }
    }

    pub(in crate::core::tasks) fn publish_lifecycle_canceled(&self, id: &Uuid) {
        use crate::events::structured::{
            Event as StructuredEvent, TaskEvent as StructuredTaskEvent,
        };
        let mut should = false;
        if self
            .with_meta(id, |m| {
                if !m.lifecycle_flags.canceled {
                    m.lifecycle_flags.canceled = true;
                    should = true;
                }
            })
            .is_some()
            && should
        {
            self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Canceled {
                id: id.to_string(),
            }));
        }
    }

    pub(in crate::core::tasks) fn publish_lifecycle_failed_if_needed(&self, id: &Uuid, message: &str) {
        use crate::events::structured::{
            Event as StructuredEvent, TaskEvent as StructuredTaskEvent,
        };
        let mut should = false;
        if self
            .with_meta(id, |m| {
                if !m.lifecycle_flags.failed {
                    m.lifecycle_flags.failed = true;
                    should = true;
                }
            })
            .is_some()
            && should
        {
            self.publish_structured(StructuredEvent::Task(StructuredTaskEvent::Failed {
                id: id.to_string(),
                category: "Unknown".into(),
                code: None,
                message: message.to_string(),
            }));
        }
    }

    pub(in crate::core::tasks) fn emit_error(&self, app: &AppHandle, evt: &TaskErrorEvent) {
        emit_all(app, EV_ERROR, evt);
        self.emit_error_structured(evt);
    }

    pub(in crate::core::tasks) fn emit_error_if_app<F>(&self, app: &Option<AppHandle>, build: F)
    where
        F: FnOnce() -> TaskErrorEvent,
    {
        if let Some(app_ref) = app {
            let evt = build();
            self.emit_error(app_ref, &evt);
        }
    }

    pub(in crate::core::tasks) fn mark_running(&self, app: &Option<AppHandle>, id: &Uuid, kind: &str) {
        match app {
            Some(app_ref) => self.set_state_emit(app_ref, id, TaskState::Running),
            None => self.set_state_noemit(id, TaskState::Running),
        }
        self.publish_lifecycle_started(id, kind);
    }

    pub(in crate::core::tasks) fn mark_completed(&self, app: &Option<AppHandle>, id: &Uuid) {
        match app {
            Some(app_ref) => self.set_state_emit(app_ref, id, TaskState::Completed),
            None => self.set_state_noemit(id, TaskState::Completed),
        }
        self.publish_lifecycle_completed(id);
    }

    pub(in crate::core::tasks) fn mark_canceled(&self, app: &Option<AppHandle>, id: &Uuid) {
        match app {
            Some(app_ref) => self.set_state_emit(app_ref, id, TaskState::Canceled),
            None => self.set_state_noemit(id, TaskState::Canceled),
        }
        self.publish_lifecycle_canceled(id);
    }

    pub(in crate::core::tasks) fn mark_failed(&self, app: &Option<AppHandle>, id: &Uuid, fallback_message: &str) {
        match app {
            Some(app_ref) => self.set_state_emit(app_ref, id, TaskState::Failed),
            None => {
                self.set_state_noemit(id, TaskState::Failed);
                self.publish_lifecycle_failed_if_needed(id, fallback_message);
            }
        }
    }

    pub fn spawn_sleep_task(
        self: &Arc<Self>,
        app: Option<AppHandle>,
        id: Uuid,
        token: CancellationToken,
        total_ms: u64,
    ) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            match &app {
                Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Running),
                None => this.set_state_noemit(&id, TaskState::Running),
            }
            this.publish_lifecycle_started(&id, "Sleep");
            let step = 50u64;
            let mut elapsed = 0u64;
            while elapsed < total_ms {
                if token.is_cancelled() {
                    match &app {
                        Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Canceled),
                        None => this.set_state_noemit(&id, TaskState::Canceled),
                    }
                    this.publish_lifecycle_canceled(&id);
                    return;
                }
                sleep(Duration::from_millis(step)).await;
                elapsed += step;
                if let Some(app_ref) = &app {
                    let percent = ((elapsed.min(total_ms) as f64 / total_ms as f64) * 100.0) as u32;
                    let prog = TaskProgressEvent {
                        task_id: id,
                        kind: "Sleep".into(),
                        phase: "Running".into(),
                        percent,
                        objects: None,
                        bytes: None,
                        total_hint: None,
                        retried_times: None,
                    };
                    emit_all(app_ref, EV_PROGRESS, &prog);
                }
            }
            match &app {
                Some(app_ref) => this.set_state_emit(app_ref, &id, TaskState::Completed),
                None => this.set_state_noemit(&id, TaskState::Completed),
            }
            this.publish_lifecycle_completed(&id);
        })
    }
}

pub type SharedTaskRegistry = Arc<TaskRegistry>;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    async fn wait_for_state(reg: &TaskRegistry, id: Uuid, target: TaskState, max_ms: u64) -> bool {
        let mut waited = 0u64;
        while waited < max_ms {
            if let Some(s) = reg.snapshot(&id) {
                if s.state == target {
                    return true;
                }
            }
            sleep(Duration::from_millis(20)).await;
            waited += 20;
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
        let ok = wait_for_state(&reg, id, TaskState::Completed, 1500).await;
        assert!(ok, "task should complete");
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_cancel_sleep_task() {
        let reg = Arc::new(TaskRegistry::new());
        let (id, token) = reg.create(TaskKind::Sleep { ms: 500 });
        let handle = reg.spawn_sleep_task(None, id, token.clone(), 500);
        let entered = wait_for_state(&reg, id, TaskState::Running, 500).await;
        assert!(entered, "should enter running");
        token.cancel();
        let canceled = wait_for_state(&reg, id, TaskState::Canceled, 1000).await;
        assert!(canceled, "should cancel");
        handle.await.unwrap();
    }
}
