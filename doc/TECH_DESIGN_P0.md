# P0 阶段细化版行动指南与 Roadmap

> 目标：在最短可控周期内交付“可用、可测、安全基线可接受”的首个技术增量：
> 1) 通用伪 SNI HTTP 请求 API（含 SAN 白名单 + 可配置伪 SNI + 日志脱敏）
> 2) 基础 Git Clone（基于 gitoxide）
> 3) 任务模型（注册 / 状态 / 事件 / 取消）及前端展示
> 4) TLS 自定义验证（SAN 白名单）
> 5) 最小配置体系（静态加载 + 写回）
> 6) 基础日志与错误分类（初级版）

---

## 0. 总览节拍（建议 2–3 周内完成）

| 子阶段 | 名称 | 主要交付 | 预估 | 依赖 |
|--------|------|----------|------|------|
| P0.1 | 基础环境与骨架 | 目录、依赖、事件通道、配置加载 | 0.5~1d | 无 |
| P0.2 | 任务基础设施 | TaskRegistry + 事件发射 + 取消 Token | 1d | P0.1 |
| P0.3 | TLS & 验证器 | rustls 集成 + SAN 白名单 + 伪 SNI 选项判定 | 1d | P0.1 |
| P0.4 | HTTP 客户端核心 | hyper/rustls 统一封装 + timing 采集 | 1.5d | P0.3 |
| P0.5 | http_fake_request API | Tauri command + 日志脱敏 + 大响应 WARN | 1d | P0.4 |
| P0.6 | Git Clone 基础 | gitoxide 集成 + 任务事件（progress/state） | 1.5d | P0.2 |
| P0.7 | 前端面板与可视化 | HTTP Tester + Git Clone 面板 + 任务列表 | 2d | P0.2 / P0.5 / P0.6 |
| P0.8 | 测试与校验 | 单元 + 集成 + 手动脚本 + 基线性能 | 1.5d | 前全部 |
| P0.9 | 文档与初发布 | README / 开发指南 / 变更日志 | 0.5d | P0.8 |

*如资源紧张，可将 P0.7 拆成：先任务列表 + HTTP Tester，再补 Git Panel*

---

## 1. 范围边界（只做 / 不做）

| 只做 | 说明 |
|------|------|
| HTTP 单次完整请求（内存缓冲 body） | 不实现流式 |
| Fake SNI 手动开关 + 静态伪域 | 不做动态策略 |
| SAN 白名单静态配置 | 不支持运行时热更新 |
| Git Clone 单 Remote（HTTPS） | 不做 shallow / partial / retry 优化 |
| 任务取消协作式 | 不做强制硬中断 |
| 配置启动加载 + 手动保存覆盖 | 不做文件监测热更新 |
| 日志脱敏（Authorization） | 仅简单替换 **REDACTED** |
| 错误分类基础版 | 不做细粒度错误码体系 |

| 不做 | 理由 |
|------|------|
| IP 优选 / 代理 / Push / Fetch | 后续阶段 |
| SPKI Pin / 指纹事件 | 属于 P7 |
| 指标系统 (metrics) | P9 |
| LFS / Streaming / HTTP/2 | 后续演进 |
| 任务策略覆盖 | P2+ |
| 大文件 body 流式处理 | P10 预研 |

---

## 2. 代码结构（首批落地最小骨架）

```
src-tauri/
  src/
    main.rs
    events/
      emitter.rs
    api/
      http_fake_api.rs
      git_api.rs
      task_api.rs
      config_api.rs
    core/
      tasks/{mod.rs, registry.rs, model.rs}
      http/{mod.rs, client.rs, types.rs}
      tls/{mod.rs, verifier.rs}
      git/{mod.rs, clone.rs, progress.rs}
      config/{mod.rs, model.rs, loader.rs}
      util/{base64.rs, timing.rs, redact.rs, error.rs}
    logging.rs
config/
  config.json (初始模板)
src/
  api/tauri.ts
  api/http.ts
  api/git.ts
  api/tasks.ts
  stores/tasks.ts
  stores/logs.ts
  views/HttpTester.vue
  views/GitPanel.vue
  components/TaskList.vue
```

---

## 3. 配置文件（P0 最小字段）

配置文件示例（仅当前需要）：
```json
{
  "http": {
    "fakeSniEnabled": true,
  "fakeSniHosts": ["baidu.com", "qq.com"],
    "followRedirects": true,
    "maxRedirects": 5,
    "largeBodyWarnBytes": 5242880
  },
  "tls": {
    "spkiPins": [],
    "metricsEnabled": false,
    "certFpLogEnabled": false,
    "certFpMaxBytes": 4096
  },
  "logging": {
    "authHeaderMasked": true,
    "logLevel": "info"
  }
}
```

> ⚠️ v1.8 起 `tls.insecureSkipVerify` / `tls.skipSanWhitelist` 被永久移除。当前实现始终通过 `RealHostCertVerifier` 以真实域名校验证书，Fake SNI 仅在与 `ip_pool::preheat::BUILTIN_IPS` 同步的内置名单命中时改写 ClientHello。

存储位置（P0.1 实际实现）：
- 写入 Tauri 应用配置目录 app_config_dir：`<app_config_dir>/config/config.json`
  - Windows 示例：`%APPDATA%/top.jwyihao.fireworks-collaboration/config/config.json`

