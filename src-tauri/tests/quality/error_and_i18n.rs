#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Error Mapping & i18n (Roadmap 12.14)
//! ------------------------------------------------------------
//! Phase4 Metrics (属性测试集中 v1.17)：
//!   * Added sections (props): `strategy_props` / `retry_props` / `partial_filter_props`
//!   * Migrated property source files: 4 (`prop_strategy_http_override.rs`, `prop_retry_override.rs`, `prop_strategy_summary_codes.rs`, `prop_partial_filter_capability.rs`)
//!   * Legacy seeds archived: `prop_tls_override.proptest-regressions`（TLS override 属性测试已退役）
//!   * Total proptest groups: 4 (http override, summary codes, retry override, partial filter fallback)
//!   * Root-level prop_* files replaced with placeholders (assert!(true)) preserving git blame
//!   * Consolidation rationale: reduce search surface & ensure future override semantic changes require touching a single file
//!   * Next follow-up (optional): extract shared `AppConfig` mutation patterns into a helper to cut duplication (~30 lines)
//! 来源文件：
//!   - `error_i18n_map.rs` (错误分类 + 中文消息 Network 分类验证)
//! 设计说明：
//!   * 当前仅有一个 legacy 用例：中文“无法 连接 ... 超时”消息映射到 Network。
//!   * 本聚合文件预留分区结构，后续接入多 locale key / fallback / 组合错误场景。
//! 分区：
//!   `section_error_mapping`        -> `错误分类（map_git2_error` -> `ErrorCategory` + `AppErrorKind` 桥接）
//!   `section_i18n_locale_basic`    -> 多语言关键 key 存在性（首版占位已实现）
//!   `section_i18n_fallback`        -> locale 回退策略（首版占位实现：不存在 locale -> fallback en）
//!   `section_integration_edge`     -> 复合错误互斥（占位）
//! 未来扩展计划 (Post-audit):
//!   - 引入 `AppErrorKind` 枚举统一抽象 (Protocol/Network/Cancel/Timeout/...)
//!   - 提供 helper: `assert_error_category(label`, err, `AppErrorKind`)
//!   - 构建 locale fixture & key 列表，断言所有关键 keys 在 zh/en 下存在
//!   - Fallback 测试：设置不存在 locale -> 回退 en
//!   - 组合：模拟 Cancel + Timeout 互斥，验证只出现一个类别
//! Cross-ref:
//!   - git_* 聚合文件中已经使用 `ErrorCategory` 进行分类断言
//!   - events_* 聚合文件中的失败/取消终态映射将在 12.15 之后结合 `AppErrorKind` 统一

use super::common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}
// 顶层不直接使用集合类型

// ---------------- section_error_mapping ----------------
mod section_error_mapping {
    use fireworks_collaboration_lib::core::git::default_impl::helpers::map_git2_error;
    use fireworks_collaboration_lib::core::git::errors::ErrorCategory;
    use git2::{Error, ErrorClass, ErrorCode};
    #[test]
    fn chinese_connection_error_classified_as_network() {
        // 构造 git2::Error：中文超时/连接信息应映射为 Network
        let err = git2::Error::from_str("无法 连接 到 服务器: 超时");
        let cat = map_git2_error(&err);
        assert!(
            matches!(cat, ErrorCategory::Network),
            "expected Network got {cat:?}"
        );
    }

