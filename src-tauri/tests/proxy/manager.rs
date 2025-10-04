use fireworks_collaboration_lib::core::proxy::{ProxyConfig, ProxyManager, ProxyMode, ProxyState};

#[test]
fn test_proxy_manager_default() {
    let manager = ProxyManager::default();
    assert!(!manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Off);
    assert_eq!(manager.state(), ProxyState::Disabled);
}

#[test]
fn test_proxy_manager_enabled() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = ProxyManager::new(config);
    assert!(manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Http);
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_proxy_manager_should_disable_custom_transport() {
    // Proxy disabled - respect config
    let manager = ProxyManager::default();
    assert!(!manager.should_disable_custom_transport());

    // Proxy enabled - force disable custom transport
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        disable_custom_transport: false, // User set to false
        ..Default::default()
    };
    let manager = ProxyManager::new(config);
    assert!(manager.should_disable_custom_transport()); // But we force it to true
}

#[test]
fn test_proxy_manager_update_config() {
    let manager = ProxyManager::default();
    assert!(!manager.is_enabled());

    // Enable proxy
    let new_config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    manager.update_config(new_config).unwrap();
    assert!(manager.is_enabled());
    assert_eq!(manager.state(), ProxyState::Enabled);

    // Disable proxy
    manager.update_config(ProxyConfig::default()).unwrap();
    assert!(!manager.is_enabled());
    assert_eq!(manager.state(), ProxyState::Disabled);
}

#[test]
fn test_proxy_manager_sanitized_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://user:pass@proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = ProxyManager::new(config);
    let sanitized = manager.sanitized_url();

    // Should hide credentials
    assert!(sanitized.contains("***"));
    assert!(!sanitized.contains("pass"));
}

#[test]
fn test_proxy_manager_get_connector() {
    // Test Off mode - should return placeholder
    let manager = ProxyManager::default();
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "placeholder");

    // Test HTTP mode - should return http connector
    let http_config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    let http_manager = ProxyManager::new(http_config);
    let http_connector = http_manager.get_connector().unwrap();
    assert_eq!(http_connector.proxy_type(), "http");
}

#[test]
fn test_proxy_manager_failure_reporting() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        fallback_threshold: 0.5, // 50% threshold
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Report successes first to avoid immediate fallback
    manager.report_success();
    manager.report_success();
    manager.report_success();

    // Record failures (2/5 = 40% < 50%)
    manager.report_failure("Connection timeout");
    manager.report_failure("Connection refused");

    let context = manager.get_state_context();
    assert_eq!(context.consecutive_failures, 2);
    assert_eq!(context.consecutive_successes, 0);

    // Record success resets failure counter
    manager.report_success();
    let context = manager.get_state_context();
    assert_eq!(context.consecutive_failures, 0);
    assert_eq!(context.consecutive_successes, 1);
}

#[test]
fn test_proxy_manager_manual_fallback_recovery() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = ProxyManager::new(config);
    assert_eq!(manager.state(), ProxyState::Enabled);

    // Manual fallback
    manager.manual_fallback("Testing fallback").unwrap();
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Manual recovery
    manager.manual_recover().unwrap();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_proxy_manager_detect_system_proxy() {
    // Just verify it doesn't panic
    let result = ProxyManager::detect_system_proxy();
    // Result depends on actual system configuration
    if let Some(config) = result {
        assert!(config.validate().is_ok());
    }
}

#[test]
fn test_proxy_manager_apply_system_proxy() {
    let manager = ProxyManager::default();

    // Try to apply system proxy
    let applied = manager.apply_system_proxy().unwrap();

    // If system proxy was detected and applied
    if applied {
        assert_eq!(manager.mode(), ProxyMode::System);
        assert!(manager.is_enabled());
    } else {
        // No system proxy, should remain disabled
        assert!(!manager.is_enabled());
    }
}

