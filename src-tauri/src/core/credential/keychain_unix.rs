//! Unix (macOS/Linux) keychain integration.
//!
//! This module provides credential storage using platform-specific keychains:
//! - macOS: Keychain via security-framework
//! - Linux: Secret Service via secret-service crate

use super::{
    model::Credential,
    storage::{CredentialStore, CredentialStoreError, CredentialStoreResult},
};

const SERVICE_NAME: &str = "fireworks-collaboration";
const ACCOUNT_PREFIX: &str = "git";

/// Unix credential store implementation.
pub struct UnixCredentialStore {
    #[cfg(target_os = "macos")]
    _phantom: std::marker::PhantomData<()>,
    #[cfg(target_os = "linux")]
    connection: secret_service::SecretService<'static>,
    #[cfg(target_os = "linux")]
    runtime: std::sync::Arc<tokio::runtime::Runtime>,
}

impl UnixCredentialStore {
    /// Creates a new Unix credential store.
    #[cfg(target_os = "macos")]
    pub fn new() -> Result<Self, String> {
        // Check if security framework is available
        // by attempting to access the keychain
        use security_framework::os::macos::keychain::SecKeychain;

        match SecKeychain::default() {
            Ok(_) => Ok(UnixCredentialStore {
                _phantom: std::marker::PhantomData,
            }),
            Err(e) => Err(format!("macOS Keychain unavailable: {}", e)),
        }
    }

    /// Creates a new Unix credential store (Linux).
    #[cfg(target_os = "linux")]
    pub fn new() -> Result<Self, String> {
        use secret_service::SecretService;
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime for Secret Service: {e}"))?;
        let connection = runtime
            .block_on(SecretService::connect(secret_service::EncryptionType::Dh))
            .map_err(|e| format!("Linux Secret Service unavailable: {e}"))?;
        Ok(UnixCredentialStore {
            connection,
            runtime: std::sync::Arc::new(runtime),
        })
    }

    /// Creates a new Unix credential store (other Unix).
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn new() -> Result<Self, String> {
        Err("Unix keychain not supported on this platform".to_string())
    }

    /// 构造账户标识 (host + username) 的内部辅助函数。
    ///
    /// 注意: 仅因集成测试 (`tests/credential/unit_tests.rs`) 需要验证格式/解析而暴露。
    /// 这不是稳定公共 API，后续实现中可能随时调整；业务代码请勿依赖。
    pub fn make_account(host: &str, username: &str) -> String {
        format!("{}:{}:{}", ACCOUNT_PREFIX, host, username)
    }

    /// 解析 `make_account` 生成的账户标识，返回 (host, username)。
    /// 仅测试使用，非稳定公共 API。
    #[doc(hidden)]
    pub fn parse_account(account: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = account.split(':').collect();
        if parts.len() == 3 && parts[0] == ACCOUNT_PREFIX {
            Some((parts[1].to_string(), parts[2].to_string()))
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
impl CredentialStore for UnixCredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>> {
        use security_framework::os::macos::keychain::SecKeychain;
        use security_framework::os::macos::passwords::find_generic_password;

        let username = username.ok_or_else(|| {
            CredentialStoreError::Other("Username required for macOS keychain".to_string())
        })?;
        let account = Self::make_account(host, username);

        match find_generic_password(None, SERVICE_NAME, &account) {
            Ok((password_bytes, _)) => {
                let password = String::from_utf8_lossy(&password_bytes).to_string();
                Ok(Some(Credential::new(
                    host.to_string(),
                    username.to_string(),
                    password,
                )))
            }
            Err(e) => {
                if e.to_string().contains("errSecItemNotFound") {
                    Ok(None)
                } else {
                    Err(CredentialStoreError::AccessError(format!(
                        "Failed to read from macOS keychain: {e}"
                    )))
                }
            }
        }
    }
    fn add(&self, credential: Credential) -> CredentialStoreResult<()> {
        use security_framework::os::macos::passwords::{
            delete_generic_password, set_generic_password,
        };

        let account = Self::make_account(&credential.host, &credential.username);

        // Delete existing credential if any (update)
        let _ = delete_generic_password(None, SERVICE_NAME, &account);

        // Add new credential
        set_generic_password(
            None,
            SERVICE_NAME,
            &account,
            credential.password_or_token.as_bytes(),
        )
        .map_err(|e| {
            CredentialStoreError::AccessError(format!("Failed to write to macOS keychain: {e}"))
        })
    }
    fn remove(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        use security_framework::os::macos::passwords::delete_generic_password;

        let account = Self::make_account(host, username);

        match delete_generic_password(None, SERVICE_NAME, &account) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Item not found is OK
                if e.to_string().contains("errSecItemNotFound") {
                    Ok(())
                } else {
                    Err(CredentialStoreError::AccessError(format!(
                        "Failed to delete from macOS keychain: {e}"
                    )))
                }
            }
        }
    }
    fn list(&self) -> CredentialStoreResult<Vec<Credential>> {
        // macOS Security Framework doesn't provide a direct way to enumerate all generic passwords
        // This is a limitation of the API - we return empty list for now
        // In a real implementation, we might maintain a separate index
        tracing::warn!("list() not fully supported on macOS keychain - returning empty list");
        Ok(Vec::new())
    }

    fn list_all(&self) -> CredentialStoreResult<Vec<Credential>> {
        self.list()
    }

