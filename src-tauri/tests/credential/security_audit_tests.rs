//! 凭证安全审计集成测试
//!
//! 测试凭证管理的完整安全审计流程，包括：
//! - 日志脱敏（Display/Debug trait）
//! - 审计日志记录（标准模式 vs 审计模式）
//! - 哈希摘要验证
//! - 内存清零验证（间接通过行为测试）

use fireworks_collaboration_lib::core::credential::{
    audit::{AuditLogger, OperationType},
    Credential,
};
use std::time::{Duration, SystemTime};

#[test]
fn test_credential_display_masking() {
    // 测试凭证的 Display trait 脱敏
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "super_secret_password_12345".to_string(),
    );

    let display_output = format!("{cred}");
    
    // 断言：显示输出不应包含完整密码
    assert!(!display_output.contains("super_secret_password_12345"));
    
    // 断言：应包含脱敏标记
    assert!(display_output.contains("****"));
    
    // 断言：应包含基本信息
    assert!(display_output.contains("github.com"));
    assert!(display_output.contains("testuser"));
}

#[test]
fn test_credential_debug_masking() {
    // 测试凭证的 Debug trait 脱敏
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "super_secret_password_12345".to_string(),
    );

    let debug_output = format!("{cred:?}");
    
    // 断言：调试输出不应包含完整密码
    assert!(!debug_output.contains("super_secret_password_12345"));
    
    // 断言：应包含脱敏标记
    assert!(debug_output.contains("****"));
    
    // 断言：应包含结构体名称
    assert!(debug_output.contains("Credential"));
}

#[test]
fn test_credential_serialization_excludes_password() {
    // 测试凭证序列化时密码被跳过
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "super_secret_password".to_string(),
    );

    let json = serde_json::to_string(&cred).unwrap();
    
    // 断言：序列化后的 JSON 不应包含密码
    assert!(!json.contains("super_secret_password"));
    assert!(!json.contains("password_or_token"));
    
    // 断言：应包含其他字段
    assert!(json.contains("github.com"));
    assert!(json.contains("testuser"));
}

#[test]
fn test_audit_logger_standard_mode() {
    // 测试标准模式（不记录哈希）
    let logger = AuditLogger::new(false); // 标准模式
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    
    let event = &events[0];
    assert_eq!(event.operation, OperationType::Add);
    assert_eq!(event.host, "github.com");
    assert_eq!(event.username, "user1");
    assert!(event.success);
    assert!(event.credential_hash.is_none()); // 标准模式下不记录哈希
}

#[test]
fn test_audit_logger_audit_mode() {
    // 测试审计模式（记录哈希）
    let logger = AuditLogger::new(true); // 审计模式
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    
    let event = &events[0];
    assert!(event.credential_hash.is_some()); // 审计模式下记录哈希
    
    let hash = event.credential_hash.as_ref().unwrap();
    assert_eq!(hash.len(), 64); // SHA-256 哈希长度为 64 个十六进制字符
    assert!(!hash.contains("password123")); // 哈希不应包含明文密码
}

#[test]
fn test_audit_logger_hash_consistency() {
    // 测试相同凭证产生相同哈希
    let logger = AuditLogger::new(true);
    
    // 记录两次相同的凭证操作
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );
    
    logger.log_operation(
        OperationType::Get,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 2);
    
    // 相同的凭证应该产生相同的哈希
    let hash1 = events[0].credential_hash.as_ref().unwrap();
    let hash2 = events[1].credential_hash.as_ref().unwrap();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_audit_logger_hash_uniqueness() {
    // 测试不同凭证产生不同哈希
    let logger = AuditLogger::new(true);
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password1"),
        true,
        None,
    );
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password2"), // 不同的密码
        true,
        None,
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 2);
    
    // 不同的密码应该产生不同的哈希
    let hash1 = events[0].credential_hash.as_ref().unwrap();
    let hash2 = events[1].credential_hash.as_ref().unwrap();
    assert_ne!(hash1, hash2);
}

#[test]
fn test_audit_logger_failure_logging() {
    // 测试失败操作的审计日志
    let logger = AuditLogger::new(true);
    
    logger.log_operation(
        OperationType::Get,
        "github.com",
        "user1",
        None,
        false, // 操作失败
        Some("凭证不存在".to_string()),
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    
    let event = &events[0];
    assert!(!event.success);
    assert!(event.error.is_some());
    assert_eq!(event.error.as_ref().unwrap(), "凭证不存在");
    assert!(event.credential_hash.is_none()); // 失败时没有密码，不记录哈希
}

#[test]
fn test_audit_logger_export_json() {
    // 测试审计日志导出为 JSON
    let logger = AuditLogger::new(true);
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );
    
    let json = logger.export_to_json().unwrap();
    
    // 断言：JSON 应包含必要字段
    assert!(json.contains("\"operation\""));
    assert!(json.contains("\"host\""));
    assert!(json.contains("github.com"));
    assert!(json.contains("\"credentialHash\""));
    
    // 断言：JSON 不应包含明文密码
    assert!(!json.contains("password123"));
}

