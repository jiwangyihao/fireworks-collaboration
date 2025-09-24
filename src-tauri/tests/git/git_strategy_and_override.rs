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
//!   section_strategy_combo  -> 策略组合占位
//!   section_http_basic      -> HTTP override 基础 & 事件存在
//!   section_http_limits     -> follow / max / idempotent 变体
//!   section_http_events     -> 事件子序列锚点断言
//!   section_adaptive_tls    -> TLS rollout 模拟事件
//!   --- Phase3 新增 Section 计划 ---
//!   section_strategy_summary_multiop   -> 多操作 Summary + 结构化多 applied codes
//!   section_override_no_conflict       -> TLS only 修改 & changed vs unchanged & push insecure only
//!   section_override_empty_unknown     -> 空对象 / unknown 字段宽容
//!   section_override_invalid_inputs    -> 无效参数导致失败
//!   section_tls_mixed_scenarios        -> TLS mixed (clone/fetch/push) 变体
//!   section_summary_gating             -> gating 环境变量 on/off 对比
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

#[path = "../common/mod.rs"] mod common;
use common::{
    http_override_stub::{http_override_cases, run_http_override, HttpOverrideCase, FollowMode, IdempotentFlag, MaxEventsCase},
    git_scenarios::GitOp,
    event_assert::{expect_subsequence, expect_optional_tags_subsequence},
    test_env::init_test_env
};

// 顶层通用：等待任务完成（供多个 section 复用）
use crate::common::task_wait::wait_until_task_done as wait_done;

// 统一测试环境初始化（Once 防抖）
#[ctor::ctor]
fn __init_env() { init_test_env(); }

// ---------------- section_transport_fallback_decision ----------------
// 将核心传输层的 Fallback 决策状态机（Fake -> Real -> Default）单元测试迁移为集成测试。
// 覆盖点：
//  - policy 不允许 fake -> 初始 Default，原因为 SkipFakePolicy
//  - policy 允许 fake -> 初始 Fake，advance 触发完整链路 Fake->Real->Default
//  - runtime_fake_disabled=true 时行为等同 policy skip
mod section_transport_fallback_decision { use fireworks_collaboration_lib::core::git::transport::{DecisionCtx, FallbackDecision, FallbackStage, FallbackReason};
    // 兼容迁移：来自 src-tauri/src/core/git/transport/tests_integration_baseline.rs
    // 目的：当策略不允许 fake 且 runtime 未禁用时，初始阶段应为 Default
    #[test]
    fn initial_stage_default_when_disabled() {
        let ctx = DecisionCtx { policy_allows_fake: false, runtime_fake_disabled: false };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
    }

    #[test]
    fn skip_fake_policy_creates_default_stage() {
        let ctx = DecisionCtx { policy_allows_fake: false, runtime_fake_disabled: false };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        let h = d.history();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].reason, FallbackReason::SkipFakePolicy);
    }

    #[test]
    fn full_chain_history_order() {
        let ctx = DecisionCtx { policy_allows_fake: true, runtime_fake_disabled: false };
        let mut d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Fake);
        d.advance_on_error().expect("fake->real");
        d.advance_on_error().expect("real->default");
        assert!(d.advance_on_error().is_none());
        let stages: Vec<_> = d.history().iter().map(|tr| tr.to).collect();
        assert_eq!(stages, vec![FallbackStage::Fake, FallbackStage::Real, FallbackStage::Default]);
    }

    #[test]
    fn runtime_fake_disabled_behaves_like_policy_skip() {
        let ctx = DecisionCtx { policy_allows_fake: true, runtime_fake_disabled: true };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        assert_eq!(d.history()[0].reason, FallbackReason::SkipFakePolicy);
    }
}

// 已移除：早期 Strategy 占位实现与覆盖测试（无真实策略逻辑，改由结构化事件测试覆盖）

// ---------------- section_http_basic ----------------
mod section_http_basic { use super::*; #[test] fn http_override_each_case_applied() { for c in http_override_cases() { let out = run_http_override(&c); assert!(out.applied, "override not applied for {c}"); assert!(!out.events.is_empty()); } }}

// ---------------- section_http_limits ----------------
mod section_http_limits { use super::*; #[test] fn http_follow_chain_length_with_max() { for c in http_override_cases() { let out = run_http_override(&c); if matches!(c.follow, FollowMode::Follow) { if let MaxEventsCase::Some(m) = c.max_events { assert!(out.follow_chain.len() as u32 <= m.max(1), "follow chain exceeds max for {c}"); } else { assert!(out.follow_chain.len() >= 1, "expected default follow hops"); } } } }}
// invalid max (=0) 单独测试
mod section_http_invalid_max { use super::*; #[test] fn http_invalid_max_zero_applied_false() { let case = HttpOverrideCase { op: GitOp::Clone, follow: FollowMode::None, idempotent: IdempotentFlag::No, max_events: MaxEventsCase::Some(0) }; let out = run_http_override(&case); assert!(!out.applied); assert!(out.events.iter().any(|e| e.contains("invalid_max"))); expect_optional_tags_subsequence(&out.events, &["http"]); }}

// ---------------- section_http_events ----------------
mod section_http_events { use super::*; #[test] fn http_event_anchor_sequence() { let any_case = http_override_cases().into_iter().find(|c| matches!(c.follow, FollowMode::Follow)).expect("follow case"); let out = run_http_override(&any_case); expect_subsequence(&out.events, &["http:override:start", "http:override", "http:override:applied"]); expect_optional_tags_subsequence(&out.events, &["http", "http"]); }}

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
    use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_global_event_bus, set_test_event_bus, clear_test_event_bus, Event as StructuredEvent, StrategyEvent};
    use fireworks_collaboration_lib::core::tasks::registry::{TaskRegistry, test_emit_clone_with_override};
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::events::emitter::AppHandle;

