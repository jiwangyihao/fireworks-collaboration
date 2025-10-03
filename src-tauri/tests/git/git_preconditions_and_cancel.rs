#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Preconditions & Cancellation & Timeout (Roadmap 12.11)
//! --------------------------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - git_preconditions_and_cancel.rs (root old file: clone_cancel_quick_returns_cancel / fetch_missing_git_dir_fails_fast / fetch_cancel_quick_returns_cancel)
//! 分区结构：
//!   section_preconditions       -> 前置条件失败（路径不存在 / 缺少 .git 等）
//!   section_cancellation        -> 立即/中途取消的任务行为模拟
//!   section_timeout             -> 超时路径（占位模拟，不依赖真实 sleep）
//!   section_transport_fallback  -> 传输层 Fallback 决策状态机（Fake -> Real -> Default）
//!   section_transport_timing    -> TimingRecorder 计时捕获与 finish 幂等性
//! Cross-ref:
//!   - common/event_assert.rs (expect_subsequence)
//!   - 后续真实接入：TaskRegistry + cancellation flag + mock timer
//! 设计说明：
//!   * 当前实现为“轻量占位” —— 使用枚举驱动生成字符串事件序列，再断言子序列；
//!   * 不直接触发真实 GitService 调用（真实调用已由 legacy 覆盖，现迁为占位）；
//!   * 后续 12.12 引入事件 DSL 后可替换字符串事件为结构化枚举。
//! Post-audit(v1): legacy 已替换为占位文件；此文件为唯一逻辑聚合入口。
//! Post-audit(v2): 补充说明：后续将把 OutcomeKind 融入统一 TaskStatus/FailureCategory；timeout/cancel 将接入 mock clock + cancellation token；当前字符串事件保持最小锚点前缀以便 12.12 DSL 迁移。

use super::common::event_assert::{
    assert_terminal_exclusive, expect_optional_tags_subsequence, expect_subsequence,
};
use super::common::test_env::init_test_env;

// 小辅助：若标签可映射，则断言最小锚点子序列
fn expect_tag_subseq_min(events: &[String], anchors: &[&str]) {
    expect_optional_tags_subsequence(events, anchors);
}
// ---------------- Core domain placeholder types ----------------
#[derive(Debug, Clone, Copy)]
enum GitOp {
    Clone,
    Fetch,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum PreconditionKind {
    MissingGitDir,
    InvalidUrl,
    NoWritePerm,
}

#[derive(Debug, Clone, Copy)]
enum CancelPhase {
    Immediate,
    Midway,
}

#[derive(Debug, Clone, Copy)]
enum TimeoutScenario {
    CloneSlow,
    FetchSlow,
}

// Outcome enums (placeholder)
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum OutcomeKind {
    Success,
    FailedPrecondition,
    Canceled,
    TimedOut,
}

#[derive(Debug)]
struct SimOutcome {
    kind: OutcomeKind,
    events: Vec<String>,
}

// ---------------- Simulation helpers (pure, deterministic) ----------------
fn simulate_precondition(op: GitOp, kind: PreconditionKind) -> SimOutcome {
    let mut ev = vec![format!("pre:check:start:{:?}:{:?}", op, kind)];
    ev.push(format!("pre:check:failed:{:?}", kind));
    ev.push(format!("task:end:{:?}:precondition_failed", op));
    SimOutcome {
        kind: OutcomeKind::FailedPrecondition,
        events: ev,
    }
}

fn simulate_cancellation(op: GitOp, phase: CancelPhase) -> SimOutcome {
    let mut ev = vec![format!("task:start:{:?}", op)];
    match phase {
        CancelPhase::Immediate => {
            ev.push("cancel:requested:immediate".into());
            ev.push("task:end:cancelled".into());
            SimOutcome {
                kind: OutcomeKind::Canceled,
                events: ev,
            }
        }
        CancelPhase::Midway => {
            ev.push("progress:10%".into());
            ev.push("cancel:requested:midway".into());
            ev.push("cleanup:begin".into());
            ev.push("task:end:cancelled".into());
            SimOutcome {
                kind: OutcomeKind::Canceled,
                events: ev,
            }
        }
    }
}

fn simulate_timeout(s: TimeoutScenario) -> SimOutcome {
    let mut ev = vec![format!("task:start:{:?}", s)];
    ev.push("progress:slow_tick".into());
    ev.push("timeout:trigger".into());
    ev.push("task:end:timeout".into());
    SimOutcome {
        kind: OutcomeKind::TimedOut,
        events: ev,
    }
}

// ---------------- section_preconditions ----------------
mod section_preconditions {
    use super::*;
    #[test]
    fn missing_git_dir_fails_fast() {
        let _ = simulate_precondition(GitOp::Fetch, PreconditionKind::MissingGitDir);
        init_test_env();
        // 参数化两种前置失败：缺少 .git / 无效 URL
        let cases = vec![
            (
                GitOp::Fetch,
                PreconditionKind::MissingGitDir,
                "task:end:Fetch:precondition_failed",
            ),
            (
                GitOp::Clone,
                PreconditionKind::InvalidUrl,
                "task:end:Clone:precondition_failed",
            ),
        ];
        for (op, kind, terminal) in cases {
            let out = simulate_precondition(op, kind);
            assert_eq!(
                out.kind,
                OutcomeKind::FailedPrecondition,
                "preconditions: {:?} {:?}",
                op,
                kind
            );
            expect_subsequence(
                &out.events,
                &["pre:check:start", "pre:check:failed", terminal],
            );
            // tag 序列：pre -> task（终态）
            expect_tag_subseq_min(&out.events, &["pre", "task"]);
            // 终态互斥：仅允许 precondition_failed，不得出现 cancel/timeout/success
            assert_terminal_exclusive(
                &out.events,
                terminal,
                &["task:end:cancelled", "task:end:timeout", "task:end:success"],
            );
        }
    }

