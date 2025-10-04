//! 凭证存储平台集成测试
//!
//! 测试 `CredentialStoreFactory` 创建的实际存储实现，
//! 包括系统钥匙串、加密文件存储和自动回退机制。

use fireworks_collaboration_lib::core::credential::{
    config::{CredentialConfig, StorageType},
    factory::CredentialStoreFactory,
    file_store::EncryptedFileStore,
    model::Credential,
    storage::CredentialStore,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// 获取测试用的临时文件路径
fn get_test_file_path(test_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fireworks_cred_test_{test_name}.enc"))
}

/// 清理测试文件
fn cleanup_test_file(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

/// 创建带主密码的文件存储（辅助函数）
fn create_file_store_with_password(
    file_path: PathBuf,
    password: &str,
) -> Result<Arc<dyn CredentialStore>, String> {
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config)?;
    store.set_master_password(password.to_string())?;

    Ok(Arc::new(store) as Arc<dyn CredentialStore>)
}

#[test]
fn test_factory_creates_memory_store() {
    let config = CredentialConfig::new().with_storage(StorageType::Memory);

    let store = CredentialStoreFactory::create(&config).expect("应该创建内存存储");

    // 测试基本操作
    let cred = Credential::new(
        "test.com".to_string(),
        "user1".to_string(),
        "pass1".to_string(),
    );

    assert!(store.add(cred).is_ok());
    assert!(store.get("test.com", Some("user1")).unwrap().is_some());
    assert!(store.remove("test.com", "user1").is_ok());
}

#[test]
fn test_factory_creates_file_store() {
    let test_file = get_test_file_path("factory_file_store");
    cleanup_test_file(&test_file);

    let store = create_file_store_with_password(test_file.clone(), "test_password_123")
        .expect("应该创建文件存储");

    // 测试基本操作
    let cred = Credential::new(
        "test.com".to_string(),
        "user2".to_string(),
        "pass2".to_string(),
    );

    assert!(store.add(cred).is_ok());
    assert!(store.get("test.com", Some("user2")).unwrap().is_some());

    // 验证文件已创建
    assert!(test_file.exists(), "加密文件应该已创建");

    // 清理
    assert!(store.remove("test.com", "user2").is_ok());
    cleanup_test_file(&test_file);
}

#[test]
fn test_file_store_persistence() {
    let test_file = get_test_file_path("persistence");
    cleanup_test_file(&test_file);

    let password = "persistent_pass";

    // 第一次创建存储并添加凭证
    {
        let store = create_file_store_with_password(test_file.clone(), password).unwrap();
        let cred = Credential::new(
            "github.com".to_string(),
            "alice".to_string(),
            "ghp_token_123".to_string(),
        );
        store.add(cred).unwrap();
    }

    // 第二次创建存储（模拟程序重启）
    {
        let store = create_file_store_with_password(test_file.clone(), password).unwrap();
        let retrieved = store.get("github.com", Some("alice")).unwrap();

        assert!(retrieved.is_some(), "凭证应该持久化");
        let cred = retrieved.unwrap();
        assert_eq!(cred.host, "github.com");
        assert_eq!(cred.username, "alice");
        assert_eq!(cred.password_or_token, "ghp_token_123");

        // 清理
        store.remove("github.com", "alice").unwrap();
    }

    cleanup_test_file(&test_file);
}

#[test]
fn test_file_store_wrong_password() {
    let test_file = get_test_file_path("wrong_password");
    cleanup_test_file(&test_file);

    // 使用正确密码创建并添加凭证
    {
        let store = create_file_store_with_password(test_file.clone(), "correct_password").unwrap();
        let cred = Credential::new(
            "test.com".to_string(),
            "user".to_string(),
            "secret".to_string(),
        );
        store.add(cred).unwrap();
    }

    // 使用错误密码尝试读取
    let store = create_file_store_with_password(test_file.clone(), "wrong_password").unwrap();
    let result = store.get("test.com", Some("user"));

    // 应该失败（解密错误或找不到凭证）
    assert!(
        result.is_err() || result.unwrap().is_none(),
        "错误密码不应该能读取凭证"
    );

    cleanup_test_file(&test_file);
}

#[test]
fn test_file_store_encryption_randomness() {
    let test_file1 = get_test_file_path("encryption1");
    let test_file2 = get_test_file_path("encryption2");
    cleanup_test_file(&test_file1);
    cleanup_test_file(&test_file2);

    let password = "same_password_123";
    let cred_data = ("test.com", "user", "secret_token");

    // 创建两个独立的加密文件
    {
        let store1 = create_file_store_with_password(test_file1.clone(), password).unwrap();
        let cred1 = Credential::new(
            cred_data.0.to_string(),
            cred_data.1.to_string(),
            cred_data.2.to_string(),
        );
        store1.add(cred1).unwrap();
    }

    {
        let store2 = create_file_store_with_password(test_file2.clone(), password).unwrap();
        let cred2 = Credential::new(
            cred_data.0.to_string(),
            cred_data.1.to_string(),
            cred_data.2.to_string(),
        );
        store2.add(cred2).unwrap();
    }

    // 读取文件内容
    let content1 = fs::read(&test_file1).unwrap();
    let content2 = fs::read(&test_file2).unwrap();

    // 即使密码和数据相同，加密后的内容应该不同（因为随机 IV 和盐）
    assert_ne!(content1, content2, "加密应该使用随机 IV，导致密文不同");

    cleanup_test_file(&test_file1);
    cleanup_test_file(&test_file2);
}

#[test]
fn test_multiple_credentials_in_file_store() {
    let test_file = get_test_file_path("multiple_creds");
    cleanup_test_file(&test_file);

    let store = create_file_store_with_password(test_file.clone(), "multi_cred_pass").unwrap();

    // 添加多个凭证
    let credentials = vec![
        ("github.com", "alice", "token1"),
        ("github.com", "bob", "token2"),
        ("gitlab.com", "alice", "token3"),
    ];

    for (host, username, token) in &credentials {
        let cred = Credential::new(host.to_string(), username.to_string(), token.to_string());
        store.add(cred).unwrap();
    }

    // 验证列表
    let list = store.list().unwrap();
    assert_eq!(list.len(), 3, "应该有 3 个凭证");

    // 验证每个凭证都能正确读取
    for (host, username, expected_token) in &credentials {
        let cred = store.get(host, Some(username)).unwrap().unwrap();
        assert_eq!(cred.password_or_token, *expected_token);
    }

    // 删除一个凭证
    store.remove("github.com", "alice").unwrap();
    let list = store.list().unwrap();
    assert_eq!(list.len(), 2, "删除后应该剩余 2 个凭证");

    cleanup_test_file(&test_file);
}

#[cfg(target_os = "windows")]
#[test]
fn test_factory_creates_system_store_windows() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::wincred::CredDeleteW;

    let config = CredentialConfig::new().with_storage(StorageType::System);

    let store = CredentialStoreFactory::create(&config);
    assert!(store.is_ok(), "Windows 上应该能创建系统凭证存储");

    let store = store.unwrap();

    // 测试基本操作
    let test_host = "fireworks.test.integration";
    let test_username = "testuser";
    let test_password = "testpass123";

    let cred = Credential::new(
        test_host.to_string(),
        test_username.to_string(),
        test_password.to_string(),
    );

    // 添加凭证
    assert!(
        store.add(cred).is_ok(),
        "应该能添加凭证到 Windows 凭据管理器"
    );

    // 读取凭证
    let retrieved = store.get(test_host, Some(test_username)).unwrap();
    assert!(retrieved.is_some(), "应该能从 Windows 凭据管理器读取凭证");

    let retrieved_cred = retrieved.unwrap();
    assert_eq!(retrieved_cred.host, test_host);
    assert_eq!(retrieved_cred.username, test_username);
    assert_eq!(retrieved_cred.password_or_token, test_password);

    // 清理（使用 Windows API 直接删除，确保清理）
    store.remove(test_host, test_username).ok();

    // 再次尝试直接删除（防止泄漏）
    unsafe {
        let target_name: Vec<u16> = OsStr::new(test_host).encode_wide().chain(Some(0)).collect();
        CredDeleteW(target_name.as_ptr(), 1, 0);
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn test_factory_fallback_to_file_on_non_windows() {
    // 请求系统存储，但在非 Windows 平台上可能会回退到文件存储
    let config = CredentialConfig::new().with_storage(StorageType::System);

    let store = CredentialStoreFactory::create(&config);

    // 应该创建成功（可能是系统钥匙串或文件存储回退）
    assert!(store.is_ok(), "应该能创建存储（系统或回退到文件）");
}

#[test]
fn test_credential_expiry_in_file_store() {
    let test_file = get_test_file_path("expiry");
    cleanup_test_file(&test_file);

    let store = create_file_store_with_password(test_file.clone(), "expiry_test").unwrap();

    // 创建即将过期的凭证（10秒后过期，给Argon2id密钥派生留出足够时间）
    let expires_at = SystemTime::now() + Duration::from_secs(10);
    let cred = Credential::new_with_expiry(
        "test.com".to_string(),
        "user".to_string(),
        "token".to_string(),
        expires_at,
    );

    store.add(cred).unwrap();

    // 立即查询应该成功（添加操作可能耗时1-2秒，所以使用10秒过期时间）
    let retrieved = store.get("test.com", Some("user")).unwrap();
    assert!(retrieved.is_some(), "凭证应该还未过期");

    // 等待过期（11秒，确保已过期）
    std::thread::sleep(Duration::from_secs(11));

    // 过期后查询应该返回 None
    let retrieved = store.get("test.com", Some("user")).unwrap();
    assert!(retrieved.is_none(), "凭证应该已过期");

    cleanup_test_file(&test_file);
}

#[test]
fn test_factory_config_validation() {
    // 有效配置 - 内存存储
    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    assert!(CredentialStoreFactory::create(&config).is_ok());

    // 文件存储需要文件路径和密码，但失败后会回退到内存
    let config = CredentialConfig::new().with_storage(StorageType::File);
    // 应该回退到内存而不是失败
    assert!(CredentialStoreFactory::create(&config).is_ok());
}

#[test]
fn test_concurrent_file_store_operations() {
    use std::thread;

    let test_file = get_test_file_path("concurrent");
    cleanup_test_file(&test_file);

    let store = create_file_store_with_password(test_file.clone(), "concurrent_pass").unwrap();

    let mut handles = vec![];

    // 5 个线程并发操作
    for i in 0..5 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let cred = Credential::new(
                format!("host{i}.com"),
                format!("user{i}"),
                format!("token{i}"),
            );

            // 添加凭证
            store_clone.add(cred).unwrap();

            // 读取凭证
            let retrieved = store_clone
                .get(&format!("host{i}.com"), Some(&format!("user{i}")))
                .unwrap();
            assert!(retrieved.is_some());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证所有凭证都存在
    let list = store.list().unwrap();
    assert_eq!(list.len(), 5, "应该有 5 个凭证");

    cleanup_test_file(&test_file);
}

#[test]
fn test_update_last_used_workflow() {
    let test_file = get_test_file_path("update_last_used");
    cleanup_test_file(&test_file);

    let store = create_file_store_with_password(test_file.clone(), "last_used_test").unwrap();

    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "token".to_string(),
    );

    store.add(cred).unwrap();

    // 第一次获取，last_used_at 应该为 None
    let before = store.get("github.com", Some("user")).unwrap().unwrap();
    assert!(before.last_used_at.is_none());

    // 更新最后使用时间
    std::thread::sleep(Duration::from_millis(10));
    store.update_last_used("github.com", "user").unwrap();

    // 再次获取，last_used_at 应该已设置
    let after = store.get("github.com", Some("user")).unwrap().unwrap();
    assert!(after.last_used_at.is_some());
    assert!(after.last_used_at.unwrap() > before.created_at);

    cleanup_test_file(&test_file);
}
