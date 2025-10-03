//! Credential store factory with automatic fallback.
//!
//! This module provides a factory for creating credential stores with automatic
//! fallback logic: system keychain -> encrypted file -> memory.
//!
//! # 设计理念
//!
//! 工厂模式确保应用程序始终能够获得可用的凭证存储，即使某些后端不可用：
//!
//! 1. **首选**：系统钥匙串（最安全，跨应用共享）
//! 2. **回退**：加密文件（需要主密码，本地存储）
//! 3. **兜底**：内存存储（进程生命周期，用于测试）
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use fireworks_collaboration_lib::core::credential::{
//!     config::{CredentialConfig, StorageType},
//!     factory::CredentialStoreFactory,
//!     model::Credential,
//! };
//!
//! // 方式1：请求系统钥匙串，失败时自动回退
//! let config = CredentialConfig::new()
//!     .with_storage(StorageType::System)
//!     .with_file_path("/tmp/creds.enc".to_string()); // 回退用
//!
//! let store = CredentialStoreFactory::create(&config)?;
//!
//! // 方式2：直接使用内存存储（测试场景）
//! let config = CredentialConfig::new()
//!     .with_storage(StorageType::Memory);
//!
//! let store = CredentialStoreFactory::create(&config)?;
//!
//! // 使用存储
//! let cred = Credential::new("host".to_string(), "user".to_string(), "pass".to_string());
//! store.add(cred)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # 平台差异
//!
//! - **Windows**: 使用 Credential Manager
//! - **macOS**: 使用 Keychain
//! - **Linux**: 使用 Secret Service (GNOME Keyring, KWallet 等)
//!
//! # 回退行为
//!
//! 当请求的存储类型不可用时（例如系统钥匙串服务未运行），工厂会自动尝试下一级存储：
//!
//! - System → File → Memory
//! - File → Memory
//! - Memory → 总是成功
//!
//! 每次回退都会通过 `tracing::warn!` 记录警告日志。

use super::{
    config::{CredentialConfig, StorageType},
    storage::CredentialStore,
};
use std::sync::Arc;

/// Factory for creating credential stores with automatic fallback.
///
/// # 功能
///
/// - 根据配置创建合适的凭证存储实现
/// - 自动处理平台差异（Windows/macOS/Linux）
/// - 提供多层回退机制，确保始终可用
///
/// # 回退策略
///
/// 根据 `CredentialConfig::storage` 配置：
///
/// - `StorageType::System`: 尝试系统钥匙串 → 加密文件 → 内存
/// - `StorageType::File`: 尝试加密文件 → 内存
/// - `StorageType::Memory`: 直接使用内存（总是成功）
///
/// # 线程安全
///
/// 返回的存储实例是 `Arc<dyn CredentialStore>`，可以安全地在多线程间共享和克隆。
///
/// # 示例
///
/// ```rust,no_run
/// use fireworks_collaboration_lib::core::credential::{
///     config::{CredentialConfig, StorageType},
///     factory::CredentialStoreFactory,
/// };
///
/// // 生产环境：优先使用系统钥匙串
/// let config = CredentialConfig::new()
///     .with_storage(StorageType::System)
///     .with_file_path("/var/lib/app/credentials.enc".to_string());
///
/// let store = CredentialStoreFactory::create(&config)?;
/// // 如果系统钥匙串可用，使用之；否则回退到文件或内存
///
/// // 开发/测试环境：使用内存存储
/// let test_config = CredentialConfig::new()
///     .with_storage(StorageType::Memory);
///
/// let test_store = CredentialStoreFactory::create(&test_config)?;
/// // 总是返回内存存储
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct CredentialStoreFactory;

impl CredentialStoreFactory {
    /// Creates a credential store based on the configuration.
    ///
    /// Fallback logic:
    /// 1. System keychain (if configured and available)
    /// 2. Encrypted file (if system keychain fails)
    /// 3. Memory (if encrypted file fails)
    ///
    /// # Arguments
    /// * `config` - Credential configuration
    ///
    /// # Returns
    /// A boxed credential store implementation
    pub fn create(config: &CredentialConfig) -> Result<Arc<dyn CredentialStore>, String> {
        match config.storage {
            StorageType::System => Self::try_system_keychain(config)
                .or_else(|e| {
                    tracing::warn!("System keychain unavailable: {}, falling back to encrypted file", e);
                    Self::try_encrypted_file(config)
                })
                .or_else(|e| {
                    tracing::warn!("Encrypted file storage unavailable: {}, falling back to memory", e);
                    Self::create_memory_store(config)
                }),
            StorageType::File => Self::try_encrypted_file(config).or_else(|e| {
                tracing::warn!("Encrypted file storage unavailable: {}, falling back to memory", e);
                Self::create_memory_store(config)
            }),
            StorageType::Memory => Self::create_memory_store(config),
        }
    }

    /// Attempts to create a system keychain store.
    #[cfg(target_os = "windows")]
    fn try_system_keychain(
        _config: &CredentialConfig,
    ) -> Result<Arc<dyn CredentialStore>, String> {
        use super::keychain_windows::WindowsCredentialStore;
        WindowsCredentialStore::new().map(|store| Arc::new(store) as Arc<dyn CredentialStore>)
    }

    /// Attempts to create a system keychain store (macOS/Linux).
    #[cfg(not(target_os = "windows"))]
    fn try_system_keychain(
        _config: &CredentialConfig,
    ) -> Result<Arc<dyn CredentialStore>, String> {
        use super::keychain_unix::UnixCredentialStore;
        UnixCredentialStore::new().map(|store| Arc::new(store) as Arc<dyn CredentialStore>)
    }

    /// Attempts to create an encrypted file store.
    fn try_encrypted_file(
        config: &CredentialConfig,
    ) -> Result<Arc<dyn CredentialStore>, String> {
        use super::file_store::EncryptedFileStore;
        EncryptedFileStore::new(config)
            .map(|store| Arc::new(store) as Arc<dyn CredentialStore>)
    }

    /// Creates a memory store (always succeeds).
    fn create_memory_store(
        _config: &CredentialConfig,
    ) -> Result<Arc<dyn CredentialStore>, String> {
        use super::storage::MemoryCredentialStore;
        Ok(Arc::new(MemoryCredentialStore::new()) as Arc<dyn CredentialStore>)
    }
}
