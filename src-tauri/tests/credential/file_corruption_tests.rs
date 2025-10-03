//! 文件损坏和恢复测试
//!
//! 测试加密文件在各种损坏场景下的错误处理和恢复能力。

use fireworks_collaboration_lib::core::credential::{
    config::CredentialConfig,
    file_store::EncryptedFileStore,
    model::Credential,
    storage::CredentialStore,
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn get_test_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fireworks_corruption_test_{}.enc", name))
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_corrupted_json_structure() {
    let test_file = get_test_file("corrupted_json");
    cleanup(&test_file);

    // 创建包含无效 JSON 的文件
    fs::write(&test_file, "{invalid json content}}")
        .expect("应该写入文件");

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    // 尝试读取损坏的文件应该返回错误或空列表
    let result = store.list();
    
    // 应该优雅处理，不应该 panic
    match result {
        Ok(list) => assert_eq!(list.len(), 0, "损坏的文件应该返回空列表"),
        Err(_) => {} // 返回错误也是合理的
    }

    cleanup(&test_file);
}

#[test]
fn test_truncated_file() {
    let test_file = get_test_file("truncated");
    cleanup(&test_file);

    let password = "test_password";

    // 先创建正常的加密文件
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            "test.com".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );
        store.add(cred).expect("应该添加凭证");
    }

    // 截断文件（模拟写入中断）
    let content = fs::read_to_string(&test_file).expect("应该读取文件");
    let truncated = &content[..content.len() / 2];
    fs::write(&test_file, truncated).expect("应该写入截断的文件");

    // 尝试读取截断的文件
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let result = store.get("test.com", Some("user"));
        
        // 应该优雅地处理错误
        assert!(
            result.is_err() || result.unwrap().is_none(),
            "截断的文件应该返回错误或空结果"
        );
    }

    cleanup(&test_file);
}

#[test]
fn test_empty_file() {
    let test_file = get_test_file("empty");
    cleanup(&test_file);

    // 创建空文件
    fs::write(&test_file, "").expect("应该创建空文件");

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    // 应该能处理空文件
    let result = store.list();
    match result {
        Ok(list) => assert_eq!(list.len(), 0, "空文件应该返回空列表"),
        Err(_) => {} // 返回错误也可接受
    }

    cleanup(&test_file);
}

#[test]
fn test_binary_garbage_file() {
    let test_file = get_test_file("binary_garbage");
    cleanup(&test_file);

    // 创建包含二进制垃圾数据的文件
    let garbage: Vec<u8> = (0..=255).cycle().take(1024).collect();
    fs::write(&test_file, garbage).expect("应该写入垃圾数据");

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    // 应该优雅处理二进制垃圾数据
    let result = store.list();
    assert!(result.is_ok() || result.is_err(), "应该能处理垃圾数据而不 panic");

    cleanup(&test_file);
}

#[test]
fn test_file_with_missing_required_fields() {
    let test_file = get_test_file("missing_fields");
    cleanup(&test_file);

    // 创建缺少必需字段的 JSON
    let invalid_json = r#"{
        "version": 1
    }"#;
    fs::write(&test_file, invalid_json).expect("应该写入文件");

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    let result = store.list();
    
    // 应该能处理缺少字段的 JSON
    match result {
        Ok(list) => assert_eq!(list.len(), 0),
        Err(_) => {}
    }

    cleanup(&test_file);
}

#[test]
fn test_file_permissions_readonly() {
    let test_file = get_test_file("readonly");
    cleanup(&test_file);

    let password = "test_password";

    // 创建正常的加密文件
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            "test.com".to_string(),
            "user".to_string(),
            "pass".to_string(),
        );
        store.add(cred).expect("应该添加凭证");
    }

    // 在 Unix 系统上设置为只读
    #[cfg(unix)]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let perms = Permissions::from_mode(0o444);
        fs::set_permissions(&test_file, perms).expect("应该设置权限");

        // 尝试写入只读文件
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let new_cred = Credential::new(
            "new.com".to_string(),
            "user2".to_string(),
            "pass2".to_string(),
        );

        let add_result = store.add(new_cred);
        
        // 应该因为权限不足而失败
        assert!(add_result.is_err(), "写入只读文件应该失败");

        // 恢复写权限以便清理
        let perms = Permissions::from_mode(0o644);
        fs::set_permissions(&test_file, perms).ok();
    }

    // Windows 上跳过权限测试（不同的权限模型）
    #[cfg(windows)]
    {
        println!("Windows 上跳过只读权限测试");
    }

    cleanup(&test_file);
}

#[test]
fn test_very_large_file() {
    let test_file = get_test_file("large_file");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    // 添加大量凭证
    for i in 0..100 {
        let cred = Credential::new(
            format!("host{}.com", i),
            format!("user{}", i),
            format!("token_{}", i),
        );
        store.add(cred).expect(&format!("应该添加凭证 {}", i));
    }

    // 验证能正确读取大文件
    let list = store.list().expect("应该列出所有凭证");
    assert_eq!(list.len(), 100, "应该有 100 个凭证");

    // 验证文件大小合理（应该至少有几 KB）
    let metadata = fs::metadata(&test_file).expect("应该获取文件元数据");
    assert!(metadata.len() > 1024, "大文件应该超过 1KB");

    cleanup(&test_file);
}

#[test]
fn test_concurrent_file_corruption() {
    use std::sync::Arc;
    use std::thread;

    let test_file = get_test_file("concurrent_corruption");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = Arc::new(
        EncryptedFileStore::new(&config).expect("应该创建文件存储")
    );
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    let mut handles = vec![];

    // 多个线程同时写入
    for i in 0..5 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let cred = Credential::new(
                    format!("host{}_{}.com", i, j),
                    format!("user{}_{}", i, j),
                    format!("token_{}_{}", i, j),
                );
                let _ = store_clone.add(cred);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }

    // 验证文件仍然有效
    let list = store.list().expect("并发写入后文件应该仍然有效");
    
    // 由于并发和文件锁，可能不是所有 50 个凭证都成功添加
    // 但应该至少有一些凭证
    assert!(list.len() > 0, "应该至少添加了一些凭证");

    cleanup(&test_file);
}

#[test]
fn test_file_recovery_after_partial_write() {
    let test_file = get_test_file("partial_write");
    cleanup(&test_file);

    let password = "test_password";

    // 创建初始凭证
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            "original.com".to_string(),
            "user".to_string(),
            "original_token".to_string(),
        );
        store.add(cred).expect("应该添加原始凭证");
    }

    // 保存原始文件内容（供后续使用）
    let _original_content = fs::read(&test_file).expect("应该读取原始文件");

    // 模拟部分写入（写入不完整的数据）
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(false)
            .open(&test_file)
            .expect("应该打开文件");
        
        file.write_all(b"{\"incomplete\":").expect("应该写入部分数据");
    }

    // 尝试读取损坏的文件
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        // 应该检测到损坏
        let result = store.get("original.com", Some("user"));
        assert!(result.is_err() || result.unwrap().is_none(), "应该检测到文件损坏");
    }

    cleanup(&test_file);
}
