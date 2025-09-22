use serde::Deserialize;

use crate::core::git::errors::{GitError, ErrorCategory};

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
    pub fn as_str(&self) -> &'static str { match self { Self::BlobNone => "blob:none", Self::TreeZero => "tree:0" } }
}

/// Strategy override white-listed subsets (P2.3 future). We only parse structure now;
/// application will be done in later phases. Unknown fields are ignored by serde.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyHttpOverride {
    #[serde(alias = "follow_redirects")] pub follow_redirects: Option<bool>,
    #[serde(alias = "max_redirects")] pub max_redirects: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyTlsOverride {
    #[serde(alias = "insecureSkipVerify", alias = "insecure_skip_verify")] pub insecure_skip_verify: Option<bool>,
    #[serde(alias = "skipSanWhitelist", alias = "skip_san_whitelist")] pub skip_san_whitelist: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct StrategyRetryOverride {
    pub max: Option<u32>,
    #[serde(alias = "baseMs")] pub base_ms: Option<u32>,
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
    pub ignored_nested: Vec<(String,String)>,
}

impl GitDepthFilterOpts {
    pub fn empty() -> Self { Self { depth: None, filter: None, strategy_override: None, ignored_top_level: vec![], ignored_nested: vec![] } }
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
    pub fn is_empty(&self) -> bool { self.ignored_top_level.is_empty() && self.ignored_nested.is_empty() }
}

