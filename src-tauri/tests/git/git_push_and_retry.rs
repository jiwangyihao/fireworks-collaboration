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
use common::{retry_matrix::{retry_cases, PolicyOverride}, git_scenarios::{run_push_with_retry, PushRetrySpec, PushResultKind}, event_assert::{expect_subsequence, tagify, default_tag_mapper, expect_tags_subsequence, assert_terminal_exclusive}};

// 小辅助：若标签可映射，则断言最小锚点子序列
fn expect_tag_subseq_min(events: &[String], anchors: &[&str]) {
    let tags = tagify(events, default_tag_mapper);
    if !tags.is_empty() { expect_tags_subsequence(&tags, anchors); }
}

// ---------------- section_push_basic ----------------
mod section_push_basic {
    use super::*;
    #[test]
    fn push_basic_success_placeholder() {
        common::test_env::init_test_env();
        // 使用一个 attempts=1 且不模拟冲突的 case：直接成功
        let case = retry_cases().into_iter().find(|c| c.attempts == 1).expect("have attempts=1 case");
        let spec = PushRetrySpec { case, simulate_conflict: false };
        let out = run_push_with_retry(&spec);
        assert!(matches!(out.result, PushResultKind::Success));
        assert_eq!(out.attempts_used, 1);
        assert!(!out.events.is_empty());
        // 标签锚点：Attempt -> result:success
        expect_tag_subseq_min(&out.events, &["Attempt", "result:success"]);
        // 终态互斥：success 不得与 exhausted/abort/conflict 同存
        assert_terminal_exclusive(&out.events, "result:success", &["result:exhausted", "result:abort"]);
    }
}

// ---------------- section_push_conflict ----------------
mod section_push_conflict {
    use super::*;
    #[test]
    fn push_conflict_variants_behave_as_expected() {
        common::test_env::init_test_env();
        // 预备不同策略 case 选择器
        let pick_exhausted = || retry_cases().into_iter().find(|c| c.attempts >= 3 && matches!(c.policy, PolicyOverride::None)).expect("have attempts>=3 none policy case");
        let pick_force_success = || retry_cases().into_iter().find(|c| matches!(c.policy, PolicyOverride::ForceSuccessEarly)).expect("force success early case");
        let pick_abort = || retry_cases().into_iter().find(|c| matches!(c.policy, PolicyOverride::AbortAfter(_))).expect("abort case");

        // Exhausted
        {
            let case = pick_exhausted();
            let out = run_push_with_retry(&PushRetrySpec { case, simulate_conflict: true });
            assert!(matches!(out.result, PushResultKind::Exhausted));
            assert_eq!(out.attempts_used, case.attempts);
            assert!(out.events.iter().any(|e| e.contains("exhausted")), "missing exhausted event");
            expect_tag_subseq_min(&out.events, &["Attempt", "Attempt"]);
            assert_terminal_exclusive(&out.events, "result:exhausted", &["result:success", "result:abort"]);
        }

        // ForceSuccessEarly
        {
            let case = pick_force_success();
            let out = run_push_with_retry(&PushRetrySpec { case, simulate_conflict: true });
            assert!(matches!(out.result, PushResultKind::Success));
            assert!(out.attempts_used >= 2, "should succeed by attempt 2 or later");
            expect_tag_subseq_min(&out.events, &["Attempt", "Attempt", "result:success"]);
            assert_terminal_exclusive(&out.events, "result:success", &["result:exhausted", "result:abort"]);
        }

        // AbortBeforeAttempt
        {
            let case = pick_abort();
            let out = run_push_with_retry(&PushRetrySpec { case, simulate_conflict: true });
            assert!(matches!(out.result, PushResultKind::Abort));
            assert!(out.events.iter().any(|e| e.contains("abort")), "missing abort event");
            expect_tag_subseq_min(&out.events, &["Attempt", "result:abort"]);
            assert_terminal_exclusive(&out.events, "result:abort", &["result:success", "result:exhausted"]);
        }
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
    fn event_attempt_and_result_subsequences() {
        common::test_env::init_test_env();
        #[derive(Copy, Clone)]
        enum PathKind { RetryThenExhausted, ForceSuccessEarly }
        let kinds = [PathKind::RetryThenExhausted, PathKind::ForceSuccessEarly];

        for kind in kinds {
            let (case, simulate_conflict, raw_anchors, tag_anchors): (_, bool, &[&str], &[&str]) = match kind {
                PathKind::RetryThenExhausted => (
                    retry_cases().into_iter().find(|c| c.attempts >= 3 && matches!(c.policy, PolicyOverride::None)).expect("case"),
                    true,
                    &["attempt#1", "attempt#2", "result:retry"],
                    &["Attempt", "Attempt", "result:retry"],
                ),
                PathKind::ForceSuccessEarly => (
                    retry_cases().into_iter().find(|c| matches!(c.policy, PolicyOverride::ForceSuccessEarly)).expect("force success early case"),
                    true,
                    &["attempt#1", "attempt#2", "result:success"],
                    &["Attempt", "Attempt", "result:success"],
                ),
            };
            let out = run_push_with_retry(&PushRetrySpec { case, simulate_conflict });
            expect_subsequence(&out.events, raw_anchors);
            expect_tag_subseq_min(&out.events, tag_anchors);
            // 基本存在性检查
            assert!(out.events.iter().any(|e| e.contains("result")));
            if out.events.iter().any(|e| e.contains("result:exhausted")) {
                assert_terminal_exclusive(&out.events, "result:exhausted", &["result:success", "result:abort"]);
            }
            if out.events.iter().any(|e| e.contains("result:success")) {
                assert_terminal_exclusive(&out.events, "result:success", &["result:exhausted", "result:abort"]);
            }
        }
    }
}