#[test]
fn test_audit_logger_clear() {
    // 测试清除审计日志
    let logger = AuditLogger::new(false);
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        None,
        true,
        None,
    );
    
    assert_eq!(logger.event_count(), 1);
    
    logger.clear();
    assert_eq!(logger.event_count(), 0);
}

#[test]
fn test_credential_masked_password_variations() {
    // 测试不同长度密码的脱敏效果
    
    // 短密码（≤8 字符）
    let short_cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "short".to_string(),
    );
    assert_eq!(short_cred.masked_password(), "***");
    
    // 中等长度密码（>8 字符）
    let medium_cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "medium_password_123".to_string(),
    );
    let masked = medium_cred.masked_password();
    assert!(masked.starts_with("medi"));
    assert!(masked.ends_with("_123"));
    assert!(masked.contains("****"));
    
    // 超长密码
    let long_cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "a".repeat(100),
    );
    let masked_long = long_cred.masked_password();
    assert!(masked_long.starts_with("aaaa"));
    assert!(masked_long.ends_with("aaaa"));
    assert!(masked_long.contains("****"));
}

#[test]
fn test_audit_logger_multiple_operations() {
    // 测试记录多种操作类型
    let logger = AuditLogger::new(true);
    
    logger.log_operation(OperationType::Add, "host1", "user1", Some("pass1"), true, None);
    logger.log_operation(OperationType::Get, "host1", "user1", Some("pass1"), true, None);
    logger.log_operation(OperationType::Update, "host1", "user1", Some("pass2"), true, None);
    logger.log_operation(OperationType::Remove, "host1", "user1", None, true, None);
    logger.log_operation(OperationType::List, "host1", "", None, true, None);
    
    let events = logger.get_events();
    assert_eq!(events.len(), 5);
    
    // 验证操作类型
    assert_eq!(events[0].operation, OperationType::Add);
    assert_eq!(events[1].operation, OperationType::Get);
    assert_eq!(events[2].operation, OperationType::Update);
    assert_eq!(events[3].operation, OperationType::Remove);
    assert_eq!(events[4].operation, OperationType::List);
}

#[test]
fn test_credential_expiry_does_not_leak() {
    // 测试过期凭证的显示也不会泄露密码
    let past_time = SystemTime::now() - Duration::from_secs(3600);
    let expired_cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "user".to_string(),
        "expired_password_123".to_string(),
        past_time,
    );
    
    assert!(expired_cred.is_expired());
    
    // 即使过期，Display 输出也应该脱敏
    let display = format!("{expired_cred}");
    assert!(!display.contains("expired_password_123"));
    assert!(display.contains("****"));
    
    // Debug 输出也应该脱敏
    let debug = format!("{expired_cred:?}");
    assert!(!debug.contains("expired_password_123"));
    assert!(debug.contains("****"));
}

#[test]
fn test_audit_logger_concurrent_safety() {
    // 测试审计日志的并发安全性
    use std::thread;
    
    let logger = AuditLogger::new(true);
    let logger_clone = logger.clone();
    
    let handle = thread::spawn(move || {
        for i in 0..10 {
            logger_clone.log_operation(
                OperationType::Add,
                "github.com",
                &format!("user{i}"),
                Some("password"),
                true,
                None,
            );
        }
    });
    
    for i in 0..10 {
        logger.log_operation(
            OperationType::Get,
            "github.com",
            &format!("user{i}"),
            Some("password"),
            true,
            None,
        );
    }
    
    handle.join().unwrap();
    assert_eq!(logger.event_count(), 20);
}

#[test]
fn test_operation_type_serialization() {
    // 测试操作类型的序列化
    assert_eq!(
        serde_json::to_string(&OperationType::Add).unwrap(),
        "\"add\""
    );
    assert_eq!(
        serde_json::to_string(&OperationType::Remove).unwrap(),
        "\"remove\""
    );
    assert_eq!(
        serde_json::to_string(&OperationType::Validate).unwrap(),
        "\"validate\""
    );
}

#[test]
fn test_credential_identifier_no_password_leak() {
    // 测试 identifier() 方法不会泄露密码
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_password".to_string(),
    );
    
    let identifier = cred.identifier();
    assert_eq!(identifier, "testuser@github.com");
    assert!(!identifier.contains("secret_password"));
}

#[test]
fn test_audit_logger_no_password_in_export() {
    // 综合测试：确保导出的审计日志绝不包含明文密码
    let logger = AuditLogger::new(true);
    
    // 记录多个操作
    for i in 0..10 {
        logger.log_operation(
            OperationType::Add,
            &format!("host{i}"),
            &format!("user{i}"),
            Some(&format!("password{i}")),
            true,
            None,
        );
    }
    
    let json = logger.export_to_json().unwrap();
    
    // 断言：导出的 JSON 不应包含任何明文密码
    for i in 0..10 {
        assert!(!json.contains(&format!("password{i}")));
    }
    
    // 断言：应包含哈希字段
    assert!(json.contains("\"credentialHash\""));
}
