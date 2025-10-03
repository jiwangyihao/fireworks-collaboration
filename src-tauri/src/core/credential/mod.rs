//! 凭证存储与安全管理模块
//!
//! 本模块提供统一的凭证存储框架，支持系统钥匙串、加密文件、内存临时存储三层策略。
//! 用于安全存储和管理 Git 操作所需的凭证信息。

pub mod config;
pub mod model;
pub mod storage;

pub use config::CredentialConfig;
pub use model::Credential;
pub use storage::CredentialStore;
