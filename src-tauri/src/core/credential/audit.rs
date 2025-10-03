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
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

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

    /// 持久化日志文件路径（可选）
    log_file_path: Option<PathBuf>,

    /// 访问控制状态
    access_control: Arc<Mutex<AccessControl>>,
}

/// 访问控制状态
#[derive(Debug, Clone)]
struct AccessControl {
    /// 认证失败次数
    failure_count: u32,
    /// 最后失败时间
    last_failure_time: Option<SystemTime>,
    /// 是否已锁定
    locked: bool,
    /// 锁定到期时间
    locked_until: Option<SystemTime>,
    /// 最大失败次数
    max_failures: u32,
    /// 锁定时长（秒）
    lockout_duration_secs: u64,
}

impl AccessControl {
    fn new() -> Self {
        Self {
            failure_count: 0,
            last_failure_time: None,
            locked: false,
            locked_until: None,
            max_failures: 5, // 默认最多5次失败
            lockout_duration_secs: 1800, // 默认锁定30分钟
        }
    }

    /// 记录失败尝试
    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(SystemTime::now());

        if self.failure_count >= self.max_failures {
            self.locked = true;
            self.locked_until = Some(
                SystemTime::now() + Duration::from_secs(self.lockout_duration_secs),
            );
        }
    }

    /// 检查是否被锁定
    fn is_locked(&self) -> bool {
        if !self.locked {
            return false;
        }

        // 检查锁定是否已过期
        if let Some(locked_until) = self.locked_until {
            if SystemTime::now() >= locked_until {
                return false; // 锁定已过期
            }
        }

        true
    }

    /// 重置失败计数
    fn reset(&mut self) {
        self.failure_count = 0;
        self.last_failure_time = None;
        self.locked = false;
        self.locked_until = None;
    }
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
            log_file_path: None,
            access_control: Arc::new(Mutex::new(AccessControl::new())),
        }
    }

    /// 创建带持久化日志文件的审计日志记录器
    ///
    /// # 参数
    ///
    /// - `audit_mode`: 是否启用审计模式
    /// - `log_file_path`: 日志文件路径
    pub fn with_log_file<P: AsRef<Path>>(audit_mode: bool, log_file_path: P) -> Result<Self, String> {
        let path = log_file_path.as_ref().to_path_buf();

        // 确保日志目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建日志目录失败: {}", e))?;
        }

        let mut logger = Self::new(audit_mode);
        logger.log_file_path = Some(path.clone());

        // 尝试从文件加载现有日志
        if path.exists() {
            match logger.load_from_file(&path) {
                Ok(_) => {
                    tracing::info!("已从 {:?} 加载 {} 条审计日志", path, logger.event_count());
                }
                Err(e) => {
                    tracing::warn!("加载审计日志失败: {}, 将创建新日志文件", e);
                }
            }
        }

        Ok(logger)
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

        // 记录到内存
        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }

        // 持久化到文件（如果配置了）
        if let Some(path) = &self.log_file_path {
            if let Err(e) = self.append_to_file(&event, path) {
                tracing::warn!("写入审计日志文件失败: {}", e);
            }
        }
    }

    /// 从文件加载审计日志
    fn load_from_file(&self, path: &Path) -> Result<(), String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("读取文件失败: {}", e))?;

        let loaded_events: Vec<AuditEvent> = serde_json::from_str(&content)
            .map_err(|e| format!("解析JSON失败: {}", e))?;

        if let Ok(mut events) = self.events.lock() {
            *events = loaded_events;
        }

        Ok(())
    }

    /// 追加事件到文件
    fn append_to_file(&self, event: &AuditEvent, path: &Path) -> Result<(), String> {
        // 读取现有事件
        let mut all_events = if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("读取文件失败: {}", e))?;
            serde_json::from_str::<Vec<AuditEvent>>(&content)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // 添加新事件
        all_events.push(event.clone());

        // 写回文件
        let file = File::create(path)
            .map_err(|e| format!("创建文件失败: {}", e))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &all_events)
            .map_err(|e| format!("写入JSON失败: {}", e))?;

        Ok(())
    }

    /// 清理过期的审计日志
    ///
    /// # 参数
    ///
    /// - `retention_days`: 保留天数，早于此时间的日志将被删除
    ///
    /// # 返回
    ///
    /// 返回删除的日志数量
    pub fn cleanup_expired_logs(&self, retention_days: u64) -> Result<usize, String> {
        let cutoff_time = SystemTime::now()
            - Duration::from_secs(retention_days * 24 * 60 * 60);

        let removed_count = if let Ok(mut events) = self.events.lock() {
            let original_len = events.len();
            events.retain(|event| event.timestamp >= cutoff_time);
            let new_len = events.len();

            // 如果有日志文件，更新它
            if let Some(path) = &self.log_file_path {
                if let Err(e) = self.save_to_file(&events, path) {
                    tracing::warn!("保存清理后的日志文件失败: {}", e);
                }
            }

            original_len - new_len
        } else {
            0
        };

        Ok(removed_count)
    }

    /// 保存事件到文件（覆盖写）
    fn save_to_file(&self, events: &[AuditEvent], path: &Path) -> Result<(), String> {
        let file = File::create(path)
            .map_err(|e| format!("创建文件失败: {}", e))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, events)
            .map_err(|e| format!("写入JSON失败: {}", e))?;
        Ok(())
    }

    /// 检查是否被锁定
    pub fn is_locked(&self) -> bool {
        self.access_control
            .lock()
            .map(|ac| ac.is_locked())
            .unwrap_or(false)
    }

    /// 记录认证失败
    pub fn record_auth_failure(&self) {
        if let Ok(mut ac) = self.access_control.lock() {
            ac.record_failure();
        }
    }

    /// 重置访问控制（用于管理员解锁）
    pub fn reset_access_control(&self) {
        if let Ok(mut ac) = self.access_control.lock() {
            ac.reset();
        }
    }

    /// 获取剩余失败次数
    pub fn remaining_attempts(&self) -> u32 {
        self.access_control
            .lock()
            .map(|ac| {
                if ac.failure_count < ac.max_failures {
                    ac.max_failures - ac.failure_count
                } else {
                    0
                }
            })
            .unwrap_or(0)
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
            log_file_path: self.log_file_path.clone(),
            access_control: Arc::clone(&self.access_control),
        }
    }
}
