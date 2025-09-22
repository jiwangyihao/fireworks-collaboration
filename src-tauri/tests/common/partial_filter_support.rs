//! Partial filter 支撑：引入 SupportLevel 与统一判定逻辑 (pre-12.8)。
//! 目的：在 clone 阶段先建立枚举语义，fetch 阶段 (12.8) 可直接复用。
//! 当前仍为占位实现：通过 filter label 字符串启发式推断。
//!
//! 规则暂定：
//!   * 包含 "unsupported" => Unsupported
//!   * 包含 "+depth" => DegradedPlaceholder (表示带 depth 的 filter 语义需后续细化)
//!   * 其它 => Supported
//!   * has_filter_marker: events 中存在包含 "filter:" (case-insensitive) 的条目。
//!
//! 后续真实实现替换：
//!   * 与底层 git 服务协商 capability，得到真实枚举。
//!   * depth 交叉使用 shallow helper 验证对象数变化。
//!   * DegradedPlaceholder 可能拆分为 Partial / Fallback 等更细粒度。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    Supported,
    Unsupported,
    DegradedPlaceholder,
    Invalid, // 新增：解析失败/非法表达式
}

#[derive(Debug, Clone)]
pub struct PartialFilterOutcome {
    pub support: SupportLevel,
    pub depth: Option<u32>,
    pub has_filter_marker: bool,
}

impl PartialFilterOutcome {
    pub fn is_fallback(&self) -> bool { matches!(self.support, SupportLevel::Unsupported) }
}

/// 若缺失 filter:* marker 且期望存在（有 filter 参数），输出统一格式警告。
pub fn warn_if_no_filter_marker(context: &str, label: &str, outcome: &PartialFilterOutcome) {
    if !outcome.has_filter_marker {
        eprintln!("[warn][{context}] missing filter:* marker (label={label})");
    }
}

pub fn assess_partial_filter(filter_label: &str, depth: Option<u32>, events: &[String]) -> PartialFilterOutcome {
    let lower = filter_label.to_ascii_lowercase();
    // 环境变量驱动 capability（测试模拟）：1=支持；未设置=未知；0=不支持
    let env_supported = std::env::var("FWC_PARTIAL_FILTER_SUPPORTED").ok().filter(|v| v=="1").is_some();
    let is_invalid = lower.contains("bad:") || lower.contains("invalid") || lower.contains("bad-filter") || lower.contains("bad:filter");
    // 判定优先级：Invalid > Unsupported(显式) > Degraded(depth 组合) > Supported(默认/显式支持) > Unsupported(保守)
    let support = if is_invalid { SupportLevel::Invalid }
        else if lower.contains("unsupported") { SupportLevel::Unsupported }
        else if lower.contains("+depth") || depth.is_some() {
            SupportLevel::DegradedPlaceholder
        } else if env_supported || lower.starts_with("filter:") || lower.is_empty() {
            SupportLevel::Supported
        } else {
            SupportLevel::Unsupported // 保守默认
        };
    let has_marker = events.iter().any(|e| e.to_ascii_lowercase().contains("filter:"));
    PartialFilterOutcome { support, depth, has_filter_marker: has_marker }
}

#[cfg(test)]
mod tests_pf_support {
    use super::*;
    #[test]
    fn assess_rules_basic() {
        let ev = vec!["Start".into(), "Filter: event-only".into()];
        let r = assess_partial_filter("filter:event-only", None, &ev);
        assert_eq!(r.support, SupportLevel::Supported);
        assert!(r.has_filter_marker);
        let r2 = assess_partial_filter("filter:unsupported-case", None, &ev);
        assert!(r2.is_fallback());
        let r3 = assess_partial_filter("filter:code+depth", Some(1), &ev);
        assert_eq!(r3.support, SupportLevel::DegradedPlaceholder);
        let r4 = assess_partial_filter("filter:bad:filter", None, &ev);
        assert_eq!(r4.support, SupportLevel::Invalid);
    }
}
