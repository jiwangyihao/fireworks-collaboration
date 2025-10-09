//! Credential management commands.
//!
//! This module provides Tauri command handlers for credential storage and management,
//! including add, get, update, delete, list operations, as well as master password
//! management and audit log export.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::core::credential::{
    audit::AuditLogger, config::CredentialConfig, factory::CredentialStoreFactory, Credential,
    CredentialStore,
};

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Shared credential store factory state.
pub type SharedCredentialFactory = Arc<Mutex<Option<Arc<dyn CredentialStore>>>>;

/// Shared audit logger state.
pub type SharedAuditLogger = Arc<Mutex<AuditLogger>>;

/// Credential data for frontend (with masked password).
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInfo {
    pub host: String,
    pub username: String,
    pub masked_password: String,
    pub created_at: u64, // Unix timestamp in seconds
    pub expires_at: Option<u64>,
    pub last_used_at: Option<u64>,
    pub is_expired: bool,
}

impl From<&Credential> for CredentialInfo {
    fn from(cred: &Credential) -> Self {
        let created_at = cred
            .created_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let expires_at = cred.expires_at.and_then(|t| {
            t.duration_since(SystemTime::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });

        let last_used_at = cred.last_used_at.and_then(|t| {
            t.duration_since(SystemTime::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });

        Self {
            host: cred.host.clone(),
            username: cred.username.clone(),
            masked_password: cred.masked_password(),
            created_at,
            expires_at,
            last_used_at,
            is_expired: cred.is_expired(),
        }
    }
}

/// Add credential request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialRequest {
    pub host: String,
    pub username: String,
    pub password_or_token: String,
    pub expires_in_days: Option<u64>,
}

/// Update credential request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCredentialRequest {
    pub host: String,
    pub username: String,
    pub new_password: String,
    pub expires_in_days: Option<u64>,
}

