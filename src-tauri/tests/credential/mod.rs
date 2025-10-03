//! 凭证模块集成测试
//!
//! 测试凭证管理的完整生命周期和模块间协作。

use fireworks_collaboration_lib::core::credential::{
    config::{CredentialConfig, StorageType},
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::time::{Duration, SystemTime};

#[test]
fn test_credential_full_lifecycle() {
    // 1. 创建存储
    let store = MemoryCredentialStore::new();

    // 2. 添加凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "alice".to_string(),
        "ghp_secret_token_123456".to_string(),
    );

    assert!(store.add(cred.clone()).is_ok());

    // 3. 查询凭证
    let retrieved = store
        .get("github.com", Some("alice"))
        .unwrap()
        .expect("凭证应该存在");

    assert_eq!(retrieved.host, "github.com");
    assert_eq!(retrieved.username, "alice");
    assert_eq!(retrieved.password_or_token, "ghp_secret_token_123456");

    // 4. 更新最后使用时间
    assert!(store.update_last_used("github.com", "alice").is_ok());

    let updated = store
        .get("github.com", Some("alice"))
        .unwrap()
        .expect("凭证应该存在");
    assert!(updated.last_used_at.is_some());

    // 5. 列出所有凭证
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);

    // 6. 删除凭证
    assert!(store.remove("github.com", "alice").is_ok());

    // 7. 验证已删除
    let retrieved = store.get("github.com", Some("alice")).unwrap();
    assert!(retrieved.is_none());
}

#[test]
fn test_credential_expiry_workflow() {
    let store = MemoryCredentialStore::new();

    // 创建即将过期的凭证
    let expires_at = SystemTime::now() + Duration::from_millis(100);
    let cred = Credential::new_with_expiry(
        "github.com".to_string(),
        "bob".to_string(),
        "token".to_string(),
        expires_at,
    );

    store.add(cred).unwrap();

    // 立即查询应该成功
    let retrieved = store.get("github.com", Some("bob")).unwrap();
    assert!(retrieved.is_some());

    // 等待过期
    std::thread::sleep(Duration::from_millis(150));

    // 过期后查询应该返回 None
    let retrieved = store.get("github.com", Some("bob")).unwrap();
    assert!(retrieved.is_none());

    // 列表中也不应包含过期凭证
    let list = store.list().unwrap();
    assert_eq!(list.len(), 0);
}

#[test]
fn test_multiple_credentials_management() {
    let store = MemoryCredentialStore::new();

    // 添加多个凭证
    let credentials = vec![
        ("github.com", "alice", "token1"),
        ("github.com", "bob", "token2"),
        ("gitlab.com", "alice", "token3"),
        ("bitbucket.org", "charlie", "token4"),
    ];

    for (host, username, token) in &credentials {
        let cred = Credential::new(host.to_string(), username.to_string(), token.to_string());
        store.add(cred).unwrap();
    }

    // 验证所有凭证都存在
    for (host, username, _) in &credentials {
        let cred = store.get(host, Some(username)).unwrap();
        assert!(cred.is_some());
    }

    // 测试按主机查询（不指定用户名）
    let github_cred = store.get("github.com", None).unwrap();
    assert!(github_cred.is_some());
    assert_eq!(github_cred.unwrap().host, "github.com");

    // 列出所有凭证
    let list = store.list().unwrap();
    assert_eq!(list.len(), 4);

    // 删除部分凭证
    store.remove("github.com", "alice").unwrap();
    store.remove("gitlab.com", "alice").unwrap();

    let remaining = store.list().unwrap();
    assert_eq!(remaining.len(), 2);
}

#[test]
fn test_credential_config_validation() {
    // 有效配置
    let valid_config = CredentialConfig::new()
        .with_storage(StorageType::Memory)
        .with_ttl(Some(3600));
    assert!(valid_config.validate().is_ok());

    // 无效配置：文件存储但未指定路径
    let invalid_config = CredentialConfig::new().with_storage(StorageType::File);
    assert!(invalid_config.validate().is_err());

    // 有效配置：文件存储且指定路径
    let valid_file_config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path("/tmp/creds.enc".to_string());
    assert!(valid_file_config.validate().is_ok());

    // 无效配置：TTL 为 0
    let invalid_ttl_config = CredentialConfig::new().with_ttl(Some(0));
    assert!(invalid_ttl_config.validate().is_err());
}

#[test]
fn test_credential_masked_display() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "ghp_1234567890abcdef".to_string(),
    );

    // 测试 Display trait
    let display = format!("{}", cred);
    assert!(!display.contains("ghp_1234567890abcdef"));
    assert!(display.contains("****"));

    // 测试 Debug trait
    let debug = format!("{:?}", cred);
    assert!(!debug.contains("ghp_1234567890abcdef"));
    assert!(debug.contains("****"));

    // 测试 masked_password 方法
    let masked = cred.masked_password();
    assert_eq!(masked, "ghp_****cdef");
}

#[test]
fn test_credential_serialization_security() {
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "secret_password".to_string(),
    );

    // 序列化
    let json = serde_json::to_string(&cred).unwrap();

    // 验证密码未被序列化
    assert!(!json.contains("secret_password"));
    assert!(json.contains("github.com"));
    assert!(json.contains("user"));
}

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
fn test_concurrent_credential_operations() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(MemoryCredentialStore::new());
    let mut handles = vec![];

    // 10 个线程并发操作
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            // 添加凭证
            let cred = Credential::new(
                format!("host{}.com", i),
                format!("user{}", i),
                format!("token{}", i),
            );
            store_clone.add(cred).unwrap();

            // 查询凭证
            let retrieved = store_clone
                .get(&format!("host{}.com", i), Some(&format!("user{}", i)))
                .unwrap();
            assert!(retrieved.is_some());

            // 更新最后使用时间
            store_clone
                .update_last_used(&format!("host{}.com", i), &format!("user{}", i))
                .unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证所有操作都成功
    let list = store.list().unwrap();
    assert_eq!(list.len(), 10);
}

#[test]
fn test_config_effective_storage_type() {
    let config = CredentialConfig::new().with_storage(StorageType::System);
    assert_eq!(config.effective_storage_type(), StorageType::System);

    let config = CredentialConfig::new().with_storage(StorageType::File);
    assert_eq!(config.effective_storage_type(), StorageType::File);

    let config = CredentialConfig::new().with_storage(StorageType::Memory);
    assert_eq!(config.effective_storage_type(), StorageType::Memory);
}

#[test]
fn test_credential_update_workflow() {
    let store = MemoryCredentialStore::new();

    // 添加初始凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "old_token".to_string(),
    );
    store.add(cred).unwrap();

    // 删除旧凭证
    store.remove("github.com", "user").unwrap();

    // 添加新凭证（模拟更新）
    let new_cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "new_token".to_string(),
    );
    store.add(new_cred).unwrap();

    // 验证新凭证
    let retrieved = store.get("github.com", Some("user")).unwrap().unwrap();
    assert_eq!(retrieved.password_or_token, "new_token");
}