加载策略：
- 启动时读取（不存在则写入默认模板）
- `set_config(newCfg)` 保存：整体覆盖写回 app_config_dir（同时更新内存），不触发应用重启（dev）

---

## 4. 详细任务拆解（按子阶段）

### P0.1 基础环境与骨架
1. 添加依赖（Cargo.toml）：
    - hyper, hyper-rustls, rustls, tokio, tracing, serde, serde_json, anyhow, thiserror, uuid, base64
    - gitoxide（`git-repository` crate）
2. 初始化 tracing（logging.rs）
3. 事件发射器（基于 Tauri `app_handle.emit_all` 封装）
4. 配置模块：
    - model.rs 定义 Config 结构
    - loader.rs：`load_or_init()`, `save(cfg)`
5. 前端：创建 store（tasks/logs）+ 基础 Tauri invoke 封装

验收：应用启动无 panic；前端能获取配置（get_config）。

### P0.2 任务基础设施
1. 定义 TaskKind / TaskState / TaskMeta
2. TaskRegistry:
    - create() -> (task_id, token)
    - update_state()
    - snapshot(id) / list()
    - cancel(id)
3. 事件：
    - `task://state` {taskId, state, kind}
    - `task://progress` {taskId, progressType, value...}
4. 取消：
    - 使用 `CancellationToken`；任务内部周期性检查

验收：模拟启动一个“假任务”（sleep）并能取消；前端任务列表实时刷新。

#### P0.2 实际实现说明 (已完成)
| 项目 | 实现情况 | 备注 |
|------|----------|------|
| TaskState | Pending / Running / Completed / Failed / Canceled | 与计划一致 |
| TaskKind | GitClone / HttpFake / Sleep / Unknown | Sleep 用于测试；GitClone/HttpFake 预留，当前核心测试集中于 Sleep |
| TaskMeta 字段 | id / kind / state / created_at / cancel_token / fail_reason | fail_reason 预留，尚未事件化 |
| TaskRegistry 方法 | create / list / snapshot / cancel | 外部无独立 update_state，内部 set_state_* 封装 |
| 状态事件 | task://state | 负载含 taskId/kind/state/createdAt(ms) |
| 进度事件 | task://progress | 现仅 {taskId, kind, phase, percent}，无 objects/bytes |
| 错误事件 | 未实现 | 后续 Git/HTTP 失败时补 task://error |
| 取消机制 | CancellationToken 轮询检查 | Sleep 循环中每 step 检查 token |
| Sleep 任务实现 | 50ms 步长累进 + percent 计算 | 便于测试快速覆盖 Running / Cancel / Completed |
| 单元测试 | 3 个（创建初始状态 / 正常完成 / 取消） | registry.rs #[cfg(test)] 通过 |
| 事件发射适配 | feature="tauri-app" 有效；测试模式 no-op | 保证核心逻辑测试不依赖 Tauri runtime |

#### 差异与待办
- 进度事件缺少 Git 所需对象/字节统计，计划在 P0.6 扩展（新增 objects/bytes/totalHint 字段）。
- 未提供 task://error；失败原因暂存 TaskMeta.fail_reason，后续统一 error 事件格式。
- 未引入 ErrorCategory 枚举；HTTP 与 Git 接入后再统一分类映射。
- 前端实时消费验证未在本阶段执行（等待 P0.7 UI 集成测试）。

#### 测试覆盖摘要
- test_create_initial_pending：验证 create 后状态=Pending。
- test_sleep_task_completes：验证运行完成转 Completed。
- test_cancel_sleep_task：验证取消后转 Canceled。

#### 目标风险与缓解
| 风险 | 影响 | 缓解 |
|------|------|------|
| 进度维度不足 | Git 进度展示不细致 | P0.6 扩展 TaskProgressEvent 字段 |
| 缺少错误事件 | 前端无法展示失败原因 | 引入 task://error + fail_reason 发射 |
| 状态更新分散 | 新任务增加重复代码 | 后续提炼统一 helper（spawn 包装） |

（仅新增说明，不修改后续未完成阶段规划。）

### P0.3 TLS 验证器
1. 自定义 `ServerCertVerifier` 包装 rustls 默认验证
2. 解析证书 SAN（使用 `x509-parser` 或 rustls 提供的 API，如果引入额外 crate 则标记依赖）
3. SAN 白名单匹配（支持通配符 `*.github.com`）
4. Fake SNI 判定逻辑函数：
   ```
   fn should_use_fake(cfg, force_real: bool) -> bool
   ```
5. 错误归类：返回自定义 Error（ErrorCategory::Verify）

验收：对 github.com 正常；对非白名单域（如 example.com）应返回 SAN mismatch。