    #[test]
    fn mapping_snapshot_matrix() {
        fn mk_err(code: ErrorCode, class: ErrorClass, msg: &str) -> Error {
            Error::new(code, class, msg)
        }
        let cases = vec![
            (
                mk_err(ErrorCode::User, ErrorClass::None, "user canceled"),
                ErrorCategory::Cancel,
                "cancel",
            ),
            (
                mk_err(
                    ErrorCode::GenericError,
                    ErrorClass::Net,
                    "connection timed out",
                ),
                ErrorCategory::Network,
                "timeout",
            ),
            (
                mk_err(ErrorCode::GenericError, ErrorClass::Net, "连接 超时"),
                ErrorCategory::Network,
                "cn-timeout",
            ),
            (
                mk_err(
                    ErrorCode::GenericError,
                    ErrorClass::Ssl,
                    "tls handshake failure",
                ),
                ErrorCategory::Tls,
                "tls",
            ),
            (
                mk_err(
                    ErrorCode::GenericError,
                    ErrorClass::Ssl,
                    "certificate verify failed",
                ),
                ErrorCategory::Verify,
                "cert",
            ),
            (
                mk_err(ErrorCode::GenericError, ErrorClass::Http, "HTTP 501"),
                ErrorCategory::Protocol,
                "http-class",
            ),
            (
                mk_err(ErrorCode::Auth, ErrorClass::Http, "401 Unauthorized"),
                ErrorCategory::Auth,
                "401",
            ),
            (
                mk_err(ErrorCode::Auth, ErrorClass::Http, "permission denied"),
                ErrorCategory::Auth,
                "perm",
            ),
            (
                mk_err(
                    ErrorCode::GenericError,
                    ErrorClass::Config,
                    "some internal weird",
                ),
                ErrorCategory::Internal,
                "internal",
            ),
        ];
        for (err, expect, tag) in cases {
            assert_eq!(
                map_git2_error(&err),
                expect,
                "case tag={tag} msg={}",
                err.message()
            );
        }
    }
}

// ---------------- section_i18n_locale_basic ----------------
mod section_i18n_locale_basic {
    use crate::common::i18n::{locale_keys, translate};
    use std::collections::HashSet;
    #[test]
    fn locale_keys_present_in_all_supported_languages() {
        let keys = locale_keys();
        assert!(!keys.is_empty(), "fixture keys empty");
        // 支持语言集合（后续扩展时仅在此添加）
        let langs = ["en", "zh"];
        for k in &keys {
            for lang in &langs {
                let t = translate(k, lang).unwrap_or_default();
                assert!(!t.is_empty(), "missing translation for key={k} lang={lang}");
            }
        }
        // key 去重校验
        let set: HashSet<&str> = keys.iter().copied().collect();
        assert_eq!(set.len(), keys.len(), "duplicate locale key detected");
    }
}

// ---------------- section_i18n_fallback ----------------
mod section_i18n_fallback {
    use crate::common::i18n::translate;
    #[test]
    fn missing_locale_fallbacks_to_en() {
        let key = "error.network.timeout";
        let zh = translate(key, "zh").expect("zh");
        let en = translate(key, "en").expect("en");
        let bogus = translate(key, "fr").expect("fallback fr->en");
        assert_ne!(zh, en, "zh/en translations should differ in fixture");
        assert_eq!(bogus, en, "fallback should return en variant");
    }

    #[test]
    fn missing_key_returns_none() {
        assert!(translate("non.existent.key", "en").is_none());
    }
}

// 删除过时占位：integration_edge_placeholder（无实际覆盖价值）

// -----------------------------------------------------------------------------
// Phase 4 属性测试集中 (strategy / retry / partial_filter)
// 来源文件 (root-level prop_*.rs，将在迁移完成后占位化):
//   - prop_strategy_http_override.rs
//   - prop_retry_override.rs
//   - prop_strategy_summary_codes.rs
//   - prop_partial_filter_capability.rs
// 说明：保持与原测试函数名一致，添加 section_* 前缀模块划分。
// 属性测试均受 cfg(not(feature = "tauri-app")) 保护，与本文件一致无需额外 cfg。
// -----------------------------------------------------------------------------

// ---------------- section_strategy_props ----------------
#[cfg(test)]
mod section_strategy_props {
    use fireworks_collaboration_lib::core::config::model::AppConfig;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use proptest::prelude::*;
    use uuid::Uuid;

