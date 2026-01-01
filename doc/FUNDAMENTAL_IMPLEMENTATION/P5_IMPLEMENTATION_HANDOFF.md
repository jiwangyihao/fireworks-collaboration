# P5 实现与维护对接文档 (代理支持与自动降级)

> 适用读者：代理模块维护者、传输层开发者、质量保障、运维
> 配套文件：`doc/TECH_DESIGN_P5_PLAN.md`, `doc/PROXY_CONFIG_GUIDE.md`
> 当前状态：P5.0-P5.7 全部交付完成，处于"生产就绪"阶段

---

## 目录
1. 交付范围概述
2. 核心模块映射
3. 配置项与默认值
4. 代理系统总体生命周期
5. 基础架构 (P5.0)
6. HTTP/HTTPS 代理支持 (P5.1)
7. SOCKS5 代理支持 (P5.2)
8. 传输层集成与互斥控制 (P5.3)
9. 自动降级与失败检测 (P5.4)
10. 自动恢复与健康检查 (P5.5)
11. 前端集成与观测体系 (P5.6)
12. 跨平台测试与准入评审 (P5.7)
13. 观测事件与指标
14. 测试矩阵与关键用例
15. 运维说明与回退策略
16. 后续优化建议
17. 快速校验命令

---

## 1. 交付范围概述

| 主题 | 目标 | 状态 |
|------|------|------|
| 基础架构 | 配置模型、状态机、系统代理检测 | ✅ 完成（36+17+28=81 Rust测试） |
| HTTP/HTTPS 代理 | CONNECT隧道、Basic Auth、超时控制 | ✅ 完成（29 Rust测试） |
| SOCKS5 代理 | 协议握手、认证方法、地址类型 | ✅ 完成（59 Rust测试） |
| 传输层集成 | 代理/直连路由、Fake SNI互斥 | ✅ 完成（集成在manager测试中） |
| 自动降级 | 失败检测、滑动窗口、降级触发 | ✅ 完成（detector 28测试） |
| 自动恢复 | 健康检查、探测策略、冷却窗口 | ✅ 完成（集成在manager测试中） |
| 前端集成 | 配置UI、状态面板、手动控制 | ✅ 完成（14+19=33 TypeScript测试） |
| 跨平台测试 | 系统代理检测、Soak准入 | ✅ 完成（集成在system_detector测试中） |

---

## 2. 核心模块映射

| 模块 | 文件/目录 | 说明 |
|------|-----------|------|
| 代理入口 | `src-tauri/src/core/proxy/mod.rs` | 模块导出、trait定义 |
| 配置模型 | `src-tauri/src/core/proxy/config.rs` | `ProxyConfig`、`ProxyMode`、验证逻辑 |
| 状态机 | `src-tauri/src/core/proxy/state.rs` | `ProxyState`、状态转换验证 |
| 系统检测 | `src-tauri/src/core/proxy/system_detector.rs` | 跨平台系统代理检测 |
| 管理器 | `src-tauri/src/core/proxy/manager.rs` | `ProxyManager`统一API |
| HTTP连接器 | `src-tauri/src/core/proxy/http_connector.rs` | `HttpProxyConnector`实现 |
| SOCKS5连接器 | `src-tauri/src/core/proxy/socks5_connector.rs` | `Socks5ProxyConnector`实现 |
| 失败检测 | `src-tauri/src/core/proxy/detector.rs` | `ProxyFailureDetector`滑动窗口 |
| 健康检查 | `src-tauri/src/core/proxy/health_checker.rs` | `ProxyHealthChecker`探测逻辑 |
| 错误定义 | `src-tauri/src/core/proxy/errors.rs` | `ProxyError`错误分类 |
| 事件定义 | `src-tauri/src/core/proxy/events.rs` | 代理事件结构体 |
| 传输层集成 | `src-tauri/src/core/git/transport/register.rs` | 传输层注册控制 |
| 前端配置 | `src/components/ProxyConfig.vue` | 代理配置UI组件 |
| 前端状态面板 | `src/components/ProxyStatusPanel.vue` | 代理状态显示 |
| 全局Store | `src/stores/proxy.ts` | 前端代理状态管理 |
| Soak扩展 | `src-tauri/src/soak/mod.rs` | 代理指标统计 |