#### P0.3 实际实现说明 (已完成)
| 项目 | 实现情况 | 备注 |
|------|----------|------|
| 依赖选择 | rustls = 0.21（启用 `dangerous_configuration` 以允许设置自定义验证器）；webpki-roots = 0.25 | 采用内置根证书集合填充 RootCertStore，避免平台差异；后续可选切换 native-certs |
| 验证器 | `WhitelistCertVerifier` 包装 `WebPkiVerifier` | 先进行标准链验证，通过后再基于 SNI 主机名进行白名单匹配；不解析证书内 SAN 列表（P0 简化）|
| 白名单匹配 | 支持精确域与前缀通配 `*.`（如 `*.github.com`）| 大小写不敏感；需要存在分隔点且以基域结尾；`github.com` 不匹配 `x.ygithub.com` |
| 空白名单策略 | 空白名单视为拒绝 | 防御性默认更安全 |
| 非 DNS SNI | 拒绝 | 当 `ServerName` 为 IP 地址等非 DNS 名称时一律拒绝 |
| 伪 SNI 判定 | `should_use_fake(cfg, force_real)` | 仅当配置启用且未强制真实 SNI 时返回 true；伪域名读取自配置（P0.4 将实际使用）|
| ClientConfig | `create_client_config(&TlsCfg)` | 返回注入白名单验证器的 rustls `ClientConfig`，无客户端证书，供 HTTP 客户端直接复用 |
| 错误暴露 | `TlsError::General("SAN whitelist mismatch")` | 上层在 P0.5 中映射为 Verify 类别 |

实现文件与接口：
- `src-tauri/src/core/tls/util.rs`
  - `should_use_fake(cfg: &AppConfig, force_real: bool) -> bool`
  - `match_domain(pattern: &str, host: &str) -> bool`
- `src-tauri/src/core/tls/verifier.rs`
  - `struct WhitelistCertVerifier`（实现 `ServerCertVerifier`）
  - `make_whitelist_verifier(tls: &TlsCfg) -> Arc<dyn ServerCertVerifier>`
  - `create_client_config(tls: &TlsCfg) -> ClientConfig`

测试覆盖摘要（均通过）：
- 伪 SNI 判定：开关与 `force_real` 组合
- 域匹配：精确/通配、大小写不敏感、多级子域、非标准通配（如 `*.*.github.com`）不匹配
- 空白名单：任意域名均拒绝
- 非 DNS SNI：IP 形式的 `ServerName` 被拒绝
- ClientConfig：可成功构造并注入验证器

差异与待办：
- 未解析证书内 SAN 列表（基于 SNI 白名单判定即可满足 P0 安全基线）；如需更严格策略，后续可在 P1+ 引入 `x509-parser` 对证书扩展进行解析再比对
- 未实现 SPKI Pin（按 Roadmap 在 P7 引入）
- 未实现 Fake 失败自动回退至 Real SNI（按 Roadmap 在 P3 与 Git 一并处理）
- 与代理策略的联动（代理模式禁用伪 SNI）将在 P4 统一处理

目标风险与缓解：
| 风险 | 影响 | 缓解 |
|------|------|------|
| 仅基于 SNI 的白名单判定 | 未核对证书 SAN 列表 | 保持严格白名单；P1+ 增强为解析 SAN 列表并与白名单求交集 |
| 根证书集合差异 | 特定平台证书差异导致握手异常 | 采用 webpki-roots 统一基线；必要时提供可选 native-certs 路径 |
| 伪 SNI 被网络策略阻断 | 连接失败 | 按 Roadmap 在 P3 引入 Fake→Real 回退；当前记录日志即可 |

验收状态：
- 本地 `cargo test` 全部通过；验证器在白名单内域名通过，在非白名单或非 DNS SNI 情况下拒绝。

（仅新增说明，不修改后续未完成阶段规划。）

### P0.4 HTTP 客户端核心
1. 建立 `HttpClient`（内部管理 hyper::Client<HttpsConnector>）
2. 连接阶段分解 timing：
    - DNS（可暂不单独分，标记为 0）
    - TCP connect
    - TLS handshake
    - 首字节 & 总时长
3. 支持伪 SNI：
    - 若 fake，构造 rustls `ServerName` 为伪域；但 Host 头仍使用真实域
4. 响应：
    - 读取全量 body -> Vec<u8>
    - 大小超警阈值 -> warn 日志
5. 返回结构 HttpFakeResp（含 timing / usedFakeSni / body_base64 / status / headers）

验收：调用 API 请求 github.com 正常返回；fakeSniEnabled 切换后 usedFakeSni 字段变化。

#### P0.4 实际实现说明 (已完成)
| 项目 | 实现情况 | 备注 |
|------|----------|------|
| 依赖与栈 | hyper 0.14 + tokio-rustls 0.24 + rustls 0.21 + webpki-roots | 与 P0.3 验证器保持一致根证书来源 |
| 连接路径 | TcpStream -> TLS(tls.connect) -> hyper::client::conn::handshake(HTTP/1.1) | 采用手动握手以便覆盖自定义 SNI |
| 伪 SNI | `compute_sni_host(forceReal, realHost)` | true 使用伪域名；否则真实域；并记录 usedFakeSni |
| Host 头 | `upsert_host_header(headers, realHost)` | 无论是否伪 SNI，Host 均强制为真实域 |
| timing | connectMs / tlsMs / firstByteMs / totalMs | DNS 仍记为 0，不单独拆分 |
| Body | 全量读取后 base64 返回；超阈值 WARN | WARN 判定为 bodySize 严格大于阈值 |
| 早失败 | 在触网前先解码 bodyBase64 | 无效 base64 立即返回错误，便于离线单测覆盖 |
| 返回结构 | 与计划一致；ip 暂为 None；redirects 为空 | 重定向将在 P0.5 的命令层处理 |
| 验证器 | 复用 P0.3 的 SAN 白名单 verifier | 基于 SNI 域名进行白名单校验 |

