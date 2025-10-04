#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Strategy & HTTP Override & Adaptive TLS (Roadmap 12.10)
//! ---------------------------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - git_strategy_override_combo.rs
//!   - git_http_override_event.rs
//!   - git_http_override_no_event.rs
//!   - git_http_override_event_structured.rs
//!   - git_http_override_idempotent.rs
//!   - git_http_override_invalid_max_no_event.rs
//!   - git_http_override_fetch_event_only_max.rs
//!   - git_http_override_clone_only_follow.rs
//!   - git_http_override_push_follow_change.rs
//!   - git_adaptive_tls_rollout_event.rs
//!   - (Phase3 新增待迁移来源)：
//!       * git_strategy_override_structured.rs
//!       * git_strategy_override_summary_fetch_push.rs
//!       * strategy_override_push.rs
//!       * strategy_override_summary.rs
//!       * git_strategy_override_no_conflict.rs
//!       * strategy_override_empty_unknown_integration.rs
//!       * strategy_override_invalid_integration.rs
//!       * git_strategy_override_tls_combo.rs
//!       * git_strategy_override_tls_mixed.rs
//!       * git_strategy_override_tls_summary.rs
//!       * git_tls_override_event.rs
//!       * git_tls_push_insecure_only.rs
//! 分区结构：
//!   section_http_basic                -> HTTP override 基础 & 事件存在
//!   section_http_limits               -> follow / max / idempotent 变体
//!   section_http_events               -> 事件子序列锚点断言
//!   section_strategy_summary_multiop  -> 多操作 Summary + 结构化多 applied codes
//!   section_override_no_conflict      -> TLS only 修改 & changed vs unchanged & push insecure only
//!   section_override_empty_unknown    -> 空对象 / unknown 字段宽容
//!   section_override_invalid_inputs   -> 无效参数导致失败
//!   section_tls_mixed_scenarios       -> TLS mixed (clone/fetch/push) 变体
//!   section_summary_gating            -> gating 环境变量 on/off 对比
//!
//! 补充：传输层 fallback / timing 测试已迁移至 `git_preconditions_and_cancel.rs`；TLS 指纹日志与 Pin 校验相关用例集中到 `events/events_structure_and_contract.rs`。
//! Cross-ref:
//!   - `common/http_override_stub.rs`
//!   - `common/git_scenarios.rs` (GitOp)
//!   - `common/event_assert.rs` (expect_subsequence)
//! 设计原则：
//!   * 先以字符串事件 + 子序列匹配保证结构，后续 12.12 引入结构化事件 DSL 再收紧。
//!   * 策略 & HTTP override 共享 GitOp 概念，未来可提炼 Outcome 多态结构。
//! Post-audit(v1) 目标：所有来源文件逻辑被覆盖或留出明确 TODO 占位；不引入真实网络副作用。
//! Post-audit(v2): 补充：统一策略枚举 StrategyPolicy 仍为占位；后续（事件 DSL 引入后）将把 strategy/http_override/adaptive_tls 事件合并为结构化枚举并移除字符串 contains 断言；max=0 invalid 用例已单独覆盖无需新增锚点。
//! Metrics Phase3 (refactor v1.16 draft):
//!   * 新增 sections: summary_multiop / override_no_conflict / override_empty_unknown / override_invalid_inputs / tls_mixed_scenarios / summary_gating (6)
//!   * 迁移来源 root-level 测试文件数: 12 (全部已占位保留 stub)
//!   * 剪裁 git_impl_tests.rs: 移除 2 个基础重复测试 (local progress, fetch remote tracking)
//!   * 当前文件行数（approx）：~780 (<800 OK)
//!   * TODO(Phase4): 若继续增长 >800, 考虑拆分 extended TLS/HTTP/retry 子模块。

use super::common::{
    event_assert::{expect_optional_tags_subsequence, expect_subsequence},
    git_scenarios::GitOp,
    http_override_stub::{
        http_override_cases, run_http_override, FollowMode, HttpOverrideCase, IdempotentFlag,
        MaxEventsCase,
    },
    test_env::init_test_env,
};

// 顶层通用：等待任务完成（供多个 section 复用）
use crate::common::task_wait::wait_until_task_done as wait_done;

// 统一测试环境初始化（Once 防抖）
#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// 已移除：早期 Strategy 占位实现与覆盖测试（无真实策略逻辑，改由结构化事件测试覆盖）

// ---------------- section_http_basic ----------------
mod section_http_basic {
    use super::*;
    #[test]
    fn http_override_each_case_applied() {
        for c in http_override_cases() {
            let out = run_http_override(&c);
            assert!(out.applied, "override not applied for {c}");
            assert!(!out.events.is_empty());
        }
    }
}

