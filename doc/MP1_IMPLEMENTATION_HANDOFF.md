# MP1 实现说明（交接稿）— Push + Subtransport(A) + Retry v1 + 事件增强

> 用途：面向 P2 阶段（Shallow/Partial + 任务级策略覆盖）研发与联调的交接文档，汇总 MP1 的实现细节、接口契约、配置、测试与回退策略，并明确 P2 对接点与注意事项。
>
> 版本：v1.0（2025-09-15） 维护者：Core Team

---

## 1. 范围与目标

- 范围（在 MP0 基线之上）：
  - MP1.1：HTTPS Push（凭证回调、进度/取消/错误分类）
  - MP1.2：自定义 smart subtransport（方式A）灰度（仅接管连接/TLS/SNI），失败自动回退
  - MP1.3：Push 使用方式A（灰度，保持回退链与代理互斥）
  - MP1.4：Retry v1（指数退避 + 类别化；Push 仅上传前重试）
  - MP1.5：事件增强（push 阶段化、标准化 task://error）

- 不变：任务模型与事件命名（`task://state|progress|error`），前端 API 与 Store 结构保持兼容；新增字段均为可选。

- 可回退：
  - Push 可通过配置/命令封禁
  - 方式A 默认关闭，可白名单灰度；失败自动回退 Real/libgit2 默认
  - Retry 可配置关闭或降低阈值

---

## 2. 命令与事件契约（稳定接口）

- 命令（Tauri）
  - `git_clone(repo: string, dest: string): Promise<string /* taskId */>`
  - `git_fetch(repo: string, dest: string, preset?: 'remote'|'branches'|'branches+tags'|'tags'): Promise<string>`
  - `git_push({ dest: string; remote?: string; refspecs?: string[]; username?: string; password?: string }): Promise<string>`
  - `task_cancel(id: string): Promise<boolean>` / `task_list(): Promise<TaskSnapshot[]>`

- 事件
  - `task://state`：`{ taskId, kind, state, createdAt }`，state ∈ `pending|running|completed|failed|canceled`
  - `task://progress`：
    - Clone/Fetch：`{ taskId, kind, phase, percent, objects?, bytes?, totalHint? }`
    - Push：`{ taskId, kind, phase /* PreUpload|Upload|PostReceive */, percent, objects?, bytes? }`
    - 重试阶段可带 `retriedTimes?`
  - `task://error`：`{ taskId, kind, category, message, retriedTimes? }`（`code?` 预留）

- 大小写兼容（前端输入端容忍，内部统一 camelCase）
  - 进度：`totalHint` | `total_hint`
  - 错误：`retriedTimes` | `retried_times`

---

## 3. 后端实现要点（Rust/Tauri + git2-rs）

- Git 服务默认实现：`core/git/default_impl/*`
  - clone/fetch/push 基于 git2-rs；
  - 回调桥接进度（对象/字节/阶段），取消检查（`ErrorCode::User` → Cancel）；
  - 错误分类：`Network|Tls|Verify|Protocol|Proxy|Auth|Cancel|Internal`（集中于 `helpers.rs`）。

- 任务注册与事件：`core/tasks/registry.rs`
  - 统一任务生命周期、事件发射；
  - Retry v1：指数退避 + 抖动，`is_retryable` 按类别判定；Push 在进入 `Upload` 后不再自动重试；
  - 在重试与错误路径发出 `task://error` 与/或带 `retriedTimes` 的进度事件。

- 自定义 smart subtransport（方式A）：`core/git/transport/*`
  - 对命中白名单域的 `https://` 改写为 `https+custom://`，仅接管连接/TLS/SNI；
  - SNI 策略：优先 Fake（来自 `http.fakeSniHosts` 候选 + last-good），失败回退 Real；代理存在时强制 Real；
  - Push 特定：通过 TLS 层线程局部注入 Authorization，仅限 receive-pack（GET info/refs、POST /git-receive-pack）；401 → 显式 `Auth`；403 在 info/refs 阶段触发一次性轮换；
  - 失败回退链：Fake → Real → libgit2 默认。

- 配置读取：通过 app config dir 热加载；subtransport 与任务层共享同一套配置。

---

## 4. 前端实现要点（Vite/Vue + Pinia）

- 事件订阅：`src/api/tasks.ts`
  - 监听 `task://state|progress|error`；
  - 进度与错误事件字段归一（camelCase），进度写入 `progressById`，错误写入 `lastErrorById` 并推送全局日志。

- Store：`src/stores/tasks.ts`
  - `items`（任务列表）、`progressById`（聚合进度）、`lastErrorById`（最近错误）；
  - `setLastError` 处理 `retried_times`/`retriedTimes` 兼容。

- UI：`src/views/GitPanel.vue`
  - 表格新增“最近错误”列：分类 Badge + 重试次数 + 错误信息；
  - Push 表单支持用户名/密码（令牌）；Clone/Fetch 支持预设；HTTP Fake SNI 策略（候选列表/命中目标/403 轮换）可编辑并即时生效。