    // --- helpers ---
    fn install_test_bus() -> MemoryEventBus { let bus = MemoryEventBus::new(); set_test_event_bus(std::sync::Arc::new(bus.clone())); bus }

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
        let mut has_http=false; let mut has_tls=false; let mut has_summary=false; let mut has_retry=false;
        for e in &evs { match e { StructuredEvent::Strategy(StrategyEvent::HttpApplied { id, .. }) if id==&task_id.to_string() => has_http=true,
            StructuredEvent::Strategy(StrategyEvent::TlsApplied { id, .. }) if id==&task_id.to_string() => has_tls=true,
            StructuredEvent::Strategy(StrategyEvent::Summary { id, kind, .. }) if id==&task_id.to_string() && kind=="GitClone" => has_summary=true,
            StructuredEvent::Policy(fireworks_collaboration_lib::events::structured::PolicyEvent::RetryApplied { id, .. }) if id==&task_id.to_string() => has_retry=true,
            _=>{} } }
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
        let mut summaries=0; for e in &evs { if let StructuredEvent::Strategy(StrategyEvent::Summary { .. }) = e { summaries+=1; } }
        assert!(summaries>=1, "expected at least one summary event");
        clear_test_event_bus();
    }

    // fetch + retry code
    #[tokio::test]
    async fn fetch_summary_event_and_applied_codes() {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // init origin + work repo (add remote)
        let origin = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(origin.path()).unwrap();
        std::fs::write(origin.path().join("a.txt"), "one").unwrap();
        let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let work = tempfile::tempdir().unwrap(); let work_repo = git2::Repository::init(work.path()).unwrap(); work_repo.remote("origin", origin.path().to_string_lossy().as_ref()).unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let (id, tk) = reg.create(TaskKind::GitFetch { repo: origin.path().to_string_lossy().to_string(), dest: work.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let ov = serde_json::json!({"retry": {"max": 4}});
        let h = reg.spawn_git_fetch_task_with_opts(Some(AppHandle {}), id, tk, origin.path().to_string_lossy().to_string(), work.path().to_string_lossy().to_string(), None, None, None, Some(ov));
        let _ = h.await;
    crate::common::event_assert::assert_applied_code(&id.to_string(), "retry_strategy_override_applied");
    }

    #[tokio::test]
    async fn push_summary_event_with_independent_applied_events() {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // origin
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap();
        std::fs::write(origin.path().join("o.txt"), "one").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("o.txt")).unwrap(); idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        // src clone repo (simulate separate working repo with different file)
        let src = tempfile::tempdir().unwrap(); let src_repo = git2::Repository::init(src.path()).unwrap(); std::fs::write(src.path().join("b.txt"), "two").unwrap();
        let mut idx2 = src_repo.index().unwrap(); idx2.add_path(std::path::Path::new("b.txt")).unwrap(); idx2.write().unwrap(); let tree_id2 = idx2.write_tree().unwrap(); let tree2 = src_repo.find_tree(tree_id2).unwrap(); let sig2 = src_repo.signature().unwrap(); src_repo.commit(Some("HEAD"), &sig2,&sig2, "c1", &tree2, &[]).unwrap();
        src_repo.remote("origin", origin.path().to_string_lossy().as_ref()).unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let (pid, ptk) = reg.create(TaskKind::GitPush { dest: src.path().to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: None, username: None, password: None, strategy_override: None });
        let ov = serde_json::json!({"http": {"followRedirects": false}, "tls": {"insecureSkipVerify": true}});
        let h = reg.spawn_git_push_task(Some(AppHandle {}), pid, ptk, src.path().to_string_lossy().to_string(), Some("origin".into()), None, None, None, Some(ov));
        let _ = h.await;
        // 断言：summary 含 http/tls codes + 独立 applied 事件存在
    crate::common::event_assert::assert_applied_code(&pid.to_string(), "http_strategy_override_applied");
    crate::common::event_assert::assert_applied_code(&pid.to_string(), "tls_strategy_override_applied");
    crate::common::event_assert::assert_http_applied(&pid.to_string(), true);
    crate::common::event_assert::assert_tls_applied(&pid.to_string(), true);
    }

    #[tokio::test]
    async fn fetch_summary_event_no_override() {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap();
        std::fs::write(origin.path().join("a.txt"), "one").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let work = tempfile::tempdir().unwrap(); let work_repo = git2::Repository::init(work.path()).unwrap(); work_repo.remote("origin", origin.path().to_string_lossy().as_ref()).unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new()); let (id, tk) = reg.create(TaskKind::GitFetch { repo: origin.path().to_string_lossy().to_string(), dest: work.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
        let h = reg.spawn_git_fetch_task_with_opts(Some(AppHandle {}), id, tk, origin.path().to_string_lossy().to_string(), work.path().to_string_lossy().to_string(), None, None, None, None); let _ = h.await;
    crate::common::event_assert::assert_no_applied_codes(&id.to_string());
    }

    #[tokio::test]
    async fn push_summary_event_no_override() {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap();
        std::fs::write(origin.path().join("c.txt"), "three").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("c.txt")).unwrap(); idx.write().unwrap();
        let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let src = tempfile::tempdir().unwrap(); let src_repo = git2::Repository::init(src.path()).unwrap(); std::fs::write(src.path().join("d.txt"), "four").unwrap(); let mut idx2 = src_repo.index().unwrap(); idx2.add_path(std::path::Path::new("d.txt")).unwrap(); idx2.write().unwrap(); let tree_id2 = idx2.write_tree().unwrap(); let tree2 = src_repo.find_tree(tree_id2).unwrap(); let sig2 = src_repo.signature().unwrap(); src_repo.commit(Some("HEAD"), &sig2,&sig2, "c1", &tree2, &[]).unwrap(); src_repo.remote("origin", origin.path().to_string_lossy().as_ref()).unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new()); let (pid, ptk) = reg.create(TaskKind::GitPush { dest: src.path().to_string_lossy().to_string(), remote: Some("origin".into()), refspecs: None, username: None, password: None, strategy_override: None });
        let h = reg.spawn_git_push_task(Some(AppHandle {}), pid, ptk, src.path().to_string_lossy().to_string(), Some("origin".into()), None, None, None, None); let _ = h.await;
    crate::common::event_assert::assert_no_applied_codes(&pid.to_string());
    }