单元测试覆盖（新增）：
- 非 https 协议立即拒绝（不触发网络连接）
- 无效 bodyBase64 早失败（未触网）
- 伪 SNI 决策函数（forceReal/开关组合）
- Host 头写入/覆盖逻辑
- 大响应 WARN 阈值边界（等于不告警，大于告警）

差异与待办：
- 未在本阶段暴露 Tauri Command；`http_fake_request` 将在 P0.5 实现
- 未实现重定向跟随；redirects 由 P0.5 的命令层统一处理
- 返回的 ip 字段当前为 None；后续在连接阶段增加对端地址提取
- 日志 WARN 捕获型测试可作为后续增强（需要日志捕获辅助）

### P0.5 http_fake_request API
1. Tauri command 参数映射前端模型
2. 校验 URL 协议 https://
3. 白名单域校验
4. 授权头脱敏日志：
    - 记录前：对 headers["Authorization"] 使用固定占位
5. 重定向（初版可简单跟随，保留计数；超限报错）
6. 错误分类映射：
    - 超时 / IO -> Network
    - TLS handshake -> Tls
    - SAN mismatch -> Verify
7. 前端 HttpTester.vue：
    - 表单：URL / Method / Headers / Body（可选）
    - 显示：Status / Timing / Body (decoded text 预览) / usedFakeSni

#### P0.5 实际实现说明 (已完成)
| 项目 | 实现情况 | 备注 |
|------|----------|------|
| Tauri Command | `http_fake_request(input: HttpRequestInput) -> Result<HttpResponseOutput, String>` | 位于 `src-tauri/src/app.rs`，签名与前端类型一致 |
| 早期校验 | 仅允许 `https://` URL；缺失 host 直接报错 | 错误归入 `Input` 类别 |
| 白名单预检 | 在触网前使用 `host_in_whitelist(host, &cfg)` 进行域名白名单校验 | 白名单来自 `cfg.tls.san_whitelist`，空白名单一律拒绝 |
| 日志脱敏 | `redact_auth_in_headers(headers, mask)` 大小写不敏感替换 Authorization | `mask` 受 `logging.authHeaderMasked` 控制，默认开启 |
| 重定向处理 | 支持 `followRedirects` 与 `maxRedirects`；收集 `RedirectInfo` 列表 | 301/302/303 规范化为 GET 并清空 body；307/308 保留当前尝试的方法与 body |
| Location 解析 | 使用 `url::Url` 基于当前 URL 进行相对/绝对解析 | 对每一跳的目标 host 再次执行白名单预检，禁止跳出白名单域 |
| 客户端复用 | 通过 `HttpClient::new(cfg)` + `send()` 发起请求 | `HttpClient` 内部负责 TCP/TLS/HTTP 握手、SNI 决策与 timing 采集（见 P0.4） |
| 错误分类 | `classify_error_msg(e)` 将错误映射到 `Verify/Tls/Network/Input/Internal` | 将原始错误信息前缀化返回，如 `Verify: SAN whitelist mismatch ...` |
| 竞争条件修复 | 克隆 `AppConfig` 后再 `await`，避免跨 `await` 持有 `MutexGuard` | 规避 Tauri/Tokio 非 `Send` 锁导致的编译错误 |
| 不安全开关 | 当 `tls.insecureSkipVerify=true` 时底层 rustls 使用 Insecure 验证器 | 仅用于原型联调（默认 false，UI 有明确风险提示） |

实现文件与接口：
- `src-tauri/src/app.rs`
  - `http_fake_request`：Tauri 命令主体；包含白名单预检、重定向跟随、错误分类与日志脱敏。
  - `redact_auth_in_headers` / `host_in_whitelist` / `classify_error_msg`：命令层辅助函数。
- `src-tauri/src/core/http/client.rs`
  - `HttpClient::send(input: HttpRequestInput) -> HttpResponseOutput`：执行连接、TLS 握手与请求发送，返回完整响应与 timing、usedFakeSni 等。
- `src-tauri/src/core/http/types.rs`
  - `HttpRequestInput` / `HttpResponseOutput` / `RedirectInfo` / `TimingInfo`：跨前后端的数据结构。
- `src-tauri/src/core/tls/verifier.rs`
  - `create_client_config(tls: &TlsCfg)`：构建 rustls `ClientConfig`；当 `insecureSkipVerify=true` 时启用 `InsecureCertVerifier`（仅原型）。
- 前端：
  - `src/api/http.ts`：封装调用 `http_fake_request`。
  - `src/views/HttpTester.vue`：表单与结果展示，并提供“跳过证书验证（不安全）”显式开关（写入 `tls.insecureSkipVerify`）。

测试覆盖摘要（均通过）：
- Rust 单测：
  - `app.rs` 中新增用例覆盖授权脱敏（大小写）、白名单匹配（精确/通配/空表）与错误分类映射。
  - `http/client.rs` 既有用例覆盖非 https 拒绝、无效 base64 早失败、Host 头覆盖、大响应告警阈值与 SNI 决策。
- 端到端：
  - 手动在 HttpTester 中对 `https://github.com/` 发起请求可获得 200；关闭白名单时会被拒绝。
  - `cargo test` 在本阶段全部通过。

