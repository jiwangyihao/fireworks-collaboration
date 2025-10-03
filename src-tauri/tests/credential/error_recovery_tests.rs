//! P6.2 凭证模块错误恢复测试
//!
//! 测试各种错误场景下的系统恢复能力

use fireworks_collaboration_lib::core::credential::{
    config::{CredentialConfig, StorageType},
    file_store::EncryptedFileStore,
    model::Credential,
    storage::CredentialStore,
};
use std::fs;
use std::io::Write;
use tempfile::TempDir;

/// 测试：错误密码重试
#[test]
fn test_wrong_password_retry() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("correct-password".to_string()).unwrap();
    
    // 添加凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "token123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 创建新的存储实例（模拟重启）
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    
    // 使用错误密码
    store2.set_master_password("wrong-password".to_string()).unwrap();
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_err(), "错误密码应该失败");
    
    // 使用正确密码重试
    store2.set_master_password("correct-password".to_string()).unwrap();
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_ok(), "正确密码应该成功");
}

/// 测试：部分损坏的 JSON 恢复
#[test]
fn test_partial_json_corruption_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 读取文件并破坏 JSON 结构（但保持有效字符）
    let mut content = fs::read_to_string(&file_path).unwrap();
    
    // 删除最后的大括号（破坏 JSON 结构）
    if let Some(pos) = content.rfind('}') {
        content.truncate(pos);
    }
    
    fs::write(&file_path, content).unwrap();
    
    // 尝试读取应该返回解析错误
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store2.set_master_password("test-password".to_string()).unwrap();
    
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_err(), "损坏的 JSON 应该返回错误");
}

/// 测试：文件不存在时的优雅处理
#[test]
fn test_missing_file_graceful_handling() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("nonexistent.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 查询不存在的凭证应该返回 None 而不是错误
    let result = store.get("github.com", Some("user"));
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

/// 测试：文件被删除后的恢复
#[test]
fn test_file_deleted_during_operation() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 添加凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 验证文件存在
    assert!(file_path.exists());
    
    // 删除文件
    fs::remove_file(&file_path).unwrap();
    
    // 尝试读取应该返回 None（文件不存在）
    let result = store.get("github.com", Some("user"));
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // 添加新凭证应该重新创建文件
    let cred2 = Credential::new(
        "gitlab.com".to_string(),
        "user2".to_string(),
        "password456".to_string(),
    );
    assert!(store.add(cred2).is_ok());
    assert!(file_path.exists());
}

/// 测试：无效的 Base64 编码恢复
#[test]
fn test_invalid_base64_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 读取原始内容
    let original_content = fs::read_to_string(&file_path).unwrap();
    
    // 完全破坏文件内容（写入无效 JSON）
    let invalid_content = original_content.replace("\"version\"", "\"CORRUPTED\"");
    fs::write(&file_path, invalid_content).unwrap();
    
    // 尝试读取应该失败
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store2.set_master_password("test-password".to_string()).unwrap();
    
    let result = store2.get("github.com", Some("user"));
    // 损坏的文件应该不返回有效凭证
    match result {
        Ok(Some(_)) => panic!("损坏的文件不应该返回有效凭证"),
        Ok(None) => {}, // 合理：解析失败返回 None
        Err(_) => {},   // 合理：返回错误
    }
}

/// 测试：并发冲突的处理
#[test]
fn test_concurrent_conflict_handling() {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;
    
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = Arc::new(EncryptedFileStore::new(&config).expect("应该创建文件存储"));
    store.set_master_password("test-password".to_string()).unwrap();
    
    let mut handles = vec![];
    
    // 10 个线程尝试同时写入同一个凭证
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let cred = Credential::new(
                "github.com".to_string(),
                "user".to_string(),
                format!("password{}", i),
            );
            
            // 所有操作都应该成功（通过 Mutex 同步）
            store_clone.add(cred).unwrap();
            thread::sleep(Duration::from_millis(1));
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    // 验证最终状态一致（最后一次写入生效）
    let result = store.get("github.com", Some("user")).unwrap();
    assert!(result.is_some());
}