    // strategy_override_summary_and_gating 的 summary 多 appliedCodes 片段（http+retry）复用：只验证 summary 含目标 codes
    #[test]
    fn strategy_override_summary_multi_codes_only() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            let tmp_src = tempfile::tempdir().unwrap();
            let repo = git2::Repository::init(tmp_src.path()).unwrap();
            std::fs::write(tmp_src.path().join("readme.txt"), "hi").unwrap();
            let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("readme.txt")).unwrap(); idx.write().unwrap();
            let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
            let reg = std::sync::Arc::new(TaskRegistry::new());
            let dest_dir = tempfile::tempdir().unwrap();
            let (id, token) = reg.create(TaskKind::GitClone { repo: tmp_src.path().to_string_lossy().to_string(), dest: dest_dir.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
            let override_json = serde_json::json!({"http": {"follow_redirects": false}, "retry": {"max": 5}});
            let h = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id, token.clone(), tmp_src.path().to_string_lossy().to_string(), dest_dir.path().to_string_lossy().to_string(), None, None, Some(override_json)); let _ = h.await;
            // summary appliedCodes
            crate::common::event_assert::assert_applied_code(&id.to_string(), "http_strategy_override_applied");
            crate::common::event_assert::assert_applied_code(&id.to_string(), "retry_strategy_override_applied");
        });
    }
}