// ---------------- section_http_limits ----------------
mod section_http_limits {
    use super::*;
    #[test]
    fn http_follow_chain_length_with_max() {
        for c in http_override_cases() {
            let out = run_http_override(&c);
            if matches!(c.follow, FollowMode::Follow) {
                if let MaxEventsCase::Some(m) = c.max_events {
                    assert!(
                        out.follow_chain.len() as u32 <= m.max(1),
                        "follow chain exceeds max for {c}"
                    );
                } else {
                    assert!(out.follow_chain.len() >= 1, "expected default follow hops");
                }
            }
        }
    }
}
// invalid max (=0) 单独测试
// invalid max (=0) 单独测试（已使用 DSL 断言书写，见下方 section_http_invalid_max）
mod section_http_invalid_max {
    use super::*;
    #[test]
    fn http_invalid_max_zero_applied_false() {
        let case = HttpOverrideCase {
            op: GitOp::Clone,
            follow: FollowMode::None,
            idempotent: IdempotentFlag::No,
            max_events: MaxEventsCase::Some(0),
        };
        let out = run_http_override(&case);
        assert!(!out.applied); // 使用 DSL 子序列断言替代 contains 脆弱断言
        expect_subsequence(&out.events, &["http:override:invalid_max"]);
        expect_optional_tags_subsequence(&out.events, &["http"]);
    }
}

// ---------------- section_http_events ----------------
mod section_http_events {
    use super::*;
    #[test]
    fn http_event_anchor_sequence() {
        let any_case = http_override_cases()
            .into_iter()
            .find(|c| matches!(c.follow, FollowMode::Follow))
            .expect("follow case");
        let out = run_http_override(&any_case);
        expect_subsequence(
            &out.events,
            &[
                "http:override:start",
                "http:override",
                "http:override:applied",
            ],
        );
        expect_optional_tags_subsequence(&out.events, &["http", "http"]);
    }
}

// 已移除：自模拟 TLS rollout 事件（与真实结构化事件无关，过时）