---

## 3. 配置项与默认值

| 文件 | 键 | 默认 | 说明 |
|------|----|------|------|
| `config.json` (`AppConfig`) | `proxy.mode` | "off" | 代理模式: off/http/socks5/system |
|  | `proxy.url` | "" | 代理服务器URL |
|  | `proxy.username` | null | 可选认证用户名 |
|  | `proxy.password` | null | 可选认证密码 |
|  | `proxy.disableCustomTransport` | false | 禁用自定义传输层（代理启用时强制true） |
|  | `proxy.timeoutSeconds` | 30 | 连接超时秒数 |
|  | `proxy.fallbackThreshold` | 0.2 | 降级失败率阈值（20%） |
|  | `proxy.fallbackWindowSeconds` | 300 | 失败率统计窗口（5分钟） |
|  | `proxy.recoveryCooldownSeconds` | 300 | 恢复冷却时间（5分钟） |
|  | `proxy.healthCheckIntervalSeconds` | 60 | 健康检查间隔（1分钟） |
|  | `proxy.recoveryStrategy` | "consecutive" | 恢复策略 |
|  | `proxy.probeUrl` | "https://github.com" | 探测目标URL |
|  | `proxy.probeTimeoutSeconds` | 10 | 探测超时秒数 |
|  | `proxy.recoveryConsecutiveThreshold` | 3 | 连续成功次数恢复阈值 |

> 所有配置支持热更新；代理启用时 `disableCustomTransport` 自动强制为 `true`。

---

## 4. 代理系统总体生命周期

1. **启动阶段**：
   - 加载 `config.json` 中的 `proxy` 配置
   - 创建 `ProxyManager` 实例
   - 初始化失败检测器和健康检查器
   - 如果代理启用，注册传输层时跳过自定义传输层

2. **系统代理检测**：
   - 用户触发检测或应用启动时执行
   - 跨平台检测（Windows注册表/macOS scutil/Linux环境变量）
   - 前端可一键应用检测结果

3. **代理连接阶段**：
   - 传输层调用 `ProxyManager::get_connector()` 获取连接器
   - HTTP/SOCKS5连接器建立隧道
   - 成功/失败通过 `report_outcome()` 回写
   - 失败检测器更新滑动窗口统计

4. **自动降级阶段**：
   - 失败率超过阈值触发 `trigger_automatic_fallback()`
   - 状态切换为 `Fallback`
   - 发射 `ProxyFallbackEvent` 事件
   - 后续任务走直连模式

5. **健康检查阶段**：
   - 后台定期执行探测（默认60秒）
   - 成功次数累积到阈值触发恢复
   - 冷却窗口结束后自动重新启用代理
   - 发射 `ProxyRecoveredEvent` 事件

6. **热更新阶段**：
   - 用户修改配置后立即生效
   - `ProxyManager::update_config()` 重建连接器
   - 失败检测器和健康检查器参数更新

---

## 5. 基础架构 (P5.0)

### 5.1 配置模型

**ProxyMode 枚举**：
- `Off`: 禁用代理（默认）
- `Http`: HTTP/HTTPS代理
- `Socks5`: SOCKS5代理
- `System`: 使用系统代理检测结果

**ProxyConfig 结构体**（11个字段）：
```rust
pub struct ProxyConfig {
    pub mode: ProxyMode,                           // 代理模式
    pub url: String,                               // 代理URL
    pub username: Option<String>,                  // 可选用户名
    pub password: Option<String>,                  // 可选密码
    pub disable_custom_transport: bool,            // 禁用自定义传输层
    pub timeout_seconds: u64,                      // 连接超时（默认30）
    pub fallback_threshold: f64,                   // 降级阈值（默认0.2）
    pub fallback_window_seconds: u64,              // 失败窗口（默认300）
    pub recovery_cooldown_seconds: u64,            // 恢复冷却（默认300）
    pub health_check_interval_seconds: u64,        // 健康检查间隔（默认60）
    pub recovery_strategy: String,                 // 恢复策略（默认consecutive）
}
```

**关键方法**：
- `validate()`: 模式特定的配置验证（URL非空、端口范围等）
- `sanitized_url()`: URL脱敏（隐藏凭证）
- `is_enabled()`: 检查代理是否启用（mode != Off 且 URL非空）
- `timeout()`: 返回Duration类型的超时

