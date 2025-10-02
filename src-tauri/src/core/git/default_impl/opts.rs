use serde::Deserialize;

use crate::core::git::errors::{ErrorCategory, GitError};

/// Supported partial clone filters (initial P2.2 scope)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartialFilter {
    BlobNone,
    TreeZero,
}

impl PartialFilter {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "blob:none" => Some(Self::BlobNone),
            "tree:0" => Some(Self::TreeZero),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BlobNone => "blob:none",
            Self::TreeZero => "tree:0",
        }
    }
}

/// Strategy override white-listed subsets (P2.3 future). We only parse structure now;
/// application will be done in later phases. Unknown fields are ignored by serde.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyHttpOverride {
    #[serde(alias = "follow_redirects")]
    pub follow_redirects: Option<bool>,
    #[serde(alias = "max_redirects")]
    pub max_redirects: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyTlsOverride {
    #[serde(alias = "insecureSkipVerify", alias = "insecure_skip_verify")]
    pub insecure_skip_verify: Option<bool>,
    #[serde(alias = "skipSanWhitelist", alias = "skip_san_whitelist")]
    pub skip_san_whitelist: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyRetryOverride {
    pub max: Option<u32>,
    #[serde(alias = "baseMs")]
    pub base_ms: Option<u32>,
    pub factor: Option<f32>,
    pub jitter: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyOverrideInput {
    pub http: Option<StrategyHttpOverride>,
    pub tls: Option<StrategyTlsOverride>,
    pub retry: Option<StrategyRetryOverride>,
    // Unknown top-level keys are ignored by serde; we emit warn logs in parser when present.
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitDepthFilterOpts {
    pub depth: Option<u32>,
    pub filter: Option<PartialFilter>,
    /// Parsed strategy override (P2.3a: parse only, application in later phases)
    pub strategy_override: Option<StrategyOverrideInput>,
    /// P2.3e: ignored top-level keys (unknown)
    pub ignored_top_level: Vec<String>,
    /// P2.3e: ignored nested keys (section.key)
    pub ignored_nested: Vec<(String, String)>,
}

impl GitDepthFilterOpts {
    pub fn empty() -> Self {
        Self {
            depth: None,
            filter: None,
            strategy_override: None,
            ignored_top_level: vec![],
            ignored_nested: vec![],
        }
    }
}

/// Result for strategy override parse including ignored (unknown) field names (P2.3e)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StrategyOverrideParseResult {
    pub parsed: Option<StrategyOverrideInput>,
    /// Unknown top-level keys
    pub ignored_top_level: Vec<String>,
    /// Unknown nested keys grouped by section name (http/tls/retry)
    pub ignored_nested: Vec<(String, String)>, // (section, key)
}

impl StrategyOverrideParseResult {
    pub fn is_empty(&self) -> bool {
        self.ignored_top_level.is_empty() && self.ignored_nested.is_empty()
    }
}

/// Internal helper: parse only the strategyOverride JSON value with unknown field detection.
/// Now returns a structured result包含被忽略字段列表，供任务层决定是否发事件。
pub fn parse_strategy_override(
    strategy_override: Option<serde_json::Value>,
) -> Result<StrategyOverrideParseResult, GitError> {
    if let Some(raw) = strategy_override {
        if raw.is_null() {
            return Ok(StrategyOverrideParseResult::default());
        }
        if !raw.is_object() {
            return Err(GitError::new(
                ErrorCategory::Protocol,
                "invalid strategyOverride: not an object",
            ));
        }
        let obj = raw.as_object().unwrap();
        let top_keys: Vec<String> = obj.keys().cloned().collect();
        let parsed: StrategyOverrideInput = serde_json::from_value(raw.clone()).map_err(|e| {
            GitError::new(
                ErrorCategory::Protocol,
                format!("invalid strategyOverride: {}", e),
            )
        })?;
        let mut res = StrategyOverrideParseResult {
            parsed: Some(parsed),
            ignored_top_level: vec![],
            ignored_nested: vec![],
        };
        // detect unknown top-level keys
        for k in &top_keys {
            if k != "http" && k != "tls" && k != "retry" {
                tracing::warn!(target="strategy", key=%k, "unknown top-level strategyOverride key ignored");
                res.ignored_top_level.push(k.clone());
            }
        }
        // detect unknown nested keys (best-effort; ignore errors)
        if let Some(http) = obj.get("http").and_then(|v| v.as_object()) {
            for k in http.keys() {
                if !matches!(
                    k.as_str(),
                    "followRedirects" | "follow_redirects" | "maxRedirects" | "max_redirects"
                ) {
                    tracing::warn!(target="strategy", section="http", key=%k, "unknown http override field ignored");
                    res.ignored_nested.push(("http".into(), k.clone()));
                }
            }
        }
        if let Some(tls) = obj.get("tls").and_then(|v| v.as_object()) {
            for k in tls.keys() {
                if !matches!(
                    k.as_str(),
                    "insecureSkipVerify"
                        | "skipSanWhitelist"
                        | "insecure_skip_verify"
                        | "skip_san_whitelist"
                ) {
                    tracing::warn!(target="strategy", section="tls", key=%k, "unknown tls override field ignored");
                    res.ignored_nested.push(("tls".into(), k.clone()));
                }
            }
        }
        if let Some(retry) = obj.get("retry").and_then(|v| v.as_object()) {
            for k in retry.keys() {
                if !matches!(
                    k.as_str(),
                    "max" | "baseMs" | "factor" | "jitter" | "base_ms"
                ) {
                    tracing::warn!(target="strategy", section="retry", key=%k, "unknown retry override field ignored");
                    res.ignored_nested.push(("retry".into(), k.clone()));
                }
            }
        }

        // value/range validation (P2.3a enhancement)
        if let Some(http) = &res.parsed.as_ref().unwrap().http {
            if let Some(max_r) = http.max_redirects {
                if max_r > 20 {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        "http.maxRedirects too large (max 20)",
                    ));
                }
            }
        }
        if let Some(retry) = &res.parsed.as_ref().unwrap().retry {
            if let Some(m) = retry.max {
                if m == 0 || m > 20 {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        "retry.max must be 1..=20",
                    ));
                }
            }
            if let Some(base) = retry.base_ms {
                if base < 10 || base > 60_000 {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        "retry.baseMs out of range (10..60000)",
                    ));
                }
            }
            if let Some(f) = retry.factor {
                if !(0.5..=10.0).contains(&f) {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        "retry.factor out of range (0.5..=10.0)",
                    ));
                }
            }
        }
        return Ok(res);
    }
    Ok(StrategyOverrideParseResult::default())
}