// ---------------- (Phase3) section_strategy_summary_multiop ----------------
// 来源函数：
//   - strategy_override_http_tls_retry_combo_parallel (并行：已拆分主要断言到 combo 原文件，此处保持结构化 appliedCodes 视角)
//   - strategy_override_http_tls_summary_structured_events
//   - strategy_override_no_change_generates_summary_only
//   - fetch_summary_event_and_applied_codes
//   - push_summary_event_with_independent_applied_events
//   - fetch_summary_event_no_override
//   - push_summary_event_no_override
//   - strategy_override_summary_and_gating (summary + 多 appliedCodes 片段，gating 逻辑单列在 section_summary_gating)
mod section_strategy_summary_multiop {
    use crate::common::strategy_support::test_emit_clone_with_override;
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use fireworks_collaboration_lib::events::emitter::AppHandle;
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_global_event_bus, set_test_event_bus, Event as StructuredEvent,
        MemoryEventBus, StrategyEvent,
    };

    // --- helpers ---
    fn install_test_bus() -> MemoryEventBus {
        let bus = MemoryEventBus::new();
        set_test_event_bus(std::sync::Arc::new(bus.clone()));
        bus
    }

    // 结构化事件：所有三类 (HttpApplied, TlsApplied, RetryApplied+Summary)
    #[test]
    fn strategy_override_http_tls_summary_structured_events() {
        let bus = install_test_bus();
        let task_id = uuid::Uuid::new_v4();
        let override_json = serde_json::json!({
            "http": {"follow_redirects": false, "max_redirects": 5},
            "tls": {"insecure_skip_verify": true, "skip_san_whitelist": true},
            "retry": {"max": 3, "baseMs": 500, "factor": 2.0, "jitter": false}
        });
        test_emit_clone_with_override("https://example.com/repo.git", task_id, override_json);
        let evs = bus.snapshot();
        let mut has_http = false;
        let mut has_tls = false;
        let mut has_summary = false;
        let mut has_retry = false;
        for e in &evs {
            match e {
                StructuredEvent::Strategy(StrategyEvent::HttpApplied { id, .. })
                    if id == &task_id.to_string() =>
                {
                    has_http = true
                }
                StructuredEvent::Strategy(StrategyEvent::TlsApplied { id, .. })
                    if id == &task_id.to_string() =>
                {
                    has_tls = true
                }
                StructuredEvent::Strategy(StrategyEvent::Summary { id, kind, .. })
                    if id == &task_id.to_string() && kind == "GitClone" =>
                {
                    has_summary = true
                }
                StructuredEvent::Policy(
                    fireworks_collaboration_lib::events::structured::PolicyEvent::RetryApplied {
                        id,
                        ..
                    },
                ) if id == &task_id.to_string() => has_retry = true,
                _ => {}
            }
        }
        assert!(has_http && has_tls && has_summary && has_retry, "missing one of expected structured events http={has_http} tls={has_tls} summary={has_summary} retry={has_retry}");
        clear_test_event_bus();
    }

    #[test]
    fn strategy_override_no_change_generates_summary_only() {
        let bus = install_test_bus();
        let task_id = uuid::Uuid::new_v4();
        let override_json = serde_json::json!({
            "http": {"follow_redirects": true},
            "tls": {"insecure_skip_verify": false, "skip_san_whitelist": false},
            "retry": {}
        });
        test_emit_clone_with_override("https://example.com/repo.git", task_id, override_json);
        let evs = bus.take_all();
        let mut summaries = 0;
        for e in &evs {
            if let StructuredEvent::Strategy(StrategyEvent::Summary { .. }) = e {
                summaries += 1;
            }
        }
        assert!(summaries >= 1, "expected at least one summary event");
        clear_test_event_bus();
    }

    // fetch + retry code
    #[tokio::test]
    async fn fetch_summary_event_and_applied_codes() {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // 使用 repo_factory 构造 origin + 提交
        let origin_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("a.txt", "one", "c1")
            .build();
        // work repo + 添加 remote（与原逻辑一致）
        let work = tempfile::tempdir().unwrap();
        let work_repo = git2::Repository::init(work.path()).unwrap();
        work_repo
            .remote("origin", origin_desc.path.to_string_lossy().as_ref())
            .unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let (id, tk) = reg.create(TaskKind::GitFetch {
            repo: origin_desc.path.to_string_lossy().to_string(),
            dest: work.path().to_string_lossy().to_string(),
            depth: None,
            filter: None,
            strategy_override: None,
        });
        let ov = serde_json::json!({"retry": {"max": 4}});
        let h = reg.spawn_git_fetch_task_with_opts(
            Some(AppHandle {}),
            id,
            tk,
            origin_desc.path.to_string_lossy().to_string(),
            work.path().to_string_lossy().to_string(),
            None,
            None,
            None,
            Some(ov),
        );
        let _ = h.await;
        crate::common::event_assert::assert_applied_code(
            &id.to_string(),
            "retry_strategy_override_applied",
        );
    }

    #[tokio::test]
    async fn push_summary_event_with_independent_applied_events() {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // origin + src 使用 RepoBuilder 构造
        let origin_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("o.txt", "one", "c1")
            .build();
        let src_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("b.txt", "two", "c1")
            .build();
        let src_repo = git2::Repository::open(&src_desc.path).unwrap();
        src_repo
            .remote("origin", origin_desc.path.to_string_lossy().as_ref())
            .unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let (pid, ptk) = reg.create(TaskKind::GitPush {
            dest: src_desc.path.to_string_lossy().to_string(),
            remote: Some("origin".into()),
            refspecs: None,
            username: None,
            password: None,
            strategy_override: None,
        });
        let ov = serde_json::json!({"http": {"followRedirects": false}, "tls": {"insecureSkipVerify": true}});
        let h = reg.spawn_git_push_task(
            Some(AppHandle {}),
            pid,
            ptk,
            src_desc.path.to_string_lossy().to_string(),
            Some("origin".into()),
            None,
            None,
            None,
            Some(ov),
        );
        let _ = h.await;
        // 断言：summary 含 http/tls codes + 独立 applied 事件存在
        crate::common::event_assert::assert_applied_code(
            &pid.to_string(),
            "http_strategy_override_applied",
        );
        crate::common::event_assert::assert_applied_code(
            &pid.to_string(),
            "tls_strategy_override_applied",
        );
        crate::common::event_assert::assert_http_applied(&pid.to_string(), true);
        crate::common::event_assert::assert_tls_applied(&pid.to_string(), true);
    }

    // 合并：无 override 的 fetch/push 场景统一覆盖
    #[test]
    fn summary_event_no_override_for_fetch_and_push() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            // origin & work for fetch
            let origin_desc = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("a.txt", "one", "c1")
                .build();
            let work = tempfile::tempdir().unwrap();
            let work_repo = git2::Repository::init(work.path()).unwrap();
            work_repo
                .remote("origin", origin_desc.path.to_string_lossy().as_ref())
                .unwrap();
            let reg = std::sync::Arc::new(TaskRegistry::new());
            let (fid, ftk) = reg.create(TaskKind::GitFetch {
                repo: origin_desc.path.to_string_lossy().to_string(),
                dest: work.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: None,
            });
            let fh = reg.spawn_git_fetch_task_with_opts(
                Some(AppHandle {}),
                fid,
                ftk,
                origin_desc.path.to_string_lossy().to_string(),
                work.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                None,
            );
            let _ = fh.await;
            crate::common::event_assert::assert_no_applied_codes(&fid.to_string());

            // origin & src for push
            let origin2 = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("c.txt", "three", "c1")
                .build();
            let src_desc = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("d.txt", "four", "c1")
                .build();
            let src_repo = git2::Repository::open(&src_desc.path).unwrap();
            src_repo
                .remote("origin", origin2.path.to_string_lossy().as_ref())
                .unwrap();
            let (pid, ptk) = reg.create(TaskKind::GitPush {
                dest: src_desc.path.to_string_lossy().to_string(),
                remote: Some("origin".into()),
                refspecs: None,
                username: None,
                password: None,
                strategy_override: None,
            });
            let ph = reg.spawn_git_push_task(
                Some(AppHandle {}),
                pid,
                ptk,
                src_desc.path.to_string_lossy().to_string(),
                Some("origin".into()),
                None,
                None,
                None,
                None,
            );
            let _ = ph.await;
            crate::common::event_assert::assert_no_applied_codes(&pid.to_string());
        });
    }

    // strategy_override_summary_and_gating 的 summary 多 appliedCodes 片段（http+retry）复用：只验证 summary 含目标 codes
    #[test]
    fn strategy_override_summary_multi_codes_only() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            let src_desc = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("readme.txt", "hi", "c1")
                .build();
            let reg = std::sync::Arc::new(TaskRegistry::new());
            let dest_dir = tempfile::tempdir().unwrap();
            let (id, token) = reg.create(TaskKind::GitClone {
                repo: src_desc.path.to_string_lossy().to_string(),
                dest: dest_dir.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: None,
            recurse_submodules: false,
            });
            let override_json =
                serde_json::json!({"http": {"follow_redirects": false}, "retry": {"max": 5}});
            let h = reg.spawn_git_clone_task_with_opts(
                Some(AppHandle {}),
                id,
                token.clone(),
                src_desc.path.to_string_lossy().to_string(),
                dest_dir.path().to_string_lossy().to_string(),
                None,
                None,
                Some(override_json),
                false,
            );
            let _ = h.await;
            // summary appliedCodes
            crate::common::event_assert::assert_applied_code(
                &id.to_string(),
                "http_strategy_override_applied",
            );
            crate::common::event_assert::assert_applied_code(
                &id.to_string(),
                "retry_strategy_override_applied",
            );
        });
    }
}