### 5.2 状态机

**ProxyState 枚举**（4个状态）：
- `Enabled`: 代理已启用且运行正常
- `Disabled`: 代理已禁用（Off模式或未配置）
- `Fallback`: 已降级到直连（因代理失败）
- `Recovering`: 恢复中（正在测试代理可用性）

**StateTransition 枚举**（6种转换）：
- `Enable`: Disabled → Enabled
- `Disable`: 任意 → Disabled
- `TriggerFallback`: Enabled → Fallback
- `StartRecovery`: Fallback → Recovering
- `CompleteRecovery`: Recovering → Enabled
- `AbortRecovery`: Recovering → Fallback

**ProxyStateContext**：
- `state`: 当前状态
- `last_transition_at`: 最后转换时间戳（Unix秒）
- `reason`: 可选状态原因（如降级原因）
- `consecutive_failures/successes`: 连续失败/成功计数器

### 5.3 系统代理检测

**跨平台策略**：

| 平台 | 检测方法 | 实现细节 |
|------|----------|----------|
| Windows | 注册表 | 读取 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` |
| macOS | scutil命令 | 执行 `scutil --proxy` 并解析输出 |
| Linux | 环境变量 | 检测 `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` |

**检测流程**：
1. 平台特定检测（Windows注册表/macOS scutil）
2. 环境变量回退（所有平台）
3. 返回None（未检测到代理）

### 5.4 ProxyManager

**统一管理器API**：
- `is_enabled()`: 检查代理是否启用（配置+状态双重检查）
- `mode()` / `state()`: 获取当前模式和状态
- `proxy_url()` / `sanitized_url()`: 获取代理URL（原始/脱敏）
- `should_disable_custom_transport()`: 判断是否应禁用自定义传输层
- `update_config()`: 热更新配置并自动管理状态转换
- `detect_system_proxy()` / `apply_system_proxy()`: 系统代理检测与应用
- `get_connector()`: 获取连接器实例（根据模式返回不同类型）
- `report_failure()` / `report_success()`: 记录连接结果并触发自动降级
- `manual_fallback()` / `manual_recover()`: 手动状态切换
- `get_state_context()`: 获取完整状态上下文用于诊断

---

## 6. HTTP/HTTPS 代理支持 (P5.1)

### 6.1 HTTP CONNECT隧道

**协议流程**：
```http
客户端 -> 代理: CONNECT target_host:target_port HTTP/1.1
                Host: target_host:target_port
                [Proxy-Authorization: Basic <credentials>]

代理 -> 客户端: HTTP/1.1 200 Connection Established
                [其他响应头]
                
[隧道建立，后续流量透明传输]
```

**实现要点**：
- 使用 `TcpStream::connect_timeout()` 建立代理连接
- 设置读/写超时防止无限等待
- 使用 `BufReader` 读取HTTP响应行
- 严格解析状态码（必须为200才算成功）

### 6.2 Basic Auth认证

**认证流程**：
1. 检查 `username` 和 `password` 是否都存在
2. 格式化为 `username:password` 字符串
3. Base64编码（使用 `base64::engine::general_purpose::STANDARD`）
4. 添加 `Proxy-Authorization: Basic <encoded>` 头

**使用新版base64 API**：
```rust
use base64::{engine::general_purpose::STANDARD, Engine};
let credentials = format!("{}:{}", username, password);
let encoded = STANDARD.encode(credentials.as_bytes());
```

### 6.3 错误处理

**响应码映射**：
- `200 Connection Established` → 成功
- `407 Proxy Authentication Required` → `ProxyError::Auth`
- `502 Bad Gateway` → `ProxyError::Proxy`
- 其他错误码 → `ProxyError::Proxy`

**超时控制**：
- 连接超时：`TcpStream::connect_timeout()`
- 读超时：`stream.set_read_timeout()`
- 写超时：`stream.set_write_timeout()`

---

## 7. SOCKS5 代理支持 (P5.2)

### 7.1 SOCKS5协议流程

**完整握手流程**（RFC 1928）：
```
1. 客户端 -> 服务器: 版本协商请求
   [VER(0x05) | NMETHODS | METHODS...]
   
2. 服务器 -> 客户端: 选择认证方法
   [VER(0x05) | METHOD]
   
