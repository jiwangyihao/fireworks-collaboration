use fireworks_collaboration_lib::app::commands::proxy::{
    detect_system_proxy, force_proxy_fallback_logic, force_proxy_recovery_logic, get_system_proxy,
};
use fireworks_collaboration_lib::app::types::SharedConfig;
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::proxy::ProxyMode;
use std::sync::{Arc, Mutex};

fn create_test_config() -> SharedConfig {
    let mut config = AppConfig::default();
    config.proxy.mode = ProxyMode::Http;
    config.proxy.url = "http://localhost:8080".into();
    Arc::new(Mutex::new(config))
}

fn create_disabled_config() -> SharedConfig {
    Arc::new(Mutex::new(AppConfig::default()))
}

#[tokio::test]
async fn test_detect_system_proxy_command() {
    let result = detect_system_proxy().await;
    assert!(result.is_ok());
    let proxy_info = result.unwrap();
    // Verify structure matches expected type
    println!("Detected system proxy: {:?}", proxy_info);
    // Fields should be accessible
    let _ = proxy_info.url;
    let _ = proxy_info.proxy_type;
}

#[tokio::test]
async fn test_force_proxy_fallback_command_wrapper() {
    // Test success case with enabled proxy
    let config = create_test_config();
    let result = force_proxy_fallback_logic(Some("test integration fallback".into()), config).await;
    assert!(
        result.is_ok(),
        "Fallback should succeed when proxy is enabled"
    );
    assert_eq!(result.unwrap(), true);
}

#[tokio::test]
async fn test_force_proxy_fallback_fails_when_disabled() {
    // Test failure case with disabled proxy
    let config = create_disabled_config();
    let result = force_proxy_fallback_logic(Some("test failure".into()), config).await;

    assert!(
        result.is_err(),
        "Fallback should fail when proxy is disabled"
    );
    let err = result.err().unwrap();
    // Use flexible check as error message wording might vary
    assert!(
        err.to_lowercase().contains("failed") || err.to_lowercase().contains("invalid"),
        "Unexpected error format: {}",
        err
    );
}

#[tokio::test]
async fn test_force_proxy_recovery_command_wrapper() {
    let config = create_test_config();
    let result = force_proxy_recovery_logic(config).await;

    // CURRENT BEHAVIOR: Returns Error because ProxyManager is ephemeral and starts in Enabled state (reset).
    // Recovery requires Fallback state.
    // See ticket/issue relating to "ProxyManager persistence".
    assert!(result.is_err(), "Expected error due to ephemeral state");
    let err = result.err().unwrap();
    assert!(
        err.contains("Cannot start recovery"),
        "Unexpected error: {}",
        err
    );
}

#[tokio::test]
async fn test_detect_system_proxy_force_disable() {
    // Set environment variable to force disable detection
    // Note: modifying environment variables in async tests can be racey if run in parallel with other env-dependent tests.
    // Ensure no other tests rely on FWC_LOCAL_PROXY_FORCE_DISABLE concurrently.
    let key = "FWC_PROXY_FORCE_DISABLE";
    std::env::set_var(key, "1");

    let result = detect_system_proxy().await;

    // Cleanup
    std::env::remove_var(key);

    assert!(result.is_ok());
    let sys_proxy = result.unwrap();
    assert!(
        sys_proxy.url.is_none(),
        "URL should be None when proxy detection is disabled"
    );
    assert!(
        sys_proxy.proxy_type.is_none(),
        "Proxy type should be None when disabled"
    );
}

#[tokio::test]
async fn test_force_proxy_fallback_default_reason() {
    // Test fallback with None reason (should use default)
    let config = create_test_config();
    let result = force_proxy_fallback_logic(None, config).await;
    assert!(
        result.is_ok(),
        "Fallback with default reason should succeed"
    );
    assert_eq!(result.unwrap(), true);
}

#[test]
fn test_get_system_proxy_legacy_command() {
    // Legacy command is synchronous and returns simple struct
    let result = get_system_proxy();
    assert!(result.is_ok());
    let proxy = result.unwrap();
    // Currently implementation returns default (empty) checks
    // We just verify it returns successfully
    println!("Legacy system proxy: {:?}", proxy);
}
