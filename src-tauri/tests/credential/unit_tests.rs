//! 凭证模块单元测试
//!
//! 从源代码文件迁移的单元测试，保持原有测试逻辑不变

use fireworks_collaboration_lib::core::credential::{
    audit::{AuditEvent, AuditLogger, OperationType},
    config::{CredentialConfig, StorageType},
    factory::CredentialStoreFactory,
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::time::{Duration, SystemTime};

// ========== Audit 模块测试 ==========

#[test]
fn test_audit_logger_creation() {
    let logger = AuditLogger::new(true);
    assert!(logger.is_audit_mode());
    assert_eq!(logger.event_count(), 0);
}

#[test]
fn test_log_operation_without_audit_mode() {
    let logger = AuditLogger::new(false);
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password123"),
        true,
        None,
    );

    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].credential_hash.is_none());
}

#[test]
fn test_log_operation_with_audit_mode() {
    let logger = AuditLogger::new(true);
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password123"),
        true,
        None,
    );

    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].credential_hash.is_some());
}

#[test]
fn test_credential_hash_consistency() {
    let logger = AuditLogger::new(true);

    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password123"),
        true,
        None,
    );
    logger.log_operation(
        OperationType::Get,
        "github.com",
        "user",
        Some("password123"),
        true,
        None,
    );

    let events = logger.get_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].credential_hash, events[1].credential_hash);
}

#[test]
fn test_credential_hash_different_passwords() {
    let logger = AuditLogger::new(true);

    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password1"),
        true,
        None,
    );
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password2"),
        true,
        None,
    );

    let events = logger.get_events();
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].credential_hash, events[1].credential_hash);
}

#[test]
fn test_log_operation_with_error() {
    let logger = AuditLogger::new(false);
    logger.log_operation(
        OperationType::Get,
        "github.com",
        "user",
        None,
        false,
        Some("凭证不存在".to_string()),
    );

    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    assert!(!events[0].success);
    assert!(events[0].error.is_some());
    assert_eq!(events[0].error.as_ref().unwrap(), "凭证不存在");
}

#[test]
fn test_clear_events() {
    let logger = AuditLogger::new(false);
    logger.log_operation(OperationType::Add, "github.com", "user", None, true, None);

    assert_eq!(logger.event_count(), 1);
    logger.clear();
    assert_eq!(logger.event_count(), 0);
}

#[test]
fn test_export_to_json() {
    let logger = AuditLogger::new(true);
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password"),
        true,
        None,
    );

    let json = logger.export_to_json().unwrap();
    assert!(json.contains("\"operation\""));
    assert!(json.contains("\"host\""));
    assert!(json.contains("github.com"));
    assert!(json.contains("\"credentialHash\""));
    assert!(!json.contains("password"));
}

#[test]
fn test_operation_type_display() {
    assert_eq!(format!("{}", OperationType::Add), "add");
    assert_eq!(format!("{}", OperationType::Get), "get");
    assert_eq!(format!("{}", OperationType::Update), "update");
    assert_eq!(format!("{}", OperationType::Remove), "remove");
}

#[test]
fn test_audit_event_serialization() {
    let event = AuditEvent {
        operation: OperationType::Add,
        host: "github.com".to_string(),
        username: "user".to_string(),
        timestamp: SystemTime::now(),
        success: true,
        error: None,
        credential_hash: Some("abc123".to_string()),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(event.operation, deserialized.operation);
    assert_eq!(event.host, deserialized.host);
    assert_eq!(event.username, deserialized.username);
}

#[test]
fn test_logger_clone() {
    let logger1 = AuditLogger::new(true);
    logger1.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password"),
        true,
        None,
    );

    let logger2 = logger1.clone();
    assert_eq!(logger2.event_count(), 1);
    assert!(logger2.is_audit_mode());
}

#[test]
fn test_concurrent_logging() {
    use std::thread;

    let logger = AuditLogger::new(false);
    let logger_clone = logger.clone();

    let handle = thread::spawn(move || {
        for i in 0..10 {
            logger_clone.log_operation(
                OperationType::Add,
                "github.com",
                &format!("user{i}"),
                None,
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
            None,
            true,
            None,
        );
    }

    handle.join().unwrap();
    assert_eq!(logger.event_count(), 20);
}

#[test]
fn test_no_password_no_hash() {
    let logger = AuditLogger::new(true);
    logger.log_operation(OperationType::List, "github.com", "user", None, true, None);

    let events = logger.get_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].credential_hash.is_none());
}