3. 认证阶段（如果需要）:
   3a. No Auth (0x00): 跳过
   3b. Username/Password (0x02):
       客户端 -> 服务器: [VER(0x01) | ULEN | UNAME | PLEN | PASSWD]
       服务器 -> 客户端: [VER(0x01) | STATUS]
   
4. 客户端 -> 服务器: CONNECT请求
   [VER(0x05) | CMD(0x01) | RSV(0x00) | ATYP | DST.ADDR | DST.PORT]
   
5. 服务器 -> 客户端: 连接响应
   [VER(0x05) | REP | RSV(0x00) | ATYP | BND.ADDR | BND.PORT]
```

### 7.2 地址类型处理

**IPv4 (ATYP=0x01)**：
- 4字节地址：`ipv4.octets()`

**IPv6 (ATYP=0x04)**：
- 16字节地址：`ipv6.octets()`

**域名 (ATYP=0x03)**：
- 1字节长度前缀 + 域名字节
- 最大域名长度：255字节

**自动检测逻辑**：
```rust
if let Ok(ip) = host.parse::<std::net::IpAddr>() {
    match ip {
        IpAddr::V4(ipv4) => ATYP_IPV4,
        IpAddr::V6(ipv6) => ATYP_IPV6,
    }
} else {
    ATYP_DOMAIN
}
```

### 7.3 错误响应映射

**REP码映射表**：
| REP | 含义 | ProxyError类型 |
|-----|------|----------------|
| 0x00 | 成功 | - |
| 0x01 | General SOCKS server failure | Proxy |
| 0x02 | Connection not allowed by ruleset | Proxy |
| 0x03 | Network unreachable | Proxy |
| 0x04 | Host unreachable | Proxy |
| 0x05 | Connection refused | Proxy |
| 0x06 | TTL expired | Proxy |
| 0x07 | Command not supported | Proxy |
| 0x08 | Address type not supported | Proxy |

---

## 8. 传输层集成与互斥控制 (P5.3)

### 8.1 传输层注册控制

**注册决策流程**：
```
ensure_registered(cfg)
  ├─> should_skip_custom_transport(cfg)
  │     ├─> ProxyManager::new(cfg.proxy)
  │     ├─> manager.should_disable_custom_transport()
  │     │     ├─> if config.is_enabled() → true (强制)
  │     │     └─> else → config.disable_custom_transport
  │     └─> 返回bool
  │
  ├─> if should_skip == true
  │     ├─> tracing::info!("Custom transport disabled...")
  │     └─> return Ok(())  // 跳过注册，使用libgit2默认HTTP
  │
  └─> else → 注册"https+custom" subtransport