/// Parse and validate depth/filter/strategyOverride portion. Returns Protocol errors for invalid values.
pub fn parse_depth_filter_opts(
    depth: Option<serde_json::Value>,
    filter: Option<String>,
    strategy_override: Option<serde_json::Value>,
) -> Result<GitDepthFilterOpts, GitError> {
    let mut out = GitDepthFilterOpts::empty();

    // depth validation (positive integer)
    if let Some(dv) = depth {
        if !dv.is_null() {
            let as_i64 = if dv.is_number() { dv.as_i64() } else { None };
            match as_i64 {
                Some(v) if v > 0 => {
                    if v > i32::MAX as i64 {
                        return Err(GitError::new(ErrorCategory::Protocol, "depth too large"));
                    }
                    out.depth = Some(v as u32);
                }
                Some(v) if v <= 0 => {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        "depth must be positive",
                    ));
                }
                None => {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        "depth must be a number",
                    ));
                }
                _ => {}
            }
        }
    }

    // filter validation
    if let Some(f_str) = filter.as_ref() {
        if !f_str.trim().is_empty() {
            match PartialFilter::parse(f_str.trim()) {
                Some(pf) => out.filter = Some(pf),
                None => {
                    return Err(GitError::new(
                        ErrorCategory::Protocol,
                        format!("unsupported filter: {}", f_str),
                    ))
                }
            }
        }
    }

    // strategyOverride parsing (with warnings)
    let parsed_res = parse_strategy_override(strategy_override)?;
    out.strategy_override = parsed_res.parsed; // ignored 字段由上层选择性处理 (P2.3e)
    out.ignored_top_level = parsed_res.ignored_top_level;
    out.ignored_nested = parsed_res.ignored_nested;

    Ok(out)
}