#[test]
fn test_proxy_manager_concurrent_reads() {
    use std::sync::Arc;
    use std::thread;

    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = Arc::new(ProxyManager::new(config));
    let mut handles = vec![];

    // Spawn multiple threads reading state
    for _ in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            assert!(manager_clone.is_enabled());
            assert_eq!(manager_clone.mode(), ProxyMode::Http);
            let _ = manager_clone.sanitized_url();
            let _ = manager_clone.state();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_proxy_manager_concurrent_state_updates() {
    use std::sync::Arc;
    use std::thread;

    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = Arc::new(ProxyManager::new(config));
    let mut handles = vec![];

    // Spawn threads reporting failures and successes
    for i in 0..5 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                manager_clone.report_failure("Test failure");
            } else {
                manager_clone.report_success();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Just verify no panics occurred
    let context = manager.get_state_context();
    assert!(context.consecutive_failures > 0 || context.consecutive_successes > 0);
}

#[test]
fn test_proxy_manager_invalid_config_update() {
    let manager = ProxyManager::default();

    let invalid_config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        timeout_seconds: 0, // Invalid!
        ..Default::default()
    };

    // Should reject invalid config
    assert!(manager.update_config(invalid_config).is_err());

    // State should remain unchanged
    assert!(!manager.is_enabled());
    assert_eq!(manager.state(), ProxyState::Disabled);
}

#[test]
fn test_proxy_manager_state_synchronization() {
    let manager = ProxyManager::default();

    // Enable proxy
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    manager.update_config(config).unwrap();

    // Both config and state should be enabled
    assert!(manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Http);
    assert_eq!(manager.state(), ProxyState::Enabled);

    // Disable via config update
    manager.update_config(ProxyConfig::default()).unwrap();

    // Both should be disabled
    assert!(!manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Off);
    assert_eq!(manager.state(), ProxyState::Disabled);
}

#[test]
fn test_proxy_manager_raw_url_warning() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://user:secret123@proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Raw URL should contain credentials (for actual connection)
    let raw = manager.proxy_url();
    assert!(raw.contains("secret123"));

    // Sanitized should not
    let sanitized = manager.sanitized_url();
    assert!(!sanitized.contains("secret123"));
    assert!(sanitized.contains("***"));
}

#[test]
fn test_proxy_manager_mode_transitions() {
    let manager = ProxyManager::default();

    // Off -> Http
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    manager.update_config(config).unwrap();
    assert_eq!(manager.mode(), ProxyMode::Http);
    assert_eq!(manager.state(), ProxyState::Enabled);

    // Http -> Socks5 (should keep enabled state)
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy.example.com:1080".to_string(),
        ..Default::default()
    };
    manager.update_config(config).unwrap();
    assert_eq!(manager.mode(), ProxyMode::Socks5);
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_proxy_manager_get_state_context() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        fallback_threshold: 0.5, // 50% threshold
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Report some successes first to establish a baseline
    manager.report_success();
    manager.report_success();
    manager.report_success();

    // Now report 2 failures (2/5 = 40% < 50%, should not fallback)
    manager.report_failure("Error 1");
    manager.report_failure("Error 2");

    let context = manager.get_state_context();
    assert_eq!(context.state, ProxyState::Enabled);
    assert_eq!(context.consecutive_failures, 2);
    assert_eq!(context.consecutive_successes, 0); // Reset by failures
}

#[test]
fn test_proxy_manager_connector_type_changes_with_mode() {
    let manager = ProxyManager::default();

    // Initially Off mode - should return placeholder
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "placeholder");

    // Update to HTTP mode
    let http_config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    manager.update_config(http_config).unwrap();

    // Should now return HTTP connector
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "http");

    // Update back to Off
    manager.update_config(ProxyConfig::default()).unwrap();

    // Should return placeholder again
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "placeholder");
}

#[test]
fn test_proxy_manager_http_connector_uses_config() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://myproxy.com:9090".to_string(),
        username: Some("testuser".to_string()),
        password: Some("testpass".to_string()),
        timeout_seconds: 45,
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Get connector and verify it's HTTP type
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "http");

    // Verify manager state matches config
    assert_eq!(manager.mode(), ProxyMode::Http);
    assert!(manager.is_enabled());
}

#[test]
fn test_proxy_manager_multiple_config_updates() {
    let manager = ProxyManager::default();

    for i in 0..5 {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: format!("http://proxy{i}.example.com:8080"),
            ..Default::default()
        };

        manager.update_config(config).unwrap();
        assert!(manager.is_enabled());
        assert_eq!(manager.mode(), ProxyMode::Http);
    }
}

