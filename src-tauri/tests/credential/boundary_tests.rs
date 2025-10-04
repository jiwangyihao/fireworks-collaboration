//! 边界条件和输入验证测试
//!
//! 测试极端输入、空值、特殊字符等边界情况。

use fireworks_collaboration_lib::core::credential::{
    config::CredentialConfig,
    file_store::EncryptedFileStore,
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::path::PathBuf;
use std::fs;

fn get_test_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fireworks_boundary_test_{name}.enc"))
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_empty_string_credentials() {
    let store = MemoryCredentialStore::new();

    // 尝试创建具有空字符串的凭证
    let cred = Credential::new(
        "".to_string(),
        "".to_string(),
        "".to_string(),
    );

    // 应该允许添加（由业务逻辑决定是否有效）
    let result = store.add(cred);
    // 验证行为一致
    match result {
        Ok(_) => {
            // 如果允许添加，应该能查询到
            let retrieved = store.get("", Some(""));
            assert!(retrieved.is_ok());
        }
        Err(_) => {
            // 如果拒绝添加，应该返回错误
        }
    }
}

#[test]
fn test_very_long_password() {
    let store = MemoryCredentialStore::new();

    // 创建非常长的密码（10KB）
    let long_password: String = "a".repeat(10_000);

    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        long_password.clone(),
    );

    store.add(cred).expect("应该能添加长密码凭证");

    let retrieved = store.get("test.com", Some("user"))
        .expect("应该能查询")
        .expect("凭证应该存在");

    assert_eq!(retrieved.password_or_token.len(), 10_000);
    assert_eq!(retrieved.password_or_token, long_password);
}

#[test]
fn test_very_long_host_and_username() {
    let store = MemoryCredentialStore::new();

    // 超长主机名和用户名
    let long_host = format!("{}.com", "subdomain.".repeat(100));
    let long_username = "user_".repeat(1000);

    let cred = Credential::new(
        long_host.clone(),
        long_username.clone(),
        "password".to_string(),
    );

    store.add(cred).expect("应该能添加超长主机名和用户名");

    let retrieved = store.get(&long_host, Some(&long_username))
        .expect("应该能查询")
        .expect("凭证应该存在");

    assert_eq!(retrieved.host, long_host);
    assert_eq!(retrieved.username, long_username);
}

#[test]
fn test_unicode_in_all_fields() {
    let store = MemoryCredentialStore::new();

    // 使用各种 Unicode 字符
    let unicode_test_cases = vec![
        ("中文.com", "用户名", "密码123"),
        ("日本語.jp", "ユーザー", "パスワード"),
        ("한국어.kr", "사용자", "비밀번호"),
        ("русский.ru", "пользователь", "пароль"),
        ("emoji.com", "user🚀", "pass🔒"),
        ("mixed中文English.com", "user混合", "pass密码"),
    ];

    for (host, username, password) in unicode_test_cases {
        let cred = Credential::new(
            host.to_string(),
            username.to_string(),
            password.to_string(),
        );

        store.add(cred).unwrap_or_else(|_| panic!("应该能添加 Unicode 凭证: {host}"));

        let retrieved = store.get(host, Some(username))
            .expect("应该能查询")
            .expect("凭证应该存在");

        assert_eq!(retrieved.host, host);
        assert_eq!(retrieved.username, username);
        assert_eq!(retrieved.password_or_token, password);
    }
}

#[test]
fn test_special_characters_in_credentials() {
    let store = MemoryCredentialStore::new();

    // 各种特殊字符组合
    let special_chars = [r#"<script>alert('xss')</script>"#,
        r#"'; DROP TABLE credentials; --"#,
        r#"../../../etc/passwd"#,
        r#"null\0byte"#,
        r#"line1\nline2\rline3"#,
        r#"tab\there"#,
        "quote's and \"double\" quotes"];

    for (i, special) in special_chars.iter().enumerate() {
        let cred = Credential::new(
            format!("host{i}.com"),
            special.to_string(),
            special.to_string(),
        );

        store.add(cred).unwrap_or_else(|_| panic!("应该能添加特殊字符凭证: {special}"));

        let retrieved = store.get(&format!("host{i}.com"), Some(special))
            .expect("应该能查询")
            .expect("凭证应该存在");

        assert_eq!(retrieved.username, *special);
        assert_eq!(retrieved.password_or_token, *special);
    }
}

#[test]
fn test_whitespace_only_credentials() {
    let store = MemoryCredentialStore::new();

    // 仅包含空白字符
    let whitespace_cases = [
        " ",           // 单个空格
        "  ",          // 多个空格
        "\t",          // 制表符
        "\n",          // 换行符
        "   \t\n  ",   // 混合空白
    ];

    for (i, ws) in whitespace_cases.iter().enumerate() {
        let cred = Credential::new(
            format!("host{i}.com"),
            ws.to_string(),
            ws.to_string(),
        );

        let result = store.add(cred);
        
        // 无论是否允许，都应该行为一致
        if result.is_ok() {
            let retrieved = store.get(&format!("host{i}.com"), Some(ws));
            assert!(retrieved.is_ok());
        }
    }
}