#[test]
fn test_hash_salt_uniqueness() {
    let logger1 = AuditLogger::new(true);
    let logger2 = AuditLogger::new(true);

    logger1.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password"),
        true,
        None,
    );

    std::thread::sleep(std::time::Duration::from_millis(1));

    logger2.log_operation(
        OperationType::Add,
        "github.com",
        "user",
        Some("password"),
        true,
        None,
    );

    let events1 = logger1.get_events();
    let events2 = logger2.get_events();

    assert_ne!(events1[0].credential_hash, events2[0].credential_hash);
}

// ========== Model 模块测试 ==========

#[test]
fn test_display_format() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token_123456".to_string(),
    );

    let display = format!("{cred}");
    assert!(display.contains("github.com"));
    assert!(display.contains("testuser"));
    assert!(!display.contains("secret_token_123456"));
    assert!(display.contains("****"));
}

#[test]
fn test_debug_format() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token_123456".to_string(),
    );

    let debug = format!("{cred:?}");
    assert!(debug.contains("github.com"));
    assert!(debug.contains("testuser"));
    assert!(!debug.contains("secret_token_123456"));
    assert!(debug.contains("****"));
}

#[test]
fn test_credential_new() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token".to_string(),
    );

    assert_eq!(cred.host, "github.com");
    assert_eq!(cred.username, "testuser");
    assert_eq!(cred.password_or_token, "secret_token");
    assert!(cred.expires_at.is_none());
    assert!(cred.last_used_at.is_none());
}

#[test]
fn test_credential_with_expiry() {
    let expires_at = SystemTime::now() + Duration::from_secs(3600);
    let cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token".to_string(),
        expires_at,
    );

    assert!(cred.expires_at.is_some());
    assert!(!cred.is_expired());
}

#[test]
fn test_credential_is_expired() {
    let past_time = SystemTime::now() - Duration::from_secs(3600);
    let cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token".to_string(),
        past_time,
    );

    assert!(cred.is_expired());
}

#[test]
fn test_credential_update_last_used() {
    let mut cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token".to_string(),
    );

    assert!(cred.last_used_at.is_none());
    cred.update_last_used();
    assert!(cred.last_used_at.is_some());
}

#[test]
fn test_credential_identifier() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token".to_string(),
    );

    assert_eq!(cred.identifier(), "testuser@github.com");
}

#[test]
fn test_credential_masked_password() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "ghp_1234567890abcdef".to_string(),
    );

    let masked = cred.masked_password();
    assert!(masked.contains("****"));
    assert!(!masked.contains("1234567890"));
}

#[test]
fn test_credential_masked_password_short() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "short".to_string(),
    );

    assert_eq!(cred.masked_password(), "***");
}

#[test]
fn test_credential_serialization() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "secret_token".to_string(),
    );

    let json = serde_json::to_string(&cred).unwrap();
    assert!(!json.contains("secret_token"));
    assert!(json.contains("github.com"));
    assert!(json.contains("testuser"));
}

#[test]
fn test_credential_with_empty_strings() {
    let cred = Credential::new("".to_string(), "".to_string(), "".to_string());
    assert_eq!(cred.host, "");
    assert_eq!(cred.username, "");
    assert_eq!(cred.masked_password(), "***");
}

#[test]
fn test_credential_with_unicode_characters() {
    let cred = Credential::new(
        "github.com".to_string(),
        "用户名".to_string(),
        "ghp_password_123456_with_unicode".to_string(),
    );
    assert_eq!(cred.username, "用户名");
    assert!(cred.masked_password().contains("****"));

    let cred2 = Credential::new(
        "github.com".to_string(),
        "user🔐".to_string(),
        "simple_password".to_string(),
    );
    assert_eq!(cred2.username, "user🔐");
}

#[test]
fn test_credential_with_very_long_password() {
    let long_password = "a".repeat(10 * 1024);
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        long_password.clone(),
    );
    assert_eq!(cred.password_or_token.len(), 10 * 1024);

    let masked = cred.masked_password();
    assert!(masked.starts_with("aaaa"));
    assert!(masked.ends_with("aaaa"));
    assert!(masked.contains("****"));
}

#[test]
fn test_credential_with_special_characters() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user@domain.com".to_string(),
        "p@ssw0rd!#$%".to_string(),
    );
    assert_eq!(cred.username, "user@domain.com");
    assert!(cred.masked_password().contains("****"));
}