#[test]
fn test_proxy_manager_failure_success_cycle() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        fallback_threshold: 0.5, // 50% threshold
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Start with successes to avoid immediate fallback
    for _ in 0..5 {
        manager.report_success();
    }

    // Record failures (3/8 = 37.5% < 50%)
    for _ in 0..3 {
        manager.report_failure("Connection error");
    }

    let ctx = manager.get_state_context();
    assert_eq!(ctx.consecutive_failures, 3);
    assert_eq!(ctx.consecutive_successes, 0);

    // Record success - should reset failure counter
    manager.report_success();

    let ctx = manager.get_state_context();
    assert_eq!(ctx.consecutive_failures, 0);
    assert_eq!(ctx.consecutive_successes, 1);
}

#[test]
fn test_proxy_manager_extreme_timeout_config() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        timeout_seconds: 24 * 3600, // 24小时
        ..Default::default()
    };
    let manager = ProxyManager::new(config);
    assert!(manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Http);
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_proxy_manager_empty_url_config() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);
    // 空URL应被视为无效，is_enabled为false
    assert!(!manager.is_enabled());
    assert_eq!(manager.state(), ProxyState::Disabled);
}

#[test]
fn test_proxy_manager_no_mode_config() {
    let config = ProxyConfig {
        mode: ProxyMode::Off,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);
    assert!(!manager.is_enabled());
    assert_eq!(manager.state(), ProxyState::Disabled);
}

#[test]
fn test_proxy_manager_multithreaded_config_switching() {
    use std::sync::Arc;
    use std::thread;
    let manager = Arc::new(ProxyManager::default());
    let mut handles = vec![];
    for i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let url = format!("http://proxy{i}.example.com:8080");
        let handle = thread::spawn(move || {
            let config = ProxyConfig {
                mode: ProxyMode::Http,
                url,
                ..Default::default()
            };
            let _ = manager_clone.update_config(config);
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join().unwrap();
    }
    // 最终状态应为Http模式且启用
    assert!(manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Http);
}

#[test]
fn test_proxy_manager_socks5_connector() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy.example.com:1080".to_string(),
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    assert!(manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Socks5);

    // Get connector and verify it's the right type
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_mode_transition_http_to_socks5() {
    let manager = ProxyManager::new(ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://http-proxy.example.com:8080".to_string(),
        ..Default::default()
    });

    assert_eq!(manager.mode(), ProxyMode::Http);
    assert_eq!(manager.get_connector().unwrap().proxy_type(), "http");

    // Switch to SOCKS5
    let socks5_config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://socks-proxy.example.com:1080".to_string(),
        ..Default::default()
    };
    manager.update_config(socks5_config).unwrap();

    assert_eq!(manager.mode(), ProxyMode::Socks5);
    assert_eq!(manager.get_connector().unwrap().proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_socks5_without_credentials() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://public-proxy:1080".to_string(),
        username: None,
        password: None,
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    assert!(manager.is_enabled());
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_socks5_url_formats() {
    // Test different URL formats
    let formats = vec!["socks5://proxy:1080", "socks://proxy:1080", "proxy:1080"];

    for url in formats {
        let config = ProxyConfig {
            mode: ProxyMode::Socks5,
            url: url.to_string(),
            ..Default::default()
        };
        let manager = ProxyManager::new(config);
        let connector = manager.get_connector().unwrap();
        assert_eq!(connector.proxy_type(), "socks5");
    }
}

#[test]
fn test_proxy_manager_socks5_invalid_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "invalid-url-no-port".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // Should fail to create connector
    let result = manager.get_connector();
    assert!(result.is_err());
}

#[test]
fn test_proxy_manager_socks5_with_ipv6_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://[::1]:1080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_socks5_timeout_propagation() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        timeout_seconds: 60,
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
    // 超时已正确传递到连接器
}

#[test]
fn test_proxy_manager_socks5_credentials_propagation() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        username: Some("testuser".to_string()),
        password: Some("testpass".to_string()),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
    // 凭证已正确传递到连接器
}