---

## 5. 配置模型（与默认值）

- `http`: `{ fakeSniEnabled: boolean, fakeSniHosts?: string[], sniRotateOn403?: boolean, followRedirects: boolean, maxRedirects: number }`（目标域名单内置，与 `ip_pool::preheat::BUILTIN_IPS` 同步）
- `tls`: `{ spkiPins?: string[], metricsEnabled?: boolean, certFpLogEnabled?: boolean, certFpMaxBytes?: number }`
- `retry`: `{ max: number, baseMs: number, factor: number, jitter: boolean }`（默认 `{ max: 6, baseMs: 300, factor: 1.5, jitter: true }`）
- `proxy`: `{ mode: 'off'|'http'|'socks5', url?: string }`
- `logging`: `{ debugAuthLogging: boolean }`（默认脱敏）

---

## 6. 测试现状

- 后端（cargo test）：
  - 覆盖任务注册、重试、分类映射，本地裸仓库 push/clone/fetch 等集成路径；
  - 全部通过（Windows 环境验证）。

- 前端（pnpm test）：
  - API 层（事件订阅与 casing 归一）、Store 层（进度/错误）、视图层（GitPanel/GitHubActions/HttpTester 等）；
  - 新增 GitPanel 错误渲染与 casing 兼容测试；
  - 全部通过（22 文件，83 用例）。

---

## 7. 回退与安全

- 回退：
  - 方式A 可关闭；失败自动回退 Real/libgit2 默认；
  - Retry 可配置关闭或降级；
  - Push 可按配置/入口禁用。

- 安全：
  - 默认脱敏 Authorization/密码/令牌；
  - Fake SNI 通过 `RealHostCertVerifier` 在握手后仍以真实域名验证证书链与 SPKI，无法通过配置关闭。

---

## 8. P2 对接点与注意事项

- Shallow/Partial 支持：
  - 建议在 `git_clone`/`git_fetch` 入参新增可选 `depth?: number` 与 `filter?: string`（如 `"blob:none"`），任务层透传给 git2-rs。
  - 进度事件不变；如需展示“省流量”指标，可在 progress 附加可选 `bytesSaved?`（兼容策略）。

- 任务级策略覆盖：
  - 仅允许 `http?`/`retry?` 子集（白名单字段），任务注册时与全局配置浅合并，仅作用于当前任务；TLS 策略覆盖已在 v1.8 移除。
  - 与方式A交互：代理模式下仍禁用 Fake SNI；如任务级覆盖启用 Fake SNI，应在注册处进行“代理互斥”校验并给出警告。

- 兼容性约束：
  - 若新增字段进入事件（如 push 的 bytesSent），必须保持可选；
  - 继续维持 snake/camel 输入兼容，内部统一 camel；
  - 禁止在事件中泄漏凭证等敏感信息。

- 测试要求：
  - 增加“depth/filter”集成测试（针对本地裸仓库与中等体量仓库模拟），校验事件稳定与正确性；
  - 任务级覆盖需单测覆盖合并策略与约束（代理 x Fake SNI 互斥）。

---

## 9. 快速排错指引（P2 仍然适用）

- Push 401/403：优先检查 PAT 权限与组织 SSO；事件 `category=Auth`，日志已脱敏。
- TLS/Verify 失败：优先检查证书链与主机名是否匹配真实目标域；Fake SNI 场景同样由 `RealHostCertVerifier` 执行真实域校验，可结合 `tls.spkiPins`/Fake SNI 命中列表排查。
- 进度停滞：关注 `phase` 是否停在 `PreUpload/Upload`；上传后不再自动重试，必要时手动取消重试。
- 403 早期：若启用轮换，info/refs 阶段会切换一次 SNI 候选，仍失败则回退 Real。

---

## 10. 变更一览（与 MP0 的差异）

- 新命令：`git_push`
- 新事件：`task://error`
- 进度增强：push 的阶段化（`PreUpload|Upload|PostReceive`）
- 传输：引入方式A子传输（仅接管连接/TLS/SNI）与回退链
- 重试：统一 Retry v1（类别化 + 指数退避，Push 限于上传前）
- 前端：增加 GitPanel 错误列与 casing 兼容处理

---

## 11. 附：关键文件速览

- 后端
  - `src-tauri/src/core/tasks/registry.rs`（任务注册、事件、Retry）
  - `src-tauri/src/core/tasks/model.rs`（事件与负载结构）
  - `src-tauri/src/core/git/default_impl/*`（git2-rs 实现、错误分类、进度桥接）
  - `src-tauri/src/core/git/transport/*`（方式A注册、改写、授权注入、流实现）
- 前端
  - `src/api/tasks.ts`（事件订阅与归一化）
  - `src/stores/tasks.ts`（进度与错误的 Store）
  - `src/views/GitPanel.vue`（Push 表单、TLS/SNI 策略、最近错误列）
  - `src/views/__tests__/git-panel.error.test.ts`（UI 错误渲染测试）

---

（完）
