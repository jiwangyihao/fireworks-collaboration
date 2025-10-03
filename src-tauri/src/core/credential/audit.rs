//! 凭证审计日志模块
//!
//! 提供凭证操作的审计跟踪能力，支持两种模式：
//! - 标准模式：记录操作类型、时间、结果，不记录凭证内容
//! - 审计模式：额外记录凭证内容的 SHA-256 哈希摘要
//!
//! # 安全性
//!
//! - 永远不记录明文密码或令牌
//! - 哈希摘要使用 SHA-256，加盐防止彩虹表攻击
//! - 审计日志可导出为 JSON 格式用于合规审查
//!
//! # 示例
//!
//! ```rust
//! use fireworks_collaboration_lib::core::credential::audit::{AuditLogger, AuditEvent, OperationType};
//!
//! let logger = AuditLogger::new(true); // 启用审计模式
//! logger.log_operation(
//!     OperationType::Add,
//!     "github.com",
//!     "user",
//!     Some("password123"),
//!     true,
//!     None,
//! );
//! ```

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// 凭证操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    /// 添加凭证
    Add,
    /// 获取凭证
    Get,
    /// 更新凭证
    Update,
    /// 删除凭证
    Remove,
    /// 列举凭证
    List,
    /// 验证凭证
    Validate,
    /// 凭证过期
    Expired,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationType::Add => write!(f, "add"),
            OperationType::Get => write!(f, "get"),
            OperationType::Update => write!(f, "update"),
            OperationType::Remove => write!(f, "remove"),
            OperationType::List => write!(f, "list"),
            OperationType::Validate => write!(f, "validate"),
            OperationType::Expired => write!(f, "expired"),
        }
    }
}

/// 审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    /// 操作类型
    pub operation: OperationType,

    /// 主机地址
    pub host: String,

    /// 用户名
    pub username: String,

    /// 操作时间
    #[serde(with = "system_time_serde")]
    pub timestamp: SystemTime,

    /// 操作是否成功
    pub success: bool,

    /// 错误消息（如果失败）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// 凭证内容的 SHA-256 哈希摘要（仅在审计模式下记录）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_hash: Option<String>,
}

// SystemTime 序列化/反序列化辅助模块
mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

/// 凭证审计日志记录器
///
/// 线程安全的审计日志记录器，支持标准模式和审计模式。
///
/// # 示例
///
/// ```rust
/// use fireworks_collaboration_lib::core::credential::audit::{AuditLogger, OperationType};
///
/// let logger = AuditLogger::new(true);
/// logger.log_operation(
///     OperationType::Add,
///     "github.com",
///     "user",
///     Some("password"),
///     true,
///     None,
/// );
///
/// let events = logger.get_events();
/// assert_eq!(events.len(), 1);
/// ```
pub struct AuditLogger {
    /// 是否启用审计模式（记录哈希摘要）
    audit_mode: bool,

    /// 审计事件列表（线程安全）
    events: Arc<Mutex<Vec<AuditEvent>>>,

    /// 哈希盐值（用于防止彩虹表攻击）
    salt: String,
}

impl AuditLogger {
    /// 创建新的审计日志记录器
    ///
    /// # 参数
    ///
    /// - `audit_mode`: 是否启用审计模式（记录哈希摘要）
    pub fn new(audit_mode: bool) -> Self {
        Self {
            audit_mode,
            events: Arc::new(Mutex::new(Vec::new())),
            salt: Self::generate_salt(),
        }
    }

    /// 生成随机盐值
    fn generate_salt() -> String {
        use std::time::UNIX_EPOCH;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("audit_salt_{}", now)
    }

    /// 计算凭证的 SHA-256 哈希摘要
    ///
    /// 使用格式: SHA256(salt + host + username + password)
    fn compute_credential_hash(&self, host: &str, username: &str, password: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.salt.as_bytes());
        hasher.update(host.as_bytes());
        hasher.update(username.as_bytes());
        hasher.update(password.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    /// 记录凭证操作
    ///
    /// # 参数
    ///
    /// - `operation`: 操作类型
    /// - `host`: 主机地址
    /// - `username`: 用户名
    /// - `password`: 密码/令牌（仅在审计模式下用于计算哈希，不会被存储）
    /// - `success`: 操作是否成功
    /// - `error`: 错误消息（如果失败）
    pub fn log_operation(
        &self,
        operation: OperationType,
        host: &str,
        username: &str,
        password: Option<&str>,
        success: bool,
        error: Option<String>,
    ) {
        let credential_hash = if self.audit_mode && password.is_some() {
            Some(self.compute_credential_hash(host, username, password.unwrap()))
        } else {
            None
        };

        let event = AuditEvent {
            operation,
            host: host.to_string(),
            username: username.to_string(),
            timestamp: SystemTime::now(),
            success,
            error,
            credential_hash,
        };

        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }

    /// 获取所有审计事件（克隆）
    pub fn get_events(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    /// 清除所有审计事件
    pub fn clear(&self) {
        if let Ok(mut events) = self.events.lock() {
            events.clear();
        }
    }

    /// 导出审计日志为 JSON 格式
    pub fn export_to_json(&self) -> Result<String, String> {
        let events = self.get_events();
        serde_json::to_string_pretty(&events).map_err(|e| format!("序列化失败: {}", e))
    }

    /// 获取事件数量
    pub fn event_count(&self) -> usize {
        self.events
            .lock()
            .map(|events| events.len())
            .unwrap_or(0)
    }

    /// 检查是否启用审计模式
    pub fn is_audit_mode(&self) -> bool {
        self.audit_mode
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Clone for AuditLogger {
    fn clone(&self) -> Self {
        Self {
            audit_mode: self.audit_mode,
            events: Arc::clone(&self.events),
            salt: self.salt.clone(),
        }
    }
}
