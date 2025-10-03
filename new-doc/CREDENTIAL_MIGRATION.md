# 凭证管理迁移指南

本文档为凭证存储模块的版本迁移和外部系统集成提供指导。

## 目录

- [版本迁移](#版本迁移)
- [从其他系统迁移](#从其他系统迁移)
- [导入导出](#导入导出)
- [向后兼容性](#向后兼容性)

---

## 版本迁移

### P6.0 → P6.1 迁移

P6.1 将引入系统钥匙串存储（System Keychain）。

#### 配置变更

```json
// P6.0 配置（仅内存存储）
{
  "credential": {
    "storage": "memory"
  }
}

// P6.1 配置（新增系统钥匙串）
{
  "credential": {
    "storage": "system"  // 新增选项
  }
}
```

#### 迁移步骤

1. **备份现有凭证**

```rust
// 在升级前导出所有凭证
let credentials = store.list()?;
let json = serde_json::to_string_pretty(&credentials)?;
std::fs::write("credentials_backup.json", json)?;
```

2. **升级配置文件**

```json
{
  "credential": {
    "storage": "system",  // 改为 system
    "defaultTtlSeconds": 7776000
  }
}
```

3. **导入凭证到新存储**

```rust
// P6.1 提供的迁移工具（示例）
use fireworks_collaboration_lib::core::credential::migration;

migration::import_from_memory_to_system(
    "credentials_backup.json",
    &system_store,
)?;
```

#### 兼容性保证

- ✅ P6.0 配置文件在 P6.1 中仍然有效
- ✅ `storage: "memory"` 在所有版本都支持
- ✅ 新增的 `storage: "system"` 字段向后兼容

---

### P6.1 → P6.2 迁移

P6.2 将引入加密文件存储（Encrypted File）。

#### 配置变更

```json
// P6.2 新增配置
{
  "credential": {
    "storage": "file",  // 新增选项
    "filePath": "/secure/path/credentials.enc",
    "encryptionKey": null  // 将在运行时提示用户输入
  }
}
```

#### 迁移步骤

1. **从系统钥匙串导出**

```rust
// P6.2 提供的工具
use fireworks_collaboration_lib::core::credential::migration;

// 导出到加密文件
migration::export_from_system_to_file(
    &system_store,
    "/path/to/credentials.enc",
    master_password,
)?;
```

2. **验证迁移**

```rust
// 读取加密文件验证
let file_store = EncryptedFileStore::new("/path/to/credentials.enc")?;
file_store.unlock(master_password)?;

let count = file_store.list()?.len();
println!("迁移后凭证数量: {}", count);
```

---

## 从其他系统迁移

### 从 Git Credential Manager 迁移

#### 1. 导出凭证

```bash
# Windows
git credential-manager list > gcm_creds.txt

# 或从注册表导出（Windows）
reg export "HKCU\Software\GitCredentialManager" gcm_export.reg
```

#### 2. 转换格式

```rust
// 转换工具（示例）
fn import_from_gcm(gcm_file: &Path) -> Result<Vec<Credential>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(gcm_file)?;
    let mut credentials = Vec::new();
    
    for line in content.lines() {
        // 解析 GCM 格式: "protocol=https\nhost=github.com\nusername=alice\npassword=token123"
        if line.starts_with("host=") {
            let host = line.strip_prefix("host=").unwrap();
            // ... 继续解析
            let cred = Credential::new(
                host.to_string(),
                username,
                password,
            );
            credentials.push(cred);
        }
    }
    
    Ok(credentials)
}
```

#### 3. 导入到本系统

```rust
let gcm_creds = import_from_gcm(Path::new("gcm_creds.txt"))?;

for cred in gcm_creds {
    store.add(cred)?;
}

println!("导入完成，共 {} 个凭证", store.list()?.len());
```

---

### 从环境变量迁移

#### 常见环境变量

```
GITHUB_TOKEN
GITLAB_TOKEN
BITBUCKET_USERNAME
BITBUCKET_PASSWORD
```

#### 导入脚本

```rust
use std::env;

fn import_from_env(store: &impl CredentialStore) -> Result<usize, Box<dyn std::error::Error>> {
    let mut count = 0;
    
    // GitHub
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        let username = env::var("GITHUB_USERNAME").unwrap_or_else(|_| "git".to_string());
        let cred = Credential::new(
            "github.com".to_string(),
            username,
            token,
        );
        store.add(cred)?;
        count += 1;
    }
    
    // GitLab
    if let Ok(token) = env::var("GITLAB_TOKEN") {
        let cred = Credential::new(
            "gitlab.com".to_string(),
            "oauth2".to_string(),
            token,
        );
        store.add(cred)?;
        count += 1;
    }
    
    // Bitbucket
    if let Ok(username) = env::var("BITBUCKET_USERNAME") {
        if let Ok(password) = env::var("BITBUCKET_PASSWORD") {
            let cred = Credential::new(
                "bitbucket.org".to_string(),
                username,
                password,
            );
            store.add(cred)?;
            count += 1;
        }
    }
    
    Ok(count)
}
```

---

### 从 .netrc 文件迁移

#### .netrc 格式

```
machine github.com
login alice
password ghp_token123

machine gitlab.com
login bob
password glpat_token456
```

#### 解析器

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};

fn import_from_netrc(netrc_path: &Path) -> Result<Vec<Credential>, Box<dyn std::error::Error>> {
    let file = File::open(netrc_path)?;
    let reader = BufReader::new(file);
    let mut credentials = Vec::new();
    
    let mut current_machine = None;
    let mut current_login = None;
    
    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() >= 2 {
            match parts[0] {
                "machine" => current_machine = Some(parts[1].to_string()),
                "login" => current_login = Some(parts[1].to_string()),
                "password" => {
                    if let (Some(machine), Some(login)) = (&current_machine, &current_login) {
                        let cred = Credential::new(
                            machine.clone(),
                            login.clone(),
                            parts[1].to_string(),
                        );
                        credentials.push(cred);
                        
                        // 重置
                        current_machine = None;
                        current_login = None;
                    }
                }
                _ => {}
            }
        }
    }
    
    Ok(credentials)
}
```

---

## 导入导出

### 导出为 JSON

```rust
fn export_to_json(
    store: &impl CredentialStore,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let credentials = store.list()?;
    
    // 警告：导出的 JSON 不包含密码（安全序列化）
    let json = serde_json::to_string_pretty(&credentials)?;
    std::fs::write(output_path, json)?;
    
    println!("已导出 {} 个凭证到 {}", credentials.len(), output_path.display());
    println!("警告：导出的 JSON 不包含密码字段（安全保护）");
    
    Ok(())
}
```

### 导出为加密备份（P6.2+）

```rust
// P6.2 加密导出功能（示例）
fn export_encrypted(
    store: &impl CredentialStore,
    output_path: &Path,
    password: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let credentials = store.list()?;
    
    // 包含密码的完整导出
    #[derive(serde::Serialize)]
    struct FullCredential {
        host: String,
        username: String,
        password_or_token: String,  // 包含密码！
        created_at: SystemTime,
        expires_at: Option<SystemTime>,
    }
    
    let full_creds: Vec<FullCredential> = credentials.into_iter()
        .map(|c| FullCredential {
            host: c.host,
            username: c.username,
            password_or_token: c.password_or_token,
            created_at: c.created_at,
            expires_at: c.expires_at,
        })
        .collect();
    
    let json = serde_json::to_string(&full_creds)?;
    
    // 使用 AES-256-GCM 加密
    let encrypted = encrypt_with_password(&json, password)?;
    std::fs::write(output_path, encrypted)?;
    
    println!("已加密导出 {} 个凭证", full_creds.len());
    
    Ok(())
}

// 对应的导入函数
fn import_encrypted(
    store: &impl CredentialStore,
    input_path: &Path,
    password: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let encrypted = std::fs::read(input_path)?;
    let json = decrypt_with_password(&encrypted, password)?;
    
    let full_creds: Vec<FullCredential> = serde_json::from_str(&json)?;
    
    for fc in &full_creds {
        let cred = Credential::new_with_expiry(
            fc.host.clone(),
            fc.username.clone(),
            fc.password_or_token.clone(),
            fc.expires_at.unwrap_or_else(|| {
                SystemTime::now() + Duration::from_secs(90 * 24 * 60 * 60)
            }),
        );
        
        // 跳过已存在的凭证
        if !store.exists(&cred.host, &cred.username) {
            store.add(cred)?;
        }
    }
    
    Ok(full_creds.len())
}
```

### CSV 导入

```rust
fn import_from_csv(
    store: &impl CredentialStore,
    csv_path: &Path,
) -> Result<usize, Box<dyn std::error::Error>> {
    use csv::Reader;
    
    let mut rdr = Reader::from_path(csv_path)?;
    let mut count = 0;
    
    for result in rdr.records() {
        let record = result?;
        
        // CSV 格式: host,username,password
        if record.len() >= 3 {
            let cred = Credential::new(
                record[0].to_string(),
                record[1].to_string(),
                record[2].to_string(),
            );
            
            if !store.exists(&record[0], &record[1]) {
                store.add(cred)?;
                count += 1;
            }
        }
    }
    
    Ok(count)
}
```

---

## 向后兼容性

### 配置兼容性矩阵

| 配置字段 | P6.0 | P6.1 | P6.2 | 说明 |
|---------|------|------|------|------|
| `storage: "memory"` | ✅ | ✅ | ✅ | 始终支持 |
| `storage: "system"` | ❌ | ✅ | ✅ | P6.1+ |
| `storage: "file"` | ❌ | ❌ | ✅ | P6.2+ |
| `defaultTtlSeconds` | ✅ | ✅ | ✅ | 可选，默认 90 天 |
| `filePath` | ❌ | ❌ | ✅ | `storage: "file"` 时必需 |

### API 兼容性

#### P6.0 API（基线）

```rust
pub trait CredentialStore {
    fn get(&self, host: &str, username: Option<&str>) -> Result<Option<Credential>>;
    fn add(&self, credential: Credential) -> Result<()>;
    fn remove(&self, host: &str, username: &str) -> Result<()>;
    fn list(&self) -> Result<Vec<Credential>>;
    fn update_last_used(&self, host: &str, username: &str) -> Result<()>;
    fn exists(&self, host: &str, username: &str) -> bool;
}
```

#### P6.1 扩展（向后兼容）

```rust
// 新增方法（使用默认实现保持兼容）
pub trait CredentialStore {
    // ... P6.0 方法 ...
    
    // P6.1 新增
    fn migrate_to_system(&self) -> Result<()> {
        // 默认实现：不支持迁移
        Err(CredentialStoreError::Other("不支持系统钥匙串迁移".into()))
    }
}
```

#### P6.2 扩展

```rust
pub trait CredentialStore {
    // ... P6.0 + P6.1 方法 ...
    
    // P6.2 新增
    fn export_encrypted(&self, path: &Path, password: &str) -> Result<()> {
        Err(CredentialStoreError::Other("不支持加密导出".into()))
    }
    
    fn import_encrypted(&self, path: &Path, password: &str) -> Result<usize> {
        Err(CredentialStoreError::Other("不支持加密导入".into()))
    }
}
```

### 数据格式兼容性

#### Credential 结构演化

```rust
// P6.0
#[derive(Serialize, Deserialize)]
pub struct Credential {
    pub host: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_or_token: String,
    pub created_at: SystemTime,
    pub expires_at: Option<SystemTime>,
    pub last_used_at: Option<SystemTime>,
}

// P6.2+ 可能新增字段（使用 #[serde(default)] 保持兼容）
#[derive(Serialize, Deserialize)]
pub struct Credential {
    // ... P6.0 字段 ...
    
    #[serde(default)]
    pub metadata: HashMap<String, String>,  // 新增：元数据
    
    #[serde(default)]
    pub tags: Vec<String>,  // 新增：标签
}
```

---

## 迁移工具

### 命令行工具（示例）

```rust
// src-tauri/src/bin/credential_migrate.rs

use clap::{Arg, Command};
use fireworks_collaboration_lib::core::credential::{
    storage::MemoryCredentialStore,
    model::Credential,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("credential-migrate")
        .version("1.0")
        .author("Your Name")
        .about("凭证迁移工具")
        .subcommand(
            Command::new("import")
                .about("导入凭证")
                .arg(Arg::new("format").required(true))
                .arg(Arg::new("file").required(true))
        )
        .subcommand(
            Command::new("export")
                .about("导出凭证")
                .arg(Arg::new("format").required(true))
                .arg(Arg::new("file").required(true))
        )
        .get_matches();
    
    match matches.subcommand() {
        Some(("import", sub_m)) => {
            let format = sub_m.get_one::<String>("format").unwrap();
            let file = sub_m.get_one::<String>("file").unwrap();
            
            println!("导入 {} 格式的凭证从 {}", format, file);
            // 实现导入逻辑
        }
        Some(("export", sub_m)) => {
            let format = sub_m.get_one::<String>("format").unwrap();
            let file = sub_m.get_one::<String>("file").unwrap();
            
            println!("导出凭证为 {} 格式到 {}", format, file);
            // 实现导出逻辑
        }
        _ => {
            println!("使用 --help 查看帮助");
        }
    }
    
    Ok(())
}
```

### PowerShell 迁移脚本

```powershell
# migrate_credentials.ps1

param(
    [string]$Source,
    [string]$Target,
    [string]$Format = "json"
)

Write-Host "凭证迁移工具"
Write-Host "源: $Source"
Write-Host "目标: $Target"
Write-Host "格式: $Format"

# 调用 Rust 工具
cargo run --bin credential_migrate -- import $Format $Source

Write-Host "迁移完成！"
```

---

## 最佳实践

1. **迁移前备份**：总是先导出现有凭证
2. **分批迁移**：大量凭证分批处理，避免一次性失败
3. **验证迁移**：迁移后检查凭证数量和完整性
4. **清理旧数据**：验证成功后再删除旧存储中的凭证
5. **记录日志**：保留迁移操作的详细日志

---

## 参考

- [CREDENTIAL_QUICKSTART.md](CREDENTIAL_QUICKSTART.md) - 快速入门
- [CREDENTIAL_ERROR_HANDLING.md](CREDENTIAL_ERROR_HANDLING.md) - 错误处理
- [CREDENTIAL_TROUBLESHOOTING.md](CREDENTIAL_TROUBLESHOOTING.md) - 故障排查
