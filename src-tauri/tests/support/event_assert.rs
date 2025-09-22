//! 测试事件结构化断言辅助
#![allow(dead_code)] // 支持模块：部分 helper 在后续测试阶段会被引用，暂时允许未使用以降低噪音。
use fireworks_collaboration_lib::events::structured::{Event, TaskEvent, PolicyEvent, StrategyEvent, TransportEvent, get_global_memory_bus};

/// 获取当前全局 MemoryEventBus 的快照事件；若不存在则返回空 Vec。
pub fn snapshot_events() -> Vec<Event> {
    get_global_memory_bus().map(|b| b.snapshot()).unwrap_or_default()
}

/// 查找是否存在满足谓词的事件。
pub fn has_event<F: Fn(&Event) -> bool>(pred: F) -> bool { snapshot_events().iter().any(pred) }

/// 收集所有策略事件便于测试遍历。
pub fn collect_policy() -> Vec<PolicyEvent> {
    snapshot_events().into_iter().filter_map(|e| match e { Event::Policy(p)=>Some(p), _=>None }).collect()
}

/// 收集 TaskEvent
pub fn collect_task() -> Vec<TaskEvent> {
    snapshot_events().into_iter().filter_map(|e| match e { Event::Task(t)=>Some(t), _=>None }).collect()
}

/// 简单断言：存在指定 code 的 PolicyEvent (例如 retry_strategy_override_applied)
pub fn assert_policy_code(code: &str) {
    let all = collect_policy();
    assert!(all.iter().any(|p| matches!(p, PolicyEvent::RetryApplied{code: c,..} if c==code)), "expected policy event code={code}, got={all:?}");
}

/// 查找所有 RetryApplied 事件，返回 (id, changed)
pub fn retry_applied_matrix() -> Vec<(String, Vec<String>)> {
    collect_policy().into_iter().filter_map(|p| match p { PolicyEvent::RetryApplied { id, changed, .. } => Some((id, changed)) }).collect()
}

/// 查找策略 Summary 事件
pub fn collect_strategy_summary() -> Vec<StrategyEvent> {
    snapshot_events().into_iter().filter_map(|e| match e { Event::Strategy(s @ StrategyEvent::Summary { .. }) => Some(s), _=>None }).collect()
}

pub fn collect_transport_partial_fallback() -> Vec<(String,bool)> {
    snapshot_events().into_iter().filter_map(|e| match e { Event::Transport(TransportEvent::PartialFilterFallback { id, shallow, .. }) => Some((id, shallow)), _=>None }).collect()
}

// =========== 新增：Strategy 冲突事件收集与断言 ===========
pub fn collect_strategy_conflicts() -> Vec<(String,String,String)> {
    snapshot_events().into_iter().filter_map(|e| match e {
        Event::Strategy(StrategyEvent::Conflict { id, kind, message }) => Some((id, kind, message)),
        _ => None
    }).collect()
}

pub fn assert_conflict_kind(id: &str, kind: &str, msg_contains: Option<&str>) {
    let all = collect_strategy_conflicts();
    let matches: Vec<_> = all.iter().filter(|(cid, ck, _)| cid==id && ck==kind).collect();
    assert!(!matches.is_empty(), "expected StrategyEvent::Conflict id={id} kind={kind}, got={all:?}");
    if let Some(m) = msg_contains { assert!(matches.iter().any(|(_,_,msg)| msg.contains(m)), "expected conflict message to contain '{m}' got={matches:?}"); }
}

// =========== 新增：Strategy Summary applied_codes 断言 ===========
pub fn summary_applied_codes(id: &str) -> Vec<String> {
    snapshot_events().into_iter().filter_map(|e| match e {
        Event::Strategy(StrategyEvent::Summary { id: sid, applied_codes, .. }) if sid==id => Some(applied_codes),
        _ => None
    }).flatten().collect()
}

pub fn assert_applied_code(id: &str, code: &str) {
    let codes = summary_applied_codes(id);
    assert!(codes.iter().any(|c| c==code), "expected applied code {code} for task {id}, got {codes:?}");
}

