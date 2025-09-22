//! Shallow / depth related matrix definitions (12.5)
//! 提供统一的浅克隆与 deepen / 本地忽略 / 非法参数等代表性用例集合。
//! 后续可扩展：
//!   * 对象计数/引用数基线收集
//!   * 与 fetch 阶段共享 (扩展 variant)
//!
//! Post-audit(v2): 部分枚举在 12.5/12.7 组合中暂未被所有测试直接消费，
//! 产生 dead_code / unused 警告；在 fetch 与 future DSL 收紧前临时允许。

#![allow(dead_code)]

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub enum ShallowCase {
    /// 初始浅克隆 depth=N
    Depth { depth: u32 },
    /// 非法 depth：0 / 负值 / 过大
    Invalid { raw: i64, label: &'static str },
    /// 递进 deepen：从 from -> to
    Deepen { from: u32, to: u32 },
    /// 本地路径（clone/fetch）忽略 depth
    LocalIgnoreClone { depth: u32 },
    LocalIgnoreFetch { depth: u32 },
    /// file:// URL 方案（当前实现可能不支持，保留占位）
    FileUrlSequence { initial: u32, second: u32 },
}

impl Display for ShallowCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ShallowCase::Depth { depth } => write!(f, "Depth({})", depth),
            ShallowCase::Invalid { raw, label } => write!(f, "Invalid({}, {})", raw, label),
            ShallowCase::Deepen { from, to } => write!(f, "Deepen({}->{})", from, to),
            ShallowCase::LocalIgnoreClone { depth } => write!(f, "LocalIgnoreClone(depth={})", depth),
            ShallowCase::LocalIgnoreFetch { depth } => write!(f, "LocalIgnoreFetch(depth={})", depth),
            ShallowCase::FileUrlSequence { initial, second } => write!(f, "FileUrlSequence({}->{})", initial, second),
        }
    }
}

/// 代表性用例集合（可依据等价类增删）
pub fn shallow_cases() -> Vec<ShallowCase> {
    vec![
        ShallowCase::Depth { depth: 1 },
        ShallowCase::Depth { depth: 2 },
        ShallowCase::Invalid { raw: 0, label: "zero" },
        ShallowCase::Invalid { raw: -3, label: "negative" },
        ShallowCase::Invalid { raw: (i32::MAX as i64) + 1, label: "too-large" },
        ShallowCase::Deepen { from: 1, to: 2 },
        ShallowCase::Deepen { from: 2, to: 4 },
        ShallowCase::LocalIgnoreClone { depth: 1 },
        ShallowCase::LocalIgnoreFetch { depth: 1 },
        ShallowCase::FileUrlSequence { initial: 1, second: 2 },
    ]
}