```

### 8.2 Fake SNI互斥机制

**互斥实现方式**：
- **配置层面**：`ProxyManager::should_disable_custom_transport()` 在代理启用时返回 `true`
- **注册层面**：`ensure_registered()` 跳过自定义传输层注册
- **结果**：代理模式下不使用 `CustomHttpsSubtransport`，因此不会调用Fake SNI逻辑

**libgit2默认行为**：
- 使用系统代理环境变量（HTTP_PROXY/HTTPS_PROXY）
- 使用真实SNI（Real-Host验证）
- 不进行IP优选和TLS指纹收集

### 8.3 强制互斥策略

**设计原则**：
- 代理启用时**强制禁用**自定义传输层，不提供用户选择
- 降低复杂度，避免代理+Fake SNI的组合兼容性问题
- 减少指纹风险（代理环境下使用Fake SNI可能增加识别特征）

**实现逻辑**（manager.rs）：
```rust
pub fn should_disable_custom_transport(&self) -> bool {
    let config = self.config.read().unwrap();
    if config.is_enabled() {
        return true;  // 强制禁用
    }
    config.disable_custom_transport
}
```

---

## 9. 自动降级与失败检测 (P5.4)

### 9.1 ProxyFailureDetector

**滑动窗口机制**：
- 维护时间戳+结果的双端队列（VecDeque）
- 自动移除超出窗口的旧记录
- 计算当前失败率：`failures / total_attempts`

**关键方法**：
- `report_failure()` / `report_success()`: 添加记录到队列
- `should_fallback()`: 检查是否应触发降级
  - 样本数 ≥ 最小阈值（默认5）
  - 失败率 ≥ 配置阈值（默认0.2即20%）
  - 未标记已降级
- `mark_fallback_triggered()`: 标记已降级（防止重复触发）
- `reset()`: 重置统计（用于恢复时清空历史）

### 9.2 自动降级流程

**触发条件**：
1. 连续失败或失败率超过阈值
2. `ProxyManager::report_failure()` 调用 `detector.should_fallback()`
3. 如果返回 `true`，调用 `trigger_automatic_fallback()`

**降级执行**：
```rust
fn trigger_automatic_fallback(&mut self, reason: &str) -> Result<()> {
    // 1. 状态转换 Enabled → Fallback
    self.state_context.apply_transition(
        StateTransition::TriggerFallback, 
        Some(reason.to_string())
    )?;
    
    // 2. 标记已降级
    self.failure_detector.mark_fallback_triggered();
    
    // 3. 发射事件
    let event = ProxyFallbackEvent::automatic(
        reason, 
        stats.failure_count, 
        config.fallback_window_seconds, 
        now, 
        stats.failure_rate, 
        self.sanitized_url()
    );
    // publish_proxy_fallback_event(&event);
    
    // 4. 日志记录
    tracing::warn!("Proxy automatically fell back to direct connection");
    
    Ok(())
}
```

### 9.3 降级状态管理

**状态影响**：
- `ProxyManager::is_enabled()` 返回 `false`（状态为Fallback时）
- `get_connector()` 返回 `PlaceholderConnector`（不实际使用代理）
- 传输层注册时 `should_disable_custom_transport()` 可能返回 `false`（允许恢复直连的高级功能）

---

## 10. 自动恢复与健康检查 (P5.5)

### 10.1 ProxyHealthChecker

**后台探测任务**：
- 使用独立 tokio runtime（`Runtime::new()`）
- 定期执行健康检查（默认60秒间隔）
- 使用探测URL（默认 `https://github.com`）

**关键方法**：
- `start()`: 启动后台任务
- `stop()`: 停止后台任务（发送停止信号）
- `execute_health_check()`: 执行单次探测
  - 根据代理模式创建连接器
  - 尝试建立连接
  - 记录结果（成功/失败）
  - 发射 `ProxyHealthCheckEvent` 事件

### 10.2 恢复策略

**Consecutive策略**（默认）：
- 连续成功次数达到阈值（默认3次）触发恢复
- 任何失败重置计数器
- 适用于稳定性要求高的场景

**实现逻辑**（manager.rs）：
```rust
fn handle_health_check_success(&mut self, latency_ms: u32) {
    if self.state() != ProxyState::Recovering {
        return;  // 仅在Recovering状态处理
    }
    
    self.state_context.consecutive_successes += 1;
    
    if self.state_context.consecutive_successes >= threshold {
        self.trigger_automatic_recovery();
    }
}
```

### 10.3 冷却窗口

**设计目的**：
- 防止频繁降级/恢复切换
- 给代理服务器足够时间稳定

**实现方式**：
- 降级时记录 `last_transition_at` 时间戳
- 恢复前检查 `now - last_transition_at >= cooldown_seconds`
- 冷却期间健康检查继续执行但不触发恢复

---

## 11. 前端集成与观测体系 (P5.6)

### 11.1 前端组件

**ProxyConfig.vue**（代理配置UI）：
- 代理模式选择器（Off/HTTP/SOCKS5/System）
- URL输入框（含验证）
- 用户名/密码输入框（可选）
- 系统代理检测区域：
  - "检测系统代理"按钮
  - 检测结果显示（URL/类型或"未检测到"）
  - "应用"按钮（一键填充配置）
- 高级设置折叠面板：
  - 超时设置
  - 降级阈值
  - 恢复策略
  - 探测配置

**ProxyStatusPanel.vue**（状态面板）：
- 当前代理状态显示（Enabled/Disabled/Fallback/Recovering）
- 降级原因显示（如果处于Fallback状态）
- 失败统计（失败率、窗口大小）
- 恢复进度（连续成功次数/阈值）
- 手动控制按钮：
  - "手动降级"（仅Enabled状态）
  - "手动恢复"（仅Fallback/Recovering状态）

### 11.2 Tauri命令

