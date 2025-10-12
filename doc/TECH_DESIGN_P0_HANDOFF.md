# P0 → P1 对接实现说明（交接稿）

> 目的：面向即将进入 P1 的开发与测试同学，提供 P0 已实现能力的精炼说明（接口、事件、配置、限制与扩展点），确保无缝对接与规划落地。

---

## TL;DR
- P0 已交付：通用伪 SNI HTTP 调试 API、基础 Git Clone、任务模型（状态/进度/取消）、SAN 白名单校验、日志脱敏与基础配置。
- 对接重点：沿用既有命令名与数据结构；前端事件监听已封装在 `initTaskEvents()`；配置通过 `get_config`/`set_config` 读写。
- P1 扩展方向：Git Fetch/Push、重试策略、进度细化、（P3 预备）Fake→Real SNI 回退、（P4 预备）代理钩子、错误分类细化。

---

## 一、P0 成果清单（功能与入口）

- 通用 HTTP（伪 SNI）
  - 命令：`http_fake_request(input)`
  - 文件：
    - 后端：`src-tauri/src/app.rs`（命令逻辑）、`src-tauri/src/core/http/{client.rs,types.rs}`（HTTP 栈）
    - 前端：`src/api/http.ts`（类型与调用）、`src/api/tauri-fetch.ts`（Fetch 语义桥接层）
- Git Clone 基础
  - 命令：`git_clone(repo, dest)`，取消：`task_cancel(id)`
  - 文件：
    - 后端：`src-tauri/src/app.rs`（命令注册）、`src-tauri/src/core/tasks/{registry.rs,model.rs}`、`src-tauri/src/core/git/{progress.rs,...}`
    - 前端：`src/api/tasks.ts`（事件订阅与启动）、`src/views/GitPanel.vue`（UI）
- 任务模型与事件
  - 命令：`task_list` / `task_snapshot` / `task_cancel` / `task_start_sleep`
  - 事件：`task://state`、`task://progress`
  - 文件：
    - 后端：`src-tauri/src/core/tasks/{registry.rs,model.rs}`、`src-tauri/src/events/emitter.rs`
    - 前端：`src/api/tasks.ts`（`initTaskEvents()`）、`src/stores/tasks.ts`
- TLS 与白名单
  - 文件：`src-tauri/src/core/tls/{verifier.rs,util.rs}`（rustls 自定义验证器 + 白名单匹配）
- 配置与日志
  - 命令：`get_config` / `set_config`
  - 文件：`src-tauri/src/core/config/{model.rs,loader.rs}`、`src-tauri/src/logging.rs`

---

## 二、命令与数据契约（稳定接口）

### 1) HTTP：`http_fake_request(input)`
- 请求（前端同名 TypeScript 接口 `HttpRequestInput`）：
  - `url: string`（仅支持 https）
  - `method: string`（GET/POST/...）
  - `headers: Record<string,string>`（Authorization 将按配置脱敏写日志）
  - `bodyBase64?: string | null`
  - `timeoutMs: number`
  - `forceRealSni: boolean`（true 时即使开启 fakeSni 也强制使用真实 SNI）
  - `followRedirects: boolean`、`maxRedirects: number`
- 响应（`HttpResponseOutput`）：
  - `ok: boolean`、`status: number`、`headers: Record<string,string>`
  - `bodyBase64: string`、`bodySize: number`
  - `usedFakeSni: boolean`、`ip?: string | null`（P0 可能为 null）
  - `timing: { connectMs, tlsMs, firstByteMs, totalMs }`
  - `redirects: Array<{ status, location, count }>`
