//! Configuration model unit tests
//!
//! Tests for ObservabilityLayer, RetryCfg, AppConfig defaults.

use fireworks_collaboration_lib::core::config::model::{
    AppConfig, ObservabilityConfig, ObservabilityLayer, RetryCfg,
};

// ============ ObservabilityLayer Tests ============

#[test]
fn test_observability_layer_ordering() {
    assert!(ObservabilityLayer::Basic < ObservabilityLayer::Aggregate);
    assert!(ObservabilityLayer::Aggregate < ObservabilityLayer::Export);
    assert!(ObservabilityLayer::Export < ObservabilityLayer::Ui);
    assert!(ObservabilityLayer::Ui < ObservabilityLayer::Alerts);
    assert!(ObservabilityLayer::Alerts < ObservabilityLayer::Optimize);
}

#[test]
fn test_observability_layer_as_u8() {
    assert_eq!(ObservabilityLayer::Basic.as_u8(), 0);
    assert_eq!(ObservabilityLayer::Aggregate.as_u8(), 1);
    assert_eq!(ObservabilityLayer::Export.as_u8(), 2);
    assert_eq!(ObservabilityLayer::Ui.as_u8(), 3);
    assert_eq!(ObservabilityLayer::Alerts.as_u8(), 4);
    assert_eq!(ObservabilityLayer::Optimize.as_u8(), 5);
}

#[test]
fn test_observability_layer_as_str() {
    assert_eq!(ObservabilityLayer::Basic.as_str(), "basic");
    assert_eq!(ObservabilityLayer::Aggregate.as_str(), "aggregate");
    assert_eq!(ObservabilityLayer::Export.as_str(), "export");
    assert_eq!(ObservabilityLayer::Ui.as_str(), "ui");
    assert_eq!(ObservabilityLayer::Alerts.as_str(), "alerts");
    assert_eq!(ObservabilityLayer::Optimize.as_str(), "optimize");
}

#[test]
fn test_observability_layer_from_u8() {
    assert_eq!(ObservabilityLayer::from_u8(0), ObservabilityLayer::Basic);
    assert_eq!(
        ObservabilityLayer::from_u8(1),
        ObservabilityLayer::Aggregate
    );
    assert_eq!(ObservabilityLayer::from_u8(2), ObservabilityLayer::Export);
    assert_eq!(ObservabilityLayer::from_u8(3), ObservabilityLayer::Ui);
    assert_eq!(ObservabilityLayer::from_u8(4), ObservabilityLayer::Alerts);
    assert_eq!(ObservabilityLayer::from_u8(5), ObservabilityLayer::Optimize);
    // Invalid values should default to Optimize
    assert_eq!(
        ObservabilityLayer::from_u8(100),
        ObservabilityLayer::Optimize
    );
}

#[test]
fn test_observability_layer_next_lower() {
    assert_eq!(ObservabilityLayer::Basic.next_lower(), None);
    assert_eq!(
        ObservabilityLayer::Aggregate.next_lower(),
        Some(ObservabilityLayer::Basic)
    );
    assert_eq!(
        ObservabilityLayer::Export.next_lower(),
        Some(ObservabilityLayer::Aggregate)
    );
    assert_eq!(
        ObservabilityLayer::Optimize.next_lower(),
        Some(ObservabilityLayer::Alerts)
    );
}

#[test]
fn test_observability_layer_next_higher() {
    assert_eq!(
        ObservabilityLayer::Basic.next_higher(),
        Some(ObservabilityLayer::Aggregate)
    );
    assert_eq!(
        ObservabilityLayer::Alerts.next_higher(),
        Some(ObservabilityLayer::Optimize)
    );
    assert_eq!(ObservabilityLayer::Optimize.next_higher(), None);
}

// ============ RetryCfg Tests ============

#[test]
fn test_retry_cfg_default() {
    let cfg = RetryCfg::default();
    assert_eq!(cfg.max, 6);
    assert_eq!(cfg.base_ms, 300);
    assert_eq!(cfg.factor, 1.5);
}

// ============ AppConfig Tests ============

#[test]
fn test_app_config_default() {
    let config = AppConfig::default();
    // HTTP defaults
    assert!(config.http.follow_redirects);
    assert_eq!(config.http.max_redirects, 5);
    // Logging defaults
    assert_eq!(config.logging.log_level, "info");
}

#[test]
fn test_app_config_proxy_default() {
    let config = AppConfig::default();
    assert!(!config.proxy.is_enabled());
}

#[test]
fn test_app_config_workspace_default() {
    let config = AppConfig::default();
    assert!(!config.workspace.enabled);
}

// ============ ObservabilityConfig Tests ============

#[test]
fn test_observability_config_default() {
    let config = ObservabilityConfig::default();
    assert_eq!(config.layer, ObservabilityLayer::Optimize);
    assert!(config.enabled);
}