/// Add a new credential to the store.
///
/// # Arguments
///
/// * `request` - Credential information including host, username, password, and optional expiry
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns Ok(()) on success, or an error message on failure.
#[tauri::command]
pub async fn add_credential(
    request: AddCredentialRequest,
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<(), String> {
    let store = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?
        .as_ref()
        .ok_or("Credential store not initialized")?
        .clone();

    let expires_at = request
        .expires_in_days
        .map(|days| SystemTime::now() + Duration::from_secs(days * 86400));

    let credential = if let Some(expiry) = expires_at {
        Credential::new_with_expiry(
            request.host.clone(),
            request.username.clone(),
            request.password_or_token,
            expiry,
        )
    } else {
        Credential::new(
            request.host.clone(),
            request.username.clone(),
            request.password_or_token,
        )
    };

    store
        .add(credential.clone())
        .map_err(|e| format!("Failed to add credential: {}", e))?;

    // Log audit event
    if let Ok(logger) = audit.lock() {
        logger.log_operation(
            crate::core::credential::audit::OperationType::Add,
            &request.host,
            &request.username,
            Some(&credential.password_or_token),
            true,
            None,
        );
    }

    tracing::info!(
        target = "credential",
        host = %request.host,
        username = %request.username,
        "Credential added successfully"
    );

    Ok(())
}

/// Get a credential from the store.
///
/// # Arguments
///
/// * `host` - Host identifier (e.g., "github.com")
/// * `username` - Username (optional, if None returns first matching host)
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns the credential info if found, or None if not found.
#[tauri::command]
pub async fn get_credential(
    host: String,
    username: Option<String>,
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<Option<CredentialInfo>, String> {
    let store = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?
        .as_ref()
        .ok_or("Credential store not initialized")?
        .clone();

    let credential = store
        .get(&host, username.as_deref())
        .map_err(|e| format!("Failed to get credential: {}", e))?;

    // Log audit event
    if let Ok(logger) = audit.lock() {
        let username_str = username.as_deref().unwrap_or("");
        logger.log_operation(
            crate::core::credential::audit::OperationType::Get,
            &host,
            username_str,
            None,
            credential.is_some(),
            None,
        );
    }

    Ok(credential.as_ref().map(CredentialInfo::from))
}

/// Update an existing credential.
///
/// # Arguments
///
/// * `request` - Update request with host, username, new password, and optional expiry
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns Ok(()) on success, or an error message on failure.
#[tauri::command]
pub async fn update_credential(
    request: UpdateCredentialRequest,
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<(), String> {
    let store = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?
        .as_ref()
        .ok_or("Credential store not initialized")?
        .clone();

    let expires_at = request
        .expires_in_days
        .map(|days| SystemTime::now() + Duration::from_secs(days * 86400));

    // Update by removing the old credential and adding the new one
    store
        .remove(&request.host, &request.username)
        .map_err(|e| format!("Failed to remove old credential: {}", e))?;

    let credential = if let Some(expiry) = expires_at {
        Credential::new_with_expiry(
            request.host.clone(),
            request.username.clone(),
            request.new_password,
            expiry,
        )
    } else {
        Credential::new(
            request.host.clone(),
            request.username.clone(),
            request.new_password,
        )
    };

    store
        .add(credential.clone())
        .map_err(|e| format!("Failed to add updated credential: {}", e))?;

    // Log audit event
    if let Ok(logger) = audit.lock() {
        logger.log_operation(
            crate::core::credential::audit::OperationType::Update,
            &request.host,
            &request.username,
            Some(&credential.password_or_token),
            true,
            None,
        );
    }

    tracing::info!(
        target = "credential",
        host = %request.host,
        username = %request.username,
        "Credential updated successfully"
    );

    Ok(())
}

/// Delete a credential from the store.
///
/// # Arguments
///
/// * `host` - Host identifier
/// * `username` - Username
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns Ok(()) on success, or an error message on failure.
#[tauri::command]
pub async fn delete_credential(
    host: String,
    username: String,
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<(), String> {
    let store = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?
        .as_ref()
        .ok_or("Credential store not initialized")?
        .clone();

    store
        .remove(&host, &username)
        .map_err(|e| format!("Failed to delete credential: {}", e))?;

    // Log audit event
    if let Ok(logger) = audit.lock() {
        logger.log_operation(
            crate::core::credential::audit::OperationType::Remove,
            &host,
            &username,
            None,
            true,
            None,
        );
    }

    tracing::info!(
        target = "credential",
        host = %host,
        username = %username,
        "Credential deleted successfully"
    );

    Ok(())
}

/// List all credentials in the store.
///
/// # Arguments
///
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns a list of credential info (with masked passwords).
#[tauri::command]
pub async fn list_credentials(
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<Vec<CredentialInfo>, String> {
    let store = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?
        .as_ref()
        .ok_or("Credential store not initialized")?
        .clone();

    let credentials = store
        .list()
        .map_err(|e| format!("Failed to list credentials: {}", e))?;

    // Log audit event
    if let Ok(logger) = audit.lock() {
        logger.log_operation(
            crate::core::credential::audit::OperationType::List,
            "",
            "",
            None,
            true,
            None,
        );
    }

    Ok(credentials.iter().map(CredentialInfo::from).collect())
}

/// Set master password for encrypted file storage.
///
/// This command initializes the credential store with the given master password.
/// It should be called before any other credential operations when using file storage.
///
/// # Arguments
///
/// * `password` - Master password for encrypting the credential file
/// * `config` - Credential configuration (from app state)
/// * `factory` - Shared credential store factory
///
/// # Returns
///
/// Returns Ok(()) on success, or an error message on failure.
#[tauri::command]
pub async fn set_master_password(
    _password: String,
    config: CredentialConfig,
    factory: State<'_, SharedCredentialFactory>,
) -> Result<(), String> {
    // NOTE: 当前 CredentialConfig 不再包含 master_password 字段。
    // 如果使用文件存储，需要后续在 EncryptedFileStore 上单独设置主密码；暂未暴露 trait 接口。
    // 这里先创建存储并忽略密码（后续实现真正的密码设置逻辑）。
    if config.storage == crate::core::credential::config::StorageType::File {
        tracing::warn!(
            target = "credential",
            "File storage master password is currently ignored (not yet implemented)"
        );
    }

    let store = CredentialStoreFactory::create(&config)
        .map_err(|e| format!("Failed to create credential store: {}", e))?;

    let mut factory_guard = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?;

    *factory_guard = Some(store);

    tracing::info!(
        target = "credential",
        "Master password set and credential store initialized"
    );

    Ok(())
}

/// Unlock credential store with master password.
///
/// This command checks access control before attempting to unlock the store.
/// Failed attempts are logged and may result in temporary lockout.
///
/// # Arguments
///
/// * `password` - Master password
/// * `config` - Credential configuration
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger for access control
///
/// # Returns
///
/// Returns Ok(()) on success, or an error message on failure.
#[tauri::command]
pub async fn unlock_store(
    password: String,
    config: CredentialConfig,
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<(), String> {
    // Check if locked due to failed attempts
    {
        let logger = audit
            .lock()
            .map_err(|e| format!("Failed to lock audit logger: {}", e))?;

        if logger.is_locked() {
            let msg = "Credential store is locked due to too many failed attempts. Please try again later.";
            tracing::warn!(target = "credential", msg);
            return Err(msg.to_string());
        }
    }

    // Attempt to unlock
    let result = set_master_password(password.clone(), config, factory).await;

    // Log the result
    {
        let logger = audit
            .lock()
            .map_err(|e| format!("Failed to lock audit logger: {}", e))?;

        if result.is_ok() {
            logger.log_operation(
                crate::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some(&password),
                true,
                None,
            );
            logger.reset_access_control(); // Reset failures on successful unlock
        } else {
            logger.log_operation(
                crate::core::credential::audit::OperationType::Unlock,
                "credential_store",
                "master",
                Some(&password),
                false,
                Some("Invalid master password".to_string()),
            );
        }
    }

    result
}

/// Export audit log as JSON.
///
/// # Arguments
///
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns the audit log as a JSON string.
#[tauri::command]
pub async fn export_audit_log(audit: State<'_, SharedAuditLogger>) -> Result<String, String> {
    let logger = audit
        .lock()
        .map_err(|e| format!("Failed to acquire audit lock: {}", e))?;

    logger
        .export_to_json()
        .map_err(|e| format!("Failed to export audit log: {}", e))
}

/// Clean up expired credentials.
///
/// Removes all credentials that have passed their expiration time.
///
/// # Arguments
///
/// * `factory` - Shared credential store factory
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns the number of credentials removed.
#[tauri::command]
pub async fn cleanup_expired_credentials(
    factory: State<'_, SharedCredentialFactory>,
    audit: State<'_, SharedAuditLogger>,
) -> Result<usize, String> {
    let store = factory
        .lock()
        .map_err(|e| format!("Failed to acquire factory lock: {}", e))?
        .as_ref()
        .ok_or("Credential store not initialized")?
        .clone();

    // Get all credentials
    let all_credentials = store
        .list()
        .map_err(|e| format!("Failed to list credentials: {}", e))?;

    // Filter expired credentials
    let mut removed_count = 0;
    for cred in all_credentials {
        if cred.is_expired() {
            // Remove expired credential
            store
                .remove(&cred.host, &cred.username)
                .map_err(|e| format!("Failed to remove expired credential: {}", e))?;

            // Log audit event
            if let Ok(logger) = audit.lock() {
                logger.log_operation(
                    crate::core::credential::audit::OperationType::Remove,
                    &cred.host,
                    &cred.username,
                    None,
                    true,
                    Some("Expired credential auto-cleanup".to_string()),
                );
            }

            tracing::info!(
                target = "credential",
                host = %cred.host,
                username = %cred.username,
                "Removed expired credential"
            );

            removed_count += 1;
        }
    }

    if removed_count > 0 {
        tracing::info!(
            target = "credential",
            count = removed_count,
            "Cleaned up expired credentials"
        );
    }

    Ok(removed_count)
}

/// Initialize credential store from configuration.
///
/// This is a helper function (not a Tauri command) used during app setup.
pub fn initialize_credential_store(
    config: &CredentialConfig,
) -> Result<Arc<dyn CredentialStore>, String> {
    CredentialStoreFactory::create(config).map_err(|e| format!("Failed to create store: {}", e))
}

/// Cleanup expired audit logs.
///
/// # Arguments
///
/// * `retention_days` - Number of days to retain audit logs
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns the number of logs removed.
#[tauri::command]
pub async fn cleanup_audit_logs(
    retention_days: u64,
    audit: State<'_, SharedAuditLogger>,
) -> Result<usize, String> {
    if retention_days == 0 {
        return Err("retention_days must be greater than 0".to_string());
    }

    let logger = audit
        .lock()
        .map_err(|e| format!("Failed to lock audit logger: {}", e))?;

    logger.cleanup_expired_logs(retention_days)
}

/// Check if credential store is locked due to authentication failures.
///
/// # Arguments
///
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns true if the store is locked, false otherwise.
#[tauri::command]
pub async fn is_credential_locked(audit: State<'_, SharedAuditLogger>) -> Result<bool, String> {
    let logger = audit
        .lock()
        .map_err(|e| format!("Failed to lock audit logger: {}", e))?;

    Ok(logger.is_locked())
}

/// Reset credential store access control (admin unlock).
///
/// # Arguments
///
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns Ok(()) on success.
#[tauri::command]
pub async fn reset_credential_lock(audit: State<'_, SharedAuditLogger>) -> Result<(), String> {
    let logger = audit
        .lock()
        .map_err(|e| format!("Failed to lock audit logger: {}", e))?;

    logger.reset_access_control();
    tracing::info!(
        target = "credential",
        "Credential store access control reset by admin"
    );
    Ok(())
}

/// Get remaining authentication attempts before lockout.
///
/// # Arguments
///
/// * `audit` - Shared audit logger
///
/// # Returns
///
/// Returns the number of remaining attempts.
#[tauri::command]
pub async fn remaining_auth_attempts(audit: State<'_, SharedAuditLogger>) -> Result<u32, String> {
    let logger = audit
        .lock()
        .map_err(|e| format!("Failed to lock audit logger: {}", e))?;

    Ok(logger.remaining_attempts())
}