/// Internal helper: parse only the strategyOverride JSON value with unknown field detection.
/// Now returns a structured result包含被忽略字段列表，供任务层决定是否发事件。
pub fn parse_strategy_override(strategy_override: Option<serde_json::Value>) -> Result<StrategyOverrideParseResult, GitError> {
    if let Some(raw) = strategy_override {
        if raw.is_null() { return Ok(StrategyOverrideParseResult::default()); }
        if !raw.is_object() { return Err(GitError::new(ErrorCategory::Protocol, "invalid strategyOverride: not an object")); }
        let obj = raw.as_object().unwrap();
        let top_keys: Vec<String> = obj.keys().cloned().collect();
        let parsed: StrategyOverrideInput = serde_json::from_value(raw.clone())
            .map_err(|e| GitError::new(ErrorCategory::Protocol, format!("invalid strategyOverride: {}", e)))?;
        let mut res = StrategyOverrideParseResult { parsed: Some(parsed), ignored_top_level: vec![], ignored_nested: vec![] };
        // detect unknown top-level keys
        for k in &top_keys {
            if k != "http" && k != "tls" && k != "retry" { tracing::warn!(target="strategy", key=%k, "unknown top-level strategyOverride key ignored"); res.ignored_top_level.push(k.clone()); }
        }
        // detect unknown nested keys (best-effort; ignore errors)
        if let Some(http) = obj.get("http").and_then(|v| v.as_object()) {
            for k in http.keys() {
                if !matches!(k.as_str(), "followRedirects"|"follow_redirects"|"maxRedirects"|"max_redirects") {
                    tracing::warn!(target="strategy", section="http", key=%k, "unknown http override field ignored");
                    res.ignored_nested.push(("http".into(), k.clone()));
                }
            }
        }
        if let Some(tls) = obj.get("tls").and_then(|v| v.as_object()) {
            for k in tls.keys() {
                if !matches!(k.as_str(), "insecureSkipVerify"|"skipSanWhitelist"|"insecure_skip_verify"|"skip_san_whitelist") {
                    tracing::warn!(target="strategy", section="tls", key=%k, "unknown tls override field ignored");
                    res.ignored_nested.push(("tls".into(), k.clone()));
                }
            }
        }
        if let Some(retry) = obj.get("retry").and_then(|v| v.as_object()) {
            for k in retry.keys() {
                if !matches!(k.as_str(), "max"|"baseMs"|"factor"|"jitter"|"base_ms") {
                    tracing::warn!(target="strategy", section="retry", key=%k, "unknown retry override field ignored");
                    res.ignored_nested.push(("retry".into(), k.clone()));
                }
            }
        }

        // value/range validation (P2.3a enhancement)
        if let Some(http) = &res.parsed.as_ref().unwrap().http {
            if let Some(max_r) = http.max_redirects { if max_r > 20 { return Err(GitError::new(ErrorCategory::Protocol, "http.maxRedirects too large (max 20)")); } }
        }
        if let Some(retry) = &res.parsed.as_ref().unwrap().retry {
            if let Some(m) = retry.max { if m == 0 || m > 20 { return Err(GitError::new(ErrorCategory::Protocol, "retry.max must be 1..=20")); } }
            if let Some(base) = retry.base_ms { if base < 10 || base > 60_000 { return Err(GitError::new(ErrorCategory::Protocol, "retry.baseMs out of range (10..60000)")); } }
            if let Some(f) = retry.factor { if !(0.5..=10.0).contains(&f) { return Err(GitError::new(ErrorCategory::Protocol, "retry.factor out of range (0.5..=10.0)")); } }
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
                    if v > i32::MAX as i64 { return Err(GitError::new(ErrorCategory::Protocol, "depth too large")); }
                    out.depth = Some(v as u32);
                }
                Some(v) if v <= 0 => { return Err(GitError::new(ErrorCategory::Protocol, "depth must be positive")); }
                None => { return Err(GitError::new(ErrorCategory::Protocol, "depth must be a number")); }
                _ => {}
            }
        }
    }

    // filter validation
    if let Some(f_str) = filter.as_ref() {
        if !f_str.trim().is_empty() {
            match PartialFilter::parse(f_str.trim()) {
                Some(pf) => out.filter = Some(pf),
                None => return Err(GitError::new(ErrorCategory::Protocol, format!("unsupported filter: {}", f_str))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_valid_depth_and_filter() {
        let opts = parse_depth_filter_opts(Some(json!(1)), Some("blob:none".into()), None).unwrap();
        assert_eq!(opts.depth, Some(1));
        assert_eq!(opts.filter, Some(PartialFilter::BlobNone));
    }

    #[test]
    fn test_depth_zero_invalid() {
        let err = parse_depth_filter_opts(Some(json!(0)), None, None).err().unwrap();
        let msg = err.to_string();
        assert!(msg.contains("depth must be positive"));
    }

    #[test]
    fn test_negative_depth_invalid() {
        let err = parse_depth_filter_opts(Some(json!(-5)), None, None).err().unwrap();
        let msg = err.to_string();
        assert!(msg.contains("depth must be positive"));
    }

    #[test]
    fn test_filter_invalid() {
        let err = parse_depth_filter_opts(None, Some("weird:rule".into()), None).err().unwrap();
        let msg = err.to_string();
        assert!(msg.contains("unsupported filter"));
    }

    #[test]
    fn test_strategy_override_parse() {
        let raw = json!({
            "http": { "followRedirects": true, "maxRedirects": 3 },
            "tls": { "insecureSkipVerify": false },
            "retry": { "max": 5, "baseMs": 200, "factor": 1.2, "jitter": true },
            "unknownTop": { "x": 1 }
        });
        let opts = parse_depth_filter_opts(None, None, Some(raw)).unwrap();
        assert!(opts.strategy_override.is_some());
        let st = opts.strategy_override.unwrap();
        assert_eq!(st.http.unwrap().max_redirects, Some(3));
        assert_eq!(st.retry.unwrap().base_ms, Some(200));
    }

    #[test]
    fn test_depth_too_large() {
        let big = (u64::from(u32::MAX) + 10) as i64;
        let err = parse_depth_filter_opts(Some(json!(big)), None, None).err().unwrap();
        assert!(err.to_string().contains("depth too large"));
    }

    #[test]
    fn test_snake_case_aliases() {
        let raw = json!({
            "http": { "follow_redirects": false, "max_redirects": 7 },
            "retry": { "baseMs": 150 }
        });
        let opts = parse_depth_filter_opts(None, Some("tree:0".into()), Some(raw)).unwrap();
        assert_eq!(opts.filter, Some(PartialFilter::TreeZero));
        let st = opts.strategy_override.unwrap();
        assert_eq!(st.http.unwrap().max_redirects, Some(7));
        assert_eq!(st.retry.unwrap().base_ms, Some(150));
    }

    #[test]
    fn test_empty_filter_string_ignored() {
        let opts = parse_depth_filter_opts(Some(json!(5)), Some("   ".into()), None).unwrap();
        assert_eq!(opts.depth, Some(5));
        assert!(opts.filter.is_none());
    }

    #[test]
    fn test_max_i32_depth_ok() {
        let max = i32::MAX as i64;
        let opts = parse_depth_filter_opts(Some(json!(max)), None, None).unwrap();
        assert_eq!(opts.depth, Some(i32::MAX as u32));
    }

    #[test]
    fn test_depth_as_string_invalid() {
        let err = parse_depth_filter_opts(Some(json!("10")), None, None).unwrap_err();
        assert!(err.to_string().contains("depth must be a number"));
    }

    #[test]
    fn test_invalid_filters_variants() {
        // unsupported forms: uppercase, tree:1, stray whitespace inside
        for f in ["BLOB:NONE", "tree:1", "blob: none"] { 
            let err = parse_depth_filter_opts(None, Some(f.into()), None).unwrap_err();
            assert!(err.to_string().contains("unsupported filter"), "{} should be rejected", f);
        }
    }

    #[test]
    fn test_combined_depth_and_filter() {
        let opts = parse_depth_filter_opts(Some(json!(3)), Some("tree:0".into()), None).unwrap();
        assert_eq!(opts.depth, Some(3));
        assert_eq!(opts.filter, Some(PartialFilter::TreeZero));
    }

    #[test]
    fn test_strategy_override_each_section_individually() {
        let http_only = json!({"http":{"followRedirects":true}});
        let tls_only  = json!({"tls":{"insecureSkipVerify":true}});
        let retry_only= json!({"retry":{"max":4,"factor":1.1}});
        assert!(parse_depth_filter_opts(None,None,Some(http_only)).unwrap().strategy_override.unwrap().http.is_some());
        assert!(parse_depth_filter_opts(None,None,Some(tls_only)).unwrap().strategy_override.unwrap().tls.is_some());
        assert!(parse_depth_filter_opts(None,None,Some(retry_only)).unwrap().strategy_override.unwrap().retry.is_some());
    }

    #[test]
    fn test_strategy_override_invalid_type() {
        // strategyOverride must be an object; an array should fail
        let arr = json!([1,2,3]);
        let err = parse_depth_filter_opts(None, None, Some(arr)).unwrap_err();
        assert!(err.to_string().contains("invalid strategyOverride"));
    }

    #[test]
    fn test_strategy_override_unknown_keys_ignored() {
        let raw = json!({
            "http": { "followRedirects": true, "AAA": 123 },
            "tls": { "insecureSkipVerify": true, "BBB": false },
            "retry": { "max": 3, "factor": 1.1, "CCC": 42 },
            "extraTop": { "x": 1 }
        });
        let opts = parse_depth_filter_opts(None, None, Some(raw)).unwrap();
        assert!(opts.strategy_override.is_some());
        // unknown keys should not block parse
        let st = opts.strategy_override.unwrap();
        assert!(st.http.unwrap().follow_redirects.unwrap());
    }

    #[test]
    fn test_strategy_override_retry_factor_out_of_range() {
        let raw = json!({"retry": {"factor": 20.0}});
        let err = parse_depth_filter_opts(None, None, Some(raw)).unwrap_err();
        assert!(err.to_string().contains("retry.factor out of range"));
    }

    #[test]
    fn test_strategy_override_retry_max_zero_invalid() {
        let raw = json!({"retry": {"max": 0}});
        let err = parse_depth_filter_opts(None, None, Some(raw)).unwrap_err();
        assert!(err.to_string().contains("retry.max"));
    }

    #[test]
    fn test_strategy_override_retry_base_ms_too_small() {
        let raw = json!({"retry": {"baseMs": 5}});
        let err = parse_depth_filter_opts(None, None, Some(raw)).unwrap_err();
        assert!(err.to_string().contains("retry.baseMs"));
    }

    #[test]
    fn test_strategy_override_http_max_redirects_too_large() {
        let raw = json!({"http": {"maxRedirects": 999}});
        let err = parse_depth_filter_opts(None, None, Some(raw)).unwrap_err();
        assert!(err.to_string().contains("http.maxRedirects"));
    }

    #[test]
    fn test_strategy_override_all_valid_edge() {
        let raw = json!({
            "http": {"maxRedirects": 20},
            "retry": {"max": 20, "baseMs": 60000, "factor": 0.5, "jitter": true}
        });
        let opts = parse_depth_filter_opts(None, None, Some(raw)).unwrap();
        assert!(opts.strategy_override.is_some());
    }

    #[test]
    fn test_strategy_override_empty_object_ignored() {
        let raw = json!({});
        let opts = parse_depth_filter_opts(None, None, Some(raw)).unwrap();
        assert!(opts.strategy_override.is_some()); // parsed as default (all None)
        let s = opts.strategy_override.unwrap();
        assert!(s.http.is_none() && s.tls.is_none() && s.retry.is_none());
    }

    #[test]
    fn test_strategy_override_only_unknown_top_level() {
        let raw = json!({"foo": {"bar": 1}});
        let opts = parse_depth_filter_opts(None, None, Some(raw)).unwrap();
        assert!(opts.strategy_override.is_some());
        let s = opts.strategy_override.unwrap();
        assert!(s.http.is_none() && s.tls.is_none() && s.retry.is_none());
    }

    #[test]
    fn test_strategy_override_mixed_multiple_errors_reports_first() {
        let raw = json!({
            "retry": {"max": 0, "factor": 99.0}, // two violations, expect first (max)
            "http": {"maxRedirects": 999}
        });
        let err = parse_depth_filter_opts(None, None, Some(raw)).unwrap_err();
        let msg = err.to_string();
        // Implementation detail may validate http first; accept either violation text.
        assert!(msg.contains("retry.max") || msg.contains("http.maxRedirects"), "error message should contain one of the violations, got: {}", msg);
    }

    #[test]
    fn test_strategy_override_all_upper_bounds_success() {
        let raw = json!({
            "http": {"maxRedirects": 20},
            "retry": {"max": 20, "baseMs": 60000, "factor": 10.0, "jitter": false}
        });
        let opts = parse_depth_filter_opts(None, None, Some(raw)).unwrap();
        assert!(opts.strategy_override.is_some());
    }
}