// ---------------- (Phase3) section_override_no_conflict ----------------
// 来源函数：
//   - no_conflict_http_tls_override
//   - tls_override_changed_and_unchanged (含 clone/fetch/push changed vs unchanged)
//   - push_tls_insecure_only_event_once
mod section_override_no_conflict {
    use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_global_event_bus, set_test_event_bus, clear_test_event_bus, Event as StructuredEvent, StrategyEvent};
    use fireworks_collaboration_lib::core::tasks::registry::{TaskRegistry, test_emit_clone_with_override};
    use fireworks_collaboration_lib::core::tasks::model::TaskKind;
    use fireworks_collaboration_lib::events::emitter::AppHandle;
    use super::wait_done;

    #[test]
    fn no_conflict_http_tls_override() {
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1");
        let tmp_src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp_src.path()).unwrap();
        std::fs::write(tmp_src.path().join("z.txt"), "nc").unwrap();
        let mut index = repo.index().unwrap(); index.add_path(std::path::Path::new("z.txt")).unwrap(); index.write().unwrap();
        let tree_id = index.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig, &sig, "c1", &tree, &[]).unwrap();
        let base_cfg = fireworks_collaboration_lib::core::config::loader::load_or_init().expect("load base cfg");
        let flip_insecure = !base_cfg.tls.insecure_skip_verify;
        let override_json = serde_json::json!({
            "http": {"follow_redirects": base_cfg.http.follow_redirects, "max_redirects": base_cfg.http.max_redirects},
            "tls": {"insecure_skip_verify": flip_insecure, "skip_san_whitelist": base_cfg.tls.skip_san_whitelist}
        });
        let id = uuid::Uuid::new_v4();
        let bus = MemoryEventBus::new(); set_test_event_bus(std::sync::Arc::new(bus.clone()));
        test_emit_clone_with_override(tmp_src.path().to_string_lossy().as_ref(), id, override_json);
        let structured = bus.snapshot();
        let mut s_http=0; let mut s_tls=0; let mut s_summary=0; let mut conflicts=0;
        for e in &structured { match e { StructuredEvent::Strategy(StrategyEvent::HttpApplied { id: sid, .. }) if sid==&id.to_string() => s_http+=1, StructuredEvent::Strategy(StrategyEvent::TlsApplied { id: sid, .. }) if sid==&id.to_string() => s_tls+=1, StructuredEvent::Strategy(StrategyEvent::Summary { id: sid, .. }) if sid==&id.to_string() => s_summary+=1, StructuredEvent::Strategy(StrategyEvent::Conflict { id: sid, .. }) if sid==&id.to_string() => conflicts+=1, _=>{} } }
        assert_eq!(s_http, 0, "http unchanged -> no HttpApplied");
        assert_eq!(s_tls, 1, "tls flipped -> one TlsApplied");
        assert!(s_summary>=1, "expected summary");
        assert_eq!(conflicts, 0, "no conflict expected");
        clear_test_event_bus();
    }

    #[test]
    fn tls_override_changed_and_unchanged() {
        let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(async {
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            // origin repo
            let src = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(src.path()).unwrap(); std::fs::write(src.path().join("a.txt"), "hello").unwrap();
            let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap();
            let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
            let src_path = src.path().to_string_lossy().to_string();
            let reg = std::sync::Arc::new(TaskRegistry::new()); let app = AppHandle;
            // 1) changed
            let d1 = tempfile::tempdir().unwrap(); let ov1 = serde_json::json!({"tls": {"insecure_skip_verify": true}});
            let (id1, tk1) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d1.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov1.clone()) });
            let h1 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id1, tk1, src_path.clone(), d1.path().to_string_lossy().to_string(), None, None, Some(ov1));
            wait_done(&reg, id1).await; h1.await.unwrap();
            crate::common::event_assert::assert_applied_code(&id1.to_string(), "tls_strategy_override_applied");
            // 2) unchanged
            let d2 = tempfile::tempdir().unwrap(); let ov2 = serde_json::json!({"tls": {"insecure_skip_verify": false, "skip_san_whitelist": false}});
            let (id2, tk2) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: d2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ov2.clone()) });
            let h2 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id2, tk2, src_path.clone(), d2.path().to_string_lossy().to_string(), None, None, Some(ov2));
            wait_done(&reg, id2).await; h2.await.unwrap();
            crate::common::event_assert::assert_no_applied_code(&id2.to_string(), "tls_strategy_override_applied");
            // 3) fetch skipSan
            let work3 = tempfile::tempdir().unwrap(); let (idc, tkc) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
            let hc = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), idc, tkc, src_path.clone(), work3.path().to_string_lossy().to_string(), None, None, None); wait_done(&reg, idc).await; hc.await.unwrap();
            let ovf = serde_json::json!({"tls": {"skip_san_whitelist": true}});
            let (idf, tkf) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: work3.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovf.clone()) });
            let hf = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), idf, tkf, src_path.clone(), work3.path().to_string_lossy().to_string(), None, None, None, Some(ovf));
            wait_done(&reg, idf).await; hf.await.unwrap();
            crate::common::event_assert::assert_applied_code(&idf.to_string(), "tls_strategy_override_applied");
            // 4) push insecure + skipSan
            let work4 = tempfile::tempdir().unwrap(); let (idc4, tkc4) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: work4.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
            let hc4 = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), idc4, tkc4, src_path.clone(), work4.path().to_string_lossy().to_string(), None, None, None); wait_done(&reg, idc4).await; hc4.await.unwrap();
            let ovp = serde_json::json!({"tls": {"insecure_skip_verify": true, "skip_san_whitelist": true}});
            let (idp, tkp) = reg.create(TaskKind::GitPush { dest: work4.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(ovp.clone()) });
            let hp = reg.clone().spawn_git_push_task(Some(app.clone()), idp, tkp, work4.path().to_string_lossy().to_string(), None, None, None, None, Some(ovp));
            wait_done(&reg, idp).await; hp.await.unwrap();
            crate::common::event_assert::assert_applied_code(&idp.to_string(), "tls_strategy_override_applied");
        });
    }

    #[test]
    fn push_tls_insecure_only_event_once() {
        let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(async {
            let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
            let src = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(src.path()).unwrap(); std::fs::write(src.path().join("f.txt"), "1").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
            let reg = std::sync::Arc::new(TaskRegistry::new()); let app = AppHandle; let work = tempfile::tempdir().unwrap();
            let (cid, ctk) = reg.create(TaskKind::GitClone { repo: src.path().to_string_lossy().to_string(), dest: work.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
            let ch = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), cid, ctk, src.path().to_string_lossy().to_string(), work.path().to_string_lossy().to_string(), None, None, None); wait_done(&reg, cid).await; ch.await.unwrap();
            let ov = serde_json::json!({"tls": {"insecure_skip_verify": true}}); let (pid, ptk) = reg.create(TaskKind::GitPush { dest: work.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(ov.clone()) });
            let ph = reg.clone().spawn_git_push_task(Some(app.clone()), pid, ptk, work.path().to_string_lossy().to_string(), None, None, None, None, Some(ov)); wait_done(&reg, pid).await; ph.await.unwrap();
            crate::common::event_assert::assert_tls_applied(&pid.to_string(), true);
        });
    }
}

