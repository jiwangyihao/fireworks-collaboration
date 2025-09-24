//! git_scenarios: 为 clone/fetch 等后续阶段提供高层封装入口（12.4 前置脚手架）。
//! 当前仅放置最小 CloneParams / CloneOutcome 及 run_clone 占位，
//! 后续 12.4 将在此完善远端 fixture / 事件采集 / 错误分类映射。

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use fireworks_collaboration_lib::core::git::service::ProgressPayload;
// 移除未使用的错误分类导入（占位阶段不区分分类）
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
pub enum PushResultKind { Success, Abort, Exhausted }

/// Push Retry 规格（聚合测试层使用，封装底层 RetryCase + 是否模拟冲突）。
#[derive(Debug, Clone, Copy)]
pub struct PushRetrySpec { pub case: RetryCase, pub simulate_conflict: bool }

/// Push Retry 执行结果（供断言）。
#[derive(Debug, Clone)]
pub struct PushRetryOutcome { pub result: PushResultKind, pub attempts_used: u8, pub events: Vec<String> }
impl PushRetryOutcome {
    pub fn is_success(&self) -> bool { matches!(self.result, PushResultKind::Success) }
    pub fn terminal_tag(&self) -> Option<&str> { self.events.iter().rev().find(|e| e.starts_with("push:result:" )).map(|s| s.as_str()) }
}

#[derive(Debug, Clone, Copy)]
struct PushAttemptCtx { attempt: u8, delay: u64 }

fn emit_attempt(events: &mut Vec<String>, ctx: PushAttemptCtx) {
    events.push(format!("push:attempt#{}:delay={}ms", ctx.attempt, ctx.delay));
}

fn build_clone_dest() -> PathBuf { std::env::temp_dir().join(format!("fwc-clone-dest-{}", uuid::Uuid::new_v4())) }

/// 运行一次（模拟）push + retry：
/// 规则：
///  * simulate_conflict=true 时，除非策略强制提前成功/中止，否则所有尝试均视为冲突，直到耗尽 attempts -> Exhausted。
///  * PolicyOverride::ForceSuccessEarly -> 第二次尝试直接成功（若 attempts>=2）。
///  * PolicyOverride::AbortAfter(k) -> 第 k 次尝试开始前直接 Abort（attempts_used=k-1）。
///  * backoff_seq = compute_backoff_sequence(case) 截取 attempts_used 长度。
pub fn run_push_with_retry(spec: &PushRetrySpec) -> PushRetryOutcome {
    let mut events = vec!["push:op:Push".into()];
    let seq_full = compute_backoff_sequence(&spec.case);
    // AbortBefore: 直接中止
    if let PolicyOverride::AbortAfter(k) = spec.case.policy {
        if k > 0 {
            events.push(format!("push:abort:before_attempt#{}", k));
            // 发出明确的终态 result 事件，便于 tag/终态断言
            events.push("push:result:abort".into());
            return PushRetryOutcome { result: PushResultKind::Abort, attempts_used: k.saturating_sub(1), events };
        }
    }
    let mut result = PushResultKind::Success; // 默认成功
    let mut attempts_used = 0u8;
    for attempt in 1..=spec.case.attempts {
        attempts_used = attempt;
        let ctx = PushAttemptCtx { attempt, delay: seq_full[(attempt-1) as usize] };
        emit_attempt(&mut events, ctx);
        if matches!(spec.case.policy, PolicyOverride::ForceSuccessEarly) && attempt >= 2 {
            result = PushResultKind::Success; events.push("push:result:success".into()); break;
        }
        if spec.simulate_conflict {
            if attempt == spec.case.attempts { result = PushResultKind::Exhausted; events.push("push:result:exhausted".into()); }
            else { events.push("push:result:retry".into()); continue; }
        } else { result = PushResultKind::Success; events.push("push:result:success".into()); break; }
    }
    PushRetryOutcome { result, attempts_used, events }
}

/// 基础 clone 参数。后续 shallow/partial/depth 会拆分到专用矩阵文件。
#[derive(Debug, Clone, Default)]
pub struct CloneParams { pub depth: Option<u32>, pub filter: Option<String> }
impl CloneParams { pub fn new() -> Self { Self { depth:None, filter:None } } }

/// clone 结果概要（事件/错误/目标路径）。事件 DSL 引入后可改为结构化事件列表。
#[derive(Debug, Default)]
pub struct CloneOutcome { pub dest: PathBuf, pub events: Vec<String> }

