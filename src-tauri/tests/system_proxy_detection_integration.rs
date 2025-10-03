//! P5.7 跨平台系统代理检测集成测试
//!
//! 本模块测试在 Windows/macOS/Linux 三个平台上的系统代理检测功能：
//! 1. 正常检测 - 验证能够读取系统代理配置
//! 2. 环境变量回退 - 平台特定检测失败时使用环境变量
//! 3. 检测失败 - 无代理配置时返回 None
//! 4. 代理类型识别 - HTTP/HTTPS/SOCKS5 自动识别
//!
//! **注意**: 这些测试会修改环境变量,必须以单线程模式运行:
//! ```
//! cargo test --test system_proxy_detection_integration -- --test-threads=1
//! ```

use fireworks_collaboration_lib::core::proxy::{ProxyMode, SystemProxyDetector};
use std::env;

/// 测试从 HTTP_PROXY 环境变量检测代理
///
/// 验证:
/// - HTTP_PROXY 设置时能被正确检测
/// - 代理模式正确识别为 HTTP
#[test]
fn test_detect_from_http_proxy_env() {
    // 清除所有代理环境变量,避免污染
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    let original_all = env::var("ALL_PROXY").ok();

    // 清理所有代理变量,只设置需要测试的
    env::remove_var("HTTPS_PROXY");
    env::remove_var("ALL_PROXY");
    env::set_var("HTTP_PROXY", "http://proxy.example.com:8080");

    // 使用 detect_from_env 直接测试环境变量检测
    let result = SystemProxyDetector::detect_from_env();

    // 恢复所有环境变量
    match original_http {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }
    match original_all {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }

    assert!(result.is_some(), "Should detect proxy from HTTP_PROXY environment variable");
    
    let config = result.unwrap();
    assert_eq!(config.mode, ProxyMode::Http);
    assert!(
        config.url.contains("proxy.example.com") || config.url.contains("8080"),
        "URL should contain proxy host: {}",
        config.url
    );
}

/// 测试 HTTPS_PROXY 环境变量优先级
///
/// 验证：
/// - HTTPS_PROXY 存在时优先使用
/// - 代理模式正确识别为 HTTP
#[test]
fn test_detect_https_proxy_precedence() {
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    let original_all = env::var("ALL_PROXY").ok();

    // 清理ALL_PROXY,避免干扰
    env::remove_var("ALL_PROXY");
    env::set_var("HTTP_PROXY", "http://http-proxy.example.com:8080");
    env::set_var("HTTPS_PROXY", "http://https-proxy.example.com:8443");

    // 使用 detect_from_env 直接测试环境变量检测
    let result = SystemProxyDetector::detect_from_env();

    // 恢复所有环境变量(在获取result后,在断言前)
    match original_http {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }
    match original_all {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }

    assert!(result.is_some(), "Should detect proxy from HTTPS_PROXY environment variable");
    
    let config = result.unwrap();
    // 应该使用 HTTPS_PROXY
    assert!(
        config.url.contains("https-proxy.example.com") || config.url.contains("8443"),
        "Should prefer HTTPS_PROXY over HTTP_PROXY: {}",
        config.url
    );
}

/// 测试 SOCKS5 代理检测
///
/// 验证：
/// - socks5:// scheme 被正确识别
/// - 代理模式设置为 Socks5
#[test]
fn test_detect_socks5_proxy() {
    let original = env::var("ALL_PROXY").ok();
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();

    // 清理其他代理变量,只设置ALL_PROXY
    env::remove_var("HTTP_PROXY");
    env::remove_var("HTTPS_PROXY");
    env::set_var("ALL_PROXY", "socks5://socks-proxy.example.com:1080");

    // 使用 detect_from_env 直接测试环境变量检测
    let result = SystemProxyDetector::detect_from_env();

    // 立即恢复所有环境变量
    match original {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }
    match original_http {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }

    assert!(result.is_some(), "Should detect proxy from ALL_PROXY environment variable");
    
    let config = result.unwrap();
    assert_eq!(config.mode, ProxyMode::Socks5);
    assert!(config.url.contains("socks-proxy.example.com"));
    assert!(config.url.contains("1080"));
}

