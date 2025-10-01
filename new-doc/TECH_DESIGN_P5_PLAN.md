# P5 阶段技术设计文档 —— 代理支持与自动降级

## 1. 概述

本阶段在 MP0～P4 已完成的 git2-rs 基线、自适应 TLS 传输层（含 Fake SNI、Real-Host 验证、自动禁用）、IP 池与优选（含预热、按需采样、熔断）基础之上，引入"代理支持与自动降级"能力。目标是在不破坏现有传输链与任务契约的前提下，为 HTTP/HTTPS 代理和 SOCKS5 代理提供统一配置接口，实现代理连接失败时的自动降级直连与恢复机制，同时保持与 Fake SNI、IP 优选等既有策略的互斥与协同关系，确保在复杂网络环境下提供稳定的 Git 操作能力。

### 1.1 背景
- 当前传输链依赖直连网络，在企业防火墙或受限环境下无法穿透访问外部 Git 服务（如 GitHub）；
- 已实现的 Fake SNI 与 IP 优选策略在代理场景下可能产生冲突或识别特征，需要明确互斥规则；
- 自适应 TLS 的自动禁用机制为代理失败提供了回退基础，但缺少代理层面的健康检测与恢复；
- 需要支持常见代理协议（HTTP CONNECT、HTTPS、SOCKS5）并提供统一的配置、观测与运维接口。

### 1.2 目标
1. 建立统一的代理配置模型：支持 HTTP/HTTPS 代理与 SOCKS5 代理，提供 URL、认证（可选）、超时等参数；
2. 支持系统代理自动检测：读取操作系统代理设置（Windows/macOS/Linux），允许用户一键应用系统代理配置；
3. 实现代理连接与失败检测：在传输层集成代理逻辑，记录连接成功/失败并触发自动降级；
4. 支持自动降级直连：当代理连接失败达到阈值时，自动切换至直连模式并发出 `proxy://fallback` 事件；
5. 支持自动恢复代理：通过心跳探测或成功率窗口检测代理可用性，在冷却后自动恢复代理模式；
6. 与既有策略协同：代理模式下强制禁用 Fake SNI（使用 Real SNI）并可选禁用自定义传输层（直接使用 libgit2 默认传输），降低复杂度并避免潜在冲突；
7. 保障观测与运维：提供代理状态事件（启用/禁用/降级/恢复）、健康检查日志、配置热更新支持。

### 1.3 范围
- 后端 Rust：实现 `proxy` 模块、系统代理检测器、健康检查器、降级/恢复状态机、与 transport 的集成；
- 配置：扩展 `config.json` 添加 `proxy.mode`（off/http/socks5/system）、`proxy.url`、`proxy.username/password`（可选）、`proxy.disableCustomTransport`（布尔，代理模式下是否禁用自定义传输层）、降级阈值、恢复策略；
- 传输层改造：在 `CustomHttpsSubtransport` 中识别代理配置并选择连接路径（代理/直连），支持完全禁用自定义传输层回退到 libgit2 默认行为，记录失败并触发降级；
- 系统代理检测：实现跨平台系统代理读取（Windows Registry/macOS scutil/Linux 环境变量），提供检测结果供用户选择应用；
- 事件与日志：新增 `proxy://state`（enabled/disabled/fallback/recovered）、`proxy://health_check`、`proxy://system_detected` 事件，结构化日志记录代理连接详情；
- 前端：UI 提供代理配置表单（含系统代理检测与一键应用按钮）、状态显示（当前模式/降级原因/恢复进度）、自定义传输层开关、手动切换按钮；
- 文档与运维：更新配置指南、系统代理兼容性说明、代理故障排查手册、与 Fake SNI/IP 池互斥说明、自定义传输层禁用影响。

### 1.4 不在本阶段
- PAC（代理自动配置）文件解析与执行（仅支持读取静态系统代理配置）；
- 代理链（多级代理）或负载均衡；
- 代理凭证的安全存储（明文配置，后续 P6 凭证管理阶段补充）；
- 企业级代理认证（如 NTLM、Kerberos），仅支持 Basic Auth；
- 代理流量审计或分析功能；
- 系统代理的实时监听与自动更新（仅在用户触发或应用启动时检测）。

### 1.5 成功标准
| 指标 | 目标 | 说明 |
|------|------|------|
| 代理连接成功率 | ≥95% | 配置正确的代理环境下任务成功率 |
| 系统代理检测准确率 | ≥90% | 能正确读取系统代理配置的场景比例 |
| 降级响应时间 | ≤10s | 从代理失败到切换直连的时延 |
| 恢复探测延迟 | ≤60s | 代理恢复后到重新启用的时延 |
| Fake SNI 互斥准确性 | 100% | 代理模式下 Fake SNI 始终禁用 |
| 自定义传输层禁用一致性 | 100% | 启用禁用选项后确实使用 libgit2 默认传输 |
| 事件完整性 | 100% | 降级/恢复事件正确发射并包含原因 |
| 配置热更新响应 | <5s | 修改代理配置后新任务立即生效 |

