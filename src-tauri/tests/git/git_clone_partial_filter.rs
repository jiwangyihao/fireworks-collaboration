#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Clone Partial Filter (Roadmap 12.6)
//! ------------------------------------------------------------
//! 迁移来源（legacy 占位将保留）：
//!   - git_partial_clone_filter_capable.rs
//!   - git_partial_clone_filter_event_baseline.rs
//!   - git_partial_clone_filter_event_code.rs
//!   - git_partial_clone_filter_event_code_with_depth.rs
//!   - git_partial_clone_filter_event_only.rs
//!   - git_partial_clone_filter_event_structure.rs
//!   - git_partial_clone_filter_event_with_depth.rs
//!   - git_partial_clone_filter_fallback.rs
//! 分区结构：
//!   section_capability   -> capability 探测（支持与不支持占位）
//!   section_filter_event -> event/code/structure 基础过滤类型差异
//!   section_filter_depth -> 与 depth 交叉 (code_with_depth / event_with_depth)
//!   section_fallback     -> 不支持 capability 时的回退行为
//! 设计要点：
//!   * 使用 `common::partial_filter_matrix` 集中描述 representative cases。
//!   * 由于当前底层实现尚无真实 partial filter 语义，这里模拟：传入 filter 参数时事件向量包含一个 "Filter:" 前缀元素。
//!   * depth 交叉与 shallow 行为尚未统一（待 12.7/12.8），此处仅断言不 panic + 事件存在。
//!   * fallback 场景使用布尔模拟（假设当 filter 包含 "unsupported" 子串则 fallback=true）。
//! 后续改进：
//!   * 引入真实服务端 capability 探测后，用明确枚举断言 capability 与 fallback 分类。
//!   * 与 shallow depth helper 共享对象/提交计数逻辑。
//!   * 使用事件 DSL 替换当前字符串 contains 断言。
//! Post-audit: 首次聚合提交，不做“完整语义”校验，只保证结构、矩阵与占位；避免假阳性。
//! Post-audit(v2): fallback 布尔与 filter:* 事件存在性检查为临时宽松策略，
//! 计划在 12.8 与 fetch partial filter 聚合时升级为枚举 SupportLevel + 事件 DSL。

#[path = "../common/mod.rs"]
mod common;

// ---------------- section_capability ----------------
mod section_capability {
    use crate::common::{partial_filter_matrix::{partial_filter_cases, PartialFilterCase, PartialFilterKind}, partial_filter_support::{assess_partial_filter, SupportLevel, warn_if_no_filter_marker}, test_env};
    use crate::common::git_scenarios::{CloneParams, run_clone};

    fn build_params_for_case(case: &PartialFilterCase) -> CloneParams {
        match case.kind {
            PartialFilterKind::EventOnly => CloneParams { filter: Some("filter:event-only".into()), ..Default::default() },
            PartialFilterKind::CodeOnly => CloneParams { filter: Some("filter:code-only".into()), ..Default::default() },
            PartialFilterKind::Structure => CloneParams { filter: Some("filter:structure".into()), ..Default::default() },
            PartialFilterKind::CodeWithDepth => CloneParams { depth: case.depth, filter: Some("filter:code+depth".into()), ..Default::default() },
            PartialFilterKind::EventWithDepth => CloneParams { depth: case.depth, filter: Some("filter:event+depth".into()), ..Default::default() },
            PartialFilterKind::NoFilter => CloneParams { filter: None, ..Default::default() },
            PartialFilterKind::InvalidFilter => CloneParams { filter: Some("filter:bad:filter".into()), ..Default::default() },
        }
    }

    #[test]
    fn clone_partial_capability_each_case_support_level_not_unsupported() {
        test_env::init_test_env();
        for case in partial_filter_cases() {
            let params = build_params_for_case(&case);
            let out = run_clone(&params);
            let outcome = assess_partial_filter(params.filter.as_deref().unwrap_or(""), params.depth, &out.events);
            assert!(!out.events.is_empty(), "[capability] events should exist for case {case}");
            match outcome.support {
                SupportLevel::Unsupported => panic!("[capability] case {case} unexpectedly Unsupported"),
                _ => { /* Supported / DegradedPlaceholder 均接受（占位） */ }
            }
            warn_if_no_filter_marker("capability", &params.filter.clone().unwrap_or_default(), &outcome);
        }
    }
}

// ---------------- section_filter_event ----------------
mod section_filter_event {
    use crate::common::{git_scenarios::{CloneParams, run_clone}, partial_filter_support::{assess_partial_filter, SupportLevel, warn_if_no_filter_marker}, test_env};

    fn run_filter(label: &str) -> (CloneParams, Vec<String>) {
        let params = CloneParams { filter: Some(format!("filter:{}", label)), ..Default::default() };
        let events = run_clone(&params).events;
        (params, events)
    }

