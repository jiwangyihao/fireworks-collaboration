//! http_override_stub: 12.10 前置占位，提供 HTTP Override / Strategy Override 测试所需最小结构。
//! 目标：统一描述 follow / max_events / idempotent 组合，生成事件向量用于早期断言。
//! 后续替换：接入真实实现后改为调用生产接口，并返回结构化事件。

use std::fmt::{Display, Formatter};
use crate::common::git_scenarios::GitOp;
use crate::common::CaseDescribe;

// ---- Event constants (便于后续结构化迁移统一管理) ----
const EV_PREFIX: &str = "http:override"; // 统一前缀
const EV_START: &str = "http:override:start"; // + :Op
const EV_INVALID_MAX: &str = "http:override:invalid_max";
const EV_APPLIED: &str = "http:override:applied";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowMode { None, Follow }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotentFlag { No, Yes }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxEventsCase { None, Some(u32) }

#[derive(Debug, Clone, Copy)]
pub struct HttpOverrideCase {
    pub op: GitOp,
    pub follow: FollowMode,
    pub idempotent: IdempotentFlag,
    pub max_events: MaxEventsCase,
}

impl Display for HttpOverrideCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HttpOverride({:?},{:?},{:?},{:?})", self.op, self.follow, self.idempotent, self.max_events)
    }
}

impl HttpOverrideCase {
    pub fn describe(&self) -> String { format!("{:?}-{:?}-{:?}-{:?}", self.op, self.follow, self.idempotent, self.max_events) }
}

impl CaseDescribe for HttpOverrideCase { fn describe(&self) -> String { self.describe() } }

#[derive(Debug, Clone)]
pub struct OverrideOutcome {
    pub applied: bool,
    pub events: Vec<String>,
    pub follow_chain: Vec<String>,
}

impl OverrideOutcome {
    pub fn is_applied(&self) -> bool { self.applied }
    pub fn terminal_event(&self) -> Option<&str> { self.events.last().map(|s| s.as_str()) }
}

/// 代表性矩阵：裁剪组合避免爆炸；保留差异来源：follow/idempotent/max_events/不同 op。
pub fn http_override_cases() -> Vec<HttpOverrideCase> {
    use IdempotentFlag::{No, Yes}; use GitOp::{Clone, Fetch, Push};
    vec![
        HttpOverrideCase { op: Clone, follow: FollowMode::None,   idempotent: No,  max_events: MaxEventsCase::None },
        HttpOverrideCase { op: Clone, follow: FollowMode::Follow, idempotent: No,  max_events: MaxEventsCase::Some(5) },
        HttpOverrideCase { op: Fetch, follow: FollowMode::Follow, idempotent: Yes, max_events: MaxEventsCase::Some(3) },
        HttpOverrideCase { op: Push,  follow: FollowMode::None,   idempotent: Yes, max_events: MaxEventsCase::None },
        HttpOverrideCase { op: Push,  follow: FollowMode::Follow, idempotent: Yes, max_events: MaxEventsCase::Some(2) },
    ]
}

/// 按操作类型过滤代表性 case（便于单文件内快速选择）
pub fn http_override_cases_for(op: GitOp) -> Vec<HttpOverrideCase> { http_override_cases().into_iter().filter(|c| c.op==op).collect() }

fn emit(events: &mut Vec<String>, e: impl Into<String>) { events.push(e.into()); }

/// 根据 case 计算 follow hops（集中限制策略）
fn follow_hops(case: &HttpOverrideCase) -> usize {
    if !matches!(case.follow, FollowMode::Follow) { return 0; }
    match case.max_events { MaxEventsCase::Some(n) => (n.min(3)).max(1) as usize, MaxEventsCase::None => 2 }
}

/// 占位执行：根据 case 生成伪事件。后续将替换为真实 override 流执行。
pub fn run_http_override(case: &HttpOverrideCase) -> OverrideOutcome {
    let mut events = Vec::new();
    emit(&mut events, format!("{EV_START}:{:?}", case.op));
    if let MaxEventsCase::Some(0) = case.max_events { emit(&mut events, EV_INVALID_MAX); return OverrideOutcome { applied: false, events, follow_chain: vec![] }; }
    if let MaxEventsCase::Some(n) = case.max_events { emit(&mut events, format!("{EV_PREFIX}:max={}", n)); }
    if matches!(case.idempotent, IdempotentFlag::Yes) { emit(&mut events, format!("{EV_PREFIX}:idempotent")); }
    let mut follow_chain = Vec::new();
    let hops = follow_hops(case);
    for i in 1..=hops { let node = format!("fhop{}", i); emit(&mut events, format!("http:follow:{}", node)); follow_chain.push(node); }
    emit(&mut events, EV_APPLIED);
    OverrideOutcome { applied: true, events, follow_chain }
}

#[cfg(test)]
mod tests_http_override_stub_smoke {
    use super::*;
    #[test]
    fn smoke_http_override_cases_non_empty() {
        for c in http_override_cases() { let out = run_http_override(&c); assert!(out.is_applied()); assert_eq!(out.terminal_event(), Some("http:override:applied")); }
    }

    #[test]
    fn cases_filter_by_op() { use crate::common::git_scenarios::GitOp; assert!(http_override_cases_for(GitOp::Clone).iter().all(|c| matches!(c.op, GitOp::Clone))); }

    #[test]
    fn follow_chain_length_rule() {
        let with_follow = http_override_cases().into_iter().filter(|c| matches!(c.follow, FollowMode::Follow));
        for c in with_follow { let out = run_http_override(&c); let hops = follow_hops(&c); assert_eq!(out.follow_chain.len(), hops, "follow hops mismatch for {c}"); }
    }
}