#[test]
fn test_credential_expiry_edge_cases() {
    let epoch = SystemTime::UNIX_EPOCH;
    let cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "user".to_string(),
        "token".to_string(),
        epoch,
    );
    assert!(cred.is_expired());

    let far_future = SystemTime::now() + Duration::from_secs(100 * 365 * 24 * 60 * 60);
    let cred2 = Credential::new_with_expiry(
        "github.com".to_string(),
        "user".to_string(),
        "token".to_string(),
        far_future,
    );
    assert!(!cred2.is_expired());
}

// ========== Config 模块测试 ==========

#[test]
fn test_default_config() {
    let config = CredentialConfig::default();
    assert_eq!(config.storage, StorageType::System);
    assert!(config.default_ttl_seconds.is_some());
    assert!(!config.debug_logging);
    assert!(!config.audit_mode);
}

#[test]
fn test_config_builder() {
    let config = CredentialConfig::new()
        .with_storage(StorageType::Memory)
        .with_ttl(Some(3600))
        .with_debug_logging(true)
        .with_audit_mode(true);

    assert_eq!(config.storage, StorageType::Memory);
    assert_eq!(config.default_ttl_seconds, Some(3600));
    assert!(config.debug_logging);
    assert!(config.audit_mode);
}

#[test]
fn test_config_validation_file_storage_without_path() {
    let config = CredentialConfig::new().with_storage(StorageType::File);
    assert!(config.validate().is_err());
}

#[test]
fn test_config_validation_file_storage_with_path() {
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path("/tmp/credentials.enc".to_string());
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_zero_ttl() {
    let config = CredentialConfig::new().with_ttl(Some(0));
    assert!(config.validate().is_err());
}

#[test]
fn test_config_serialization() {
    let config = CredentialConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: CredentialConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.storage, deserialized.storage);
    assert_eq!(config.default_ttl_seconds, deserialized.default_ttl_seconds);
}

#[test]
fn test_storage_type_serialization() {
    assert_eq!(
        serde_json::to_string(&StorageType::System).unwrap(),
        "\"system\""
    );
    assert_eq!(
        serde_json::to_string(&StorageType::File).unwrap(),
        "\"file\""
    );
    assert_eq!(
        serde_json::to_string(&StorageType::Memory).unwrap(),
        "\"memory\""
    );
}

#[test]
fn test_storage_type_deserialization() {
    assert_eq!(
        serde_json::from_str::<StorageType>("\"system\"").unwrap(),
        StorageType::System
    );
    assert_eq!(
        serde_json::from_str::<StorageType>("\"file\"").unwrap(),
        StorageType::File
    );
    assert_eq!(
        serde_json::from_str::<StorageType>("\"memory\"").unwrap(),
        StorageType::Memory
    );
}

#[test]
fn test_effective_storage_type() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    assert_eq!(config.effective_storage_type(), StorageType::Memory);
}

#[test]
fn test_config_backward_compatibility_missing_fields() {
    let json = r#"{"storage":"memory"}"#;
    let config: CredentialConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.storage, StorageType::Memory);
    assert!(config.default_ttl_seconds.is_some());
    assert!(!config.debug_logging);
    assert!(!config.audit_mode);
}

#[test]
fn test_config_validation_zero_key_cache_ttl() {
    let mut config = CredentialConfig::default();
    config.key_cache_ttl_seconds = 0;

    assert!(config.validate().is_err());
}

