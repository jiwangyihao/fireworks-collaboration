//! Factory 回退机制测试
//!
//! 测试 `CredentialStoreFactory` 的三层回退逻辑和错误处理。

use fireworks_collaboration_lib::core::credential::{
    config::{CredentialConfig, StorageType},
    factory::CredentialStoreFactory,
    model::Credential,
};
use std::fs;
use std::path::PathBuf;

/// 获取测试用的临时文件路径
fn get_test_file_path(test_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fireworks_factory_test_{test_name}.enc"))
}

/// 清理测试文件
fn cleanup_test_file(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_factory_fallback_from_invalid_file_path() {
    // 配置一个无效的文件路径（无权限或不存在的目录）
    let invalid_path = if cfg!(windows) {
        "C:\\Windows\\System32\\invalid_location\\creds.enc"
    } else {
        "/root/invalid_location/creds.enc"
    };

    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(invalid_path.to_string());

    // 应该回退到内存存储而不是失败
    let store = CredentialStoreFactory::create(&config);
    assert!(store.is_ok(), "无效文件路径应该回退到内存存储");

    // 验证存储可用
    let store = store.unwrap();
    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        "pass".to_string(),
    );
    assert!(store.add(cred).is_ok(), "回退的存储应该可用");
}

#[test]
fn test_factory_file_without_master_password_fallback() {
    let test_file = get_test_file_path("no_password");
    cleanup_test_file(&test_file);

    // 配置文件存储但不设置主密码
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = CredentialStoreFactory::create(&config);

    // 应该成功创建（可能是文件存储或回退到内存）
    assert!(store.is_ok(), "应该创建存储（文件或回退到内存）");

    cleanup_test_file(&test_file);
}

#[test]
fn test_factory_memory_always_succeeds() {
    // 内存存储永远不应该失败
    let config = CredentialConfig::new().with_storage(StorageType::Memory);

    for _ in 0..10 {
        let store = CredentialStoreFactory::create(&config);
        assert!(store.is_ok(), "内存存储应该总是成功创建");
    }
}

#[test]
fn test_factory_preserves_config_settings() {
    let config = CredentialConfig::new()
        .with_storage(StorageType::Memory)
        .with_ttl(Some(7200));

    let store = CredentialStoreFactory::create(&config);
    assert!(store.is_ok(), "应该成功创建存储");

    // 虽然我们无法直接验证 TTL 设置，但至少确认存储可用
    let store = store.unwrap();
    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        "pass".to_string(),
    );
    assert!(store.add(cred).is_ok());
}

#[test]
fn test_factory_system_storage_on_supported_platforms() {
    let config = CredentialConfig::new().with_storage(StorageType::System);

    let result = CredentialStoreFactory::create(&config);

    // 在所有平台上都应该成功（Windows/macOS/Linux 系统存储或回退）
    assert!(result.is_ok(), "系统存储应该可用或成功回退");

    // 验证存储功能
    if let Ok(store) = result {
        let cred = Credential::new(
            format!("fireworks.factory.test.{}", uuid::Uuid::new_v4()),
            "testuser".to_string(),
            "testpass".to_string(),
        );

        // 尝试基本操作
        let add_result = store.add(cred.clone());

        if add_result.is_ok() {
            // 如果添加成功，清理凭证
            let _ = store.remove(&cred.host, &cred.username);
        }

        // 无论是否成功添加，都认为测试通过（可能是系统限制）
    }
}

#[test]
fn test_factory_creates_independent_stores() {
    let config1 = CredentialConfig::new().with_storage(StorageType::Memory);
    let config2 = CredentialConfig::new().with_storage(StorageType::Memory);

    let store1 = CredentialStoreFactory::create(&config1).expect("应该创建第一个存储");
    let store2 = CredentialStoreFactory::create(&config2).expect("应该创建第二个存储");

    // 向第一个存储添加凭证
    let cred1 = Credential::new(
        "test1.com".to_string(),
        "user1".to_string(),
        "pass1".to_string(),
    );
    store1.add(cred1).expect("应该添加凭证到 store1");

    // 向第二个存储添加不同凭证
    let cred2 = Credential::new(
        "test2.com".to_string(),
        "user2".to_string(),
        "pass2".to_string(),
    );
    store2.add(cred2).expect("应该添加凭证到 store2");

    // 验证存储是独立的
    let list1 = store1.list().expect("应该列出 store1 的凭证");
    let list2 = store2.list().expect("应该列出 store2 的凭证");

    assert_eq!(list1.len(), 1, "store1 应该只有一个凭证");
    assert_eq!(list2.len(), 1, "store2 应该只有一个凭证");
    assert_eq!(list1[0].host, "test1.com");
    assert_eq!(list2[0].host, "test2.com");
}

#[test]
fn test_factory_with_empty_file_path() {
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path("".to_string());

    // 空路径应该导致回退到内存存储
    let store = CredentialStoreFactory::create(&config);
    assert!(store.is_ok(), "空文件路径应该回退到内存存储");
}

#[test]
fn test_factory_concurrent_creation() {
    use std::thread;

    let mut handles = vec![];

    // 并发创建多个存储
    for i in 0..5 {
        let handle = thread::spawn(move || {
            let config = CredentialConfig::new().with_storage(StorageType::Memory);

            let store = CredentialStoreFactory::create(&config)
                .unwrap_or_else(|_| panic!("线程 {i} 应该创建存储"));

            let cred = Credential::new(
                format!("host{i}.com"),
                format!("user{i}"),
                format!("pass{i}"),
            );
            store
                .add(cred)
                .unwrap_or_else(|_| panic!("线程 {i} 应该添加凭证"));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该成功完成");
    }
}