pub fn assert_no_applied_code(id: &str, code: &str) {
    let codes = summary_applied_codes(id);
    assert!(!codes.iter().any(|c| c==code), "did not expect applied code {code} for task {id}, got {codes:?}");
}

// =========== 新增：Transport PartialFilter Capability / Unsupported ===========
pub fn collect_transport_partial_capability() -> Vec<(String,bool)> {
    snapshot_events().into_iter().filter_map(|e| match e { Event::Transport(TransportEvent::PartialFilterCapability { id, supported }) => Some((id, supported)), _=>None }).collect()
}

pub fn collect_transport_partial_unsupported() -> Vec<(String,String)> {
    snapshot_events().into_iter().filter_map(|e| match e { Event::Transport(TransportEvent::PartialFilterUnsupported { id, requested }) => Some((id, requested)), _=>None }).collect()
}

pub fn assert_partial_capability(id: &str, expect_supported: bool) {
    let all = collect_transport_partial_capability();
    let hit: Vec<_> = all.iter().filter(|(tid, _)| tid==id).collect();
    assert!(!hit.is_empty(), "expected PartialFilterCapability event for {id}, got none: all={all:?}");
    assert!(hit.iter().any(|(_, s)| *s==expect_supported), "expected supported={expect_supported} for {id}, got={hit:?}");
}

pub fn assert_no_partial_capability(id: &str) {
    let all = collect_transport_partial_capability();
    assert!(!all.iter().any(|(tid, _)| tid==id), "did not expect PartialFilterCapability for {id}, got={all:?}");
}

pub fn assert_partial_unsupported(id: &str, requested_contains: Option<&str>) {
    let all = collect_transport_partial_unsupported();
    let matches: Vec<_> = all.iter().filter(|(tid, _)| tid==id).collect();
    assert!(!matches.is_empty(), "expected PartialFilterUnsupported for {id}, got none: all={all:?}");
    if let Some(pat) = requested_contains { assert!(matches.iter().any(|(_, r)| r.contains(pat)), "expected requested to contain '{pat}' got matches={matches:?}"); }
}

pub fn assert_no_partial_unsupported(id: &str) {
    let all = collect_transport_partial_unsupported();
    assert!(!all.iter().any(|(tid, _)| tid==id), "did not expect PartialFilterUnsupported for {id}, got={all:?}");
}

pub fn assert_partial_fallback(id:&str, expect_shallow:Option<bool>) {
    let all = collect_transport_partial_fallback();
    let found: Vec<_> = all.iter().filter(|(tid, _)| tid==id).collect();
    assert!(!found.is_empty(), "expected partial filter fallback event for task {id}, got none: all={all:?}");
    if let Some(s) = expect_shallow { assert!(found.iter().any(|(_,sh)| *sh==s), "expected shallow={s} in fallback events for {id}, got={found:?}"); }
}

pub fn assert_no_partial_fallback(id:&str) {
    let all = collect_transport_partial_fallback();
    assert!(!all.iter().any(|(tid, _)| tid==id), "did not expect partial filter fallback for task {id}, but found: {all:?}");
}

/// 统计指定 task id 的生命周期计数
pub fn task_lifecycle_counters(id: &str) -> (usize, usize, usize, usize) {
    let mut started=0; let mut completed=0; let mut canceled=0; let mut failed=0;
    for t in collect_task() {
        match t {
            TaskEvent::Started { id: tid, .. } if tid==id => started+=1,
            TaskEvent::Completed { id: tid } if tid==id => completed+=1,
            TaskEvent::Canceled { id: tid } if tid==id => canceled+=1,
            TaskEvent::Failed { id: tid, .. } if tid==id => failed+=1,
            _=>{}
        }
    }
    (started, completed, canceled, failed)
}

/// Debug 打印（可在迁移阶段临时调用）
#[allow(dead_code)]
pub fn debug_dump() {
    for e in snapshot_events() { eprintln!("STRUCTURED_EVENT: {:?}", e); }
}
