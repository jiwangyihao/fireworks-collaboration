#![cfg(not(feature = "tauri-app"))]
//! Git 基础操作综合测试
//! 合并了 `git_clone_core.rs`, `git_init_and_repo_structure.rs`,
//! `git_clone_partial_filter.rs`, `git_fetch_partial_filter.rs`

// ============================================================================
// git_clone_core.rs 的测试
// ============================================================================

mod clone_core {
    use super::super::common::git_scenarios::{
        run_clone, CloneParams, _run_clone_with_cancel, assert_clone_events,
    };
    use std::sync::atomic::AtomicBool;

    #[derive(Debug, Clone, Copy)]
    struct Case {
        depth: Option<u32>,
        filter: Option<&'static str>,
    }
    impl Case {
        fn label(&self) -> String {
            format!("depth={:?},filter={:?}", self.depth, self.filter)
        }
    }
    fn cases() -> Vec<Case> {
        vec![
            Case {
                depth: None,
                filter: None,
            },
            Case {
                depth: Some(1),
                filter: None,
            },
            Case {
                depth: None,
                filter: Some("blob:none"),
            },
            Case {
                depth: Some(1),
                filter: Some("blob:none"),
            },
            Case {
                depth: Some(0),
                filter: None,
            },
        ]
    }

    #[test]
    fn clone_parameter_cases_emit_events() {
        for c in cases() {
            let out = run_clone(&CloneParams {
                depth: c.depth,
                filter: c.filter.map(|s| s.to_string()),
            });
            assert_clone_events(&format!("clone-core case {}", c.label()), &out);
        }
    }

    #[test]
    fn clone_cancel_early() {
        let cancel = AtomicBool::new(true);
        let out = _run_clone_with_cancel(
            &CloneParams {
                depth: Some(1),
                filter: None,
            },
            &cancel,
        );
        assert!(
            out.dest.exists(),
            "[clone-core cancel-early] dest should exist"
        );
    }
}

// ============================================================================
// git_init_and_repo_structure.rs 的测试
// ============================================================================

mod init_and_structure {
    use fireworks_collaboration_lib::core::git::default_impl::init::git_init;
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use std::sync::atomic::AtomicBool;

    use super::super::common::git_helpers;
    use super::super::common::{fixtures, test_env};

    #[test]
    fn git_init_success_and_idempotent() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(false);
        git_init(&dest, &flag, |_p| {}).expect("[init-basic] init ok");
        assert!(
            dest.join(".git").exists(),
            "[init-basic] expect .git dir after init"
        );
        git_init(&dest, &flag, |_p| {}).expect("[init-basic] idempotent");
    }

    #[test]
    fn git_init_cancel_before() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        let flag = AtomicBool::new(true);
        let out = git_init(&dest, &flag, |_p| {});
        assert!(out.is_err(), "[init-basic] expect cancel error");
        git_helpers::assert_err_category(
            "init-basic cancel",
            out.err().unwrap(),
            ErrorCategory::Cancel,
        );
    }

    #[test]
    fn init_fails_when_target_is_file() {
        test_env::init_test_env();
        let dest = fixtures::temp_dir();
        std::fs::create_dir_all(&dest).unwrap();
        let file_path = dest.join("a.txt");
        std::fs::write(&file_path, "hi").unwrap();
        let cancel = AtomicBool::new(false);
        let out = git_init(&file_path, &cancel, |_p| {});
        assert!(
            out.is_err(),
            "[preflight] expect protocol error for file path"
        );
        git_helpers::assert_err_category(
            "preflight path-is-file",
            out.err().unwrap(),
            ErrorCategory::Protocol,
        );
    }
}

// ============================================================================
// git_clone_partial_filter.rs 的测试
// ============================================================================

mod clone_partial_filter {
    use super::super::common::{
        git_scenarios::{run_clone, CloneParams},
        partial_filter_support::{
            assess_partial_filter, warn_if_no_filter_marker, PartialFilterOutcome, SupportLevel,
        },
        test_env,
    };

    fn params_from_label(label: &str, depth: Option<u32>) -> CloneParams {
        CloneParams {
            depth,
            filter: Some(format!("filter:{label}")),
        }
    }

    fn exec_assess(
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

    fn assert_events_non_empty(context: &str, events: &[String]) {
        assert!(!events.is_empty(), "[{context}] events should not be empty");
    }

    #[test]
    fn capability_matrix_cases() {
        test_env::init_test_env();
        use super::super::common::partial_filter_matrix::clone_partial_filter_cases;
        for case in clone_partial_filter_cases() {
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
            let (_params, outcome, events) = exec_assess(label, depth);
            assert_events_non_empty("capability", &events);
            warn_if_no_filter_marker("capability", &format!("filter:{label}"), &outcome);
        }
    }

    #[test]
    fn all_supported_or_degraded_not_unsupported() {
        test_env::init_test_env();
        let variants = vec![
            ("event-only", None),
            ("code-only", None),
            ("structure", None),
            ("code+depth", Some(1)),
            ("event+depth", Some(1)),
        ];
        for (label, depth) in variants {
            let (_params, outcome, events) = exec_assess(label, depth);
            assert_events_non_empty("filter_variants", &events);
            assert!(
                !matches!(
                    outcome.support,
                    SupportLevel::Unsupported | SupportLevel::Invalid
                ),
                "[{label}] should not be Unsupported/Invalid (got {:?})",
                outcome.support
            );
            warn_if_no_filter_marker("filter_variants", &format!("filter:{label}"), &outcome);
        }
    }

    #[test]
    fn fallback_supported_and_unsupported() {
        test_env::init_test_env();
        for (label, expect_unsupported) in [("unsupported-case", true), ("event-only", false)] {
            let (_params, outcome, events) = exec_assess(label, None);
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
            warn_if_no_filter_marker("fallback", &format!("filter:{label}"), &outcome);
        }
    }
}

// ============================================================================
// git_fetch_partial_filter.rs 的测试
// ============================================================================

mod fetch_partial_filter {
    use super::super::common::git_scenarios::{run_fetch, FetchParams};
    use super::super::common::{
        event_assert::expect_optional_tags_subsequence,
        partial_filter_matrix::{
            partial_filter_cases_for, PartialFilterCase, PartialFilterKind, PartialFilterOp,
        },
        partial_filter_support::{assess_partial_filter, warn_if_no_filter_marker, SupportLevel},
        test_env,
    };

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

