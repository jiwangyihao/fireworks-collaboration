//! 凭证数据模型
//!
//! 定义凭证的数据结构和相关类型。
//!
//! # 示例
//!
//! ```rust
//! use fireworks_collaboration_lib::core::credential::Credential;
//! use std::time::{SystemTime, Duration};
//!
//! // 创建基本凭证
//! let cred = Credential::new(
//!     "github.com".to_string(),
//!     "user".to_string(),
//!     "token".to_string(),
//! );
//!
//! // 创建带过期时间的凭证
//! let expires_at = SystemTime::now() + Duration::from_secs(86400);
//! let cred_with_expiry = Credential::new_with_expiry(
//!     "github.com".to_string(),
//!     "user".to_string(),
//!     "token".to_string(),
//!     expires_at,
//! );
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::SystemTime;

/// 凭证信息
///
/// 存储 Git 操作所需的认证凭证，包括主机、用户名、密码/令牌等信息。
///
/// # 安全性
///
/// - `password_or_token` 字段在序列化时自动跳过，防止泄露到日志或文件
/// - 使用 `Display` trait 输出时自动脱敏
/// - 支持过期检测和最后使用时间跟踪
///
/// # 示例
///
/// ```rust
/// use fireworks_collaboration_lib::core::credential::Credential;
///
/// let cred = Credential::new(
///     "github.com".to_string(),
///     "alice".to_string(),
///     "ghp_1234567890abcdef".to_string(),
/// );
///
/// // 检查是否过期
/// assert!(!cred.is_expired());
///
/// // 获取脱敏后的密码
/// let masked = cred.masked_password();
/// assert!(masked.contains("****"));
/// ```
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Credential {
    /// 主机地址（如 github.com）
    pub host: String,

    /// 用户名
    pub username: String,

    /// 密码或令牌（敏感信息）
    #[serde(skip_serializing)]
    pub password_or_token: String,

    /// 过期时间（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<SystemTime>,

    /// 创建时间
    pub created_at: SystemTime,

    /// 最后使用时间（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<SystemTime>,
}

impl Credential {
    /// 创建新凭证
    pub fn new(host: String, username: String, password_or_token: String) -> Self {
        Self {
            host,
            username,
            password_or_token,
            expires_at: None,
            created_at: SystemTime::now(),
            last_used_at: None,
        }
    }

    /// 创建带过期时间的新凭证
    pub fn new_with_expiry(
        host: String,
        username: String,
        password_or_token: String,
        expires_at: SystemTime,
    ) -> Self {
        Self {
            host,
            username,
            password_or_token,
            expires_at: Some(expires_at),
            created_at: SystemTime::now(),
            last_used_at: None,
        }
    }

    /// 检查凭证是否已过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() > expires_at
        } else {
            false
        }
    }

    /// 更新最后使用时间
    pub fn update_last_used(&mut self) {
        self.last_used_at = Some(SystemTime::now());
    }

    /// 获取凭证的唯一标识符（host + username）
    pub fn identifier(&self) -> String {
        format!("{}@{}", self.username, self.host)
    }

    /// 获取脱敏后的密码/令牌（用于日志和显示）
    ///
    /// # 脱敏规则
    ///
    /// - 长度 ≤ 8: 显示为 "***"
    /// - 长度 > 8: 显示前 4 位和后 4 位，中间用 "****" 替代
    ///
    /// # 示例
    ///
    /// ```rust
    /// use fireworks_collaboration_lib::core::credential::Credential;
    ///
    /// let cred = Credential::new(
    ///     "github.com".to_string(),
    ///     "user".to_string(),
    ///     "ghp_1234567890abcdef".to_string(),
    /// );
    ///
    /// assert_eq!(cred.masked_password(), "ghp_****cdef");
    /// ```
    pub fn masked_password(&self) -> String {
        let token = &self.password_or_token;
        if token.len() <= 8 {
            "***".to_string()
        } else {
            let prefix = &token[..4];
            let suffix = &token[token.len() - 4..];
            format!("{prefix}****{suffix}")
        }
    }
}

/// 实现 Display trait 以支持脱敏显示
///
/// 凭证在日志中显示时会自动脱敏，防止敏感信息泄露。
impl fmt::Display for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Credential {{ host: {}, username: {}, password: {}, created_at: {:?}, expires_at: {:?} }}",
            self.host,
            self.username,
            self.masked_password(),
            self.created_at,
            self.expires_at
        )
    }
}

/// 实现 Debug trait 以支持脱敏调试输出
impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("host", &self.host)
            .field("username", &self.username)
            .field("password_or_token", &self.masked_password())
            .field("expires_at", &self.expires_at)
            .field("created_at", &self.created_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}