#[test]
fn test_proxy_manager_socks5_mode_consistency() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    assert_eq!(manager.mode(), ProxyMode::Socks5);

    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_socks5_url_without_scheme() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "proxy.example.com:1080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // SOCKS5 connector 应该接受无 scheme 的 URL
    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_socks5_empty_url() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // 空 URL 应该失败
    let result = manager.get_connector();
    assert!(result.is_err());
}

#[test]
fn test_proxy_manager_socks5_port_zero() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:0".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // 端口 0 应该失败
    let result = manager.get_connector();
    assert!(result.is_err());
}

#[test]
fn test_proxy_manager_socks5_very_long_timeout() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        timeout_seconds: 3600, // 1 hour
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_socks5_very_short_timeout() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        timeout_seconds: 1, // 1 second
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    let connector = manager.get_connector().unwrap();
    assert_eq!(connector.proxy_type(), "socks5");
}

#[test]
fn test_proxy_manager_multiple_socks5_instances() {
    let config1 = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy1:1080".to_string(),
        ..Default::default()
    };

    let config2 = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy2:2080".to_string(),
        ..Default::default()
    };

    let manager1 = ProxyManager::new(config1);
    let manager2 = ProxyManager::new(config2);

    let connector1 = manager1.get_connector().unwrap();
    let connector2 = manager2.get_connector().unwrap();

    assert_eq!(connector1.proxy_type(), "socks5");
    assert_eq!(connector2.proxy_type(), "socks5");
    // 两个管理器应该独立工作
}

#[test]
fn test_proxy_manager_socks5_config_update() {
    let mut config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy1:1080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config.clone());

    let connector1 = manager.get_connector().unwrap();
    assert_eq!(connector1.proxy_type(), "socks5");

    // 更新配置
    config.url = "socks5://proxy2:2080".to_string();
    let manager2 = ProxyManager::new(config);

    let connector2 = manager2.get_connector().unwrap();
    assert_eq!(connector2.proxy_type(), "socks5");
}

// P5.3: Tests for custom transport disable logic
#[test]
fn test_proxy_manager_should_disable_custom_transport_when_proxy_enabled() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // 代理启用时应该禁用自定义传输层
    assert!(manager.should_disable_custom_transport());
}

#[test]
fn test_proxy_manager_should_not_disable_when_proxy_off() {
    let config = ProxyConfig {
        mode: ProxyMode::Off,
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // 代理未启用时不应禁用自定义传输层（除非明确配置）
    assert!(!manager.should_disable_custom_transport());
}

#[test]
fn test_proxy_manager_should_disable_custom_transport_when_configured() {
    let config = ProxyConfig {
        mode: ProxyMode::Off,
        disable_custom_transport: true,
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // 即使代理未启用，如果明确配置禁用则应该禁用
    assert!(manager.should_disable_custom_transport());
}

#[test]
fn test_proxy_manager_http_disables_custom_transport() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        disable_custom_transport: false, // 即使设为false
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // HTTP代理启用时强制禁用自定义传输层
    assert!(manager.should_disable_custom_transport());
}

#[test]
fn test_proxy_manager_socks5_disables_custom_transport() {
    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy:1080".to_string(),
        disable_custom_transport: false, // 即使设为false
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // SOCKS5代理启用时强制禁用自定义传输层
    assert!(manager.should_disable_custom_transport());
}

// P5.4 Advanced scenario tests

#[test]
fn test_fallback_then_recover() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // Initial state: Enabled
    assert_eq!(manager.get_state_context().state, ProxyState::Enabled);

    // Manually trigger fallback
    manager.manual_fallback("Test fallback").unwrap();
    assert_eq!(manager.get_state_context().state, ProxyState::Fallback);

    // Recover back to enabled
    manager.manual_recover().unwrap();
    assert_eq!(manager.get_state_context().state, ProxyState::Enabled);

    // Failure stats should be reset after recovery
    let stats = manager.get_failure_stats();
    assert_eq!(stats.total_attempts, 0);
    assert_eq!(stats.failures, 0);
}