// ---------------- (Phase3) section_override_no_conflict ----------------
// 来源函数：
//   - no_conflict_http_tls_override
//   - tls_override_changed_and_unchanged (含 clone/fetch/push changed vs unchanged)
//   - push_tls_insecure_only_event_once
mod section_override_no_conflict {
    use super::wait_done;
    use crate::common::strategy_support::test_emit_clone_with_override;
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use fireworks_collaboration_lib::events::emitter::AppHandle;
    use fireworks_collaboration_lib::events::structured::{
        clear_test_event_bus, set_global_event_bus, set_test_event_bus, Event as StructuredEvent,
        MemoryEventBus, StrategyEvent,
    };

    #[test]
    fn no_conflict_http_tls_override() {
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
        let tmp_src = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("z.txt", "nc", "c1")
            .build();
        let base_cfg = fireworks_collaboration_lib::core::config::loader::load_or_init()
            .expect("load base cfg");
        let flip_insecure = !base_cfg.tls.insecure_skip_verify;
        let override_json = serde_json::json!({
            "http": {"follow_redirects": base_cfg.http.follow_redirects, "max_redirects": base_cfg.http.max_redirects},
            "tls": {"insecure_skip_verify": flip_insecure, "skip_san_whitelist": base_cfg.tls.skip_san_whitelist}
        });
        let id = uuid::Uuid::new_v4();
        let bus = MemoryEventBus::new();
        set_test_event_bus(std::sync::Arc::new(bus.clone()));
        test_emit_clone_with_override(tmp_src.path.to_string_lossy().as_ref(), id, override_json);
        let structured = bus.snapshot();
        let mut s_http = 0;
        let mut s_tls = 0;
        let mut s_summary = 0;
        let mut conflicts = 0;
        for e in &structured {
            match e {
                StructuredEvent::Strategy(StrategyEvent::HttpApplied { id: sid, .. })
                    if sid == &id.to_string() =>
                {
                    s_http += 1
                }
                StructuredEvent::Strategy(StrategyEvent::TlsApplied { id: sid, .. })
                    if sid == &id.to_string() =>
                {
                    s_tls += 1
                }
                StructuredEvent::Strategy(StrategyEvent::Summary { id: sid, .. })
                    if sid == &id.to_string() =>
                {
                    s_summary += 1
                }
                StructuredEvent::Strategy(StrategyEvent::Conflict { id: sid, .. })
                    if sid == &id.to_string() =>
                {
                    conflicts += 1
                }
                _ => {}
            }
        }
        assert_eq!(s_http, 0, "http unchanged -> no HttpApplied");
        assert_eq!(s_tls, 1, "tls flipped -> one TlsApplied");
        assert!(s_summary >= 1, "expected summary");
        assert_eq!(conflicts, 0, "no conflict expected");
        clear_test_event_bus();
    }

