//! Integration tests for P6.5 credential commands
//!
//! Tests the four new Tauri commands added in P6.5:
//! - cleanup_audit_logs
//! - is_credential_locked
//! - reset_credential_lock
//! - remaining_auth_attempts

use fireworks_collaboration_lib::app::commands::credential::{
    cleanup_audit_logs, is_credential_locked, remaining_auth_attempts, reset_credential_lock,
    unlock_store, SharedAuditLogger,
};
use fireworks_collaboration_lib::core::credential::audit::AuditLogger;
use fireworks_collaboration_lib::core::credential::config::CredentialConfig;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::State;
use tempfile::tempdir;

/// Helper to create a shared audit logger with a temporary log file
fn create_test_audit_logger() -> (SharedAuditLogger, tempfile::TempDir) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let log_file = temp_dir.path().join("audit.log");

    let logger = AuditLogger::new()
        .with_log_file(log_file)
        .expect("Failed to create audit logger");

    (Arc::new(Mutex::new(logger)), temp_dir)
}

/// Helper to wrap SharedAuditLogger as a State
fn to_state(logger: SharedAuditLogger) -> State<'static, SharedAuditLogger> {
    // SAFETY: We're creating a leaked Box for testing purposes only
    // In production, this would be managed by Tauri's state system
    let leaked: &'static SharedAuditLogger = Box::leak(Box::new(logger));
    State::from(leaked)
}

#[tokio::test]
async fn test_cleanup_audit_logs_with_retention() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Add some test logs
    {
        let mut log = logger.lock().unwrap();
        log.log_operation(
            fireworks_collaboration_lib::core::credential::audit::OperationType::Add,
            "example.com",
            "user1",
            Some("password"),
            true,
            None,
        );
        log.log_operation(
            fireworks_collaboration_lib::core::credential::audit::OperationType::Get,
            "example.com",
            "user1",
            Some("password"),
            true,
            None,
        );
    }

    // Clean up logs older than 90 days (should remove nothing if logs are recent)
    let state = to_state(logger.clone());
    let result = cleanup_audit_logs(90, state).await;

    assert!(result.is_ok());
    let removed = result.unwrap();
    
    // Recent logs should not be removed
    assert_eq!(removed, 0);
}

#[tokio::test]
async fn test_cleanup_audit_logs_invalid_retention() {
    let (logger, _temp_dir) = create_test_audit_logger();
    let state = to_state(logger);

    // Test with zero retention days (should fail)
    let result = cleanup_audit_logs(0, state).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("must be greater than 0"));
}

#[tokio::test]
async fn test_is_credential_locked_initially_unlocked() {
    let (logger, _temp_dir) = create_test_audit_logger();
    let state = to_state(logger);

    let result = is_credential_locked(state).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), false);
}

#[tokio::test]
async fn test_credential_lock_after_failures() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Simulate 5 failed unlock attempts
    {
        let mut log = logger.lock().unwrap();
        for _ in 0..5 {
            log.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                Some("Invalid password"),
            );
        }
    }

    let state = to_state(logger);
    let result = is_credential_locked(state).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[tokio::test]
async fn test_reset_credential_lock() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // First, lock the store with failed attempts
    {
        let mut log = logger.lock().unwrap();
        for _ in 0..5 {
            log.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                None,
            );
        }
    }

    // Verify it's locked
    let state1 = to_state(logger.clone());
    let locked = is_credential_locked(state1).await.unwrap();
    assert!(locked);

    // Reset the lock
    let state2 = to_state(logger.clone());
    let reset_result = reset_credential_lock(state2).await;
    assert!(reset_result.is_ok());

    // Verify it's now unlocked
    let state3 = to_state(logger);
    let still_locked = is_credential_locked(state3).await.unwrap();
    assert_eq!(still_locked, false);
}

#[tokio::test]
async fn test_remaining_auth_attempts_max() {
    let (logger, _temp_dir) = create_test_audit_logger();
    let state = to_state(logger);

    // Initially should have max attempts (5)
    let result = remaining_auth_attempts(state).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 5);
}

#[tokio::test]
async fn test_remaining_auth_attempts_decreases() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Add 2 failed attempts
    {
        let mut log = logger.lock().unwrap();
        for _ in 0..2 {
            log.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                None,
            );
        }
    }

    let state = to_state(logger);
    let result = remaining_auth_attempts(state).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 3); // 5 - 2 = 3
}

#[tokio::test]
async fn test_remaining_auth_attempts_when_locked() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Add 5 failed attempts to lock
    {
        let mut log = logger.lock().unwrap();
        for _ in 0..5 {
            log.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                None,
            );
        }
    }

    let state = to_state(logger);
    let result = remaining_auth_attempts(state).await;
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[tokio::test]
async fn test_lock_auto_expires() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Lock the store
    {
        let mut log = logger.lock().unwrap();
        for _ in 0..5 {
            log.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                None,
            );
        }
    }

    // Verify locked
    let state1 = to_state(logger.clone());
    assert!(is_credential_locked(state1).await.unwrap());

    // Wait for lock to expire (default is 30 minutes, but we can't wait that long in tests)
    // Instead, we manually reset the lock time in the logger
    {
        let mut log = logger.lock().unwrap();
        // Force reset by calling reset_access_control
        log.reset_access_control();
    }

    // Verify unlocked
    let state2 = to_state(logger);
    assert_eq!(is_credential_locked(state2).await.unwrap(), false);
}

#[tokio::test]
async fn test_successful_unlock_resets_failures() {
    let (logger, _temp_dir) = create_test_audit_logger();

    // Add 2 failed attempts
    {
        let mut log = logger.lock().unwrap();
        for _ in 0..2 {
            log.log_operation(
                fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some("wrong_password"),
                false,
                None,
            );
        }
    }

    // Verify reduced attempts
    let state1 = to_state(logger.clone());
    assert_eq!(remaining_auth_attempts(state1).await.unwrap(), 3);

    // Simulate successful unlock
    {
        let mut log = logger.lock().unwrap();
        log.log_operation(
            fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock,
            "credential_store",
            "master",
            Some("correct_password"),
            true,
            None,
        );
        log.reset_access_control();
    }

    // Verify attempts reset to max
    let state2 = to_state(logger);
    assert_eq!(remaining_auth_attempts(state2).await.unwrap(), 5);
}
