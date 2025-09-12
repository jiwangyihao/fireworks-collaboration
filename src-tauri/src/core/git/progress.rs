use std::sync::{atomic::AtomicBool, Arc};
use uuid::Uuid;
use crate::events::emitter::{emit_all, AppHandle};
use crate::core::tasks::model::TaskProgressEvent;

// 使用 prodash 提供的进度 traits（gix re-export）
use gix::progress::{self, Count, NestedProgress, Progress, Unit};

/// 一个简单的进度桥，将 gix 的进度回调转换为 TaskProgressEvent
pub struct TaskProgressBridge {
    pub task_id: Uuid,
    pub app: Option<AppHandle>,
    pub kind: String,
    pub phase: String,
    pub total: Option<usize>,
    pub current: usize,
    pub objects: u64,
    pub bytes: u64,
}

impl TaskProgressBridge {
    pub fn new(task_id: Uuid, app: Option<AppHandle>, kind: &str, phase: &str) -> Self {
        Self { task_id, app, kind: kind.into(), phase: phase.into(), total: None, current: 0, objects: 0, bytes: 0 }
    }

    fn emit(&self) {
        if let Some(app) = &self.app {
            let percent = match self.total {
                Some(t) if t > 0 => ((self.current as f64 / t as f64) * 100.0) as u32,
                _ => 0,
            };
            let evt = TaskProgressEvent {
                task_id: self.task_id,
                kind: self.kind.clone(),
                phase: self.phase.clone(),
                percent,
                objects: Some(self.objects),
                bytes: Some(self.bytes),
                total_hint: self.total.map(|v| v as u64),
            };
            emit_all(app, "task://progress", &evt);
        }
    }
}

impl Progress for TaskProgressBridge {
    fn init(&mut self, max: Option<usize>, _unit: Option<Unit>) {
        self.total = max;
        self.current = 0;
        self.emit();
    }

    fn set_name(&mut self, name: String) {
        self.phase = name;
        self.emit();
    }

    fn name(&self) -> Option<String> { Some(self.phase.clone()) }

    fn id(&self) -> [u8; 4] { progress::UNKNOWN }

    fn message(&self, _level: gix::progress::MessageLevel, _message: String) { /* ignore */ }
}

impl Count for TaskProgressBridge {
    fn set(&self, _step: usize) { /* not used as &self prevents mutation */ }
    fn step(&self) -> usize { self.current }
    fn inc_by(&self, _step: usize) { /* no-op */ }
    fn inc(&self) { /* no-op */ }
    fn counter(&self) -> std::sync::Arc<std::sync::atomic::AtomicUsize> { std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(self.current)) }
}

impl NestedProgress for TaskProgressBridge {
    type SubProgress = TaskProgressBridge;

    fn add_child(&mut self, name: impl Into<String>) -> Self::SubProgress {
        let mut child = TaskProgressBridge::new(self.task_id, self.app.clone(), &self.kind, &self.phase);
        child.phase = name.into();
        child
    }

    fn add_child_with_id(&mut self, name: impl Into<String>, _id: [u8; 4]) -> Self::SubProgress {
        self.add_child(name)
    }
}

/// 组合型的克隆进度，包含不同阶段的子进度
pub struct CloneProgress {
    pub root: TaskProgressBridge,
    pub negotiate: TaskProgressBridge,
    pub receive: TaskProgressBridge,
    pub checkout: TaskProgressBridge,
}

impl CloneProgress {
    pub fn new(task_id: Uuid, app: Option<AppHandle>) -> Self {
        let root = TaskProgressBridge::new(task_id, app.clone(), "GitClone", "Init");
        let negotiate = TaskProgressBridge::new(task_id, app.clone(), "GitClone", "Negotiating");
        let receive = TaskProgressBridge::new(task_id, app.clone(), "GitClone", "ReceivingPack");
        let checkout = TaskProgressBridge::new(task_id, app, "GitClone", "Checkout");
        Self { root, negotiate, receive, checkout }
    }
}

/// 将 AtomicBool 视图暴露给 gix 的 should_interrupt
pub fn should_interrupt_from_token(_token: &tokio_util::sync::CancellationToken) -> Vec<Arc<AtomicBool>> {
    // 我们用一个自定义 AtomicBool，并在上层循环里轮询 token 并写入该标志。
    // 由于 gix 仅仅读取该数组的标志，我们可以在阻塞任务中后台线程更新。
    vec![Arc::new(AtomicBool::new(false))]
}
