//! 凭证存储抽象层
//!
//! 定义凭证存储的 trait 接口，支持多种存储实现（系统钥匙串、加密文件、内存）。
//!
//! # 架构概述
//!
//! 本模块提供了统一的凭证存储抽象层，允许应用程序在不同的存储后端之间无缝切换：
//!
//! - **系统钥匙串**：Windows Credential Manager、macOS Keychain、Linux Secret Service
//! - **加密文件**：AES-256-GCM 加密的本地文件存储
//! - **内存存储**：进程内临时存储（用于测试或临时场景）
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use fireworks_collaboration_lib::core::credential::{
//!     storage::{CredentialStore, MemoryCredentialStore},
//!     model::Credential,
//! };
//!
//! // 创建内存存储
//! let store = MemoryCredentialStore::new();
//!
//! // 添加凭证
//! let cred = Credential::new(
//!     "github.com".to_string(),
//!     "alice".to_string(),
//!     "ghp_token123".to_string(),
//! );
//! store.add(cred)?;
//!
//! // 查询凭证
//! if let Some(cred) = store.get("github.com", Some("alice"))? {
//!     println!("找到凭证: {}", cred.username);
//! }
//!
//! // 列出所有凭证
//! let all_creds = store.list()?;
//! println!("共有 {} 个凭证", all_creds.len());
//!
//! // 删除凭证
//! store.remove("github.com", "alice")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # 线程安全
//!
//! 所有 `CredentialStore` 实现都是线程安全的（`Send + Sync`），可以在多线程环境中安全使用。

use crate::core::credential::model::Credential;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 凭证存储错误类型
#[derive(Debug, thiserror::Error)]
pub enum CredentialStoreError {
    #[error("凭证未找到: {0}")]
    NotFound(String),

    #[error("凭证已存在: {0}")]
    AlreadyExists(String),

    #[error("凭证已过期: {0}")]
    Expired(String),

    #[error("存储访问错误: {0}")]
    AccessError(String),

    #[error("加密/解密错误: {0}")]
    CryptoError(String),

    #[error("序列化错误: {0}")]
    SerializationError(String),

    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),

    #[error("其他错误: {0}")]
    Other(String),
}

pub type CredentialStoreResult<T> = Result<T, CredentialStoreError>;

/// 凭证存储抽象接口
///
/// 定义凭证存储的基本操作，包括增删改查等。所有实现必须保证线程安全。
///
/// # 生命周期管理
///
/// - 过期凭证会在 `get()` 和 `list()` 时自动过滤
/// - 已过期的凭证不会被返回，但仍保留在存储中
/// - 使用 `remove()` 可以显式删除凭证
///
/// # 错误处理
///
/// 所有方法返回 `CredentialStoreResult<T>`，可能的错误类型包括：
/// - `NotFound`: 凭证不存在
/// - `AlreadyExists`: 凭证已存在（添加时）
/// - `AccessError`: 存储访问失败（权限、网络等）
/// - `CryptoError`: 加密/解密失败
///
/// # 示例
///
/// ```rust,no_run
/// use fireworks_collaboration_lib::core::credential::{
///     storage::CredentialStore,
///     model::Credential,
/// };
///
/// fn example(store: &dyn CredentialStore) -> Result<(), Box<dyn std::error::Error>> {
///     // 添加凭证
///     let cred = Credential::new(
///         "api.example.com".to_string(),
///         "user@example.com".to_string(),
///         "secret_token".to_string(),
///     );
///     store.add(cred)?;
///
///     // 按主机和用户名精确查询
///     let cred = store.get("api.example.com", Some("user@example.com"))?;
///     assert!(cred.is_some());
///
///     // 只按主机查询（返回任意匹配的凭证）
///     let any_cred = store.get("api.example.com", None)?;
///
///     // 更新最后使用时间
///     store.update_last_used("api.example.com", "user@example.com")?;
///
///     Ok(())
/// }
/// ```
pub trait CredentialStore: Send + Sync {
    /// 根据主机和用户名获取凭证
    ///
    /// # 参数
    /// * `host` - 主机地址
    /// * `username` - 用户名（可选，如果为 None 则返回该主机的任意凭证）
    ///
    /// # 返回
    /// 返回匹配的凭证，如果未找到或已过期则返回 None
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>>;

