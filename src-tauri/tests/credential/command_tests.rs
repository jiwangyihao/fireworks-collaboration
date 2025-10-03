// Credential commands integration tests

use fireworks_collaboration_lib::core::credential::{
    config::{CredentialConfig, StorageType},
    factory::CredentialStoreFactory,
    Credential,
};
use std::time::{Duration, SystemTime};

#[test]
fn test_credential_store_memory_add_get() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    let store = CredentialStoreFactory::create(&config).expect("Failed to create store");

    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "testtoken".to_string(),
    );

    // Add credential
    store.add(cred.clone()).expect("Failed to add credential");

    // Get credential
    let retrieved = store
        .get("github.com", Some("testuser"))
        .expect("Failed to get credential");

    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.host, "github.com");
    assert_eq!(retrieved.username, "testuser");
    assert_eq!(retrieved.password_or_token, "testtoken");
}

#[test]
fn test_credential_store_memory_list() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    let store = CredentialStoreFactory::create(&config).expect("Failed to create store");

    // Add multiple credentials
    let cred1 = Credential::new(
        "github.com".to_string(),
        "user1".to_string(),
        "token1".to_string(),
    );
    let cred2 = Credential::new(
        "gitlab.com".to_string(),
        "user2".to_string(),
        "token2".to_string(),
    );

    store.add(cred1).expect("Failed to add credential 1");
    store.add(cred2).expect("Failed to add credential 2");

    // List all credentials
    let all_creds = store.list().expect("Failed to list credentials");
    assert_eq!(all_creds.len(), 2);
}

#[test]
fn test_credential_store_memory_update() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    let store = CredentialStoreFactory::create(&config).expect("Failed to create store");

    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "oldtoken".to_string(),
    );

    store.add(cred).expect("Failed to add credential");

    // Update by removing and adding again with new token
    store
        .remove("github.com", "testuser")
        .expect("Failed to remove credential");

    let updated_cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "newtoken".to_string(),
    );

    store
        .add(updated_cred)
        .expect("Failed to add updated credential");

    // Verify update
    let retrieved = store
        .get("github.com", Some("testuser"))
        .expect("Failed to get credential")
        .expect("Credential not found");

    assert_eq!(retrieved.password_or_token, "newtoken");
}

#[test]
fn test_credential_store_memory_delete() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    let store = CredentialStoreFactory::create(&config).expect("Failed to create store");

    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "testtoken".to_string(),
    );

    store.add(cred).expect("Failed to add credential");

    // Delete credential
    store
        .remove("github.com", "testuser")
        .expect("Failed to delete credential");

    // Verify deletion
    let retrieved = store
        .get("github.com", Some("testuser"))
        .expect("Failed to get credential");

    assert!(retrieved.is_none());
}

#[test]
fn test_credential_expiry() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    let store = CredentialStoreFactory::create(&config).expect("Failed to create store");

    // Create expired credential (expired 1 day ago)
    let expires_at = SystemTime::now() - Duration::from_secs(86400);
    let cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "testuser".to_string(),
        "testtoken".to_string(),
        expires_at,
    );

    store.add(cred).expect("Failed to add credential");

    // Get should filter out expired credentials
    let retrieved = store
        .get("github.com", Some("testuser"))
        .expect("Failed to get credential");

    // Expired credentials should not be returned
    assert!(
        retrieved.is_none(),
        "Expired credential should not be returned"
    );
}

#[test]
fn test_credential_masked_password() {
    let cred = Credential::new(
        "github.com".to_string(),
        "testuser".to_string(),
        "ghp_1234567890abcdefghijklmnopqrstuvwxyz".to_string(),
    );

    let masked = cred.masked_password();

    // Should contain asterisks
    assert!(masked.contains("****"));

    // Should not contain the full token
    assert!(!masked.contains("ghp_1234567890abcdefghijklmnopqrstuvwxyz"));

    // Should show prefix
    assert!(masked.starts_with("ghp_"));
}

// ==================== P6.5 Command Integration Tests ====================

#[cfg(test)]
mod p6_5_command_tests {
    use fireworks_collaboration_lib::core::credential::audit::AuditLogger;
    use tempfile::tempdir;

    #[test]
    fn test_audit_logger_cleanup() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let log_file = temp_dir.path().join("audit.log");

        let logger = AuditLogger::with_log_file(true, log_file)
            .expect("Failed to create audit logger");

        // Add some logs
        logger.log_operation(
            fireworks_collaboration_lib::core::credential::audit::OperationType::Add,
            "example.com",
            "user1",
            Some("password"),
            true,
            None,
        );

        // Cleanup with 90 days retention (should not remove recent logs)
        let result = logger.cleanup_expired_logs(90);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_access_control_lock_unlock() {
        let logger = AuditLogger::new(true);

        // Initially unlocked
        assert!(!logger.is_locked());
        assert_eq!(logger.remaining_attempts(), 5);

        // Simulate failed attempts
        for _ in 0..5 {
            logger.record_auth_failure();
        }

        // Should be locked now
        assert!(logger.is_locked());
        assert_eq!(logger.remaining_attempts(), 0);

        // Reset access control
        logger.reset_access_control();
        assert!(!logger.is_locked());
        assert_eq!(logger.remaining_attempts(), 5);
    }

    #[test]
    fn test_access_control_partial_failures() {
        let logger = AuditLogger::new(true);

        // Add 2 failures
        for _ in 0..2 {
            logger.record_auth_failure();
        }

        // Should have 3 attempts remaining
        assert!(!logger.is_locked());
        assert_eq!(logger.remaining_attempts(), 3);

        // Successful unlock resets
        logger.reset_access_control();

        assert_eq!(logger.remaining_attempts(), 5);
    }

    #[test]
    fn test_audit_log_persistence() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let log_file = temp_dir.path().join("audit.log");

        // Create logger and add logs
        {
            let logger = AuditLogger::with_log_file(true, log_file.clone())
                .expect("Failed to create audit logger");

            logger.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Add,
                "example.com",
                "user1",
                Some("password"),
                true,
                None,
            );
        }

        // Verify file exists
        assert!(log_file.exists());

        // Create new logger and verify logs persisted
        let logger2 = AuditLogger::with_log_file(true, log_file)
            .expect("Failed to create audit logger");

        let json = logger2.export_to_json().expect("Failed to export logs");
        assert!(json.contains("example.com"));
        assert!(json.contains("user1"));
    }

    #[test]
    fn test_cleanup_invalid_retention() {
        let logger = AuditLogger::new(true);

        // Zero retention should succeed but return Ok(0) as no logs to clean
        let result = logger.cleanup_expired_logs(0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
