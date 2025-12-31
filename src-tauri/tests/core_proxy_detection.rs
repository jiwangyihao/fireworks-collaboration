use fireworks_collaboration_lib::core::proxy::{ProxyMode, SystemProxyDetector};
use std::env;
use std::sync::Mutex;

// Mutex to serialize environment variable tests to prevent race conditions
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn run_with_env<F>(f: F)
where
    F: FnOnce(),
{
    let _guard = ENV_LOCK.lock().unwrap();
    // Clear relevant env vars before start
    let vars = [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ];
    for var in &vars {
        env::remove_var(var);
    }

    f();

    // Cleanup after test
    for var in &vars {
        env::remove_var(var);
    }
}

#[test]
fn test_parse_proxy_url_schemes() {
    // 1. HTTP
    let config = SystemProxyDetector::parse_proxy_url("http://proxy.example.com:8080").unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "http://proxy.example.com:8080");

    // 2. HTTPS
    let config = SystemProxyDetector::parse_proxy_url("https://secure.proxy:8443").unwrap();
    assert_eq!(config.mode, ProxyMode::Http); // Note: Current impl treats https:// as Http mode, usually, or is there a separate Https mode?
                                              // Let's check system_detector.rs logic:
                                              // if url.starts_with("http://") || url.starts_with("https://") { ProxyMode::Http }
                                              // So yes, it maps HTTPS URL to ProxyMode::Http (which usually handles CONNECT)
    assert_eq!(config.url, "https://secure.proxy:8443");

    // 3. SOCKS5
    let config = SystemProxyDetector::parse_proxy_url("socks5://127.0.0.1:1080").unwrap();
    assert_eq!(config.mode, ProxyMode::Socks5);
    assert_eq!(config.url, "socks5://127.0.0.1:1080");

    // 4. Default Scheme (implicit HTTP)
    let config = SystemProxyDetector::parse_proxy_url("proxy.local:3128").unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert_eq!(config.url, "http://proxy.local:3128"); // It prepends http://
}

#[test]
fn test_parse_proxy_url_invalid() {
    // 1. Empty
    assert!(SystemProxyDetector::parse_proxy_url("").is_none());

    // 2. Invalid Scheme
    assert!(SystemProxyDetector::parse_proxy_url("ftp://proxy").is_none());

    // 3. Missing Host
    assert!(SystemProxyDetector::parse_proxy_url("http://").is_none());

    // 4. Empty Host part
    assert!(SystemProxyDetector::parse_proxy_url("socks5://:1080").is_none());
}

#[test]
fn test_env_detection_precedence() {
    run_with_env(|| {
        // 1. No vars -> None
        assert!(SystemProxyDetector::detect_from_env().is_none());

        // 2. ALL_PROXY only
        env::set_var("ALL_PROXY", "socks5://all.proxy:1080");
        let config = SystemProxyDetector::detect_from_env().unwrap();
        assert_eq!(config.url, "socks5://all.proxy:1080");

        // 3. HTTP_PROXY overrides ALL_PROXY?
        // Logic checks keys in order: HTTPS_PROXY, https_proxy, HTTP_PROXY, http_proxy, ALL_PROXY.
        // So HTTP_PROXY comes before ALL_PROXY.
        env::set_var("HTTP_PROXY", "http://http.proxy:8080");
        let config = SystemProxyDetector::detect_from_env().unwrap();
        assert_eq!(config.url, "http://http.proxy:8080");

        // 4. HTTPS_PROXY overrides HTTP_PROXY?
        // Logic check HTTPS_PROXY first.
        env::set_var("HTTPS_PROXY", "http://https.proxy:8443");
        let config = SystemProxyDetector::detect_from_env().unwrap();
        assert_eq!(config.url, "http://https.proxy:8443");
    });
}

#[test]
fn test_env_detection_empty_values() {
    run_with_env(|| {
        // Set HTTPS_PROXY to empty string (should be ignored)
        env::set_var("HTTPS_PROXY", "   ");
        // Set HTTP_PROXY to valid
        env::set_var("HTTP_PROXY", "http://fallback:8080");

        let config = SystemProxyDetector::detect_from_env().unwrap();
        // Should hit HTTP_PROXY
        assert_eq!(config.url, "http://fallback:8080");
    });
}
