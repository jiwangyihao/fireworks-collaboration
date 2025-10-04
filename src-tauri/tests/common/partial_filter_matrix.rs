//! Partial Filter 测试矩阵（12.6 聚合 -> 预备 12.8 扩展）
//! ------------------------------------------------------
//! 目标：集中列举代表性 partial filter 场景，为 clone / fetch 共享做准备。
//! 版本轨迹：
//!   - v1 (12.6): 仅 clone 使用，区分事件 / 代码 / 结构 及其与 depth 的交叉。
//!   - v2 (Post-audit v3): 引入 `PartialFilterOp` 占位（当前只实现 Clone），
//!     与独立的 `partial_filter_support` (`SupportLevel`) 模块解耦，方便 12.8 fetch 聚合。
//!
//! Post-audit(v2): 使用 `allow(dead_code)` 抑制未使用枚举成员警告，等待 fetch 聚合。
//! Post-audit(v3): 添加 `PartialFilterOp`、文档化后续扩展计划（invalid / `no_filter` / fetch 特有）。
//!
//! 后续计划（12.8+）：
//!   * 扩展 op=Fetch 维度；
//!   * 增加 Invalid / `NoFilter` / `SparseEdge` 等用例；
//!   * 与事件断言 DSL 融合（替换字符串 contains）。

// 移除全局 dead_code 允许；若后续某些 fetch-only 枚举临时未被引用，可局部添加。

use crate::common::CaseDescribe;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartialFilterOp {
    Clone,
    Fetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartialFilterKind {
    EventOnly,
    CodeOnly,
    Structure,
    CodeWithDepth,
    EventWithDepth,
    NoFilter,      // fetch only: 未提供 --filter （用于对比 baseline）
    InvalidFilter, // fetch only: 解析失败/非法表达式
}

impl PartialFilterKind {
    /// 是否带 depth 语义（需要 depth:Some 才算合法深度用例）。
    pub fn is_depth_related(&self) -> bool {
        matches!(self, Self::CodeWithDepth | Self::EventWithDepth)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartialFilterCase {
    pub op: PartialFilterOp,
    pub kind: PartialFilterKind,
    pub depth: Option<u32>,
}

impl PartialFilterCase {
    /// 便捷描述（用于参数化测试名称 / `case.describe()`）。
    pub fn describe(&self) -> String {
        format!("{:?}-{:?}-d{}", self.op, self.kind, self.depth.unwrap_or(0))
    }
    // 先前的 is_depth_case() 在迁移后未被使用，可直接通过 `case.kind.is_depth_related() && case.depth.is_some()` 表达，已移除。
}

impl CaseDescribe for PartialFilterCase {
    fn describe(&self) -> String {
        self.describe()
    }
}

impl Display for PartialFilterCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let op = match self.op {
            PartialFilterOp::Clone => "Clone",
            PartialFilterOp::Fetch => "Fetch",
        };
        match self.kind {
            PartialFilterKind::EventOnly => write!(f, "{op}:EventOnly"),
            PartialFilterKind::CodeOnly => write!(f, "{op}:CodeOnly"),
            PartialFilterKind::Structure => write!(f, "{op}:Structure"),
            PartialFilterKind::CodeWithDepth => {
                write!(f, "{op}:CodeWithDepth(depth={})", self.depth.unwrap_or(0))
            }
            PartialFilterKind::EventWithDepth => {
                write!(f, "{op}:EventWithDepth(depth={})", self.depth.unwrap_or(0))
            }
            PartialFilterKind::NoFilter => write!(f, "{op}:NoFilter"),
            PartialFilterKind::InvalidFilter => write!(f, "{op}:InvalidFilter"),
        }
    }
}

/// 新：专用 clone 集合（语义更清晰，可在调用侧替换旧函数）。
pub fn clone_partial_filter_cases() -> Vec<PartialFilterCase> {
    partial_filter_cases_for(PartialFilterOp::Clone)
}
/// 新：专用 fetch 集合。
pub fn fetch_partial_filter_cases() -> Vec<PartialFilterCase> {
    partial_filter_cases_for(PartialFilterOp::Fetch)
}

/// 全量（clone + fetch）用例集合，便于需要覆盖所有 op 的测试。
pub fn all_partial_filter_cases() -> Vec<PartialFilterCase> {
    [clone_partial_filter_cases(), fetch_partial_filter_cases()]
        .into_iter()
        .flatten()
        .collect()
}

/// 按 op 生成用例集合（内部共享逻辑）。
pub fn partial_filter_cases_for(op: PartialFilterOp) -> Vec<PartialFilterCase> {
    use PartialFilterKind::*;
    let base = [
        EventOnly,
        CodeOnly,
        Structure,
        CodeWithDepth,
        EventWithDepth,
    ];
    let mut out: Vec<PartialFilterCase> = base
        .into_iter()
        .map(|k| PartialFilterCase {
            op,
            kind: k,
            depth: depth_for_kind(k),
        })
        .collect();
    if matches!(op, PartialFilterOp::Fetch) {
        out.push(PartialFilterCase {
            op,
            kind: NoFilter,
            depth: None,
        });
        out.push(PartialFilterCase {
            op,
            kind: InvalidFilter,
            depth: None,
        });
    }
    out
}

fn depth_for_kind(kind: PartialFilterKind) -> Option<u32> {
    match kind {
        PartialFilterKind::CodeWithDepth | PartialFilterKind::EventWithDepth => Some(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests_partial_filter_matrix {
    use super::*;
    #[test]
    fn clone_cases_depth_invariants() {
        for c in clone_partial_filter_cases() {
            if c.kind.is_depth_related() {
                assert!(
                    c.depth.is_some(),
                    "depth-related kind must carry depth: {c:?}"
                );
            }
        }
    }
    #[test]
    fn fetch_specific_kinds_present() {
        let fetch_cases = fetch_partial_filter_cases();
        assert!(
            fetch_cases
                .iter()
                .any(|c| matches!(c.kind, PartialFilterKind::NoFilter)),
            "fetch should have NoFilter case"
        );
        assert!(
            fetch_cases
                .iter()
                .any(|c| matches!(c.kind, PartialFilterKind::InvalidFilter)),
            "fetch should have InvalidFilter case"
        );
    }
    #[test]
    fn describe_unique() {
        let all: Vec<_> = all_partial_filter_cases();
        let _ = crate::common::assert_unique_describe(&all);
    }
}