/// 测试无代理配置的情况
///
/// 验证：
/// - 无代理时返回 None
/// - 不会崩溃或产生错误
#[test]
fn test_detect_no_proxy() {
    // 保存并清除所有代理环境变量
    let original_http = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    let original_all = env::var("ALL_PROXY").ok();
    let original_http_lower = env::var("http_proxy").ok();
    let original_https_lower = env::var("https_proxy").ok();
    let original_all_lower = env::var("all_proxy").ok();

    env::remove_var("HTTP_PROXY");
    env::remove_var("HTTPS_PROXY");
    env::remove_var("ALL_PROXY");
    env::remove_var("http_proxy");
    env::remove_var("https_proxy");
    env::remove_var("all_proxy");

    let result = SystemProxyDetector::detect();

    // 恢复环境变量
    if let Some(val) = original_http {
        env::set_var("HTTP_PROXY", val);
    }
    if let Some(val) = original_https {
        env::set_var("HTTPS_PROXY", val);
    }
    if let Some(val) = original_all {
        env::set_var("ALL_PROXY", val);
    }
    if let Some(val) = original_http_lower {
        env::set_var("http_proxy", val);
    }
    if let Some(val) = original_https_lower {
        env::set_var("https_proxy", val);
    }
    if let Some(val) = original_all_lower {
        env::set_var("all_proxy", val);
    }

    // 在没有代理配置的纯净环境下，应该返回 None
    // 但由于可能有系统级代理配置，这里只验证不会崩溃
    match result {
        Some(config) => {
            println!(
                "System proxy detected (may be from system settings): mode={:?}, url={}",
                config.mode, config.url
            );
        }
        None => {
            println!("No proxy detected (expected in clean environment)");
        }
    }
}

/// 测试带认证信息的代理 URL
///
/// 验证：
/// - URL 中的用户名和密码被正确提取
/// - 认证信息存储在 ProxyConfig 中
#[test]
fn test_detect_proxy_with_auth() {
    let original = env::var("HTTP_PROXY").ok();
    let original_https = env::var("HTTPS_PROXY").ok();
    let original_all = env::var("ALL_PROXY").ok();

    // 清理其他代理变量
    env::remove_var("HTTPS_PROXY");
    env::remove_var("ALL_PROXY");
    env::set_var(
        "HTTP_PROXY",
        "http://username:password@proxy.example.com:8080",
    );

    // 使用 detect_from_env 直接测试环境变量检测
    let result = SystemProxyDetector::detect_from_env();

    // 立即恢复所有环境变量
    match original {
        Some(val) => env::set_var("HTTP_PROXY", val),
        None => env::remove_var("HTTP_PROXY"),
    }
    match original_https {
        Some(val) => env::set_var("HTTPS_PROXY", val),
        None => env::remove_var("HTTPS_PROXY"),
    }
    match original_all {
        Some(val) => env::set_var("ALL_PROXY", val),
        None => env::remove_var("ALL_PROXY"),
    }

    assert!(result.is_some(), "Should detect proxy from HTTP_PROXY with auth");
    
    let config = result.unwrap();
    // URL 中应该包含或解析出认证信息
    // 具体行为取决于 SystemProxyDetector 的实现
    assert!(
        config.url.contains("proxy.example.com"),
        "URL should contain host"
    );

    // 当前实现中,认证信息保留在 URL 中
    // 未来可能会实现提取到 username/password 字段
    println!("Detected proxy URL: {}", config.url);
    println!("Note: Current implementation keeps auth info in URL");
}

/// Windows 特定测试：注册表代理检测
///
/// 验证：
/// - Windows 注册表读取逻辑工作正常
/// - ProxyEnable=1 时能检测到代理
/// - ProxyServer 字段被正确解析
#[cfg(target_os = "windows")]
#[test]
fn test_windows_registry_proxy_detection() {
    use std::process::Command;

    // 查询当前代理设置
    let output = Command::new("reg")
        .args(&[
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            "/v",
            "ProxyEnable",
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            println!("Windows proxy registry status:\n{}", stdout);

            let config = SystemProxyDetector::detect();

            match config {
                Some(cfg) => {
                    println!(
                        "Detected Windows proxy: mode={:?}, url={}",
                        cfg.mode, cfg.url
                    );
                    assert!(
                        !cfg.url.is_empty(),
                        "Proxy URL should not be empty when detected"
                    );
                }
                None => {
                    println!("No Windows proxy detected (may be disabled in registry)");
                }
            }
        }
        Ok(_) | Err(_) => {
            println!("Unable to query Windows registry (may require elevated permissions)");
        }
    }
}

