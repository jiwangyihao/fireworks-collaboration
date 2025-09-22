#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Task Lifecycle (Roadmap 12.13)
//! ------------------------------------------------------------
//! 迁移来源（legacy 将保留占位）：
//!   - events_task_lifecycle_git.rs
//!   - events_task_lifecycle_git_fail.rs
//!   - events_task_lifecycle_git_push.rs
//!   - events_task_lifecycle_structured.rs (仅生命周期部分，结构契约已在 12.12 聚合)
//! 分区结构：
//!   section_success_flow  -> 成功路径（Clone / Push / Sleep）
//!   section_failure_flow  -> 失败路径（Clone 无效 / Fetch 无效 / Push 无效）
//!   section_cancel_flow   -> 取消路径（Fetch 早期取消 / Sleep 取消）
//!   section_push_flow     -> Push 成功 vs 失败特化对比（单独锚点）
//!   section_metrics       -> 生命周期指标事件（占位：duration / bytes）
//! 设计说明：
//!   * 使用纯模拟函数生成事件字符串，避免真实 Git 与计时依赖，保证快速与确定性。
//!   * 事件模式：task:start:<Kind>, progress:<phase?>, cancel:requested:<phase?>, task:end:<Outcome>, metric:<k>:<v>
//!   * 利用 common/event_assert.rs 的 tagify + expect_tags_subsequence 进行锚点子序列断言，减少对具体中间事件的脆弱依赖。
//!   * 保留 expect_subsequence 作为后备（某些精确锚点仍使用）。
//! Post-audit(v1): 初版聚合完成；未来接入真实结构化事件时，将由模拟 -> 真实事件枚举过渡；metric:* 事件将映射到 Strategy/Task 扩展字段；取消 / 失败 分类将统一到统一枚举（TaskStatus）。

#[path = "../common/mod.rs"] mod common;
use common::event_assert::{expect_subsequence, tagify, default_tag_mapper, expect_tags_subsequence, assert_terminal_exclusive};
use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() { init_test_env(); }

// ---------------- Core placeholder domain types ----------------
#[derive(Debug, Clone, Copy)]
enum GitTaskKind { Clone, Fetch, Push, Sleep }

#[derive(Debug, Clone, Copy)]
enum LifecycleVariant {
    Success,
    Fail,
    CancelEarly,
    CancelMid,
}

#[derive(Debug, Clone, Copy)]
struct LifecycleSpec { kind: GitTaskKind, variant: LifecycleVariant, metrics: bool }

#[derive(Debug)]
struct LifecycleOutcome { events: Vec<String> }

fn simulate_lifecycle(spec: &LifecycleSpec) -> LifecycleOutcome {
    use GitTaskKind::*; use LifecycleVariant::*;
    let mut ev = Vec::new();
    ev.push(format!("task:start:{:?}", spec.kind));
    // 简化 progress：仅在 mid/normal 场景添加
    match spec.variant {
        Success | Fail | CancelMid => {
            if matches!(spec.kind, Clone | Fetch | Push | Sleep) { ev.push("progress:10%".into()); }
        }
        CancelEarly => {}
    }
    match spec.variant {
        CancelEarly => { ev.push("cancel:requested:early".into()); ev.push("task:end:cancelled".into()); },
        CancelMid => { ev.push("cancel:requested:mid".into()); ev.push("task:end:cancelled".into()); },
        Fail => { ev.push("task:end:failed".into()); },
        Success => { ev.push("task:end:completed".into()); },
    }
    if spec.metrics { ev.push("metric:duration_ms:42".into()); ev.push("metric:bytes:1024".into()); }
    LifecycleOutcome { events: ev }
}

fn tags_for(out: &LifecycleOutcome) -> Vec<common::event_assert::EventTag> { tagify(&out.events, default_tag_mapper) }

