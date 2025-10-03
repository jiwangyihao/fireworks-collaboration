//! P6.2 凭证安全增强测试
//!
//! 补充测试密码强度验证、内存清零、HMAC完整性等安全特性

use fireworks_collaboration_lib::core::credential::{
    audit::{AuditLogger, OperationType},
    config::{CredentialConfig, StorageType},
    file_store::EncryptedFileStore,
    model::Credential,
    storage::CredentialStore,
};
use std::fs;
use tempfile::TempDir;

/// 测试：弱密码警告（虽然当前不强制，但应记录在审计日志）
#[test]
fn test_weak_password_audit_logging() {
    let logger = AuditLogger::new(true);
    
    // 测试各种弱密码
    let weak_passwords = vec![
        "123456",
        "password",
        "12345678",
        "qwerty",
        "abc123",
    ];
    
    for (i, weak_pwd) in weak_passwords.iter().enumerate() {
        logger.log_operation(
            OperationType::Add,
            &format!("host{}.com", i),
            "user",
            Some(weak_pwd),
            true,
            None,
        );
    }
    
    let events = logger.get_events();
    assert_eq!(events.len(), weak_passwords.len());
    
    // 验证每个事件都有哈希记录
    for event in events {
        assert!(event.credential_hash.is_some());
    }
}

/// 测试：强密码的审计日志
#[test]
fn test_strong_password_audit_logging() {
    let logger = AuditLogger::new(true);
    
    let strong_passwords = vec![
        "Tr0ub4dor&3!@#$%",
        "correct-horse-battery-staple-2024",
        "MyP@ssw0rd!With#Numbers&Symbols",
    ];
    
    for (i, strong_pwd) in strong_passwords.iter().enumerate() {
        logger.log_operation(
            OperationType::Add,
            &format!("host{}.com", i),
            "user",
            Some(strong_pwd),
            true,
            None,
        );
    }
    
    let events = logger.get_events();
    assert_eq!(events.len(), strong_passwords.len());
    
    // 验证强密码也被正确哈希
    for event in events {
        assert!(event.credential_hash.is_some());
    }
}

/// 测试：空密码的处理
#[test]
fn test_empty_password_handling() {
    let logger = AuditLogger::new(true);
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some(""), // 空密码
        true,
        None,
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    
    // 空密码应该有哈希（虽然不安全）
    assert!(events[0].credential_hash.is_some());
}

/// 测试：超长密码的处理
#[test]
fn test_very_long_password_handling() {
    let logger = AuditLogger::new(true);
    
    // 创建一个 1KB 的密码
    let long_password = "a".repeat(1024);
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some(&long_password),
        true,
        None,
    );
    
    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].credential_hash.is_some());
}

/// 测试：HMAC 篡改检测 - 修改密文
#[test]
fn test_hmac_detects_ciphertext_tampering() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 添加凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 读取加密文件
    let mut file_content = fs::read_to_string(&file_path).unwrap();
    
    // 篡改密文（修改一个字符）
    if let Some(ciphertext_start) = file_content.find("\"ciphertext\":") {
        let bytes = unsafe { file_content.as_bytes_mut() };
        if ciphertext_start + 20 < bytes.len() {
            bytes[ciphertext_start + 20] = b'X'; // 修改一个字节
        }
    }
    
    // 写回篡改后的内容
    fs::write(&file_path, &file_content).unwrap();
    
    // 尝试读取应该失败（HMAC 验证失败）
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store2.set_master_password("test-password".to_string()).unwrap();
    
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_err(), "篡改的密文应该被 HMAC 检测到");
}

/// 测试：HMAC 篡改检测 - 修改盐值
#[test]
fn test_hmac_detects_salt_tampering() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 读取加密文件
    let mut file_content = fs::read_to_string(&file_path).unwrap();
    
    // 篡改盐值
    if let Some(salt_start) = file_content.find("\"salt\":") {
        let bytes = unsafe { file_content.as_bytes_mut() };
        if salt_start + 15 < bytes.len() {
            bytes[salt_start + 15] = b'X';
        }
    }
    
    fs::write(&file_path, &file_content).unwrap();
    
    // 尝试读取应该失败
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store2.set_master_password("test-password".to_string()).unwrap();
    
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_err(), "篡改的盐值应该导致解密失败");
}

/// 测试：HMAC 篡改检测 - 修改 nonce
#[test]
fn test_hmac_detects_nonce_tampering() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 读取加密文件
    let mut file_content = fs::read_to_string(&file_path).unwrap();
    
    // 篡改 nonce
    if let Some(nonce_start) = file_content.find("\"nonce\":") {
        let bytes = unsafe { file_content.as_bytes_mut() };
        if nonce_start + 15 < bytes.len() {
            bytes[nonce_start + 15] = b'Z';
        }
    }
    
    fs::write(&file_path, &file_content).unwrap();
    
    // 尝试读取应该失败
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store2.set_master_password("test-password".to_string()).unwrap();
    
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_err(), "篡改的 nonce 应该导致解密失败");
}