    #[test]
    fn event_only_vs_code_only_support_level() {
        test_env::init_test_env();
        let (p_ev, ev) = run_filter("event-only");
        let (p_code, code) = run_filter("code-only");
        assert!(!ev.is_empty() && !code.is_empty(), "[filter_event] events present for event/code");
        let o_ev = assess_partial_filter(p_ev.filter.as_deref().unwrap(), p_ev.depth, &ev);
        let o_code = assess_partial_filter(p_code.filter.as_deref().unwrap(), p_code.depth, &code);
    assert!(!matches!(o_ev.support, SupportLevel::Unsupported), "event-only unsupported? unexpected");
    assert!(!matches!(o_code.support, SupportLevel::Unsupported), "code-only unsupported? unexpected");
    warn_if_no_filter_marker("filter_event", p_ev.filter.as_deref().unwrap_or("") , &o_ev);
    warn_if_no_filter_marker("filter_event", p_code.filter.as_deref().unwrap_or("") , &o_code);
    }

    #[test]
    fn structure_case_support_level() {
        test_env::init_test_env();
        let (params, ev) = run_filter("structure");
        assert!(!ev.is_empty(), "[filter_event] structure events non-empty");
        let outcome = assess_partial_filter(params.filter.as_deref().unwrap(), params.depth, &ev);
    if matches!(outcome.support, SupportLevel::Unsupported) { panic!("structure unexpectedly unsupported"); }
    warn_if_no_filter_marker("filter_event", params.filter.as_deref().unwrap_or("") , &outcome);
    }
}

// ---------------- section_filter_depth ----------------
mod section_filter_depth {
    use crate::common::{git_scenarios::{CloneParams, run_clone}, partial_filter_support::{assess_partial_filter, SupportLevel, warn_if_no_filter_marker}, test_env};

    #[test]
    fn code_with_depth_support_outcome() {
        test_env::init_test_env();
        let params = CloneParams { depth: Some(1), filter: Some("filter:code+depth".into()), ..Default::default() };
        let out = run_clone(&params);
        let outcome = assess_partial_filter(params.filter.as_deref().unwrap(), params.depth, &out.events);
        assert!(!out.events.is_empty(), "[filter_depth] code+depth events");
    if matches!(outcome.support, SupportLevel::Unsupported) { panic!("code+depth unexpectedly unsupported"); }
    warn_if_no_filter_marker("filter_depth", params.filter.as_deref().unwrap_or("") , &outcome);
    }

    #[test]
    fn event_with_depth_support_outcome() {
        test_env::init_test_env();
        let params = CloneParams { depth: Some(1), filter: Some("filter:event+depth".into()), ..Default::default() };
        let out = run_clone(&params);
        let outcome = assess_partial_filter(params.filter.as_deref().unwrap(), params.depth, &out.events);
        assert!(!out.events.is_empty(), "[filter_depth] event+depth events");
    if matches!(outcome.support, SupportLevel::Unsupported) { panic!("event+depth unexpectedly unsupported"); }
    warn_if_no_filter_marker("filter_depth", params.filter.as_deref().unwrap_or("") , &outcome);
    }
}

// ---------------- section_fallback ----------------
mod section_fallback {
    use crate::common::{git_scenarios::{CloneParams, run_clone}, partial_filter_support::{assess_partial_filter, SupportLevel, warn_if_no_filter_marker}, test_env};

    fn run(filter_label: &str) -> (CloneParams, Vec<String>) {
        let params = CloneParams { filter: Some(format!("filter:{}", filter_label)), ..Default::default() };
        let events = run_clone(&params).events; (params, events)
    }

    #[test]
    fn unsupported_filter_yields_unsupported_level() {
        test_env::init_test_env();
        let (params, ev) = run("unsupported-case");
        assert!(!ev.is_empty(), "[fallback] events non-empty");
    let out = assess_partial_filter(params.filter.as_deref().unwrap(), params.depth, &ev);
    assert!(matches!(out.support, SupportLevel::Unsupported), "expected Unsupported SupportLevel for unsupported-case");
    warn_if_no_filter_marker("fallback", params.filter.as_deref().unwrap_or("") , &out);
    }

    #[test]
    fn supported_filter_not_unsupported() {
        test_env::init_test_env();
        let (params, ev) = run("event-only");
        assert!(!ev.is_empty(), "[fallback] events non-empty");
    let out = assess_partial_filter(params.filter.as_deref().unwrap(), params.depth, &ev);
    assert!(!matches!(out.support, SupportLevel::Unsupported), "event-only unexpectedly Unsupported");
    warn_if_no_filter_marker("fallback", params.filter.as_deref().unwrap_or("") , &out);
    }
}
