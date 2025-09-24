#![cfg(not(feature = "tauri-app"))]
//! 聚合测试：Git Task Lifecycle (简化 & 去冗余版)
//! ------------------------------------------------------------
//! 历史来源（多文件已聚合 -> 单文件矩阵）：
//!   * success / failure / cancel / push diff / metrics 现合并为统一矩阵测试 + 单独 metrics。
//! 目标：
//!   1. 去除重复 push 成功/失败 与通用成功/失败测试的重叠。
//!   2. 使用单一参数矩阵覆盖：成功(多 Kind)、失败(限定 Kind)、取消(早期/中途) 两类差异。
//!   3. 提取高频断言逻辑到 helper：`run_spec` + `assert_lifecycle_core`。
//!   4. 仅保留 Tag DSL 断言（移除冗余的字符串 expect_subsequence 重复检查）。
//! 事件模式：
//!   task:start:<Kind>
//!   [progress:10%]* (仅非早期取消变体)
//!   [cancel:requested:<phase>]* (取消变体)
//!   task:end:<Outcome>  (completed | failed | cancelled)
//!   [metric:duration_ms:.., metric:bytes:..]* (开启 metrics 时附加，位于终态之后)
//! Tag DSL 期望序列：
//!   - 成功 / 失败 => [task, task]
//!   - 取消       => [task, cancel, task]
//! 迁移与清理：
//!   - 删除分散 section_* modules，便于快速阅读与后续扩展。
//!   - 删除 `tags_for` 兼容包装，直接使用 `tagify` + `default_tag_mapper`。
//!   - 保留最小进度事件（未断言），为将来结构化扩展预留位置。
//! 后续潜在升级：接入真实结构化事件枚举后，把字符串模拟替换为强类型构造，再次收敛到公共构造模块。

#[path = "../common/mod.rs"] mod common;
use common::event_assert::{expect_optional_tags_subsequence, assert_terminal_exclusive};
use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() { init_test_env(); }

// ---------------- 模拟领域类型 & 规格 ----------------
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

/// 模拟生命周期（纯字符串事件），确保顺序及可预测性。
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
// ---------------- 公共断言辅助 ----------------
fn expected_tag_sequence(variant: LifecycleVariant) -> &'static [&'static str] {
    use LifecycleVariant::*;
    match variant { Success | Fail => &["task", "task"], CancelEarly | CancelMid => &["task", "cancel", "task"] }
}

fn terminal_anchor(variant: LifecycleVariant) -> (&'static str, &'static [&'static str]) {
    use LifecycleVariant::*;
    match variant {
        Success => ("task:end:completed", &["task:end:failed", "task:end:cancelled"]),
        Fail => ("task:end:failed", &["task:end:completed", "task:end:cancelled"]),
        CancelEarly | CancelMid => ("task:end:cancelled", &["task:end:completed", "task:end:failed"]),
    }
}

fn assert_lifecycle_core(spec: &LifecycleSpec) -> LifecycleOutcome {
    let out = simulate_lifecycle(spec);
    // 基础形状断言：首事件 & 终态互斥
    assert!(out.events.first().unwrap().starts_with("task:start:"), "first event must be task:start:*: {:?}", out.events.first().unwrap());
    let (terminal, forbidden) = terminal_anchor(spec.variant);
    assert_terminal_exclusive(&out.events, terminal, forbidden);
    // Tag DSL 序列（最小锚点）
    expect_optional_tags_subsequence(&out.events, expected_tag_sequence(spec.variant));
    out
}


// ---------------- 核心矩阵测试 ----------------
#[test]
fn lifecycle_core_matrix() {
    use GitTaskKind as K; use LifecycleVariant as V;
    let matrix: &[LifecycleSpec] = &[
        // Success
        LifecycleSpec { kind: K::Clone, variant: V::Success, metrics: false },
        LifecycleSpec { kind: K::Push, variant: V::Success, metrics: false },
        LifecycleSpec { kind: K::Sleep, variant: V::Success, metrics: false },
        // Fail (与历史覆盖一致：Clone/Fetch/Push)
        LifecycleSpec { kind: K::Clone, variant: V::Fail, metrics: false },
        LifecycleSpec { kind: K::Fetch, variant: V::Fail, metrics: false },
        LifecycleSpec { kind: K::Push, variant: V::Fail, metrics: false },
        // Cancel variants
        LifecycleSpec { kind: K::Fetch, variant: V::CancelEarly, metrics: false },
        LifecycleSpec { kind: K::Sleep, variant: V::CancelMid, metrics: false },
    ];
    for spec in matrix { let _ = assert_lifecycle_core(spec); }
}

// ---------------- Metrics 专项 ----------------
#[test]
fn lifecycle_metrics_success() {
    use GitTaskKind as K; use LifecycleVariant as V;
    let spec = LifecycleSpec { kind: K::Clone, variant: V::Success, metrics: true };
    let out = assert_lifecycle_core(&spec);
    let metric_lines: Vec<&String> = out.events.iter().filter(|e| e.starts_with("metric:" )).collect();
    assert!(metric_lines.len() >= 2, "expected >=2 metric lines, got {:?}", metric_lines);
    // 确认 metrics 出现在终态之后
    let end_pos = out.events.iter().position(|e| e.contains("task:end:completed")).unwrap();
    for m in metric_lines { let pos = out.events.iter().position(|e| e == m).unwrap(); assert!(pos > end_pos, "metric event should appear after terminal event"); }
}
