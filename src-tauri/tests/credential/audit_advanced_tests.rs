//! Advanced audit functionality tests
//!
//! Tests for P6.5 features:
//! - Audit log persistence
//! - Automatic cleanup
//! - Access control
//! - Lockout mechanism

use fireworks_collaboration_lib::core::credential::audit::{AuditLogger, OperationType};
use std::fs;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_audit_log_persistence() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_path = temp_dir.path().join("audit.json");

    // Create logger with persistent storage
    let logger = AuditLogger::with_log_file(true, &log_path)
        .expect("Failed to create logger with file");

    // Log some operations
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
        None,
        true,
        None,
    );

    // Check that file exists and contains events
    assert!(log_path.exists(), "Audit log file should exist");

    let content = fs::read_to_string(&log_path).expect("Failed to read audit log");
    assert!(content.contains("github.com"), "Log should contain host");
    assert!(content.contains("user1"), "Log should contain username");

    // Create new logger from same file
    let logger2 = AuditLogger::with_log_file(true, &log_path)
        .expect("Failed to load logger from file");

    assert_eq!(logger2.event_count(), 2, "Should load 2 events from file");
}

#[test]
fn test_cleanup_expired_logs() {
    let logger = AuditLogger::new(true);

    // Add some events
    for i in 0..10 {
        logger.log_operation(
            OperationType::Add,
            "github.com",
            &format!("user{}", i),
            Some("password"),
            true,
            None,
        );
    }

    assert_eq!(logger.event_count(), 10, "Should have 10 events");

    // Cleanup logs older than 0 days (should remove nothing since all are fresh)
    let removed = logger
        .cleanup_expired_logs(0)
        .expect("Cleanup should succeed");
    assert_eq!(removed, 10, "Should remove all logs when retention is 0 days");
    assert_eq!(logger.event_count(), 0, "Should have 0 events after cleanup");
}

#[test]
fn test_access_control_lockout() {
    let logger = AuditLogger::new(false);

    // Initially not locked
    assert!(!logger.is_locked(), "Should not be locked initially");
    assert_eq!(logger.remaining_attempts(), 5, "Should have 5 attempts");

    // Record failures
    for i in 0..4 {
        logger.record_auth_failure();
        assert!(!logger.is_locked(), "Should not be locked after {} failures", i + 1);
    }

    // 5th failure should trigger lockout
    logger.record_auth_failure();
    assert!(logger.is_locked(), "Should be locked after 5 failures");
    assert_eq!(logger.remaining_attempts(), 0, "Should have 0 attempts");
}

#[test]
fn test_access_control_reset() {
    let logger = AuditLogger::new(false);

    // Lock the store
    for _ in 0..5 {
        logger.record_auth_failure();
    }
    assert!(logger.is_locked(), "Should be locked");

    // Reset access control
    logger.reset_access_control();
    assert!(!logger.is_locked(), "Should not be locked after reset");
    assert_eq!(logger.remaining_attempts(), 5, "Should have 5 attempts after reset");
}

#[test]
fn test_audit_mode_hash_persistence() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_path = temp_dir.path().join("audit.json");

    // Create logger with audit mode enabled
    let logger = AuditLogger::with_log_file(true, &log_path)
        .expect("Failed to create logger");

    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );

    // Read file content
    let content = fs::read_to_string(&log_path).expect("Failed to read file");

    // Should contain credential hash in audit mode
    assert!(content.contains("credentialHash"), "Should contain hash field");
    // Should not contain plain password
    assert!(!content.contains("password123"), "Should not contain plain password");
}

#[test]
fn test_standard_mode_no_hash_persistence() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_path = temp_dir.path().join("audit.json");

    // Create logger with audit mode disabled
    let logger = AuditLogger::with_log_file(false, &log_path)
        .expect("Failed to create logger");

    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password123"),
        true,
        None,
    );

    // Read file content
    let content = fs::read_to_string(&log_path).expect("Failed to read file");

    // Should NOT contain credential hash in standard mode
    assert!(!content.contains("credentialHash"), "Should not contain hash in standard mode");
    // Should not contain plain password
    assert!(!content.contains("password123"), "Should not contain plain password");
}

#[test]
fn test_concurrent_audit_logging_with_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_path = temp_dir.path().join("audit.json");

    let logger = AuditLogger::with_log_file(true, &log_path)
        .expect("Failed to create logger");

    let logger_clone1 = logger.clone();
    let logger_clone2 = logger.clone();

    let handle1 = thread::spawn(move || {
        for i in 0..5 {
            logger_clone1.log_operation(
                OperationType::Add,
                "github.com",
                &format!("thread1_user{}", i),
                Some("password"),
                true,
                None,
            );
            thread::sleep(Duration::from_millis(10));
        }
    });

    let handle2 = thread::spawn(move || {
        for i in 0..5 {
            logger_clone2.log_operation(
                OperationType::Add,
                "github.com",
                &format!("thread2_user{}", i),
                Some("password"),
                true,
                None,
            );
            thread::sleep(Duration::from_millis(10));
        }
    });

    handle1.join().expect("Thread 1 failed");
    handle2.join().expect("Thread 2 failed");

    // Should have exactly 10 events
    assert_eq!(logger.event_count(), 10, "Should have 10 events from 2 threads");
}

#[test]
fn test_audit_log_file_corruption_recovery() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let log_path = temp_dir.path().join("audit.json");

    // Create corrupt file
    fs::write(&log_path, "{ invalid json ").expect("Failed to write corrupt file");

    // Should handle corruption gracefully
    let logger = AuditLogger::with_log_file(true, &log_path);
    assert!(logger.is_ok(), "Should create logger even if file is corrupt");

    let logger = logger.unwrap();
    // Should start with empty log after corruption
    assert_eq!(logger.event_count(), 0, "Should start with 0 events after corruption");

    // Should be able to log new events
    logger.log_operation(
        OperationType::Add,
        "github.com",
        "user1",
        Some("password"),
        true,
        None,
    );

    assert_eq!(logger.event_count(), 1, "Should have 1 event after logging");
}
