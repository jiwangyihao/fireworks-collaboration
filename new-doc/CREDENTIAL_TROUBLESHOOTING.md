# 凭证管理故障排查指南

本文档提供凭证存储模块的常见问题排查流程、调试技巧和日志分析方法。

## 目录

- [快速诊断检查清单](#快速诊断检查清单)
- [常见问题排查](#常见问题排查)
- [调试技巧](#调试技巧)
- [日志分析](#日志分析)
- [性能问题](#性能问题)
- [安全问题](#安全问题)

---

## 快速诊断检查清单

遇到问题时，请按以下顺序检查：

```
□ 1. 检查凭证是否存在
   cargo test test_memory_store_add_and_get

□ 2. 检查凭证是否过期
   查看日志中的 "is_expired" 信息

□ 3. 检查存储配置
   验证 config.json 中的 credential 配置

□ 4. 检查错误日志
   查看 CredentialStoreError 类型

□ 5. 运行完整测试套件
   cargo test credential --lib
   cargo test --test credential

□ 6. 检查系统资源
   磁盘空间、文件权限、内存使用
```

---

## 常见问题排查

### 问题 1: 凭证添加失败

#### 症状

```
Error: 凭证已存在: alice@github.com
```

#### 诊断步骤

```rust
// 1. 检查凭证是否已存在
if store.exists("github.com", "alice") {
    println!("凭证已存在");
    
    // 2. 查看现有凭证详情
    if let Ok(Some(cred)) = store.get("github.com", Some("alice")) {
        println!("现有凭证:");
        println!("  创建时间: {:?}", cred.created_at);
        println!("  过期时间: {:?}", cred.expires_at);
        println!("  最后使用: {:?}", cred.last_used_at);
        println!("  是否过期: {}", cred.is_expired());
    }
}

// 3. 尝试删除后重新添加
store.remove("github.com", "alice")?;
store.add(new_cred)?;
```

#### 解决方案

见 [CREDENTIAL_ERROR_HANDLING.md#AlreadyExists](CREDENTIAL_ERROR_HANDLING.md#2-alreadyexists---凭证已存在)

---

### 问题 2: 凭证查询返回 None

#### 症状

```rust
let cred = store.get("github.com", Some("alice"))?;
assert!(cred.is_none());  // 意外的 None
```

#### 诊断步骤

```rust
// 1. 列出所有凭证
let all_creds = store.list()?;
println!("当前存储中的凭证: {}", all_creds.len());
for cred in &all_creds {
    println!("  - {}", cred.identifier());
}

// 2. 检查拼写
println!("查询: github.com / alice");
println!("实际: {} / {}", cred.host, cred.username);

// 3. 检查是否过期
if let Ok(all) = get_all_including_expired(&store) {
    for cred in all {
        if cred.host == "github.com" && cred.username == "alice" {
            println!("找到凭证，但已过期: {:?}", cred.expires_at);
        }
    }
}

// 辅助函数：获取包括过期凭证的所有凭证
fn get_all_including_expired(store: &MemoryCredentialStore) -> Result<Vec<Credential>, String> {
    // 需要访问内部数据结构（仅用于调试）
    // 在生产代码中应使用 list() 方法
    todo!("仅调试用，生产环境使用 store.list()")
}
```

#### 可能原因

1. **凭证不存在** - 从未添加或已被删除
2. **凭证已过期** - `expires_at < SystemTime::now()`
3. **拼写错误** - host 或 username 不匹配
4. **存储类型错误** - 使用了不同的存储实例

#### 解决方案

```rust
// 方案 1: 重新添加凭证
if !store.exists("github.com", "alice") {
    let cred = Credential::new(
        "github.com".to_string(),
        "alice".to_string(),
        "token".to_string(),
    );
    store.add(cred)?;
}

// 方案 2: 使用不指定用户名的查询
let cred = store.get("github.com", None)?;
if let Some(c) = cred {
    println!("找到 github.com 的凭证: {}", c.username);
}
```

---

### 问题 3: 并发访问冲突

#### 症状

```
Error: 存储访问错误: 读锁获取失败: PoisonError { ... }
```

#### 诊断步骤

```rust
use std::sync::Arc;
use std::thread;

// 1. 复现并发问题
let store = Arc::new(MemoryCredentialStore::new());
let mut handles = vec![];

for i in 0..10 {
    let store_clone = Arc::clone(&store);
    let handle = thread::spawn(move || {
        // 尝试触发锁冲突
        for _ in 0..100 {
            let cred = Credential::new(
                format!("host{}.com", i),
                "user".to_string(),
                "token".to_string(),
            );
            
            match store_clone.add(cred) {
                Ok(_) => {},
                Err(e) => println!("线程 {} 错误: {}", i, e),
            }
        }
    });
    handles.push(handle);
}

for handle in handles {
    handle.join().unwrap();
}

// 2. 检查是否有 panic
// 如果有线程 panic，锁会中毒
```

#### 解决方案

```rust
// 方案 1: 捕获 PoisonError 并恢复
use std::sync::PoisonError;

let credentials = match self.credentials.read() {
    Ok(guard) => guard,
    Err(poison_error) => {
        eprintln!("警告: 锁中毒，正在恢复");
        poison_error.into_inner()  // 恢复数据
    }
};

// 方案 2: 避免在锁内 panic
fn safe_operation(&self) -> Result<(), CredentialStoreError> {
    let guard = self.credentials.read()
        .map_err(|e| CredentialStoreError::AccessError(format!("读锁失败: {}", e)))?;
    
    // 所有操作都返回 Result，不使用 unwrap()
    Ok(())
}
```

---

### 问题 4: 凭证过期过快

#### 症状

```
凭证刚添加就过期了
```

#### 诊断步骤

```rust
use std::time::{SystemTime, Duration};

// 1. 检查系统时间
let now = SystemTime::now();
println!("当前系统时间: {:?}", now);

// 2. 检查凭证的过期时间
let cred = Credential::new_with_expiry(
    "github.com".to_string(),
    "alice".to_string(),
    "token".to_string(),
    SystemTime::now() + Duration::from_secs(3600),  // 1小时
);

println!("过期时间: {:?}", cred.expires_at);
println!("创建时间: {:?}", cred.created_at);

if let Some(expires) = cred.expires_at {
    if let Ok(duration) = expires.duration_since(SystemTime::now()) {
        println!("距离过期: {:?}", duration);
    }
}

// 3. 检查配置中的 defaultTtlSeconds
let config = CredentialConfig::default();
println!("默认 TTL: {:?}", config.default_ttl_seconds);
```

#### 解决方案

```rust
// 方案 1: 使用更长的过期时间
let expires_at = SystemTime::now() + Duration::from_secs(90 * 24 * 60 * 60);  // 90天

// 方案 2: 使用永不过期（None）
let cred = Credential::new(  // 不指定 expires_at
    "github.com".to_string(),
    "alice".to_string(),
    "token".to_string(),
);

// 方案 3: 在配置中设置默认 TTL
let config = CredentialConfig::new()
    .with_ttl(Some(90 * 24 * 60 * 60));  // 90天
```

---

### 问题 5: 内存使用过高

#### 症状

```
应用内存占用持续增长
```

#### 诊断步骤

```rust
// 1. 检查凭证数量
let count = store.list()?.len();
println!("当前凭证数量: {}", count);

// 2. 检查是否有内存泄漏
use std::mem::size_of_val;

let all_creds = store.list()?;
let total_size: usize = all_creds.iter()
    .map(|c| {
        size_of_val(&c.host) +
        size_of_val(&c.username) +
        size_of_val(&c.password_or_token)
    })
    .sum();

println!("凭证占用内存（估算）: {} bytes", total_size);

// 3. 列出所有凭证，检查异常
for cred in &all_creds {
    println!("{}: token长度={}", 
        cred.identifier(), 
        cred.password_or_token.len()
    );
}
```

#### 解决方案

```rust
// 方案 1: 定期清理过期凭证
fn cleanup_expired(store: &MemoryCredentialStore) -> Result<usize, CredentialStoreError> {
    let all = store.list()?;
    let mut removed = 0;
    
    for cred in all {
        if cred.is_expired() {
            store.remove(&cred.host, &cred.username)?;
            removed += 1;
        }
    }
    
    Ok(removed)
}

// 方案 2: 限制凭证数量
const MAX_CREDENTIALS: usize = 1000;

fn add_with_limit(
    store: &MemoryCredentialStore,
    cred: Credential,
) -> Result<(), CredentialStoreError> {
    if store.list()?.len() >= MAX_CREDENTIALS {
        return Err(CredentialStoreError::Other(
            "凭证数量已达上限".to_string()
        ));
    }
    
    store.add(cred)
}

// 方案 3: 清空所有凭证
fn clear_all(store: &MemoryCredentialStore) -> Result<(), CredentialStoreError> {
    for cred in store.list()? {
        store.remove(&cred.host, &cred.username)?;
    }
    Ok(())
}
```

---

## 调试技巧

### 1. 启用详细日志

```rust
// 在 Cargo.toml 中
[dependencies]
env_logger = "0.10"
log = "0.4"

// 在 main.rs 中
fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();
    
    // 你的代码
}

// 使用
use log::{debug, info, warn, error};

debug!("查询凭证: {}@{}", username, host);
info!("凭证已添加: {}", cred.identifier());
warn!("凭证即将过期: {}", cred.identifier());
error!("凭证操作失败: {}", e);
```

### 2. 打印调试信息

```rust
// 使用 Debug trait（自动脱敏）
println!("{:?}", cred);

// 使用 Display trait（自动脱敏）
println!("{}", cred);

// 获取详细信息
println!("Credential:");
println!("  identifier: {}", cred.identifier());
println!("  host: {}", cred.host);
println!("  username: {}", cred.username);
println!("  password: {} (masked)", cred.masked_password());
println!("  created_at: {:?}", cred.created_at);
println!("  expires_at: {:?}", cred.expires_at);
println!("  last_used_at: {:?}", cred.last_used_at);
println!("  is_expired: {}", cred.is_expired());
```

### 3. 单元测试调试

```rust
#[test]
fn debug_credential_issue() {
    // 设置测试环境
    let store = MemoryCredentialStore::new();
    
    // 添加测试数据
    let cred = Credential::new(
        "github.com".to_string(),
        "test".to_string(),
        "token123".to_string(),
    );
    store.add(cred).unwrap();
    
    // 断点调试位置
    let retrieved = store.get("github.com", Some("test")).unwrap();
    
    // 打印调试信息
    dbg!(&retrieved);
    
    // 断言
    assert!(retrieved.is_some());
}
```

### 4. 使用 dbg! 宏

```rust
// 打印变量值并返回
let cred = dbg!(store.get("github.com", Some("alice"))?);

// 打印表达式
dbg!(cred.is_expired());

// 打印多个值
dbg!(&cred.host, &cred.username, cred.is_expired());
```

---

## 日志分析

### 正常操作日志模式

```
[INFO] 凭证已添加: alice@github.com
[DEBUG] 查询凭证: alice@github.com
[INFO] 成功获取凭证: alice@github.com
[DEBUG] 更新最后使用时间: alice@github.com
[INFO] 凭证已删除: alice@github.com
```

### 错误日志模式

#### 模式 1: 凭证不存在

```
[DEBUG] 查询凭证: bob@github.com
[WARN] 凭证不存在或已过期: bob@github.com
```

**分析**: 凭证可能未创建或已过期  
**操作**: 检查凭证是否存在，或重新添加

#### 模式 2: 并发冲突

```
[ERROR] 存储访问错误: 写锁获取失败: ...
[ERROR] 存储访问错误: 写锁获取失败: ...
[ERROR] 存储访问错误: 写锁获取失败: ...
```

**分析**: 高并发写入导致锁竞争  
**操作**: 优化并发访问模式，或增加重试逻辑

#### 模式 3: 重复添加

```
[INFO] 尝试添加凭证: alice@github.com
[ERROR] 凭证已存在: alice@github.com
[INFO] 尝试添加凭证: alice@github.com
[ERROR] 凭证已存在: alice@github.com
```

**分析**: 程序逻辑问题，未检查凭证是否存在  
**操作**: 在添加前使用 `exists()` 检查

---

## 性能问题

### 诊断工具

```rust
use std::time::Instant;

// 测量操作耗时
fn benchmark_operation<F, T>(name: &str, mut op: F) -> T
where
    F: FnMut() -> T,
{
    let start = Instant::now();
    let result = op();
    let duration = start.elapsed();
    
    println!("{} 耗时: {:?}", name, duration);
    result
}

// 使用示例
let cred = benchmark_operation("添加凭证", || {
    store.add(Credential::new(
        "github.com".to_string(),
        "user".to_string(),
        "token".to_string(),
    ))
});

let list = benchmark_operation("列出凭证", || {
    store.list()
});
```

### 性能基准

| 操作 | 预期耗时 (内存存储) | 说明 |
|------|-------------------|------|
| add() | < 1ms | 单次添加 |
| get() | < 1ms | 单次查询 |
| remove() | < 1ms | 单次删除 |
| list() | < 10ms | 1000 个凭证 |
| update_last_used() | < 1ms | 单次更新 |

### 性能优化

见 [CREDENTIAL_PERFORMANCE.md](CREDENTIAL_PERFORMANCE.md)

---

## 安全问题

### 检查清单

```
□ 1. 日志中是否包含明文密码？
   grep -i "password\|token" logs/*.log
   # 应该只看到脱敏的密码 (****cdef)

□ 2. 序列化输出是否安全？
   检查 JSON/配置文件是否包含密码字段

□ 3. 凭证是否设置过期时间？
   长期凭证应设置 90 天过期时间

□ 4. 是否定期清理过期凭证？
   实现自动清理机制

□ 5. 并发访问是否安全？
   运行并发测试验证
```

### 安全审计脚本

```bash
# PowerShell 脚本

# 检查日志中的敏感信息
Select-String -Path "logs\*.log" -Pattern "ghp_[a-zA-Z0-9]{36}" -CaseSensitive
# 如果找到完整 token，说明日志泄露

# 检查序列化文件
Get-Content "config.json" | Select-String -Pattern '"password"|"token"' -CaseSensitive
# 应该没有 password_or_token 字段

# 检查凭证数量
cargo test test_memory_store_list -- --nocapture | Select-String "len"
```

---

## 高级调试

### 使用 LLDB/GDB 调试

```bash
# 编译 debug 版本
cargo build

# 使用 rust-lldb
rust-lldb ./target/debug/fireworks-collaboration

# 设置断点
(lldb) b credential::storage::add
(lldb) run

# 查看变量
(lldb) p cred
(lldb) p self.credentials
```

### 使用 cargo-expand 查看宏展开

```bash
cargo install cargo-expand
cargo expand core::credential::model
```

### 使用 Valgrind 检测内存泄漏（Linux）

```bash
cargo build --release
valgrind --leak-check=full ./target/release/fireworks-collaboration
```

---

## 获取帮助

如果以上方法都无法解决问题：

1. **查看相关文档**
   - [CREDENTIAL_QUICKSTART.md](CREDENTIAL_QUICKSTART.md)
   - [CREDENTIAL_ERROR_HANDLING.md](CREDENTIAL_ERROR_HANDLING.md)
   - [CREDENTIAL_USAGE_EXAMPLES.md](CREDENTIAL_USAGE_EXAMPLES.md)

2. **运行完整测试套件**
   ```powershell
   cargo test credential --lib -- --nocapture
   cargo test --test credential -- --nocapture
   ```

3. **检查系统环境**
   - Rust 版本: `rustc --version`
   - Cargo 版本: `cargo --version`
   - 操作系统: `systeminfo` (Windows)

4. **收集诊断信息**
   - 错误消息完整文本
   - 复现步骤
   - 相关配置文件
   - 日志文件

5. **提交 Issue**
   - 提供上述诊断信息
   - 附上最小复现代码
   - 说明预期行为和实际行为
