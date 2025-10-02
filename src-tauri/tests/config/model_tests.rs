use fireworks_collaboration_lib::core::config::model::{
    default_auto_disable_cooldown_sec, default_auto_disable_threshold_pct, AppConfig,
};

#[test]
fn test_serialize_camel_case_keys() {
    let cfg = AppConfig::default();
    let s = serde_json::to_string(&cfg).unwrap();
    // 关键字段以 camelCase 出现
    assert!(s.contains("\"fakeSniEnabled\""));
    assert!(s.contains("\"followRedirects\""));
    assert!(s.contains("\"maxRedirects\""));
    assert!(s.contains("\"largeBodyWarnBytes\""));
    assert!(s.contains("\"autoDisableFakeThresholdPct\""));
    assert!(s.contains("\"autoDisableFakeCooldownSec\""));
    assert!(s.contains("\"sanWhitelist\""));
    assert!(s.contains("\"authHeaderMasked\""));
    assert!(s.contains("\"logLevel\""));
    assert!(s.contains("\"retry\""));
    assert!(s.contains("\"baseMs\""));
    assert!(s.contains("\"factor\""));
    assert!(s.contains("\"jitter\""));
    assert!(s.contains("\"partialFilterSupported\""));
    assert!(s.contains("\"realHostVerifyEnabled\""));
    assert!(s.contains("\"metricsEnabled\""));
    assert!(s.contains("\"certFpLogEnabled\""));
    assert!(s.contains("\"certFpMaxBytes\""));
    assert!(s.contains("\"spkiPins\""));
    assert!(s.contains("\"ipPool\""));
    assert!(s.contains("\"proxy\""));
}

#[test]
fn test_deserialize_with_defaults() {
    // 只提供部分字段，其他应回退默认
    let json = r#"{
      "http": { "fakeSniEnabled": false },
      "tls": {},
                "logging": { "logLevel": "debug" },
                "retry": { "max": 2, "baseMs": 200, "factor": 2.0, "jitter": false }
    }"#;
    let cfg: AppConfig = serde_json::from_str(json).unwrap();
    // 提供的值覆盖
    assert!(!cfg.http.fake_sni_enabled);
    assert_eq!(cfg.logging.log_level, "debug");
    assert_eq!(cfg.retry.max, 2);
    assert_eq!(cfg.retry.base_ms, 200);
    assert!(!cfg.retry.jitter);
    // 未提供的保持默认
    assert!(cfg.http.follow_redirects);
    assert_eq!(cfg.http.max_redirects, 5);
    assert!(cfg
        .tls
        .san_whitelist
        .iter()
        .any(|d| d.ends_with("github.com")));
    assert_eq!(
        cfg.http.auto_disable_fake_threshold_pct,
        default_auto_disable_threshold_pct()
    );
    assert_eq!(
        cfg.http.auto_disable_fake_cooldown_sec,
        default_auto_disable_cooldown_sec()
    );
    assert!(
        cfg.tls.real_host_verify_enabled,
        "realHostVerifyEnabled default true"
    );
    assert!(cfg.tls.metrics_enabled, "metricsEnabled default true");
    assert!(cfg.tls.cert_fp_log_enabled, "certFpLogEnabled default true");
    assert_eq!(cfg.tls.cert_fp_max_bytes, 5 * 1024 * 1024);
    // P3.4: spkiPins default empty
    assert!(cfg.tls.spki_pins.is_empty());
    assert!(!cfg.ip_pool.enabled, "ipPool defaults to disabled");
    // P5.0: proxy defaults to off mode
    assert!(!cfg.proxy.is_enabled(), "proxy defaults to disabled");
}
