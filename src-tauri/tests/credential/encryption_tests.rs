//! 加密强度和安全性验证测试
//!
//! 测试 AES-256-GCM、HMAC-SHA256、Argon2id 等加密组件的正确性。

use fireworks_collaboration_lib::core::credential::{
    config::CredentialConfig,
    file_store::EncryptedFileStore,
    model::Credential,
    storage::CredentialStore,
};
use std::fs;
use std::path::PathBuf;

fn get_test_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fireworks_enc_test_{}.enc", name))
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_encryption_uses_random_iv() {
    let file1 = get_test_file("random_iv_1");
    let file2 = get_test_file("random_iv_2");
    cleanup(&file1);
    cleanup(&file2);

    let password = "same_password_for_both";
    let data = ("github.com", "alice", "secret_token_123");

    // 创建两个独立的加密文件，使用相同密码和数据
    for file in [&file1, &file2] {
        let config = CredentialConfig::new()
            .with_file_path(file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config)
            .expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            data.0.to_string(),
            data.1.to_string(),
            data.2.to_string(),
        );
        store.add(cred).expect("应该添加凭证");
    }

    // 读取两个文件的原始内容
    let content1 = fs::read(&file1).expect("应该读取文件1");
    let content2 = fs::read(&file2).expect("应该读取文件2");

    // 密文应该不同（因为使用了不同的随机 IV 和盐）
    assert_ne!(
        content1, content2,
        "相同数据的两次加密应该产生不同的密文（随机 IV）"
    );

    cleanup(&file1);
    cleanup(&file2);
}

#[test]
fn test_encryption_produces_valid_json_structure() {
    let test_file = get_test_file("json_structure");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("test_password".to_string())
        .expect("应该设置主密码");

    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        "pass".to_string(),
    );
    store.add(cred).expect("应该添加凭证");

    // 读取文件内容
    let file_content = fs::read_to_string(&test_file)
        .expect("应该读取文件");

    // 验证是有效的 JSON
    let json: serde_json::Value = serde_json::from_str(&file_content)
        .expect("文件应该包含有效的 JSON");

    // 验证 JSON 结构包含必要字段
    assert!(json.get("version").is_some(), "应该有 version 字段");
    assert!(json.get("salt").is_some(), "应该有 salt 字段");
    assert!(json.get("ciphertext").is_some(), "应该有 ciphertext 字段（加密数据）");
    assert!(json.get("nonce").is_some(), "应该有 nonce 字段");
    assert!(json.get("hmac").is_some(), "应该有 hmac 字段");

    // 验证凭证是加密的（不应包含明文密码）
    let content_str = file_content.to_lowercase();
    assert!(!content_str.contains("pass"), "文件不应包含明文密码");

    cleanup(&test_file);
}

#[test]
fn test_hmac_verification_detects_tampering() {
    let test_file = get_test_file("hmac_tamper");
    cleanup(&test_file);

    let password = "secure_password_123";

    // 创建并添加凭证
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            "github.com".to_string(),
            "alice".to_string(),
            "token123".to_string(),
        );
        store.add(cred).expect("应该添加凭证");
    }

    // 篡改文件内容
    let mut file_content = fs::read_to_string(&test_file)
        .expect("应该读取文件");
    
    // 修改 JSON 中的密文（破坏 HMAC）
    if let Some(pos) = file_content.find("\"ciphertext\"") {
        // 修改密文的一部分
        let mut chars: Vec<char> = file_content.chars().collect();
        if pos + 20 < chars.len() {
            chars[pos + 20] = 'X';
            file_content = chars.into_iter().collect();
            fs::write(&test_file, file_content)
                .expect("应该写入篡改的文件");
        }
    }

    // 尝试读取被篡改的文件
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let result = store.get("github.com", Some("alice"));
        
        // 应该检测到篡改并失败
        assert!(
            result.is_err() || result.unwrap().is_none(),
            "HMAC 校验应该检测到文件被篡改"
        );
    }

    cleanup(&test_file);
}