### 1.6 验收条件
1. 配置代理后可成功完成 clone/fetch/push，日志显示通过代理连接；
2. 系统代理检测功能在 Windows/macOS/Linux 下能正确读取代理配置，前端可一键应用；
3. 启用 `proxy.disableCustomTransport=true` 后，任务使用 libgit2 默认传输（日志中无自定义 subtransport 注册记录）；
4. 模拟代理不可达时触发自动降级，后续任务走直连，`proxy://fallback` 事件发出；
5. 代理恢复后心跳探测成功，自动重新启用代理，`proxy://recovered` 事件发出；
6. 代理模式下确认 Fake SNI 被禁用（日志/事件中 `used_fake_sni=false`），若禁用自定义传输层则无相关事件；
7. 所有新增单元/集成测试通过，现有回归测试无失败；
8. 文档、配置样例、运维手册更新完毕，包含系统代理检测使用方法、自定义传输层禁用影响、常见代理故障诊断步骤。

### 1.7 交付物
- 代码：`core/proxy` 模块（含系统代理检测器）、健康检查器、降级控制器、transport 集成改造（含自定义传输层禁用逻辑）；
- 配置：更新 `config.json` 添加 `proxy` 顶层字段（mode[off/http/socks5/system]/url/username/password/disableCustomTransport/降级阈值/恢复策略）；
- 事件：新增 `proxy://state`、`proxy://health_check`、`proxy://system_detected` 事件及其字段定义；
- 前端：代理配置 UI（含系统代理检测与应用按钮、自定义传输层开关）、状态面板、手动切换按钮；
- 测试：单元测试、集成测试（模拟代理服务器、系统代理配置）、故障注入场景、跨平台系统代理检测测试；
- 文档：P5 设计文档、代理配置指南（含系统代理使用）、自定义传输层禁用影响说明、故障排查手册、与 Fake SNI/IP 池互斥说明。

### 1.8 回退策略
| 场景 | 操作 | 影响 |
|------|------|------|
| 代理整体异常 | 设置 `proxy.mode=off` 或手动触发降级 | 立即切换直连，不影响任务成功率 |
| 代理配置错误 | 校验失败时拒绝启用并告警 | 保持直连模式，前端提示配置问题 |
| 降级逻辑误判 | 调整降级阈值或禁用自动降级 | 强制代理模式，由运维手动介入 |
| 恢复探测干扰 | 禁用自动恢复或延长探测间隔 | 保持降级状态，手动恢复代理 |
| 观测噪声过大 | 降低事件等级或关闭健康检查日志 | 不影响核心流程 |

### 1.9 依赖与前置条件

#### 1.9.1 软件依赖
- libgit2 ≥ 1.6（支持自定义 HTTP 传输和代理配置）
- git2-rs（Rust 绑定）
- tokio（异步运行时，用于健康检查）
- Windows 平台：`winreg` crate（读取注册表）

#### 1.9.2 前置阶段
- P3：IP 优选框架（为降级提供备用连接方式）
- P4：QUIC 传输层（提供额外的传输协议选项）

#### 1.9.3 配置基础
- 已有 `AppConfig` 结构，需新增 `proxy` 子配置

#### 1.9.4 环境假设
- 代理服务器支持标准 HTTP CONNECT 或 SOCKS5 协议，响应符合 RFC 规范；
- 代理认证（如需）使用 Basic Auth，凭证通过配置明文提供（P6 前临时方案）；
- 网络环境允许在后台执行代理健康检查（不会被防火墙或策略阻断）；
- 现有自适应 TLS transport 可通过配置开关禁用 Fake SNI（已在 P3 实现）；
- 降级/恢复状态为进程级，跨进程需通过配置文件同步；
- 前端能够展示代理状态并提供手动控制（基于事件订阅）。

#### 1.9.5 现有代码基础与对接说明

**现有功能**（位于 `src-tauri/src/core/tls/util.rs`）：

1. **`proxy_present() -> bool`**
   - 功能：检测环境变量代理（`HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`）
   - 对接方式：`SystemProxyDetector` 在 Linux 平台复用此函数，在 Windows/macOS 扩展注册表和 scutil 检测
   - 调用位置：`ProxyManager::detect_system_proxy()` 先调用 `proxy_present()`，若返回 true 则解析环境变量返回配置

2. **`decide_sni_host_with_proxy(cfg, force_real, real_host, proxy_present) -> (String, bool)`**
   - 功能：根据代理存在情况决定 SNI 主机名，代理存在时强制真实 SNI
   - 对接方式：在 `CustomHttpsSubtransport::connect_tls` 中调用，传入 `ProxyManager::is_enabled()` 作为 `proxy_present` 参数
   - 行为保持：代理启用时返回 `(real_host, false)`，确保 `used_fake_sni=false`

