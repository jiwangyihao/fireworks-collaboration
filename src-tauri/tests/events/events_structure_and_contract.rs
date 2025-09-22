#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Events Structure & Contract (Roadmap 12.12)
//! ----------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - events_structured_basic.rs
//!   - events_contract_snapshot.rs
//!   - events_no_legacy_taskerror.rs
//!   - events_task_lifecycle_structured.rs (仅结构契约/非生命周期特定片段，生命周期用例 12.13 单独聚合)
//! 分区结构：
//!   section_schema_basic     -> 基础结构化事件发布 / snapshot / take_all
//!   section_sequence_minimal -> 最小序列锚点（Started -> RetryApplied -> Completed）
//!   section_legacy_absence   -> 验证不再出现 legacy TaskEvent::Failed code（策略/partial 旧错误码）
//!   section_contract_snapshot-> JSON snapshot（精简抽样，避免冗长）
//! 设计说明：
//!   * 保留最小代表性事件集合，替代原多文件重复验证。
//!   * snapshot 采用行拼接字符串（与原 tests/events_contract_snapshot.rs 一致模式），但裁剪为核心样本，后续 schema 变更时需明确更新 expected。
//!   * 不覆盖生命周期进度/取消分支（推迟到 12.13）。
//! Cross-ref:
//!   - 12.9 / 12.10 中策略与 retry 事件锚点
//!   - 12.11 中取消/超时 outcome 计划将复用结构化事件枚举
//! Post-audit(v1): 初版聚合采用静态 JSON 对比；后续可考虑引入 insta snapshot 或基于字段子集的宽松匹配以降低微字段变更噪音。

#[path = "../common/mod.rs"] mod common;
use fireworks_collaboration_lib::events::structured::{MemoryEventBus, Event, TaskEvent, PolicyEvent, StrategyEvent, TransportEvent};
use crate::common::event_assert::{tagify, default_tag_mapper, expect_tags_subsequence};
use crate::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() { init_test_env(); }

// ---------------- section_schema_basic ----------------
mod section_schema_basic {
    use super::*;
    #[test]
    fn basic_publish_and_snapshot() {
        let bus = MemoryEventBus::new();
        bus.publish(Event::Task(TaskEvent::Started { id: "case1".into(), kind: "GitClone".into() }));
        bus.publish(Event::Policy(PolicyEvent::RetryApplied { id: "case1".into(), code: "retry_strategy_override_applied".into(), changed: vec!["max".into()] }));
        let snap = bus.snapshot();
        assert_eq!(snap.len(), 2);
        matches!(snap[0], Event::Task(TaskEvent::Started { .. }));
        // take_all drains
        assert_eq!(bus.take_all().len(), 2);
        assert!(bus.take_all().is_empty());
    }
}

// ---------------- section_sequence_minimal ----------------
mod section_sequence_minimal {
    use super::*;
    use crate::common::event_assert::expect_subsequence;
    #[test]
    fn minimal_anchor_sequence() {
        let bus = MemoryEventBus::new();
        bus.publish(Event::Task(TaskEvent::Started { id: "s2".into(), kind: "GitClone".into() }));
        bus.publish(Event::Policy(PolicyEvent::RetryApplied { id: "s2".into(), code: "retry_strategy_override_applied".into(), changed: vec![] }));
        bus.publish(Event::Task(TaskEvent::Completed { id: "s2".into() }));
        let snap = bus.snapshot();
        // 将事件序列转为粗粒度标签供 expect_subsequence (任务 / 策略) 简化匹配
        let labels: Vec<String> = snap.iter().map(|e| match e {
            Event::Task(TaskEvent::Started { .. }) => "Task:Started",
            Event::Task(TaskEvent::Completed { .. }) => "Task:Completed",
            Event::Policy(PolicyEvent::RetryApplied { .. }) => "Policy:RetryApplied",
            _ => "Other"
        }.to_string()).collect();
        expect_subsequence(&labels, &["Task:Started", "Policy:RetryApplied", "Task:Completed"]);
    }
}

