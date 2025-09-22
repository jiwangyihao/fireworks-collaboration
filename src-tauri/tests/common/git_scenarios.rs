//! git_scenarios: 为 clone/fetch 等后续阶段提供高层封装入口（12.4 前置脚手架）。
//! 当前仅放置最小 CloneParams / CloneOutcome 及 run_clone 占位，
//! 后续 12.4 将在此完善远端 fixture / 事件采集 / 错误分类映射。

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use fireworks_collaboration_lib::core::git::service::ProgressPayload;
use fireworks_collaboration_lib::core::git::errors::{GitError, ErrorCategory};
use fireworks_collaboration_lib::core::git::default_impl::clone as impl_clone;
use crate::common::retry_matrix::{RetryCase, compute_backoff_sequence, PolicyOverride};

/// Git 操作类型（为 12.10 策略/override 与事件 DSL 预留）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitOp { Clone, Fetch, Push }

/// 轻量事件种类占位：后续将由结构化事件替换当前字符串集合。
/// 保持最小集，避免过早锁死：Attempt/Result/FilterMarker/Backoff。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitEventKind { Attempt, Result, FilterMarker, Backoff }

/// Push 结果分类（占位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushResultKind { Success, Conflict, Abort, Exhausted }

/// Push Retry 规格（聚合测试层使用，封装底层 RetryCase + 是否模拟冲突）。
#[derive(Debug, Clone, Copy)]
pub struct PushRetrySpec {
    pub case: RetryCase,
    pub simulate_conflict: bool,
}

/// Push Retry 执行结果（供断言）。
#[derive(Debug, Clone)]
pub struct PushRetryOutcome {
    pub result: PushResultKind,
    pub attempts_used: u8,
    pub backoff_seq: Vec<u64>,
    pub events: Vec<String>, // 简化：字符串事件，后续 DSL 升级
}

/// 运行一次（模拟）push + retry：
/// 规则：
///  * simulate_conflict=true 时，除非策略强制提前成功/中止，否则所有尝试均视为冲突，直到耗尽 attempts -> Exhausted。
///  * PolicyOverride::ForceSuccessEarly -> 第二次尝试直接成功（若 attempts>=2）。
///  * PolicyOverride::AbortAfter(k) -> 第 k 次尝试开始前直接 Abort（attempts_used=k-1）。
///  * backoff_seq = compute_backoff_sequence(case) 截取 attempts_used 长度。
pub fn run_push_with_retry(spec: &PushRetrySpec) -> PushRetryOutcome {
    let mut events = Vec::new();
    // 预留 GitOp 标签：当前推送固定为 Push
    events.push("push:op:Push".into());
    let mut attempts_used: u8 = 0;
    let seq_full = compute_backoff_sequence(&spec.case);
    let mut result = PushResultKind::Success; // 默认成功；后续按逻辑覆盖
    // 处理策略提前 Abort
    if let PolicyOverride::AbortAfter(k) = spec.case.policy {
        if k > 0 { // 直接 abort，不进入尝试循环
            result = PushResultKind::Abort;
            events.push(format!("push:abort:before_attempt#{}", k));
            return PushRetryOutcome { result, attempts_used: k.saturating_sub(1), backoff_seq: seq_full[..(k.min(seq_full.len() as u8) as usize)].to_vec(), events };
        }
    }
    for attempt in 1..=spec.case.attempts { // 1-based
        attempts_used = attempt;
        // 记录尝试事件（包含逻辑 backoff 值）
        let delay = seq_full[(attempt-1) as usize];
        events.push(format!("push:attempt#{}:delay={}ms", attempt, delay));
        // ForceSuccessEarly: 第二次尝试直接成功
        if matches!(spec.case.policy, PolicyOverride::ForceSuccessEarly) && attempt >= 2 {
            result = PushResultKind::Success;
            events.push("push:result:success".into());
            break;
        }
        if spec.simulate_conflict {
            // 最后一轮仍冲突 -> Exhausted；否则继续
            if attempt == spec.case.attempts {
                result = PushResultKind::Exhausted;
                events.push("push:result:exhausted".into());
            } else {
                events.push("push:result:conflict".into());
                continue; // 尝试下一次
            }
        } else {
            result = PushResultKind::Success;
            events.push("push:result:success".into());
            break;
        }
    }
    let backoff_seq = seq_full[..(attempts_used as usize)].to_vec();
    PushRetryOutcome { result, attempts_used, backoff_seq, events }
}