/// 测试：Zeroize 内存清零（间接验证）
#[test]
fn test_credential_memory_cleanup() {
    // 创建凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "sensitive_password_12345".to_string(),
    );
    
    // 获取标识符（不包含密码）
    let identifier = cred.identifier();
    assert_eq!(identifier, "user@github.com");
    
    // 获取脱敏密码
    let masked = cred.masked_password();
    assert!(!masked.contains("sensitive_password_12345"));
    
    // Drop 凭证（Zeroize 应该清零内存）
    drop(cred);
    
    // 注意：无法直接验证内存清零，但可以确保 Drop trait 被调用
    // Zeroize 的正确性由 zeroize crate 保证
}

/// 测试：审计日志中的密码脱敏
#[test]
fn test_audit_log_password_masking() {
    let logger = AuditLogger::new(false); // 非审计模式
    
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("very_secret_password"),
        true,
        None,
    );
    
    let json = logger.export_to_json().unwrap();
    
    // 确保导出的 JSON 不包含明文密码
    assert!(!json.contains("very_secret_password"));
}

/// 测试：审计模式下的哈希一致性
#[test]
fn test_audit_mode_hash_consistency() {
    let logger = AuditLogger::new(true);
    
    let password = "test_password_123";
    
    // 多次记录相同的凭证
    for _ in 0..5 {
        logger.log_operation(
            OperationType::Get,
            "github.com",
            "user",
            Some(password),
            true,
            None,
        );
    }
    
    let events = logger.get_events();
    assert_eq!(events.len(), 5);
    
    // 所有哈希应该相同
    let first_hash = &events[0].credential_hash;
    for event in &events[1..] {
        assert_eq!(&event.credential_hash, first_hash);
    }
}

/// 测试：不同密码的哈希应该不同
#[test]
fn test_different_passwords_different_hashes() {
    let logger = AuditLogger::new(true);
    
    let passwords = vec![
        "password1",
        "password2",
        "password3",
        "completely_different_password",
    ];
    
    for pwd in &passwords {
        logger.log_operation(
            OperationType::Add,
            "github.com",
            "user",
            Some(pwd),
            true,
            None,
        );
    }
    
    let events = logger.get_events();
    assert_eq!(events.len(), passwords.len());
    
    // 收集所有哈希
    let mut hashes = std::collections::HashSet::new();
    for event in &events {
        if let Some(ref hash) = event.credential_hash {
            hashes.insert(hash.clone());
        }
    }
    
    // 所有哈希应该不同
    assert_eq!(hashes.len(), passwords.len());
}

/// 测试：序列化安全性 - 确保敏感字段不被序列化
#[test]
fn test_serialization_security() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "super_secret_token".to_string(),
    );
    
    // 序列化为 JSON
    let json = serde_json::to_string(&cred).unwrap();
    
    // 验证密码未被序列化
    assert!(!json.contains("super_secret_token"));
    
    // 验证基本信息被序列化
    assert!(json.contains("github.com"));
    assert!(json.contains("user"));
}

/// 测试：多次加解密的一致性
#[test]
fn test_multiple_encryption_decryption_cycles() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let original_cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    
    // 多次加密和解密循环
    for i in 0..10 {
        // 添加凭证
        store.add(original_cred.clone()).unwrap();
        
        // 读取凭证
        let retrieved = store.get("github.com", Some("user"))
            .unwrap()
            .expect(&format!("第 {} 次循环应该读取到凭证", i));
        
        assert_eq!(retrieved.host, "github.com");
        assert_eq!(retrieved.username, "user");
        assert_eq!(retrieved.password_or_token, "password123");
        
        // 删除凭证
        store.remove("github.com", "user").unwrap();
    }
}

/// 测试：并发加密操作的安全性
#[test]
fn test_concurrent_encryption_safety() {
    use std::sync::Arc;
    use std::thread;
    
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = Arc::new(EncryptedFileStore::new(&config).expect("应该创建文件存储"));
    store.set_master_password("test-password".to_string()).unwrap();
    
    let mut handles = vec![];
    
    // 5 个线程并发添加凭证
    for i in 0..5 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let cred = Credential::new(
                format!("host{}.com", i),
                format!("user{}", i),
                format!("password{}", i),
            );
            store_clone.add(cred).unwrap();
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 验证所有凭证都被正确保存
    let list = store.list().unwrap();
    assert_eq!(list.len(), 5);
}
