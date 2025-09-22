//! Partial Filter 测试矩阵（12.6 聚合 -> 预备 12.8 扩展）
//! ------------------------------------------------------
//! 目标：集中列举代表性 partial filter 场景，为 clone / fetch 共享做准备。
//! 版本轨迹：
//!   - v1 (12.6): 仅 clone 使用，区分事件 / 代码 / 结构 及其与 depth 的交叉。
//!   - v2 (Post-audit v3): 引入 `PartialFilterOp` 占位（当前只实现 Clone），
//!     与独立的 `partial_filter_support` (SupportLevel) 模块解耦，方便 12.8 fetch 聚合。
//!
//! Post-audit(v2): 使用 allow(dead_code) 抑制未使用枚举成员警告，等待 fetch 聚合。
//! Post-audit(v3): 添加 `PartialFilterOp`、文档化后续扩展计划（invalid / no_filter / fetch 特有）。
//!
//! 后续计划（12.8+）：
//!   * 扩展 op=Fetch 维度；
//!   * 增加 Invalid / NoFilter / SparseEdge 等用例；
//!   * 与事件断言 DSL 融合（替换字符串 contains）。

#![allow(dead_code)]

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy)]
pub enum PartialFilterOp { Clone, Fetch }

#[derive(Debug, Clone, Copy)]
pub enum PartialFilterKind {
    EventOnly,
    CodeOnly,
    Structure,
    CodeWithDepth,
    EventWithDepth,
    NoFilter,       // fetch only: 未提供 --filter （用于对比 baseline）
    InvalidFilter,  // fetch only: 解析失败/非法表达式
}

#[derive(Debug, Clone, Copy)]
pub struct PartialFilterCase {
    pub op: PartialFilterOp,
    pub kind: PartialFilterKind,
    pub depth: Option<u32>,
}

impl Display for PartialFilterCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let op = match self.op { PartialFilterOp::Clone => "Clone", PartialFilterOp::Fetch => "Fetch" };
        match self.kind {
            PartialFilterKind::EventOnly => write!(f, "{op}:EventOnly"),
            PartialFilterKind::CodeOnly => write!(f, "{op}:CodeOnly"),
            PartialFilterKind::Structure => write!(f, "{op}:Structure"),
            PartialFilterKind::CodeWithDepth => write!(f, "{op}:CodeWithDepth(depth={})", self.depth.unwrap_or(0)),
            PartialFilterKind::EventWithDepth => write!(f, "{op}:EventWithDepth(depth={})", self.depth.unwrap_or(0)),
            PartialFilterKind::NoFilter => write!(f, "{op}:NoFilter"),
            PartialFilterKind::InvalidFilter => write!(f, "{op}:InvalidFilter"),
        }
    }
}

pub fn partial_filter_cases() -> Vec<PartialFilterCase> { // 兼容旧 clone-only 调用
    partial_filter_cases_for(PartialFilterOp::Clone)
}

pub fn partial_filter_cases_for(op: PartialFilterOp) -> Vec<PartialFilterCase> {
    use PartialFilterKind::*; use PartialFilterOp::*;
    let mut v = vec![
        PartialFilterCase { op, kind: EventOnly, depth: None },
        PartialFilterCase { op, kind: CodeOnly, depth: None },
        PartialFilterCase { op, kind: Structure, depth: None },
        PartialFilterCase { op, kind: CodeWithDepth, depth: Some(1) },
        PartialFilterCase { op, kind: EventWithDepth, depth: Some(1) },
    ];
    if matches!(op, Fetch) { // fetch 场景特有基线与非法表达式
        v.push(PartialFilterCase { op, kind: NoFilter, depth: None });
        v.push(PartialFilterCase { op, kind: InvalidFilter, depth: None });
    }
    v
}
