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
        
        match SecretService::connect(secret_service::EncryptionType::Dh) {
            Ok(connection) => Ok(UnixCredentialStore { connection }),
            Err(e) => Err(format!("Linux Secret Service unavailable: {}", e)),
        }
    }

    /// Creates a new Unix credential store (other Unix).
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn new() -> Result<Self, String> {
        Err("Unix keychain not supported on this platform".to_string())
    }

    /// Makes an account name from host and username.
    fn make_account(host: &str, username: &str) -> String {
        format!("{}:{}:{}", ACCOUNT_PREFIX, host, username)
    }

    /// Parses host and username from account name.
    fn parse_account(account: &str) -> Option<(String, String)> {
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

        let username = username.ok_or_else(|| CredentialStoreError::Other("Username required for macOS keychain".to_string()))?;
        let account = Self::make_account(host, username);

        match find_generic_password(None, SERVICE_NAME, &account) {
            Ok((password_bytes, _)) => {
                let password = String::from_utf8_lossy(&password_bytes).to_string();
                Ok(Some(Credential::new(
                    host.to_string(),
                    username.to_string(),
                    password,
                    None,
                )))
            }
            Err(e) => {
                // Check if error is "item not found"
                if e.to_string().contains("errSecItemNotFound") {
                    Ok(None)
                } else {
                    Err(format!("Failed to read from macOS keychain: {}", e))
                }
            }
        }
    }

    fn add(&self, credential: &Credential) -> Result<(), String> {
        use security_framework::os::macos::passwords::{set_generic_password, delete_generic_password};

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
        .map_err(|e| format!("Failed to write to macOS keychain: {}", e))
    }

    fn remove(&self, host: &str, username: &str) -> Result<(), String> {
        use security_framework::os::macos::passwords::delete_generic_password;

        let account = Self::make_account(host, username);
        
        match delete_generic_password(None, SERVICE_NAME, &account) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Item not found is OK
                if e.to_string().contains("errSecItemNotFound") {
                    Ok(())
                } else {
                    Err(format!("Failed to delete from macOS keychain: {}", e))
                }
            }
        }
    }

    fn list(&self) -> Result<Vec<Credential>, String> {
        // macOS Security Framework doesn't provide a direct way to enumerate all generic passwords
        // This is a limitation of the API - we return empty list for now
        // In a real implementation, we might maintain a separate index
        tracing::warn!("list() not fully supported on macOS keychain - returning empty list");
        Ok(Vec::new())
    }
}

#[cfg(target_os = "linux")]
impl CredentialStore for UnixCredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> Result<Option<Credential>, String> {
        use secret_service::Collection;
        
        let username = username.ok_or_else(|| "Username required for Linux Secret Service".to_string())?;
        let account = Self::make_account(host, username);

        let collection = Collection::default(&self.connection)
            .map_err(|e| format!("Failed to access default collection: {}", e))?;

        let search_items = collection
            .search_items(vec![("service", SERVICE_NAME), ("account", &account)])
            .map_err(|e| format!("Failed to search items: {}", e))?;

        if search_items.is_empty() {
            return Ok(None);
        }

        let item = &search_items[0];
        let secret = item
            .get_secret()
            .map_err(|e| format!("Failed to get secret: {}", e))?;
        
        let password = String::from_utf8_lossy(&secret).to_string();

        Ok(Some(Credential::new(
            host.to_string(),
            username.to_string(),
            password,
            None,
        )))
    }

    fn add(&self, credential: &Credential) -> Result<(), String> {
        use secret_service::Collection;
        
        let account = Self::make_account(&credential.host, &credential.username);
        let collection = Collection::default(&self.connection)
            .map_err(|e| format!("Failed to access default collection: {}", e))?;

        // Delete existing item if any
        let _ = self.remove(&credential.host, &credential.username);

        // Create new item
        collection
            .create_item(
                &format!("Git credential for {}@{}", credential.username, credential.host),
                vec![("service", SERVICE_NAME), ("account", &account)],
                credential.password_or_token.as_bytes(),
                true, // replace existing
                "text/plain",
            )
            .map_err(|e| format!("Failed to create item: {}", e))?;

        Ok(())
    }

    fn remove(&self, host: &str, username: &str) -> Result<(), String> {
        use secret_service::Collection;
        
        let account = Self::make_account(host, username);
        let collection = Collection::default(&self.connection)
            .map_err(|e| format!("Failed to access default collection: {}", e))?;

        let search_items = collection
            .search_items(vec![("service", SERVICE_NAME), ("account", &account)])
            .map_err(|e| format!("Failed to search items: {}", e))?;

        for item in search_items {
            item.delete()
                .map_err(|e| format!("Failed to delete item: {}", e))?;
        }

        Ok(())
    }

    fn list(&self) -> Result<Vec<Credential>, String> {
        use secret_service::Collection;
        
        let collection = Collection::default(&self.connection)
            .map_err(|e| format!("Failed to access default collection: {}", e))?;

        let all_items = collection
            .get_all_items()
            .map_err(|e| format!("Failed to get all items: {}", e))?;

        let mut credentials = Vec::new();

        for item in all_items {
            let attributes = item
                .get_attributes()
                .map_err(|e| format!("Failed to get attributes: {}", e))?;

            // Check if this is our item
            if let Some(service) = attributes.get("service") {
                if service == SERVICE_NAME {
                    if let Some(account) = attributes.get("account") {
                        if let Some((host, username)) = Self::parse_account(account) {
                            let secret = item
                                .get_secret()
                                .map_err(|e| format!("Failed to get secret: {}", e))?;
                            
                            let password = String::from_utf8_lossy(&secret).to_string();
                            
                            credentials.push(Credential::new(
                                host,
                                username,
                                password,
                                None,
                            ));
                        }
                    }
                }
            }
        }

        Ok(credentials)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl CredentialStore for UnixCredentialStore {
    fn get(&self, _host: &str, _username: Option<&str>) -> Result<Option<Credential>, String> {
        Err("Unix keychain not supported on this platform".to_string())
    }

    fn add(&self, _credential: &Credential) -> Result<(), String> {
        Err("Unix keychain not supported on this platform".to_string())
    }

    fn remove(&self, _host: &str, _username: &str) -> Result<(), String> {
        Err("Unix keychain not supported on this platform".to_string())
    }

    fn list(&self) -> Result<Vec<Credential>, String> {
        Err("Unix keychain not supported on this platform".to_string())
    }
}