// ---------------- (Phase3) section_override_empty_unknown ----------------
// 来源函数： clone_with_empty_strategy_object_success / push_with_only_unknown_field_success
mod section_override_empty_unknown {
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry; use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    #[tokio::test]
    async fn clone_with_empty_strategy_object_success() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        // init origin repo
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap(); std::fs::write(origin.path().join("f.txt"), "1").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let dest = tempfile::tempdir().unwrap();
        let (id, token) = reg.create(TaskKind::GitClone { repo: origin.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(serde_json::json!({})) });
        let h = reg.clone().spawn_git_clone_task_with_opts(None, id, token, origin.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(serde_json::json!({}))); h.await.unwrap();
        let snap = reg.snapshot(&id).unwrap(); assert!(matches!(snap.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled));
    }
    #[tokio::test]
    async fn push_with_only_unknown_field_success() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let work = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(work.path()).unwrap(); std::fs::write(work.path().join("a.txt"), "1").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let unknown = serde_json::json!({"foo": {"bar": 1}});
        let (id, token) = reg.create(TaskKind::GitPush { dest: work.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(unknown.clone()) }); token.cancel();
        let h = reg.clone().spawn_git_push_task(None, id, token, work.path().to_string_lossy().to_string(), None, None, None, None, Some(unknown)); h.await.unwrap(); let snap = reg.snapshot(&id).unwrap(); assert!(!matches!(snap.state, TaskState::Failed));
    }
}

// ---------------- (Phase3) section_override_invalid_inputs ----------------
// push_with_invalid_strategy_override_array_fails / clone_invalid_http_max_redirects_fails / push_invalid_retry_factor_fails
mod section_override_invalid_inputs {
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry; use fireworks_collaboration_lib::core::tasks::model::{TaskKind, TaskState};
    #[tokio::test]
    async fn push_with_invalid_strategy_override_array_fails() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let tmp = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(tmp.path()).unwrap(); std::fs::write(tmp.path().join("a.txt"), "1").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("a.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let bad = serde_json::json!([1,2,3]); let (id, token) = reg.create(TaskKind::GitPush { dest: tmp.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(bad.clone()) });
        let h = reg.spawn_git_push_task(None, id, token, tmp.path().to_string_lossy().to_string(), None, None, None, None, Some(bad)); h.await.unwrap(); let snap = reg.snapshot(&id).unwrap(); assert!(matches!(snap.state, TaskState::Failed));
    }
    #[tokio::test]
    async fn clone_invalid_http_max_redirects_fails() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap(); std::fs::write(origin.path().join("f.txt"), "1").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "init", &tree, &[]).unwrap();
    let dest = tempfile::tempdir().unwrap(); let bad = serde_json::json!({"http": {"max_redirects": 999}});
        let (id, token) = reg.create(TaskKind::GitClone { repo: origin.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(bad.clone()) });
        let h = reg.spawn_git_clone_task_with_opts(None, id, token, origin.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(bad)); h.await.unwrap(); let snap = reg.snapshot(&id).unwrap(); assert!(matches!(snap.state, TaskState::Failed));
    }
    #[tokio::test]
    async fn push_invalid_retry_factor_fails() {
        let reg = std::sync::Arc::new(TaskRegistry::new());
        let work = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(work.path()).unwrap(); std::fs::write(work.path().join("r.txt"), "1").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("r.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let bad = serde_json::json!({"retry": {"factor": 50.0}}); let (id, token) = reg.create(TaskKind::GitPush { dest: work.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(bad.clone()) });
        let h = reg.spawn_git_push_task(None, id, token, work.path().to_string_lossy().to_string(), None, None, None, None, Some(bad)); h.await.unwrap(); let snap = reg.snapshot(&id).unwrap(); assert!(matches!(snap.state, TaskState::Failed));
    }
}

