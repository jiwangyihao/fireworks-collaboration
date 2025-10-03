//! 凭证管理配置
//!
//! 定义凭证存储和管理相关的配置结构。

use serde::{Deserialize, Serialize};

/// 凭证存储类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    /// 系统钥匙串（macOS Keychain、Windows Credential Manager、Linux Secret Service）
    System,
    /// 加密文件存储
    File,
    /// 内存临时存储（进程重启后丢失）
    Memory,
}

impl Default for StorageType {
    fn default() -> Self {
        StorageType::System
    }
}

/// 凭证管理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialConfig {
    /// 存储类型（system/file/memory）
    #[serde(default)]
    pub storage: StorageType,

    /// 默认凭证过期时间（秒），None 表示永不过期
    #[serde(default = "default_ttl_seconds")]
    pub default_ttl_seconds: Option<u64>,

    /// 是否启用调试日志（包含敏感信息，仅用于开发调试）
    #[serde(default)]
    pub debug_logging: bool,

    /// 是否启用审计模式（记录凭证操作的哈希摘要）
    #[serde(default)]
    pub audit_mode: bool,

    /// 是否在敏感操作前要求确认
    #[serde(default)]
    pub require_confirmation: bool,

    /// 加密文件存储路径（当 storage=file 时使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,

    /// 密钥缓存时间（秒），用于加密文件模式
    #[serde(default = "default_key_cache_ttl")]
    pub key_cache_ttl_seconds: u64,
}

fn default_ttl_seconds() -> Option<u64> {
    // 默认 90 天
    Some(90 * 24 * 60 * 60)
}

fn default_key_cache_ttl() -> u64 {
    // 默认缓存 1 小时
    3600
}

impl Default for CredentialConfig {
    fn default() -> Self {
        Self {
            storage: StorageType::default(),
            default_ttl_seconds: default_ttl_seconds(),
            debug_logging: false,
            audit_mode: false,
            require_confirmation: false,
            file_path: None,
            key_cache_ttl_seconds: default_key_cache_ttl(),
        }
    }
}

impl CredentialConfig {
    /// 创建新的配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置存储类型
    pub fn with_storage(mut self, storage: StorageType) -> Self {
        self.storage = storage;
        self
    }

    /// 设置默认 TTL
    pub fn with_ttl(mut self, ttl_seconds: Option<u64>) -> Self {
        self.default_ttl_seconds = ttl_seconds;
        self
    }

    /// 启用调试日志
    pub fn with_debug_logging(mut self, enabled: bool) -> Self {
        self.debug_logging = enabled;
        self
    }

    /// 启用审计模式
    pub fn with_audit_mode(mut self, enabled: bool) -> Self {
        self.audit_mode = enabled;
        self
    }

    /// 设置加密文件路径
    pub fn with_file_path(mut self, path: String) -> Self {
        self.file_path = Some(path);
        self
    }

    /// 验证配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        // 如果使用文件存储，必须指定文件路径
        if self.storage == StorageType::File && self.file_path.is_none() {
            return Err("使用文件存储时必须指定 file_path".to_string());
        }

        // TTL 不能为 0
        if let Some(ttl) = self.default_ttl_seconds {
            if ttl == 0 {
                return Err("default_ttl_seconds 不能为 0".to_string());
            }
        }

        // 密钥缓存时间必须大于 0
        if self.key_cache_ttl_seconds == 0 {
            return Err("key_cache_ttl_seconds 必须大于 0".to_string());
        }