3. **`get_last_good_sni(real_host) / set_last_good_sni(real_host, sni)`**
   - 功能：记录最近成功的伪 SNI，用于优先选择
   - 影响：代理启用时不调用这些函数（因为强制真实 SNI），直连模式正常使用

**对接要点**：

- P5.0：`SystemProxyDetector` 新增 Windows/macOS 特定检测，但 Linux 继续调用 `proxy_present()`
- P5.3：在传输层注册前调用 `tls::util::proxy_present()` 或 `ProxyManager::is_enabled()` 判断是否跳过自定义传输层
- 测试：验证 `decide_sni_host_with_proxy` 在 `proxy_present=true` 时始终返回真实 SNI

**强制互斥策略实现**：

代理启用时**强制禁用**自定义传输层与 Fake SNI，不提供用户选择。

1. **配置加载时**（`core/config/loader.rs` 或 `app.rs`）：
   ```rust
   if proxy_config.mode != ProxyMode::Off {
       proxy_config.disable_custom_transport = true; // 强制设置
       tracing::info!("Proxy enabled, force disable custom transport and Fake SNI");
   }
   ```

2. **传输层注册时**（`core/git/transport/mod.rs`）：
   ```rust
   pub fn ensure_registered() -> Result<()> {
       if should_skip_custom_transport() {
           tracing::info!("Custom transport disabled, using libgit2 default HTTP");
           return Ok(());
       }
       // 注册 https+custom subtransport...
   }
   
   fn should_skip_custom_transport() -> bool {
       tls::util::proxy_present() || /* 读取配置判断 */
   }
   ```

3. **libgit2 代理配置**（P5.3）：
   ```rust
   if proxy_manager.is_enabled() {
       let proxy_url = proxy_manager.get_proxy_url()?;
       repo_config.set_str("http.proxy", &proxy_url)?;
   }
   ```

**系统代理检测的跨平台策略**：

