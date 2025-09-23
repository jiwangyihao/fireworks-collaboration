//! Shallow / depth related matrix definitions (12.5 → refactored 12.15)
//!
//! 目标：
//! * 统一浅克隆、加深、非法 depth、忽略场景与 file:// 方案代表性用例
//! * 提供精确的分类 / 描述能力，便于测试输出与最小子序列锚点命名
//! * 为后续“对象计数基线”“fetch 共享”扩展预留结构.
//!
//! 本次优化：
//! 1. 引入 `ShallowCaseKind`（稳定分类语义）
//! 2. 为 `ShallowCase` 添加核心辅助方法：`kind()`, `describe()`（其余判定/深度派生方法已移除以保持最小接口）
//! 3. 拆分子集合生成函数：`invalid_depth_cases()`, `deepen_cases()`, `ignore_cases()`, `file_url_cases()`，主集合 `shallow_cases()` 聚合它们
//! 4. 添加不变量测试：描述唯一性、加深合法性(from < to)、invalid 原始值域覆盖
//! 5. 移除全局 `allow(dead_code)`；通过更细粒度函数让编译器更可控地裁剪
//!
//! 兼容性：`shallow_cases()` 与枚举变体保持不变；已有调用无需修改。

use std::fmt::{Display, Formatter};
use crate::common::CaseDescribe;

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

/// 稳定分类（用于统计 / 分组断言 / 过滤）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShallowCaseKind {
    Depth,
    Invalid,
    Deepen,
    IgnoreClone,
    IgnoreFetch,
    FileUrl,
}

impl ShallowCase {
    /// 稳定分类 kind
    pub fn kind(&self) -> ShallowCaseKind {
        match self {
            ShallowCase::Depth { .. } => ShallowCaseKind::Depth,
            ShallowCase::Invalid { .. } => ShallowCaseKind::Invalid,
            ShallowCase::Deepen { .. } => ShallowCaseKind::Deepen,
            ShallowCase::LocalIgnoreClone { .. } => ShallowCaseKind::IgnoreClone,
            ShallowCase::LocalIgnoreFetch { .. } => ShallowCaseKind::IgnoreFetch,
            ShallowCase::FileUrlSequence { .. } => ShallowCaseKind::FileUrl,
        }
    }

    // 先前存在的 is_invalid / is_deepen / initial_depth / target_depth 等方法在当前测试集中未被使用，
    // 且可由模式匹配直接表达，故删除以收敛公共 API；未来若重新需要可在最小语义下增量回归。

    /// 稳定描述（可用于路径 slug / 测试名称）。
    pub fn describe(&self) -> String {
        match self {
            ShallowCase::Depth { depth } => format!("depth-{}", depth),
            ShallowCase::Invalid { label, .. } => format!("invalid-{}", label),
            ShallowCase::Deepen { from, to } => format!("deepen-{}-{}", from, to),
            ShallowCase::LocalIgnoreClone { depth } => format!("ignore-clone-{}", depth),
            ShallowCase::LocalIgnoreFetch { depth } => format!("ignore-fetch-{}", depth),
            ShallowCase::FileUrlSequence { initial, second } => format!("file-url-{}-{}", initial, second),
        }
    }
}

impl CaseDescribe for ShallowCase { fn describe(&self) -> String { self.describe() } }

// -------------------------- 子集合生成函数 --------------------------

pub fn depth_cases() -> Vec<ShallowCase> { vec![ShallowCase::Depth { depth: 1 }, ShallowCase::Depth { depth: 2 }] }
pub fn invalid_depth_cases() -> Vec<ShallowCase> { vec![
    ShallowCase::Invalid { raw: 0, label: "zero" },
    ShallowCase::Invalid { raw: -3, label: "negative" },
    ShallowCase::Invalid { raw: (i32::MAX as i64) + 1, label: "too-large" },
] }
pub fn deepen_cases() -> Vec<ShallowCase> { vec![
    ShallowCase::Deepen { from: 1, to: 2 },
    ShallowCase::Deepen { from: 2, to: 4 },
] }
pub fn ignore_cases() -> Vec<ShallowCase> { vec![
    ShallowCase::LocalIgnoreClone { depth: 1 },
    ShallowCase::LocalIgnoreFetch { depth: 1 },
] }
pub fn file_url_cases() -> Vec<ShallowCase> { vec![
    ShallowCase::FileUrlSequence { initial: 1, second: 2 },
] }

/// 代表性用例集合（可依据等价类增删）
pub fn shallow_cases() -> Vec<ShallowCase> {
    let mut v = Vec::new();
    v.extend(depth_cases());
    v.extend(invalid_depth_cases());
    v.extend(deepen_cases());
    v.extend(ignore_cases());
    v.extend(file_url_cases());
    v
}

// -------------------------- 不变量测试 --------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, HashMap};

    #[test]
    fn describe_uniqueness_and_kind_coverage() {
        let cases = shallow_cases();
        let _ = crate::common::assert_unique_describe(&cases);
        let mut kinds = HashSet::new();
        for c in &cases { 
            kinds.insert(c.kind());
        }
        // 所有 kind 应被覆盖
        assert_eq!(kinds.len(), 6, "expected every ShallowCaseKind to appear");
    }

    #[test]
    fn deepen_from_to_invariant() {
        for c in deepen_cases() { if let ShallowCase::Deepen { from, to } = c { assert!(from < to, "deepen from < to violated: {} >= {}", from, to); } }
    }

    #[test]
    fn invalid_labels_map_to_distinct_raw() {
        let mut map: HashMap<&'static str, i64> = HashMap::new();
        for c in invalid_depth_cases() { if let ShallowCase::Invalid { raw, label } = c { let prev = map.insert(label, raw); assert!(prev.is_none(), "duplicate invalid label: {}", label); } }
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn slug_helper_smoke() {
        // 取一个 case 生成 slug，验证仅含允许字符
        let case = ShallowCase::Deepen { from:1, to:2 };
        let slug = crate::common::describe_slug(&case);
        assert!(slug.chars().all(|ch| ch.is_ascii_alphanumeric() || ch=='-' || ch=='_'), "slug contains unexpected char: {slug}");
        assert!(slug.contains("deepen"));
    }
}