#[test]
fn test_different_passwords_cannot_decrypt() {
    let test_file = get_test_file("wrong_password");
    cleanup(&test_file);

    let correct_password = "correct_password_123";
    let wrong_password = "wrong_password_456";

    // 使用正确密码创建并加密
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(correct_password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            "test.com".to_string(),
            "user".to_string(),
            "secret_data".to_string(),
        );
        store.add(cred).expect("应该添加凭证");
    }

    // 使用错误密码尝试解密
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(wrong_password.to_string())
            .expect("应该设置主密码");

        let result = store.get("test.com", Some("user"));
        
        // 应该无法解密或找不到凭证
        assert!(
            result.is_err() || result.unwrap().is_none(),
            "错误的密码不应该能解密数据"
        );
    }

    cleanup(&test_file);
}

#[test]
fn test_encryption_handles_empty_credentials_list() {
    let test_file = get_test_file("empty_list");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    store.set_master_password("password".to_string())
        .expect("应该设置主密码");

    // 列出空凭证列表
    let list = store.list().expect("应该能列出空列表");
    assert_eq!(list.len(), 0, "初始列表应该为空");

    // 文件不应该在没有凭证时创建
    // 只有在添加凭证时才会创建文件
    assert!(!test_file.exists(), "空存储不应该创建文件");

    cleanup(&test_file);
}

#[test]
fn test_encryption_survives_file_reopen() {
    let test_file = get_test_file("reopen");
    cleanup(&test_file);

    let password = "persistent_password";
    let test_data = ("gitlab.com", "bob", "secure_token_xyz");

    // 第一次：创建并添加数据
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let cred = Credential::new(
            test_data.0.to_string(),
            test_data.1.to_string(),
            test_data.2.to_string(),
        );
        store.add(cred).expect("应该添加凭证");
    }

    // 第二次：重新打开并验证数据
    {
        let config = CredentialConfig::new()
            .with_file_path(test_file.to_string_lossy().to_string());

        let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
        store.set_master_password(password.to_string())
            .expect("应该设置主密码");

        let retrieved = store.get(test_data.0, Some(test_data.1))
            .expect("应该能读取凭证")
            .expect("凭证应该存在");

        assert_eq!(retrieved.host, test_data.0);
        assert_eq!(retrieved.username, test_data.1);
        assert_eq!(retrieved.password_or_token, test_data.2);
    }

    cleanup(&test_file);
}

#[test]
fn test_encryption_with_special_characters() {
    let test_file = get_test_file("special_chars");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");
    
    // 密码包含特殊字符
    let password = r#"P@ssw0rd!<>{}[]"'\|;:,./?"#;
    store.set_master_password(password.to_string())
        .expect("应该接受包含特殊字符的密码");

    // 凭证数据包含特殊字符
    let cred = Credential::new(
        "test.com".to_string(),
        "user@example.com".to_string(),
        r#"token!@#$%^&*()_+-={}[]|\:";'<>?,./~`"#.to_string(),
    );

    store.add(cred.clone()).expect("应该添加包含特殊字符的凭证");

    // 验证能正确读取
    let retrieved = store.get("test.com", Some("user@example.com"))
        .expect("应该能读取")
        .expect("凭证应该存在");

    assert_eq!(retrieved.password_or_token, cred.password_or_token);

    cleanup(&test_file);
}

#[test]
fn test_encryption_salt_is_unique() {
    let file1 = get_test_file("salt_unique_1");
    let file2 = get_test_file("salt_unique_2");
    cleanup(&file1);
    cleanup(&file2);

    let password = "same_password";

    for file in [&file1, &file2] {
        let config = CredentialConfig::new()
            .with_file_path(file.to_string_lossy().to_string());

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

    // 读取两个文件并解析 salt
    let content1 = fs::read_to_string(&file1).expect("应该读取文件1");
    let content2 = fs::read_to_string(&file2).expect("应该读取文件2");

    let json1: serde_json::Value = serde_json::from_str(&content1).expect("应该解析 JSON");
    let json2: serde_json::Value = serde_json::from_str(&content2).expect("应该解析 JSON");

    let salt1 = json1.get("salt").expect("应该有 salt 字段");
    let salt2 = json2.get("salt").expect("应该有 salt 字段");

    // 两个文件的盐值应该不同
    assert_ne!(salt1, salt2, "每个加密文件应该使用唯一的盐值");

    cleanup(&file1);
    cleanup(&file2);
}
