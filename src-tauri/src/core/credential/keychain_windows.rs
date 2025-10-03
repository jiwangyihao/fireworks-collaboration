//! Windows Credential Manager integration.
//!
//! This module provides credential storage using Windows Credential Manager API.

use super::{
    model::Credential,
    storage::{CredentialStore, CredentialStoreError, CredentialStoreResult},
};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use winapi::um::wincred::{
    CredDeleteW, CredEnumerateW, CredFree, CredReadW, CredWriteW, CREDENTIALW,
    CRED_ENUMERATE_ALL_CREDENTIALS, CRED_MAX_CREDENTIAL_BLOB_SIZE, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC, PCREDENTIALW,
};

const TARGET_PREFIX: &str = "fireworks-collaboration:git:";

/// Windows Credential Manager store implementation.
pub struct WindowsCredentialStore;

impl WindowsCredentialStore {
    /// Creates a new Windows credential store.
    pub fn new() -> Result<Self, String> {
        // Check if we can access Windows Credential Manager
        // by attempting to enumerate credentials
        unsafe {
            let mut count: u32 = 0;
            let mut credentials: *mut PCREDENTIALW = ptr::null_mut();
            let result = CredEnumerateW(
                ptr::null(),
                CRED_ENUMERATE_ALL_CREDENTIALS,
                &mut count,
                &mut credentials,
            );
            
            if !credentials.is_null() {
                CredFree(credentials as *mut _);
            }

            if result == 0 {
                let error_code = winapi::um::errhandlingapi::GetLastError();
                // ERROR_NOT_FOUND (1168) is OK - means no credentials stored yet
                if error_code != 1168 {
                    return Err(format!(
                        "Windows Credential Manager unavailable, error code: {}",
                        error_code
                    ));
                }
            }
        }

        Ok(WindowsCredentialStore)
    }

    /// Converts a host and username to a Windows credential target name.
    fn make_target_name(host: &str, username: &str) -> String {
        format!("{}{}:{}", TARGET_PREFIX, host, username)
    }

    /// Converts a string to Windows wide string (UTF-16).
    fn to_wide_string(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    /// Converts a Windows wide string to Rust string.
    fn from_wide_ptr(ptr: *const u16) -> String {
        if ptr.is_null() {
            return String::new();
        }
        unsafe {
            let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
            let slice = std::slice::from_raw_parts(ptr, len);
            String::from_utf16_lossy(slice)
        }
    }
}

impl CredentialStore for WindowsCredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>> {
        let username = username.ok_or_else(|| CredentialStoreError::Other("Username required for Windows keychain".to_string()))?;
        
        let target_name = Self::make_target_name(host, username);
        let target_wide = Self::to_wide_string(&target_name);

        unsafe {
            let mut credential: PCREDENTIALW = ptr::null_mut();
            let result = CredReadW(
                target_wide.as_ptr(),
                CRED_TYPE_GENERIC,
                0,
                &mut credential,
            );

            if result == 0 {
                // Credential not found
                return Ok(None);
            }

            if credential.is_null() {
                return Err(CredentialStoreError::AccessError(
                    "Failed to read credential from Windows Credential Manager".to_string()
                ));
            }

            let cred_ref = &*credential;
            let password_blob = std::slice::from_raw_parts(
                cred_ref.CredentialBlob,
                cred_ref.CredentialBlobSize as usize,
            );
            let password = String::from_utf8_lossy(password_blob).to_string();

            let credential_obj = Credential::new(
                host.to_string(),
                username.to_string(),
                password,
            );

            CredFree(credential as *mut _);

            Ok(Some(credential_obj))
        }
    }

    fn add(&self, credential: Credential) -> CredentialStoreResult<()> {
        let target_name = Self::make_target_name(&credential.host, &credential.username);
        let target_wide = Self::to_wide_string(&target_name);
        let username_wide = Self::to_wide_string(&credential.username);
        
        let password_bytes = credential.password_or_token.as_bytes();
        if password_bytes.len() > CRED_MAX_CREDENTIAL_BLOB_SIZE as usize {
            return Err(CredentialStoreError::Other(format!(
                "Password too long ({} bytes, max {})",
                password_bytes.len(),
                CRED_MAX_CREDENTIAL_BLOB_SIZE
            )));
        }

        unsafe {
            let mut cred = CREDENTIALW {
                Flags: 0,
                Type: CRED_TYPE_GENERIC,
                TargetName: target_wide.as_ptr() as *mut _,
                Comment: ptr::null_mut(),
                LastWritten: std::mem::zeroed(),
                CredentialBlobSize: password_bytes.len() as u32,
                CredentialBlob: password_bytes.as_ptr() as *mut _,
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                AttributeCount: 0,
                Attributes: ptr::null_mut(),
                TargetAlias: ptr::null_mut(),
                UserName: username_wide.as_ptr() as *mut _,
            };

            let result = CredWriteW(&mut cred, 0);
            if result == 0 {
                let error_code = winapi::um::errhandlingapi::GetLastError();
                return Err(CredentialStoreError::AccessError(format!(
                    "Failed to write credential to Windows Credential Manager, error code: {}",
                    error_code
                )));
            }
        }

        Ok(())
    }

