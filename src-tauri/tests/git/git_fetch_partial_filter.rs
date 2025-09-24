#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Fetch Partial Filter (12.8 完成度提升)
//! --------------------------------------------------
//! 目标：
//! - 使用公共 `run_fetch`（tests/common/git_scenarios.rs）替换 clone 前缀模拟；
//! - 合并相似分区：variants/depth/invalid/no-filter 统一由矩阵驱动；
//! - 复用 `partial_filter_support` 的判定与统一告警；
//! - 最小 Tag 断言：存在 fetch 前缀事件。
//! 后续（真实 fetch 接入后）：收紧无 marker -> 断言、加入对象/提交计数与 capability 协商断言。

#[path = "../common/mod.rs"]
mod common;

use common::{partial_filter_support::{assess_partial_filter, SupportLevel, warn_if_no_filter_marker}, partial_filter_matrix::{partial_filter_cases_for, PartialFilterOp, PartialFilterKind, PartialFilterCase}, test_env, event_assert::{tagify, default_tag_mapper, expect_tags_subsequence}};
use common::git_scenarios::{run_fetch, FetchParams};

fn build_label(case: &PartialFilterCase) -> Option<&'static str> {
    use PartialFilterKind::*;
    match case.kind {
        EventOnly => Some("event-only"),
        CodeOnly => Some("code-only"),
        Structure => Some("structure"),
        CodeWithDepth => Some("code+depth"),
        EventWithDepth => Some("event+depth"),
        NoFilter => None,
        InvalidFilter => Some("bad:filter"),
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
            let params = FetchParams { depth: case.depth, filter: label_opt.map(|l| format!("filter:{}", l)) };
            let events = run_fetch(&params).events;
            assert!(!events.is_empty(), "[fetch_capability] events non-empty for {case}");
            let f_label = label_opt.unwrap_or("");
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
            let case = PartialFilterCase { op: PartialFilterOp::Fetch, kind, depth: None };
            let label_opt = build_label(&case);
            let params = FetchParams { depth: None, filter: label_opt.map(|l| format!("filter:{}", l)) };
            let events = run_fetch(&params).events;
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
            let params = FetchParams { depth: case.depth, filter: label_opt.map(|l| format!("filter:{}", l)) };
            let events = run_fetch(&params).events;
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
        let params = FetchParams { depth: None, filter: label_opt.map(|l| format!("filter:{}", l)) };
        let events = run_fetch(&params).events;
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
        let params = FetchParams { depth: None, filter: Some("filter:unsupported-case".into()) };
        let events = run_fetch(&params).events;
        assert!(!events.is_empty(), "[fetch_fallback] events non-empty");
        let out = assess_partial_filter("filter:unsupported-case", None, &events);
        assert!(matches!(out.support, SupportLevel::Unsupported), "expected Unsupported got {:?}", out.support);
        warn_if_no_filter_marker("fetch_fallback", "filter:unsupported-case", &out);
    }
}