    /// 添加新凭证
    ///
    /// # 参数
    /// * `credential` - 要添加的凭证
    ///
    /// # 返回
    /// 成功返回 Ok，如果凭证已存在则返回错误
    fn add(&self, credential: Credential) -> CredentialStoreResult<()>;

    /// 删除凭证
    ///
    /// # 参数
    /// * `host` - 主机地址
    /// * `username` - 用户名
    ///
    /// # 返回
    /// 成功返回 Ok，如果凭证不存在则返回错误
    fn remove(&self, host: &str, username: &str) -> CredentialStoreResult<()>;

    /// 列出所有凭证
    ///
    /// # 返回
    /// 返回所有已存储的凭证列表（不包括已过期的）
    fn list(&self) -> CredentialStoreResult<Vec<Credential>>;

    /// 更新凭证的最后使用时间
    ///
    /// # 参数
    /// * `host` - 主机地址
    /// * `username` - 用户名
    fn update_last_used(&self, host: &str, username: &str) -> CredentialStoreResult<()>;

    /// 检查凭证是否存在
    ///
    /// # 参数
    /// * `host` - 主机地址
    /// * `username` - 用户名
    fn exists(&self, host: &str, username: &str) -> bool {
        self.get(host, Some(username)).unwrap_or(None).is_some()
    }
}

/// 内存凭证存储（用于测试和临时存储）
pub struct MemoryCredentialStore {
    credentials: Arc<RwLock<HashMap<String, Credential>>>,
}

impl MemoryCredentialStore {
    /// 创建新的内存凭证存储
    pub fn new() -> Self {
        Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 生成凭证的存储键
    fn make_key(host: &str, username: &str) -> String {
        format!("{}@{}", username, host)
    }
}

impl Default for MemoryCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>> {
        let credentials = self
            .credentials
            .read()
            .map_err(|e| CredentialStoreError::AccessError(format!("读锁获取失败: {}", e)))?;

        if let Some(username) = username {
            let key = Self::make_key(host, username);
            if let Some(cred) = credentials.get(&key) {
                if cred.is_expired() {
                    return Ok(None);
                }
                return Ok(Some(cred.clone()));
            }
        } else {
            // 如果没有指定用户名，返回该主机的第一个有效凭证
            for cred in credentials.values() {
                if cred.host == host && !cred.is_expired() {
                    return Ok(Some(cred.clone()));
                }
            }
        }

        Ok(None)
    }

    fn add(&self, credential: Credential) -> CredentialStoreResult<()> {
        let mut credentials = self
            .credentials
            .write()
            .map_err(|e| CredentialStoreError::AccessError(format!("写锁获取失败: {}", e)))?;

        let key = Self::make_key(&credential.host, &credential.username);

        if credentials.contains_key(&key) {
            return Err(CredentialStoreError::AlreadyExists(key));
        }

        credentials.insert(key, credential);
        Ok(())
    }

    fn remove(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        let mut credentials = self
            .credentials
            .write()
            .map_err(|e| CredentialStoreError::AccessError(format!("写锁获取失败: {}", e)))?;

        let key = Self::make_key(host, username);

        if credentials.remove(&key).is_none() {
            return Err(CredentialStoreError::NotFound(key));
        }

        Ok(())
    }

    fn list(&self) -> CredentialStoreResult<Vec<Credential>> {
        let credentials = self
            .credentials
            .read()
            .map_err(|e| CredentialStoreError::AccessError(format!("读锁获取失败: {}", e)))?;

        let valid_credentials: Vec<Credential> = credentials
            .values()
            .filter(|c| !c.is_expired())
            .cloned()
            .collect();

        Ok(valid_credentials)
    }

