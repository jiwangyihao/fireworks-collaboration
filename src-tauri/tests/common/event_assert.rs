//! event_assert: 事件断言工具（占位演进中）。
//! 提供能力（当前阶段最小集合）：
//!  - contains: 字符串事件子串存在性
//!  - subsequence: 锚点子序列（字符串层）
//!  - tag DSL (新增 12.8~12.12 Post-audit 增量)：将原始事件行映射为抽象标签，降低后续结构化事件迁移成本。
//! 未来（在 12.13+ 阶段）将：
//!  - 引入结构化枚举匹配（避免字符串 contains）
//!  - 支持可选/重复段 & 锚点集合
//!  - 与真实事件类型集成 (Task/Policy/Strategy/Transport/...)
//! 设计原则：保持向后兼容——旧 expect_subsequence/contains API 不移除，便于渐进迁移。

#[derive(Debug, Clone)]
pub struct EventPhase<'a>(pub &'a str);

// ---- Tag DSL 最小实现 ----
/// 抽象事件标签（当前为简单包装 &str，可扩展为枚举）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventTag(pub String);

impl EventTag {
    pub fn new<S: Into<String>>(s: S) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

/// 将原始字符串事件映射为标签的函数类型。
pub type TagMapper = fn(&str) -> Option<EventTag>;

/// 默认映射（启发式）：
///  - 识别前缀（task:, pre:, cancel:, timeout:, strategy:, http:, tls:, retry:, progress:, policy:, transport:）
///  - 提取前缀作为标签；特殊处理 attempt#/result: 等模式。
pub fn default_tag_mapper(line: &str) -> Option<EventTag> {
    let prefixes = [
        "task:", "pre:check:", "cancel:requested:", "timeout:", "strategy:",
        "http:override", "tls:rollout", "attempt#", "result:", "progress:",
        "retry_", "policy:", "transport:", "metric:",
    ];
    for p in prefixes {
        if line.starts_with(p) || line.contains(p) { // contains 兼容某些中部 anchor
            // attempt#/result: 保留后缀关键片段
            if line.starts_with("attempt#") {
                // attempt#1 -> Attempt
                return Some(EventTag::new("Attempt"));
            }
            if line.starts_with("result:") { return Some(EventTag::new(&line[..line.find(' ').unwrap_or(line.len())])); }
            if let Some(idx) = line.find(':') { return Some(EventTag::new(&line[..idx])); }
        }
    }
    None
}

/// 批量将事件行映射为标签；忽略映射失败的行。
pub fn tagify<'a, I: IntoIterator<Item=&'a String>>(events: I, mapper: TagMapper) -> Vec<EventTag> {
    events.into_iter().filter_map(|e| mapper(e)).collect()
}

/// 标签子序列匹配（与 expect_subsequence 一致逻辑，但操作 EventTag）。
pub fn expect_tags_subsequence(tags: &[EventTag], anchors: &[&str]) {
    let mut pos = 0usize;
    for &a in anchors {
        let mut found = None;
        for (idx, tag) in tags.iter().enumerate().skip(pos) {
            if tag.as_str().contains(a) { found = Some(idx); break; }
        }
        match found {
            Some(i) => pos = i + 1,
            None => panic!("[event-assert] tag subsequence anchor '{}' not found after index {}", a, if pos==0 {0usize.saturating_sub(1)} else {pos-1}),
        }
    }
}

/// 终态互斥断言：确保 expected 子串至少出现一次，且 forbidden 任意一个都不出现。
pub fn assert_terminal_exclusive(events: &[String], expected: &str, forbidden: &[&str]) {
    let has_expected = events.iter().any(|e| e.contains(expected));
    assert!(has_expected, "[event-assert] expected terminal '{}' missing", expected);
    for f in forbidden { assert!(!events.iter().any(|e| e.contains(f)), "[event-assert] forbidden terminal '{}' found alongside '{}'", f, expected); }
}

/// 断言事件列表（目前为字符串）包含所有期望子串。
pub fn assert_contains_phases(events: &[String], phases: &[EventPhase<'_>]) {
    for p in phases {
        let found = events.iter().any(|e| e.contains(p.0));
        assert!(found, "[event-assert] missing phase substring: {}", p.0);
    }
}

/// 断言结尾阶段包含指定子串（锚点式）。
pub fn assert_last_phase_contains(events: &[String], expected: &str) {
    let last = events.last().expect("events non-empty");
    assert!(last.contains(expected), "[event-assert] last phase {:?} !contains {:?}", last, expected);
}

/// 简单子序列匹配：按顺序确认每个锚点字符串在后续事件中第一次出现。
/// 未来将升级为结构化事件类型匹配，并支持可选/重复段模式。
pub fn expect_subsequence(events: &[String], anchors: &[&str]) {
    let mut pos = 0usize;
    for &a in anchors {
        let mut found = None;
        for (idx, ev) in events.iter().enumerate().skip(pos) {
            if ev.contains(a) { found = Some(idx); break; }
        }
        match found {
            Some(i) => { pos = i + 1; },
            None => {
                panic!("[event-assert] subsequence anchor '{}' not found after index {}", a, if pos==0 {0usize.saturating_sub(1)} else {pos-1});
            }
        }
    }
}

// ---- 结构化事件过渡占位 ----
/// 未来将把生产事件枚举 (Event::Task/Policy/Strategy/...) 转换为统一标签；
/// 目前提供占位函数，允许测试代码在升级前后保持调用点不变。
/// 当前实现仅直接调用 tagify 以保证兼容。
pub fn structured_tags<'a, I: IntoIterator<Item=&'a String>>(events: I) -> Vec<EventTag> {
    tagify(events, default_tag_mapper)
}

/// 结构化锚点子序列断言占位：等价于 expect_tags_subsequence。
pub fn expect_structured_sequence(tags: &[EventTag], anchors: &[&str]) {
    expect_tags_subsequence(tags, anchors)
}

// ---------------------------------------------------------------------------
// Structured Event Helpers (migrated from support/event_assert.rs)
// 提供针对生产结构化事件枚举的快照、筛选与语义断言。
// 后续若 common 事件 DSL 升级为直接基于枚举，可融合精简。
// ---------------------------------------------------------------------------
// allow(dead_code): 结构化事件辅助函数在部分测试阶段可能未被全部引用
use fireworks_collaboration_lib::events::structured::{Event, TaskEvent, PolicyEvent, StrategyEvent, TransportEvent, get_global_memory_bus};

/// 获取当前全局 MemoryEventBus 的快照事件；若不存在则返回空 Vec。
pub fn snapshot_events() -> Vec<Event> { get_global_memory_bus().map(|b| b.snapshot()).unwrap_or_default() }
/// 查找是否存在满足谓词的事件。
pub fn has_event<F: Fn(&Event) -> bool>(pred: F) -> bool { snapshot_events().iter().any(pred) }
/// 收集所有策略事件便于测试遍历。
pub fn collect_policy() -> Vec<PolicyEvent> { snapshot_events().into_iter().filter_map(|e| match e { Event::Policy(p)=>Some(p), _=>None }).collect() }
/// 收集 TaskEvent
pub fn collect_task() -> Vec<TaskEvent> { snapshot_events().into_iter().filter_map(|e| match e { Event::Task(t)=>Some(t), _=>None }).collect() }
/// 简单断言：存在指定 code 的 PolicyEvent (例如 retry_strategy_override_applied)
pub fn assert_policy_code(code: &str) { let all = collect_policy(); assert!(all.iter().any(|p| matches!(p, PolicyEvent::RetryApplied{code: c,..} if c==code)), "expected policy event code={code}, got={all:?}"); }
/// 查找所有 RetryApplied 事件，返回 (id, changed)
pub fn retry_applied_matrix() -> Vec<(String, Vec<String>)> { collect_policy().into_iter().filter_map(|p| match p { PolicyEvent::RetryApplied { id, changed, .. } => Some((id, changed)) }).collect() }
/// 查找策略 Summary 事件
pub fn collect_strategy_summary() -> Vec<StrategyEvent> { snapshot_events().into_iter().filter_map(|e| match e { Event::Strategy(s @ StrategyEvent::Summary { .. }) => Some(s), _=>None }).collect() }
pub fn collect_transport_partial_fallback() -> Vec<(String,bool)> { snapshot_events().into_iter().filter_map(|e| match e { Event::Transport(TransportEvent::PartialFilterFallback { id, shallow, .. }) => Some((id, shallow)), _=>None }).collect() }
/// Strategy 冲突事件收集与断言
pub fn collect_strategy_conflicts() -> Vec<(String,String,String)> { snapshot_events().into_iter().filter_map(|e| match e { Event::Strategy(StrategyEvent::Conflict { id, kind, message }) => Some((id, kind, message)), _ => None }).collect() }
pub fn assert_conflict_kind(id: &str, kind: &str, msg_contains: Option<&str>) {
    let all = collect_strategy_conflicts();
    let matches: Vec<_> = all.iter().filter(|(cid, ck, _)| cid==id && ck==kind).collect();
    assert!(!matches.is_empty(), "expected StrategyEvent::Conflict id={id} kind={kind}, got={all:?}");
    if let Some(m) = msg_contains { assert!(matches.iter().any(|(_,_,msg)| msg.contains(m)), "expected conflict message to contain '{m}' got={matches:?}"); }
}
/// Strategy Summary applied_codes 断言
pub fn summary_applied_codes(id: &str) -> Vec<String> { snapshot_events().into_iter().filter_map(|e| match e { Event::Strategy(StrategyEvent::Summary { id: sid, applied_codes, .. }) if sid==id => Some(applied_codes), _ => None }).flatten().collect() }
pub fn assert_applied_code(id: &str, code: &str) { let codes = summary_applied_codes(id); assert!(codes.iter().any(|c| c==code), "expected applied code {code} for task {id}, got {codes:?}"); }
pub fn assert_no_applied_code(id: &str, code: &str) { let codes = summary_applied_codes(id); assert!(!codes.iter().any(|c| c==code), "did not expect applied code {code} for task {id}, got {codes:?}"); }
/// Transport PartialFilter Capability / Unsupported
pub fn collect_transport_partial_capability() -> Vec<(String,bool)> { snapshot_events().into_iter().filter_map(|e| match e { Event::Transport(TransportEvent::PartialFilterCapability { id, supported }) => Some((id, supported)), _=>None }).collect() }
pub fn collect_transport_partial_unsupported() -> Vec<(String,String)> { snapshot_events().into_iter().filter_map(|e| match e { Event::Transport(TransportEvent::PartialFilterUnsupported { id, requested }) => Some((id, requested)), _=>None }).collect() }
pub fn assert_partial_capability(id: &str, expect_supported: bool) {
    let all = collect_transport_partial_capability();
    let hit: Vec<_> = all.iter().filter(|(tid, _)| tid==id).collect();
    assert!(!hit.is_empty(), "expected PartialFilterCapability event for {id}, got none: all={all:?}");
    assert!(hit.iter().any(|(_, s)| *s==expect_supported), "expected supported={expect_supported} for {id}, got={hit:?}");
}
pub fn assert_no_partial_capability(id: &str) { let all = collect_transport_partial_capability(); assert!(!all.iter().any(|(tid, _)| tid==id), "did not expect PartialFilterCapability for {id}, got={all:?}"); }
pub fn assert_partial_unsupported(id: &str, requested_contains: Option<&str>) {
    let all = collect_transport_partial_unsupported();
    let matches: Vec<_> = all.iter().filter(|(tid, _)| tid==id).collect();
    assert!(!matches.is_empty(), "expected PartialFilterUnsupported for {id}, got none: all={all:?}");
    if let Some(pat) = requested_contains { assert!(matches.iter().any(|(_, r)| r.contains(pat)), "expected requested to contain '{pat}' got matches={matches:?}"); }
}
pub fn assert_no_partial_unsupported(id: &str) { let all = collect_transport_partial_unsupported(); assert!(!all.iter().any(|(tid, _)| tid==id), "did not expect PartialFilterUnsupported for {id}, got={all:?}"); }
pub fn assert_partial_fallback(id:&str, expect_shallow:Option<bool>) {
    let all = collect_transport_partial_fallback();
    let found: Vec<_> = all.iter().filter(|(tid, _)| tid==id).collect();
    assert!(!found.is_empty(), "expected partial filter fallback event for task {id}, got none: all={all:?}");
    if let Some(s) = expect_shallow { assert!(found.iter().any(|(_,sh)| *sh==s), "expected shallow={s} in fallback events for {id}, got={found:?}"); }
}
pub fn assert_no_partial_fallback(id:&str) { let all = collect_transport_partial_fallback(); assert!(!all.iter().any(|(tid, _)| tid==id), "did not expect partial filter fallback for task {id}, but found: {all:?}"); }
/// 统计指定 task id 的生命周期计数
pub fn task_lifecycle_counters(id: &str) -> (usize, usize, usize, usize) {
    let mut started=0; let mut completed=0; let mut canceled=0; let mut failed=0;
    for t in collect_task() { match t { TaskEvent::Started { id: tid, .. } if tid==id => started+=1, TaskEvent::Completed { id: tid } if tid==id => completed+=1, TaskEvent::Canceled { id: tid } if tid==id => canceled+=1, TaskEvent::Failed { id: tid, .. } if tid==id => failed+=1, _=>{} } }
    (started, completed, canceled, failed)
}
/// Debug 打印
pub fn debug_dump() { for e in snapshot_events() { eprintln!("STRUCTURED_EVENT: {:?}", e); } }

#[cfg(test)]
mod tests_event_assert_smoke {
    use super::*;
    #[test]
    fn smoke_assert_phases() {
        let events = vec!["Init".into(), "Enumerate".into(), "Complete".into()];
        assert_contains_phases(&events, &[EventPhase("Init"), EventPhase("Complete")]);
        assert_last_phase_contains(&events, "Complete");
    }

    #[test]
    fn smoke_tag_subsequence() {
        let events = vec![
            "task:start:Clone".into(),
            "progress:10%".into(),
            "cancel:requested:midway".into(),
            "task:end:cancelled".into(),
        ];
        let tags = tagify(&events, default_tag_mapper);
        expect_tags_subsequence(&tags, &["task", "cancel", "task"]);
    }
}
