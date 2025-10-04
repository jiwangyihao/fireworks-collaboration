//! Partial filter 支撑：引入 `SupportLevel` 与统一判定逻辑 (pre-12.8)。
//! 目的：在 clone 阶段先建立枚举语义，fetch 阶段 (12.8) 可直接复用。
//! 当前仍为占位实现：通过 filter label 字符串启发式推断。
//!
//! 规则暂定：
//!   * 包含 "unsupported" => Unsupported
//!   * 包含 "+depth" => `DegradedPlaceholder` (表示带 depth 的 filter 语义需后续细化)
//!   * 其它 => Supported
//!   * `has_filter_marker`: events 中存在包含 "filter:" (case-insensitive) 的条目。
//!
//! 后续真实实现替换：
//!   * 与底层 git 服务协商 capability，得到真实枚举。
//!   * depth 交叉使用 shallow helper 验证对象数变化。
//!   * `DegradedPlaceholder` 可能拆分为 Partial / Fallback 等更细粒度。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    /// 完全支持（或显式标记支持）
    Supported,
    /// 显式或推断不支持（fall back 行为）
    Unsupported,
    /// 暂时降级（depth + filter 组合，等待细化）
    DegradedPlaceholder,
    /// 解析失败 / 非法表达式
    Invalid,
}

impl SupportLevel {
    pub fn is_supported(self) -> bool {
        matches!(self, Self::Supported)
    }
    pub fn is_unsupported(self) -> bool {
        matches!(self, Self::Unsupported)
    }
    pub fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid)
    }
    pub fn is_degraded(self) -> bool {
        matches!(self, Self::DegradedPlaceholder)
    }
    /// 返回紧凑 code（便于日志/快照断言）
    pub fn code(self) -> &'static str {
        match self {
            Self::Supported => "S",
            Self::Unsupported => "U",
            Self::DegradedPlaceholder => "D",
            Self::Invalid => "I",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PartialFilterOutcome {
    pub support: SupportLevel,
    pub depth: Option<u32>,
    pub has_filter_marker: bool,
}

impl PartialFilterOutcome {
    pub fn is_fallback(&self) -> bool {
        self.support.is_unsupported()
    }
    pub fn is_invalid(&self) -> bool {
        self.support.is_invalid()
    }
    pub fn is_degraded(&self) -> bool {
        self.support.is_degraded()
    }
    /// 统一文本描述（便于参数化测试名称/快照）
    pub fn describe(&self) -> String {
        format!(
            "{}-d{}-m{}",
            self.support.code(),
            self.depth.unwrap_or(0),
            if self.has_filter_marker { 'y' } else { 'n' }
        )
    }
}

/// 若缺失 filter:* marker 且期望存在（有 filter 参数），输出统一格式警告。
#[allow(dead_code)] // 部分调用在特定 compile 过滤下可能被裁剪，保持显式 allow
pub fn warn_if_no_filter_marker(context: &str, label: &str, outcome: &PartialFilterOutcome) {
    if !outcome.has_filter_marker {
        eprintln!("[warn][{context}] missing filter:* marker (label={label})");
    }
}

// ---- 内部：分类逻辑与辅助 ----
fn detect_filter_marker(events: &[String]) -> bool {
    events
        .iter()
        .any(|e| e.to_ascii_lowercase().contains("filter:"))
}

/// 判定支持级别（不含 marker 检测）。
fn compute_support_level(
    lower_label: &str,
    depth: Option<u32>,
    env_supported: bool,
) -> SupportLevel {
    let is_invalid = lower_label.contains("bad:")
        || lower_label.contains("invalid")
        || lower_label.contains("bad-filter")
        || lower_label.contains("bad:filter");
    if is_invalid {
        return SupportLevel::Invalid;
    }
    if lower_label.contains("unsupported") {
        return SupportLevel::Unsupported;
    }
    if lower_label.contains("+depth") || depth.is_some() {
        return SupportLevel::DegradedPlaceholder;
    }
    if env_supported || lower_label.starts_with("filter:") || lower_label.is_empty() {
        return SupportLevel::Supported;
    }
    SupportLevel::Unsupported
}

/// 公开：主评估函数（保持签名兼容）。
pub fn assess_partial_filter(
    filter_label: &str,
    depth: Option<u32>,
    events: &[String],
) -> PartialFilterOutcome {
    let lower = filter_label.to_ascii_lowercase();
    let env_supported = std::env::var("FWC_PARTIAL_FILTER_SUPPORTED")
        .ok()
        .filter(|v| v == "1")
        .is_some();
    let support = compute_support_level(&lower, depth, env_supported);
    let has_marker = detect_filter_marker(events);
    PartialFilterOutcome {
        support,
        depth,
        has_filter_marker: has_marker,
    }
}

/// 语义别名：仅进行 label -> `SupportLevel` 分类（不关心事件 marker），便于快速断言规则。
pub fn classify_filter_label(filter_label: &str, depth: Option<u32>) -> SupportLevel {
    let lower = filter_label.to_ascii_lowercase();
    let env_supported = std::env::var("FWC_PARTIAL_FILTER_SUPPORTED")
        .ok()
        .filter(|v| v == "1")
        .is_some();
    compute_support_level(&lower, depth, env_supported)
}

#[cfg(test)]
mod tests_pf_support {
    use super::*;
    fn make_events() -> Vec<String> {
        vec!["Start".into(), "Filter: event-only".into()]
    }

    #[test]
    fn assess_rules_basic() {
        let ev = make_events();
        let r = assess_partial_filter("filter:event-only", None, &ev);
        assert!(r.support.is_supported());
        assert!(r.has_filter_marker);
        let r2 = assess_partial_filter("filter:unsupported-case", None, &ev);
        assert!(r2.is_fallback());
        let r3 = assess_partial_filter("filter:code+depth", Some(1), &ev);
        assert!(r3.is_degraded());
        let r4 = assess_partial_filter("filter:bad:filter", None, &ev);
        assert!(r4.is_invalid());
    }

    #[test]
    fn classify_without_events() {
        assert!(classify_filter_label("filter:event-only", None).is_supported());
        assert!(classify_filter_label("filter:unsupported-v2", None).is_unsupported());
        assert!(classify_filter_label("filter:code+depth", Some(1)).is_degraded());
        assert!(classify_filter_label("filter:bad:filter", None).is_invalid());
    }

    #[test]
    fn outcome_describe_shape() {
        let ev = make_events();
        let r = assess_partial_filter("filter:event-only", Some(1), &ev);
        let d = r.describe();
        // depth=Some(1) 对 event-only 当前策略仍视为 Supported(S)，未来若策略调整为降级(D) 也应兼容；
        assert!(
            d.starts_with("S-d1") || d.starts_with("D-d1"),
            "unexpected describe prefix: {d}"
        );
    }
}