    #[test]
    fn fetch_partial_capability_each_case_not_unsupported() {
        test_env::init_test_env();
        for case in partial_filter_cases_for(PartialFilterOp::Fetch) {
            let label_opt = build_label(&case);
            let params = FetchParams {
                depth: case.depth,
                filter: label_opt.map(|l| format!("filter:{l}")),
            };
            let events = run_fetch(&params).events;
            assert!(
                !events.is_empty(),
                "[fetch_capability] events non-empty for {case}"
            );
            let f_label = label_opt.unwrap_or("");
            let out = assess_partial_filter(&format!("filter:{f_label}"), case.depth, &events);
            if matches!(out.support, SupportLevel::Unsupported) {
                panic!("[fetch_capability] case {case} unexpectedly Unsupported");
            }
            if label_opt.is_some() {
                warn_if_no_filter_marker("fetch_capability", &format!("filter:{f_label}"), &out);
            }
        }
    }

    #[test]
    fn fetch_event_code_structure_no_filter_variants() {
        test_env::init_test_env();
        let variants = [
            PartialFilterKind::EventOnly,
            PartialFilterKind::CodeOnly,
            PartialFilterKind::Structure,
            PartialFilterKind::NoFilter,
        ];
        for kind in variants {
            let case = PartialFilterCase {
                op: PartialFilterOp::Fetch,
                kind,
                depth: None,
            };
            let label_opt = build_label(&case);
            let params = FetchParams {
                depth: None,
                filter: label_opt.map(|l| format!("filter:{l}")),
            };
            let events = run_fetch(&params).events;
            assert!(
                !events.is_empty(),
                "[fetch_variants] events non-empty for {kind:?}"
            );
            let filter_expr = label_opt
                .map(|l| format!("filter:{l}"))
                .unwrap_or_else(|| "".into());
            let out = assess_partial_filter(&filter_expr, None, &events);
            if matches!(
                out.support,
                SupportLevel::Unsupported | SupportLevel::Invalid
            ) {
                panic!(
                    "[fetch_variants] kind {kind:?} unexpected support={:?}",
                    out.support
                );
            }
            if label_opt.is_some() {
                warn_if_no_filter_marker("fetch_variants", &filter_expr, &out);
            }
        }
    }

    #[test]
    fn fetch_code_and_event_with_depth() {
        test_env::init_test_env();
        let depth_cases = [
            PartialFilterKind::CodeWithDepth,
            PartialFilterKind::EventWithDepth,
        ];
        for kind in depth_cases {
            let case = PartialFilterCase {
                op: PartialFilterOp::Fetch,
                kind,
                depth: Some(1),
            };
            let label_opt = build_label(&case);
            let params = FetchParams {
                depth: case.depth,
                filter: label_opt.map(|l| format!("filter:{l}")),
            };
            let events = run_fetch(&params).events;
            assert!(
                !events.is_empty(),
                "[fetch_depth] events non-empty for {kind:?}"
            );
            let label = label_opt.unwrap();
            let out = assess_partial_filter(&format!("filter:{label}"), case.depth, &events);
            if matches!(
                out.support,
                SupportLevel::Unsupported | SupportLevel::Invalid
            ) {
                panic!(
                    "[fetch_depth] {kind:?} unexpected support={:?}",
                    out.support
                );
            }
            warn_if_no_filter_marker("fetch_depth", &format!("filter:{label}"), &out);
            expect_optional_tags_subsequence(&events, &["fetch"]);
        }
    }

    #[test]
    fn fetch_invalid_filter_support_level_invalid() {
        test_env::init_test_env();
        let case = PartialFilterCase {
            op: PartialFilterOp::Fetch,
            kind: PartialFilterKind::InvalidFilter,
            depth: None,
        };
        let label_opt = build_label(&case);
        let params = FetchParams {
            depth: None,
            filter: label_opt.map(|l| format!("filter:{l}")),
        };
        let events = run_fetch(&params).events;
        assert!(!events.is_empty(), "[fetch_invalid] events non-empty");
        let out = assess_partial_filter("filter:bad:filter", None, &events);
        assert!(
            matches!(out.support, SupportLevel::Invalid),
            "invalid filter should map to Invalid support (got {:?})",
            out.support
        );
        warn_if_no_filter_marker("fetch_invalid", "filter:bad:filter", &out);
    }

    #[test]
    fn fetch_unsupported_filter_yields_unsupported() {
        test_env::init_test_env();
        let params = FetchParams {
            depth: None,
            filter: Some("filter:unsupported-case".into()),
        };
        let events = run_fetch(&params).events;
        assert!(!events.is_empty(), "[fetch_fallback] events non-empty");
        let out = assess_partial_filter("filter:unsupported-case", None, &events);
        assert!(
            matches!(out.support, SupportLevel::Unsupported),
            "expected Unsupported got {:?})",
            out.support
        );
        warn_if_no_filter_marker("fetch_fallback", "filter:unsupported-case", &out);
    }
}
