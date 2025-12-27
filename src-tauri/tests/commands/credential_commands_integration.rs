//! Integration tests for credential commands
//!
//! Tests the credential command types and logic.
//! State-dependent command tests are covered via the underlying core logic.

use fireworks_collaboration_lib::app::commands::credential::CredentialInfo;
use fireworks_collaboration_lib::core::credential::audit::{AuditLogger, OperationType};
use fireworks_collaboration_lib::core::credential::Credential;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

// ============================================================================
// Core AuditLogger Logic Tests (testing the logic that commands wrap)
// ============================================================================

/// Helper to create a shared audit logger with a temporary log file
fn create_test_audit_logger() -> (Arc<Mutex<AuditLogger>>, tempfile::TempDir) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("audit.log");

    let logger =
        AuditLogger::with_log_file(false, log_file).expect("Failed to create audit logger");

    (Arc::new(Mutex::new(logger)), temp_dir)
}

#[test]
fn test_audit_logger_cleanup_recent_logs() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Add some test logs
    {
        let log = logger.lock().unwrap();
        log.log_operation(
            OperationType::Add,
            "example.com",
            "user1",
            Some("password"),
            true,
            None,
        );
        log.log_operation(
            OperationType::Get,
            "example.com",
            "user1",
            Some("password"),
            true,
            None,
        );
    }

    // Clean up logs older than 90 days (should remove nothing if logs are recent)
    let result = {
        let log = logger.lock().unwrap();
        log.cleanup_expired_logs(90)
    };

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn test_audit_logger_initially_unlocked() {
    let (logger, _temp_dir) = create_test_audit_logger();

    let log = logger.lock().unwrap();
    assert!(!log.is_locked());
}

#[test]
fn test_audit_logger_locks_after_failures() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Simulate 5 failed unlock attempts
    {
        let log = logger.lock().unwrap();
        for _ in 0..5 {
            log.log_operation(
                OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                Some("Invalid password".to_string()),
            );
            log.record_auth_failure();
        }
    }

    let log = logger.lock().unwrap();
    assert!(log.is_locked());
}

#[test]
fn test_audit_logger_remaining_attempts() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Initially should have max attempts (5)
    {
        let log = logger.lock().unwrap();
        assert_eq!(log.remaining_attempts(), 5);
    }

    // Add 2 failed attempts
    {
        let log = logger.lock().unwrap();
        for _ in 0..2 {
            log.record_auth_failure();
        }
    }

    let log = logger.lock().unwrap();
    assert_eq!(log.remaining_attempts(), 3); // 5 - 2 = 3
}

#[test]
fn test_audit_logger_reset_access_control() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Lock the store with failed attempts
    {
        let log = logger.lock().unwrap();
        for _ in 0..5 {
            log.record_auth_failure();
        }
        assert!(log.is_locked());
    }

    // Reset the lock
    {
        let log = logger.lock().unwrap();
        log.reset_access_control();
    }

    // Verify it's now unlocked
    let log = logger.lock().unwrap();
    assert!(!log.is_locked());
    assert_eq!(log.remaining_attempts(), 5);
}

// ============================================================================
// CredentialInfo Type Conversion Tests
// ============================================================================

#[test]
fn test_credential_info_from_basic() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "secret123".to_string(),
    );
    let info = CredentialInfo::from(&cred);

    assert_eq!(info.host, "github.com");
    assert_eq!(info.username, "user");
    // Password should be masked
    assert!(!info.masked_password.contains("secret123"));
    assert!(info.masked_password.contains("***"));
    assert!(!info.is_expired);
    assert!(info.expires_at.is_none());
    assert!(info.last_used_at.is_none());
}

#[test]
fn test_credential_info_from_with_expiry() {
    let future_time = SystemTime::now() + Duration::from_secs(86400); // 1 day
    let cred = Credential::new_with_expiry(
        "gitlab.com".to_string(),
        "admin".to_string(),
        "token".to_string(),
        future_time,
    );
    let info = CredentialInfo::from(&cred);

    assert_eq!(info.host, "gitlab.com");
    assert!(!info.is_expired);
    assert!(info.expires_at.is_some());
    // expires_at should be a reasonable future timestamp
    assert!(info.expires_at.unwrap() > info.created_at);
}

#[test]
fn test_credential_info_from_expired() {
    let past_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1); // Very old
    let cred = Credential::new_with_expiry(
        "example.com".to_string(),
        "test".to_string(),
        "pass".to_string(),
        past_time,
    );
    let info = CredentialInfo::from(&cred);

    assert!(info.is_expired);
    assert!(info.expires_at.is_some());
    assert_eq!(info.expires_at.unwrap(), 1); // 1 second after epoch
}

#[test]
fn test_credential_info_timestamp_conversion() {
    let cred = Credential::new("test.com".to_string(), "u".to_string(), "p".to_string());
    let info = CredentialInfo::from(&cred);

    // created_at should be a recent timestamp (within last minute)
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(info.created_at <= now_secs);
    assert!(info.created_at > now_secs - 60);
}

#[test]
fn test_credential_info_masked_password_short() {
    let cred = Credential::new(
        "host".to_string(),
        "u".to_string(),
        "abc".to_string(), // Very short password
    );
    let info = CredentialInfo::from(&cred);

    // Even short passwords should be masked
    assert!(!info.masked_password.contains("abc"));
}