边界与待办：
- 尚未引入重定向链的集成测试（可后续添加使用本地 mock 端点）。
- 响应 `ip` 字段当前可能为 `None`；后续可在连接阶段注入对端地址。
- 错误分类表可进一步细化（如 HTTP 状态类别化）；当前满足 P0 最小可用。
- Insecure 模式仅用于原型；默认关闭并在 UI 明示风险。

验收：可对 `https://github.com/` 发请求获 200；关闭白名单时（测试） -> 拒绝；伪 SNI on/off 可见差异。

### P0.6 Git Clone 基础
1. 引入 gitoxide：使用 `git_repository::clone::fetch_then_checkout(...)` 或等价 API
2. 包装为异步（spawn blocking）
3. 进度回调：
    - side-band 进度解析（对象数量 / bytes）
    - 发送 `git://progress` 或复用 `task://progress`
4. 错误分类：
    - 网络失败：Network
    - 协议解析：Protocol
5. 取消支持：
    - 在循环/回调中检查 token；若取消 -> 中断返回 Err(Cancel)
6. API：
    - `git_clone(repo_url, dest_path)` 返回 taskId
    - `git_cancel(taskId)`

前端 GitPanel：
- 输入：仓库 URL、保存路径（可用固定临时目录先行）
- 展示：任务状态 + 进度条 + 取消按钮

验收：克隆中型公开仓库（如 `https://github.com/rust-lang/log`）成功；取消在中途触发后任务状态= canceled。

#### P0.6 实际实现说明 (已完成)
| 项目 | 实现情况 | 备注 |
|------|----------|------|
| 依赖 | 引入 `gix = 0.73` 且禁用默认特性，启用 `blocking-network-client`、`worktree-mutation`、`parallel` | 仅使用阻塞克隆 API，放入 `spawn_blocking` 线程 |
| 任务接线 | `TaskRegistry::spawn_git_clone_task(app, id, token, repo, dest)` | 状态事件：`Pending -> Running -> Completed/Failed/Canceled` |
| 进度事件 | 现阶段发送粗粒度阶段：`Starting (0)`、`Fetching (10)`、`Checkout (80)`、`Completed (100)` | `TaskProgressEvent` 已扩展 `objects/bytes/total_hint` 字段，后续接入细粒度 |
| 取消 | 通过 `CancellationToken` + `&AtomicBool` 传给 gix 的 `should_interrupt` | 取消时任务状态转为 `Canceled` |
| 命令 | `git_clone(repo: String, dest: String) -> taskId` | 由前端发起；取消复用 `task_cancel(taskId)` |
| 事件通道 | `task://state` 与 `task://progress` | 与 P0.2 一致 |

补充落地细节与兼容性说明：
- HTTPS 传输支持：默认 `gix` 不内置 HTTPS，需要在 `Cargo.toml` 额外启用 `gix-transport` 的 `http-client-reqwest` 特性以支持 `https://` 克隆；本仓库已在 `src-tauri/Cargo.toml` 增加如下配置：
  - `gix = { version = "0.73", default-features = false, features = ["blocking-network-client","worktree-mutation","parallel"] }`
  - `gix-transport = { version = "0.48", default-features = false, features = ["http-client-reqwest"] }`
  这样即可通过 reqwest/hyper-rustls 栈完成 HTTPS Git 传输，解决“'https' is not compiled in”报错。
- 阻塞执行与取消桥接：`clone_blocking()` 运行在 `tokio::task::spawn_blocking` 中，避免阻塞异步调度；取消逻辑通过 `CancellationToken` 派生一个 `AtomicBool` 标志传入 gix 的 `should_interrupt`，由一个后台监听任务在收到取消时设置为 true，保证 gix 在安全检查点尽快返回。
- Windows 路径与 URL 兼容：为避免将本地路径误判为 URL（可能导致长时间等待），`clone_blocking()` 实现了路径判定：
  - 绝对路径（含盘符，如 `C:\repo`/`C:/repo`）或包含反斜杠 `\`、相对前缀 `./`、`../` 将按“本地路径”处理，使用 `gix::prepare_clone(Path)` 分支；
  - 其余按远程 URL 处理（`https://`/`http://` 等）。
- 失败分类与任务收尾：当 gix 返回错误且取消标志未触发，则视为失败，发射 `Failed` 状态事件；若取消标志为真，则发射 `Canceled`；无论何种终止路径均清理内部监听句柄，避免测试或运行时悬挂。
- 前端集成与调试：
  - `src/api/tasks.ts` 新增 `startGitClone(repo,dest)`；
  - 开发环境中在 `src/main.ts` 注入 `window.__fw_debug = { invoke, listen, emit }` 便于 DevTools 手工验证：
    - `await window.__fw_debug.invoke('git_clone', { repo: 'https://github.com/rust-lang/log', dest: 'C:/tmp/log' })`
    - 监听 `task://state` / `task://progress` 观察生命周期与进度；
    - `await window.__fw_debug.invoke('task_cancel', { id })` 触发取消。
- 测试覆盖（Rust）：增加了无网络依赖的健壮性测试，防止卡死：
  - 无效 URL/相对路径快速失败；
  - 立即取消应尽快返回；
  - Registry 层任务取消、取消前完成、取消前失败的句柄收尾，均确保 `JoinHandle` 等待，不残留后台 watcher 线程。