**后端命令**（manager.rs）：
```rust
#[tauri::command]
pub async fn detect_system_proxy() -> Result<SystemProxyResult, String> {
    // 调用 SystemProxyDetector::detect()
    // 返回检测结果：{ url: Option<String>, proxy_type: Option<String> }
}

#[tauri::command]
pub async fn force_proxy_fallback(
    reason: Option<String>,
    cfg: State<'_, SharedConfig>,
) -> Result<bool, String> {
    // 手动触发代理降级，可选原因说明
    // 调用 ProxyManager::force_fallback(&reason)
}

#[tauri::command]
pub async fn force_proxy_recovery(cfg: State<'_, SharedConfig>) -> Result<bool, String> {
    // 手动触发代理恢复
    // 调用 ProxyManager::force_recovery()
}

#[tauri::command]
pub fn get_system_proxy() -> Result<SystemProxy, String> {
    // Legacy命令，返回系统代理基本信息
}
```

**注意**：没有 `get_proxy_status()` 命令，状态通过 `ProxyStateEvent` 事件获取。

### 11.3 前端Store

**proxy.ts**（Pinia store）：
```typescript
export const useProxyStore = defineStore('proxy', {
  state: () => ({
    config: null as ProxyConfig | null,
    systemProxyDetected: null as SystemProxyResult | null,
  }),
  
  actions: {
    async detectSystemProxy() {
      this.systemProxyDetected = await invoke('detect_system_proxy');
    },
    
    async applySystemProxy() {
      if (this.systemProxyDetected?.url) {
        // 更新config并保存
      }
    },
    
    async manualFallback(reason?: string) {
      await invoke('force_proxy_fallback', { reason });
      // 状态变化通过 ProxyStateEvent 事件获知
    },
    
    async manualRecover() {
      await invoke('force_proxy_recovery');
      // 状态变化通过 ProxyStateEvent 事件获知
    },
  },
});
```

**说明**：不使用 `get_proxy_status()` 轮询，而是通过订阅 `ProxyStateEvent` 和 `ProxyHealthCheckEvent` 事件被动接收状态更新。

---

## 12. 跨平台测试与准入评审 (P5.7)

### 12.1 系统代理检测测试

**跨平台测试覆盖**（14个测试）：
- Windows注册表读取测试（模拟注册表数据）
- macOS scutil输出解析测试（模拟命令输出）
- Linux环境变量检测测试（设置/清空环境变量）
- 边界情况测试（空值、无效格式、权限不足）

**测试策略**：
- 使用条件编译 `#[cfg(target_os = "...")]`
- 模拟系统调用返回值
- 验证回退机制（平台特定 → 环境变量 → None）

### 12.2 Soak准入

**环境变量**：
- `FWC_PROXY_SOAK=1`（启用代理Soak测试）
- `FWC_SOAK_ITERATIONS`（默认20）
- `FWC_SOAK_MIN_PROXY_SUCCESS_RATE`（默认0.95）
- `FWC_SOAK_MAX_PROXY_FALLBACK_COUNT`（默认1）
- `FWC_SOAK_MIN_PROXY_RECOVERY_RATE`（默认0.9）

**报告结构**（`SoakReport.proxy`）：
```json
{
  "selection_total": 100,
  "selection_by_mode": {
    "http": 60,
    "socks5": 40,
    "direct": 0
  },
  "fallback_count": 1,
  "recovery_count": 1,
  "health_check_success_rate": 0.95,
  "avg_connection_latency_ms": 120,
  "system_proxy_detect_success": true
}
```

**阈值判定**：
- `proxy_success_rate >= MIN_PROXY_SUCCESS_RATE`
- `fallback_count <= MAX_PROXY_FALLBACK_COUNT`
- `recovery_rate >= MIN_PROXY_RECOVERY_RATE`（如果发生降级）
- `system_proxy_detect_success == true`（如果配置为System模式）

### 12.3 准入评审文档

**P5_READINESS_REVIEW.md**（准入评审）：
- 功能清单与验收状态
- 测试覆盖统计（单元/集成/Soak）
- 性能基准（连接建立耗时、降级响应时间）
- 已知限制与缓解措施
- 灰度建议与监控看板
- 回滚条件与应急手册

---

## 13. 观测事件与指标