| 平台 | 检测方法 | 现有基础 | P5 扩展 |
|------|----------|----------|---------|
| Linux | 环境变量 | `proxy_present()` 已实现 | 解析环境变量提取 URL 和类型 |
| Windows | 注册表 | 无 | 读取 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` |
| macOS | scutil | 无 | 执行 `scutil --proxy` 并解析输出 |

**配置字段映射**：

| 配置字段 | 类型 | 默认值 | 代理启用时行为 |
|----------|------|--------|----------------|
| `proxy.mode` | enum | `off` | 设为 `http`/`socks5`/`system` 时启用代理 |
| `proxy.url` | string | `""` | 代理服务器地址 |
| `proxy.disableCustomTransport` | bool | `false` | **代理启用时强制设为 `true`** |
| `http.fake_sni_enabled` | bool | - | 代理启用时被 `decide_sni_host_with_proxy` 忽略 |

**测试覆盖要求**：

1. 单元测试：
   - `tls::util::decide_sni_host_with_proxy` 在 `proxy_present=true` 时返回真实 SNI（已有测试，验证不破坏）
   - `SystemProxyDetector` 各平台检测逻辑（模拟环境变量/注册表/scutil 输出）
   - 配置加载时的强制互斥逻辑（代理启用 → `disable_custom_transport=true`）

2. 集成测试：
   - 启用代理后确认不注册 `https+custom` subtransport
   - 代理模式下任务通过 libgit2 默认 HTTP 成功完成
   - 禁用代理后自定义传输层恢复正常

3. 回归测试：
   - 确保 P3/P4 的 Fake SNI 和 IP 优选测试在直连模式下仍通过
   - 代理模式不影响基础 clone/fetch/push 功能



### 1.10 风险概览
| 风险 | 等级 | 描述 | 缓解 |
|------|------|------|------|
| 代理凭证泄漏 | 高 | 明文配置或日志输出 | 默认脱敏，P6 引入安全存储 |
| 系统代理检测失败 | 中 | 不同操作系统/配置方式导致读取失败 | 提供手动配置回退，记录检测失败日志 |
| 自定义传输层禁用副作用 | 中 | 失去 Fake SNI、IP 优选等增强能力 | 文档明确说明影响，提供开关灵活控制 |
| 降级误触发 | 中 | 瞬时网络抖动导致频繁切换 | 滑动窗口 + 失败率阈值 |
| 恢复探测失败 | 中 | 代理恢复后未检测到，长时间直连 | 多重探测策略 + 手动恢复接口 |
| 代理协议不兼容 | 中 | 企业代理使用非标准实现 | 提供回退选项 + 日志诊断 |
| Fake SNI 互斥失效 | 高 | 配置冲突导致代理+Fake SNI 同时启用 | 启动时校验 + 运行时强制互斥 |
| 性能开销 | 低 | 代理连接延迟增加 | 记录 timing，可选关闭健康检查 |
| 并发状态竞争 | 中 | 降级/恢复状态切换时的并发任务处理 | 原子状态 + 事务性切换 |

### 1.11 兼容与迁移
| 旧版本行为 | P5 调整 | 保证措施 |
|--------------|-----------|-----------|
| 所有任务使用直连 | 引入可选代理模式 | 默认 `proxy.mode=off`，向后兼容 |
| 无代理配置字段 | 新增 `proxy` 顶层字段 | 缺省填默认值，不破坏现有配置 |
| Fake SNI 与直连共存 | 代理模式强制禁用 Fake SNI | 启动时校验，冲突时告警并回退 |
| 无降级/恢复机制 | 新增自动降级与心跳恢复 | 可通过配置禁用，保持手动控制 |
| transport 不识别代理 | 扩展连接逻辑支持代理 | 直连路径保持不变，代理为独立分支 |

## 2. 详细路线图

### 子阶段划分
| 阶段 | 主题 | 核心关键词 |
|------|------|------------|
| P5.0 | 基线架构与配置模型 | 模块化 proxy / 配置解析 / 状态枚举 / 系统代理检测器 |
| P5.1 | HTTP/HTTPS 代理支持 | CONNECT 隧道 / 认证 / 超时控制 |
| P5.2 | SOCKS5 代理支持 | SOCKS5 握手 / 认证 / 统一接口 |
| P5.3 | 传输层集成与互斥控制 | Transport 改造 / Fake SNI 互斥 / 连接路由 / 自定义传输层禁用 |
| P5.4 | 自动降级与失败检测 | 失败阈值 / 滑动窗口 / 降级事件 |
| P5.5 | 自动恢复与心跳探测 | 健康检查 / 恢复策略 / 冷却窗口 |
| P5.6 | 观测、事件与前端集成 | 代理状态事件 / 系统代理应用 / UI 面板 / 手动控制 |
| P5.7 | 稳定性验证与准入 | 故障注入 / Soak 测试 / 跨平台测试 / 准入报告 |

### P5.0 基线架构与配置模型
- **目标**：建立独立的 `proxy` 基础模块，完成配置解析、状态枚举、系统代理检测器和测试支撑，使后续子阶段在不影响现有传输链的情况下增量接入代理逻辑。
- **范围**：
	- 新建 `core/proxy/{mod.rs,config.rs,state.rs,system_detector.rs}`，定义 `ProxyConfig`、`ProxyMode`（Off/Http/Socks5/System）、`ProxyState`（Enabled/Disabled/Fallback/Recovering）等核心数据结构；
	- 实现 `SystemProxyDetector`，支持跨平台系统代理检测：
		- Windows: 读取注册表 `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Internet Settings`（ProxyEnable/ProxyServer）
		- macOS: 执行 `scutil --proxy` 解析输出
		- Linux: 读取环境变量 `http_proxy`/`https_proxy`/`all_proxy`
		- **复用现有代码**：`tls::util::proxy_present()` 已实现环境变量检测，`SystemProxyDetector` 在此基础上扩展 Windows 注册表和 macOS scutil 支持，统一返回 `Option<ProxyConfig>`
	- 加载 `config.json` 中与代理相关的新字段（`proxy.mode`、`proxy.url`、`proxy.username`、`proxy.password`、`proxy.disableCustomTransport`、降级阈值、恢复策略），支持热加载；
	- 设计统一的代理连接器接口（trait `ProxyConnector`），当前返回占位结果并回退直连；
	- 预留与 transport 的集成点（如 `ProxyManager::connect` 方法、`should_disable_custom_transport` 方法），确保直连路径不受影响。
- **交付物**：
	- 模块骨架与单元测试（配置默认值、枚举序列化、状态迁移逻辑、系统代理检测）；
	- `SystemProxyDetector` 实现与跨平台测试（Windows/macOS/Linux 模拟场景）；
	- 新增配置示例（含 `disableCustomTransport` 字段说明）与文档；
	- `ProxyState` 状态机图与迁移规则文档。
- **依赖**：复用 P3/P4 的配置加载/热更新机制；Windows 需 `winreg` crate，macOS/Linux 需标准库支持；无外部服务依赖。
- **验收**：
	- 后端可在无代理实现的情况下顺利编译、运行；
	- 新配置项缺省值不破坏现有任务（默认 `mode=off, disableCustomTransport=false`）；
	- 系统代理检测在 Windows/macOS/Linux 下能正确读取或返回 None（无代理配置时）；
	- 单元测试覆盖配置解析、状态枚举、占位连接器、系统代理检测各平台路径。
- **风险与缓解**：
	- 配置兼容风险 → 提供默认值并在日志中提示新字段启用状态；
	- 模块侵入度 → 通过 trait 接口与现有 transport 解耦，仅在 P5.3 进行实际集成；
	- 系统代理检测跨平台差异 → 提供统一接口，平台特定逻辑封装在 `system_detector.rs`，测试覆盖各平台。

### P5.1 HTTP/HTTPS 代理支持
- **目标**：实现 HTTP CONNECT 隧道协议，支持通过 HTTP/HTTPS 代理建立到目标主机的 TCP 连接，完成认证（Basic Auth）与超时控制。
- **范围**：
	- 实现 `HttpProxyConnector`，支持 CONNECT 方法建立隧道；
	- 解析代理 URL，提取 host/port，支持 `http://` 与 `https://` scheme；
	- 支持 Basic Auth（username/password 通过配置提供），在 CONNECT 请求中携带 `Proxy-Authorization` 头；
	- 处理代理响应（200 Connection Established / 407 Proxy Authentication Required / 其他错误），映射到错误分类（Proxy/Auth/Network）；
	- 集成超时控制（连接超时、握手超时），支持可配置的超时参数；
	- 记录代理连接成功/失败日志，为后续降级检测提供数据。