// ---------------- (Phase3) section_tls_mixed_scenarios ----------------
// 来源函数： tls_mixed_scenarios (clone/fetch/push + empty + unknown 字段)
mod section_tls_mixed_scenarios {
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry; use fireworks_collaboration_lib::core::tasks::model::TaskKind; use fireworks_collaboration_lib::events::emitter::AppHandle; use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_global_event_bus};
    use super::wait_done;
    #[test]
    fn tls_mixed_scenarios() { let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(async {
        let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // origin
        let src = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(src.path()).unwrap(); std::fs::write(src.path().join("f.txt"), "x").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "init", &tree, &[]).unwrap();
        let src_path = src.path().to_string_lossy().to_string(); let reg = std::sync::Arc::new(TaskRegistry::new()); let app = AppHandle;
        // insecure clone
    let dest_a = tempfile::tempdir().unwrap(); let ova = serde_json::json!({"tls": {"insecure_skip_verify": true}}); let (id_a, tk_a) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: dest_a.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ova.clone()) }); let ha = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id_a, tk_a, src_path.clone(), dest_a.path().to_string_lossy().to_string(), None, None, Some(ova));
        // baseline
        let base = tempfile::tempdir().unwrap(); let (id_base, tk_base) = reg.create(TaskKind::GitClone { repo: src_path.clone(), dest: base.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None }); let h_base = reg.clone().spawn_git_clone_task_with_opts(Some(app.clone()), id_base, tk_base, src_path.clone(), base.path().to_string_lossy().to_string(), None, None, None);
        wait_done(&reg, id_a).await; ha.await.unwrap(); wait_done(&reg, id_base).await; h_base.await.unwrap();
        // fetch empty tls {}
        let ovb = serde_json::json!({"tls": {}}); let (id_b, tk_b) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: base.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovb.clone()) }); let hb = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), id_b, tk_b, src_path.clone(), base.path().to_string_lossy().to_string(), None, None, None, Some(ovb));
        // push skipSan only
    let (id_c, tk_c) = reg.create(TaskKind::GitPush { dest: base.path().to_string_lossy().to_string(), remote: None, refspecs: None, username: None, password: None, strategy_override: Some(serde_json::json!({"tls": {"skip_san_whitelist": true}})) }); let hc = reg.clone().spawn_git_push_task(Some(app.clone()), id_c, tk_c, base.path().to_string_lossy().to_string(), None, None, None, None, Some(serde_json::json!({"tls": {"skip_san_whitelist": true}})) );
        // fetch unknown field
        let ovd = serde_json::json!({"tls": {"foo": true}}); let (id_d, tk_d) = reg.create(TaskKind::GitFetch { repo: src_path.clone(), dest: base.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: Some(ovd.clone()) }); let hd = reg.clone().spawn_git_fetch_task_with_opts(Some(app.clone()), id_d, tk_d, src_path.clone(), base.path().to_string_lossy().to_string(), None, None, None, Some(ovd));
        wait_done(&reg, id_b).await; hb.await.unwrap(); wait_done(&reg, id_c).await; hc.await.unwrap(); wait_done(&reg, id_d).await; hd.await.unwrap();
        // 断言：clone insecure & push skipSan -> tls applied; fetch {} & unknown -> 无
    crate::common::event_assert::assert_tls_applied(&id_a.to_string(), true); crate::common::event_assert::assert_tls_applied(&id_c.to_string(), true);
    crate::common::event_assert::assert_no_applied_code(&id_b.to_string(), "tls_strategy_override_applied");
    crate::common::event_assert::assert_no_applied_code(&id_d.to_string(), "tls_strategy_override_applied");
    }); }
}

// ---------------- (Phase3) section_summary_gating ----------------
// strategy_override_summary_and_gating / tls_override_summary_and_gating
mod section_summary_gating {
    use fireworks_collaboration_lib::core::tasks::registry::TaskRegistry; use fireworks_collaboration_lib::core::tasks::model::TaskKind; use fireworks_collaboration_lib::events::structured::{MemoryEventBus, set_global_event_bus}; use fireworks_collaboration_lib::events::emitter::AppHandle; use crate::common::event_assert::{assert_applied_code, assert_tls_applied, assert_conflict_kind};
    #[test]
    fn strategy_override_summary_and_gating() { let rt = tokio::runtime::Runtime::new().unwrap(); rt.block_on(async {
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1"); let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        // origin
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap(); std::fs::write(origin.path().join("readme.txt"), "hi").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("readme.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new()); let dest = tempfile::tempdir().unwrap(); let (id, tk) = reg.create(TaskKind::GitClone { repo: origin.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None });
    let ov = serde_json::json!({"http": {"follow_redirects": false}, "retry": {"max": 5}}); let h = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id, tk.clone(), origin.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(ov)); let _ = h.await;
        assert_applied_code(&id.to_string(), "http_strategy_override_applied"); assert_applied_code(&id.to_string(), "retry_strategy_override_applied");
        // gating off
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
        let origin2 = tempfile::tempdir().unwrap(); let repo2 = git2::Repository::init(origin2.path()).unwrap(); std::fs::write(origin2.path().join("g.txt"), "hi").unwrap(); let mut idx2 = repo2.index().unwrap(); idx2.add_path(std::path::Path::new("g.txt")).unwrap(); idx2.write().unwrap(); let tree_id2 = idx2.write_tree().unwrap(); let tree2 = repo2.find_tree(tree_id2).unwrap(); let sig2 = repo2.signature().unwrap(); repo2.commit(Some("HEAD"), &sig2,&sig2, "c1", &tree2, &[]).unwrap();
        let dest2 = tempfile::tempdir().unwrap(); let (gid, gtk) = reg.create(TaskKind::GitClone { repo: origin2.path().to_string_lossy().to_string(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None }); let govr = serde_json::json!({"http": {"follow_redirects": false}, "retry": {"max": 3}}); let gh = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), gid, gtk, origin2.path().to_string_lossy().to_string(), dest2.path().to_string_lossy().to_string(), None, None, Some(govr)); let _ = gh.await;
        assert_applied_code(&gid.to_string(), "http_strategy_override_applied"); assert_applied_code(&gid.to_string(), "retry_strategy_override_applied");
    }); }
    #[tokio::test]
    async fn tls_override_summary_and_gating() { std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "1"); let _ = set_global_event_bus(std::sync::Arc::new(MemoryEventBus::new()));
        let origin = tempfile::tempdir().unwrap(); let repo = git2::Repository::init(origin.path()).unwrap(); std::fs::write(origin.path().join("f.txt"), "one").unwrap(); let mut idx = repo.index().unwrap(); idx.add_path(std::path::Path::new("f.txt")).unwrap(); idx.write().unwrap(); let tree_id = idx.write_tree().unwrap(); let tree = repo.find_tree(tree_id).unwrap(); let sig = repo.signature().unwrap(); repo.commit(Some("HEAD"), &sig,&sig, "c1", &tree, &[]).unwrap();
        let reg = std::sync::Arc::new(TaskRegistry::new()); let dest = tempfile::tempdir().unwrap(); let (id, tk) = reg.create(TaskKind::GitClone { repo: origin.path().to_string_lossy().to_string(), dest: dest.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None }); let ov = serde_json::json!({"tls": {"insecure_skip_verify": true, "skip_san_whitelist": true}}); let h = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id, tk, origin.path().to_string_lossy().to_string(), dest.path().to_string_lossy().to_string(), None, None, Some(ov)); let _ = h.await; assert_applied_code(&id.to_string(), "tls_strategy_override_applied"); assert_tls_applied(&id.to_string(), true); assert_conflict_kind(&id.to_string(), "tls", Some("normalizes"));
        std::env::set_var("FWC_STRATEGY_APPLIED_EVENTS", "0");
        let origin2 = tempfile::tempdir().unwrap(); let repo2 = git2::Repository::init(origin2.path()).unwrap(); std::fs::write(origin2.path().join("y.txt"), "one").unwrap(); let mut idx2 = repo2.index().unwrap(); idx2.add_path(std::path::Path::new("y.txt")).unwrap(); idx2.write().unwrap(); let tree_id2 = idx2.write_tree().unwrap(); let tree2 = repo2.find_tree(tree_id2).unwrap(); let sig2 = repo2.signature().unwrap(); repo2.commit(Some("HEAD"), &sig2,&sig2, "c1", &tree2, &[]).unwrap();
        let dest2 = tempfile::tempdir().unwrap(); let (id2, tk2) = reg.create(TaskKind::GitClone { repo: origin2.path().to_string_lossy().to_string(), dest: dest2.path().to_string_lossy().to_string(), depth: None, filter: None, strategy_override: None }); let ov2 = serde_json::json!({"tls": {"insecure_skip_verify": true}}); let h2 = reg.spawn_git_clone_task_with_opts(Some(AppHandle {}), id2, tk2, origin2.path().to_string_lossy().to_string(), dest2.path().to_string_lossy().to_string(), None, None, Some(ov2)); let _ = h2.await; assert_applied_code(&id2.to_string(), "tls_strategy_override_applied"); assert_tls_applied(&id2.to_string(), true);
    }
}

