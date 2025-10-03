//! Encrypted file-based credential storage.
//!
//! This module provides encrypted file storage for credentials using:
//! - AES-256-GCM for encryption
//! - Argon2id for key derivation from master password
//! - HMAC-SHA256 for integrity checking
//! - Zeroize for secure memory cleanup
//!
//! # 安全设计
//!
//! ## 加密方案
//!
//! - **算法**: AES-256-GCM (AEAD)
//! - **密钥长度**: 256 位
//! - **Nonce**: 每次加密随机生成 96 位
//! - **认证**: GCM 模式内置认证标签
//!
//! ## 密钥派生
//!
//! - **算法**: Argon2id
//! - **参数**: m_cost=64MB, t_cost=3, p_cost=1
//! - **盐值**: 每个文件随机生成，存储在文件头
//! - **输出**: 32 字节密钥
//!
//! ## 完整性保护
//!
//! - **HMAC-SHA256**: 验证密文未被篡改
//! - **Salt**: 防止彩虹表攻击
//! - **IV/Nonce**: 防止重放攻击
//!
//! # 性能优化
//!
//! ## 密钥缓存
//!
//! 由于 Argon2id 密钥派生耗时较长（~1-2秒），实现了密钥缓存机制：
//!
//! - 首次派生后缓存密钥
//! - 默认 TTL: 5 分钟（可配置）
//! - 缓存失效后自动重新派生
//!
//! ## 并发控制
//!
//! - 使用 `Mutex` 保护文件读写，防止并发冲突
//! - 支持多线程安全访问
//!
//! # 使用示例
//!
//! ```rust,no_run
//! use fireworks_collaboration_lib::core::credential::{
//!     config::CredentialConfig,
//!     file_store::EncryptedFileStore,
//!     storage::CredentialStore,
//!     model::Credential,
//! };
//!
//! // 创建加密文件存储
//! let config = CredentialConfig::new()
//!     .with_file_path("/path/to/credentials.enc".to_string());
//!
//! let store = EncryptedFileStore::new(&config)?;
//!
//! // 设置主密码（必须在使用前调用）
//! store.set_master_password("my-secure-password".to_string())?;
//!
//! // 添加凭证（首次会触发密钥派生，耗时 1-2 秒）
//! let cred = Credential::new(
//!     "github.com".to_string(),
//!     "user".to_string(),
//!     "token".to_string(),
//! );
//! store.add(cred)?;
//!
//! // 后续操作使用缓存密钥，速度很快（<10ms）
//! let retrieved = store.get("github.com", Some("user"))?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # 文件格式
//!
//! 加密文件采用 JSON 格式存储：
//!
//! ```json
//! {
//!   "version": 1,
//!   "salt": "base64-encoded-salt",
//!   "nonce": "base64-encoded-nonce",
//!   "ciphertext": "base64-encoded-encrypted-data",
//!   "hmac": "base64-encoded-hmac"
//! }
//! ```
//!
//! # 安全注意事项
//!
//! 1. **主密码强度**: 建议使用 12+ 字符的强密码
//! 2. **文件权限**: 确保加密文件只有当前用户可读写 (0600)
//! 3. **密码存储**: 主密码不应硬编码在代码中，建议从环境变量或安全提示获取
//! 4. **备份**: 定期备份加密文件，但不要在不安全的位置存储主密码
//!
//! # 错误处理
//!
//! - `AccessError`: 文件读写失败（权限、磁盘空间等）
//! - `CryptoError`: 密码错误或数据损坏
//! - `SerializationError`: JSON 序列化/反序列化失败

use super::{
    config::CredentialConfig,
    model::Credential,
    storage::{CredentialStore, CredentialStoreError, CredentialStoreResult},
};
use base64::{engine::general_purpose, Engine as _};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use zeroize::ZeroizeOnDrop;

type HmacSha256 = Hmac<Sha256>;

/// File format version for compatibility.
const FILE_VERSION: u32 = 1;

/// Default Argon2 parameters (balanced security/performance).
const ARGON2_M_COST: u32 = 65536; // 64 MB memory
const ARGON2_T_COST: u32 = 3; // 3 iterations
const ARGON2_P_COST: u32 = 1; // 1 parallelism

