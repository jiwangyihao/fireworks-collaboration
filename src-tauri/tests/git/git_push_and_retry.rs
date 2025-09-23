#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Push & Retry (Roadmap 12.9)
//! ------------------------------------------------------------
//! 来源文件：
//!   - git_push.rs
//!   - git_retry_override_event.rs
//!   - git_retry_override_event_structured.rs
//!   - git_retry_override_backoff.rs
//! Cross-ref:
//!   - `common/retry_matrix.rs` (Backoff/Policy 组合)
//!   - `common/git_scenarios.rs` (run_push_with_retry 占位)
//!   - `common/event_assert.rs` (expect_subsequence)
//! Post-audit(v1): 初版采用模拟 push 冲突/成功逻辑；后续阶段将接入真实 Git push 结果与结构化事件 DSL，替换字符串匹配。
//! Post-audit(v2): 本次审查：头部与 12.10/12.11 统一；确认事件子序列使用 expect_subsequence 仅锚定 attempt / result 关键字，避免未来 DSL 重构大面积改动；暂不去除字符串前缀 push: 保留语义提示。
//! 分区：
//!   section_push_basic     -> 基础 push 成功/无变化占位
//!   section_push_conflict  -> 冲突 / 耗尽 / Abort / ForceSuccessEarly
//!   section_retry_policy   -> 不同 backoff 序列形状验证
//!   section_retry_event    -> 事件锚点子序列验证

#[path = "../common/mod.rs"] mod common;
use common::{retry_matrix::{retry_cases, PolicyOverride}, git_scenarios::{run_push_with_retry, PushRetrySpec, PushResultKind}, event_assert::{expect_subsequence, assert_contains_phases, EventPhase, tagify, default_tag_mapper, expect_tags_subsequence, assert_terminal_exclusive}};

// ---------------- section_push_basic ----------------
mod section_push_basic {
    use super::*;
    #[test]
    fn push_basic_success_placeholder() {
        // 使用一个 attempts=1 且不模拟冲突的 case：直接成功
        let case = retry_cases().into_iter().find(|c| c.attempts == 1).expect("have attempts=1 case");
        let spec = PushRetrySpec { case, simulate_conflict: false };
        let out = run_push_with_retry(&spec);
        assert!(matches!(out.result, PushResultKind::Success));
        assert_eq!(out.attempts_used, 1);
        assert!(!out.events.is_empty());
        // 标签锚点：Attempt -> result:success
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["Attempt", "result:success"]); }
        // 终态互斥：success 不得与 exhausted/abort/conflict 同存
    assert_terminal_exclusive(&out.events, "result:success", &["result:exhausted", "result:abort"]);
    }
}

// ---------------- section_push_conflict ----------------
mod section_push_conflict {
    use super::*;
    #[test]
    fn push_conflict_exhausted() {
        // 选一个 attempts>=3 的 case，模拟冲突直到耗尽
        let case = retry_cases().into_iter().find(|c| c.attempts >= 3 && matches!(c.policy, PolicyOverride::None)).expect("have attempts>=3 none policy case");
        let spec = PushRetrySpec { case, simulate_conflict: true };
        let out = run_push_with_retry(&spec);
        assert!(matches!(out.result, PushResultKind::Exhausted));
        assert_eq!(out.attempts_used, case.attempts);
        // 最后事件包含 exhausted
        assert!(out.events.iter().any(|e| e.contains("exhausted")), "missing exhausted event");
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["Attempt", "Attempt"]); }
        assert_terminal_exclusive(&out.events, "result:exhausted", &["result:success", "result:abort"]);
    }

    #[test]
    fn push_conflict_force_success_early() {
        let case = retry_cases().into_iter().find(|c| matches!(c.policy, PolicyOverride::ForceSuccessEarly)).expect("force success early case");
        let spec = PushRetrySpec { case, simulate_conflict: true };
        let out = run_push_with_retry(&spec);
        assert!(matches!(out.result, PushResultKind::Success));
        assert!(out.attempts_used >= 2, "should succeed by attempt 2 or later");
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["Attempt", "Attempt", "result:success"]); }
    assert_terminal_exclusive(&out.events, "result:success", &["result:exhausted", "result:abort"]);
    }

    #[test]
    fn push_conflict_abort_before_attempt() {
        // AbortAfter(k) case -> 直接 Abort
        let case = retry_cases().into_iter().find(|c| matches!(c.policy, PolicyOverride::AbortAfter(_))).expect("abort case");
        let spec = PushRetrySpec { case, simulate_conflict: true };
        let out = run_push_with_retry(&spec);
        assert!(matches!(out.result, PushResultKind::Abort));
        assert!(out.events.iter().any(|e| e.contains("abort")), "missing abort event");
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["Attempt", "result:abort"]); }
        assert_terminal_exclusive(&out.events, "result:abort", &["result:success", "result:exhausted"]);
    }
}

// ---------------- section_retry_policy ----------------
mod section_retry_policy {
    use super::*;
    #[test]
    fn backoff_sequence_shapes_match_matrix() {
        // 简化：当前已不再在 Outcome 中携带 backoff_seq，验证逻辑改为再计算基准序列长度不为 0。
        for case in retry_cases() { let seq = common::retry_matrix::compute_backoff_sequence(&case); assert_eq!(seq.len() as u8, case.attempts); }
    }
}

// ---------------- section_retry_event ----------------
mod section_retry_event {
    use super::*;
    #[test]
    fn event_subsequence_contains_attempt_and_result() {
        let case = retry_cases().into_iter().find(|c| c.attempts >= 3 && matches!(c.policy, PolicyOverride::None)).expect("case");
        let spec = PushRetrySpec { case, simulate_conflict: true };
        let out = run_push_with_retry(&spec);
        // 预期至少出现 attempt#1 -> attempt#2 -> retry -> exhausted（子序列锚点）
    expect_subsequence(&out.events, &["attempt#1", "attempt#2", "result:retry"]);
    // 标签序列（Attempt -> Attempt -> result:retry）
    let tags = tagify(&out.events, default_tag_mapper);
    expect_tags_subsequence(&tags, &["Attempt", "Attempt", "result:retry"]);
        // retry 模式下最终要么 retry/exhausted 事件存在
        assert!(out.events.iter().any(|e| e.contains("retry")));
        assert!(out.events.iter().any(|e| e.contains("result")));
        // 如果已耗尽，终态应为 exhausted；否则允许 conflict 中间态（不做互斥断言以免与中间 conflict 重合）
        if out.events.iter().any(|e| e.contains("result:exhausted")) {
            assert_terminal_exclusive(&out.events, "result:exhausted", &["result:success", "result:abort"]);
        }
    }

    #[test]
    fn event_success_subsequence() {
        let case = retry_cases().into_iter().find(|c| matches!(c.policy, PolicyOverride::ForceSuccessEarly)).expect("force success early case");
        let spec = PushRetrySpec { case, simulate_conflict: true };
        let out = run_push_with_retry(&spec);
        expect_subsequence(&out.events, &["attempt#1", "attempt#2", "result:success"]);
        let tags = tagify(&out.events, default_tag_mapper);
        expect_tags_subsequence(&tags, &["Attempt", "Attempt", "result:success"]);
        assert_terminal_exclusive(&out.events, "result:success", &["result:exhausted", "result:abort"]);
    }
}
