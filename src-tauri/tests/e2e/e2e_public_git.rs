#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Public Git E2E Pipeline (Roadmap 12.15 - Refined)
//! -----------------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - git_e2e_public.rs
//! 分区结构：
//!   scenario_clone_build_push  -> 完整流水线 (clone -> modify -> commit -> push -> fetch)
//!   scenario_read_only         -> 只读路径 (clone -> fetch)
//!   scenario_error_boundary    -> 错误边界占位（后续注入网络/权限/超时）
//! 设计说明：
//!   * 当前阶段不访问公网：使用 pipeline DSL 占位事件。
//!   * legacy 原公网用例保留为跳过型 placeholder，待确认策略后完全迁移/删除。
//!   * 后续将引入：共享远端 bare 仓库 fixture + 缓存目录 + 可选真实网络 (feature flag)。

#[path = "../common/mod.rs"] mod common;
use common::{pipeline::{PipelineSpec, run_pipeline_with, PipelineConfig, FaultKind}, event_assert::{expect_subsequence, tagify, default_tag_mapper, expect_tags_subsequence}};
use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() { init_test_env(); }

// ---------------- scenario_clone_build_push ----------------
mod scenario_clone_build_push {
    use super::*;
    #[test]
    fn pipeline_full_flow_has_core_events_and_commit_growth() {
        let spec = PipelineSpec::basic_clone_build_push();
        let cfg = PipelineConfig { remote_commits: 2, enable_real: true, faults: vec![] };
        let out = run_pipeline_with(&spec, &cfg);
        // 关键事件锚点：clone 完成 -> push 成功 -> fetch 完成
        expect_subsequence(&out.events, &["pipeline:clone:complete", "pipeline:push:success", "pipeline:fetch:complete"]);
        // 提交计数：after 应 >= before + 1 （一次新提交）
        let before = out.commit_count_before.expect("before count");
        let after = out.commit_count_after.expect("after count");
        assert!(after >= before + 1, "expect at least one new commit: before={}, after={}", before, after);
        // tag DSL（占位：当前 events 仍为字符串，保持与其它阶段一致的接口调用）
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["pipeline", "pipeline"]); }
    }
}

// ---------------- scenario_read_only ----------------
mod scenario_read_only {
    use super::*;
    #[test]
    fn pipeline_read_only_clone_fetch_commit_count_unchanged() {
        let spec = PipelineSpec::read_only();
        let cfg = PipelineConfig { remote_commits: 3, enable_real: true, faults: vec![] };
        let out = run_pipeline_with(&spec, &cfg);
        expect_subsequence(&out.events, &["pipeline:clone:complete", "pipeline:fetch:complete"]);
        let before = out.commit_count_before.expect("before count");
        let after = out.commit_count_after.expect("after count");
        assert_eq!(after, before, "read-only pipeline should not create new commits: before={}, after={}", before, after);
    }
}

// ---------------- scenario_error_boundary ----------------
mod scenario_error_boundary {
    use super::*;
    #[test]
    fn push_fault_injection_force_failure() {
        // 故障注入：ForcePushFailure 通过篡改 remote URL 导致 push 失败
        let mut spec = PipelineSpec::basic_clone_build_push();
        // 仍使用完整 spec：在 Push 处触发失败，后续 Fetch 可能也失败/缺失
        let cfg = PipelineConfig { remote_commits: 1, enable_real: true, faults: vec![FaultKind::ForcePushFailure] };
        let out = run_pipeline_with(&spec, &cfg);
        assert!(out.failed, "expected pipeline failed flag set");
        // 事件包含 push:failed
        expect_subsequence(&out.events, &["pipeline:clone:complete", "pipeline:push:failed"]);
        // 失败场景：after 计数若存在应与 before 相同（提交未成功传播）
        if let (Some(b), Some(a)) = (out.commit_count_before, out.commit_count_after) {
            assert_eq!(a, b, "commit count should not advance on failed push: before={}, after={}", b, a);
        }
    }
}