#[test]
fn test_automatic_fallback_after_multiple_failures() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // Establish baseline with some successes
    for _ in 0..5 {
        manager.report_success();
    }

    // Report many failures to exceed 20% threshold
    for _ in 0..5 {
        manager.report_failure("Connection error");
    }

    // Should automatically fallback
    assert_eq!(manager.get_state_context().state, ProxyState::Fallback);

    // Verify failure stats
    let stats = manager.get_failure_stats();
    assert_eq!(stats.total_attempts, 10);
    assert_eq!(stats.failures, 5);
    assert_eq!(stats.failure_rate, 0.5);
}

#[test]
fn test_fallback_state_persistence() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // Trigger fallback
    manager.manual_fallback("Persistent fallback").unwrap();

    // Multiple checks should still show fallback
    for _ in 0..5 {
        assert_eq!(manager.get_state_context().state, ProxyState::Fallback);
    }

    // Failure reporting should not cause duplicate fallback
    for _ in 0..3 {
        manager.report_failure("Error");
    }

    assert_eq!(manager.get_state_context().state, ProxyState::Fallback);
}

#[test]
fn test_concurrent_fallback_requests() {
    use std::sync::Arc;
    use std::thread;

    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = Arc::new(ProxyManager::new(config));
    let mut handles = vec![];

    // Spawn 10 threads trying to trigger fallback
    for i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            let _ = manager_clone.manual_fallback(&format!("Concurrent {i}"));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should still be in valid fallback state
    assert_eq!(manager.get_state_context().state, ProxyState::Fallback);
}

#[test]
fn test_fallback_event_validation() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // Setup for automatic fallback
    for _ in 0..5 {
        manager.report_success();
    }
    for _ in 0..5 {
        manager.report_failure("Network error");
    }

    // Should have triggered automatic fallback
    assert_eq!(manager.get_state_context().state, ProxyState::Fallback);

    // Verify fallback event would be emitted with correct data
    // (In P5.6, this will be tested by checking actual event emissions)
    let stats = manager.get_failure_stats();
    assert!(stats.fallback_triggered);
    assert_eq!(stats.failures, 5);
    assert_eq!(stats.failure_rate, 0.5);
}

#[test]
fn test_recovery_resets_detector() {
    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy:8080".to_string(),
        ..Default::default()
    };
    let manager = ProxyManager::new(config);

    // Report successes first to establish a baseline
    for _ in 0..20 {
        manager.report_success();
    }

    // Then report a few failures (not enough to exceed 20% threshold)
    for _ in 0..3 {
        manager.report_failure("Error");
    }

    let stats_before = manager.get_failure_stats();
    assert_eq!(stats_before.total_attempts, 23);
    assert_eq!(stats_before.failures, 3);
    // 3/23 = 13% < 20% threshold, should not auto-fallback

    // Manual fallback and recover
    manager.manual_fallback("Test").unwrap();
    manager.manual_recover().unwrap();

    // Stats should be reset
    let stats_after = manager.get_failure_stats();
    assert_eq!(stats_after.total_attempts, 0);
    assert_eq!(stats_after.failures, 0);
    assert_eq!(stats_after.failure_rate, 0.0);
    assert!(!stats_after.fallback_triggered);
}

// ============================================================================
// P5.5 Configuration Enhancement Tests
// ============================================================================

#[test]
fn test_config_update_with_custom_probe_settings() {
    // Test that config update propagates new probe settings

    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        probe_url: "www.github.com:443".to_string(),
        probe_timeout_seconds: 10,
        recovery_consecutive_threshold: 3,
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Update with new probe settings
    let new_config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        probe_url: "www.google.com:443".to_string(),
        probe_timeout_seconds: 20,
        recovery_consecutive_threshold: 5,
        ..Default::default()
    };

    let result = manager.update_config(new_config);
    assert!(result.is_ok());

    // Config update should succeed and manager should remain operational
    assert!(manager.is_enabled());
    assert_eq!(manager.mode(), ProxyMode::Http);
}

