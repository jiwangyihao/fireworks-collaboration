use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::http::client::HttpClient;
use fireworks_collaboration_lib::core::http::types::HttpRequestInput;
use hyper::header::{HeaderMap, HOST};
use std::collections::HashMap;

#[tokio::test]
async fn test_reject_non_https() {
    let client = HttpClient::new(AppConfig::default());
    let input = HttpRequestInput {
        url: "http://example.com/".into(),
        method: "GET".into(),
        headers: HashMap::new(),
        body_base64: None,
        timeout_ms: 100,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };
    let err = client.send(input).await.err().expect("should fail");
    let msg = format!("{}", err);
    assert!(msg.contains("only https"));
}

#[tokio::test]
async fn test_invalid_base64_early() {
    let client = HttpClient::new(AppConfig::default());
    let input = HttpRequestInput {
        url: "https://example.com/".into(),
        method: "POST".into(),
        headers: HashMap::new(),
        body_base64: Some("***not-base64***".into()),
        timeout_ms: 100,
        force_real_sni: false,
        follow_redirects: false,
        max_redirects: 0,
    };
    let err = client.send(input).await.err().expect("should fail");
    let msg = format!("{}", err);
    assert!(msg.contains("decode bodyBase64"));
}

#[test]
fn test_compute_sni_host_fake_and_real() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["baidu.com".into()];
    let client = HttpClient::new(cfg.clone());
    let (sni, used_fake) = client.compute_sni_host(false, "github.com");
    assert_eq!(sni, "baidu.com");
    assert!(used_fake);
    let (sni2, used_fake2) = client.compute_sni_host(true, "github.com");
    assert_eq!(sni2, "github.com");
    assert!(!used_fake2);
}

#[test]
fn test_upsert_host_header_overrides() {
    let client = HttpClient::new(AppConfig::default());
    let mut h = HeaderMap::new();
    client.upsert_host_header(&mut h, "example.com");
    assert_eq!(h.get(HOST).unwrap(), "example.com");
    // override
    client.upsert_host_header(&mut h, "another.test");
    assert_eq!(h.get(HOST).unwrap(), "another.test");
}

#[test]
fn test_should_warn_large_body_boundary() {
    let mut cfg = AppConfig::default();
    cfg.http.large_body_warn_bytes = 10;
    let client = HttpClient::new(cfg);
    assert!(!client.should_warn_large_body(10)); // equal -> no warn
    assert!(client.should_warn_large_body(11)); // greater -> warn
}
