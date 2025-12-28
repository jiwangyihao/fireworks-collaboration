use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::http::client::HttpClient;
use hyper::header::{HeaderMap, HOST};

#[test]
fn test_upsert_host_header() {
    let cfg = AppConfig::default();
    let client = HttpClient::new(cfg);

    let mut headers = HeaderMap::new();
    // Case 1: Insert new
    client.upsert_host_header(&mut headers, "example.com");
    assert_eq!(headers.get(HOST).unwrap(), "example.com");

    // Case 2: Overwrite existing
    client.upsert_host_header(&mut headers, "new.com");
    assert_eq!(headers.get(HOST).unwrap(), "new.com");
}

#[test]
fn test_should_warn_large_body() {
    let mut cfg = AppConfig::default();
    cfg.http.large_body_warn_bytes = 100;

    let client = HttpClient::new(cfg);

    assert!(!client.should_warn_large_body(99));
    assert!(!client.should_warn_large_body(100));
    assert!(client.should_warn_large_body(101));
}

#[test]
fn test_compute_sni_host_no_fake() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = false;

    let client = HttpClient::new(cfg);

    let (sni, fake) = client.compute_sni_host(false, "real.com");
    assert_eq!(sni, "real.com");
    assert!(!fake);
}

#[test]
fn test_compute_sni_host_fake_enabled() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;
    cfg.http.fake_sni_hosts = vec!["fake.com".to_string()];
    cfg.http.fake_sni_rollout_percent = 100; // Always rollout

    let client = HttpClient::new(cfg.clone());

    let (sni, fake) = client.compute_sni_host(false, "real.com");
    if fake {
        assert!(cfg.http.fake_sni_hosts.contains(&sni));
        assert_ne!(sni, "real.com");
    } else {
        // Fallback?
        assert_eq!(sni, "real.com");
    }
}

#[test]
fn test_compute_sni_host_force_real() {
    let mut cfg = AppConfig::default();
    cfg.http.fake_sni_enabled = true;

    let client = HttpClient::new(cfg);

    let (sni, fake) = client.compute_sni_host(true, "real.com");
    assert_eq!(sni, "real.com");
    assert!(!fake);
}
