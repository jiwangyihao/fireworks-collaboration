//! Credential command integration tests (Direct Command Call)

use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use tauri::{Assets, Manager};
use tauri_utils::assets::{AssetKey, CspHash};

use fireworks_collaboration_lib::app::commands::credential::*;
use fireworks_collaboration_lib::core::credential::{
    audit::AuditLogger,
    config::{CredentialConfig, StorageType},
};

// Include MockAssets definition
struct MockAssets;

impl<R: tauri::Runtime> Assets<R> for MockAssets {
    fn get(&self, _key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        None
    }
    fn iter(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Cow<'_, [u8]>)> + '_> {
        Box::new(std::iter::empty())
    }
    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

fn create_mock_app() -> (
    tauri::App<tauri::test::MockRuntime>,
    SharedCredentialFactory,
    SharedAuditLogger,
) {
    let factory: SharedCredentialFactory = Arc::new(Mutex::new(None));
    let audit_logger = Arc::new(Mutex::new(AuditLogger::new(false))); // Memory mode audit

    let context = tauri::test::mock_context(MockAssets);

    let app = tauri::test::mock_builder()
        .manage::<SharedCredentialFactory>(factory.clone())
        .manage::<SharedAuditLogger>(audit_logger.clone())
        .build(context)
        .expect("Failed to build mock app");

    (app, factory, audit_logger)
}

/// Helper to initialize store with memory backend
async fn init_store(app: &tauri::App<tauri::test::MockRuntime>) {
    let config = CredentialConfig {
        storage: StorageType::Memory,
        ..Default::default()
    };
    let state = app.state::<SharedCredentialFactory>();
    set_master_password("dummy".to_string(), config, state)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_set_master_password_init() {
    let (app, factory, _) = create_mock_app();

    let config = CredentialConfig {
        storage: StorageType::Memory,
        ..Default::default()
    };
    let state = app.state::<SharedCredentialFactory>();

    let result = set_master_password("master".to_string(), config, state).await;
    assert!(result.is_ok());

    assert!(factory.lock().unwrap().is_some());
}

#[tokio::test]
async fn test_add_and_get_credential() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    let req = AddCredentialRequest {
        host: "github.com".to_string(),
        username: "testuser".to_string(),
        password_or_token: "secret".to_string(),
        expires_in_days: None,
    };

    let add_res = add_credential(req, app.state(), app.state()).await;
    assert!(add_res.is_ok());

    let get_res = get_credential(
        "github.com".to_string(),
        Some("testuser".to_string()),
        app.state(),
        app.state(),
    )
    .await;

    assert!(get_res.is_ok());
    let cred = get_res.unwrap();
    assert!(cred.is_some());
    assert_eq!(cred.unwrap().username, "testuser");
}

#[tokio::test]
async fn test_update_credential() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    // Add initial
    let req = AddCredentialRequest {
        host: "gitlab.com".to_string(),
        username: "dev".to_string(),
        password_or_token: "pass1".to_string(),
        expires_in_days: None,
    };
    add_credential(req, app.state(), app.state()).await.unwrap();

    // Update
    let update_req = UpdateCredentialRequest {
        host: "gitlab.com".to_string(),
        username: "dev".to_string(),
        new_password: "pass2".to_string(),
        expires_in_days: None,
    };
    let res = update_credential(update_req, app.state(), app.state()).await;
    assert!(res.is_ok());

    // Verify does not return password in list, but operation succeeded
    // We can't verify password value via get_credential as it masks it.
    // But success implies update worked.
}

#[tokio::test]
async fn test_delete_credential() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    // Add
    let req = AddCredentialRequest {
        host: "bitbucket.org".to_string(),
        username: "user".to_string(),
        password_or_token: "pass".to_string(),
        expires_in_days: None,
    };
    add_credential(req, app.state(), app.state()).await.unwrap();

    // Delete
    let del_res = delete_credential(
        "bitbucket.org".to_string(),
        "user".to_string(),
        app.state(),
        app.state(),
    )
    .await;
    assert!(del_res.is_ok());