/// Nonce size for AES-256-GCM (96 bits).
const NONCE_SIZE: usize = 12;

/// Master password wrapper with zeroization.
#[derive(Clone, ZeroizeOnDrop)]
struct MasterPassword(String);

impl MasterPassword {
    fn new(password: String) -> Self {
        Self(password)
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// Encryption key with automatic zeroization.
#[derive(ZeroizeOnDrop)]
struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

/// Cached key with expiration.
struct CachedKey {
    key: EncryptionKey,
    expires_at: SystemTime,
}

/// Encrypted credential file structure.
#[derive(Serialize, Deserialize)]
struct EncryptedCredentialFile {
    version: u32,
    salt: String,           // Base64-encoded salt for Argon2
    nonce: String,          // Base64-encoded nonce for AES-GCM
    ciphertext: String,     // Base64-encoded encrypted credentials JSON
    hmac: String,           // Base64-encoded HMAC of ciphertext
}

/// Internal credential structure for serialization (includes password).
#[derive(Serialize, Deserialize, Clone)]
struct SerializableCredential {
    host: String,
    username: String,
    password_or_token: String,
    expires_at: Option<SystemTime>,
    created_at: SystemTime,
    last_used_at: Option<SystemTime>,
}

impl From<&Credential> for SerializableCredential {
    fn from(cred: &Credential) -> Self {
        Self {
            host: cred.host.clone(),
            username: cred.username.clone(),
            password_or_token: cred.password_or_token.clone(),
            expires_at: cred.expires_at,
            created_at: cred.created_at,
            last_used_at: cred.last_used_at,
        }
    }
}

impl From<SerializableCredential> for Credential {
    fn from(sc: SerializableCredential) -> Self {
        let mut cred = Credential::new(sc.host, sc.username, sc.password_or_token);
        cred.expires_at = sc.expires_at;
        cred.created_at = sc.created_at;
        cred.last_used_at = sc.last_used_at;
        cred
    }
}

/// Plaintext credentials container (encrypted when stored).
#[derive(Serialize, Deserialize)]
struct CredentialsContainer {
    credentials: HashMap<String, SerializableCredential>,
}

/// Encrypted file store implementation.
///
/// 提供基于 AES-256-GCM 加密的文件存储，使用 Argon2id 从主密码派生密钥。
///
/// # 线程安全
///
/// 本结构体是线程安全的，可以安全地在多线程环境中使用：
/// - 使用 `Arc<Mutex<>>` 保护共享状态
/// - 实现了 `Send + Sync` trait
///
/// # 字段说明
///
/// - `file_path`: 加密文件的存储路径
/// - `key_cache`: 缓存派生的密钥，避免重复计算
/// - `key_cache_ttl`: 密钥缓存的生存时间
/// - `master_password`: 主密码（使用 Zeroize 保护）
/// - `file_lock`: 保护文件并发访问的互斥锁
pub struct EncryptedFileStore {
    file_path: PathBuf,
    key_cache: Arc<Mutex<Option<CachedKey>>>,
    key_cache_ttl: Duration,
    master_password: Arc<Mutex<Option<MasterPassword>>>,
    /// Mutex to protect concurrent file access
    file_lock: Arc<Mutex<()>>,
}

impl EncryptedFileStore {
    /// Creates a new encrypted file store.
    ///
    /// # 参数
    ///
    /// * `config` - Credential configuration containing file path and cache settings
    ///
    /// # 返回
    ///
    /// - `Ok(EncryptedFileStore)`: 成功创建存储
    /// - `Err(String)`: 创建失败，可能的原因：
    ///   - 配置中未指定文件路径
    ///   - 无法创建父目录（权限问题）
    ///   - 路径无效
    ///
    /// # 注意
    ///
    /// 创建后需要调用 `set_master_password()` 设置主密码，才能进行实际的存储操作。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use fireworks_collaboration_lib::core::credential::{
    ///     config::CredentialConfig,
    ///     file_store::EncryptedFileStore,
    /// };
    ///
    /// let config = CredentialConfig::new()
    ///     .with_file_path("/tmp/creds.enc".to_string());
    ///
    /// let store = EncryptedFileStore::new(&config)?;
    /// store.set_master_password("my-password".to_string())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(config: &CredentialConfig) -> Result<Self, String> {
        let file_path = config
            .file_path
            .as_ref()
            .ok_or_else(|| "File path required for encrypted file storage".to_string())?
            .clone();

        let file_path = PathBuf::from(file_path);

        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create credential file directory: {}", e)
            })?;
        }

        let key_cache_ttl = Duration::from_secs(config.key_cache_ttl_seconds as u64);

        Ok(EncryptedFileStore {
            file_path,
            key_cache: Arc::new(Mutex::new(None)),
            key_cache_ttl,
            master_password: Arc::new(Mutex::new(None)),
            file_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Sets the master password (required before any operations).
    ///
    /// # 参数
    ///
    /// * `password` - 主密码字符串
    ///
    /// # 返回
    ///
    /// 成功返回 `Ok(())`，总是成功（除非发生内部 panic）
    ///
    /// # 副作用
    ///
    /// - 清空密钥缓存，强制下次操作重新派生密钥
    /// - 旧密码会被 zeroize 清除
    ///
    /// # 安全性
    ///
    /// - 密码存储在 `MasterPassword` 结构中，该结构实现了 `ZeroizeOnDrop`
    /// - 当结构被销毁时，密码内存会自动清零
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # use fireworks_collaboration_lib::core::credential::{
    /// #     config::CredentialConfig,
    /// #     file_store::EncryptedFileStore,
    /// # };
    /// # let config = CredentialConfig::new()
    /// #     .with_file_path("/tmp/test.enc".to_string());
    /// # let store = EncryptedFileStore::new(&config)?;
    /// // 首次设置密码
    /// store.set_master_password("initial-password".to_string())?;
    ///
    /// // 更换密码（会清空缓存）
    /// store.set_master_password("new-password".to_string())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # 注意
    ///
    /// 更换密码后，旧密码加密的文件将无法读取。如需迁移数据，应先用旧密码读取所有凭证，
    /// 再用新密码重新写入。
    pub fn set_master_password(&self, password: String) -> Result<(), String> {
        let mut mp = self.master_password.lock().unwrap();
        *mp = Some(MasterPassword::new(password));
        
        // Clear key cache to force re-derivation
        let mut cache = self.key_cache.lock().unwrap();
        *cache = None;
        
        Ok(())
    }

    /// Derives encryption key from master password using Argon2id.
    fn derive_key(&self, salt: &SaltString) -> Result<EncryptionKey, String> {
        let mp_guard = self.master_password.lock().unwrap();
        let master_password = mp_guard
            .as_ref()
            .ok_or_else(|| "Master password not set".to_string())?;

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(32))
                .map_err(|e| format!("Invalid Argon2 params: {}", e))?,
        );

        let password_hash = argon2
            .hash_password(master_password.as_str().as_bytes(), salt)
            .map_err(|e| format!("Failed to derive key: {}", e))?;

        let hash_bytes = password_hash.hash.ok_or("No hash produced")?;
        let hash_slice = hash_bytes.as_bytes();

        if hash_slice.len() < 32 {
            return Err("Derived key too short".to_string());
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&hash_slice[..32]);

        Ok(EncryptionKey::new(key_bytes))
    }

    /// Gets or derives the encryption key (with caching).
    fn get_or_derive_key(&self, salt: &SaltString) -> Result<EncryptionKey, String> {
        let mut cache = self.key_cache.lock().unwrap();

        // Check if cached key is still valid
        if let Some(cached) = cache.as_ref() {
            if SystemTime::now() < cached.expires_at {
                // Clone the key bytes (creates new EncryptionKey)
                let key_bytes = cached.key.as_slice();
                let mut new_key_bytes = [0u8; 32];
                new_key_bytes.copy_from_slice(key_bytes);
                return Ok(EncryptionKey::new(new_key_bytes));
            }
        }

        // Derive new key
        let key = self.derive_key(salt)?;
        
        // Cache it
        let key_bytes = key.as_slice();
        let mut cached_key_bytes = [0u8; 32];
        cached_key_bytes.copy_from_slice(key_bytes);
        
        *cache = Some(CachedKey {
            key: EncryptionKey::new(cached_key_bytes),
            expires_at: SystemTime::now() + self.key_cache_ttl,
        });

        Ok(key)
    }

    /// Encrypts credentials container.
    fn encrypt(
        &self,
        container: &CredentialsContainer,
        salt: &SaltString,
    ) -> Result<EncryptedCredentialFile, String> {
        let key = self.get_or_derive_key(salt)?;
        
        // Serialize credentials to JSON
        let plaintext = serde_json::to_string(container)
            .map_err(|e| format!("Failed to serialize credentials: {}", e))?;

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(key.as_slice())
            .map_err(|e| format!("Failed to create cipher: {}", e))?;

        // Encrypt
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("Encryption failed: {}", e))?;

        // Compute HMAC
        let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key.as_slice())
            .map_err(|e| format!("Failed to create HMAC: {}", e))?;
        mac.update(&ciphertext);
        let hmac_result = mac.finalize();
        let hmac_bytes = hmac_result.into_bytes();

        Ok(EncryptedCredentialFile {
            version: FILE_VERSION,
            salt: general_purpose::STANDARD.encode(salt.as_str()),
            nonce: general_purpose::STANDARD.encode(nonce.as_slice()),
            ciphertext: general_purpose::STANDARD.encode(&ciphertext),
            hmac: general_purpose::STANDARD.encode(hmac_bytes),
        })
    }

    /// Decrypts credentials container.
    fn decrypt(&self, file: &EncryptedCredentialFile) -> Result<CredentialsContainer, String> {
        // Decode salt
        let salt_bytes = general_purpose::STANDARD
            .decode(&file.salt)
            .map_err(|e| format!("Failed to decode salt: {}", e))?;
        let salt_str = String::from_utf8(salt_bytes)
            .map_err(|e| format!("Invalid salt encoding: {}", e))?;
        let salt = SaltString::from_b64(&salt_str)
            .map_err(|e| format!("Invalid salt: {}", e))?;

        let key = self.get_or_derive_key(&salt)?;

        // Decode ciphertext and HMAC
        let ciphertext = general_purpose::STANDARD
            .decode(&file.ciphertext)
            .map_err(|e| format!("Failed to decode ciphertext: {}", e))?;
        let hmac_expected = general_purpose::STANDARD
            .decode(&file.hmac)
            .map_err(|e| format!("Failed to decode HMAC: {}", e))?;

        // Verify HMAC
        let mut mac = <HmacSha256 as KeyInit>::new_from_slice(key.as_slice())
            .map_err(|e| format!("Failed to create HMAC: {}", e))?;
        mac.update(&ciphertext);
        mac.verify_slice(&hmac_expected)
            .map_err(|_| "HMAC verification failed - file may be corrupted or tampered".to_string())?;

        // Decode nonce
        let nonce_bytes = general_purpose::STANDARD
            .decode(&file.nonce)
            .map_err(|e| format!("Failed to decode nonce: {}", e))?;
        if nonce_bytes.len() != NONCE_SIZE {
            return Err(format!(
                "Invalid nonce size: expected {}, got {}",
                NONCE_SIZE,
                nonce_bytes.len()
            ));
        }
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(key.as_slice())
            .map_err(|e| format!("Failed to create cipher: {}", e))?;

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| format!("Decryption failed: {}", e))?;

        // Deserialize
        let container: CredentialsContainer = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("Failed to deserialize credentials: {}", e))?;

        Ok(container)
    }

    /// Loads credentials from file.
    fn load_credentials(&self) -> Result<CredentialsContainer, String> {
        if !self.file_path.exists() {
            return Ok(CredentialsContainer {
                credentials: HashMap::new(),
            });
        }

        let file_content = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read credential file: {}", e))?;

        let encrypted_file: EncryptedCredentialFile = serde_json::from_str(&file_content)
            .map_err(|e| format!("Failed to parse credential file: {}", e))?;

        if encrypted_file.version != FILE_VERSION {
            return Err(format!(
                "Unsupported file version: {}, expected {}",
                encrypted_file.version, FILE_VERSION
            ));
        }

        self.decrypt(&encrypted_file)
    }

    /// Saves credentials to file.
    fn save_credentials(&self, container: &CredentialsContainer) -> Result<(), String> {
        // Generate new salt for each save
        let salt = SaltString::generate(&mut OsRng);
        
        let encrypted_file = self.encrypt(container, &salt)?;

        let file_content = serde_json::to_string_pretty(&encrypted_file)
            .map_err(|e| format!("Failed to serialize encrypted file: {}", e))?;

        // Write with restricted permissions (owner read/write only)
        fs::write(&self.file_path, file_content)
            .map_err(|e| format!("Failed to write credential file: {}", e))?;

        // Set file permissions (Unix-like systems)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.file_path)
                .map_err(|e| format!("Failed to get file metadata: {}", e))?
                .permissions();
            perms.set_mode(0o600); // rw-------
            fs::set_permissions(&self.file_path, perms)
                .map_err(|e| format!("Failed to set file permissions: {}", e))?;
        }

        Ok(())
    }

    /// Makes a credential key from host and username.
    fn make_key(host: &str, username: &str) -> String {
        format!("{}:{}", host, username)
    }
}