    fn update_last_used(&self, _host: &str, _username: &str) -> CredentialStoreResult<()> {
        // Keychain 不提供 last_used_at 元数据，这里直接返回 Ok
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl CredentialStore for UnixCredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>> {
        use std::collections::HashMap;

        // Build attribute map. We always include host; optionally username.
        let mut query: HashMap<&str, String> = HashMap::new();
        query.insert("host", host.to_string());
        if let Some(u) = username {
            query.insert("username", u.to_string());
        }

        // Convert to HashMap<&str, &str> for API call (collect references lifetime limited to this scope)
        let attr: HashMap<&str, &str> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let result = self
            .runtime
            .block_on(self.connection.search_items(attr))
            .map_err(|e| {
                CredentialStoreError::AccessError(format!("Secret Service search error: {e}"))
            })?;

        // Prefer unlocked items; if first locked exists attempt unlock (operate on references, no clone needed)
        let mut item_ref: Option<&secret_service::Item<'_>> = result.unlocked.first();
        if item_ref.is_none() {
            if let Some(locked) = result.locked.first() {
                let _ = self.runtime.block_on(locked.unlock());
                // After unlock we still hold reference
                item_ref = Some(locked);
            }
        }

        if let Some(item) = item_ref {
            let secret = self.runtime.block_on(item.get_secret()).map_err(|e| {
                CredentialStoreError::AccessError(format!("Secret retrieval failed: {e}"))
            })?;
            // Reconstruct username if not provided, via attributes
            let username_val = username
                .map(|s| s.to_string())
                .or_else(|| {
                    // Attempt to fetch attributes if username was not specified.
                    self.runtime
                        .block_on(item.get_attributes())
                        .ok()
                        .and_then(|attrs| attrs.get("username").map(|s| s.to_string()))
                })
                .unwrap_or_default();
            let cred = Credential::new(
                host.to_string(),
                username_val,
                String::from_utf8_lossy(&secret).to_string(),
            );
            Ok(Some(cred))
        } else {
            Ok(None)
        }
    }

    fn add(&self, credential: Credential) -> CredentialStoreResult<()> {
        use std::collections::HashMap;
        // First check existence to mimic AlreadyExists semantics
        if self
            .get(&credential.host, Some(&credential.username))?
            .is_some()
        {
            return Err(CredentialStoreError::AlreadyExists(format!(
                "{}:{}",
                credential.host, credential.username
            )));
        }
        let collection = self
            .runtime
            .block_on(self.connection.get_default_collection())
            .map_err(|e| {
                CredentialStoreError::AccessError(format!("Secret Service collection error: {e}"))
            })?;
        let mut props: HashMap<&str, &str> = HashMap::new();
        props.insert("service", SERVICE_NAME);
        props.insert("host", &credential.host);
        props.insert("username", &credential.username);
        let label = format!("{SERVICE_NAME}:{}:{}", credential.host, credential.username);
        self.runtime
            .block_on(collection.create_item(
                &label,
                props,
                credential.password_or_token.as_bytes(),
                false, // do not replace
                "text/plain",
            ))
            .map_err(|e| CredentialStoreError::AccessError(format!("Create item failed: {e}")))?;
        Ok(())
    }

    fn remove(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        use std::collections::HashMap;
        let attrs: HashMap<&str, &str> = HashMap::from([("host", host), ("username", username)]);
        let result = self
            .runtime
            .block_on(self.connection.search_items(attrs))
            .map_err(|e| {
                CredentialStoreError::AccessError(format!("Secret Service search error: {e}"))
            })?;
        // Work with references; unlock locked item if chosen
        let mut item_ref: Option<&secret_service::Item<'_>> = result.unlocked.first();
        if item_ref.is_none() {
            if let Some(locked) = result.locked.first() {
                let _ = self.runtime.block_on(locked.unlock());
                item_ref = Some(locked);
            }
        }
        if let Some(item) = item_ref {
            self.runtime
                .block_on(item.delete())
                .map_err(|e| CredentialStoreError::AccessError(format!("Delete failed: {e}")))?;
            Ok(())
        } else {
            Err(CredentialStoreError::NotFound(format!(
                "{}:{}",
                host, username
            )))
        }
    }

    fn list(&self) -> CredentialStoreResult<Vec<Credential>> {
        // Secret Service API doesn't provide a direct "list all" without knowing attributes.
        // Returning empty list keeps semantics consistent with macOS implementation.
        tracing::warn!("list() not implemented for Secret Service - returning empty list");
        Ok(Vec::new())
    }

    fn list_all(&self) -> CredentialStoreResult<Vec<Credential>> {
        self.list()
    }

    fn update_last_used(&self, _host: &str, _username: &str) -> CredentialStoreResult<()> {
        // No last-used metadata maintained; noop
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl CredentialStore for UnixCredentialStore {
    fn get(
        &self,
        _host: &str,
        _username: Option<&str>,
    ) -> CredentialStoreResult<Option<Credential>> {
        Err(CredentialStoreError::Other(
            "Unix keychain not supported on this platform".to_string(),
        ))
    }

    fn add(&self, _credential: Credential) -> CredentialStoreResult<()> {
        Err(CredentialStoreError::Other(
            "Unix keychain not supported on this platform".to_string(),
        ))
    }

    fn remove(&self, _host: &str, _username: &str) -> CredentialStoreResult<()> {
        Err(CredentialStoreError::Other(
            "Unix keychain not supported on this platform".to_string(),
        ))
    }

    fn list(&self) -> CredentialStoreResult<Vec<Credential>> {
        Err(CredentialStoreError::Other(
            "Unix keychain not supported on this platform".to_string(),
        ))
    }

    fn list_all(&self) -> CredentialStoreResult<Vec<Credential>> {
        self.list()
    }

    fn update_last_used(&self, _host: &str, _username: &str) -> CredentialStoreResult<()> {
        Ok(())
    }
}