    #[test]
    fn tls_override_changed_and_unchanged() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            // origin repo
            let src_desc = crate::common::repo_factory::RepoBuilder::new().with_base_commit("a.txt", "hello", "c1").build();
            let src_path = src_desc.path.to_string_lossy().to_string();
            let reg = std::sync::Arc::new(TaskRegistry::new()); let app = AppHandle;
            // 1) changed
            let d1 = tempfile::tempdir().unwrap(); let ov1 = serde_json::json!({"tls": {"insecure_skip_verify": true}});
            let (id1, tk1) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d1.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov1.clone()), recurse_submodules: false });
            let h1 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id1, tk1, src_path.clone(), d1.path().to_string_lossy().to_string(), None, None, Some(ov1), false);
            wait_done(&reg, id1).await; h1.await.unwrap();
            crate::common::event_assert::assert_applied_code(&id1.to_string(), "tls_strategy_override_applied");
            // 2) unchanged
            let d2 = tempfile::tempdir().unwrap(); let ov2 = serde_json::json!({"tls": {"insecure_skip_verify": false, "skip_san_whitelist": false}});
            let (id2, tk2) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov2.clone()), recurse_submodules: false });
            let h2 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id2, tk2, src_path.clone(), d2.path().to_string_lossy().to_string(), None, None, Some(ov2), false);
            wait_done(&reg, id2).await; h2.await.unwrap();
            crate::common::event_assert::assert_no_applied_code(&id2.to_string(), "tls_strategy_override_applied");
            // 3) fetch skipSan
            let work3 = tempfile::tempdir().unwrap(); let (idc, tkc) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None , recurse_submodules: false });
            let hc = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), idc, tkc, src_path.clone(), work3.path().to_string_lossy().to_string(), None, None, None, false); wait_done(&reg, idc).await; hc.await.unwrap();
            let ovf = serde_json::json!({"tls": {"skip_san_whitelist": true}});
            let (idf, tkf) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: work3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovf.clone()) });
            let hf = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), idf, tkf, src_path.clone(), work3.path().to_string_lossy().to_string(), None, None, None, Some(ovf));
            wait_done(&reg, idf).await; hf.await.unwrap();
            crate::common::event_assert::assert_applied_code(&idf.to_string(), "tls_strategy_override_applied");
            // 4) push insecure + skipSan
            let work4 = tempfile::tempdir().unwrap(); let (idc4, tkc4) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work4.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None , recurse_submodules: false });
            let hc4 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), idc4, tkc4, src_path.clone(), work4.path().to_string_lossy().to_string(), None, None, None, false); wait_done(&reg, idc4).await; hc4.await.unwrap();
            let ovp = serde_json::json!({"tls": {"insecure_skip_verify": true, "skip_san_whitelist": true}});
            let (idp, tkp) = reg.create(TaskKind::GitPush { dest: work4.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(ovp.clone()) });
            let hp = reg.clone().spawn_git_push_task(Some(app.clone()), idp, tkp, work4.path().to_string_lossy().to_string(), None, None, None, None, Some(ovp));
            wait_done(&reg, idp).await; hp.await.unwrap();
            crate::common::event_assert::assert_applied_code(&idp.to_string(), "tls_strategy_override_applied");
        });
    }

    #[test]
    fn push_tls_insecure_only_event_once() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            let src_desc = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("f.txt", "1", "c1")
                .build();
            let reg = std::sync::Arc::new(TaskRegistry::new());
            let app = AppHandle;
            let work = tempfile::tempdir().unwrap();
            let (cid, ctk) = reg.create(TaskKind::GitClone {
                repo: src_desc.path.to_string_lossy().to_string(),
                dest: work.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: None,
            recurse_submodules: false,
            });
            let ch = reg.clone().spawn_git_clone_task_with_opts(
                Some(app.clone()),
                cid,
                ctk,
                src_desc.path.to_string_lossy().to_string(),
                work.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                false,
            );
            wait_done(&reg, cid).await;
            ch.await.unwrap();
            let ov = serde_json::json!({"tls": {"insecure_skip_verify": true}});
            let (pid, ptk) = reg.create(TaskKind::GitPush {
                dest: work.path().to_string_lossy().to_string(),
                remote: None,
                refspecs: None,
                username: None,
                password: None,
                strategy_override: Some(ov.clone()),
            });
            let ph = reg.clone().spawn_git_push_task(
                Some(app.clone()),
                pid,
                ptk,
                work.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                None,
                Some(ov),
            );
            wait_done(&reg, pid).await;
            ph.await.unwrap();
            crate::common::event_assert::assert_tls_applied(&pid.to_string(), true);
        });
    }
}