- 行为要点：
  - 仅 https；触网前进行“域白名单预检”。
  - 前端通过 `tauriFetch` 访问时（`src/api/tauri-fetch.ts`），若未显式提供 `User-Agent` 会自动补全为 `fireworks-collaboration/tauri-fetch`，同时原样透传 Authorization 等头部以满足 GitHub API 对 UA 的要求且不影响凭证携带。
  - Fake SNI 决策：配置开启且未 `forceRealSni` 时使用伪域；Host 头始终为真实域。
  - 重定向链受白名单限制；301/302/303 规范化为 GET 且清空 body，307/308 保留方法与 body。
  - 错误分类映射：Verify/Tls/Network/Input/Internal（以字符串前缀体现在错误消息中）。

### 2) Git：`git_clone(repo, dest)` / `task_cancel(id)`
- 启动克隆：返回 `taskId: string`。
- 取消：返回 `boolean`（是否成功触发取消）。
- 事件（见下一节）驱动 UI 进度与状态。
- 行为要点：
  - 使用 `gitoxide(gix)` 阻塞克隆 API，放入 `spawn_blocking`；取消通过 `CancellationToken` → `AtomicBool` 桥接到 gix。
  - 进度 P0 为粗粒度阶段百分比（`percent` + `phase`），已预留 objects/bytes/total_hint 字段（P1 可接入细化）。

### 3) 任务：`task_list` / `task_snapshot(id)` / `task_cancel(id)` / `task_start_sleep(ms)`
- 事件接线与 Store 见下。

### 4) 配置：`get_config()` / `set_config(newCfg)`
- AppConfig（camelCase）：
  - `http`: `{ fakeSniEnabled: boolean, fakeSniHosts?: string[], sniRotateOn403?: boolean, followRedirects: boolean, maxRedirects: number, largeBodyWarnBytes: number }`
  - `tls`: `{ spkiPins?: string[], metricsEnabled?: boolean, certFpLogEnabled?: boolean, certFpMaxBytes?: number }`
  - `logging`: `{ authHeaderMasked: boolean, logLevel: string }`
- 存储：`<app_config_dir>/config/config.json`
  - Windows 示例：`%APPDATA%/top.jwyihao.fireworks-collaboration/config/config.json`

---

## 三、事件模型（前端接线）

- 事件通道：
  - `task://state`：`{ taskId, kind, state, createdAt }`
  - `task://progress`：`{ taskId, kind, phase, percent, objects?, bytes?, total_hint? }`
- 前端封装：`src/api/tasks.ts`
  - `initTaskEvents()` 内部订阅上述事件、并调用 `src/stores/tasks.ts` 的 `upsert()` 与 `updateProgress()`。
  - Store 结构：
    - `items: TaskItem[]`（`TaskItem { id, kind, state, createdAt }`）
    - `progressById: Record<taskId, { percent; phase?; objects?; bytes?; total_hint? }>`

---

## 四、TLS 与 Fake SNI 验证（安全基线）

- 验证器：`RealHostCertVerifier` 继承自 `rustls` 的 `WebPkiVerifier`，即便握手阶段改写了 SNI，也始终以真实目标域名验证证书链与主机名。
- Fake SNI 触发条件：仅当目标域名命中与 `ip_pool::preheat::BUILTIN_IPS` 同步的内置名单时才改写 ClientHello 中的 SNI；握手结束后仍记录真实域名并执行验证，未命中则使用真实域名直连。
- TLS 配置面板聚焦观测与 Pin：
  - `tls.spkiPins`：可选的 Base64URL SPKI Pin 列表。
  - `tls.metricsEnabled` / `tls.certFpLogEnabled` / `tls.certFpMaxBytes`：控制证书指纹与观测指标采集。
- 不再提供 `insecureSkipVerify` 或 SAN 白名单跳过开关；相关逻辑在 v1.8 之后移除，避免用户绕过真实主机验证。

---

## 五、错误分类与日志策略

- 错误分类（字符串前缀出现在错误消息中）：
  - `Verify`（白名单不符/验证失败）
  - `Tls`（握手或链错误）
  - `Network`（连接/读写/过多重定向）
  - `Input`（URL 非 https/缺 host/无效 redirect 等）
  - `Internal`（其他未归类错误）
