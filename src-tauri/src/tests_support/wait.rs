use std::time::Duration;
use uuid::Uuid;
use crate::tasks::{TaskRegistry, model::TaskState};
use crate::events::structured::{get_global_memory_bus, Event};

/// 默认轮询间隔 (ms)
pub const WAIT_INTERVAL_MS: u64 = 50;
/// 默认最大尝试次数
pub const WAIT_MAX_ATTEMPTS: usize = 120; // 50ms *120 ~=6s

#[derive(Debug, thiserror::Error)]
pub enum WaitError { #[error("timeout waiting condition")] Timeout }

/// 通用等待：直到任务进入终止状态 (Completed/Failed/Canceled) 或达到最大尝试次数。
pub async fn wait_task_terminal(reg: &TaskRegistry, id: &Uuid, interval_ms: u64, max_attempts: usize) -> Result<(), WaitError> {
    for attempt in 0..max_attempts {
        if let Some(s) = reg.snapshot(id) {
            if matches!(s.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) { return Ok(()); }
        }
        if attempt + 1 < max_attempts { tokio::time::sleep(Duration::from_millis(interval_ms)).await; }
    }
    Err(WaitError::Timeout)
}

/// 使用默认常量的便捷终止等待
pub async fn wait_task_terminal_default(reg:&TaskRegistry, id:&Uuid) -> Result<(), WaitError> {
    wait_task_terminal(reg,id,WAIT_INTERVAL_MS,WAIT_MAX_ATTEMPTS).await
}

/// 等待指定状态出现（返回 true 表示命中，false 表示超时）。
pub async fn wait_for_state(reg: &TaskRegistry, id: &Uuid, target: TaskState, interval_ms: u64, max_attempts: usize) -> Result<(), WaitError> {
    for attempt in 0..max_attempts {
        if let Some(s) = reg.snapshot(id) { if s.state == target { return Ok(()); } }
        if attempt + 1 < max_attempts { tokio::time::sleep(Duration::from_millis(interval_ms)).await; }
    }
    Err(WaitError::Timeout)
}

/// 通用等待：直到给定谓词为 true 或超时。
pub async fn wait_until<F: Fn() -> bool>(predicate: F, interval_ms: u64, max_attempts: usize) -> Result<(), WaitError> {
    for attempt in 0..max_attempts {
        if predicate() { return Ok(()); }
        if attempt + 1 < max_attempts { tokio::time::sleep(Duration::from_millis(interval_ms)).await; }
    }
    Err(WaitError::Timeout)
}

/// 事件等待：轮询全局 MemoryEventBus（若未设置则持续等待直至超时）。
pub async fn wait_for_event<P: Fn(&Event)->bool>(predicate: P, interval_ms: u64, max_attempts: usize) -> Result<(), WaitError> {
    for attempt in 0..max_attempts {
        if let Some(bus) = get_global_memory_bus() {
            let evts = bus.snapshot();
            if evts.iter().any(|e| predicate(e)) { return Ok(()); }
        }
        if attempt + 1 < max_attempts { tokio::time::sleep(Duration::from_millis(interval_ms)).await; }
    }
    Err(WaitError::Timeout)
}

/// 等待某任务 Summary.applied_codes 包含给定 code。
pub async fn wait_for_applied_code(id:&Uuid, code:&str, interval_ms:u64, max_attempts:usize) -> Result<(), WaitError> {
    let target = id.to_string();
    wait_for_event(|e| match e {
        Event::Strategy(crate::events::structured::StrategyEvent::Summary { id: sid, applied_codes, .. }) if sid == &target => applied_codes.iter().any(|c| c==code),
        _=>false
    }, interval_ms, max_attempts).await
}

pub async fn wait_for_applied_code_default(id:&Uuid, code:&str) -> Result<(), WaitError> {
    wait_for_applied_code(id, code, WAIT_INTERVAL_MS, WAIT_MAX_ATTEMPTS).await
}