- **交付物**：
	- `HttpProxyConnector` 实现与单元测试（成功连接、认证、超时、错误响应）；
	- 错误映射逻辑与测试（407→Auth、连接超时→Network）；
	- 代理连接日志（debug 级别，包含代理 URL、目标 host/port、耗时）。
- **依赖**：依赖 P5.0 的配置与 trait 定义；需要 tokio TCP/TLS 支持。
- **验收**：
	- 配置 HTTP 代理后可成功建立 CONNECT 隧道，后续 TLS 握手成功；
	- 认证失败时返回 Auth 错误，日志包含原因；
	- 超时场景触发 Network 错误并不阻塞任务；
	- 单元测试覆盖正常路径与 3 个失败场景。
- **风险与缓解**：
	- 代理凭证日志泄漏 → 默认脱敏，仅在 `debugAuthLogging=true` 时输出完整凭证；
	- CONNECT 响应解析失败 → 严格校验状态行，非 200 视为代理错误；
	- 代理不支持 HTTPS → 提供回退直连选项，文档说明兼容性。

### P5.2 SOCKS5 代理支持
- **目标**：实现 SOCKS5 协议握手与认证，支持通过 SOCKS5 代理建立 TCP 连接，与 HTTP 代理共享统一接口。
- **范围**：
	- 实现 `Socks5ProxyConnector`，支持 SOCKS5 握手流程（版本协商、认证、连接请求）；
	- 支持 No Auth（0x00）与 Username/Password Auth（0x02）两种认证方法；
	- 处理 SOCKS5 响应（成功 0x00 / 失败 0x01-0x08），映射到错误分类（Proxy/Auth/Network）；
	- 支持 IPv4/IPv6 与域名解析（ATYP=0x01/0x03/0x04）；
	- 集成超时控制，与 HTTP 代理保持一致；
	- 统一 `ProxyConnector` trait 实现，确保传输层无需区分代理类型。
- **交付物**：
	- `Socks5ProxyConnector` 实现与单元测试（握手、认证、连接、错误响应）；
	- 与 `HttpProxyConnector` 共享的错误映射与日志格式；
	- SOCKS5 协议握手日志（debug 级别）。
- **依赖**：依赖 P5.0 的 trait 定义与 P5.1 的超时控制逻辑。
- **验收**：
	- 配置 SOCKS5 代理后可成功建立连接，后续 TLS 握手成功；
	- 认证失败时返回 Auth 错误；
	- 不支持的认证方法（如 GSSAPI）返回 Proxy 错误并回退；
	- 单元测试覆盖正常路径与 3 个失败场景。
- **风险与缓解**：
	- SOCKS5 版本不兼容 → 严格检查版本号（0x05），非标准版本拒绝并告警；
	- 域名解析失败 → 支持 ATYP=0x03 让代理解析，避免客户端 DNS 依赖；
	- 认证方法协商失败 → 记录支持的方法列表，提示配置问题。