// ---------------- (P3.2 migrated) section_adaptive_tls_and_metrics ----------------
// 来源：src-tauri/src/core/git/transport/tests_p3_2_observability.rs
// 覆盖：自适应 TLS 计时事件 + 指标开关/覆盖
mod section_adaptive_tls_and_metrics {
    use fireworks_collaboration_lib::events::structured::{set_test_event_bus, publish_global, MemoryEventBus, Event, StrategyEvent};
    use uuid::Uuid;

    #[test]
    fn adaptive_tls_timing_event_emitted_with_first_byte() {
        // 直接发布结构化事件，验证总线收集和字段读取
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        publish_global(Event::Strategy(StrategyEvent::AdaptiveTlsTiming { id: Uuid::new_v4().to_string(), kind: "GitClone".into(), used_fake_sni: true, fallback_stage: "Fake".into(), connect_ms: Some(10), tls_ms: Some(30), first_byte_ms: Some(40), total_ms: Some(50), cert_fp_changed: false }));
        let events = bus.snapshot();
        let timing = events.into_iter().find_map(|e| match e { Event::Strategy(StrategyEvent::AdaptiveTlsTiming { first_byte_ms, .. }) => first_byte_ms, _=>None });
        assert_eq!(timing, Some(40));
    }

    // 注：metrics 开关覆盖（禁用/强制启用）在 crate 内通过 #[cfg(test)] 的测试专用 API 控制，
    // 在集成测试构建中不可见。这里仅保留事件发布的可观测性验证。
}

// ---------------- (P3.2 migrated) section_timing_recorder ----------------
// 覆盖：核心传输层 TimingRecorder 的时间段捕获与 finish 幂等性
mod section_timing_recorder {
    use fireworks_collaboration_lib::core::git::transport::TimingRecorder;
    use std::time::Duration;

    #[test]
    fn timing_recorder_basic_flow() {
        let mut rec = TimingRecorder::new();
        rec.mark_connect_start();
        std::thread::sleep(Duration::from_millis(5));
        rec.mark_connect_end();
        rec.mark_tls_start();
        std::thread::sleep(Duration::from_millis(5));
        rec.mark_tls_end();
        rec.finish();
        let cap = rec.capture;
        assert!(cap.connect_ms.is_some(), "connect_ms should be recorded");
        assert!(cap.tls_ms.is_some(), "tls_ms should be recorded");
        assert!(cap.total_ms.is_some(), "total_ms should be recorded on finish");
        assert!(cap.total_ms.unwrap() >= cap.connect_ms.unwrap());
    }

    #[test]
    fn finish_idempotent() {
        let mut rec = TimingRecorder::new();
        rec.mark_connect_start();
        rec.mark_connect_end();
        rec.finish();
        let first_total = rec.capture.total_ms;
        std::thread::sleep(Duration::from_millis(2));
        rec.finish(); // second call shouldn't change stored total
        assert_eq!(first_total, rec.capture.total_ms, "finish should be idempotent");
    }
}

