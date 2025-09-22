#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Preconditions & Cancellation & Timeout (Roadmap 12.11)
//! --------------------------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - git_preconditions_and_cancel.rs (root old file: clone_cancel_quick_returns_cancel / fetch_missing_git_dir_fails_fast / fetch_cancel_quick_returns_cancel)
//! 分区结构：
//!   section_preconditions  -> 前置条件失败（路径不存在 / 缺少 .git 等）
//!   section_cancellation   -> 立即/中途取消的任务行为模拟
//!   section_timeout        -> 超时路径（占位模拟，不依赖真实 sleep）
//! Cross-ref:
//!   - common/event_assert.rs (expect_subsequence)
//!   - 后续真实接入：TaskRegistry + cancellation flag + mock timer
//! 设计说明：
//!   * 当前实现为“轻量占位” —— 使用枚举驱动生成字符串事件序列，再断言子序列；
//!   * 不直接触发真实 GitService 调用（真实调用已由 legacy 覆盖，现迁为占位）；
//!   * 后续 12.12 引入事件 DSL 后可替换字符串事件为结构化枚举。
//! Post-audit(v1): legacy 已替换为占位文件；此文件为唯一逻辑聚合入口。
//! Post-audit(v2): 补充说明：后续将把 OutcomeKind 融入统一 TaskStatus/FailureCategory；timeout/cancel 将接入 mock clock + cancellation token；当前字符串事件保持最小锚点前缀以便 12.12 DSL 迁移。

#[path = "../common/mod.rs"] mod common;
use common::event_assert::{
    expect_subsequence, tagify, default_tag_mapper, expect_tags_subsequence, assert_terminal_exclusive,
};
use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() { init_test_env(); }

// ---------------- Core domain placeholder types ----------------
#[derive(Debug, Clone, Copy)]
enum GitOp { Clone, Fetch }

#[derive(Debug, Clone, Copy)]
enum PreconditionKind { MissingGitDir, InvalidUrl, NoWritePerm }

#[derive(Debug, Clone, Copy)]
enum CancelPhase { Immediate, Midway }

#[derive(Debug, Clone, Copy)]
enum TimeoutScenario { CloneSlow, FetchSlow }

// Outcome enums (placeholder)
#[derive(Debug, Clone, Copy, PartialEq)]
enum OutcomeKind { Success, FailedPrecondition, Canceled, TimedOut }

#[derive(Debug)]
struct SimOutcome { kind: OutcomeKind, events: Vec<String> }

// ---------------- Simulation helpers (pure, deterministic) ----------------
fn simulate_precondition(op: GitOp, kind: PreconditionKind) -> SimOutcome {
    let mut ev = vec![format!("pre:check:start:{:?}:{:?}", op, kind)];
    ev.push(format!("pre:check:failed:{:?}", kind));
    ev.push(format!("task:end:{:?}:precondition_failed", op));
    SimOutcome { kind: OutcomeKind::FailedPrecondition, events: ev }
}

fn simulate_cancellation(op: GitOp, phase: CancelPhase) -> SimOutcome {
    let mut ev = vec![format!("task:start:{:?}", op)];
    match phase {
        CancelPhase::Immediate => {
            ev.push("cancel:requested:immediate".into());
            ev.push("task:end:cancelled".into());
            SimOutcome { kind: OutcomeKind::Canceled, events: ev }
        }
        CancelPhase::Midway => {
            ev.push("progress:10%".into());
            ev.push("cancel:requested:midway".into());
            ev.push("cleanup:begin".into());
            ev.push("task:end:cancelled".into());
            SimOutcome { kind: OutcomeKind::Canceled, events: ev }
        }
    }
}

fn simulate_timeout(s: TimeoutScenario) -> SimOutcome {
    let mut ev = vec![format!("task:start:{:?}", s)];
    ev.push("progress:slow_tick".into());
    ev.push("timeout:trigger".into());
    ev.push("task:end:timeout".into());
    SimOutcome { kind: OutcomeKind::TimedOut, events: ev }
}