### P5.3 传输层集成与互斥控制
- **目标**：在不破坏自适应 TLS 既有回退链的前提下，将代理连接逻辑注入传输层，实现代理/直连路由决策，强制执行 Fake SNI 互斥规则，并支持可选禁用自定义传输层降低复杂度。
- **范围**：
	- 修改 `CustomHttpsSubtransport`，在 `connect_tcp` 前检查代理配置并选择连接路径（代理/直连）；
	- **强制互斥策略（核心）**：
		- 当 `proxy.mode != off` 或检测到系统代理时，**强制**设置 `proxy.disableCustomTransport = true`
		- 代理启用时**同时禁用**自定义传输层与 Fake SNI，直接使用 libgit2 默认 HTTP 传输
		- **复用现有逻辑**：`tls::util::decide_sni_host_with_proxy()` 已支持 `proxy_present` 参数强制真实 SNI，P5 将通过 `ProxyManager::is_enabled()` 调用该参数
		- 降低复杂度，避免代理与 Fake SNI/IP 优选/自适应 TLS 的潜在冲突与识别特征
	- 在传输层注册阶段（`transport::ensure_registered`）检查代理配置：
		- 若 `proxy.disableCustomTransport = true`（包括因代理强制设置），则跳过 `git2::transport_register("https+custom", ...)`
		- 直接使用 libgit2 内置 HTTP 传输，通过 `git2::Config` 设置代理（`http.proxy`）
	- 在代理连接失败时调用 `ProxyManager::report_failure`，为降级检测提供数据；
	- 保持 IP 池在直连模式下的正常工作，代理模式下完全跳过 IP 优选与自定义传输层；
	- 扩展 timing 事件携带 `proxy_type`、`proxy_latency_ms`、`custom_transport_disabled` 可选字段；
	- 与 Retry 机制对齐：代理连接失败触发一次直连重试（若配置允许回退），成功后记录降级候选。
- **交付物**：
	- 传输层改造代码、路由决策逻辑与单元测试（代理成功、代理失败回退直连、自定义传输层禁用）；
	- **代理与自定义传输层互斥逻辑**：
		- `ProxyManager::should_disable_custom_transport()` 方法，当代理启用时返回 true
		- 在 `app.rs` 启动时检查互斥并设置强制禁用标志
		- 单元测试验证代理启用时 `custom_transport_disabled` 自动为 true
	- Fake SNI 互斥校验与测试（代理模式下确认 Fake SNI 被禁用，复用 `tls::util::decide_sni_host_with_proxy` 现有逻辑）；
	- 自定义传输层禁用逻辑与测试（启用后确认不注册 subtransport、使用 libgit2 默认行为，通过 `git2::Config::set_str("http.proxy", ...)` 传递代理配置）；
	- 事件/日志扩展：`used_proxy`、`proxy_type`、`proxy_latency_ms`、`custom_transport_disabled` 字段；
	- 配置开关 `proxy.mode`（Off/Http/Socks5/System）、`proxy.disableCustomTransport`（布尔，代理启用时自动设为 true），支持即时切换。
- **依赖**：依赖 P5.1/P5.2 的代理连接器实现；需要与 P3 的 timing 事件与 P4 的 IP 池协同。
- **验收**：
	- 启用代理时任务日志显示 `used_proxy=true`，Fake SNI 未启用；
	- **强制互斥验证**：配置 `proxy.mode=http` 后，即使用户未手动设置 `disableCustomTransport`，系统也自动设为 true，日志显示 `custom_transport_disabled=true`；
	- 启用代理时，`tls::util::decide_sni_host_with_proxy` 的 `proxy_present` 参数为 true，返回真实 SNI；
	- 禁用自定义传输层后，日志中无 `transport::ensure_registered` 的 `https+custom` 注册记录，任务通过 libgit2 默认 HTTP 传输成功完成；
	- 禁用代理后恢复直连与 IP 优选，事件中 `used_proxy=false, custom_transport_disabled=false`；
	- 代理连接失败时自动尝试直连（若配置允许回退），任务成功率不下降；
	- Retry 触发次数与 P3 基线一致，无额外重复尝试。
- **风险与缓解**：
	- 路由决策逻辑复杂 → 提取独立函数并覆盖全路径测试；
	- 互斥规则失效 → 启动时校验配置冲突（代理启用时强制 `disableCustomTransport=true`），运行时强制互斥并告警；
	- 自定义传输层禁用后失去增强能力 → **代理启用时强制禁用**，文档明确说明影响（无 Fake SNI、IP 优选、自适应 TLS），这是设计选择以降低复杂度和指纹风险；
	- libgit2 默认传输代理配置失败 → 通过 `git2::Config` 设置 `http.proxy`，测试验证配置生效；
	- 事件暴露代理信息 → 仅输出代理类型（http/socks5），URL 仅写 debug 日志。

### P5.4 自动降级与失败检测
- **目标**：当代理连接失败达到阈值时，自动切换至直连模式，发出 `proxy://fallback` 事件，并记录降级原因与时间。
- **范围**：
	- 引入 `ProxyFailureDetector`，使用滑动窗口统计代理连接失败率（如 5 分钟内失败率 >20%）；
	- 实现自动降级状态机：`Enabled` → `Fallback`，触发条件包括连续失败、失败率阈值、超时累计；
	- 发射 `proxy://fallback` 事件（包含原因、失败次数、窗口时长）；
	- 在降级状态下，所有新任务直接走直连路径，不再尝试代理；
	- 支持手动强制降级接口（运维介入）；
	- 记录降级时间，为后续恢复冷却提供基准。