#[test]
fn test_config_with_extremely_large_ttl() {
    let config = CredentialConfig::new().with_ttl(Some(100 * 365 * 24 * 60 * 60));
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_serialization_roundtrip() {
    let original = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path("/test/path.enc".to_string())
        .with_ttl(Some(86400))
        .with_debug_logging(true)
        .with_audit_mode(true);

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: CredentialConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(original.storage, deserialized.storage);
    assert_eq!(original.file_path, deserialized.file_path);
    assert_eq!(
        original.default_ttl_seconds,
        deserialized.default_ttl_seconds
    );
    assert_eq!(original.debug_logging, deserialized.debug_logging);
    assert_eq!(original.audit_mode, deserialized.audit_mode);
}

#[test]
fn test_config_default_values_match_spec() {
    let config = CredentialConfig::default();

    assert_eq!(config.storage, StorageType::System);
    assert_eq!(config.default_ttl_seconds, Some(90 * 24 * 60 * 60));
    assert_eq!(config.key_cache_ttl_seconds, 3600);
    assert!(!config.debug_logging);
    assert!(!config.require_confirmation);
}

// ========== Factory 模块测试 ==========

#[test]
fn test_factory_memory_storage() {
    let config = CredentialConfig {
        storage: StorageType::Memory,
        ..Default::default()
    };

    let store = CredentialStoreFactory::create(&config);
    assert!(store.is_ok(), "Memory store should always succeed");
}

#[test]
fn test_factory_file_storage_fallback() {
    let config = CredentialConfig {
        storage: StorageType::File,
        file_path: Some("/invalid/path/that/does/not/exist/credentials.enc".to_string()),
        ..Default::default()
    };

    let store = CredentialStoreFactory::create(&config);
    assert!(
        store.is_ok(),
        "Should fallback to memory when file storage fails"
    );
}

#[test]
fn test_factory_system_storage_fallback() {
    let config = CredentialConfig {
        storage: StorageType::System,
        ..Default::default()
    };

    let store = CredentialStoreFactory::create(&config);
    assert!(
        store.is_ok(),
        "Should fallback when system keychain unavailable"
    );
}

#[test]
fn test_factory_default_config() {
    let config = CredentialConfig::default();
    let store = CredentialStoreFactory::create(&config);
    assert!(store.is_ok(), "Default config should create a valid store");
}

// ========== Storage 模块测试 ==========

#[test]
fn test_memory_store_creation() {
    let store = MemoryCredentialStore::new();
    let list = store.list().unwrap();
    assert_eq!(list.len(), 0);
}

#[test]
fn test_memory_store_add_and_get() {
    let store = MemoryCredentialStore::new();
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password".to_string(),
    );

    store.add(cred.clone()).unwrap();

    let retrieved = store.get("github.com", Some("user")).unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.host, "github.com");
    assert_eq!(retrieved.username, "user");
}

#[test]
fn test_memory_store_remove() {
    let store = MemoryCredentialStore::new();
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password".to_string(),
    );

    store.add(cred).unwrap();
    store.remove("github.com", "user").unwrap();

    let retrieved = store.get("github.com", Some("user")).unwrap();
    assert!(retrieved.is_none());
}

#[test]
fn test_memory_store_list() {
    let store = MemoryCredentialStore::new();

    for i in 0..5 {
        let cred = Credential::new(
            format!("host{i}.com"),
            format!("user{i}"),
            "password".to_string(),
        );
        store.add(cred).unwrap();
    }

    let list = store.list().unwrap();
    assert_eq!(list.len(), 5);
}

#[test]
fn test_memory_store_update_last_used() {
    let store = MemoryCredentialStore::new();
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password".to_string(),
    );

    store.add(cred).unwrap();
    store.update_last_used("github.com", "user").unwrap();

    let retrieved = store.get("github.com", Some("user")).unwrap().unwrap();
    assert!(retrieved.last_used_at.is_some());
}

#[test]
fn test_memory_store_get_by_host_only() {
    let store = MemoryCredentialStore::new();
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password".to_string(),
    );

    store.add(cred).unwrap();

    let retrieved = store.get("github.com", None).unwrap();
    assert!(retrieved.is_some());
}

#[test]
fn test_memory_store_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(MemoryCredentialStore::new());
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let cred = Credential::new(
                format!("host{i}.com"),
                format!("user{i}"),
                "password".to_string(),
            );
            store_clone.add(cred).unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let list = store.list().unwrap();
    assert_eq!(list.len(), 10);
}

#[test]
fn test_memory_store_expired_credentials_filtered() {
    let store = MemoryCredentialStore::new();

    let expires_at = SystemTime::now() - Duration::from_secs(3600);
    let expired_cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "user1".to_string(),
        "password".to_string(),
        expires_at,
    );

    let valid_cred = Credential::new(
        "gitlab.com".to_string(),
        "user2".to_string(),
        "password".to_string(),
    );

    store.add(expired_cred).unwrap();
    store.add(valid_cred).unwrap();

    let retrieved = store.get("github.com", Some("user1")).unwrap();
    assert!(retrieved.is_none());

    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
}

// ========== Platform-specific Keychain 测试 ==========