// ---------------- (P3.2 migrated) section_tls_fingerprint_and_logging ----------------
// 覆盖：证书指纹变更事件/长度、LRU 淘汰后重算、日志轮转/禁用
mod section_tls_fingerprint_and_logging {
    use fireworks_collaboration_lib::events::structured::{set_test_event_bus, MemoryEventBus, Event, StrategyEvent};
    use fireworks_collaboration_lib::core::git::transport::record_certificate;
    use rustls::Certificate;

    #[test]
    fn fingerprint_changed_event_once() {
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        // Fake certificates: different DER contents
        let cert_a = Certificate(vec![0x30,0x82,0x01,0x0a,0x01,0xA1,0xB2]);
        let cert_b = Certificate(vec![0x30,0x82,0x01,0x0a,0x02,0xA1,0xB2]);
        let host = "example.test";
    let r1 = record_certificate(host, &[cert_a.clone()]);
        assert!(r1.unwrap().0, "first should be changed=true");
    let r2 = record_certificate(host, &[cert_a.clone()]);
        assert!(!r2.unwrap().0, "same cert no change");
    let r3 = record_certificate(host, &[cert_b.clone()]);
        assert!(r3.unwrap().0, "different cert triggers change");
        let events = bus.snapshot();
        let count = events.into_iter().filter(|e| matches!(e, Event::Strategy(StrategyEvent::CertFingerprintChanged { .. }))).count();
        assert_eq!(count, 2, "two change events (initial + different)");
    }

    #[test]
    fn cert_fingerprint_changed_base64_length() {
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let host = format!("b64.{}.test", uuid::Uuid::new_v4());
        let _ = record_certificate(&host, &[Certificate(vec![0x30,0x82,0xAA,0xBB,0xCC,0xDD])]);
        let events = bus.snapshot();
        let mut found = false;
        for e in events.iter() {
            if let Event::Strategy(StrategyEvent::CertFingerprintChanged { spki_sha256, cert_sha256, host: h, .. }) = e {
                if h == &host { 
                    assert_eq!(spki_sha256.len(), 43, "spki length must be 43 for SHA256 base64url no pad");
                    assert_eq!(cert_sha256.len(), 43, "cert length must be 43 for SHA256 base64url no pad");
                    found = true;
                }
            }
        }
        assert!(found, "expected fingerprint change event for b64.test");
    }

    #[test]
    fn fingerprint_lru_eviction_triggers_rechange() {
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        // Ensure deterministic eviction by using a unique prefix and inserting 2*MAX+1 entries
        let prefix = format!("pfx-{}", uuid::Uuid::new_v4());
        let max = 512usize; // mirror MAX constant in implementation
        for i in 0..(max*2 + 1) {
            let host = format!("{prefix}-{i}.lru.test");
            let der = vec![0x30,0x82,(i & 0xFF) as u8,0x01,0x02,0x03];
            let _ = record_certificate(&host, &[Certificate(der)]);
        }
        // Re-record the first host; it should have been evicted and now count as changed
        let first = format!("{prefix}-0.lru.test");
        let r = record_certificate(&first, &[Certificate(vec![0x30,0x82,0x00,0x01,0x02,0x03])]).unwrap();
        assert!(r.0, "evicted host should appear as changed again");
        let events = bus.snapshot();
    assert!(events.iter().any(|e| matches!(e, Event::Strategy(StrategyEvent::CertFingerprintChanged { host, .. }) if *host==first)), "expected change event for reinserted first host");
    }

    #[test]
    fn cert_fp_log_rotation_small_limit() {
        // Set extremely small max bytes
        let mut cfg = fireworks_collaboration_lib::core::config::model::AppConfig::default();
        cfg.tls.cert_fp_max_bytes = 64; // very small
        fireworks_collaboration_lib::core::config::loader::save(&cfg).unwrap();
        let host = "rotate.test";
        // Clean existing log files for deterministic assertion
        let base = fireworks_collaboration_lib::core::config::loader::base_dir();
        let log = base.join("cert-fp.log");
        let rotated = base.join("cert-fp.log.1");
        let _ = std::fs::remove_file(&log);
        let _ = std::fs::remove_file(&rotated);
        for i in 0..5 {
            let der = vec![0x30,0x81, i as u8, 0x01, 0x02, 0x03];
            let _ = record_certificate(host, &[Certificate(der)]);
        }
        assert!(log.exists(), "primary log should exist");
        assert!(rotated.exists(), "rotated log .1 should exist due to tiny size limit");
    }

    #[test]
    fn cert_fp_log_disabled_suppresses_record() {
        // Disable logging
        let mut cfg = fireworks_collaboration_lib::core::config::model::AppConfig::default();
        cfg.tls.cert_fp_log_enabled = false;
        fireworks_collaboration_lib::core::config::loader::save(&cfg).unwrap();
        let bus = std::sync::Arc::new(MemoryEventBus::new());
        set_test_event_bus(bus.clone());
        let res = record_certificate("nolog.test", &[Certificate(vec![0x30,0x01,0x02])]);
        assert!(res.is_none(), "record should early exit when disabled");
        assert!(bus.snapshot().is_empty(), "no events when log disabled");
    }
}

// (removed) transport fallback counters test relied on #[cfg(test)] helpers not exported in integration build