- **交付物**：
	- `ProxyFailureDetector` 实现与单元测试（滑动窗口、阈值判定、状态迁移）；
	- 降级状态机与测试（触发条件、事件发射、手动降级）；
	- 新增配置 `proxy.fallbackThreshold`、`proxy.fallbackWindowSeconds`；
	- 事件定义：`proxy://fallback { reason, failure_count, window_seconds, fallback_at }`。
- **依赖**：依赖 P5.3 的 `report_failure` 调用；需要与 P4 的熔断机制区分（代理降级为传输层级，IP 熔断为候选级）。
- **验收**：
	- 模拟代理不可达时触发降级，后续任务走直连，事件记录原因；
	- 降级后直连任务成功率保持与无代理时一致；
	- 手动降级接口生效，立即切换模式；
	- 单元测试覆盖滑动窗口边界、阈值触发与手动降级。
- **风险与缓解**：
	- 阈值误设导致频繁降级 → 默认阈值保守（如 20% / 5 分钟）并提供运维监控；
	- 降级与恢复同时触发 → 使用原子状态切换，避免竞态；
	- 事件噪声过大 → 降级事件仅在状态变化时发射一次。

### P5.5 自动恢复与心跳探测
- **目标**：通过心跳探测检测代理恢复可用性，在冷却后自动重新启用代理模式，发出 `proxy://recovered` 事件。
- **范围**：
	- 实现 `ProxyHealthChecker`，定期（如 60 秒）向代理发送轻量探测请求（HEAD 或 CONNECT 到已知可达主机）；
	- 支持多种探测策略：单次成功恢复、连续 N 次成功恢复、成功率窗口恢复；
	- 实现恢复状态机：`Fallback` → `Recovering` → `Enabled`，冷却窗口（如 5 分钟）内不立即恢复；
	- 发射 `proxy://health_check` 事件（探测结果、延迟、成功率）与 `proxy://recovered` 事件；
	- 支持手动强制恢复接口（运维介入）；
	- 在恢复后重置失败统计，避免历史失败影响后续判定。
- **交付物**：
	- `ProxyHealthChecker` 实现与单元测试（探测逻辑、成功/失败处理、冷却窗口）；
	- 恢复状态机与测试（触发条件、事件发射、手动恢复）；
	- 新增配置 `proxy.recoveryStrategy`（single/consecutive/rate）、`proxy.recoveryCooldownSeconds`、`proxy.healthCheckIntervalSeconds`；
	- 事件定义：`proxy://health_check { success, latency_ms, probe_url }`、`proxy://recovered { recovered_at, cooldown_seconds }`。
- **依赖**：依赖 P5.4 的降级状态；需要后台任务调度器（如 tokio interval）。
- **验收**：
	- 降级后心跳探测定期执行，日志显示探测结果；
	- 代理恢复后自动重新启用，后续任务走代理，事件记录恢复时间；
	- 冷却窗口内不立即恢复，避免频繁切换；
	- 手动恢复接口生效，跳过冷却立即启用；
	- 单元测试覆盖探测策略、冷却窗口与手动恢复。
- **风险与缓解**：
	- 探测干扰正常流量 → 限制探测频率（默认 60 秒）并使用轻量请求；
	- 探测失败误判 → 使用连续成功或成功率窗口策略，提高鲁棒性；
	- 冷却窗口过长 → 提供配置调整，默认 5 分钟平衡稳定性与响应速度。

### P5.6 观测、事件与前端集成
- **目标**：完善代理运行期的可观测性，提供前端 UI 展示代理状态、系统代理检测结果、降级原因、恢复进度，并支持手动控制与一键应用系统代理。
- **范围**：
	- 扩展 Strategy 事件，新增 `proxy://state`（当前状态、模式、降级原因、恢复进度）、`proxy://fallback`、`proxy://recovered`、`proxy://health_check`、`proxy://system_detected` 事件；
	- 定义事件字段：`proxy_mode`（off/http/socks5/system）、`proxy_state`（enabled/disabled/fallback/recovering）、`fallback_reason`、`failure_count`、`health_check_success_rate`、`next_health_check_at`、`system_proxy_url`（系统检测结果）、`custom_transport_disabled`；
	- 前端 UI：
		- 代理配置表单（mode 选择器含 System 选项/url/username/password/disableCustomTransport 复选框）
		- 系统代理检测区域（检测按钮、检测结果显示、一键应用按钮）
		- 状态面板（当前模式、降级原因、恢复倒计时、自定义传输层状态）
		- 手动控制按钮（强制降级/恢复）
	- 提供调试日志（等级 debug）输出代理连接详情（URL、认证状态、耗时、自定义传输层状态），仅在 `debugProxyLogging=true` 时开启；
	- 更新 Soak 模块统计代理事件（降级次数、恢复次数、平均探测延迟、系统代理检测成功率）。