impl CredentialStore for EncryptedFileStore {
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>> {
        let _lock = self.file_lock.lock().unwrap();
        
        let username = username.ok_or_else(|| CredentialStoreError::Other("Username required for file store".to_string()))?;
        
        let container = self.load_credentials()
            .map_err(|e| CredentialStoreError::AccessError(e))?;
        let key = Self::make_key(host, username);
        
        let credential: Option<Credential> = container.credentials.get(&key).cloned().map(|sc| sc.into());
        
        // 过滤过期凭证
        match credential {
            Some(cred) if !cred.is_expired() => Ok(Some(cred)),
            _ => Ok(None),
        }
    }

    fn add(&self, credential: Credential) -> CredentialStoreResult<()> {
        let _lock = self.file_lock.lock().unwrap();
        
        let mut container = self.load_credentials()
            .map_err(|e| CredentialStoreError::AccessError(e))?;
        let key = Self::make_key(&credential.host, &credential.username);
        
        container.credentials.insert(key, SerializableCredential::from(&credential));
        
        self.save_credentials(&container)
            .map_err(|e| CredentialStoreError::AccessError(e))
    }

    fn remove(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        let _lock = self.file_lock.lock().unwrap();
        
        let mut container = self.load_credentials()
            .map_err(|e| CredentialStoreError::AccessError(e))?;
        let key = Self::make_key(host, username);
        
        container.credentials.remove(&key);
        
        self.save_credentials(&container)
            .map_err(|e| CredentialStoreError::AccessError(e))
    }

    fn list(&self) -> CredentialStoreResult<Vec<Credential>> {
        let _lock = self.file_lock.lock().unwrap();
        
        let container = self.load_credentials()
            .map_err(|e| CredentialStoreError::AccessError(e))?;
        
        // 过滤过期凭证
        let credentials: Vec<Credential> = container
            .credentials
            .values()
            .cloned()
            .map(|sc| sc.into())
            .filter(|cred: &Credential| !cred.is_expired())
            .collect();
        
        Ok(credentials)
    }

    fn update_last_used(&self, host: &str, username: &str) -> CredentialStoreResult<()> {
        let _lock = self.file_lock.lock().unwrap();
        
        let mut container = self.load_credentials()
            .map_err(|e| CredentialStoreError::AccessError(e))?;
        let key = Self::make_key(host, username);
        
        if let Some(cred) = container.credentials.get_mut(&key) {
            cred.last_used_at = Some(SystemTime::now());
            self.save_credentials(&container)
                .map_err(|e| CredentialStoreError::AccessError(e))?;
        }
        
        Ok(())
    }
}