- 测试覆盖（前端）：新增 `src/api/__tests__/git.integration.test.ts`，在模拟 Tauri 环境下验证：
  - `startGitClone()` 会调用 `git_clone` 并返回 taskId；
  - 初始化事件后，模拟 `task://state`/`task://progress` 能正确驱动 Pinia store 更新；
  - 取消流程通过 `task_cancel` 调用验证参数正确。

手动验证要点：
- 在 Dev 模式下通过 `window.__fw_debug` 进行克隆与取消手动测试；
- 确保目标目录可写且为空目录；若目录已存在，当前策略为交由 gix 返回错误（P0 行为）；
- Windows 下优先使用绝对路径 `C:/tmp/xxx` 以避免路径歧义；
- 已启用 HTTPS 支持，如遇证书链问题请先确认系统时间与网络可达性。

前端补充：在 `src/api/tasks.ts` 增加 `startGitClone(repo,dest)` 封装；扩展 Progress 事件 payload 可选字段以适配后续细粒度指标。

### P0.7 前端面板与可视化
1. TaskList.vue：列表显示（ID / Kind / State / Start Time）
2. GitPanel.vue：发起 clone + 进度条（对象数 / 已接收大小）
3. HttpTester.vue：历史请求简表（保留最近 N 条）
4. 全局错误提示（分类显示）
5. 配置界面（简版）开关 fakeSniEnabled + 保存调用 set_config

验收：单应用内同时进行多个 clone + HTTP 调试不会 UI 卡死；任务状态实时刷新。

#### P0.7 实际实现说明 (已完成)
| 项目 | 实现情况 | 备注 |
|------|----------|------|
| 任务列表 | 复用现有 `TaskList.vue`，显示 ID/Kind/State/时间 | 基于 Pinia `tasks` store 的 `list` 快照 |
| Git 面板 | 新增 `src/views/GitPanel.vue`：输入 repo/dest，启动克隆、展示进度、可取消 | 进度条显示 `percent` 与 `phase`；列表展示任务状态与时间 |
| 任务进度 | `task://progress` 事件已在前端注册并汇总到 store | `src/api/tasks.ts` 监听 progress，调用 `tasks.updateProgress` |
| 进度模型 | `tasks` store 扩展 `progressById`（按任务聚合） | 兼容可选字段：`percent`、`phase`、`objects`、`bytes`、`total_hint` |
| 取消任务 | 面板“取消”按钮调用 `task_cancel` | UI 仅对运行中任务展示取消按钮 |
| HTTP 面板 | `src/views/HttpTester.vue` 强化：历史记录（最近 N 条）、点击回填；策略开关 | 支持 Fake SNI 候选/命中列表与 403 轮换开关，保存到配置 |
| 配置读写 | `getConfig`/`setConfig` 读写 `http.fakeSniEnabled/http.fakeSniHosts/http.sniRotateOn403` 与 `tls.spkiPins/tls.metricsEnabled/tls.certFpLogEnabled/tls.certFpMaxBytes` | 保存后即时生效，不需重启 |
| 全局错误 | 新增 `src/stores/logs.ts` 与 `src/components/GlobalErrors.vue` 浮动提示 | `HttpTester` 中将错误推送到全局错误队列 |
| 路由 | 在 `src/router/index.ts` 注册 `/git` 路由 | 可从首页/导航进入克隆面板 |
| UI 风格 | 继承项目现有样式体系（Tailwind/DaisyUI） | 保持一致的输入/按钮/表格风格 |

实现文件小结：
- 前端视图与组件：
  - `src/views/GitPanel.vue`（新增）：克隆输入/启动/进度条/取消/任务表。
  - `src/views/HttpTester.vue`（增强）：历史列表、Fake SNI 策略开关（候选/命中/轮换）、错误提示。
  - `src/components/GlobalErrors.vue`（新增）：全局错误吐司展示。
- Store 与 API：
  - `src/stores/tasks.ts`（增强）：新增 `progressById` 与 `updateProgress`；保留 `upsert/remove` 逻辑。
  - `src/api/tasks.ts`（增强）：`initTaskEvents()` 订阅 `task://state` 与 `task://progress`，更新 store。
  - `src/stores/logs.ts`（新增）：简单日志栈，支持 push/clear 与长度上限。
  - `src/router/index.ts`（增强）：新增 `/git` 路由。

测试覆盖摘要（均通过）：
- Store：`updateProgress` 百分比钳制与可选字段合并；`state` 事件 upsert 行为。
- 事件接线：模拟 `task://progress` 事件驱动 store 更新。
- Git 面板：启动克隆调用参数正确；运行中任务展示取消并触发 `task_cancel`。
- HTTP 面板：发送请求后写入历史；点击历史回填表单；保存策略调用 `setConfig`。

差异与后续待办：
- Git 进度条当前以 `percent/phase` 为主，`objects/bytes/total_hint` 预留但未在 UI 细化展示（P1+ 可补充丰富展示）。
- HTTP 历史仅保存在内存，页面刷新丢失；可追加 `localStorage` 持久化（低风险增强）。
- 可考虑在任务列表增加过滤/搜索、以及错误分类的可视化标记（P1+）。