#[cfg(windows)]
mod windows_keychain_tests {
    use fireworks_collaboration_lib::core::credential::{
        keychain_windows::WindowsCredentialStore, model::Credential, storage::CredentialStore,
    };

    #[test]
    fn test_windows_store_creation() {
        // Should succeed or fail gracefully
        let result = WindowsCredentialStore::new();
        match result {
            Ok(_) => println!("Windows Credential Manager available"),
            Err(e) => println!("Windows Credential Manager unavailable: {e}"),
        }
    }

    #[test]
    fn test_windows_store_add_get_remove() {
        let store = match WindowsCredentialStore::new() {
            Ok(s) => s,
            Err(_) => {
                println!("Skipping test - Windows Credential Manager unavailable");
                return;
            }
        };

        let host = "github.com";
        let username = "test_user_windows";
        let password = "test_password_12345";

        // Clean up any existing credential
        let _ = store.remove(host, username);

        // Add credential
        let cred = Credential::new(host.to_string(), username.to_string(), password.to_string());
        assert!(store.add(cred).is_ok());

        // Get credential
        let retrieved = store.get(host, Some(username)).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.host, host);
        assert_eq!(retrieved.username, username);
        assert_eq!(retrieved.password_or_token, password);

        // Remove credential
        assert!(store.remove(host, username).is_ok());

        // Verify removed
        let after_remove = store.get(host, Some(username)).unwrap();
        assert!(after_remove.is_none());
    }

    #[test]
    fn test_windows_store_list() {
        let store = match WindowsCredentialStore::new() {
            Ok(s) => s,
            Err(_) => {
                println!("Skipping test - Windows Credential Manager unavailable");
                return;
            }
        };

        // Add test credentials
        let creds = vec![
            Credential::new(
                "github.com".to_string(),
                "user1".to_string(),
                "pass1".to_string(),
            ),
            Credential::new(
                "gitlab.com".to_string(),
                "user2".to_string(),
                "pass2".to_string(),
            ),
        ];

        // Clean up first
        for cred in &creds {
            let _ = store.remove(&cred.host, &cred.username);
        }

        // Add credentials
        for cred in creds.clone() {
            assert!(store.add(cred).is_ok());
        }

        // List credentials
        let list = store.list().unwrap();
        // Check that our credentials are in the list
        // (may include other credentials too)
        let our_creds: Vec<_> = list
            .iter()
            .filter(|c| {
                (c.host == "github.com" && c.username == "user1")
                    || (c.host == "gitlab.com" && c.username == "user2")
            })
            .collect();
        assert!(
            our_creds.len() >= 2,
            "Expected at least 2 credentials, found {}",
            our_creds.len()
        );

        // Clean up
        for cred in &creds {
            let _ = store.remove(&cred.host, &cred.username);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix_keychain_tests {
    use fireworks_collaboration_lib::core::credential::{
        keychain_unix::UnixCredentialStore, model::Credential, storage::CredentialStore,
    };

    #[test]
    fn test_unix_store_creation() {
        let result = UnixCredentialStore::new();
        match result {
            Ok(_) => println!("Unix keychain available"),
            Err(e) => println!("Unix keychain unavailable: {}", e),
        }
    }

    #[test]
    fn test_unix_store_add_get_remove() {
        let store = match UnixCredentialStore::new() {
            Ok(s) => s,
            Err(_) => {
                println!("Skipping test - Unix keychain unavailable");
                return;
            }
        };

        let host = "github.com";
        let username = "test_user_unix";
        let password = "test_password_54321";

        // Clean up any existing credential
        let _ = store.remove(host, username);

        // Add credential
        let cred = Credential::new(host.to_string(), username.to_string(), password.to_string());
        assert!(store.add(cred).is_ok());

        // Get credential
        let retrieved = store.get(host, Some(username)).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.host, host);
        assert_eq!(retrieved.username, username);
        assert_eq!(retrieved.password_or_token, password);

        // Remove credential
        assert!(store.remove(host, username).is_ok());

        // Verify removed
        let after_remove = store.get(host, Some(username)).unwrap();
        assert!(after_remove.is_none());
    }

    #[test]
    fn test_account_parsing() {
        let account = UnixCredentialStore::make_account("github.com", "user1");
        assert_eq!(account, "git:github.com:user1");

        let parsed = UnixCredentialStore::parse_account(&account);
        assert!(parsed.is_some());
        let (host, username) = parsed.unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(username, "user1");
    }
}