- 日志脱敏：若 `logging.authHeaderMasked` 为 true（默认），则在记录请求概览时将 `Authorization` 头值替换为 `REDACTED`。
- 大响应告警：`http.largeBodyWarnBytes`（默认 5MB）阈值上打印 WARN。

---

## 六、已知限制与 P1 优先扩展点

- 限制（P0 保持简化）：
  - HTTP 响应为全量内存缓冲（暂无流式）；`ip` 字段可能为 null。
  - Git 进度粗粒度（percent/phase），尚未对接 objects/bytes 明细。
  - 重定向跨域严格受白名单限制；未提供“跨域放宽策略”。
  - 未实现 Fake→Real SNI 自动回退；无代理/无 IP 优选。
- P1 建议优先事项：
  1) Git Fetch/Push 命令与 UI 接入（含基础鉴权能力）。
  2) HTTP/Git 重试策略（区分 Network/Transient）。
  3) 进度细化：Git objects/bytes/totalHint 前后端打通并展示。
  4) 错误分类细化与前端提示友好化（可纳入“HTTP 状态类别化”）。
  5) 预埋 Fake→Real 回退钩子（P3 全量接入时仅切换策略）。
  6) 任务列表增强（过滤/搜索、失败原因 `task://error` 事件）。

---

## 七、如何在 P1 复用与扩展（代码指引）

- 扩展命令：在 `src-tauri/src/app.rs` 中新增 Tauri 命令，复用现有 `TaskRegistry` 模式，保持事件命名一致性（`task://state|progress|error`）。
- HTTP 客户端：`src-tauri/src/core/http/client.rs`
  - 可在 `send()` 前后扩展重试逻辑、注入代理配置；或将连接逻辑抽象为 trait 以便未来替换传输（P3/P4）。
- TLS 验证：`src-tauri/src/core/tls/verifier.rs`
  - 保持现有白名单策略；P3 将引入“Real-Host”验证器并保留现有实现以便回退。
- 配置：`src-tauri/src/core/config/model.rs`
  - 可为 P1 新增 `retry`、`perTaskOverride` 等字段；前端通过 `get_config`/`set_config` 对接。
- 前端事件：`src/api/tasks.ts` / `src/stores/tasks.ts`
  - 若新增 `task://error`，保持 payload 结构 `{ taskId, category, message }`，并在 `logs` store 或新的错误面板中展示。

---

## 八、运行与验证（开发约定）

- 前端测试：`pnpm test`
- 后端测试：在 `src-tauri` 目录执行 `cargo test`
- 开发运行：`pnpm dev`（或 VS Code 中运行 Tauri 预设任务）
- Windows PowerShell 示例（可选）：
  - 前端测试：`pnpm -s test`
  - 后端测试：
    - `cd src-tauri`
    - `cargo test`

---

## 九、质量门（当前状态）

- Build/Lint：依赖锁定，前端 `pnpm test` 通过（以 CI/本地为准）。
- Rust 单测：覆盖配置序列化、白名单匹配、HTTP 输入校验与授权脱敏、任务注册与取消等核心路径。
- 集成/手动：HTTP Tester 可对 `https://github.com/` 正常返回；Git 克隆公开仓库成功且可取消。
- 覆盖率：前端以 vitest 覆盖，后端以 rust 单测为主；P1 将继续补齐关键分支用例与重定向链集成测试。

---

## 十、附录：字段速查（对齐前端 TS 类型）

- `HttpRequestInput` / `HttpResponseOutput` / `TimingInfo` / `RedirectInfo`：`src-tauri/src/core/http/types.rs` 与 `src/api/http.ts` 一致。
- `TaskStateEventPayload` / `TaskProgressEventPayload`：`src/api/tasks.ts`。
- `AppConfig`（camelCase）：后端 `model.rs` 与前端 `src/api/config.ts` 对齐。

---

若对本说明中的命令、事件或字段有新增需求，请在 P1 任务单中补充“接口变更记录”，并在实现时保持后向兼容或提供迁移说明。
