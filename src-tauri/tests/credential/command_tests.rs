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