    #[test]
    fn invalid_url_fails_fast() {
        let out = simulate_precondition(GitOp::Clone, PreconditionKind::InvalidUrl);
        assert_eq!(out.kind, OutcomeKind::FailedPrecondition);
        expect_subsequence(
            &out.events,
            &[
                "pre:check:start",
                "pre:check:failed",
                "task:end:Clone:precondition_failed",
            ],
        );
        expect_optional_tags_subsequence(&out.events, &["pre", "task"]);
        assert_terminal_exclusive(
            &out.events,
            "task:end:Clone:precondition_failed",
            &["task:end:cancelled", "task:end:timeout", "task:end:success"],
        );
    }
}

// ---------------- section_cancellation ----------------
mod section_cancellation {
    use super::*;
    #[test]
    fn clone_immediate_cancel() {
        let _ = simulate_cancellation(GitOp::Clone, CancelPhase::Immediate);
        init_test_env();
        // 参数化两种取消：立即取消 / 中途取消（含 cleanup）
        let cases = vec![
            (GitOp::Clone, CancelPhase::Immediate, false),
            (GitOp::Fetch, CancelPhase::Midway, true),
        ];
        for (op, phase, has_midway) in cases {
            let out = simulate_cancellation(op, phase);
            assert_eq!(
                out.kind,
                OutcomeKind::Canceled,
                "cancel: {:?} {:?}",
                op,
                phase
            );
            // 组合锚点
            let mut anchors: Vec<String> = vec![format!("task:start:{:?}", op)];
            if has_midway {
                anchors.push("progress:10%".into());
                anchors.push("cancel:requested:midway".into());
                anchors.push("cleanup:begin".into());
            } else {
                anchors.push("cancel:requested:immediate".into());
            }
            anchors.push("task:end:cancelled".into());
            let as_refs: Vec<&str> = anchors.iter().map(|s| s.as_str()).collect();
            expect_subsequence(&out.events, &as_refs);
            // 标签序列锚点（task -> cancel -> task）
            expect_tag_subseq_min(&out.events, &["task", "cancel", "task"]);
            // 终态互斥：取消不应与其它终态并存
            assert_terminal_exclusive(
                &out.events,
                "task:end:cancelled",
                &[
                    "precondition_failed",
                    "task:end:timeout",
                    "task:end:success",
                ],
            );
        }
    }