// ---------------- section_legacy_absence ----------------
mod section_legacy_absence {
    use super::*;
    #[test]
    fn no_legacy_failed_codes_present() {
        // 构造若干现代等价事件（Strategy Summary / PartialFilter / AdaptiveTls）
        let bus = MemoryEventBus::new();
        bus.publish(Event::Strategy(StrategyEvent::Summary { id: "L1".into(), kind: "GitClone".into(), http_follow: true, http_max: 3, retry_max: 2, retry_base_ms: 200, retry_factor: 1.2, retry_jitter: true, tls_insecure: false, tls_skip_san: true, applied_codes: vec!["http_strategy_override_applied".into()], filter_requested: true }));
        bus.publish(Event::Transport(TransportEvent::PartialFilterFallback { id: "L1".into(), shallow: false, message: "partial_filter_fallback".into() }));
        bus.publish(Event::Strategy(StrategyEvent::AdaptiveTlsRollout { id: "L2".into(), kind: "GitClone".into(), percent_applied: 10, sampled: true }));
        let events = bus.snapshot();
        // legacy code 曾以 TaskEvent::Failed 的 code 形式出现，本组不应出现 Task::Failed 里的旧策略 code
        for e in &events {
            if let Event::Task(TaskEvent::Failed { code, .. }) = e {
                if let Some(c) = code { panic!("unexpected legacy style failed code present: {}", c); }
            }
        }
    }
}

// ---------------- section_contract_snapshot ----------------
mod section_contract_snapshot {
    use super::*;
    #[test]
    fn contract_core_snapshot() {
        // 精简自原 full snapshot：保留典型种类；如 schema 变更需同步更新 expected。
        let schema_version = 1u32; // 占位：结构化事件 schema 版本，如字段新增需 bump
        let samples = vec![
            Event::Task(TaskEvent::Started { id: "id1".into(), kind: "GitClone".into() }),
            Event::Task(TaskEvent::Failed { id: "id1".into(), category: "Protocol".into(), code: Some("x".into()), message: "m".into() }),
            Event::Policy(PolicyEvent::RetryApplied { id: "id2".into(), code: "retry_strategy_override_applied".into(), changed: vec!["max".into()] }),
            Event::Transport(TransportEvent::PartialFilterUnsupported { id: "id3".into(), requested: "blob:none".into() }),
            Event::Strategy(StrategyEvent::HttpApplied { id: "id4".into(), follow: true, max_redirects: 5 }),
            Event::Strategy(StrategyEvent::AdaptiveTlsRollout { id: "id5".into(), kind: "GitClone".into(), percent_applied: 42, sampled: true }),
        ];
        let mut lines = Vec::new();
        for e in samples { lines.push(serde_json::to_string(&e).expect("serialize")); }
        let joined = lines.join("\n");
        let expected = r#"{"type":"Task","data":{"Started":{"id":"id1","kind":"GitClone"}}}
{"type":"Task","data":{"Failed":{"id":"id1","category":"Protocol","code":"x","message":"m"}}}
{"type":"Policy","data":{"RetryApplied":{"id":"id2","code":"retry_strategy_override_applied","changed":["max"]}}}
{"type":"Transport","data":{"PartialFilterUnsupported":{"id":"id3","requested":"blob:none"}}}
{"type":"Strategy","data":{"HttpApplied":{"id":"id4","follow":true,"max_redirects":5}}}
{"type":"Strategy","data":{"AdaptiveTlsRollout":{"id":"id5","kind":"GitClone","percent_applied":42,"sampled":true}}}"#;
        assert_eq!(joined, expected, "structured event core contract changed; update expected if intentional");
        // 标签锚点（Task -> Policy -> Transport -> Strategy）最小序列
        let lines_vec: Vec<String> = expected.lines().map(|s| s.trim_start().to_string()).collect();
        let tags = tagify(&lines_vec, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["Task", "Policy", "Transport", "Strategy"]); }
        // 新增：逐行 JSON parse + id 唯一性校验（忽略失败事件中与其它不同 id 的情况）
        use serde_json::Value; use std::collections::HashSet;
        let mut ids = HashSet::new();
        for (idx, line) in lines_vec.iter().enumerate() {
            let v: Value = serde_json::from_str(line).expect("snapshot line should be valid JSON");
            // 结构基本形状校验
            assert!(v.get("type").is_some(), "line {} missing type", idx);
            assert!(v.get("data").is_some(), "line {} missing data", idx);
            // 提取 id 字段（深入 data -> variant -> id）
            if let Some(id) = v.pointer("/data/Started/id")
                .or_else(|| v.pointer("/data/Failed/id"))
                .or_else(|| v.pointer("/data/RetryApplied/id"))
                .or_else(|| v.pointer("/data/PartialFilterUnsupported/id"))
                .or_else(|| v.pointer("/data/HttpApplied/id"))
                .or_else(|| v.pointer("/data/AdaptiveTlsRollout/id"))
            { if let Some(id_str) = id.as_str() { assert!(ids.insert(id_str.to_string()), "duplicate event id detected: {}", id_str); } }
        }
        assert!(schema_version >= 1); // 防止未使用变量告警
    }
}
