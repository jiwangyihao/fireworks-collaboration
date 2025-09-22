#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Fetch Partial Filter (Roadmap 12.8)
//! ------------------------------------------------------------
//! Cross-ref: `git_clone_partial_filter.rs`（SupportLevel 与矩阵来源），`partial_filter_support.rs`（assess_partial_filter）
//! Post-audit(v1): header 标准化（来源/结构/宽松策略）；后续计划：引入真实 run_fetch、事件 DSL、Fetch Op 枚举化（op=Fetch），收紧 DegradedPlaceholder。
//! 迁移来源（legacy 将保留占位）：
//!   - git_partial_fetch_filter_capable.rs
//!   - git_partial_fetch_filter_event_baseline.rs
//!   - git_partial_fetch_filter_event_code.rs
//!   - git_partial_fetch_filter_event_code_with_depth.rs
//!   - git_partial_fetch_filter_event_invalid_filter_no_code.rs
//!   - git_partial_fetch_filter_event_no_filter_no_code.rs
//!   - git_partial_fetch_filter_event_only.rs
//!   - git_partial_fetch_filter_event_with_depth.rs
//!   - git_partial_fetch_filter_fallback.rs
//!   - git_partial_fetch_invalid_filter_capable.rs
//! 分区结构：
//!   section_capability     -> capability 检测 (含 no-filter baseline)
//!   section_filter_variants-> event/code/structure/no_filter 差异
//!   section_filter_depth   -> 带 depth 的 filter 组合
//!   section_invalid        -> invalid filter 分类 (SupportLevel::Invalid)
//!   section_fallback       -> 不支持 fallback 行为 (SupportLevel::Unsupported)
//! 语义说明：
//!   * 仍使用启发式事件 (run_fetch 模拟) + assess_partial_filter 组合。
//!   * Fetch 与 Clone 复用 SupportLevel；将来接入真实远端 capability 协商后替换启发式。
//!   * NoFilter 与 InvalidFilter 通过矩阵衍生；Invalid 专用断言 support=Invalid。
//! 宽松策略 (占位)：
//!   * 未强制要求 filter:* marker 必存在；缺失仅打印警告。
//!   * DegradedPlaceholder depth 组合暂不区分 code 与 event 差异。
//! Post-audit(v2): 本次补充：保持与 12.7/12.9 头部格式一致；明确后续接入真实 run_fetch 后将：1) 用结构化事件替换字符串；2) 收紧无 marker 警告为断言；3) depth 交叉将校验对象/提交数量差异。

#[path = "../common/mod.rs"]
mod common;

use common::{partial_filter_support::{assess_partial_filter, SupportLevel, warn_if_no_filter_marker}, partial_filter_matrix::{partial_filter_cases_for, PartialFilterOp, PartialFilterKind, PartialFilterCase}, test_env, event_assert::{tagify, default_tag_mapper, expect_tags_subsequence, EventTag}};
use common::git_scenarios::{run_clone, CloneParams}; // fetch 暂复用 clone 事件模拟，后续接入真实 fetch 封装 (prefix events with fetch: 以示区分)

// 模拟 fetch: 复用 run_clone 以生成事件向量（未来可替换 run_fetch）
fn run_fetch_sim(filter: Option<&str>, depth: Option<u32>) -> Vec<String> {
    let params = CloneParams { filter: filter.map(|f| format!("filter:{}", f)), depth, ..Default::default() };
    let mut ev = run_clone(&params).events;
    // 为区分 clone 与 fetch 模拟，添加统一前缀（后续用真实 run_fetch 替换）
    for e in &mut ev { *e = format!("fetch:{}", e); }
    ev
}

fn build_label(case: &PartialFilterCase) -> Option<String> {
    use PartialFilterKind::*;
    match case.kind {
        PartialFilterKind::EventOnly => Some("event-only".into()),
        PartialFilterKind::CodeOnly => Some("code-only".into()),
        PartialFilterKind::Structure => Some("structure".into()),
        PartialFilterKind::CodeWithDepth => Some("code+depth".into()),
        PartialFilterKind::EventWithDepth => Some("event+depth".into()),
        PartialFilterKind::NoFilter => None,
        PartialFilterKind::InvalidFilter => Some("bad:filter".into()),
    }
}