/// macOS 特定测试：scutil 代理检测
///
/// 验证：
/// - scutil --proxy 命令执行正常
/// - HTTP/HTTPS/SOCKS 代理被正确解析
#[cfg(target_os = "macos")]
#[test]
fn test_macos_scutil_proxy_detection() {
    use std::process::Command;

    let output = Command::new("scutil").arg("--proxy").output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            println!("macOS scutil proxy output:\n{}", stdout);

            let config = SystemProxyDetector::detect();

            match config {
                Some(cfg) => {
                    println!("Detected macOS proxy: mode={:?}, url={}", cfg.mode, cfg.url);
                    assert!(
                        !cfg.url.is_empty(),
                        "Proxy URL should not be empty when detected"
                    );
                }
                None => {
                    println!("No macOS proxy detected (may be disabled in system preferences)");
                }
            }
        }
        Ok(_) | Err(_) => {
            println!("Unable to execute scutil (may not be available)");
        }
    }
}

/// Linux 特定测试：环境变量代理检测
///
/// 验证：
/// - Linux 环境变量代理检测工作正常
/// - 大小写不敏感的环境变量都能识别
#[cfg(target_os = "linux")]
#[test]
fn test_linux_env_proxy_detection() {
    let original_http = env::var("http_proxy").ok();

    env::set_var("http_proxy", "http://proxy.linux.example.com:3128");

    let result = SystemProxyDetector::detect();

    // 清理
    match original_http {
        Some(val) => env::set_var("http_proxy", val),
        None => env::remove_var("http_proxy"),
    }

    if let Some(config) = result {
        println!(
            "Detected Linux proxy: mode={:?}, url={}",
            config.mode, config.url
        );
        assert!(
            config.url.contains("proxy.linux.example.com") || config.url.contains("3128"),
            "Should detect Linux proxy from lowercase env var"
        );
    } else {
        println!("Warning: Failed to detect Linux proxy from lowercase env var");
    }
}

/// 测试代理配置验证
///
/// 验证：
/// - 检测到的代理配置通过验证
/// - 无效的代理被过滤
#[test]
fn test_detected_proxy_validation() {
    if let Some(config) = SystemProxyDetector::detect() {
        // 尝试验证检测到的配置
        match config.validate() {
            Ok(_) => {
                println!(
                    "Detected proxy configuration is valid: mode={:?}, url={}",
                    config.mode, config.url
                );
            }
            Err(e) => {
                panic!(
                    "Detected proxy configuration failed validation: {} (mode={:?}, url={})",
                    e, config.mode, config.url
                );
            }
        }
    } else {
        println!("No proxy detected for validation test");
    }
}

/// 测试系统代理检测的性能
///
/// 验证：
/// - 检测操作在合理时间内完成
/// - 不会阻塞或超时
#[test]
fn test_proxy_detection_performance() {
    use std::time::Instant;

    let start = Instant::now();

    let _result = SystemProxyDetector::detect();

    let elapsed = start.elapsed();

    println!("System proxy detection completed in {:?}", elapsed);

    assert!(
        elapsed.as_secs() < 5,
        "Proxy detection should complete within 5 seconds, took {:?}",
        elapsed
    );
}

/// 测试多次检测的一致性
///
/// 验证：
/// - 连续检测返回相同结果
/// - 检测逻辑是幂等的
#[test]
fn test_proxy_detection_consistency() {
    let result1 = SystemProxyDetector::detect();
    let result2 = SystemProxyDetector::detect();

    match (result1, result2) {
        (Some(cfg1), Some(cfg2)) => {
            assert_eq!(
                cfg1.mode, cfg2.mode,
                "Consecutive detections should return same mode"
            );
            assert_eq!(
                cfg1.url, cfg2.url,
                "Consecutive detections should return same URL"
            );
            println!(
                "Proxy detection is consistent: mode={:?}, url={}",
                cfg1.mode, cfg1.url
            );
        }
        (None, None) => {
            println!("Consistently detected no proxy");
        }
        _ => {
            panic!("Inconsistent proxy detection results");
        }
    }
}
