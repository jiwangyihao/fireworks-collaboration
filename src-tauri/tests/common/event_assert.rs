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
    // 更宽泛的前缀集合 + 新增 fetch/pipeline/filter/capability/push
    const PREFIX_HINTS: &[&str] = &[
        "task:", "pre:check:", "cancel:requested", "timeout:", "strategy:",
        "http:override", "tls:", "attempt#", "result:", "progress:",
        "retry", "policy:", "transport:", "metric:", "fetch:", "pipeline:",
        "push:", "filter:", "capability:", "shallow:",
    ];

    // attempt#N 归一化为 Attempt；result:xxx 归一化为 result:xxx token
    if let Some(rest) = line.strip_prefix("attempt#") { if !rest.is_empty() { return Some(EventTag::new("Attempt")); } }
    if line.starts_with("result:") {
        let token = line.split_whitespace().next().unwrap_or(line);
        return Some(EventTag::new(token));
    }

    // 直接匹配首个冒号前 token（允许多级，例如 tls:rollout:start -> tls）
    if let Some(idx) = line.find(':') {
        let head = &line[..=idx]; // 含冒号，利于与提示列表对齐
        if PREFIX_HINTS.iter().any(|p| head.starts_with(p)) {
            return Some(EventTag::new(&head[..head.len()-1])); // 去掉尾随冒号
        }
    }

    // contains 兜底：某些 tag 可能在中部（例如 pre:check:failed）
    for p in PREFIX_HINTS { if line.contains(p) { return Some(EventTag::new(p.trim_end_matches(':'))); } }
    None
}

/// 批量将事件行映射为标签；忽略映射失败的行。
pub fn tagify<'a, I: IntoIterator<Item=&'a String>>(events: I, mapper: TagMapper) -> Vec<EventTag> {
    events.into_iter().filter_map(|e| mapper(e)).collect()
}

/// 自定义闭包版本，便于一次性捕获上下文（例如基于正则或外部映射表）
pub fn tagify_with<'a, I, F>(events: I, mut f: F) -> Vec<EventTag>
where
    I: IntoIterator<Item=&'a String>,
    F: FnMut(&str) -> Option<EventTag>,
{
    events.into_iter().filter_map(|e| f(e)).collect()
}

/// 标签子序列匹配（与 expect_subsequence 一致逻辑，但操作 EventTag）。
pub fn expect_tags_subsequence(tags: &[EventTag], anchors: &[&str]) {
    subsequence_core(tags, anchors, |t| t.as_str(), "tag")
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
    subsequence_core(events, anchors, |s| s.as_str(), "line")
}

// ---------------- Internal generic core ----------------
fn subsequence_core<T, F>(items: &[T], anchors: &[&str], project: F, kind: &str)
where
    F: Fn(&T) -> &str,
{
    let mut pos = 0usize;
    for (ai, &anchor) in anchors.iter().enumerate() {
        let mut found = None;
        for (idx, it) in items.iter().enumerate().skip(pos) {
            if project(it).contains(anchor) { found = Some(idx); break; }
        }
        match found {
            Some(i) => { pos = i + 1; },
            None => {
                // 提供上下文窗口：前后各2个元素展示 & 已匹配锚点数
                let window_start = pos.saturating_sub(2);
                let window_slice: Vec<_> = items.iter().skip(window_start).take(5).map(|it| project(it)).collect();
                panic!(
                    "[event-assert] {kind} subsequence mismatch: missing anchor '{anchor}' at step {ai} after index {}. Context(before+after)={:?} anchors={:?}",
                    pos.saturating_sub(1), window_slice, anchors
                );
            }
        }
    }
}

// （structured_tags / expect_structured_sequence 已移除：未来结构化事件阶段再引入强类型接口）


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

    #[test]
    fn tagify_with_custom_mapper() {
        let events = vec!["alpha:one".into(), "beta:two".into(), "alpha:three".into()];
        let tags = tagify_with(&events, |l| l.strip_prefix("alpha:").map(|_| EventTag::new("alpha")));
        assert_eq!(tags.iter().map(|t| t.as_str()).collect::<Vec<_>>(), vec!["alpha", "alpha"]);
        expect_tags_subsequence(&tags, &["alpha", "alpha"]);
    }

    #[test]
    #[should_panic(expected = "missing anchor")] // 断言包含核心提示
    fn subsequence_panic_contains_context() {
        let events = vec!["a:start".into(), "b:mid".into(), "c:end".into()];
        // 第二个锚点不存在
        expect_subsequence(&events, &["a:start", "z:missing"]);
    }

    #[test]
    fn terminal_exclusive_smoke_usage() {
        let events = vec!["result:success".into()];
        assert_terminal_exclusive(&events, "result:success", &["result:exhausted", "result:abort"]);
    }
}

// ---- Structured events helper (opt-in) ----
// 说明：为了避免在多个测试文件中重复实现结构化事件 -> JSON 行 / 唯一 id 校验 / 类型标签映射逻辑，
// 这里提供最小抽象；当未来引入强类型枚举断言时可直接替换内部实现。

#[allow(dead_code)] // 多个集成测试 crate 未同时使用全部 helper，避免重复 dead_code 噪音
pub mod structured_ext {
    use std::collections::HashSet;
    use fireworks_collaboration_lib::events::structured::Event;

    /// 将结构化事件序列序列化为 JSON 行（稳定排序按输入顺序）。
    pub fn serialize_events_to_json_lines(events: &[Event]) -> Vec<String> {
        events.iter()
            .map(|e| serde_json::to_string(e).expect("serialize structured event"))
            .collect()
    }

    /// 基于序列化 JSON 行验证所有出现的 id 唯一；支持多种 variant 路径。
    pub fn assert_unique_event_ids(json_lines: &[String]) {
        use serde_json::Value;
        let mut ids = HashSet::new();
        for (idx, line) in json_lines.iter().enumerate() {
            let v: Value = serde_json::from_str(line).expect("line should be valid json");
            // 遍历一层 data 下第一个对象 variant 取其 id 字段（若存在）
            if let Some(data) = v.get("data") {
                if let Some(obj) = data.as_object() {
                    if let Some((_, variant_v)) = obj.iter().next() { // 取首个 variant
                        if let Some(id) = variant_v.get("id").and_then(|idv| idv.as_str()) {
                            assert!(ids.insert(id.to_string()), "duplicate event id detected: {} (line {})", id, idx);
                        }
                    }
                }
            }
        }
    }

    /// 将结构化事件映射为顶层类型标签（Task/Policy/Strategy/Transport/...）。
    pub fn map_structured_events_to_type_tags(events: &[Event]) -> Vec<String> {
        events.iter().map(|e| {
            match e { // 依赖 Display/Debug 之外的稳定 variant 名称
                Event::Task(_) => "Task",
                Event::Policy(_) => "Policy",
                Event::Strategy(_) => "Strategy",
                Event::Transport(_) => "Transport",
            }.to_string()
        }).collect()
    }
}
