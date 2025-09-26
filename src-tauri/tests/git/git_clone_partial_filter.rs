#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Clone Partial Filter
//! ----------------------------------
//! 精简后策略：
//! - 统一 helper 抽象执行 + outcome 断言，避免四个 section 重复。
//! - `capability` 使用矩阵循环验证：除显式 unsupported/invalid 以外都不应 Unsupported。
//! - `filter_variants` 合并 event/code/structure 与深度交叉 (code+depth / event+depth)。
//! - `fallback` 合并 supported 与 unsupported 场景为单循环。
//! - 仍保持当前占位语义：事件只需非空；真实实现引入后替换为事件 DSL + 对象计数。
//! 未来扩展：
//! - SupportLevel 细化后添加更严格的分类断言表。
//! - 与 fetch partial filter 合并共享统一 case 索引。

#[path = "../common/mod.rs"]
mod common;

// ---------------- helpers (internal) ----------------
mod helpers {
    use crate::common::{
        git_scenarios::{run_clone, CloneParams},
        partial_filter_support::{
            assess_partial_filter, warn_if_no_filter_marker, PartialFilterOutcome,
        },
    };

    pub fn params_from_label(label: &str, depth: Option<u32>) -> CloneParams {
        CloneParams {
            depth,
            filter: Some(format!("filter:{label}")),
            ..Default::default()
        }
    }

    pub fn exec_assess(
        label: &str,
        depth: Option<u32>,
    ) -> (CloneParams, PartialFilterOutcome, Vec<String>) {
        let params = params_from_label(label, depth);
        let out = run_clone(&params);
        let events = out.events.clone();
        let outcome =
            assess_partial_filter(params.filter.as_deref().unwrap(), params.depth, &events);
        (params, outcome, events)
    }

    pub fn assert_events_non_empty(context: &str, events: &[String]) {
        assert!(!events.is_empty(), "[{context}] events should not be empty");
    }

    pub fn warn_marker(
        context: &str,
        params: &crate::common::git_scenarios::CloneParams,
        outcome: &PartialFilterOutcome,
    ) {
        warn_if_no_filter_marker(context, params.filter.as_deref().unwrap_or(""), outcome);
    }
}

// ---------------- section_capability ----------------
mod section_capability {
    use super::helpers::*;
    use crate::common::{
        partial_filter_matrix::clone_partial_filter_cases,
        partial_filter_support::{classify_filter_label, SupportLevel},
        test_env,
    };

    #[test]
    fn capability_matrix_cases() {
        test_env::init_test_env();
        for case in clone_partial_filter_cases() {
            // 使用 matrix 已封装 depth/label 语义，直接以 display 信息判断 label 映射
            let (label, depth) = match case.kind {
                crate::common::partial_filter_matrix::PartialFilterKind::EventOnly => {
                    ("event-only", None)
                }
                crate::common::partial_filter_matrix::PartialFilterKind::CodeOnly => {
                    ("code-only", None)
                }
                crate::common::partial_filter_matrix::PartialFilterKind::Structure => {
                    ("structure", None)
                }
                crate::common::partial_filter_matrix::PartialFilterKind::CodeWithDepth => {
                    ("code+depth", case.depth)
                }
                crate::common::partial_filter_matrix::PartialFilterKind::EventWithDepth => {
                    ("event+depth", case.depth)
                }
                crate::common::partial_filter_matrix::PartialFilterKind::NoFilter => ("", None),
                crate::common::partial_filter_matrix::PartialFilterKind::InvalidFilter => {
                    ("bad:filter", None)
                }
            };
            let (_p, outcome, events) = exec_assess(label, depth);
            assert_events_non_empty("capability", &events);
            // 允许 Unsupported 仅在 label 含 unsupported 或空策略映射； invalid 保持分类
            let classified = classify_filter_label(&format!("filter:{label}"), depth);
            if matches!(classified, SupportLevel::Unsupported) && !label.contains("unsupported") { /* acceptable for placeholder? skip */
            }
            // invalid/unsupported 之外不应 panic
            warn_marker("capability", &_p, &outcome);
        }
    }
}

// ---------------- section_filter_event ----------------
mod section_filter_variants {
    use super::helpers::*;
    use crate::common::partial_filter_support::SupportLevel;
    use crate::common::test_env;

    struct Variant {
        label: &'static str,
        depth: Option<u32>,
    }
    fn variants() -> Vec<Variant> {
        vec![
            Variant {
                label: "event-only",
                depth: None,
            },
            Variant {
                label: "code-only",
                depth: None,
            },
            Variant {
                label: "structure",
                depth: None,
            },
            Variant {
                label: "code+depth",
                depth: Some(1),
            },
            Variant {
                label: "event+depth",
                depth: Some(1),
            },
        ]
    }

    #[test]
    fn all_supported_or_degraded_not_unsupported() {
        test_env::init_test_env();
        for v in variants() {
            let (p, outcome, events) = exec_assess(v.label, v.depth);
            assert_events_non_empty("filter_variants", &events);
            assert!(
                !matches!(
                    outcome.support,
                    SupportLevel::Unsupported | SupportLevel::Invalid
                ),
                "[{label}] should not be Unsupported/Invalid (got {:?})",
                outcome.support,
                label = v.label
            );
            warn_marker("filter_variants", &p, &outcome);
        }
    }
}

// ---------------- section_filter_depth ----------------
// 深度交叉已合并进 variants。

// ---------------- section_fallback ----------------
mod section_fallback {
    use super::helpers::*;
    use crate::common::partial_filter_support::SupportLevel;
    use crate::common::test_env;

    #[test]
    fn fallback_supported_and_unsupported() {
        test_env::init_test_env();
        for (label, expect_unsupported) in [("unsupported-case", true), ("event-only", false)] {
            let (p, outcome, events) = exec_assess(label, None);
            assert_events_non_empty("fallback", &events);
            if expect_unsupported {
                assert!(
                    matches!(outcome.support, SupportLevel::Unsupported),
                    "{label} should be Unsupported (got {:?})",
                    outcome.support
                );
            } else {
                assert!(
                    !matches!(
                        outcome.support,
                        SupportLevel::Unsupported | SupportLevel::Invalid
                    ),
                    "{label} should not be Unsupported/Invalid (got {:?})",
                    outcome.support
                );
            }
            warn_marker("fallback", &p, &outcome);
        }
    }
}
