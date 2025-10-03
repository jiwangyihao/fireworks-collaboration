# 凭证管理快速入门（5分钟）

P6.0 阶段提供了凭证存储的基础架构。本指南帮助您快速上手。

## 🚀 快速开始

### 1. 创建并保存凭证（30秒）

```rust
use fireworks_collaboration_lib::core::credential::{
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};

// 创建内存存储
let store = MemoryCredentialStore::new();

// 创建凭证
let cred = Credential::new(
    "github.com".to_string(),
    "your_username".to_string(),
    "ghp_your_token_here".to_string(),
);

// 保存
store.add(cred)?;
```

### 2. 查询凭证（10秒）

```rust
// 查询凭证
let cred = store.get("github.com", Some("your_username"))?;

if let Some(c) = cred {
    println!("找到凭证: {}", c.identifier());
    // 使用 c.password_or_token 进行认证
}
```

### 3. 更新使用时间（10秒）

```rust
// 成功使用凭证后更新时间戳
store.update_last_used("github.com", "your_username")?;
```

### 4. 删除凭证（10秒）

```rust
// 删除凭证
store.remove("github.com", "your_username")?;
```

---

## ⚙️ 最小化配置

### 方案 1：使用默认配置（推荐）

无需任何配置，直接使用：

```rust
use fireworks_collaboration_lib::core::credential::config::CredentialConfig;

let config = CredentialConfig::default();
// storage: System (系统钥匙串)
// defaultTtlSeconds: 7776000 (90天)
```

### 方案 2：内存存储（测试环境）

```json
{
  "credential": {
    "storage": "memory"
  }
}
```

### 方案 3：临时会话（无过期时间）

```json
{
  "credential": {
    "storage": "memory",
    "defaultTtlSeconds": null
  }
}
```

---

## 🔒 安全特性（自动启用）

### 1. 日志自动脱敏

```rust
let cred = Credential::new(
    "github.com".to_string(),
    "user".to_string(),
    "ghp_1234567890abcdef".to_string(),
);

// 密码自动脱敏
println!("{}", cred);
// 输出: Credential { ..., password: ghp_****cdef, ... }
```

### 2. 序列化安全

```rust
let json = serde_json::to_string(&cred)?;
// JSON 中不包含密码字段
```

### 3. 过期检测

```rust
if cred.is_expired() {
    println!("凭证已过期");
    // 自动处理
}
```

---

## 📋 常见操作速查

### 创建带过期时间的凭证

```rust
use std::time::{SystemTime, Duration};

let expires_at = SystemTime::now() + Duration::from_secs(30 * 24 * 60 * 60); // 30天

let cred = Credential::new_with_expiry(
    "github.com".to_string(),
    "user".to_string(),
    "token".to_string(),
    expires_at,
);
```

### 列出所有凭证

```rust
let all_creds = store.list()?;
for cred in all_creds {
    println!("{}", cred.identifier());
}
```

### 检查凭证是否存在

```rust
if store.exists("github.com", "user") {
    println!("凭证存在");
}
```

### 获取脱敏密码（用于显示）

```rust
let masked = cred.masked_password();
// "ghp_1234567890abcdef" → "ghp_****cdef"
```

---

## ⚠️ 错误处理快速模式

```rust
use fireworks_collaboration_lib::core::credential::storage::CredentialStoreError;

match store.get("github.com", Some("user")) {
    Ok(Some(cred)) => {
        // 使用凭证
    }
    Ok(None) => {
        println!("凭证不存在或已过期");
    }
    Err(CredentialStoreError::NotFound(id)) => {
        println!("未找到: {}", id);
    }
    Err(CredentialStoreError::AlreadyExists(id)) => {
        println!("凭证已存在: {}", id);
    }
    Err(e) => {
        eprintln!("错误: {}", e);
    }
}
```

---

## 🔄 完整工作流程示例

```rust
use fireworks_collaboration_lib::core::credential::{
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建存储
    let store = MemoryCredentialStore::new();
    
    // 2. 添加凭证
    let cred = Credential::new(
        "github.com".to_string(),
        "alice".to_string(),
        "ghp_token_123".to_string(),
    );
    store.add(cred)?;
    
    // 3. 使用凭证
    let cred = store.get("github.com", Some("alice"))?.unwrap();
    println!("使用凭证: {}", cred.masked_password());
    
    // 4. 更新使用时间
    store.update_last_used("github.com", "alice")?;
    
    // 5. 完成后不需要删除（自动过期）
    Ok(())
}
```

---

## 📚 下一步

- **详细示例**: 查看 [CREDENTIAL_USAGE_EXAMPLES.md](CREDENTIAL_USAGE_EXAMPLES.md)
- **错误处理**: 查看 [CREDENTIAL_ERROR_HANDLING.md](CREDENTIAL_ERROR_HANDLING.md)
- **故障排查**: 查看 [CREDENTIAL_TROUBLESHOOTING.md](CREDENTIAL_TROUBLESHOOTING.md)
- **安全评估**: 查看 [CREDENTIAL_SECURITY_ASSESSMENT.md](CREDENTIAL_SECURITY_ASSESSMENT.md)

---

## 🎯 关键要点

1. ✅ **默认安全**: 所有安全特性自动启用
2. ✅ **零配置**: 使用默认配置即可开始
3. ✅ **自动脱敏**: 日志和序列化自动保护密码
4. ✅ **过期管理**: 自动过滤过期凭证
5. ✅ **并发安全**: 内置读写锁保护

---

## ⏱️ 5分钟检查清单

- [ ] 创建 `MemoryCredentialStore`
- [ ] 使用 `Credential::new()` 创建凭证
- [ ] 调用 `store.add()` 保存凭证
- [ ] 调用 `store.get()` 查询凭证
- [ ] 使用 `masked_password()` 安全显示
- [ ] 测试错误处理（`AlreadyExists`, `NotFound`）
- [ ] 验证日志输出已脱敏

完成这 7 步，您就掌握了 P6.0 的核心功能！