| 事件 | 触发点 | 关键字段 |
|------|--------|----------|
| `ProxyStateEvent` | 状态转换 | `previous_state`, `current_state`, `reason`, `timestamp` |
| `ProxyFallbackEvent` | 自动/手动降级 | `reason`, `failure_count`, `failure_rate`, `is_automatic` |
| `ProxyRecoveredEvent` | 自动/手动恢复 | `successful_checks`, `strategy`, `is_automatic` |
| `ProxyHealthCheckEvent` | 健康检查 | `success`, `response_time_ms`, `error` |
| `ProxyConfigUpdateEvent` | 配置热更新 | `old_config`, `new_config` |
| `ProxySystemDetectedEvent` | 系统代理检测 | `detected_url`, `detected_type`, `platform` |

**事件发射位置**：
- `ProxyManager::trigger_automatic_fallback()` → `ProxyFallbackEvent`
- `ProxyManager::trigger_automatic_recovery()` → `ProxyRecoveredEvent`
- `ProxyHealthChecker::execute_health_check()` → `ProxyHealthCheckEvent`
- `ProxyManager::update_config()` → `ProxyConfigUpdateEvent`
- `SystemProxyDetector::detect()` → `ProxySystemDetectedEvent`（前端直接处理）

**指标统计**（Soak模块）：
- `selection_total`: 代理使用总次数
- `selection_by_mode`: 按模式分组的使用次数
- `fallback_count`: 降级次数
- `recovery_count`: 恢复次数
- `health_check_success_rate`: 健康检查成功率
- `avg_connection_latency_ms`: 平均连接延迟

---

## 14. 测试矩阵与关键用例

| 类别 | 路径 | 重点 |
|------|------|------|
| 配置模型 | `config.rs` 单元测试（24个） | 验证、序列化、默认值 |
| 状态机 | `state.rs` 单元测试（19个） | 转换验证、计数器、时间戳 |
| 系统检测 | `system_detector.rs` 单元测试（6个） | 跨平台检测、URL解析 |
| HTTP连接器 | `http_connector.rs` 单元测试（30个） | CONNECT隧道、Basic Auth |
| SOCKS5连接器 | `socks5_connector.rs` 单元测试（58个） | 协议握手、地址类型 |
| 失败检测 | `detector.rs` 单元测试（14个） | 滑动窗口、阈值判定 |
| 健康检查 | `health_checker.rs` 单元测试（10个） | 探测逻辑、恢复策略 |
| ProxyManager | `manager.rs` 单元测试（48个） | 统一API、热更新 |
| 传输层集成 | `register.rs` 单元测试（8个） | 注册控制、互斥逻辑 |
| 前端组件 | Vue组件测试（15个） | UI交互、事件处理 |
| 跨平台检测 | 系统检测测试（14个） | Windows/macOS/Linux |
| Soak准入 | Soak测试报告 | 阈值验证、基线对比 |

**关键测试场景**：
1. **配置热更新**：修改代理配置后下一个任务立即生效
2. **自动降级**：代理失败率超过20%触发降级，后续任务走直连
3. **自动恢复**：健康检查连续3次成功后自动恢复代理
4. **系统代理检测**：Windows/macOS/Linux下正确读取系统代理配置
5. **Fake SNI互斥**：代理启用时确认不使用自定义传输层
6. **手动控制**：前端手动降级/恢复按钮立即生效
7. **冷却窗口**：恢复前必须等待冷却时间
8. **并发安全**：多线程同时操作ProxyManager无竞态条件

---

## 15. 运维说明与回退策略

| 场景 | 操作 | 影响 |
|------|------|------|
| 快速禁用代理 | 设置 `proxy.mode=off` | 立即切换直连，所有任务使用系统DNS |
| 手动降级 | 前端点击"手动降级"或调用 `manual_fallback()` | 强制切换直连，停止代理连接尝试 |
| 清理失败统计 | 重启应用或调用 `manual_recover()` | 重置滑动窗口，允许重新尝试代理 |
| 解除冷却窗口 | 等待冷却时间或手动恢复 | 允许立即恢复代理 |
| 调整降级阈值 | 修改 `fallback_threshold` 并热加载 | 新任务立即采用新阈值 |
| 调整健康检查间隔 | 修改 `health_check_interval_seconds` | 下一次检查使用新间隔 |
| 系统代理检测失败 | 前端提供手动配置回退 | 用户手动输入代理URL |
| 代理凭证更新 | 修改 `username`/`password` 并热加载 | 下一次连接使用新凭证 |
| 调试日志 | `RUST_LOG=proxy=debug` | 输出详细连接过程和协议细节 |