        Ok(())
    }

    /// 获取实际使用的存储类型（考虑回退策略）
    pub fn effective_storage_type(&self) -> StorageType {
        // 在 P6.0 阶段，直接返回配置的存储类型
        // 后续阶段会实现自动回退逻辑
        self.storage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CredentialConfig::default();
        assert_eq!(config.storage, StorageType::System);
        assert!(config.default_ttl_seconds.is_some());
        assert!(!config.debug_logging);
        assert!(!config.audit_mode);
    }

    #[test]
    fn test_config_builder() {
        let config = CredentialConfig::new()
            .with_storage(StorageType::Memory)
            .with_ttl(Some(3600))
            .with_debug_logging(true)
            .with_audit_mode(true);

        assert_eq!(config.storage, StorageType::Memory);
        assert_eq!(config.default_ttl_seconds, Some(3600));
        assert!(config.debug_logging);
        assert!(config.audit_mode);
    }

    #[test]
    fn test_config_validation_file_storage_without_path() {
        let config = CredentialConfig::new().with_storage(StorageType::File);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_file_storage_with_path() {
        let config = CredentialConfig::new()
            .with_storage(StorageType::File)
            .with_file_path("/tmp/credentials.enc".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_zero_ttl() {
        let config = CredentialConfig::new().with_ttl(Some(0));
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = CredentialConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CredentialConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.storage, deserialized.storage);
        assert_eq!(config.default_ttl_seconds, deserialized.default_ttl_seconds);
    }

    #[test]
    fn test_storage_type_serialization() {
        assert_eq!(
            serde_json::to_string(&StorageType::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&StorageType::File).unwrap(),
            "\"file\""
        );
        assert_eq!(
            serde_json::to_string(&StorageType::Memory).unwrap(),
            "\"memory\""
        );
    }

    #[test]
    fn test_storage_type_deserialization() {
        assert_eq!(
            serde_json::from_str::<StorageType>("\"system\"").unwrap(),
            StorageType::System
        );
        assert_eq!(
            serde_json::from_str::<StorageType>("\"file\"").unwrap(),
            StorageType::File
        );
        assert_eq!(
            serde_json::from_str::<StorageType>("\"memory\"").unwrap(),
            StorageType::Memory
        );
    }

    #[test]
    fn test_effective_storage_type() {
        let config = CredentialConfig::new().with_storage(StorageType::Memory);
        assert_eq!(config.effective_storage_type(), StorageType::Memory);
    }

    // ========== 边界条件与兼容性测试 ==========

    #[test]
    fn test_config_backward_compatibility_missing_fields() {
        // 测试缺少某些字段时的默认值填充
        let json = r#"{"storage":"memory"}"#;
        let config: CredentialConfig = serde_json::from_str(json).unwrap();
        
        assert_eq!(config.storage, StorageType::Memory);
        assert!(config.default_ttl_seconds.is_some());
        assert!(!config.debug_logging);
        assert!(!config.audit_mode);
    }

    #[test]
    fn test_config_validation_zero_key_cache_ttl() {
        // 手动构造无效配置（绕过构建器）
        let mut config = CredentialConfig::default();
        config.key_cache_ttl_seconds = 0;
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_with_extremely_large_ttl() {
        // 测试极大的TTL值（100年）
        let config = CredentialConfig::new()
            .with_ttl(Some(100 * 365 * 24 * 60 * 60));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let original = CredentialConfig::new()
            .with_storage(StorageType::File)
            .with_file_path("/test/path.enc".to_string())
            .with_ttl(Some(86400))
            .with_debug_logging(true)
            .with_audit_mode(true);

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CredentialConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(original.storage, deserialized.storage);
        assert_eq!(original.file_path, deserialized.file_path);
        assert_eq!(original.default_ttl_seconds, deserialized.default_ttl_seconds);
        assert_eq!(original.debug_logging, deserialized.debug_logging);
        assert_eq!(original.audit_mode, deserialized.audit_mode);
    }

    #[test]
    fn test_config_default_values_match_spec() {
        let config = CredentialConfig::default();
        
        assert_eq!(config.storage, StorageType::System);
        assert_eq!(config.default_ttl_seconds, Some(90 * 24 * 60 * 60));
        assert_eq!(config.key_cache_ttl_seconds, 3600);
        assert!(!config.debug_logging);
        assert!(!config.require_confirmation);
    }
}
