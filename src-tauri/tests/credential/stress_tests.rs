//! P6.2 凭证模块压力测试
//!
//! 测试极端情况下的性能和稳定性

use fireworks_collaboration_lib::core::credential::{
    audit::AuditLogger,
    config::{CredentialConfig, StorageType},
    file_store::EncryptedFileStore,
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

/// 测试：大量凭证存储（1000个）
#[test]
fn test_large_number_of_credentials() {
    let store = MemoryCredentialStore::new();

    // 添加 1000 个凭证
    for i in 0..1000 {
        let cred = Credential::new(
            format!("host{i}.com"),
            format!("user{i}"),
            format!("password{i}"),
        );
        store.add(cred).unwrap();
    }

    // 验证所有凭证都存在
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1000);

    // 随机查询一些凭证
    for i in (0..1000).step_by(100) {
        let cred = store
            .get(&format!("host{i}.com"), Some(&format!("user{i}")))
            .unwrap()
            .expect("凭证应该存在");
        assert_eq!(cred.password_or_token, format!("password{i}"));
    }
}

/// 测试：超长主机名和用户名
#[test]
fn test_very_long_host_and_username() {
    let store = MemoryCredentialStore::new();

    // 创建非常长的主机名和用户名（各 1KB）
    let long_host = "a".repeat(1024) + ".com";
    let long_username = "user_".to_string() + &"b".repeat(1000);

    let cred = Credential::new(
        long_host.clone(),
        long_username.clone(),
        "password123".to_string(),
    );

    store.add(cred).unwrap();

    let retrieved = store
        .get(&long_host, Some(&long_username))
        .unwrap()
        .expect("应该找到凭证");

    assert_eq!(retrieved.host, long_host);
    assert_eq!(retrieved.username, long_username);
}

/// 测试：超长密码（10KB）
#[test]
fn test_very_long_password() {
    let store = MemoryCredentialStore::new();

    // 创建 10KB 的密码
    let long_password = "x".repeat(10 * 1024);

    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        long_password.clone(),
    );

    store.add(cred).unwrap();

    let retrieved = store
        .get("github.com", Some("user"))
        .unwrap()
        .expect("应该找到凭证");

    assert_eq!(retrieved.password_or_token.len(), 10 * 1024);
    assert_eq!(retrieved.password_or_token, long_password);
}

/// 测试：高并发读写（100个线程）
#[test]
fn test_high_concurrency_operations() {
    let store = Arc::new(MemoryCredentialStore::new());
    let mut handles = vec![];

    // 100 个线程并发操作
    for i in 0..100 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            // 每个线程执行多个操作
            for j in 0..10 {
                let cred = Credential::new(
                    format!("host{i}-{j}.com"),
                    format!("user{i}"),
                    format!("password{j}"),
                );
                store_clone.add(cred).unwrap();

                // 立即读取
                let retrieved = store_clone
                    .get(&format!("host{i}-{j}.com"), Some(&format!("user{i}")))
                    .unwrap();
                assert!(retrieved.is_some());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证所有凭证都被添加
    let list = store.list().unwrap();
    assert_eq!(list.len(), 100 * 10);
}

/// 测试：快速连续添加和删除
#[test]
fn test_rapid_add_remove_cycles() {
    let store = MemoryCredentialStore::new();

    // 1000 次快速添加和删除循环
    for i in 0..1000 {
        let cred = Credential::new(
            "github.com".to_string(),
            "user".to_string(),
            format!("password{i}"),
        );

        store.add(cred).unwrap();
        assert!(store.get("github.com", Some("user")).unwrap().is_some());

        store.remove("github.com", "user").unwrap();
        assert!(store.get("github.com", Some("user")).unwrap().is_none());
    }
}

/// 测试：大量审计日志（10000条）
#[test]
fn test_large_audit_log() {
    let logger = AuditLogger::new(true);

    // 记录 10000 条审计日志
    for i in 0..10000 {
        logger.log_operation(
            if i % 4 == 0 {
                fireworks_collaboration_lib::core::credential::audit::OperationType::Add
            } else if i % 4 == 1 {
                fireworks_collaboration_lib::core::credential::audit::OperationType::Get
            } else if i % 4 == 2 {
                fireworks_collaboration_lib::core::credential::audit::OperationType::Update
            } else {
                fireworks_collaboration_lib::core::credential::audit::OperationType::Remove
            },
            &format!("host{}.com", i % 100),
            "user",
            Some(&format!("password{i}")),
            true,
            None,
        );
    }

    let events = logger.get_events();
    assert_eq!(events.len(), 10000);

    // 验证可以导出为 JSON
    let json = logger.export_to_json().unwrap();
    assert!(!json.is_empty());
}

/// 测试：并发审计日志记录
#[test]
fn test_concurrent_audit_logging() {
    let logger = Arc::new(AuditLogger::new(true));
    let mut handles = vec![];

    // 50 个线程并发记录日志
    for i in 0..50 {
        let logger_clone = Arc::clone(&logger);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                logger_clone.log_operation(
                    fireworks_collaboration_lib::core::credential::audit::OperationType::Add,
                    &format!("host{i}-{j}.com"),
                    "user",
                    Some("password"),
                    true,
                    None,
                );
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证所有日志都被记录
    let events = logger.get_events();
    assert_eq!(events.len(), 50 * 100);
}

/// 测试：文件存储的大量凭证加密
#[test]
fn test_encrypted_file_store_stress() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");

    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store
        .set_master_password("test-password".to_string())
        .unwrap();

    // 添加 100 个凭证（加密文件存储较慢）
    for i in 0..100 {
        let cred = Credential::new(
            format!("host{i}.com"),
            format!("user{i}"),
            format!("password{i}"),
        );
        store.add(cred).unwrap();
    }

    // 验证所有凭证都可以读取
    let list = store.list().unwrap();
    assert_eq!(list.len(), 100);
}

/// 测试：极端过期时间
#[test]
fn test_extreme_expiry_times() {
    let store = MemoryCredentialStore::new();

    // 测试已过期的凭证
    let past = SystemTime::now() - Duration::from_secs(365 * 24 * 60 * 60); // 1 年前
    let cred_past = Credential::new_with_expiry(
        "host1.com".to_string(),
        "user".to_string(),
        "password".to_string(),
        past,
    );
    store.add(cred_past).unwrap();

    // 测试遥远未来的凭证
    let future = SystemTime::now() + Duration::from_secs(100 * 365 * 24 * 60 * 60); // 100 年后
    let cred_future = Credential::new_with_expiry(
        "host2.com".to_string(),
        "user".to_string(),
        "password".to_string(),
        future,
    );
    store.add(cred_future).unwrap();

    // 过期的应该被过滤
    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].host, "host2.com");
}