    fn remove(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        let target_name = Self::make_target_name(host, username);
        let target_wide = Self::to_wide_string(&target_name);

        unsafe {
            let result = CredDeleteW(target_wide.as_ptr(), CRED_TYPE_GENERIC, 0);
            if result == 0 {
                let error_code = winapi::um::errhandlingapi::GetLastError();
                // ERROR_NOT_FOUND is OK - credential already deleted
                if error_code != 1168 {
                    return Err(CredentialStoreError::AccessError(format!(
                        "Failed to delete credential from Windows Credential Manager, error code: {}",
                        error_code
                    )));
                }
            }
        }

        Ok(())
    }

    fn list(&self) -> CredentialStoreResult<Vec<Credential>> {
        let mut credentials = Vec::new();

        unsafe {
            let mut count: u32 = 0;
            let mut cred_array: *mut PCREDENTIALW = ptr::null_mut();
            
            // Enumerate all credentials (filter is not working reliably)
            let result = CredEnumerateW(
                ptr::null(),
                CRED_ENUMERATE_ALL_CREDENTIALS,
                &mut count,
                &mut cred_array,
            );
            
            if result == 0 {
                let error_code = winapi::um::errhandlingapi::GetLastError();
                // ERROR_NOT_FOUND means no credentials - return empty list
                if error_code == 1168 {
                    return Ok(credentials);
                }
                return Err(CredentialStoreError::AccessError(format!(
                    "Failed to enumerate credentials from Windows Credential Manager, error code: {}",
                    error_code
                )));
            }

            if !cred_array.is_null() {
                for i in 0..count as isize {
                    let cred = *cred_array.offset(i);
                    if !cred.is_null() {
                        let cred_ref = &*cred;
                        
                        // Only include generic credentials
                        if cred_ref.Type != CRED_TYPE_GENERIC {
                            continue;
                        }
                        
                        let mut target_name = Self::from_wide_ptr(cred_ref.TargetName);
                        
                        // Windows may add prefix like "LegacyGeneric:target="
                        if let Some(stripped) = target_name.strip_prefix("LegacyGeneric:target=") {
                            target_name = stripped.to_string();
                        }
                        
                        // Parse host and username from target name (filter manually)
                        if let Some(suffix) = target_name.strip_prefix(TARGET_PREFIX) {
                            if let Some((host, username)) = suffix.split_once(':') {
                                let password_blob = std::slice::from_raw_parts(
                                    cred_ref.CredentialBlob,
                                    cred_ref.CredentialBlobSize as usize,
                                );
                                let password = String::from_utf8_lossy(password_blob).to_string();

                                credentials.push(Credential::new(
                                    host.to_string(),
                                    username.to_string(),
                                    password,
                                ));
                            }
                        }
                    }
                }

                CredFree(cred_array as *mut _);
            }
        }

        Ok(credentials)
    }

    fn update_last_used(&self, _host: &str, _username: &str) -> CredentialStoreResult<()> {
        // Windows Credential Manager doesn't support updating last used time
        // This is a no-op for Windows
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_store_creation() {
        // Should succeed or fail gracefully
        let result = WindowsCredentialStore::new();
        match result {
            Ok(_) => println!("Windows Credential Manager available"),
            Err(e) => println!("Windows Credential Manager unavailable: {}", e),
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_store_add_get_remove() {
        let store = match WindowsCredentialStore::new() {
            Ok(s) => s,
            Err(_) => {
                println!("Skipping test - Windows Credential Manager unavailable");
                return;
            }
        };

        let host = "github.com";
        let username = "test_user_windows";
        let password = "test_password_12345";

        // Clean up any existing credential
        let _ = store.remove(host, username);

        // Add credential
        let cred = Credential::new(
            host.to_string(),
            username.to_string(),
            password.to_string(),
        );
        assert!(store.add(cred).is_ok());

        // Get credential
        let retrieved = store.get(host, Some(username)).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.host, host);
        assert_eq!(retrieved.username, username);
        assert_eq!(retrieved.password_or_token, password);

        // Remove credential
        assert!(store.remove(host, username).is_ok());

        // Verify removed
        let after_remove = store.get(host, Some(username)).unwrap();
        assert!(after_remove.is_none());
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_store_list() {
        let store = match WindowsCredentialStore::new() {
            Ok(s) => s,
            Err(_) => {
                println!("Skipping test - Windows Credential Manager unavailable");
                return;
            }
        };

        // Add test credentials
        let creds = vec![
            Credential::new(
                "github.com".to_string(),
                "user1".to_string(),
                "pass1".to_string(),
            ),
            Credential::new(
                "gitlab.com".to_string(),
                "user2".to_string(),
                "pass2".to_string(),
            ),
        ];

        // Clean up first
        for cred in &creds {
            let _ = store.remove(&cred.host, &cred.username);
        }

        // Add credentials
        for cred in creds.clone() {
            assert!(store.add(cred).is_ok());
        }

        // List credentials
        let list = store.list().unwrap();
        // Check that our credentials are in the list
        // (may include other credentials too)
        let our_creds: Vec<_> = list.iter()
            .filter(|c| {
                (c.host == "github.com" && c.username == "user1") ||
                (c.host == "gitlab.com" && c.username == "user2")
            })
            .collect();
        assert!(our_creds.len() >= 2, "Expected at least 2 credentials, found {}", our_creds.len());

        // Clean up
        for cred in &creds {
            let _ = store.remove(&cred.host, &cred.username);
        }
    }
}