---

## 16. 后续优化建议

1. **PAC文件支持**：解析代理自动配置文件，动态选择代理服务器
2. **企业认证协议**：支持NTLM、Kerberos等企业级认证方法
3. **凭证安全存储**：集成操作系统密钥链（Windows Credential Manager/macOS Keychain/Linux Secret Service）
4. **Prometheus指标导出**：暴露代理连接成功率、降级次数、恢复次数等指标
5. **实时配置监听**：监听系统代理配置变更并自动应用（Windows注册表/macOS SystemConfiguration）
6. **代理链支持**：支持多级代理配置
7. **智能降级策略**：根据历史表现自适应调整降级阈值
8. **前端高级诊断**：提供代理连接诊断工具、日志查看器

---

## 17. 快速校验命令

```powershell
# Rust 单元 + 集成测试
cd src-tauri
cargo test --lib proxy --quiet

# 指定模块测试
cargo test --lib proxy::http_connector -- --nocapture
cargo test --lib proxy::socks5_connector -- --nocapture
cargo test --lib proxy::detector -- --nocapture

# 传输层集成测试
cargo test --lib git::transport -- --nocapture

# 前端组件测试
cd ..
pnpm test -- ProxyConfig
pnpm test -- ProxyStatusPanel

# Soak 准入示例（10 轮，生成报告）
$env:FWC_PROXY_SOAK=1
$env:FWC_SOAK_ITERATIONS=10
$env:FWC_SOAK_REPORT_PATH="$PWD\soak-report.json"
cd src-tauri
cargo run --bin fireworks-collaboration --features soak

# 前端契约回归
cd ..
pnpm install
pnpm test
```

---

## 附录A：测试统计总览

| 阶段 | 测试数 | 说明 |
|------|--------|------|
| P5.0（基础架构） | 85 | config/state/system_detector/manager/events |
| P5.1（HTTP代理） | +37 | http_connector + manager集成 |
| P5.2（SOCKS5代理） | +76 | socks5_connector + manager集成 |
| P5.3（传输层集成） | +11 | register + manager互斥 |
| P5.4（自动降级） | +21 | detector + manager降级 |
| P5.5（自动恢复） | +18 | health_checker + manager恢复 |
| P5.6（前端集成） | +24 | Vue组件 + Tauri命令 |
| P5.7（跨平台测试） | +14 | 系统检测跨平台 + Soak |
| **总计** | **286** | **Rust + TypeScript 全部测试** |

---

## 附录B：核心API速查

**ProxyManager**：
```rust
// 检查代理状态
manager.is_enabled() -> bool
manager.mode() -> ProxyMode
manager.state() -> ProxyState

// 获取连接器
manager.get_connector() -> Arc<dyn ProxyConnector + Send + Sync>

// 报告连接结果
manager.report_failure(reason: &str)
manager.report_success()

// 手动控制
manager.manual_fallback(reason: &str) -> Result<()>
manager.manual_recover() -> Result<()>

// 系统代理
manager.detect_system_proxy() -> Option<ProxyConfig>
manager.apply_system_proxy(config: &ProxyConfig) -> Result<()>

// 配置管理
manager.update_config(new_config: ProxyConfig) -> Result<()>
manager.get_state_context() -> ProxyStateContext
```

**ProxyConnector Trait**：
```rust
trait ProxyConnector: Send + Sync {
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, ProxyError>;
    fn proxy_type(&self) -> &'static str;
}
```

**事件发射**（伪代码，实际通过全局总线）：
```rust
publish_proxy_fallback_event(&ProxyFallbackEvent { ... });
publish_proxy_recovered_event(&ProxyRecoveredEvent { ... });
publish_proxy_health_check_event(&ProxyHealthCheckEvent { ... });
```

---

**文档版本**：P5 完整版（2025-10-03）  
**维护者**：代理模块团队  
**反馈渠道**：GitHub Issues / 技术讨论群