手动验收要点：
- 在 `/git` 发起两个不同仓库的 clone，观察两个任务并行运行且进度条独立更新，点击其中一个“取消”后状态转为 `Canceled`。
- 在 `HttpTester` 分别开启/关闭 Fake SNI 发起 `https://github.com/` 请求，确认响应与 `usedFakeSni` 字段变化；保存/恢复策略开关生效。
- 任一面板异常时，右上角出现全局错误吐司，若为授权或敏感信息，日志中已做脱敏处理（见 P0.5）。

### P0.8 测试与校验
1. Rust 单元测试：
    - SAN 匹配（通配符 / 精确域）
    - 脱敏函数 redact_auth()
    - Fake SNI 判定逻辑
2. 集成测试（可选feature `ci-tests`）：
    - http_fake_request github.com 返回 200
    - 白名单外域拒绝
3. 手动脚本（docs/manual-tests.md）：
    - 分别测 Fake SNI on/off
    - 取消一个 clone
4. 性能基线（人工记录）：
    - Clone 同一仓库 vs 系统 git 时间（只做参考，不优化）

验收：测试全部通过；关键路径无 panic；严重日志无 ERROR（除故意触发）

### P0.9 文档与发布
1. README 添加：
    - P0 能力列表
    - 构建步骤（pnpm install / cargo tauri dev）
    - 安全基线（SAN 白名单 & 伪 SNI 行为）
2. CONTRIBUTING（简版）
3. CHANGELOG：`P0 Initial Delivery`
4. 打标：创建 GitHub Release `v0.1.0-P0`

验收：新开发者按 README 能运行并成功执行一次 clone + http_fake_request。

---

## 5. 事件与数据格式（P0 最低集）

| 事件 | Payload 示例 |
|------|--------------|
| task://state | `{ "taskId":"...", "kind":"GitClone", "state":"running" }` |
| task://progress | `{ "taskId":"...","kind":"GitClone","phase":"Receiving","objects":120,"bytes":1048576 }` |
| task://error | `{ "taskId":"...","category":"Network","message":"timeout" }` |
| http://fakeRequest (可选) | `{ "url":"https://github.com","status":200,"usedFakeSni":true,"connectMs":42 }` |

（前端先至少处理 state / progress / error）

---

## 6. 错误分类（P0 最小映射表）

| Rust 层错误来源 | 分类 |
|-----------------|------|
| IO (连接超时 / refused) | Network |
| TLS handshake (rustls error) | Tls |
| SAN mismatch | Verify |
| gitoxide protocol decode | Protocol |
| 用户调用 cancel | Cancel |
| unwrap/panic (recover) | Internal |
| 非 2xx HTTP | Network（或后续扩展 HTTPStatus） |

---

## 7. 日志策略

| 级别 | 内容 |
|------|------|
| info | 启动 / 配置加载 / 任务开始/结束 |
| warn | 大响应警告 / SAN 列表为空（防御） |
| error | 任务失败原因（已脱敏） |
| debug（可选） | TLS timing / redirect 链（默认关闭） |

脱敏实现：
```
fn redact_auth(headers: &mut HeaderMap) {
  if let Some(v) = headers.get_mut("Authorization") {
    *v = HeaderValue::from_static("REDACTED");
  }
}
```

---

## 8. 测试清单（快速版）

| 类别 | 用例 | 预期 |
|------|------|------|
| SAN | github.com | OK |
| SAN | api.github.com | OK (通配符) |
| SAN | example.com | Verify Error |
| Fake SNI | enabled + force_real=false | usedFakeSni=true |
| Fake SNI | enabled + force_real=true | usedFakeSni=false |
| HTTP Redirect | 301 -> final | redirect count ≤ max |
| Large Body | 下载 > 阈值 | WARN 日志 |
| Git Clone | 正常仓库 | Completed |
| Git Cancel | 中途取消 | State=Canceled |
| 多任务 | 2 clone + 1 http | 均正常，UI 不阻塞 |
| 错误分类 | 非白名单域 | Verify |
| 错误分类 | TLS 故意错误域（自建不可信证书，可跳过） | Tls |
| 日志脱敏 | 带 Authorization | 日志中无 token |
| Insecure Skip | insecureSkipVerify=true + 假 SNI | 握手不因证书不匹配而失败（仅原型验证） |

---

## 9. 验收指标（P0 Done Definition）

| 指标 | 标准 |
|------|------|
| 功能 | 可成功执行 http_fake_request（fake on/off）与 git clone（至少两个不同仓库） |
| 任务 | 任务列表实时展示，取消有效 |
| 安全 | 白名单拦截非授权域；Authorization 未出现在日志 |
| 稳定 | 连续运行 3 次 clone + 5 次 HTTP 请求无崩溃 |
| 可用性 | 新开发者 30 分钟内可成功构建并跑通 |
| 文档 | README/CHANGELOG/基本贡献说明存在 |

---

## 10. 风险与即时缓解（仅 P0）

| 风险 | 触发 | 缓解 |
|------|------|------|
| gitoxide 进度未正确传递 | API 差异 | 简化先只发送总 bytes；后续细化 |
| Fake SNI 导致握手失败 | 某些网络策略 | 自动回退：失败后用真实 SNI 重试一次（P0 可直接 fail，记录 TODO） |
| 前端状态丢失 | 刷新页面 | 低风险，P0 不持久化任务（标记 enhancement） |
| 大响应耗内存 | 用户请求极大文件 | WARN + 建议后续 Streaming |
| 证书解析依赖不稳定 | 解析 SAN 失败 | 若解析失败 -> Treat as verify error，记录日志 |

