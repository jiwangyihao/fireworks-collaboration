//! 凭证存储与安全管理模块
//!
//! 本模块提供统一的凭证存储框架，支持系统钥匙串、加密文件、内存临时存储三层策略。
//! 用于安全存储和管理 Git 操作所需的凭证信息。

pub mod config;
pub mod factory;
pub mod model;
pub mod storage;

// Platform-specific keychain implementations
#[cfg(target_os = "windows")]
pub mod keychain_windows;

#[cfg(not(target_os = "windows"))]
pub mod keychain_unix;

// Encrypted file storage
pub mod file_store;

pub use config::CredentialConfig;
pub use factory::CredentialStoreFactory;
pub use model::Credential;
pub use storage::CredentialStore;