#[test]
fn test_duplicate_credentials_handling() {
    let store = MemoryCredentialStore::new();

    let cred1 = Credential::new(
        "github.com".to_string(),
        "alice".to_string(),
        "password1".to_string(),
    );

    let cred2 = Credential::new(
        "github.com".to_string(),
        "alice".to_string(),
        "password2".to_string(),
    );

    // 添加第一个凭证
    store.add(cred1).expect("应该添加第一个凭证");

    // 尝试添加重复凭证（相同主机和用户名）
    let result = store.add(cred2);

    // 应该拒绝重复凭证
    assert!(result.is_err(), "不应该允许添加重复凭证");

    // 验证原凭证未被覆盖
    let retrieved = store.get("github.com", Some("alice"))
        .expect("应该能查询")
        .expect("凭证应该存在");

    assert_eq!(retrieved.password_or_token, "password1", "原密码不应被覆盖");
}

#[test]
fn test_encrypted_file_with_very_long_password() {
    let test_file = get_test_file("long_master_password");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = EncryptedFileStore::new(&config).expect("应该创建文件存储");

    // 使用非常长的主密码
    let long_master_password = "master_".repeat(1000);
    store.set_master_password(long_master_password.clone())
        .expect("应该接受长主密码");

    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        "data".to_string(),
    );

    store.add(cred).expect("应该能添加凭证");

    // 验证能用长密码解密
    let retrieved = store.get("test.com", Some("user"))
        .expect("应该能查询")
        .expect("凭证应该存在");

    assert_eq!(retrieved.password_or_token, "data");

    cleanup(&test_file);
}

#[test]
fn test_null_bytes_in_password() {
    let store = MemoryCredentialStore::new();

    // Rust 字符串可以包含 \0（使用十六进制转义避免误认为八进制）
    let password_with_null = "pass\0word\x00123";

    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        password_with_null.to_string(),
    );

    store.add(cred).expect("应该能添加包含 null 字节的密码");

    let retrieved = store.get("test.com", Some("user"))
        .expect("应该能查询")
        .expect("凭证应该存在");

    assert_eq!(retrieved.password_or_token, password_with_null);
}

#[test]
fn test_maximum_credentials_in_memory() {
    let store = MemoryCredentialStore::new();

    // 添加大量凭证测试内存限制
    let count = 1000;

    for i in 0..count {
        let cred = Credential::new(
            format!("host{i}.com"),
            format!("user{i}"),
            format!("password{i}"),
        );
        store.add(cred).unwrap_or_else(|_| panic!("应该添加凭证 {i}"));
    }

    let list = store.list().expect("应该列出所有凭证");
    assert_eq!(list.len(), count, "应该有 {count} 个凭证");

    // 验证随机凭证仍然可访问
    let random_id = 500;
    let retrieved = store.get(
        &format!("host{random_id}.com"),
        Some(&format!("user{random_id}"))
    )
    .expect("应该能查询")
    .expect("凭证应该存在");

    assert_eq!(retrieved.password_or_token, format!("password{random_id}"));
}

#[test]
fn test_host_without_tld() {
    let store = MemoryCredentialStore::new();

    // 不带顶级域名的主机
    let hosts = vec![
        "localhost",
        "192.168.1.1",
        "::1",
        "my-server",
        "dev-machine",
    ];

    for host in hosts {
        let cred = Credential::new(
            host.to_string(),
            "user".to_string(),
            "pass".to_string(),
        );

        store.add(cred).unwrap_or_else(|_| panic!("应该能添加主机: {host}"));

        let retrieved = store.get(host, Some("user"))
            .expect("应该能查询")
            .expect("凭证应该存在");

        assert_eq!(retrieved.host, host);
    }
}

#[test]
fn test_case_sensitive_credentials() {
    let store = MemoryCredentialStore::new();

    // 添加大小写不同的凭证
    let test_cases = vec![
        ("GitHub.com", "Alice"),
        ("github.com", "alice"),
        ("GITHUB.COM", "ALICE"),
    ];

    for (host, username) in &test_cases {
        let cred = Credential::new(
            host.to_string(),
            username.to_string(),
            format!("pass_{host}_{username}"),
        );
        store.add(cred).unwrap_or_else(|_| panic!("应该添加: {username}@{host}"));
    }

    // 验证所有凭证都独立存在（大小写敏感）
    let list = store.list().expect("应该列出所有凭证");
    assert_eq!(list.len(), 3, "应该有 3 个独立凭证");

    // 验证精确匹配
    for (host, username) in &test_cases {
        let retrieved = store.get(host, Some(username))
            .expect("应该能查询")
            .expect("凭证应该存在");

        assert_eq!(retrieved.host, *host);
        assert_eq!(retrieved.username, *username);
        assert_eq!(retrieved.password_or_token, format!("pass_{host}_{username}"));
    }
}
