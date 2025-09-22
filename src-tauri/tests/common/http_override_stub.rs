//! http_override_stub: 12.10 前置占位，提供 HTTP Override / Strategy Override 测试所需最小结构。
//! 目标：统一描述 follow / max_events / idempotent 组合，生成事件向量用于早期断言。
//! 后续替换：接入真实实现后改为调用生产接口，并返回结构化事件。

use std::fmt::{Display, Formatter};
use crate::common::git_scenarios::GitOp;

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

#[derive(Debug, Clone)]
pub struct OverrideOutcome {
    pub applied: bool,
    pub events: Vec<String>,
    pub follow_chain: Vec<String>,
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

/// 占位执行：根据 case 生成伪事件。后续将替换为真实 override 流执行。
pub fn run_http_override(case: &HttpOverrideCase) -> OverrideOutcome {
    let mut events = Vec::new();
    events.push(format!("http:override:start:{:?}", case.op));
    if let MaxEventsCase::Some(0) = case.max_events {
        events.push("http:override:invalid_max".into());
        return OverrideOutcome { applied: false, events, follow_chain: Vec::new() };
    }
    if let MaxEventsCase::Some(n) = case.max_events { events.push(format!("http:override:max={}", n)); }
    if matches!(case.idempotent, IdempotentFlag::Yes) { events.push("http:override:idempotent".into()); }
    let mut follow_chain = Vec::new();
    if matches!(case.follow, FollowMode::Follow) {
        // 模拟 follow 链路：数量与 max_events 或固定 2 相关
        let hops = match case.max_events { MaxEventsCase::Some(n) => (n.min(3)).max(1), MaxEventsCase::None => 2 };
        for i in 1..=hops { let node = format!("fhop{}", i); events.push(format!("http:follow:{}", node)); follow_chain.push(node); }
    }
    events.push("http:override:applied".into());
    OverrideOutcome { applied: true, events, follow_chain }
}

#[cfg(test)]
mod tests_http_override_stub_smoke {
    use super::*;
    #[test]
    fn smoke_http_override_cases_non_empty() {
        for c in http_override_cases() { let out = run_http_override(&c); assert!(out.applied); assert!(!out.events.is_empty(), "events empty for {c}"); }
    }
}