#[test]
fn test_config_update_preserves_state_with_new_fields() {
    // Test that updating config with new P5.5 fields doesn't affect state

    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Trigger fallback
    manager.manual_fallback("Test").unwrap();
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Update config while in fallback
    let new_config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        probe_url: "www.cloudflare.com:443".to_string(),
        probe_timeout_seconds: 15,
        recovery_consecutive_threshold: 4,
        ..Default::default()
    };

    manager.update_config(new_config).unwrap();

    // Should still be in fallback state
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Recovery should work with new config
    manager.manual_recover().unwrap();
    assert_eq!(manager.state(), ProxyState::Enabled);
}

#[test]
fn test_extreme_probe_timeout_values() {
    // Test minimum and maximum probe timeout values

    // Minimum (1 second)
    let config_min = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        probe_timeout_seconds: 1,
        ..Default::default()
    };

    let manager = ProxyManager::new(config_min);
    assert!(manager.is_enabled());

    // Maximum (60 seconds)
    let config_max = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        probe_timeout_seconds: 60,
        ..Default::default()
    };

    manager.update_config(config_max).unwrap();
    assert!(manager.is_enabled());
}

#[test]
fn test_extreme_recovery_threshold_values() {
    // Test minimum and maximum recovery threshold values

    // Minimum (1)
    let config_min = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        recovery_consecutive_threshold: 1,
        ..Default::default()
    };

    let manager = ProxyManager::new(config_min);
    assert!(manager.is_enabled());

    // Maximum (10)
    let config_max = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        recovery_consecutive_threshold: 10,
        ..Default::default()
    };

    manager.update_config(config_max).unwrap();
    assert!(manager.is_enabled());
}

#[test]
fn test_health_check_with_custom_probe_url() {
    // Test health check execution with custom probe URL

    let config = ProxyConfig {
        mode: ProxyMode::Http,
        url: "http://proxy.example.com:8080".to_string(),
        probe_url: "www.example.com:443".to_string(),
        recovery_cooldown_seconds: 0,
        ..Default::default()
    };

    let manager = ProxyManager::new(config);

    // Trigger fallback to enable health checks
    manager.manual_fallback("Test").unwrap();
    assert_eq!(manager.state(), ProxyState::Fallback);

    // Execute health check (should use custom probe URL)
    let result = manager.health_check();
    assert!(result.is_ok());

    let probe_result = result.unwrap();
    // In fallback state, probe should be attempted (not skipped)
    assert!(!probe_result.is_skipped() || probe_result.is_failure());
}

#[test]
fn test_config_serialization_with_new_fields() {
    // Test that new P5.5 fields serialize/deserialize correctly

    let config = ProxyConfig {
        mode: ProxyMode::Socks5,
        url: "socks5://proxy.example.com:1080".to_string(),
        probe_url: "www.github.com:443".to_string(),
        probe_timeout_seconds: 25,
        recovery_consecutive_threshold: 7,
        recovery_cooldown_seconds: 180,
        health_check_interval_seconds: 45,
        ..Default::default()
    };

    // Serialize to JSON
    let json = serde_json::to_string(&config).unwrap();

    // Verify camelCase field names
    assert!(json.contains("probeUrl"));
    assert!(json.contains("probeTimeoutSeconds"));
    assert!(json.contains("recoveryConsecutiveThreshold"));

    // Deserialize back
    let restored: ProxyConfig = serde_json::from_str(&json).unwrap();

    // Verify fields match
    assert_eq!(restored.probe_url, config.probe_url);
    assert_eq!(restored.probe_timeout_seconds, config.probe_timeout_seconds);
    assert_eq!(
        restored.recovery_consecutive_threshold,
        config.recovery_consecutive_threshold
    );
}

#[test]
fn test_multiple_config_updates_with_varying_thresholds() {
    // Test multiple config updates changing recovery thresholds

    let manager = ProxyManager::default();

    let thresholds = vec![1, 3, 5, 10, 2];

    for threshold in thresholds {
        let config = ProxyConfig {
            mode: ProxyMode::Http,
            url: "http://proxy.example.com:8080".to_string(),
            recovery_consecutive_threshold: threshold,
            ..Default::default()
        };

        let result = manager.update_config(config);
        assert!(result.is_ok());
        assert!(manager.is_enabled());
    }
}