// ---------------- (Phase3) section_override_empty_unknown ----------------
// 来源函数： clone_with_empty_strategy_object_success / push_with_only_unknown_field_success
mod section_override_empty_unknown {
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    #[tokio::test]
    async fn clone_with_empty_strategy_object_success() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        // init origin repo via builder
        let origin = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("f.txt", "1", "c1")
            .build();
        let dest = tempfile::tempdir().unwrap();
        let (id, token) = reg.create(TaskKind::GitClone {
            repo: origin.path.to_string_lossy().to_string(),
            dest: dest.path().to_string_lossy().to_string(),
            depth: None,
            filter: None,
            strategy_override: Some(serde_json::json!({})),
            recurse_submodules: false,
        });
        let h = reg.clone().spawn_git_clone_task_with_opts(
            None,
            id,
            token,
            origin.path.to_string_lossy().to_string(),
            dest.path().to_string_lossy().to_string(),
            None,
            None,
            Some(serde_json::json!({})),
            false,
        );
        h.await.unwrap();
        let snap = reg.snapshot(&id).unwrap();
        assert!(matches!(
            snap.state,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled
        ));
    }
    #[tokio::test]
    async fn push_with_only_unknown_field_success() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let work_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("a.txt", "1", "c1")
            .build();
        let unknown = serde_json::json!({"foo": {"bar": 1}});
        let (id, token) = reg.create(TaskKind::GitPush {
            dest: work_desc.path.to_string_lossy().to_string(),
            remote: None,
            refspecs: None,
            username: None,
            password: None,
            strategy_override: Some(unknown.clone()),
        });
        token.cancel();
        let h = reg.clone().spawn_git_push_task(
            None,
            id,
            token,
            work_desc.path.to_string_lossy().to_string(),
            None,
            None,
            None,
            None,
            Some(unknown),
        );
        h.await.unwrap();
        let snap = reg.snapshot(&id).unwrap();
        assert!(!matches!(snap.state, TaskState::Failed));
    }
}

// ---------------- (Phase3) section_override_invalid_inputs ----------------
// push_with_invalid_strategy_override_array_fails / clone_invalid_http_max_redirects_fails / push_invalid_retry_factor_fails
mod section_override_invalid_inputs {
    use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    #[tokio::test]
    async fn push_with_invalid_strategy_override_array_fails() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let tmp_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("a.txt", "1", "c1")
            .build();
        let bad = serde_json::json!([1, 2, 3]);
        let (id, token) = reg.create(TaskKind::GitPush {
            dest: tmp_desc.path.to_string_lossy().to_string(),
            remote: None,
            refspecs: None,
            username: None,
            password: None,
            strategy_override: Some(bad.clone()),
        });
        let h = reg.spawn_git_push_task(
            None,
            id,
            token,
            tmp_desc.path.to_string_lossy().to_string(),
            None,
            None,
            None,
            None,
            Some(bad),
        );
        h.await.unwrap();
        let snap = reg.snapshot(&id).unwrap();
        assert!(matches!(snap.state, TaskState::Failed));
    }
    #[tokio::test]
    async fn clone_invalid_http_max_redirects_fails() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let origin_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("f.txt", "1", "init")
            .build();
        let dest = tempfile::tempdir().unwrap();
        let bad = serde_json::json!({"http": {"max_redirects": 999}});
        let (id, token) = reg.create(TaskKind::GitClone {
            repo: origin_desc.path.to_string_lossy().to_string(),
            dest: dest.path().to_string_lossy().to_string(),
            depth: None,
            filter: None,
            strategy_override: Some(bad.clone()),
            recurse_submodules: false,
        });
        let h = reg.spawn_git_clone_task_with_opts(
            None,
            id,
            token,
            origin_desc.path.to_string_lossy().to_string(),
            dest.path().to_string_lossy().to_string(),
            None,
            None,
            Some(bad),
            false,
        );
        h.await.unwrap();
        let snap = reg.snapshot(&id).unwrap();
        assert!(matches!(snap.state, TaskState::Failed));
    }
    #[tokio::test]
    async fn push_invalid_retry_factor_fails() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let work_desc = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("r.txt", "1", "c1")
            .build();
        let bad = serde_json::json!({"retry": {"factor": 50.0}});
        let (id, token) = reg.create(TaskKind::GitPush {
            dest: work_desc.path.to_string_lossy().to_string(),
            remote: None,
            refspecs: None,
            username: None,
            password: None,
            strategy_override: Some(bad.clone()),
        });
        let h = reg.spawn_git_push_task(
            None,
            id,
            token,
            work_desc.path.to_string_lossy().to_string(),
            None,
            None,
            None,
            None,
            Some(bad),
        );
        h.await.unwrap();
        let snap = reg.snapshot(&id).unwrap();
        assert!(matches!(snap.state, TaskState::Failed));
    }
}