    proptest! {
        #[test]
        fn http_override_conflict_normalizes(follow in any::<bool>(), raw_max in 0u32..25) {
            let mut global = AppConfig::default();
            global.http.follow_redirects = !follow;
            let over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyHttpOverride {
                follow_redirects: Some(follow),
                max_redirects: Some(raw_max),
            };
            let (_f, m, _changed, conflict) = TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&over));
            prop_assert!(m <= 20, "clamped max should be <=20, got {m}");
            if !follow && m > 0 { prop_assert!(conflict.is_some(), "expected conflict message when follow=false and m>0"); }
            if conflict.is_some() { prop_assert_eq!(m, 0, "conflict must normalize max to 0"); }
        }
    }

    proptest! {
        #[test]
        fn strategy_summary_applied_codes_consistency(http_follow in proptest::bool::ANY,
                                                      http_max in 0u8..=20,
                                                      retry_max in 1u32..10,
                                                      retry_base in 50u64..500,
                                                      retry_factor in 1u32..5) {
            let mut global = AppConfig::default();
            global.http.follow_redirects = !http_follow;
            global.http.max_redirects = if http_max==0 {1} else {http_max};
            global.retry.max = retry_max + 5;
            global.retry.base_ms = retry_base + 10;
            global.retry.factor = (retry_factor as f64) + 0.5;
            let http_over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyHttpOverride { follow_redirects: Some(http_follow), max_redirects: Some(http_max as u32) };
            let retry_over = fireworks_collaboration_lib::core::git::default_impl::opts::StrategyRetryOverride { max: Some(retry_max), base_ms: Some(retry_base as u32), factor: Some(retry_factor as f32), jitter: Some(false) };
            let (f,m,http_changed,_http_conflict) = TaskRegistry::apply_http_override("GitClone", &Uuid::nil(), &global, Some(&http_over));
            let (_retry_plan, retry_changed) = TaskRegistry::apply_retry_override(&global.retry, Some(&retry_over));
            let mut codes: Vec<String> = vec![];
            if http_changed { codes.push("http_strategy_override_applied".into()); }
            if retry_changed { codes.push("retry_strategy_override_applied".into()); }
            codes.sort(); codes.dedup();
            if http_changed { assert!(codes.iter().any(|c| c=="http_strategy_override_applied")); }
            if retry_changed { assert!(codes.iter().any(|c| c=="retry_strategy_override_applied")); }
            if !http_changed && !retry_changed { assert!(codes.is_empty()); }
            assert_ne!(global.http.follow_redirects, f, "we intentionally changed follow to trigger changed");
            let _ = m; // silence warnings
        }
    }
}

// ---------------- section_retry_props ----------------
#[cfg(test)]
mod section_retry_props {
    use fireworks_collaboration_lib::core::config::model::RetryCfg;
    use fireworks_collaboration_lib::core::git::default_impl::opts::StrategyRetryOverride;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn retry_override_changes_detected(max in 0u32..10, base_ms in 50u32..2000, factor in 1.0f64..3.0f64, jitter in any::<bool>()) {
            let mut global = RetryCfg::default();
            global.max = 5; global.base_ms = 300; global.factor = 1.5; global.jitter = true;
            let over = StrategyRetryOverride { max: Some(max), base_ms: Some(base_ms), factor: Some(factor as f32), jitter: Some(jitter) };
            let (plan, changed) = TaskRegistry::apply_retry_override(&global, Some(&over));
            if plan.max == global.max && plan.base_ms == global.base_ms && (plan.factor - global.factor).abs() < f64::EPSILON && plan.jitter == global.jitter {
                prop_assert!(!changed, "no field changed but changed=true");
            } else { prop_assert!(changed, "some field changed but changed=false"); }
            prop_assert!(plan.factor >= 0.0, "factor should stay non-negative");
        }
    }
}

// ---------------- section_partial_filter_props ----------------
#[cfg(test)]
mod section_partial_filter_props {
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn partial_filter_fallback_decision(depth in prop::option::of(0u32..5), filter in prop::option::of("[a-z0-9:_]{1,8}")) {
            for supported in [true,false] {
                let shallow_expected = depth.is_some();
                let res = TaskRegistry::decide_partial_fallback(depth, filter.as_deref(), supported);
                if filter.is_none() || supported { assert!(res.is_none(), "no fallback when no filter or supported"); }
                else {
                    let (msg, shallow_flag) = res.expect("expected fallback");
                    assert!(msg.contains("partial filter unsupported"));
                    assert_eq!(shallow_flag, shallow_expected, "shallow flag should mirror depth presence");
                }
            }
        }
    }
}