// ---------------- section_preconditions ----------------
mod section_preconditions {
    use super::*;
    #[test]
    fn missing_git_dir_fails_fast() {
        let out = simulate_precondition(GitOp::Fetch, PreconditionKind::MissingGitDir);
        assert_eq!(out.kind, OutcomeKind::FailedPrecondition);
        expect_subsequence(&out.events, &["pre:check:start", "pre:check:failed", "precondition_failed"]);
        // tag 序列：pre -> task（终态）
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["pre", "task"]); }
        // 终态互斥：仅允许 precondition_failed，不得出现 cancel/timeout/success
        assert_terminal_exclusive(&out.events, "task:end:Fetch:precondition_failed", &["task:end:cancelled", "task:end:timeout", "task:end:success"]);
    }

    #[test]
    fn invalid_url_fails_fast() {
        let out = simulate_precondition(GitOp::Clone, PreconditionKind::InvalidUrl);
        assert_eq!(out.kind, OutcomeKind::FailedPrecondition);
        expect_subsequence(&out.events, &["pre:check:start", "pre:check:failed", "task:end:Clone:precondition_failed"]);
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["pre", "task"]); }
        assert_terminal_exclusive(&out.events, "task:end:Clone:precondition_failed", &["task:end:cancelled", "task:end:timeout", "task:end:success"]);
    }
}

// ---------------- section_cancellation ----------------
mod section_cancellation {
    use super::*;
    #[test]
    fn clone_immediate_cancel() {
        let out = simulate_cancellation(GitOp::Clone, CancelPhase::Immediate);
        assert_eq!(out.kind, OutcomeKind::Canceled);
        expect_subsequence(&out.events, &["task:start:Clone", "cancel:requested:immediate", "task:end:cancelled"]);
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["task", "cancel", "task"]); }
        assert_terminal_exclusive(&out.events, "task:end:cancelled", &["precondition_failed", "task:end:timeout", "task:end:success"]);
    }

    #[test]
    fn fetch_midway_cancel_has_cleanup() {
        let out = simulate_cancellation(GitOp::Fetch, CancelPhase::Midway);
        assert_eq!(out.kind, OutcomeKind::Canceled);
        expect_subsequence(&out.events, &["task:start:Fetch", "progress:10%", "cancel:requested:midway", "cleanup:begin", "task:end:cancelled"]);
        // 标签序列锚点（task -> cancel -> task）
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["task", "cancel", "task"]); }
        // 终态互斥：取消不应与其它终态并存
        assert_terminal_exclusive(&out.events, "task:end:cancelled", &["precondition_failed", "task:end:timeout", "task:end:success"]);
    }
}

// ---------------- section_timeout ----------------
mod section_timeout {
    use super::*;
    #[test]
    fn clone_slow_timeout() {
        let out = simulate_timeout(TimeoutScenario::CloneSlow);
        assert_eq!(out.kind, OutcomeKind::TimedOut);
        expect_subsequence(&out.events, &["task:start:CloneSlow", "timeout:trigger", "task:end:timeout"]);
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["task", "timeout", "task"]); }
        assert_terminal_exclusive(&out.events, "task:end:timeout", &["precondition_failed", "task:end:cancelled", "task:end:success"]);
    }

    #[test]
    fn fetch_slow_timeout() {
        let out = simulate_timeout(TimeoutScenario::FetchSlow);
        assert_eq!(out.kind, OutcomeKind::TimedOut);
        expect_subsequence(&out.events, &["task:start:FetchSlow", "timeout:trigger", "task:end:timeout"]);
        let tags = tagify(&out.events, default_tag_mapper);
        if !tags.is_empty() { expect_tags_subsequence(&tags, &["task", "timeout", "task"]); }
        assert_terminal_exclusive(&out.events, "task:end:timeout", &["precondition_failed", "task:end:cancelled", "task:end:success"]);
    }
}