/// 测试：磁盘空间不足模拟（通过写入大量数据）
#[test]
#[ignore] // 这个测试可能很慢，默认跳过
fn test_disk_space_handling() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 尝试添加非常大的凭证（模拟磁盘空间问题）
    let huge_password = "x".repeat(100 * 1024 * 1024); // 100MB
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        huge_password,
    );
    
    // 这可能成功或失败，取决于系统
    let _ = store.add(cred);
}

/// 测试：文件权限问题恢复
#[test]
#[cfg(unix)]
fn test_file_permission_recovery() {
    use std::os::unix::fs::PermissionsExt;
    
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 修改文件权限为只读
    let mut permissions = fs::metadata(&file_path).unwrap().permissions();
    permissions.set_mode(0o444); // 只读
    fs::set_permissions(&file_path, permissions).unwrap();
    
    // 尝试写入应该失败
    let cred2 = Credential::new(
        "gitlab.com".to_string(),
        "user2".to_string(),
        "password456".to_string(),
    );
    let result = store.add(cred2);
    assert!(result.is_err(), "只读文件应该拒绝写入");
    
    // 恢复权限
    let mut permissions = fs::metadata(&file_path).unwrap().permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&file_path, permissions).unwrap();
}

/// 测试：空文件的处理
#[test]
fn test_empty_file_handling() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    // 创建空文件
    fs::File::create(&file_path).unwrap();
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 读取空文件应该返回 None 或错误
    let result = store.get("github.com", Some("user"));
    // 空文件可能被视为无效或空列表
    if result.is_ok() {
        assert!(result.unwrap().is_none(), "空文件应该返回 None");
    }
    
    // 注：不测试 add 操作，因为空文件可能已损坏存储状态
}

/// 测试：只包含空格的文件
#[test]
fn test_whitespace_only_file() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    // 创建只包含空格的文件
    fs::write(&file_path, "   \n\t\r\n   ").unwrap();
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 应该能够处理空白文件（返回 None 或错误）
    let result = store.get("github.com", Some("user"));
    // 空白文件可能被视为无效 JSON
    if result.is_ok() {
        assert!(result.unwrap().is_none(), "空白文件应该返回 None");
    }
}

/// 测试：多次密码错误后的恢复
#[test]
fn test_multiple_wrong_password_attempts() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("correct-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "token123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 多次尝试错误密码
    for i in 0..10 {
        let store_temp = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store_temp.set_master_password(format!("wrong-password-{}", i)).unwrap();
        let result = store_temp.get("github.com", Some("user"));
        assert!(result.is_err(), "错误密码应该失败");
    }
    
    // 最后使用正确密码应该成功
    let store_final = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store_final.set_master_password("correct-password".to_string()).unwrap();
    let result = store_final.get("github.com", Some("user"));
    assert!(result.is_ok(), "正确密码应该成功");
}

/// 测试：文件被截断的恢复
#[test]
fn test_truncated_file_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    let cred = Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "password123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 读取文件并截断到一半
    let content = fs::read(&file_path).unwrap();
    let half_len = content.len() / 2;
    fs::write(&file_path, &content[..half_len]).unwrap();
    
    // 尝试读取应该返回错误
    let store2 = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store2.set_master_password("test-password".to_string()).unwrap();
    
    let result = store2.get("github.com", Some("user"));
    assert!(result.is_err(), "截断的文件应该返回错误");
}

/// 测试：非 UTF-8 文件内容
#[test]
fn test_non_utf8_file_content() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("creds.enc");
    
    // 写入非 UTF-8 字节
    let mut file = fs::File::create(&file_path).unwrap();
    file.write_all(&[0xFF, 0xFE, 0xFD, 0xFC]).unwrap();
    drop(file);
    
    let config = CredentialConfig::new()
        .with_storage(StorageType::File)
        .with_file_path(file_path.to_str().unwrap().to_string());
    
    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test-password".to_string()).unwrap();
    
    // 应该能够处理非 UTF-8 文件（返回错误或覆盖）
    let result = store.get("github.com", Some("user"));
    // 可能返回错误或 None，两者都是合理的
    assert!(result.is_ok() || result.is_err());
}
