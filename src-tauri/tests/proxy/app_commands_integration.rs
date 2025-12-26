use fireworks_collaboration_lib::app::commands::proxy::{
    detect_system_proxy, force_proxy_fallback_logic, force_proxy_recovery_logic,
};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::app::types::SharedConfig;
use std::sync::{Arc, Mutex};

fn create_test_config() -> SharedConfig {
    Arc::new(Mutex::new(AppConfig::default()))
}

#[tokio::test]
async fn test_detect_system_proxy_command() {
    let result = detect_system_proxy().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_force_proxy_fallback_command_wrapper() {
    let config = create_test_config();
    let result = force_proxy_fallback_logic(Some("test integration fallback".into()), config).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[tokio::test]
async fn test_force_proxy_recovery_command_wrapper() {
    let config = create_test_config();
    let result = force_proxy_recovery_logic(config).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}
