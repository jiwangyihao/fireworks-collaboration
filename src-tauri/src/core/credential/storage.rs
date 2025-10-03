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
