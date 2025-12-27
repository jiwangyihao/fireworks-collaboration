use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

use fireworks_collaboration_lib::core::credential::{
    config::CredentialConfig, file_store::EncryptedFileStore, model::Credential,
    storage::CredentialStore,
};

#[test]
fn test_credential_store_creation_failure_on_readonly_directory() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("creds.enc");

    // Make directory read-only (Windows specific trick might be needed or just use open file lock).
    // On Windows, directories are tricky. Let's try creating a file that is read-only then try to overwrite/create in it?
    // Actually, `EncryptedFileStore::new` tries to create/probe the file.
    // Let's create the file first, make it read-only.
    {
        let _ = fs::File::create(&file_path).unwrap();
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&file_path, perms).unwrap();
    }

    let config = CredentialConfig::new().with_file_path(file_path.to_string_lossy().to_string());

    // Attempt to creating store should fail at probe stage or subsequent write
    // The current implementation attempts to open with `.write(true)`.
    // If file is existing and read-only, open with write should fail.
    let result = EncryptedFileStore::new(&config);
    assert!(
        result.is_err(),
        "Should fail to create store on read-only file"
    );
}

#[test]
fn test_credential_store_concurrent_writes() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("concurrent_creds.enc");

    let config = CredentialConfig::new().with_file_path(file_path.to_string_lossy().to_string());
    let store = Arc::new(EncryptedFileStore::new(&config).unwrap());
    store
        .set_master_password("master-password".to_string())
        .unwrap();

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = store.clone();
        let barrier_clone = barrier.clone();
        handles.push(thread::spawn(move || {
            barrier_clone.wait();
            let cred = Credential::new(
                format!("host-{}", i),
                format!("user-{}", i),
                "token".to_string(),
            );
            store_clone.add(cred).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all 10 are present
    let list = store.list().unwrap();
    assert_eq!(list.len(), 10, "Should have 10 credentials");

    // Verify persistence by reloading
    drop(store);
    let store_reloaded = EncryptedFileStore::new(&config).unwrap();
    store_reloaded
        .set_master_password("master-password".to_string())
        .unwrap();
    let list_reloaded = store_reloaded.list().unwrap();
    assert_eq!(
        list_reloaded.len(),
        10,
        "Should have 10 credentials after reload"
    );
}