// ---------------- section_capability ----------------
mod section_capability {
    use super::*;
    #[test]
    fn fetch_partial_capability_each_case_not_unsupported() {
        test_env::init_test_env();
    for case in partial_filter_cases_for(PartialFilterOp::Fetch) {
            let label_opt = build_label(&case);
            let events = run_fetch_sim(label_opt.as_deref(), case.depth);
            assert!(!events.is_empty(), "[fetch_capability] events non-empty for {case}");
            let f_label = label_opt.as_deref().unwrap_or("");
            let out = assess_partial_filter(&format!("filter:{}", f_label), case.depth, &events);
            if matches!(out.support, SupportLevel::Unsupported) {
                panic!("[fetch_capability] case {case} unexpectedly Unsupported");
            }
            if label_opt.is_some() { warn_if_no_filter_marker("fetch_capability", &format!("filter:{}", f_label), &out); }
        }
    }
}

// ---------------- section_filter_variants ----------------
mod section_filter_variants {
    use super::*;
    #[test]
    fn fetch_event_code_structure_no_filter_variants() {
        test_env::init_test_env();
        let variants = [PartialFilterKind::EventOnly, PartialFilterKind::CodeOnly, PartialFilterKind::Structure, PartialFilterKind::NoFilter];
        for kind in variants {
            let case = PartialFilterCase { op: PartialFilterOp::Fetch, kind, depth: None }; // 已使用 Fetch op
            let label_opt = build_label(&case);
            let events = run_fetch_sim(label_opt.as_deref(), None);
            assert!(!events.is_empty(), "[fetch_variants] events non-empty for {kind:?}");
            let filter_expr = label_opt.as_deref().map(|l| format!("filter:{}", l)).unwrap_or_else(|| "".into());
            let out = assess_partial_filter(&filter_expr, None, &events);
            if matches!(out.support, SupportLevel::Unsupported | SupportLevel::Invalid) {
                panic!("[fetch_variants] kind {kind:?} unexpected support={:?}", out.support);
            }
            if label_opt.is_some() { warn_if_no_filter_marker("fetch_variants", &filter_expr, &out); }
        }
    }
}

// ---------------- section_filter_depth ----------------
mod section_filter_depth {
    use super::*;
    #[test]
    fn fetch_code_and_event_with_depth() {
        test_env::init_test_env();
        let depth_cases = [PartialFilterKind::CodeWithDepth, PartialFilterKind::EventWithDepth];
        for kind in depth_cases {
            let case = PartialFilterCase { op: PartialFilterOp::Fetch, kind, depth: Some(1) };
            let label_opt = build_label(&case);
            let events = run_fetch_sim(label_opt.as_deref(), case.depth);
            assert!(!events.is_empty(), "[fetch_depth] events non-empty for {kind:?}");
            let label = label_opt.unwrap();
            let out = assess_partial_filter(&format!("filter:{}", label), case.depth, &events);
            if matches!(out.support, SupportLevel::Unsupported | SupportLevel::Invalid) { panic!("[fetch_depth] {kind:?} unexpected support={:?}", out.support); }
            warn_if_no_filter_marker("fetch_depth", &format!("filter:{}", label), &out);
            // 新增：标签子序列最小锚点（fetch + filter 前缀存在 -> strategy/transport 等后续可扩展）
            let tags = tagify(&events, default_tag_mapper);
            if !tags.is_empty() { expect_tags_subsequence(&tags, &["fetch"]); }
            // 占位：暂不对 depth 行为做对象/commit 数量断言
        }
    }
}

// ---------------- section_invalid ----------------
mod section_invalid {
    use super::*;
    #[test]
    fn fetch_invalid_filter_support_level_invalid() {
        test_env::init_test_env();
    let case = PartialFilterCase { op: PartialFilterOp::Fetch, kind: PartialFilterKind::InvalidFilter, depth: None };
        let label_opt = build_label(&case);
        let events = run_fetch_sim(label_opt.as_deref(), None);
        assert!(!events.is_empty(), "[fetch_invalid] events non-empty");
        let out = assess_partial_filter("filter:bad:filter", None, &events);
        assert!(matches!(out.support, SupportLevel::Invalid), "invalid filter should map to Invalid support (got {:?})", out.support);
    warn_if_no_filter_marker("fetch_invalid", "filter:bad:filter", &out);
    }
}

// ---------------- section_fallback ----------------
mod section_fallback {
    use super::*;
    #[test]
    fn fetch_unsupported_filter_yields_unsupported() {
        test_env::init_test_env();
        let events = run_fetch_sim(Some("unsupported-case"), None);
        assert!(!events.is_empty(), "[fetch_fallback] events non-empty");
        let out = assess_partial_filter("filter:unsupported-case", None, &events);
        assert!(matches!(out.support, SupportLevel::Unsupported), "expected Unsupported got {:?}", out.support);
    warn_if_no_filter_marker("fetch_fallback", "filter:unsupported-case", &out);
    }
}