---

## 11. 后续进入 P1 的准备点（P0 内留 Hook）

| 未来点 | P0 准备 |
|--------|---------|
| Retry 策略 | 预留 HttpStrategy.retry 结构（暂未使用） |
| IP 优选 | http::client 内部 connect 抽象成 trait 接口 connect_host() |
| 代理 | 预留构造 HttpClient 时的 ProxyConfig Option |
| 指纹记录 | 在 TLS 验证成功路径上记录 leaf cert DER Hash（暂写 debug） |
| Push/Fetch | Git 任务分类枚举预留 GitFetch/GitPush |

---

## 12. 建议每日迭代检查列表（Standup Checklist）

- 昨日完成哪一子阶段清单项？
- 今日计划是否会引入新依赖或结构变更？
- 是否新增/修改公共接口（需要前端同步）？
- 是否出现未记录的错误分类情况？ → 更新 mapping
- 是否发现潜在流式需求提前暴露？ → 记录到 Backlog

---

## 13. Backlog（P0 实施中可能出现但不阻塞交付的项）

| 项 | 说明 | 处理策略 |
|----|------|----------|
| Redirect 安全限制 | 同域 / 跨域策略增强 | 标记 P1 |
| 任务持久化 | 重启恢复 | P2 考虑 |
| Clone 目标路径冲突策略 | 覆盖 / 跳过 / 失败 | 先直接失败，记录 |
| UI 任务过滤/搜索 | 体验增强 | P1+ |
| Fake SNI 回退机制 | 失败自动真实 SNI | P3 与 Git 统一 |
| 进度估算优化 | Git pack 深度估计 | 后期 |

---

## 14. 初始实现顺序（开发者可直接按此执行）

1. 拉分支：`feature/p0-core`
2. 添加依赖 & logging 初始化
3. 配置模块 + 默认文件写入
4. TaskRegistry + 简单测试
5. TLS 验证器 + 白名单测试
6. HTTP 客户端（含伪 SNI、timing）
7. http_fake_request command + 前端 HttpTester
8. git clone 封装 + 任务事件 + 前端 GitPanel
9. 错误分类与日志脱敏完善
10. 单元 & 手动测试清单执行
11. 文档与整理 / tag v0.1.0-P0

---

## 15. 输出示例（关键接口约定）

### Tauri Command：http_fake_request（请求/响应示例）

请求：
```json
{
  "url": "https://github.com/",
  "method": "GET",
  "headers": { "User-Agent": "P0Test" },
  "bodyBase64": null,
  "forceRealSni": false,
  "followRedirects": true,
  "maxRedirects": 5,
  "timeoutMs": 30000
}
```

响应：
```json
{
  "ok": true,
  "status": 200,
  "usedFakeSni": true,
  "ip": "140.82.xx.xx",
  "timing": { "connectMs": 41, "tlsMs": 55, "firstByteMs": 120, "totalMs": 350 },
  "headers": { "content-type": "text/html; charset=utf-8" },
  "bodyBase64": "...",
  "redirects": [],
  "bodySize": 58231
}
```

---

## 16. 快速验收脚本建议（手动）

```bash
# 1. 启动应用后：
# 2. 在 HTTP Tester 发请求：
curl -I https://github.com  # 对比状态

# 3. 切换 config fakeSniEnabled=false 再次请求比较 usedFakeSni 标志
# 4. Git Clone (UI 发起) 目录检查是否完整
# 5. Clone 过程中点击取消 -> 目录应不完整且任务状态=Canceled
# 6. 检查日志：未出现 Authorization 原文
```

---

## 17. 交付清单（完成时应打勾）

- [ ] config.json 默认文件生成
- [ ] SAN 白名单验证通过测试
- [ ] http_fake_request 支持 Fake SNI
- [ ] 大 body 警告日志
- [ ] 任务注册 / 状态 / 取消
- [ ] Git Clone 成功 + 进度事件可见
- [ ] 错误分类基础可用
- [ ] 授权头日志脱敏
- [ ] 前端：任务列表 + GitPanel + HttpTester
- [ ] README / CHANGELOG / 手动测试文档
- [ ] v0.1.0-P0 tag

---

## 18. 如需并行分工建议

| 角色 | 并行切块 |
|------|----------|
| Backend A | TaskRegistry + Git Clone 封装 |
| Backend B | TLS 验证器 + HTTP 客户端 |
| Frontend A | Task store + TaskList + GitPanel |
| Frontend B | HttpTester + 配置界面 |
| QA/Support | 用例草拟 + 手动脚本 + 文档校对 |

每日合并顺序：先核心库（tasks / tls / http），再 API，最后前端对接。

---

## 19. 成功标志（业务/体验角度）

- “我能快速诊断某个网络路径（fake on/off 差异）”
- “我能看到 Git 克隆实时进度并随时取消”
- “我不会因为误操作访问一个非 GitHub 域”
- “授权凭证不会泄漏到日志”

---

## 20. 下一阶段衔接提示（P1 准备）

在完成 P0 后，可立即创建以下 Issue：
1. Support Git Fetch / Push
2. Introduce retry strategy in HttpClient
3. Add per-task strategy override placeholder
4. Improve progress granularity (objects vs bytes)
5. Optional: fallback to real SNI on fake failure
