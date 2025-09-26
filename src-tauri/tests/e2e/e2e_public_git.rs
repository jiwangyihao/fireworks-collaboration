#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Public Git E2E Pipeline
//! 精简后版本：
//!   * 合并原 success / read-only / failure 三个用例为参数化循环，减少重复。
//!   * 使用 `common::pipeline` 中新增的提交计数断言辅助。
//!   * 移除未生效的 Tag DSL 占位调用（当前事件仍为简单字符串）。
//!   * 仅保留对事件最小子序列的断言，遵循“最小锚点”原则。
//! 目标：验证不同场景（成功新增提交/只读/故障不前进）的核心行为与事件序列。

#[path = "../common/mod.rs"]
mod common;
use common::event_assert::expect_subsequence;
use common::pipeline::{
    assert_commit_growth_at_least, assert_commit_unchanged, assert_failure_commit_not_advanced,
    run_pipeline_with, FaultKind, PipelineConfig, PipelineSpec,
};
use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

#[derive(Debug, Clone, Copy)]
enum Scenario {
    FullSuccess,
    ReadOnly,
    PushFailure,
}

impl Scenario {
    fn spec(self) -> PipelineSpec {
        match self {
            Scenario::FullSuccess | Scenario::PushFailure => PipelineSpec::basic_clone_build_push(),
            Scenario::ReadOnly => PipelineSpec::read_only(),
        }
    }
    fn config(self) -> PipelineConfig {
        match self {
            Scenario::FullSuccess => PipelineConfig {
                remote_commits: 2,
                enable_real: true,
                faults: vec![],
            },
            Scenario::ReadOnly => PipelineConfig {
                remote_commits: 3,
                enable_real: true,
                faults: vec![],
            },
            Scenario::PushFailure => PipelineConfig {
                remote_commits: 1,
                enable_real: true,
                faults: vec![FaultKind::ForcePushFailure],
            },
        }
    }
}

#[test]
fn pipeline_scenarios_cover_success_readonly_and_failure() {
    for scenario in [
        Scenario::FullSuccess,
        Scenario::ReadOnly,
        Scenario::PushFailure,
    ] {
        let spec = scenario.spec();
        let cfg = scenario.config();
        let out = run_pipeline_with(&spec, &cfg);
        match scenario {
            Scenario::FullSuccess => {
                expect_subsequence(
                    &out.events,
                    &[
                        "pipeline:clone:complete",
                        "pipeline:push:success",
                        "pipeline:fetch:complete",
                    ],
                );
                assert!(out.is_success(), "full success scenario should succeed");
                assert_commit_growth_at_least(&out, 1);
            }
            Scenario::ReadOnly => {
                expect_subsequence(
                    &out.events,
                    &["pipeline:clone:complete", "pipeline:fetch:complete"],
                );
                assert!(out.is_success(), "read-only scenario should succeed");
                assert_commit_unchanged(&out);
            }
            Scenario::PushFailure => {
                expect_subsequence(
                    &out.events,
                    &["pipeline:clone:complete", "pipeline:push:failed"],
                );
                assert!(out.is_failed(), "push failure scenario should be failed");
                assert_failure_commit_not_advanced(&out);
            }
        }
    }
}