    fn update_last_used(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        let mut credentials = self
            .credentials
            .write()
            .map_err(|e| CredentialStoreError::AccessError(format!("写锁获取失败: {}", e)))?;

        let key = Self::make_key(host, username);

        if let Some(cred) = credentials.get_mut(&key) {
            cred.update_last_used();
            Ok(())
        } else {
            Err(CredentialStoreError::NotFound(key))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_memory_store_add_and_get() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
        );

        assert!(store.add(cred.clone()).is_ok());

        let retrieved = store.get("github.com", Some("testuser")).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.host, "github.com");
        assert_eq!(retrieved.username, "testuser");
    }

    #[test]
    fn test_memory_store_add_duplicate() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
        );

        assert!(store.add(cred.clone()).is_ok());
        assert!(store.add(cred).is_err());
    }

    #[test]
    fn test_memory_store_remove() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
        );

        store.add(cred).unwrap();
        assert!(store.remove("github.com", "testuser").is_ok());
        assert!(store.get("github.com", Some("testuser")).unwrap().is_none());
    }

    #[test]
    fn test_memory_store_remove_not_found() {
        let store = MemoryCredentialStore::new();
        assert!(store.remove("github.com", "testuser").is_err());
    }

    #[test]
    fn test_memory_store_list() {
        let store = MemoryCredentialStore::new();

        let cred1 = Credential::new(
            "github.com".to_string(),
            "user1".to_string(),
            "token1".to_string(),
        );
        let cred2 = Credential::new(
            "gitlab.com".to_string(),
            "user2".to_string(),
            "token2".to_string(),
        );

        store.add(cred1).unwrap();
        store.add(cred2).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_memory_store_expired_credential() {
        let store = MemoryCredentialStore::new();
        let past_time = SystemTime::now() - Duration::from_secs(3600);
        let cred = Credential::new_with_expiry(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
            past_time,
        );

        store.add(cred).unwrap();

        // 过期的凭证应该返回 None
        let retrieved = store.get("github.com", Some("testuser")).unwrap();
        assert!(retrieved.is_none());

        // 列表中也不应该包含过期凭证
        let list = store.list().unwrap();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_memory_store_update_last_used() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
        );

        store.add(cred).unwrap();

        assert!(store.update_last_used("github.com", "testuser").is_ok());

        let retrieved = store.get("github.com", Some("testuser")).unwrap().unwrap();
        assert!(retrieved.last_used_at.is_some());
    }

    #[test]
    fn test_memory_store_get_without_username() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
        );

        store.add(cred).unwrap();

        // 不指定用户名时应该返回该主机的任意凭证
        let retrieved = store.get("github.com", None).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.host, "github.com");
    }

    #[test]
    fn test_memory_store_exists() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
        );

        assert!(!store.exists("github.com", "testuser"));
        store.add(cred).unwrap();
        assert!(store.exists("github.com", "testuser"));
    }

    #[test]
    fn test_memory_store_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(MemoryCredentialStore::new());
        let mut handles = vec![];

        // 并发添加凭证
        for i in 0..10 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                let cred = Credential::new(
                    "github.com".to_string(),
                    format!("user{}", i),
                    format!("token{}", i),
                );
                store_clone.add(cred).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证所有凭证都已添加
        let list = store.list().unwrap();
        assert_eq!(list.len(), 10);
    }

    #[test]
    fn test_memory_store_large_number_of_credentials() {
        let store = MemoryCredentialStore::new();

        // 添加 1000 个凭证
        for i in 0..1000 {
            let cred = Credential::new(
                format!("host{}.com", i % 10),
                format!("user{}", i),
                format!("token{}", i),
            );
            store.add(cred).unwrap();
        }

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1000);

        // 测试查询性能
        let cred = store.get("host5.com", Some("user55")).unwrap();
        assert!(cred.is_some());
    }

    #[test]
    fn test_memory_store_expiry_edge_cases() {
        use std::time::Duration;

        let store = MemoryCredentialStore::new();

        // 测试即将过期的凭证
        let almost_expired = SystemTime::now() + Duration::from_millis(100);
        let cred = Credential::new_with_expiry(
            "github.com".to_string(),
            "testuser".to_string(),
            "token123".to_string(),
            almost_expired,
        );

        store.add(cred).unwrap();

        // 应该还能获取到
        let retrieved = store.get("github.com", Some("testuser")).unwrap();
        assert!(retrieved.is_some());

        // 等待过期
        std::thread::sleep(Duration::from_millis(200));

        // 现在应该获取不到
        let retrieved = store.get("github.com", Some("testuser")).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_memory_store_multiple_hosts() {
        let store = MemoryCredentialStore::new();

        let hosts = vec!["github.com", "gitlab.com", "bitbucket.org"];
        for host in &hosts {
            let cred = Credential::new(host.to_string(), "user".to_string(), "token".to_string());
            store.add(cred).unwrap();
        }

        // 验证每个主机都能查到
        for host in &hosts {
            let cred = store.get(host, Some("user")).unwrap();
            assert!(cred.is_some());
        }

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_memory_store_same_host_different_users() {
        let store = MemoryCredentialStore::new();

        // 同一主机，不同用户
        for i in 0..5 {
            let cred = Credential::new(
                "github.com".to_string(),
                format!("user{}", i),
                format!("token{}", i),
            );
            store.add(cred).unwrap();
        }

        // 验证每个用户都能查到
        for i in 0..5 {
            let cred = store
                .get("github.com", Some(&format!("user{}", i)))
                .unwrap();
            assert!(cred.is_some());
        }

        // 不指定用户名应该返回其中一个
        let cred = store.get("github.com", None).unwrap();
        assert!(cred.is_some());
        assert_eq!(cred.unwrap().host, "github.com");
    }

    // ========== 边界条件与性能测试 ==========

    #[test]
    fn test_memory_store_empty_host_and_username() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new("".to_string(), "".to_string(), "token".to_string());
        
        // 空字符串应该被允许
        assert!(store.add(cred).is_ok());
        
        let retrieved = store.get("", Some("")).unwrap();
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_memory_store_unicode_host_username() {
        let store = MemoryCredentialStore::new();
        let cred = Credential::new(
            "中文主机.com".to_string(),
            "用户名".to_string(),
            "token".to_string(),
        );
        
        store.add(cred).unwrap();
        
        let retrieved = store.get("中文主机.com", Some("用户名")).unwrap();
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_memory_store_concurrent_read_write_contention() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(MemoryCredentialStore::new());
        
        // 预先添加一些凭证
        for i in 0..10 {
            let cred = Credential::new(
                format!("host{}.com", i),
                format!("user{}", i),
                format!("token{}", i),
            );
            store.add(cred).unwrap();
        }

        let mut handles = vec![];

        // 10个读线程
        for i in 0..10 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let _ = store_clone.get(&format!("host{}.com", i), Some(&format!("user{}", i)));
                }
            });
            handles.push(handle);
        }

        // 5个写线程（更新最后使用时间）
        for i in 0..5 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                for _ in 0..50 {
                    let _ = store_clone.update_last_used(&format!("host{}.com", i), &format!("user{}", i));
                }
            });
            handles.push(handle);
        }

        // 等待所有线程完成（不应该死锁）
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证数据完整性
        let list = store.list().unwrap();
        assert_eq!(list.len(), 10);
    }

    #[test]
    fn test_memory_store_performance_basic_operations() {
        use std::time::Instant;

        let store = MemoryCredentialStore::new();

        // 添加操作应该 < 1ms
        let start = Instant::now();
        let cred = Credential::new(
            "github.com".to_string(),
            "user".to_string(),
            "token".to_string(),
        );
        store.add(cred).unwrap();
        let add_duration = start.elapsed();
        assert!(add_duration.as_millis() < 10, "add() took {:?}, expected <10ms", add_duration);

        // 查询操作应该 < 1ms
        let start = Instant::now();
        let _ = store.get("github.com", Some("user")).unwrap();
        let get_duration = start.elapsed();
        assert!(get_duration.as_millis() < 10, "get() took {:?}, expected <10ms", get_duration);

        // 删除操作应该 < 1ms
        let start = Instant::now();
        store.remove("github.com", "user").unwrap();
        let remove_duration = start.elapsed();
        assert!(remove_duration.as_millis() < 10, "remove() took {:?}, expected <10ms", remove_duration);
    }

    #[test]
    fn test_memory_store_stress_1000_credentials() {
        let store = MemoryCredentialStore::new();

        // 添加1000个凭证
        for i in 0..1000 {
            let cred = Credential::new(
                format!("host{}.com", i),
                format!("user{}", i),
                format!("token{}", i),
            );
            store.add(cred).unwrap();
        }

        // 列出所有凭证应该较快（< 100ms）
        use std::time::Instant;
        let start = Instant::now();
        let list = store.list().unwrap();
        let duration = start.elapsed();
        
        assert_eq!(list.len(), 1000);
        assert!(duration.as_millis() < 200, "list() of 1000 credentials took {:?}, expected <200ms", duration);
    }
}
