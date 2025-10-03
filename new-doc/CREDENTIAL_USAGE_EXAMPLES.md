# 凭证管理使用示例

本文档展示凭证存储与安全管理模块（P6.0）的常见使用场景和最佳实践。

## 目录

- [基本使用](#基本使用)
- [凭证生命周期管理](#凭证生命周期管理)
- [配置选项](#配置选项)
- [安全最佳实践](#安全最佳实践)
- [错误处理](#错误处理)

---

## 基本使用

### 1. 创建和存储凭证

```rust
use fireworks_collaboration_lib::core::credential::{
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};

// 创建存储
let store = MemoryCredentialStore::new();

// 创建凭证
let cred = Credential::new(
    "github.com".to_string(),
    "alice".to_string(),
    "ghp_1234567890abcdef".to_string(),
);

// 存储凭证
store.add(cred)?;
```

### 2. 查询凭证

```rust
// 根据主机和用户名查询
let cred = store.get("github.com", Some("alice"))?;

if let Some(c) = cred {
    println!("找到凭证: {}", c.identifier());
    // 使用 c.password_or_token 进行认证
} else {
    println!("凭证不存在或已过期");
}

// 仅根据主机查询（返回任意匹配的凭证）
let cred = store.get("github.com", None)?;
```

### 3. 列出所有凭证

```rust
let credentials = store.list()?;

for cred in credentials {
    println!("Host: {}, User: {}, Created: {:?}", 
        cred.host, 
        cred.username, 
        cred.created_at
    );
}
```

### 4. 删除凭证

```rust
// 删除指定凭证
store.remove("github.com", "alice")?;

// 验证删除成功
assert!(!store.exists("github.com", "alice"));
```

---

## 凭证生命周期管理

### 1. 创建带过期时间的凭证

```rust
use std::time::{SystemTime, Duration};

// 创建 30 天后过期的凭证
let expires_at = SystemTime::now() + Duration::from_secs(30 * 24 * 60 * 60);

let cred = Credential::new_with_expiry(
    "github.com".to_string(),
    "bob".to_string(),
    "token_123".to_string(),
    expires_at,
);

store.add(cred)?;
```

### 2. 检查凭证是否过期

```rust
let cred = store.get("github.com", Some("bob"))?.unwrap();

if cred.is_expired() {
    println!("凭证已过期，请更新");
} else {
    println!("凭证有效");
}
```

### 3. 更新最后使用时间

```rust
// 成功使用凭证后更新时间戳
store.update_last_used("github.com", "bob")?;

// 查询更新后的信息
let cred = store.get("github.com", Some("bob"))?.unwrap();
if let Some(last_used) = cred.last_used_at {
    println!("最后使用时间: {:?}", last_used);
}
```

### 4. 凭证更新（替换）

```rust
// 删除旧凭证
store.remove("github.com", "alice")?;

// 添加新凭证
let new_cred = Credential::new(
    "github.com".to_string(),
    "alice".to_string(),
    "new_token_456".to_string(),
);
store.add(new_cred)?;
```

---

## 配置选项

### 1. 基本配置（默认系统钥匙串）

```json
{
  "credential": {
    "storage": "system"
  }
}
```

### 2. 加密文件存储

```json
{
  "credential": {
    "storage": "file",
    "filePath": "/secure/path/credentials.enc"
  }
}
```

### 3. 高安全配置

```json
{
  "credential": {
    "storage": "system",
    "defaultTtlSeconds": 2592000,
    "auditMode": true,
    "requireConfirmation": true,
    "keyCacheTtlSeconds": 1800
  }
}
```

### 4. 临时会话存储（测试环境）

```json
{
  "credential": {
    "storage": "memory",
    "defaultTtlSeconds": null,
    "debugLogging": true
  }
}
```

### 5. 从代码加载配置

```rust
use fireworks_collaboration_lib::core::credential::config::{
    CredentialConfig, StorageType
};

let config = CredentialConfig::new()
    .with_storage(StorageType::Memory)
    .with_ttl(Some(3600))
    .with_debug_logging(false)
    .with_audit_mode(true);

// 验证配置
config.validate()?;
```

---

## 安全最佳实践

### 1. 日志脱敏

凭证在日志中自动脱敏：

```rust
let cred = Credential::new(
    "github.com".to_string(),
    "user".to_string(),
    "ghp_1234567890abcdef".to_string(),
);

// Display 输出自动脱敏
println!("{}", cred);
// 输出: Credential { host: github.com, username: user, password: ghp_****cdef, ... }

// Debug 输出也自动脱敏
println!("{:?}", cred);
// 输出类似，password_or_token 显示为 "ghp_****cdef"
```

### 2. 获取脱敏密码

```rust
let masked = cred.masked_password();
// 对于 "ghp_1234567890abcdef" 返回 "ghp_****cdef"
// 对于短密码 "abc" 返回 "***"

println!("Token: {}", masked); // 安全地显示在 UI 或日志中
```

### 3. 序列化安全

```rust
let json = serde_json::to_string(&cred)?;

// 验证密码未被序列化
assert!(!json.contains("ghp_1234567890abcdef"));
assert!(json.contains("github.com"));
assert!(json.contains("user"));
```

### 4. 配置验证

```rust
// 无效配置会被检测
let invalid_config = CredentialConfig::new()
    .with_storage(StorageType::File);
    // 缺少 file_path

assert!(invalid_config.validate().is_err());

// 修正配置
let valid_config = invalid_config
    .with_file_path("/tmp/creds.enc".to_string());

assert!(valid_config.validate().is_ok());
```

---

## 错误处理

### 1. 凭证未找到

```rust
use fireworks_collaboration_lib::core::credential::storage::CredentialStoreError;

match store.get("unknown.com", Some("user")) {
    Ok(Some(cred)) => {
        // 使用凭证
    }
    Ok(None) => {
        println!("凭证不存在或已过期");
        // 提示用户输入凭证
    }
    Err(e) => {
        eprintln!("查询失败: {}", e);
    }
}
```

### 2. 凭证已存在

```rust
let cred = Credential::new(
    "github.com".to_string(),
    "alice".to_string(),
    "token".to_string(),
);

match store.add(cred) {
    Ok(_) => println!("凭证已添加"),
    Err(CredentialStoreError::AlreadyExists(id)) => {
        println!("凭证已存在: {}", id);
        // 可选：询问用户是否替换
    }
    Err(e) => eprintln!("添加失败: {}", e),
}
```

### 3. 删除不存在的凭证

```rust
match store.remove("github.com", "nonexistent") {
    Ok(_) => println!("凭证已删除"),
    Err(CredentialStoreError::NotFound(id)) => {
        println!("凭证不存在: {}", id);
    }
    Err(e) => eprintln!("删除失败: {}", e),
}
```

### 4. 并发访问错误

```rust
use std::sync::Arc;

let store = Arc::new(MemoryCredentialStore::new());

// 即使多线程并发访问，内部使用读写锁保护
let store_clone = Arc::clone(&store);
std::thread::spawn(move || {
    store_clone.add(Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "token".to_string(),
    ))
});
```

---

## 完整示例

### Git Push 凭证自动填充示例

```rust
use fireworks_collaboration_lib::core::credential::{
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::time::{SystemTime, Duration};

fn git_push_with_credentials(
    store: &MemoryCredentialStore,
    host: &str,
    repo_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 尝试从存储获取凭证
    match store.get(host, None)? {
        Some(cred) if !cred.is_expired() => {
            // 使用已存储的凭证
            println!("使用已存储的凭证: {}", cred.identifier());
            
            // 执行 Git Push（伪代码）
            // git_push(repo_url, &cred.username, &cred.password_or_token)?;
            
            // 更新最后使用时间
            store.update_last_used(&cred.host, &cred.username)?;
            
            Ok(())
        }
        Some(_) => {
            // 凭证已过期
            println!("凭证已过期，请重新输入");
            prompt_and_save_credentials(store, host)
        }
        None => {
            // 无凭证，提示用户输入
            println!("未找到凭证，请输入");
            prompt_and_save_credentials(store, host)
        }
    }
}

fn prompt_and_save_credentials(
    store: &MemoryCredentialStore,
    host: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 提示用户输入（伪代码）
    let username = prompt_user("Username: ")?;
    let password = prompt_user("Password/Token: ")?;
    let save = prompt_yes_no("Save credentials? (y/n): ")?;
    
    if save {
        // 创建 90 天后过期的凭证
        let expires_at = SystemTime::now() + Duration::from_secs(90 * 24 * 60 * 60);
        let cred = Credential::new_with_expiry(
            host.to_string(),
            username,
            password,
            expires_at,
        );
        
        store.add(cred)?;
        println!("凭证已保存");
    }
    
    Ok(())
}

// 辅助函数（伪代码）
fn prompt_user(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    // 实际实现需要从 stdin 或 UI 获取输入
    Ok("user_input".to_string())
}

fn prompt_yes_no(prompt: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // 实际实现需要从 stdin 或 UI 获取输入
    Ok(true)
}
```

---

## 测试示例

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_credential_lifecycle() {
        let store = MemoryCredentialStore::new();
        
        // 添加凭证
        let cred = Credential::new(
            "github.com".to_string(),
            "test".to_string(),
            "token".to_string(),
        );
        assert!(store.add(cred).is_ok());
        
        // 查询凭证
        let retrieved = store.get("github.com", Some("test")).unwrap();
        assert!(retrieved.is_some());
        
        // 更新使用时间
        assert!(store.update_last_used("github.com", "test").is_ok());
        
        // 删除凭证
        assert!(store.remove("github.com", "test").is_ok());
        
        // 验证删除
        let after_remove = store.get("github.com", Some("test")).unwrap();
        assert!(after_remove.is_none());
    }
    
    #[test]
    fn test_credential_expiry() {
        let store = MemoryCredentialStore::new();
        
        // 创建已过期的凭证
        let past = SystemTime::now() - Duration::from_secs(3600);
        let cred = Credential::new_with_expiry(
            "github.com".to_string(),
            "test".to_string(),
            "token".to_string(),
            past,
        );
        
        store.add(cred).unwrap();
        
        // 过期凭证应该返回 None
        let retrieved = store.get("github.com", Some("test")).unwrap();
        assert!(retrieved.is_none());
    }
}
```

---

## Tauri 前端集成示例

### 后端 Tauri 命令

```rust
// src-tauri/src/app/commands/credential.rs

use crate::core::credential::{
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::sync::Arc;
use tauri::State;

// 全局存储（使用 State 管理）
pub struct CredentialState {
    store: Arc<MemoryCredentialStore>,
}

impl CredentialState {
    pub fn new() -> Self {
        Self {
            store: Arc::new(MemoryCredentialStore::new()),
        }
    }
}

#[tauri::command]
pub async fn add_credential(
    state: State<'_, CredentialState>,
    host: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let cred = Credential::new(host, username, password);
    state.store.add(cred)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_credential(
    state: State<'_, CredentialState>,
    host: String,
    username: Option<String>,
) -> Result<Option<CredentialInfo>, String> {
    state.store.get(&host, username.as_deref())
        .map(|opt| opt.map(|c| CredentialInfo::from(c)))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_credential(
    state: State<'_, CredentialState>,
    host: String,
    username: String,
) -> Result<(), String> {
    state.store.remove(&host, &username)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_credentials(
    state: State<'_, CredentialState>,
) -> Result<Vec<CredentialInfo>, String> {
    state.store.list()
        .map(|creds| creds.into_iter().map(CredentialInfo::from).collect())
        .map_err(|e| e.to_string())
}

// 脱敏的凭证信息（用于前端显示）
#[derive(serde::Serialize)]
pub struct CredentialInfo {
    pub host: String,
    pub username: String,
    pub masked_password: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub is_expired: bool,
}

impl From<Credential> for CredentialInfo {
    fn from(c: Credential) -> Self {
        Self {
            host: c.host,
            username: c.username,
            masked_password: c.masked_password(),
            created_at: format!("{:?}", c.created_at),
            expires_at: c.expires_at.map(|t| format!("{:?}", t)),
            last_used_at: c.last_used_at.map(|t| format!("{:?}", t)),
            is_expired: c.is_expired(),
        }
    }
}

// 在 main.rs 中注册
fn main() {
    tauri::Builder::default()
        .manage(CredentialState::new())
        .invoke_handler(tauri::generate_handler![
            add_credential,
            get_credential,
            remove_credential,
            list_credentials,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 前端 TypeScript 封装

```typescript
// src/api/credential.ts

import { invoke } from '@tauri-apps/api/core';

export interface CredentialInfo {
  host: string;
  username: string;
  maskedPassword: string;
  createdAt: string;
  expiresAt?: string;
  lastUsedAt?: string;
  isExpired: boolean;
}

export class CredentialAPI {
  /**
   * 添加凭证
   */
  static async add(
    host: string,
    username: string,
    password: string
  ): Promise<void> {
    return invoke('add_credential', { host, username, password });
  }

  /**
   * 获取凭证
   */
  static async get(
    host: string,
    username?: string
  ): Promise<CredentialInfo | null> {
    return invoke('get_credential', { host, username });
  }

  /**
   * 删除凭证
   */
  static async remove(host: string, username: string): Promise<void> {
    return invoke('remove_credential', { host, username });
  }

  /**
   * 列出所有凭证
   */
  static async list(): Promise<CredentialInfo[]> {
    return invoke('list_credentials');
  }
}
```

### Vue 组件示例

```vue
<!-- src/components/CredentialManager.vue -->

<template>
  <div class="credential-manager">
    <h2>凭证管理</h2>

    <!-- 添加凭证表单 -->
    <div class="add-form">
      <input v-model="newCred.host" placeholder="主机 (如 github.com)" />
      <input v-model="newCred.username" placeholder="用户名" />
      <input
        v-model="newCred.password"
        type="password"
        placeholder="密码/Token"
      />
      <button @click="addCredential">添加凭证</button>
    </div>

    <!-- 凭证列表 -->
    <div class="credential-list">
      <div
        v-for="cred in credentials"
        :key="`${cred.host}-${cred.username}`"
        class="credential-item"
        :class="{ expired: cred.isExpired }"
      >
        <div class="info">
          <strong>{{ cred.host }}</strong>
          <span>{{ cred.username }}</span>
          <span class="password">{{ cred.maskedPassword }}</span>
          <span v-if="cred.isExpired" class="badge expired">已过期</span>
        </div>
        <button @click="removeCredential(cred.host, cred.username)">
          删除
        </button>
      </div>
    </div>

    <!-- 错误提示 -->
    <div v-if="error" class="error">{{ error }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { CredentialAPI, type CredentialInfo } from '../api/credential';

const credentials = ref<CredentialInfo[]>([]);
const error = ref<string>('');
const newCred = ref({
  host: '',
  username: '',
  password: '',
});

async function loadCredentials() {
  try {
    credentials.value = await CredentialAPI.list();
  } catch (e) {
    error.value = `加载凭证失败: ${e}`;
  }
}

async function addCredential() {
  try {
    await CredentialAPI.add(
      newCred.value.host,
      newCred.value.username,
      newCred.value.password
    );

    // 清空表单
    newCred.value = { host: '', username: '', password: '' };

    // 重新加载
    await loadCredentials();
    error.value = '';
  } catch (e) {
    error.value = `添加凭证失败: ${e}`;
  }
}

async function removeCredential(host: string, username: string) {
  if (!confirm(`确定删除 ${username}@${host} 的凭证吗？`)) {
    return;
  }

  try {
    await CredentialAPI.remove(host, username);
    await loadCredentials();
    error.value = '';
  } catch (e) {
    error.value = `删除凭证失败: ${e}`;
  }
}

onMounted(() => {
  loadCredentials();
});
</script>

<style scoped>
.credential-manager {
  padding: 20px;
}

.add-form {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.add-form input {
  flex: 1;
  padding: 8px;
}

.credential-item {
  display: flex;
  justify-content: space-between;
  padding: 10px;
  border: 1px solid #ddd;
  margin-bottom: 5px;
}

.credential-item.expired {
  background-color: #fee;
}

.password {
  font-family: monospace;
  color: #666;
}

.badge.expired {
  color: red;
  font-weight: bold;
}

.error {
  color: red;
  padding: 10px;
  background-color: #fee;
  margin-top: 10px;
}
</style>
```

---

## 参考文档

- [CREDENTIAL_SECURITY_ASSESSMENT.md](../new-doc/CREDENTIAL_SECURITY_ASSESSMENT.md) - 安全威胁评估
- [CREDENTIAL_ENCRYPTION_DESIGN.md](../new-doc/CREDENTIAL_ENCRYPTION_DESIGN.md) - 加密方案设计
- [CREDENTIAL_QUICKSTART.md](../new-doc/CREDENTIAL_QUICKSTART.md) - 快速入门
- [CREDENTIAL_ERROR_HANDLING.md](../new-doc/CREDENTIAL_ERROR_HANDLING.md) - 错误处理
- [CREDENTIAL_TROUBLESHOOTING.md](../new-doc/CREDENTIAL_TROUBLESHOOTING.md) - 故障排查
- [config.example.json](../config.example.json) - 配置示例

---

## 下一步

P6.0 阶段仅建立了基线架构。后续阶段将实现：

- **P6.1**: 系统钥匙串集成（macOS Keychain、Windows Credential Manager、Linux Secret Service）
- **P6.2**: 加密文件存储（AES-256-GCM + Argon2id）
- **P6.3**: 前端 UI 集成（凭证管理界面）
- **P6.4**: 生命周期管理（自动清理、批量操作）
- **P6.5**: 安全审计与准入
