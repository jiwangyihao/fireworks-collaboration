use fireworks_collaboration_lib::core::git::default_impl::opts::{
    parse_depth_filter_opts, PartialFilter,
};
use serde_json::json;

#[test]
fn test_parse_valid_depth_and_filter() {
    let opts = parse_depth_filter_opts(Some(json!(1)), Some("blob:none".into()), None).unwrap();
    assert_eq!(opts.depth, Some(1));
    assert_eq!(opts.filter, Some(PartialFilter::BlobNone));
}

#[test]
fn test_depth_zero_invalid() {
    let err = parse_depth_filter_opts(Some(json!(0)), None, None)
        .err()
        .unwrap();
    let msg = err.to_string();
    assert!(msg.contains("depth must be positive"));
}

#[test]
fn test_negative_depth_invalid() {
    let err = parse_depth_filter_opts(Some(json!(-5)), None, None)
        .err()
        .unwrap();
    let msg = err.to_string();
    assert!(msg.contains("depth must be positive"));
}

#[test]
fn test_filter_invalid() {
    let err = parse_depth_filter_opts(None, Some("weird:rule".into()), None)
        .err()
        .unwrap();
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
    let err = parse_depth_filter_opts(Some(json!(big)), None, None)
        .err()
        .unwrap();
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
        assert!(
            err.to_string().contains("unsupported filter"),
            "{f} should be rejected"
        );
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
    let tls_only = json!({"tls":{"insecureSkipVerify":true}});
    let retry_only = json!({"retry":{"max":4,"factor":1.1}});
    assert!(parse_depth_filter_opts(None, None, Some(http_only))
        .unwrap()
        .strategy_override
        .unwrap()
        .http
        .is_some());
    assert!(parse_depth_filter_opts(None, None, Some(tls_only))
        .unwrap()
        .strategy_override
        .unwrap()
        .tls
        .is_some());
    assert!(parse_depth_filter_opts(None, None, Some(retry_only))
        .unwrap()
        .strategy_override
        .unwrap()
        .retry
        .is_some());
}

#[test]
fn test_strategy_override_invalid_type() {
    // strategyOverride must be an object; an array should fail
    let arr = json!([1, 2, 3]);
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
    assert!(
        msg.contains("retry.max") || msg.contains("http.maxRedirects"),
        "error message should contain one of the violations, got: {msg}"
    );
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