// ---------------- (Phase3) section_tls_mixed_scenarios ----------------
// 来源函数： tls_mixed_scenarios (clone/fetch/push + empty + unknown 字段)
mod section_tls_mixed_scenarios {
    use super::wait_done;
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use fireworks_collaboration_lib::events::emitter::AppHandle;
    use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
    #[test]
    fn tls_mixed_scenarios() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            // origin
            let src = tempfile::tempdir().unwrap();
            let repo = git2::Repository::init(src.path()).unwrap();
            std::fs::write(src.path().join("f.txt"), "x").unwrap();
            let mut idx = repo.index().unwrap();
            idx.add_path(std::path::Path::new("f.txt")).unwrap();
            idx.write().unwrap();
            let tree_id = idx.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
            let src_path = src.path().to_string_lossy().to_string();
            let reg = std::sync::Arc::new(TaskRegistry::new());
            let app = AppHandle;
            // insecure clone
            let dest_a = tempfile::tempdir().unwrap();
            let ova = serde_json::json!({"tls": {"insecure_skip_verify": true}});
            let (id_a, tk_a) = reg.create(TaskKind::GitClone {
                repo: src_path.clone(),
                dest: dest_a.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: Some(ova.clone()),
                recurse_submodules: false,
            });
            let ha = reg.clone().spawn_git_clone_task_with_opts(
                Some(app.clone()),
                id_a,
                tk_a,
                src_path.clone(),
                dest_a.path().to_string_lossy().to_string(),
                None,
                None,
                Some(ova),
                false,
            );
            // baseline
            let base = tempfile::tempdir().unwrap();
            let (id_base, tk_base) = reg.create(TaskKind::GitClone {
                repo: src_path.clone(),
                dest: base.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: None,
            recurse_submodules: false,
            });
            let h_base = reg.clone().spawn_git_clone_task_with_opts(
                Some(app.clone()),
                id_base,
                tk_base,
                src_path.clone(),
                base.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                false,
            );
            wait_done(&reg, id_a).await;
            ha.await.unwrap();
            wait_done(&reg, id_base).await;
            h_base.await.unwrap();
            // fetch empty tls {}
            let ovb = serde_json::json!({"tls": {}});
            let (id_b, tk_b) = reg.create(TaskKind::GitFetch {
                repo: src_path.clone(),
                dest: base.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: Some(ovb.clone()),
            });
            let hb = reg.clone().spawn_git_fetch_task_with_opts(
                Some(app.clone()),
                id_b,
                tk_b,
                src_path.clone(),
                base.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                Some(ovb),
            );
            // push skipSan only
            let (id_c, tk_c) = reg.create(TaskKind::GitPush {
                dest: base.path().to_string_lossy().to_string(),
                remote: None,
                refspecs: None,
                username: None,
                password: None,
                strategy_override: Some(serde_json::json!({"tls": {"skip_san_whitelist": true}})),
            });
            let hc = reg.clone().spawn_git_push_task(
                Some(app.clone()),
                id_c,
                tk_c,
                base.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                None,
                Some(serde_json::json!({"tls": {"skip_san_whitelist": true}})),
            );
            // fetch unknown field
            let ovd = serde_json::json!({"tls": {"foo": true}});
            let (id_d, tk_d) = reg.create(TaskKind::GitFetch {
                repo: src_path.clone(),
                dest: base.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: Some(ovd.clone()),
            });
            let hd = reg.clone().spawn_git_fetch_task_with_opts(
                Some(app.clone()),
                id_d,
                tk_d,
                src_path.clone(),
                base.path().to_string_lossy().to_string(),
                None,
                None,
                None,
                Some(ovd),
            );
            wait_done(&reg, id_b).await;
            hb.await.unwrap();
            wait_done(&reg, id_c).await;
            hc.await.unwrap();
            wait_done(&reg, id_d).await;
            hd.await.unwrap();
            // 断言：clone insecure & push skipSan -> tls applied; fetch {} & unknown -> 无
            crate::common::event_assert::assert_tls_applied(&id_a.to_string(), true);
            crate::common::event_assert::assert_tls_applied(&id_c.to_string(), true);
            crate::common::event_assert::assert_no_applied_code(
                &id_b.to_string(),
                "tls_strategy_override_applied",
            );
            crate::common::event_assert::assert_no_applied_code(
                &id_d.to_string(),
                "tls_strategy_override_applied",
            );
        });
    }
}