- **交付物**：
	- 事件定义与实现（序列化/反序列化、字段验证，含 `proxy://system_detected` 事件）；
	- 前端 UI 组件（ProxyConfig.vue 含系统代理检测区域、ProxyStatusPanel.vue）与 store 更新（新增 `detectSystemProxy` action）；
	- 后端命令：`detect_system_proxy(): Promise<{ url?: string; type?: 'http'|'socks5' }>` 供前端调用；
	- 更新运维文档与配置样例（代理配置含 System 模式说明、系统代理检测使用方法、自定义传输层禁用影响、故障排查步骤）；
	- Soak 报告扩展（proxy 摘要字段：fallback_count、recovered_count、avg_health_check_latency_ms、system_proxy_detect_success_rate）。
- **依赖**：依赖 P5.4/P5.5 的事件发射；需要前端事件订阅机制与 Pinia store。
- **验收**：
	- 前端系统代理检测按钮点击后能显示检测结果（URL/类型或"未检测到"），一键应用后配置自动填充；
	- 前端显示代理状态（启用/降级/恢复中）、自定义传输层状态，降级原因与失败次数可见；
	- 手动控制按钮生效，立即切换模式并发出事件；
	- Soak 报告包含代理摘要字段（含系统代理检测成功率），JSON 序列化正常；
	- Debug 日志包含代理连接详情与自定义传输层状态，默认脱敏（不输出完整 URL/凭证）。
- **风险与缓解**：
	- 观测噪声过大 → 健康检查事件默认仅在状态变化时发射，提供采样率配置；
	- 前端 UI 复杂度 → 分阶段实现，P5.6 仅提供基础状态展示与系统代理检测，高级诊断面板延后；
	- 系统代理检测跨平台差异 → 前端提供友好错误提示，文档说明各平台兼容性；
	- 凭证泄漏 → 前端不展示完整凭证，仅显示配置状态（已配置/未配置）。

### P5.7 稳定性验证与准入
- **目标**：通过 soak、故障注入与准入评审验证代理链路在长时间运行下的稳定性与降级/恢复机制的有效性，为生产灰度提供可执行的准入结论。
- **范围**：
	- 扩展 `soak/` 脚本模拟代理场景（成功连接、代理失败、降级、恢复），收集降级/恢复次数、探测延迟、任务成功率；
	- 设计覆盖异常场景的故障注入脚本（代理不可达、认证失败、超时、频繁切换），确保降级/恢复按预期触发；
	- 定义准入阈值（代理成功率、降级响应时间、恢复探测延迟、Fake SNI 互斥准确性）并编写自动化报告；
	- 与运维协作制定灰度计划、监控看板与手动回滚手册，明确启用/禁用流程；
	- 汇总测试数据，形成最终 P5 阶段 readiness review 结论，输出到技术设计与运维文档。
- **交付物**：
	- 更新后的 soak 脚本与 CI 配置、带有代理指标的测试报告模板；
	- 故障注入与准入 checklist 文档（触发步骤与预期结果）；
	- readiness review 会议纪要与上线建议（包含灰度范围、监控项、回滚条件）。
- **依赖**：依赖 P5.1～P5.6 功能完整并在测试环境可用；需要 CI 环境与 soak 集群具备代理服务器（或 mock）；准入评审需协调运维与安全团队时间窗口。
- **验收**：
	- 连续 soak >=24 小时期间，代理路径任务成功率符合阈值（≥95%）；
	- 故障注入场景均触发降级/恢复并在日志、指标中可追踪；
	- 准入报告明确给出上线/灰度建议与需要关注的风险项，获得相关团队签字确认；
	- 灰度开关演练通过（启用、禁用、降级、恢复全程 <5 分钟，日志完整）。
- **风险与缓解**：
	- Soak 环境缺少真实代理 → 使用 mock 代理服务器（如 squid、dante）模拟；
	- 准入阈值过严导致迟迟不能上线 → 分阶段设定基线/目标值，并在评审中讨论调整；
	- 协同团队时间冲突 → 提前预约评审窗口，准备异步报告供审阅。

## 3. 实现说明

以下章节预留给后续交付后的实现复盘，结构对齐 P3/P4 文档。每个子阶段完成后请在对应小节补充：
- 关键代码路径与文件列表；
- 实际交付与设计差异；
- 验收/测试情况与残留风险；
- 运维手册或配置样例的落地状态。

### P5.0 基线架构与配置模型 实现说明
（待实现后补充）

### P5.1 HTTP/HTTPS 代理支持 实现说明
（待实现后补充）

### P5.2 SOCKS5 代理支持 实现说明
（待实现后补充）

### P5.3 传输层集成与互斥控制 实现说明
（待实现后补充）

### P5.4 自动降级与失败检测 实现说明
（待实现后补充）

### P5.5 自动恢复与心跳探测 实现说明
（待实现后补充）

### P5.6 观测、事件与前端集成 实现说明
（待实现后补充）

### P5.7 稳定性验证与准入 实现说明
（待实现后补充）
