# 凭证管理错误处理指南

本文档详细说明凭证存储模块中每种错误类型的原因、解决方案和预防措施。

## 目录

- [错误类型总览](#错误类型总览)
- [详细错误指南](#详细错误指南)
- [错误处理最佳实践](#错误处理最佳实践)
- [错误恢复策略](#错误恢复策略)

---

## 错误类型总览

```rust
pub enum CredentialStoreError {
    NotFound(String),           // 凭证未找到
    AlreadyExists(String),      // 凭证已存在
    Expired(String),            // 凭证已过期
    AccessError(String),        // 存储访问错误
    CryptoError(String),        // 加密/解密错误
    SerializationError(String), // 序列化错误
    IoError(std::io::Error),    // IO 错误
    Other(String),              // 其他错误
}
```

---

## 详细错误指南

### 1. NotFound - 凭证未找到

#### 错误消息

```
凭证未找到: alice@github.com
```

#### 原因

- 凭证从未被添加
- 凭证已被删除
- 凭证已过期（被自动过滤）
- host 或 username 拼写错误

#### 解决方案

```rust
match store.get("github.com", Some("alice")) {
    Ok(None) | Err(CredentialStoreError::NotFound(_)) => {
        // 方案 1: 提示用户输入新凭证
        let cred = prompt_user_for_credential()?;
        store.add(cred)?;
        
        // 方案 2: 从其他来源导入
        let cred = import_from_env_or_config()?;
        store.add(cred)?;
        
        // 方案 3: 使用默认凭证
        let cred = get_default_credential()?;
        store.add(cred)?;
    }
    Ok(Some(cred)) => {
        // 正常使用
    }
    Err(e) => {
        eprintln!("其他错误: {}", e);
    }
}
```

#### 预防措施

```rust
// 在删除前检查是否存在
if store.exists("github.com", "alice") {
    store.remove("github.com", "alice")?;
} else {
    println!("凭证不存在，无需删除");
}

// 定期刷新凭证，避免过期
if let Some(cred) = store.get(host, Some(username))? {
    if cred.is_expired() {
        println!("凭证即将过期，请更新");
    }
}
```

---

### 2. AlreadyExists - 凭证已存在

#### 错误消息

```
凭证已存在: alice@github.com
```

#### 原因

- 尝试添加重复的凭证（相同 host + username）
- 未先删除旧凭证就尝试更新

#### 解决方案

```rust
// 方案 1: 更新凭证（先删除后添加）
fn update_credential(
    store: &impl CredentialStore,
    new_cred: Credential,
) -> Result<(), CredentialStoreError> {
    // 先删除旧凭证（如果存在）
    let _ = store.remove(&new_cred.host, &new_cred.username);
    
    // 添加新凭证
    store.add(new_cred)?;
    Ok(())
}

// 方案 2: 检查后再添加
fn add_or_skip(
    store: &impl CredentialStore,
    cred: Credential,
) -> Result<bool, CredentialStoreError> {
    if store.exists(&cred.host, &cred.username) {
        println!("凭证已存在，跳过");
        Ok(false)
    } else {
        store.add(cred)?;
        Ok(true)
    }
}

// 方案 3: 询问用户
match store.add(cred.clone()) {
    Ok(_) => println!("凭证已添加"),
    Err(CredentialStoreError::AlreadyExists(_)) => {
        if confirm_user("凭证已存在，是否替换？")? {
            update_credential(store, cred)?;
        }
    }
    Err(e) => return Err(e),
}
```

#### 预防措施

```rust
// 使用 exists 预检查
if !store.exists(host, username) {
    store.add(cred)?;
} else {
    // 处理已存在的情况
}

// 或使用辅助函数
fn add_or_update(
    store: &impl CredentialStore,
    cred: Credential,
) -> Result<(), CredentialStoreError> {
    match store.add(cred.clone()) {
        Ok(_) => Ok(()),
        Err(CredentialStoreError::AlreadyExists(_)) => {
            store.remove(&cred.host, &cred.username)?;
            store.add(cred)
        }
        Err(e) => Err(e),
    }
}
```

---

### 3. Expired - 凭证已过期

#### 错误消息

```
凭证已过期: alice@github.com
```

#### 原因

- 凭证的 `expires_at` 时间已过
- 创建时设置了过短的过期时间

#### 解决方案

```rust
// 自动刷新过期凭证
fn get_or_refresh(
    store: &impl CredentialStore,
    host: &str,
    username: &str,
) -> Result<Credential, Box<dyn std::error::Error>> {
    match store.get(host, Some(username))? {
        Some(cred) if !cred.is_expired() => Ok(cred),
        _ => {
            // 凭证不存在或已过期，提示用户输入新凭证
            let new_cred = prompt_for_new_credential(host, username)?;
            
            // 删除旧凭证（如果存在）
            let _ = store.remove(host, username);
            
            // 添加新凭证
            store.add(new_cred.clone())?;
            Ok(new_cred)
        }
    }
}

// 批量清理过期凭证
fn cleanup_expired(store: &impl CredentialStore) -> Result<usize, CredentialStoreError> {
    let all_creds = store.list()?;
    let mut removed = 0;
    
    for cred in all_creds {
        if cred.is_expired() {
            store.remove(&cred.host, &cred.username)?;
            removed += 1;
        }
    }
    
    Ok(removed)
}
```

#### 预防措施

```rust
use std::time::{SystemTime, Duration};

// 设置合理的过期时间（90天）
let expires_at = SystemTime::now() + Duration::from_secs(90 * 24 * 60 * 60);
let cred = Credential::new_with_expiry(
    "github.com".to_string(),
    "alice".to_string(),
    "token".to_string(),
    expires_at,
);

// 或使用永不过期（None）
let cred = Credential::new(
    "github.com".to_string(),
    "alice".to_string(),
    "token".to_string(),
);
// cred.expires_at == None

// 定期检查即将过期的凭证
fn check_expiring_soon(
    store: &impl CredentialStore,
    days: u64,
) -> Result<Vec<Credential>, CredentialStoreError> {
    let threshold = SystemTime::now() + Duration::from_secs(days * 24 * 60 * 60);
    
    Ok(store.list()?
        .into_iter()
        .filter(|c| {
            c.expires_at
                .map(|exp| exp < threshold)
                .unwrap_or(false)
        })
        .collect())
}
```

---

### 4. AccessError - 存储访问错误

#### 错误消息

```
存储访问错误: 读锁获取失败: ...
存储访问错误: 写锁获取失败: ...
```

#### 原因

- 并发访问冲突（读写锁中毒）
- 其他线程 panic 导致锁中毒
- 死锁（极少见）

#### 解决方案

```rust
use std::sync::Arc;

// 方案 1: 使用 Arc 包装存储，避免锁中毒
let store = Arc::new(MemoryCredentialStore::new());

// 方案 2: 捕获并恢复
match store.get(host, Some(username)) {
    Err(CredentialStoreError::AccessError(msg)) => {
        eprintln!("访问错误: {}", msg);
        
        // 选项 A: 重试
        std::thread::sleep(Duration::from_millis(100));
        store.get(host, Some(username))?
        
        // 选项 B: 创建新存储实例
        // let new_store = MemoryCredentialStore::new();
    }
    result => result,
}

// 方案 3: 使用 PoisonError 恢复（底层）
use std::sync::{RwLock, PoisonError};

// 在存储实现中
let credentials = match self.credentials.read() {
    Ok(guard) => guard,
    Err(PoisonError { .. }) => {
        // 锁中毒，但仍可恢复数据
        self.credentials.read().unwrap()
    }
};
```

#### 预防措施

```rust
// 1. 避免在锁内 panic
impl CredentialStore for MemoryCredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> CredentialStoreResult<Option<Credential>> {
        let credentials = self.credentials.read()
            .map_err(|e| CredentialStoreError::AccessError(format!("读锁获取失败: {}", e)))?;
        
        // 所有操作都不应该 panic
        // 使用 ? 而不是 unwrap()
        Ok(/* ... */)
    }
}

// 2. 缩小锁的范围
fn add(&self, credential: Credential) -> CredentialStoreResult<()> {
    let key = Self::make_key(&credential.host, &credential.username);
    
    // 预先检查（不持有锁）
    {
        let credentials = self.credentials.read()?;
        if credentials.contains_key(&key) {
            return Err(CredentialStoreError::AlreadyExists(key));
        }
    }
    
    // 快速写入
    {
        let mut credentials = self.credentials.write()?;
        credentials.insert(key, credential);
    }
    
    Ok(())
}
```

---

### 5. CryptoError - 加密/解密错误

#### 错误消息

```
加密/解密错误: 密钥派生失败
加密/解密错误: 解密失败，数据可能损坏
```

#### 原因（P6.2+ 阶段）

- 密码错误
- 数据损坏
- 加密算法不兼容
- 盐值丢失

#### 解决方案

```rust
// P6.0 暂不涉及，P6.2 实现后的示例：

match store.get(host, Some(username)) {
    Err(CredentialStoreError::CryptoError(msg)) => {
        eprintln!("解密错误: {}", msg);
        
        // 方案 1: 要求用户重新输入主密码
        let master_password = prompt_master_password()?;
        store.unlock(master_password)?;
        
        // 方案 2: 清空损坏的数据，重新开始
        if confirm_user("数据损坏，是否清空并重新初始化？")? {
            store.reset()?;
        }
        
        // 方案 3: 尝试从备份恢复
        store.restore_from_backup("backup.enc")?;
    }
    result => result,
}
```

#### 预防措施

```rust
// P6.2+ 阶段的预防措施：

// 1. 定期备份
fn backup_credentials(store: &EncryptedFileStore) -> Result<(), Box<dyn std::error::Error>> {
    let backup_path = format!("credentials.backup.{}.enc", timestamp());
    store.export_encrypted(&backup_path)?;
    Ok(())
}

// 2. 校验完整性
fn verify_integrity(store: &EncryptedFileStore) -> Result<bool, CredentialStoreError> {
    store.verify_checksum()
}

// 3. 使用强密码
fn validate_master_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 12 {
        return Err("主密码长度至少 12 字符");
    }
    // 更多校验...
    Ok(())
}
```

---

### 6. SerializationError - 序列化错误

#### 错误消息

```
序列化错误: JSON 格式错误
```

#### 原因

- 数据格式不兼容
- 文件损坏
- 版本不匹配

#### 解决方案

```rust
use serde_json;

match serde_json::from_str::<Credential>(&json_str) {
    Ok(cred) => {
        // 成功解析
    }
    Err(e) => {
        eprintln!("序列化错误: {}", e);
        
        // 方案 1: 尝试兼容旧格式
        if let Ok(old_cred) = serde_json::from_str::<OldCredential>(&json_str) {
            let cred = migrate_from_old(old_cred);
            // 使用迁移后的凭证
        }
        
        // 方案 2: 跳过损坏的条目
        eprintln!("跳过损坏的凭证: {}", json_str);
    }
}
```

#### 预防措施

```rust
// 1. 版本化数据结构
#[derive(Serialize, Deserialize)]
struct CredentialV1 {
    version: u32,  // 始终为 1
    host: String,
    username: String,
    // ...
}

// 2. 使用向后兼容的序列化
#[derive(Serialize, Deserialize)]
struct Credential {
    host: String,
    username: String,
    
    #[serde(default)]  // 新字段使用默认值
    expires_at: Option<SystemTime>,
    
    #[serde(skip_serializing_if = "Option::is_none")]  // 可选字段
    last_used_at: Option<SystemTime>,
}

// 3. 验证后再保存
fn save_credential(cred: &Credential) -> Result<(), Box<dyn std::error::Error>> {
    // 序列化
    let json = serde_json::to_string(cred)?;
    
    // 验证可以反序列化
    let _: Credential = serde_json::from_str(&json)?;
    
    // 保存
    std::fs::write("credential.json", json)?;
    Ok(())
}
```

---

### 7. IoError - IO 错误

#### 错误消息

```
Permission denied (os error 13)
No such file or directory (os error 2)
Disk full (os error 28)
```

#### 原因

- 文件权限不足
- 文件或目录不存在
- 磁盘空间不足
- 文件被占用

#### 解决方案

```rust
use std::io::ErrorKind;

match store.load_from_file("/path/to/credentials.enc") {
    Err(CredentialStoreError::IoError(e)) => {
        match e.kind() {
            ErrorKind::NotFound => {
                // 文件不存在，创建新文件
                store.create_new_file("/path/to/credentials.enc")?;
            }
            ErrorKind::PermissionDenied => {
                // 权限不足，尝试使用临时目录
                let temp_path = std::env::temp_dir().join("credentials.enc");
                store.load_from_file(&temp_path)?;
            }
            _ => {
                return Err(e.into());
            }
        }
    }
    result => result?,
}
```

#### 预防措施

```rust
use std::fs;
use std::path::Path;

// 1. 提前检查权限
fn check_file_permissions(path: &Path) -> Result<(), String> {
    if !path.exists() {
        let parent = path.parent().ok_or("无法获取父目录")?;
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {}", e))?;
        }
    }
    
    // 尝试创建测试文件
    let test_file = path.with_extension("test");
    fs::write(&test_file, b"test")
        .map_err(|e| format!("权限检查失败: {}", e))?;
    fs::remove_file(&test_file).ok();
    
    Ok(())
}

// 2. 检查磁盘空间
fn check_disk_space(path: &Path, required_bytes: u64) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path)
            .map_err(|e| format!("获取文件信息失败: {}", e))?;
        // 检查可用空间...
    }
    Ok(())
}

// 3. 使用回退路径
fn get_credential_file_path() -> PathBuf {
    let paths = vec![
        PathBuf::from("/secure/path/credentials.enc"),
        dirs::config_dir().unwrap().join("app/credentials.enc"),
        std::env::temp_dir().join("credentials.enc"),
    ];
    
    for path in paths {
        if let Ok(_) = check_file_permissions(&path) {
            return path;
        }
    }
    
    // 最后的备选
    std::env::temp_dir().join("credentials_fallback.enc")
}
```

---

## 错误处理最佳实践

### 1. 分层错误处理

```rust
// 底层：详细错误
fn low_level_operation() -> Result<(), CredentialStoreError> {
    // 返回具体错误
}

// 中层：转换错误
fn mid_level_operation() -> Result<(), AppError> {
    low_level_operation()
        .map_err(|e| AppError::CredentialError(e))?;
    Ok(())
}

// 顶层：用户友好消息
fn user_facing_operation() -> Result<(), String> {
    mid_level_operation()
        .map_err(|e| format!("凭证操作失败: {}", e))?;
    Ok(())
}
```

### 2. 使用 Result 类型

```rust
// ✅ 推荐：返回 Result
fn get_credential(host: &str) -> Result<Credential, CredentialStoreError> {
    // ...
}

// ❌ 不推荐：unwrap() 或 expect()
fn get_credential(host: &str) -> Credential {
    store.get(host, None).unwrap().unwrap()  // 危险！
}
```

### 3. 日志记录

```rust
use log::{error, warn, info};

match store.get(host, Some(username)) {
    Ok(Some(cred)) => {
        info!("成功获取凭证: {}", cred.identifier());
        Ok(cred)
    }
    Ok(None) => {
        warn!("凭证不存在: {}@{}", username, host);
        Err("凭证不存在".into())
    }
    Err(e) => {
        error!("凭证查询失败: {}", e);
        Err(e.into())
    }
}
```

### 4. 优雅降级

```rust
fn get_credential_with_fallback(
    store: &impl CredentialStore,
    host: &str,
    username: &str,
) -> Result<Credential, Box<dyn std::error::Error>> {
    // 尝试 1: 从存储获取
    if let Ok(Some(cred)) = store.get(host, Some(username)) {
        return Ok(cred);
    }
    
    // 尝试 2: 从环境变量获取
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        let cred = Credential::new(host.to_string(), username.to_string(), token);
        let _ = store.add(cred.clone());  // 保存供下次使用
        return Ok(cred);
    }
    
    // 尝试 3: 提示用户输入
    prompt_user_for_credential(host, username)
}
```

---

## 错误恢复策略

### 自动恢复流程图

```
错误发生
  ↓
记录错误日志
  ↓
判断错误类型
  ↓
┌─────────────┬──────────────┬───────────────┐
│ 可恢复错误   │ 需用户干预    │ 致命错误       │
│ (NotFound)  │ (Expired)    │ (AccessError) │
└─────────────┴──────────────┴───────────────┘
  ↓             ↓              ↓
自动重试       提示用户        优雅退出
  ↓             ↓              ↓
成功/失败      用户输入        清理资源
  ↓             ↓              ↓
继续执行       重新尝试        报告错误
```

### 示例代码

```rust
fn robust_credential_operation<F, T>(
    mut operation: F,
    max_retries: u32,
) -> Result<T, CredentialStoreError>
where
    F: FnMut() -> Result<T, CredentialStoreError>,
{
    let mut attempts = 0;
    
    loop {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                
                // 判断是否可重试
                let should_retry = matches!(e,
                    CredentialStoreError::AccessError(_) |
                    CredentialStoreError::IoError(_)
                ) && attempts < max_retries;
                
                if should_retry {
                    warn!("操作失败，重试 {}/{}: {}", attempts, max_retries, e);
                    std::thread::sleep(Duration::from_millis(100 * attempts as u64));
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

// 使用示例
let result = robust_credential_operation(
    || store.get("github.com", Some("alice")),
    3,  // 最多重试 3 次
)?;
```

---

## 参考

- [CREDENTIAL_QUICKSTART.md](CREDENTIAL_QUICKSTART.md) - 快速入门
- [CREDENTIAL_TROUBLESHOOTING.md](CREDENTIAL_TROUBLESHOOTING.md) - 故障排查
- [CREDENTIAL_USAGE_EXAMPLES.md](CREDENTIAL_USAGE_EXAMPLES.md) - 使用示例