    // Verify gone
    let get_res = get_credential(
        "bitbucket.org".to_string(),
        Some("user".to_string()),
        app.state(),
        app.state(),
    )
    .await
    .unwrap();
    assert!(get_res.is_none());
}

#[tokio::test]
async fn test_list_credentials() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    add_credential(
        AddCredentialRequest {
            host: "h1".to_string(),
            username: "u1".to_string(),
            password_or_token: "p".to_string(),
            expires_in_days: None,
        },
        app.state(),
        app.state(),
    )
    .await
    .unwrap();

    add_credential(
        AddCredentialRequest {
            host: "h2".to_string(),
            username: "u2".to_string(),
            password_or_token: "p".to_string(),
            expires_in_days: None,
        },
        app.state(),
        app.state(),
    )
    .await
    .unwrap();

    let res = list_credentials(app.state(), app.state()).await;
    assert!(res.is_ok());
    assert_eq!(res.unwrap().len(), 2);
}

#[tokio::test]
async fn test_audit_log_workflow() {
    let (app, _, audit) = create_mock_app();
    init_store(&app).await;

    // Perform operations to generate logs
    add_credential(
        AddCredentialRequest {
            host: "h1".to_string(),
            username: "u1".to_string(),
            password_or_token: "p".to_string(),
            expires_in_days: None,
        },
        app.state(),
        app.state(),
    )
    .await
    .unwrap();

    let log_json = export_audit_log(app.state()).await;
    assert!(log_json.is_ok());
    let json = log_json.unwrap();
    assert!(json.contains("h1"));
    assert!(json.contains("add"));

    // Verify internally
    let logger = audit.lock().unwrap();
    assert!(logger.event_count() >= 1);
}

#[tokio::test]
async fn test_unlock_store() {
    let (app, _, audit) = create_mock_app();
    // No init needed for unlock_store technically, but set_master_password does init.
    // unlock_store calls set_master_password internally to verify/re-init if needed?
    // Looking at unlock_store impl: it logic mainly checks lock state then calls set_master_password.

    let config = CredentialConfig {
        storage: StorageType::Memory,
        ..Default::default()
    };

    // Unlock
    let res = unlock_store("masterpass".to_string(), config, app.state(), app.state()).await;

    assert!(res.is_ok());

    // Check audit
    let logger = audit.lock().unwrap();
    let events = logger.get_events();
    let unlock_event = events.last().unwrap();
    assert_eq!(
        unlock_event.operation,
        fireworks_collaboration_lib::core::credential::audit::OperationType::Unlock
    );
    assert!(unlock_event.success);
}

// ============ Error Path Tests ============

#[tokio::test]
async fn test_get_credential_not_found() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    let get_res = get_credential(
        "nonexistent.com".to_string(),
        Some("nobody".to_string()),
        app.state(),
        app.state(),
    )
    .await;

    assert!(get_res.is_ok());
    assert!(get_res.unwrap().is_none());
}

#[tokio::test]
async fn test_update_nonexistent_credential() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    let update_req = UpdateCredentialRequest {
        host: "nonexistent.com".to_string(),
        username: "nobody".to_string(),
        new_password: "newpass".to_string(),
        expires_in_days: None,
    };
    let res = update_credential(update_req, app.state(), app.state()).await;

    // Update of nonexistent credential should fail
    assert!(res.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_credential() {
    let (app, _, _) = create_mock_app();
    init_store(&app).await;

    let del_res = delete_credential(
        "nonexistent.com".to_string(),
        "nobody".to_string(),
        app.state(),
        app.state(),
    )
    .await;

    // Delete of nonexistent credential may succeed or fail depending on implementation
    // but it should not panic
    let _ = del_res; // Operation completes without panic
}

#[tokio::test]
async fn test_add_credential_without_init() {
    let (app, _, _) = create_mock_app();
    // NO init_store call - store is not initialized

    let req = AddCredentialRequest {
        host: "github.com".to_string(),
        username: "testuser".to_string(),
        password_or_token: "secret".to_string(),
        expires_in_days: None,
    };

    let add_res = add_credential(req, app.state(), app.state()).await;
    // Should fail because store is not initialized
    assert!(add_res.is_err());
}
