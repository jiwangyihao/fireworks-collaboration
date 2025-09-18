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
    #[serde(alias = "insecureSkipVerify")] pub insecure_skip_verify: Option<bool>,
    #[serde(alias = "skipSanWhitelist")] pub skip_san_whitelist: Option<bool>,
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
    // Unknown top-level keys are ignored by serde, satisfying 'ignore unknown & warn' (warn logged later).
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitDepthFilterOpts {
    pub depth: Option<u32>,
    pub filter: Option<PartialFilter>,
    /// Raw strategy override JSON (preserved for logging / future application)
    pub strategy_override: Option<StrategyOverrideInput>,
}

impl GitDepthFilterOpts {
    pub fn empty() -> Self { Self { depth: None, filter: None, strategy_override: None } }
}

/// Parse and validate depth/filter/strategyOverride portion. Does not alter runtime behavior yet.
/// Returns Protocol errors for invalid user-supplied values (P2.2a requirement).
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
                    if v > u32::MAX as i64 { return Err(GitError::new(ErrorCategory::Protocol, "depth too large")); }
                    out.depth = Some(v as u32);
                }
                Some(v) if v <= 0 => {
                    return Err(GitError::new(ErrorCategory::Protocol, "depth must be positive"));
                }
                None => {
                    return Err(GitError::new(ErrorCategory::Protocol, "depth must be a number"));
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
                None => return Err(GitError::new(ErrorCategory::Protocol, format!("unsupported filter: {}", f_str))),
            }
        }
    }

    // strategyOverride: we accept an object; unknown keys inside will be ignored.
    if let Some(raw) = strategy_override {
        if !raw.is_null() {
            if let Some(obj) = raw.as_object() { if obj.is_empty() { /* ignore empty */ } }
            let parsed: StrategyOverrideInput = serde_json::from_value(raw).map_err(|e| GitError::new(ErrorCategory::Protocol, format!("invalid strategyOverride: {}", e)))?;
            out.strategy_override = Some(parsed);
        }
    }

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
    fn test_max_u32_depth_ok() {
        let max = u32::MAX as i64;
        let opts = parse_depth_filter_opts(Some(json!(max)), None, None).unwrap();
        assert_eq!(opts.depth, Some(u32::MAX));
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
}