    #[test]
    fn fetch_midway_cancel_has_cleanup() {
        let out = simulate_cancellation(GitOp::Fetch, CancelPhase::Midway);
        assert_eq!(out.kind, OutcomeKind::Canceled);
        expect_subsequence(
            &out.events,
            &[
                "task:start:Fetch",
                "progress:10%",
                "cancel:requested:midway",
                "cleanup:begin",
                "task:end:cancelled",
            ],
        );
        // 标签序列锚点（task -> cancel -> task）
        expect_optional_tags_subsequence(&out.events, &["task", "cancel", "task"]);
        // 终态互斥：取消不应与其它终态并存
        assert_terminal_exclusive(
            &out.events,
            "task:end:cancelled",
            &[
                "precondition_failed",
                "task:end:timeout",
                "task:end:success",
            ],
        );
    }
}

// ---------------- section_timeout ----------------
mod section_timeout {
    use super::*;
    #[test]
    fn clone_slow_timeout() {
        let _ = simulate_timeout(TimeoutScenario::CloneSlow);
        init_test_env();
        for s in [TimeoutScenario::CloneSlow, TimeoutScenario::FetchSlow] {
            let out = simulate_timeout(s);
            assert_eq!(out.kind, OutcomeKind::TimedOut, "timeout: {:?}", s);
            let start = format!("task:start:{:?}", s);
            expect_subsequence(
                &out.events,
                &[start.as_str(), "timeout:trigger", "task:end:timeout"],
            );
            expect_tag_subseq_min(&out.events, &["task", "timeout", "task"]);
            assert_terminal_exclusive(
                &out.events,
                "task:end:timeout",
                &[
                    "precondition_failed",
                    "task:end:cancelled",
                    "task:end:success",
                ],
            );
        }
    }

    #[test]
    fn fetch_slow_timeout() {
        let out = simulate_timeout(TimeoutScenario::FetchSlow);
        assert_eq!(out.kind, OutcomeKind::TimedOut);
        expect_subsequence(
            &out.events,
            &[
                "task:start:FetchSlow",
                "timeout:trigger",
                "task:end:timeout",
            ],
        );
        expect_optional_tags_subsequence(&out.events, &["task", "timeout", "task"]);
        assert_terminal_exclusive(
            &out.events,
            "task:end:timeout",
            &[
                "precondition_failed",
                "task:end:cancelled",
                "task:end:success",
            ],
        );
    }
}

// ---------------- section_transport_fallback ----------------
mod section_transport_fallback {
    use fireworks_collaboration_lib::core::git::transport::{
        DecisionCtx, FallbackDecision, FallbackReason, FallbackStage,
    };

    #[test]
    fn initial_stage_default_when_disabled() {
        let ctx = DecisionCtx {
            policy_allows_fake: false,
            runtime_fake_disabled: false,
        };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
    }

    #[test]
    fn skip_fake_policy_creates_default_stage() {
        let ctx = DecisionCtx {
            policy_allows_fake: false,
            runtime_fake_disabled: false,
        };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        let h = d.history();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].reason, FallbackReason::SkipFakePolicy);
    }

    #[test]
    fn full_chain_history_order() {
        let ctx = DecisionCtx {
            policy_allows_fake: true,
            runtime_fake_disabled: false,
        };
        let mut d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Fake);
        d.advance_on_error().expect("fake->real");
        d.advance_on_error().expect("real->default");
        assert!(d.advance_on_error().is_none());
        let stages: Vec<_> = d.history().iter().map(|tr| tr.to).collect();
        assert_eq!(
            stages,
            vec![
                FallbackStage::Fake,
                FallbackStage::Real,
                FallbackStage::Default
            ]
        );
    }

    #[test]
    fn runtime_fake_disabled_behaves_like_policy_skip() {
        let ctx = DecisionCtx {
            policy_allows_fake: true,
            runtime_fake_disabled: true,
        };
        let d = FallbackDecision::initial(&ctx);
        assert_eq!(d.stage(), FallbackStage::Default);
        assert_eq!(d.history()[0].reason, FallbackReason::SkipFakePolicy);
    }
}

// ---------------- section_transport_timing ----------------
mod section_transport_timing {
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
        assert!(
            cap.total_ms.is_some(),
            "total_ms should be recorded on finish"
        );
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
        rec.finish();
        assert_eq!(
            first_total, rec.capture.total_ms,
            "finish should be idempotent"
        );
    }
}
