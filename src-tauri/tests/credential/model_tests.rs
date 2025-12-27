//! Credential Model 测试
//!
//! 测试 `core::credential::Credential` 模型的各种功能

use fireworks_collaboration_lib::core::credential::Credential;
use std::time::{Duration, SystemTime};

// ============================================================================
// Credential 基本功能测试
// ============================================================================

#[test]
fn test_credential_new() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "token123".to_string(),
    );

    assert_eq!(cred.host, "github.com");
    assert_eq!(cred.username, "user");
    assert_eq!(cred.password_or_token, "token123");
    assert!(cred.expires_at.is_none());
    assert!(cred.last_used_at.is_none());
}

#[test]
fn test_credential_new_with_expiry() {
    let expires_at = SystemTime::now() + Duration::from_secs(3600);
    let cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "user".to_string(),
        "token".to_string(),
        expires_at,
    );

    assert!(cred.expires_at.is_some());
}

// ============================================================================
// is_expired 测试
// ============================================================================

#[test]
fn test_credential_not_expired_when_no_expiry() {
    let cred = Credential::new("host".to_string(), "user".to_string(), "pass".to_string());
    assert!(!cred.is_expired());
}

#[test]
fn test_credential_not_expired_when_future_expiry() {
    let future = SystemTime::now() + Duration::from_secs(3600);
    let cred = Credential::new_with_expiry(
        "host".to_string(),
        "user".to_string(),
        "pass".to_string(),
        future,
    );
    assert!(!cred.is_expired());
}

#[test]
fn test_credential_is_expired_when_past_expiry() {
    // Create with expiry in the past
    let past = SystemTime::now() - Duration::from_secs(3600);
    let cred = Credential::new_with_expiry(
        "host".to_string(),
        "user".to_string(),
        "pass".to_string(),
        past,
    );
    assert!(cred.is_expired());
}

// ============================================================================
// identifier 测试
// ============================================================================

#[test]
fn test_credential_identifier() {
    let cred = Credential::new(
        "github.com".to_string(),
        "alice".to_string(),
        "token".to_string(),
    );
    assert_eq!(cred.identifier(), "alice@github.com");
}

#[test]
fn test_credential_identifier_with_special_chars() {
    let cred = Credential::new(
        "git.example.com".to_string(),
        "user+test".to_string(),
        "token".to_string(),
    );
    assert_eq!(cred.identifier(), "user+test@git.example.com");
}

// ============================================================================
// masked_password 测试
// ============================================================================

#[test]
fn test_masked_password_short() {
    let cred = Credential::new(
        "host".to_string(),
        "user".to_string(),
        "short".to_string(), // 5 chars
    );
    assert_eq!(cred.masked_password(), "***");
}

#[test]
fn test_masked_password_exactly_8() {
    let cred = Credential::new(
        "host".to_string(),
        "user".to_string(),
        "12345678".to_string(), // 8 chars
    );
    assert_eq!(cred.masked_password(), "***");
}

#[test]
fn test_masked_password_longer_than_8() {
    let cred = Credential::new(
        "host".to_string(),
        "user".to_string(),
        "ghp_1234567890abcdef".to_string(), // 20 chars
    );
    assert_eq!(cred.masked_password(), "ghp_****cdef");
}

#[test]
fn test_masked_password_9_chars() {
    let cred = Credential::new(
        "host".to_string(),
        "user".to_string(),
        "123456789".to_string(), // 9 chars
    );
    // First 4 + **** + last 4 = "1234****6789"
    assert_eq!(cred.masked_password(), "1234****6789");
}

// ============================================================================
// update_last_used 测试
// ============================================================================

#[test]
fn test_update_last_used() {
    let mut cred = Credential::new("host".to_string(), "user".to_string(), "pass".to_string());
    assert!(cred.last_used_at.is_none());

    cred.update_last_used();
    assert!(cred.last_used_at.is_some());

    // Verify it's recent
    let elapsed = cred
        .last_used_at
        .unwrap()
        .elapsed()
        .unwrap_or(Duration::from_secs(100));
    assert!(elapsed < Duration::from_secs(2));
}

// ============================================================================
// Display/Debug 测试
// ============================================================================

#[test]
fn test_credential_display_masks_password() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "supersecretpassword".to_string(),
    );
    let display = format!("{}", cred);

    // Should contain masked password, not real one
    assert!(display.contains("****"));
    assert!(!display.contains("supersecretpassword"));
}

#[test]
fn test_credential_debug_masks_password() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "supersecretpassword".to_string(),
    );
    let debug = format!("{:?}", cred);

    // Should contain masked password, not real one
    assert!(debug.contains("****"));
    assert!(!debug.contains("supersecretpassword"));
}