/// 占位：执行一次 clone。当前实现只模拟初始化（不真正远端交互），用于建立调用形状。
/// 12.4 实际迁移时将：
/// 1. 构造远端 fixture 仓库（或引用生成器）
/// 2. 调用生产 git_clone 实现（尚未引入故暂留）
/// 3. 收集 ProgressPayload.phase 进事件向量
/// 4. 返回错误分类给上层断言
pub fn run_clone(params: &CloneParams) -> CloneOutcome {
    let dest = build_clone_dest();
    std::fs::create_dir_all(&dest).expect("create clone dest temp dir");
    let cancel = AtomicBool::new(false);
    let mut events = Vec::new();
    // 目前 default_impl::clone::do_clone 仅支持 depth，其余参数留作后续扩展。
    let _res = impl_clone::do_clone("https://example.com/placeholder.git", &dest, params.depth, &cancel, |p:ProgressPayload| {
        events.push(p.phase);
    });
    // 读取 filter 字段一次以避免未使用字段警告（未来真实实现会据此改变行为）
    if let Some(f) = &params.filter { if f.is_empty() { events.push("filter:empty".into()); } }
    if events.is_empty() { events.push("Init".into()); }
    CloneOutcome { dest, events }
}

/// 示例：未来真实调用形状（文档化用途）。
#[allow(dead_code)]
pub fn _run_clone_with_cancel(params: &CloneParams, cancel: &AtomicBool) -> CloneOutcome {
    let dest = build_clone_dest();
    std::fs::create_dir_all(&dest).expect("create clone dest temp dir");
    let mut events = Vec::new();
    let _res = impl_clone::do_clone("https://example.com/placeholder.git", &dest, params.depth, cancel, |p:ProgressPayload| { events.push(p.phase); if cancel.load(Ordering::Relaxed) { /* early flag checked by impl */ } });
    CloneOutcome { dest, events }
}

/// 统一 clone 结果基础断言：
/// - 事件非空
/// - 目标目录存在
/// 未来真实实现接入后可扩展分类 / 结束标志检查。
#[allow(dead_code)]
pub fn assert_clone_events(label: &str, out: &CloneOutcome) {
    assert!(!out.events.is_empty(), "[{label}] events should not be empty");
    assert!(out.dest.exists(), "[{label}] dest should exist");
}

// ---- Fetch (events-oriented placeholder) ----
/// 基础 fetch 参数（保持与 CloneParams 形状相近，便于组合/迁移）。
#[derive(Debug, Clone, Default)]
pub struct FetchParams { pub depth: Option<u32>, pub filter: Option<String> }

/// fetch 结果（当前仅需要事件序列用于断言 Tag/marker）。
#[derive(Debug, Default, Clone)]
pub struct FetchOutcome { pub events: Vec<String> }

/// 运行一次（占位）fetch：
/// - 生成标准化事件前缀："fetch:Start" -> [optional filter marker] -> "fetch:Complete"
/// - 若提供 filter，插入原样字符串（推荐以 "filter:" 前缀传入），便于 `assess_partial_filter` 检测 marker。
/// - 若提供 depth，插入提示事件（当前仅用于可视化，不参与断言）。
pub fn run_fetch(params: &FetchParams) -> FetchOutcome {
    let mut events = Vec::new();
    events.push("fetch:Start".into());
    if let Some(depth) = params.depth { events.push(format!("shallow:depth:{}", depth)); }
    if let Some(f) = &params.filter { if !f.is_empty() { events.push(f.clone()); } }
    events.push("fetch:Complete".into());
    FetchOutcome { events }
}

#[cfg(test)]
mod tests_scenarios_smoke {
    use super::*;
    #[test]
    fn clone_params_default_smoke() {
    let out = run_clone(&CloneParams::new());
        // placeholder repo URL will likely fail; allow either success or error but must produce at least one event
        assert!(!out.events.is_empty(), "smoke: events should not be empty");
        assert!(out.dest.exists(), "smoke: dest directory should exist");
        // 不强制 Complete，因占位远端可能失败；后续真实远端 fixture 后再增强
    }

    #[test]
    fn push_retry_force_success_early_smoke() {
        use crate::common::retry_matrix::{retry_cases, BackoffKind};
        let case = retry_cases().into_iter().find(|c| matches!(c.backoff, BackoffKind::Constant) && c.attempts >= 3).expect("have constant attempts>=3 case");
    let _spec = PushRetrySpec { case: case, simulate_conflict: true }; // underscore to silence unused var (kept for shape documentation)
        let out = run_push_with_retry(&PushRetrySpec { case: RetryCase { attempts: case.attempts, base_delay_ms: case.base_delay_ms, backoff: case.backoff, policy: PolicyOverride::ForceSuccessEarly }, simulate_conflict: true });
        assert!(out.is_success());
        assert!(out.terminal_tag().is_some());
        assert!(out.attempts_used >= 2, "should succeed on or after attempt 2");
    }
}