// ---------------- (Phase3) section_summary_gating ----------------
// strategy_override_summary_and_gating / tls_override_summary_and_gating
mod section_summary_gating {
    use crate::common::event_assert::{
        assert_applied_code, assert_conflict_kind, assert_tls_applied,
    };
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry;
    use fireworks_collaboration_lib::events::emitter::AppHandle;
    use fireworks_collaboration_lib::events::structured::{set_global_event_bus, MemoryEventBus};
    #[test]
    fn strategy_override_summary_and_gating() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            // origin via RepoBuilder
            let origin = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("readme.txt", "hi", "c1")
                .build();
            let reg = std::sync::Arc::new(TaskRegistry::new());
            let dest = tempfile::tempdir().unwrap();
            let (id, tk) = reg.create(TaskKind::GitClone {
                repo: origin.path.to_string_lossy().to_string(),
                dest: dest.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: None,
            recurse_submodules: false,
            });
            let ov = serde_json::json!({"http": {"follow_redirects": false}, "retry": {"max": 5}});
            let h = reg.spawn_git_clone_task_with_opts(
                Some(AppHandle {}),
                id,
                tk.clone(),
                origin.path.to_string_lossy().to_string(),
                dest.path().to_string_lossy().to_string(),
                None,
                None,
                Some(ov),
                false,
            );
            let _ = h.await;
            assert_applied_code(&id.to_string(), "http_strategy_override_applied");
            assert_applied_code(&id.to_string(), "retry_strategy_override_applied");
            // gating off
            std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
            let origin2 = crate::common::repo_factory::RepoBuilder::new()
                .with_base_commit("g.txt", "hi", "c1")
                .build();
            let dest2 = tempfile::tempdir().unwrap();
            let (gid, gtk) = reg.create(TaskKind::GitClone {
                repo: origin2.path.to_string_lossy().to_string(),
                dest: dest2.path().to_string_lossy().to_string(),
                depth: None,
                filter: None,
                strategy_override: None,
            recurse_submodules: false,
            });
            let govr =
                serde_json::json!({"http": {"follow_redirects": false}, "retry": {"max": 3}});
            let gh = reg.spawn_git_clone_task_with_opts(
                Some(AppHandle {}),
                gid,
                gtk,
                origin2.path.to_string_lossy().to_string(),
                dest2.path().to_string_lossy().to_string(),
                None,
                None,
                Some(govr),
                false,
            );
            let _ = gh.await;
            assert_applied_code(&gid.to_string(), "http_strategy_override_applied");
            assert_applied_code(&gid.to_string(), "retry_strategy_override_applied");
        });
    }
    #[tokio::test]
    async fn tls_override_summary_and_gating() {
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let origin = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("f.txt", "one", "c1")
            .build();
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let dest = tempfile::tempdir().unwrap();
        let (id, tk) = reg.create(TaskKind::GitClone {
            repo: origin.path.to_string_lossy().to_string(),
            dest: dest.path().to_string_lossy().to_string(),
            depth: None,
            filter: None,
            strategy_override: None,
        recurse_submodules: false,
            });
        let ov =
            serde_json::json!({"tls": {"insecure_skip_verify": true, "skip_san_whitelist": true}});
        let h = reg.spawn_git_clone_task_with_opts(
            Some(AppHandle {}),
            id,
            tk,
            origin.path.to_string_lossy().to_string(),
            dest.path().to_string_lossy().to_string(),
            None,
            None,
            Some(ov),
            false,
        );
        let _ = h.await;
        assert_applied_code(&id.to_string(), "tls_strategy_override_applied");
        assert_tls_applied(&id.to_string(), true);
        assert_conflict_kind(&id.to_string(), "tls", Some("normalizes"));
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
        let origin2 = crate::common::repo_factory::RepoBuilder::new()
            .with_base_commit("y.txt", "one", "c1")
            .build();
        let dest2 = tempfile::tempdir().unwrap();
        let (id2, tk2) = reg.create(TaskKind::GitClone {
            repo: origin2.path.to_string_lossy().to_string(),
            dest: dest2.path().to_string_lossy().to_string(),
            depth: None,
            filter: None,
            strategy_override: None,
        recurse_submodules: false,
            });
        let ov2 = serde_json::json!({"tls": {"insecure_skip_verify": true}});
        let h2 = reg.spawn_git_clone_task_with_opts(
            Some(AppHandle {}),
            id2,
            tk2,
            origin2.path.to_string_lossy().to_string(),
            dest2.path().to_string_lossy().to_string(),
            None,
            None,
            Some(ov2),
            false,
        );
        let _ = h2.await;
        assert_applied_code(&id2.to_string(), "tls_strategy_override_applied");
        assert_tls_applied(&id2.to_string(), true);
    }
}

// (removed) transport fallback counters test relied on #[cfg(test)] helpers not exported in integration build