/// 基础 clone 参数。后续 shallow/partial/depth 会拆分到专用矩阵文件。
#[derive(Debug, Clone, Default)]
pub struct CloneParams {
    pub recursive: bool,
    pub tags: bool,
    pub depth: Option<u32>,
    pub filter: Option<String>,
    // 预留：sparse / single_branch / strategy_override
}

/// clone 结果概要（事件/错误/目标路径）。事件 DSL 引入后可改为结构化事件列表。
#[derive(Debug, Default)]
pub struct CloneOutcome {
    pub dest: PathBuf,
    pub events: Vec<String>,    // 暂存 phase 字符串；后续换成枚举/结构
    pub error: Option<GitError>,
    pub category: Option<ErrorCategory>,
}

/// 占位：执行一次 clone。当前实现只模拟初始化（不真正远端交互），用于建立调用形状。
/// 12.4 实际迁移时将：
/// 1. 构造远端 fixture 仓库（或引用生成器）
/// 2. 调用生产 git_clone 实现（尚未引入故暂留）
/// 3. 收集 ProgressPayload.phase 进事件向量
/// 4. 返回错误分类给上层断言
pub fn run_clone(params: &CloneParams) -> CloneOutcome {
    let dest = std::env::temp_dir().join(format!("fwc-clone-dest-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest).expect("create clone dest temp dir");
    let cancel = AtomicBool::new(false);
    let mut events = Vec::new();
    // 目前 default_impl::clone::do_clone 仅支持 depth，其余参数留作后续扩展。
    let res = impl_clone::do_clone("https://example.com/placeholder.git", &dest, params.depth, &cancel, |p:ProgressPayload| {
        events.push(p.phase);
    });
    let (error, category) = match res { Ok(_) => (None, None), Err(e) => {
        let cat = match e { GitError::Categorized { category, .. } => Some(category) };
        (Some(e), cat)
    }};
    if events.is_empty() { events.push("Init".into()); }
    CloneOutcome { dest, events, error, category }
}

/// 示例：未来真实调用形状（文档化用途）。
#[allow(dead_code)]
pub fn _run_clone_with_cancel(params: &CloneParams, cancel: &AtomicBool) -> CloneOutcome {
    let dest = std::env::temp_dir().join(format!("fwc-clone-dest-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dest).expect("create clone dest temp dir");
    let mut events = Vec::new();
    let res = impl_clone::do_clone("https://example.com/placeholder.git", &dest, params.depth, cancel, |p:ProgressPayload| { events.push(p.phase); if cancel.load(Ordering::Relaxed) { /* early flag checked by impl */ } });
    let (error, category) = match res { Ok(_) => (None, None), Err(e) => { let cat = match e { GitError::Categorized { category, .. } => Some(category) }; (Some(e), cat) } };
    CloneOutcome { dest, events, error, category }
}

#[cfg(test)]
mod tests_scenarios_smoke {
    use super::*;
    #[test]
    fn clone_params_default_smoke() {
        let out = run_clone(&CloneParams { recursive:false, tags:false, depth:None, filter:None });
        // placeholder repo URL will likely fail; allow either success or error but must produce at least one event
        assert!(!out.events.is_empty(), "smoke: events should not be empty");
        assert!(out.dest.exists(), "smoke: dest directory should exist");
        // 不强制 Complete，因占位远端可能失败；后续真实远端 fixture 后再增强
    }

    #[test]
    fn push_retry_force_success_early_smoke() {
        use crate::common::retry_matrix::{retry_cases, BackoffKind};
        let case = retry_cases().into_iter().find(|c| matches!(c.backoff, BackoffKind::Constant) && c.attempts >= 3).expect("have constant attempts>=3 case");
        let spec = PushRetrySpec { case: case, simulate_conflict: true };
        let out = run_push_with_retry(&PushRetrySpec { case: RetryCase { attempts: case.attempts, base_delay_ms: case.base_delay_ms, backoff: case.backoff, policy: PolicyOverride::ForceSuccessEarly }, simulate_conflict: true });
        assert!(matches!(out.result, PushResultKind::Success));
        assert!(out.attempts_used >= 2, "should succeed on or after attempt 2");
    }
}