/// 测试：混合 Unicode 字符
#[test]
fn test_unicode_credentials() {
    let store = MemoryCredentialStore::new();

    let unicode_tests = vec![
        ("主机.中国", "用户", "密码123"),
        ("хост.рф", "пользователь", "пароль"),
        ("🌟.com", "😀user", "🔒password"),
        ("مضيف.com", "مستخدم", "كلمة السر"),
    ];

    for (host, user, pwd) in unicode_tests {
        let cred = Credential::new(host.to_string(), user.to_string(), pwd.to_string());
        store.add(cred).unwrap();
    }

    let list = store.list().unwrap();
    assert_eq!(list.len(), 4);
}

/// 测试：空字符串边界情况
#[test]
fn test_empty_string_edge_cases() {
    let store = MemoryCredentialStore::new();

    // 空主机名应该可以存储（虽然不推荐）
    let cred = Credential::new("".to_string(), "user".to_string(), "password".to_string());
    assert!(store.add(cred).is_ok());

    // 空用户名
    let cred2 = Credential::new(
        "github.com".to_string(),
        "".to_string(),
        "password".to_string(),
    );
    assert!(store.add(cred2).is_ok());
}

/// 测试：特殊字符密码
#[test]
fn test_special_characters_in_password() {
    let store = MemoryCredentialStore::new();

    let special_passwords = [
        "!@#$%^&*()_+-=[]{}|;:',.<>?/~`",
        "\n\r\t\\\"'",
        "    spaces    everywhere    ",
        "emoji🔒🔑🛡️混合password",
    ];

    for (i, pwd) in special_passwords.iter().enumerate() {
        let cred = Credential::new(format!("host{i}.com"), "user".to_string(), pwd.to_string());
        store.add(cred).unwrap();

        let retrieved = store
            .get(&format!("host{i}.com"), Some("user"))
            .unwrap()
            .expect("应该找到凭证");
        assert_eq!(&retrieved.password_or_token, pwd);
    }
}

/// 测试：并发文件存储操作的资源管理
#[test]
fn test_concurrent_file_store_resource_management() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");

    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());

    let store = Arc::new(EncryptedFileStore::new(&config).expect("应该创建文件存储"));
    store
        .set_master_password("test-password".to_string())
        .unwrap();

    let mut handles = vec![];

    // 10 个线程并发读写
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            for j in 0..5 {
                let cred = Credential::new(
                    format!("host{i}-{j}.com"),
                    format!("user{i}"),
                    format!("password{j}"),
                );

                // 添加
                store_clone.add(cred).unwrap();

                // 读取
                let _ = store_clone.get(&format!("host{i}-{j}.com"), Some(&format!("user{i}")));

                // 短暂休眠以增加并发冲突概率
                thread::sleep(Duration::from_millis(1));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // 验证最终状态
    let list = store.list().unwrap();
    assert_eq!(list.len(), 10 * 5);
}

/// 测试：快速重复设置主密码
#[test]
fn test_rapid_master_password_changes() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");

    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");

    // 快速重复设置主密码 100 次
    for i in 0..100 {
        assert!(store.set_master_password(format!("password{i}")).is_ok());
    }
}