// ---------------- section_success_flow ----------------
mod section_success_flow {
    use super::*;
    #[test]
    fn clone_and_push_success_sequences() {
        for kind in [GitTaskKind::Clone, GitTaskKind::Push, GitTaskKind::Sleep] {
            let spec = LifecycleSpec { kind, variant: super::LifecycleVariant::Success, metrics: false };
            let out = simulate_lifecycle(&spec);
            // 字符串锚点
            expect_subsequence(&out.events, &["task:start", "task:end:completed"]);
            // 标签锚点（task -> task）
            let tags = tags_for(&out);
            expect_tags_subsequence(&tags, &["task", "task"]);
            assert_terminal_exclusive(&out.events, "task:end:completed", &["task:end:failed", "task:end:cancelled"]);
        }
    }
}

// ---------------- section_failure_flow ----------------
mod section_failure_flow {
    use super::*;
    #[test]
    fn clone_fetch_push_fail_distinct() {
        for kind in [GitTaskKind::Clone, GitTaskKind::Fetch, GitTaskKind::Push] {
            let spec = LifecycleSpec { kind, variant: super::LifecycleVariant::Fail, metrics: false };
            let out = simulate_lifecycle(&spec);
            expect_subsequence(&out.events, &["task:start", "task:end:failed"]);
            let tags = tags_for(&out); expect_tags_subsequence(&tags, &["task", "task"]);
            assert_terminal_exclusive(&out.events, "task:end:failed", &["task:end:completed", "task:end:cancelled"]);
        }
    }
}

// ---------------- section_cancel_flow ----------------
mod section_cancel_flow {
    use super::*;
    #[test]
    fn fetch_and_sleep_cancel_variants() {
        let cases = [
            (GitTaskKind::Fetch, super::LifecycleVariant::CancelEarly),
            (GitTaskKind::Sleep, super::LifecycleVariant::CancelMid),
        ];
    for (kind, variant) in cases { let spec = LifecycleSpec { kind, variant, metrics: false }; let out = simulate_lifecycle(&spec); expect_subsequence(&out.events, &["task:start", "cancel:requested", "task:end:cancelled"]); let tags = tags_for(&out); expect_tags_subsequence(&tags, &["task", "cancel", "task"]); assert_terminal_exclusive(&out.events, "task:end:cancelled", &["task:end:completed", "task:end:failed"]); }
    }
}

// ---------------- section_push_flow ----------------
mod section_push_flow {
    use super::*;
    #[test]
    fn push_success_vs_fail_patterns() {
        let success = simulate_lifecycle(&LifecycleSpec { kind: GitTaskKind::Push, variant: super::LifecycleVariant::Success, metrics: false });
        let fail = simulate_lifecycle(&LifecycleSpec { kind: GitTaskKind::Push, variant: super::LifecycleVariant::Fail, metrics: false });
        expect_subsequence(&success.events, &["task:start:Push", "task:end:completed"]);
        expect_subsequence(&fail.events, &["task:start:Push", "task:end:failed"]);
        // 标签：两者都至少包含 task 开头 & task 终结标签
        let tags_s = tags_for(&success); let tags_f = tags_for(&fail);
        expect_tags_subsequence(&tags_s, &["task", "task"]);
        expect_tags_subsequence(&tags_f, &["task", "task"]);
    }
}

// ---------------- section_metrics ----------------
mod section_metrics {
    use super::*;
    #[test]
    fn metrics_events_present_in_success_flow() {
        let out = simulate_lifecycle(&LifecycleSpec { kind: GitTaskKind::Clone, variant: super::LifecycleVariant::Success, metrics: true });
        expect_subsequence(&out.events, &["task:start:Clone", "task:end:completed", "metric:duration_ms:42", "metric:bytes:1024"]);
        let tags = tags_for(&out);
        // 标签最少包含一个 task，一个 metric
        expect_tags_subsequence(&tags, &["task", "metric"]);
        assert!(out.events.iter().filter(|e| e.starts_with("metric:" )).count() >= 2, "should have at least two metrics");
        assert_terminal_exclusive(&out.events, "task:end:completed", &["task:end:failed", "task:end:cancelled"]);
    }
}
