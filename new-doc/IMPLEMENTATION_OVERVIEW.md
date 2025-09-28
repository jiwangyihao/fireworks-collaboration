# Fireworks Collaboration 实现总览（MP0 -> MP3 & 测试重构）

> 目的：以统一视角梳理 MP0、MP1、P2、P3 四个阶段以及测试重构后的现状，面向后续演进（P4+）的研发、运维与质量成员，提供完整的实现细节、配置指引、事件契约、测试矩阵与回退策略。
>
> 版本：v1.0（2025-09-28） 维护者：Core Team

---

## 1. 范围与阅读指引

- **涵盖阶段**：
  - **MP0**：gitoxide -> git2-rs 替换，任务/事件契约保持不变；
  - **MP1**：Push、方式A自定义 smart subtransport、Retry v1、事件增强；
  - **P2**：本地 Git 操作扩展、Shallow/Partial、任务级策略覆盖、策略信息事件与护栏；
  - **P3**：自适应 TLS 全量 rollout、可观测性强化、Real Host 校验、SPKI Pin、自动禁用、Soak；
  - **测试重构**：`src-tauri/tests` 聚合结构、事件 DSL、属性测试与回归种子策略。
- **读者画像**：
  - 新接手的后端/前端开发；
  - 运维与 SRE（回退、监控、调参）；
  - 测试与质量保障（测试矩阵、DSL 约束）。
- **联动文档**：`new-doc/MP*.md` 系列详细交接稿、`new-doc/TECH_DESIGN_*.md` 设计稿、`doc/TESTS_REFACTOR_HANDOFF.md`。

---

## 2. 里程碑一览

| 阶段 | 核心交付 | 事件新增/调整 | 配置扩展 | 回退策略 | 测试现状 |
|------|-----------|---------------|----------|----------|----------|
| MP0 | git2-rs 基线，Clone/Fetch 稳定，取消/错误分类统一 | 无新增，进度保持兼容 | 继承 HTTP Fake 调试配置 | 可回退到旧二进制（保留 gitoxide tag 归档） | `cargo test` / `pnpm test` 全绿 |
| MP1 | Push、方式A smart subtransport、Retry v1、进度阶段化 | `task://error`，Push `PreUpload/Upload/PostReceive`，错误分类输出 | HTTP/TLS/Fake SNI 配置热加载 | Push/方式A/Retry 可配置关闭或自动回退 | Rust/前端测试覆盖 push、事件 casing |
| P2 | 本地操作（commit/branch/checkout/tag/remote）、Shallow/Partial、策略覆盖、护栏、Summary | 覆盖策略事件：`*_override_applied`、`strategy_override_summary` 等 | `strategyOverride` 入参，env gating（`FWC_PARTIAL_FILTER_SUPPORTED`、`FWC_STRATEGY_APPLIED_EVENTS`） | 逐项移除 TaskKind / 关闭 gating | 新增矩阵测试、属性测试覆盖策略解析 |
| P3 | 自适应 TLS rollout + 可观测性、Real Host 校验、SPKI Pin、自动禁用、Soak | `AdaptiveTls*` 结构化事件、指纹变化事件 | `http.fakeSniRolloutPercent`、`tls.metricsEnabled`、`tls.certFpLogEnabled`、`tls.spkiPins` 等 | 配置层关闭 Fake/metrics/pin；自动禁用冷却 | Soak 测试 + 指标契约测试 |
| 测试重构 | 主题聚合、事件 DSL、属性测试集中管理 | DSL 输出 Tag 子序列 | N/A | N/A | `src-tauri/tests` 结构稳定，CI 使用共享 helper |

---

## 3. 总体架构快照

### 3.1 命令与任务接口

Tauri 暴露的稳定命令（保持 camelCase 输入，容忍 snake_case）：

```ts
// Git 操作
git_clone(repo: string, dest: string): Promise<string>
git_fetch(repo: string, dest: string, preset?: 'remote'|'branches'|'branches+tags'|'tags'): Promise<string>
git_push(opts: { dest: string; remote?: string; refspecs?: string[]; username?: string; password?: string }): Promise<string>

// 本地操作（P2）
git_commit(opts: CommitInput): Promise<string>
git_branch(opts: BranchInput): Promise<string>
git_checkout(opts: CheckoutInput): Promise<string>
git_tag(opts: TagInput): Promise<string>
git_remote_add|set|remove(opts: RemoteInput): Promise<string>

// 任务控制
task_cancel(id: string): Promise<boolean>
task_list(): Promise<TaskSnapshot[]>

// 调试
git_task_debug?(internal)
http_fake_request(input: HttpRequestInput): Promise<HttpResponseOutput>
```

所有命令返回 `taskId`，前端通过事件流追踪生命周期。

### 3.2 事件总览

- `task://state`：`{ taskId, kind, state, createdAt }`，`state ∈ pending|running|completed|failed|canceled`；
- `task://progress`：
  - Clone/Fetch：`{ taskId, kind, phase, percent, objects?, bytes?, totalHint?, retriedTimes? }`；
  - Push：`phase ∈ PreUpload|Upload|PostReceive`；
- `task://error`：分类或信息事件，`{ taskId, kind, category, message, code?, retriedTimes? }`；
- 自适应 TLS 结构化事件（P3）：`AdaptiveTlsRollout`、`AdaptiveTlsTiming`、`AdaptiveTlsFallback`、`AdaptiveTlsAutoDisable`、`CertFingerprintChanged`、`CertFpPinMismatch`；
- 策略信息事件（P2）：`http_strategy_override_applied`、`retry_strategy_override_applied`、`tls_strategy_override_applied`、`strategy_override_conflict`、`strategy_override_ignored_fields`、`partial_filter_fallback`、`strategy_override_summary`。

事件顺序约束在测试中锁定：策略 applied -> conflict -> ignored -> partial fallback -> summary；TLS 事件在任务结束前统一刷出。

### 3.3 服务与分层

```
TaskRegistry (core/tasks/registry.rs)
 ├─ 状态机：注册/运行/取消/重试
 ├─ 事件汇聚：state/progress/error -> Tauri emitter
 ├─ Retry v1：指数退避、类别判定（Push 上传前）
 └─ 策略应用：策略覆盖、护栏、Summary（P2+）

GitService (core/git/default_impl/*)
 ├─ git2-rs clone/fetch/push 基线（MP0/MP1）
 ├─ Push 凭证回调、进度阶段（MP1）
 ├─ 自定义 smart subtransport 方式A（transport/*, MP1）
 ├─ Shallow/Partial 策略与 capability（P2）
 └─ Adaptive TLS、fallback、metrics（P3）

Transport Stack
 ├─ Rewrite + rollout 决策（P3）
 ├─ Fallback 状态机 Fake->Real->Default
 ├─ TLS 验证与 SPKI Pin
 └─ 自动禁用窗口
```

前端（Pinia + Vue）在 `src/api/tasks.ts` 统一订阅事件，将 snake/camel 输入归一，`src/stores/tasks.ts` 管理任务、进度、错误、策略事件。

### 3.4 核心依赖与版本策略
- 后端 Rust 依赖：
  - `git2 = "0.19"`（MP0 起启用，MP1 推送凭证仍依赖该版本提供的回调 API；若需升级，需先验证 Windows/MSVC 与 macOS 通路的二进制兼容性）。
  - `tauri = 1.x` + `tauri-build`：通过可选特性 `tauri-app` 控制，`cargo test` 默认禁用 Tauri UI，确保核心逻辑可在 CI 上独立构建。
  - 传输层模块自带 `reqwest`（方式A 仅在 Fake SNI 调试路径使用，MP3 adaptive TLS 不直接依赖）。
  - TLS 校验依赖 `rustls` + `webpki`（P3 引入 Real Host 校验与 SPKI Pin 时同步升级到最新 LTS）。
- 前端依赖：
  - `vite`, `vue 3`, `pinia`, `@tauri-apps/api`，与 MP0 前保持一致；P2 起新增策略编辑组件使用 `zod` 做轻量校验。
  - 测试栈 `vitest` + `@testing-library/vue`；事件 DSL 断言主要位于 `src/views/__tests__`。
- 配置加载：`src-tauri/src/config/loader.rs` 使用 `directories` crate 定位应用目录，支持热加载；更改配置文件后由任务注册器下一次读取时生效。
- 版本管理：所有 Git 子传输代码在 `src-tauri/src/core/git/transport` 下有 `COVERAGE.md` 与 `MUTATION_TESTING.md`，升级依赖须同步更新两份保障文档。

### 3.5 发布节奏与跨阶段集成
- **推广顺序**：遵循 MP0 -> MP1 -> P2 -> P3 的递进路径，每阶段功能上线前都需确认前置阶段的回退手段仍可用；详见 `new-doc/MP*_IMPLEMENTATION_HANDOFF.md`。
- **配置切换流程**：
  1. 在预生产环境调整 `AppConfig`/环境变量验证行为；
  2. 触发 `fwcctl reload-config` 或重启 Tauri 容器以生效；
  3. 通过 `task_list` 验证无历史任务处于异常状态，再逐步扩大发布半径。
- **灰度策略**：MP1 的方式A 与 Retry、P3 的 fake SNI rollout 都支持按域或百分比分级，推荐每级灰度至少运行一次 soak 或回归脚本；
  - Rollout 0% -> 25% -> 50% -> 100%，期间监控 `AdaptiveTlsRollout` 与 `AdaptiveTlsFallback` 事件；
  - Push/策略相关配置调整需同步更新前端提示与文档，避免用户误解重试次数或凭证要求。
- **跨阶段依赖**：
  - P2 的策略覆盖与 P3 的 adaptive TLS 共享 HTTP/TLS 配置，只在任务级做差异化；
  - P3 的指纹日志与自动禁用依赖 MP1 方式A 的传输框架，如需临时关闭 Fake SNI，应评估 P3 指标链路的可见性。
- **回滚指引**：
  - 生产事故时优先通过配置禁用新增功能（Push、策略覆盖、Fake SNI、指标采集等）；
  - 若需降级二进制，参考 `src-tauri/_archive` 中的 legacy 实现及各阶段 handoff 文档的“回退矩阵”。
- **交接资料**：
  - 每次版本发布前更新 `CHANGELOG.md`、`new-doc/IMPLEMENTATION_OVERVIEW_MP0-P3.md` 与对应用的 handoff 文档；
  - 附上最新 soak 报告、配置快照、事件截图，供下游团队复用。

---

## 4. 阶段详情

### 4.1 MP0 - git2-rs 基线

- **目标**：替换 gitoxide 实现，保持前端行为；
- **关键实现**：
  - `GitService` 使用 git2-rs，桥接 `transfer_progress` 与 `checkout` 回调；
  - `TaskRegistry` 统一取消 token（`ErrorCode::User` -> Cancel）；
  - 错误分类：Network/Tls/Verify/Protocol/Auth/Cancel/Internal；
  - HTTP Fake 调试接口沿用，提供白名单/重定向/脱敏；
- **配置**：`AppConfig` 热加载 `tls.sanWhitelist`、`logging.authHeaderMasked`；
- **测试**：`cargo test` 与 `pnpm test` 全部通过，git2-rs 在 Windows 确认可构建；
- **限制**：无 push、无浅/部分克隆、无策略覆盖。
- **后端细节**：
  - Clone/Fetch 分别在独立工作线程执行，`TaskRegistry` 通过 `std::thread::spawn` 搭配 `CancellationToken` 协调取消；
  - 进度换算：Checkout 阶段在 `default_impl/ops.rs::do_clone` 中将 git2 的 0-100 线性映射到全局 90-100（90 + percent * 0.1），保持前端进度条平滑；
  - 错误映射集中在 `default_impl/errors.rs`，方便后续阶段扩展错误分类而不影响调用方；
  - HTTP Fake API 通过 `reqwest` 自行发起请求，与 git2 传输栈隔离。
- **前端配合**：`src/api/tasks.ts` 中的事件归一化函数需处理 MP0 仍然包含的 snake_case 字段（`total_hint`），在 MP1+ 中继续沿用；
- **交接要点**：
  - 如需回滚到 gitoxide 版本，可使用 `src-tauri/_archive/default_impl.legacy_*` 中的旧实现，编译时需重新启用 `gix` 相关依赖（非推荐，仅供紧急回退）。
  - 修改 git2 版本前务必在 Windows + macOS 双平台运行 `cargo test -q` 验证动态库加载。
- **交接 checklist**：
  - 对照 `new-doc/MP0_IMPLEMENTATION_HANDOFF.md` §2 的代码结构，确认 `core/git/default_impl.rs`、`core/tasks/registry.rs` 的负责人，并在交接记录中注明。
  - 每次发版前手动执行 `pnpm test -s` 与 `cargo test -q`；同时用 `http_fake_request` 校验白名单与脱敏日志，确保调试工具保持可用。
  - 若需要临时回滚到 gitoxide 版本，提前验证 `_archive/default_impl.legacy_*` 分支仍能通过最小 smoke，避免紧急恢复时缺乏可用包。

### 4.2 MP1 - Push + Subtransport + Retry + 事件增强

- **新增能力**：
  - `git_push` 命令与 Push 任务，凭证回调支持用户名/密码（PAT）；
  - Push 进度阶段化 `PreUpload|Upload|PostReceive`，进度事件含对象/字节；
  - `task://error` 引入，分类 `Network|Tls|Verify|Protocol|Proxy|Auth|Cancel|Internal`；
  - Retry v1：指数退避 + 抖动，`Upload` 阶段后不自动重试；
  - 自定义 smart subtransport 方式A：接管连接/TLS/SNI，代理场景自动禁用，Fake->Real->libgit2 回退链；
  - Push 特化：TLS 层注入 Authorization，401 -> Auth 分类，403 info/refs 阶段触发一次性 SNI 轮换；
- **配置热加载**：
  - `http.fakeSniEnabled`、`http.fakeSniHosts`、`http.sniRotateOn403`；
  - `retry.max`/`baseMs`/`factor`/`jitter`；
  - `proxy.mode|url`；`logging.debugAuthLogging`（脱敏开关）；
- **前端改动**：
  - GitPanel 支持 Push 表单（凭证、TLS/SNI 策略编辑）；
  - 全局错误列显示分类 Badge + 重试次数；
- **回退策略**：配置禁用 Push/方式A/Retry；方式A 失败自动回退 Real/libgit2；
- **测试**：Push 集成测试、事件 casing 测试（snake/camel 兼容），Retry 指数退避测试。
- **后端细节**：
  - Push 使用 `RemoteCallbacks::credentials` 结合 Tauri 命令提供的用户名/密码，回调在每次授权失败时重试；
  - `push_transfer_progress` 回调仅在服务器支持时触发，若无该回调则通过 libgit2 上传阶段对象/字节推估进度；
  - Retry v1 位于 `TaskRegistry::run_with_retry`，根据 `ErrorCategory` 判定是否 `is_retryable`，重试间隔计算函数在 `retry/backoff.rs`，使用 `rand` 抖动避免雪崩；
  - 方式A 的重写逻辑 `transport/rewrite.rs` 仅改写 `https://` 前缀，保留查询参数与 fragment；`transport/runtime.rs` 按代理模式决定是否禁用 Fake；
  - Authorization 注入通过线程局部 `AUTH_HEADER_OVERRIDE` 在 TLS 层读取，确保不会泄漏到非 Push 请求。
- **前端/服务配合**：
  - `src/stores/tasks.ts` 中的 `setLastError` 兼容 `retried_times` 与 `retriedTimes`；
  - Push 表单存储的凭证仅保存在内存，取消或完成后主动清空，避免残留。
- **交接要点**：
  - 若需在后续阶段扩展 Push 认证方式（如 OAuth 设备码），需扩展 `CredentialProvider` 接口并更新 Tauri 命令签名；
  - 方式A 白名单可在 `app config` 中追加域名，但必须同步更新 `tls.sanWhitelist`，否则会触发 Verify 错误；
  - Retry 参数调整需同时更新前端提示文本，保持用户对重试次数与耗时的认知。
- **关键文件定位**：
  - 传输层：`src-tauri/src/core/git/transport/rewrite.rs`（改写决策）、`transport/runtime.rs`（Fake/Real 回退）、`transport/streams.rs`（方式A IO 桥接）、`transport/auth.rs`（Authorization 注入）。
  - 任务与重试：`src-tauri/src/core/tasks/registry.rs`（`run_with_retry`、事件发射）、`src-tauri/src/core/tasks/model.rs`（任务/事件载荷结构）。
  - 前端落点：`src/views/GitPanel.vue`（Push 表单、TLS/SNI 编辑）、`src/views/__tests__/git-panel.error.test.ts`（错误列校验）。
- **事件契约补充**：
  - `task://error` 的 `message` 以人类可读文本呈现，`code` 预留（Retry v1 暂未使用）；`retriedTimes` 仅在自动重试后出现，Push 上传阶段不再自增。
  - Push 进度事件固定 `phase` 顺序 `PreUpload -> Upload -> PostReceive`，当服务器不提供回调时 `objects`/`bytes` 可能缺失。
  - 方式A 失败回退会通过一次性 `task://error` 将原因标记为 `Proxy` 或 `Tls`，便于前端展示提示。
- **常见故障排查**：
  - Push 401/403：检查 PAT/组织 SSO；Inspect `task://error` `category=Auth`，必要时启用 `logging.debugAuthLogging`（仍脱敏）。
  - TLS/Verify 失败：确认 `tls.sanWhitelist` 与 Fake SNI 白名单匹配；代理模式下默认禁用 Fake。
  - 进度停滞：若 Upload 阶段长时间无变化，提示手动取消并重试；事件中 `retriedTimes` 不再增长属于预期表现。
- **交接 checklist**：
  - 评估 Push/方式A rollout 前，在预生产逐项验证：正常 Push、401/403、代理透传、取消路径、Retry 关闭/开启效果。
  - 更新 `new-doc/MP1_IMPLEMENTATION_HANDOFF.md` 中的白名单与凭证说明，确保 SRE 拥有最新连接策略。
  - 前端确认 `GitPanel` 凭证输入不落盘，并在发布说明中告知新事件字段，便于文档同步。

### 4.3 P2 - 本地操作与策略扩展

- **目标**：
  - 引入常用本地 Git 操作（commit/branch/checkout/tag/remote），与任务系统保持一致的事件语义；
  - 支持 Shallow/Partial 克隆与 per-task 策略覆盖，为后续策略实验提供接口；
  - 增强护栏与信息事件，避免误配置导致的不可预期行为。
- **关键实现**：
  - 新增命令 `git_commit`、`git_branch`、`git_checkout`、`git_tag`、`git_remote_add|set|remove`，在 `core/tasks` 中落地为对应 TaskKind；
  - Clone/Fetch 支持 `depth` 与 `filter` 参数，决策枚举 `DepthFilterDecision` 负责记录 fallback 结果；
  - `strategy_override.rs` 解析任务级覆盖，统一 camel/snake casing，并输出 `ParsedOverride`；
  - 护栏规则在 `apply_*_override.rs` 中执行，冲突/忽略立即通过 `task://error` 信息事件上报；
  - Summary 事件 `strategy_override_summary` 聚合最终生效的 HTTP/TLS/Retry 策略及 `appliedCodes`。
- **配置与 gating**：
  - `strategyOverride` 支持白名单字段：HTTP (`followRedirects`、`maxRedirects`、`fakeSniEnabled?`、`fakeSniHosts?`)、Retry (`max`、`baseMs`、`factor`、`jitter`)、TLS (`insecureSkipVerify`、`skipSanWhitelist`)；
  - 环境变量 `FWC_PARTIAL_FILTER_SUPPORTED` 控制是否启用 Partial filter 能力，`FWC_STRATEGY_APPLIED_EVENTS` 控制是否发送 applied 事件；
  - 解析阶段会对数值做上限裁剪（如 `baseMs`、`max`），并允许 `max=0` 表示禁用自动重试。
- **测试矩阵**：
  - `git_clone_partial_filter.rs`、`git_fetch_partial_filter.rs` 覆盖 capability 与 fallback 场景；
  - `git_strategy_and_override.rs` 覆盖 HTTP/Retry/TLS 组合、冲突与 ignored 事件；
  - `git_tag_and_remote.rs`、`git_branch_and_checkout.rs` 覆盖本地操作 happy/edge/cancel；
  - `quality/error_and_i18n.rs` 属性测试验证策略解析边界与国际化映射。
- **后端细节**：
  - `ParsedOverride` 记录 `ignored_top` 与 `ignored_nested`，用于在信息事件中准确回显未知字段；
  - HTTP 覆盖通过 `apply_http_override.rs` 与全局配置进行浅合并，保留未覆盖字段；
  - TLS 覆盖在 `apply_tls_override.rs` 将互斥开关重置到安全值，并记录 `strategy_override_conflict`；
  - `partial_filter_support.rs` 引入 capability cache，以远端 URL 为键，减少重复探测；
  - 本地操作在写引用前进行取消检查，确保半成品不会落盘。
- **数据模型与事件顺序**：
  - `ParsedOverride` 会在解析阶段返回 `ParsedOverride::empty()` 以避免空对象触发多余事件；只有字段发生变化才追加 `*_override_applied`。
  - `DepthFilterDecision` 统一描述 shallow/partial fallback，事件 `partial_filter_fallback` 的 `decision` 字段匹配该枚举，测试中锁定顺序。
  - `strategy_override_summary` 聚合 `appliedCodes`（如 `http_strategy_override_applied`），`finalStrategy` 字段按 HTTP/TLS/Retry 分层，供前端展示。
- **护栏策略**：
  - HTTP 覆盖限制 `maxRedirects` 范围 0-10，若与 `followRedirects=false` 冲突则写入 `strategy_override_conflict` 并回落到安全值。
  - TLS 覆盖禁止修改 `sanWhitelist` 列表，仅允许开关 `insecureSkipVerify` 与 `skipSanWhitelist`；任何未知字段写入 `strategy_override_ignored_fields`。
  - Retry 覆盖允许 `max=0` 表示禁用自动重试，前端需提示用户；过大 `baseMs` 会被截断并记入 Summary。
- **常见故障排查**：
  - 出现 `strategy_override_conflict` 时，参考事件 `message` 提供的修正值；若持续冲突，检查 UI 是否传入互斥字段组合。
  - `partial_filter_fallback` 多次出现意味着目标远端不支持 partial；可设置 `FWC_PARTIAL_FILTER_SUPPORTED=0` 关闭探测以减少噪声。
  - 本地操作失败常见为 ref 已存在或缺失 `force` 标记，错误分类为 `Protocol`，在 `task://error` `message` 中包含原始 git2 文本。
- **交接 checklist**：
  - 与前端约定新增策略字段的字段名、默认值与展示顺序，更新 `GitPanel` 表单与校验 schema。
  - 在 `git_strategy_and_override.rs` 中为每个新增事件补充 DSL 断言，防止遗漏。
  - 落实运维手册：记录 `FWC_PARTIAL_FILTER_SUPPORTED`、`FWC_STRATEGY_APPLIED_EVENTS` 的默认值与改动流程，确保事故响应时有明确参考。
- **前端/服务配合**：
  - `src/views/GitPanel.vue` 支持策略编辑面板，提交前使用 `zod` 校验字段；
  - Task store 继续兼容 `retried_times` / `retriedTimes`，并在全局日志展示策略信息事件；
  - UI 上的 Summary 展示依赖 `appliedCodes`，保持与后端事件缩写一致。
- **交接要点**：
  - 新增策略字段时需同步更新：`ParsedOverride`、护栏规则、Summary 序列化、前端编辑器、事件 DSL 与测试；
  - 引入新的 capability provider（如企业镜像）必须实现 `supports_partial_filter` 接口并更新缓存键；
  - 回退策略：可逐项移除 TaskKind，或关闭 `strategyOverride` 解析模块，整体回落到全局配置。

### 4.4 P3 - 自适应 TLS 与可观测性强化

- **目标**：
  - 将方式A 自适应 TLS 全量 rollout，并提供可观测性与自动护栏；
  - 强化指纹、时延与回退事件，便于运维审计；
  - 引入 Soak 报告确保长时运行稳定性。
- **关键实现**：
  - `transport/rewrite.rs` 执行 Fake SNI 改写与采样决策，记录 `AdaptiveTlsRollout` 事件；
  - `transport/runtime.rs` 维护 Fake->Real->Default fallback 状态机，并触发 `AdaptiveTlsFallback`；
  - `transport/metrics.rs` 的 `TimingRecorder` 与 `fingerprint.rs` 的日志逻辑在任务结束时统一 flush 事件；
  - 自动禁用窗口根据失败率触发 `AdaptiveTlsAutoDisable`，冷却后自动恢复；
  - Soak runner (`src-tauri/src/soak`) 以环境变量驱动迭代运行并生成报告。
- **配置与指标**：
  - 关键项：`http.fakeSniEnabled`、`http.fakeSniRolloutPercent`、`http.autoDisableFakeThresholdPct`、`http.autoDisableFakeCooldownSec`、`tls.metricsEnabled`、`tls.certFpLogEnabled`、`tls.spkiPins`、`tls.realHostVerifyEnabled`；
  - 指纹日志写入 `cert-fp.log`，滚动阈值由 `tls.certFpMaxBytes` 控制；
  - 环境变量：`FWC_TEST_FORCE_METRICS` 强制指标采集，`FWC_ADAPTIVE_TLS_SOAK` 和 `FWC_SOAK_*` 控制 soak。
- **测试矩阵**：
  - `transport/rewrite.rs` 单测覆盖 0%/10%/100% 采样与 URL 处理；
  - `transport/runtime.rs` 单测验证 fallback 状态机与 auto disable；
  - `tls/verifier.rs` 单测覆盖 Real host 校验与 SPKI pin，`events/events_structure_and_contract.rs` 锁定事件 schema；
  - Soak 模块单测确保报告生成、基线对比与阈值判定。
- **后端细节**：
  - `RewriteDecision` 使用 host + path 的稳定哈希决定 `sampled`，同一仓库在不同任务中行为一致；
  - `TimingRecorder` 捕获 connect_ms/tls_ms/first_byte_ms/total_ms（毫秒），并在任务完成时产生单一 `AdaptiveTlsTiming` 事件；
  - 指纹缓存 LRU 记录 512 个 host，24 小时内变化才会触发 `CertFingerprintChanged`；
  - 自动禁用窗口 `SAMPLE_CAP=20`，最少样本 `MIN_SAMPLES=5`，触发后立即清空窗口并记录 `enabled=false/true` 两个事件；
  - Real host 验证失败视为 Verify 类错误，同时触发 Fake->Real fallback 统计。
- **模块映射与代码指针**：
  - `transport/metrics.rs`（TimingRecorder）、`transport/fingerprint.rs`（指纹日志）、`transport/fallback.rs`（状态机）、`transport/runtime.rs`（自动禁用/状态协调）。
  - TLS 验证集中在 `src-tauri/src/core/tls/verifier.rs`：同时负责 Real host 与 SPKI pin；测试样例位于同路径 `tests` 模块。
  - Soak 入口 `src-tauri/src/soak/mod.rs`，报告结构与基线比对实现位于同目录；`soak/README.md` 列出运行参数。
- **事件 & 指标字段细化**：
  - `AdaptiveTlsRollout` 字段：`percent_applied`、`sampled`、`eligible`，用于确认采样策略是否命中；搭配监控可量化 rollout 覆盖面。
  - `AdaptiveTlsTiming` 仅在 `tls.metricsEnabled=true` 时发送；`cert_fp_changed=true` 表示 24 小时窗口内指纹发生更新。
  - `AdaptiveTlsAutoDisable` 在触发与恢复时分别发送一次，`enabled=false` 表示 Fake SNI 被暂停；结合日志可定位原因。
- **常见故障排查**：
  - Fake 回退频繁：查看 `AdaptiveTlsFallback` 中的 `reason`，若为 `FakeHandshakeError`，多为目标域证书/SAN 变更；先同步白名单再恢复 rollout。
  - 指纹 mismatch：事件 `CertFpPinMismatch` 出现后应立即核对 `tls.spkiPins`；如误配导致大面积失败，临时清空 pin 后重新采集。
  - 自动禁用 oscillation：检查失败率阈值是否过低，或 Soak 报告中是否存在网络抖动；必要时提高 `autoDisableFakeThresholdPct`。
- **交接 checklist**：
  - 发布前确认 `cert-fp.log` 滚动机制与磁盘配额，必要时在运维标准中加入归档脚本。
  - 调整 rollout 百分比时，更新监控告警阈值并记录在 `new-doc/P3_IMPLEMENTATION_HANDOFF.md` 交接表。
  - Soak 报告归档到 `doc/` 目录并附在发布邮件，确保后续回溯有依据。
- **前端/服务配合**：
  - GitPanel 监听 `AdaptiveTls*` 信息事件并折叠展示关键字段，未开启 UI 时仍可在全局日志查看；
  - 指纹事件在 UI 中标记为敏感，仅显示哈希前缀；
  - Soak 报告默认输出至项目根目录，可在运维脚本中采集并上报。
- **交接要点**：
  - 调整 rollout 百分比需同步更新监控告警阈值，推荐 0 -> 25 -> 50 -> 100 渐进策略；
  - 新增 SPKI pin 前需先通过 `cert-fp.log` 获取现有指纹，避免误配置导致 Verify 失败；
  - Auto disable 阈值或冷却时间变化后请执行短程 soak，确认不会反复触发；
  - 指纹日志与 soak 报告包含敏感信息，导出前确保 `logging.authHeaderMasked` 与脱敏策略开启。
- **回退策略**：配置层可关闭 Fake SNI（`fakeSniEnabled=false` 或 rollout=0）、关闭指标采集（`metricsEnabled=false`）、清空 `tls.spkiPins` 或调高 auto disable 阈值，必要时可回退到 `src-tauri/src/core/git/transport/_archive` 中的 legacy 实现。

### 4.5 测试重构 - 统一验证体系

- **目录布局**：`src-tauri/tests` 现按主题聚合——`common/`（共享 DSL 与 fixtures）、`git/`（Git 语义）、`events/`、`quality/`、`tasks/`、`e2e/`；每个聚合文件控制在 800 行内，新增用例优先追加至现有 section。
- **公共模块**：`common/test_env.rs`（全局初始化）、`fixtures.rs` 与 `repo_factory.rs`（仓库构造）、`git_scenarios.rs`（复合操作）、`shallow_matrix.rs`/`partial_filter_matrix.rs`/`retry_matrix.rs`（参数矩阵），确保相同语义只实现一次。
- **事件 DSL**：`common/event_assert.rs` 提供 `expect_subsequence`、`expect_tags_subsequence`、策略/TLS 专用断言；测试通过 Tag 序列或结构化辅助降低脆弱度，所有策略/TLS 事件均已接入。
- **属性测试与回归种子**：集中在 `quality/error_and_i18n.rs`，按 `strategy_props`、`retry_props`、`partial_filter_props`、`tls_props` 分 section；`prop_tls_override.proptest-regressions` 保存最小化案例，遵循附录 B SOP 定期清理。
- **指标与质量监测**：重构后维护若干基线指标（单文件行数、关键词出现次数、属性测试执行时间），通过 PowerShell 脚本或 `wc -l` 快速检查，防止回归到碎片化结构。
- **新增用例流程**：
  1. 选择合适聚合文件并引用 `test_env::init_test_env()`；
  2. 若覆盖新参数维度，先在当前文件内定义 case 枚举；只有 ≥2 文件复用时才上移 `common/`；
  3. 针对策略/TLS/事件，使用 DSL 子序列断言而非硬编码完整列表；
  4. 需要属性测试时，在 `quality/error_and_i18n.rs` 新建 section，并在生成器中实现 `Display` 方便调试；失败案例写入 seed 文件尾部。
- **交接 checklist**：
  - 新功能合入前检查对应聚合文件行数与 DSL 覆盖，必要时拆分 section 或抽象 helper；
  - 在 PR 模板中勾选“更新测试 DSL/矩阵”项，避免忘记同步；
  - 发布前运行 `cargo test -q` 与 `pnpm test -s`，若属性测试时间 >5s 需调查生成器是否退化。

## 5. 交接与发布 checklist 概览

- **文档同步**：合并前核对 `new-doc/MP*_IMPLEMENTATION_HANDOFF.md`、`new-doc/IMPLEMENTATION_OVERVIEW_MP0-P3.md` 与 `doc/TESTS_REFACTOR_HANDOFF.md` 的版本号与变更记录，一并更新 `CHANGELOG.md`。
- **配置审计**：按阶段确认环境变量与 AppConfig 值：MP1（Fake SNI 白名单、Retry 阈值）、P2（`FWC_PARTIAL_FILTER_SUPPORTED`、`FWC_STRATEGY_APPLIED_EVENTS`）、P3（rollout 百分比、auto disable 阈值、SPKI pin 列表）。
- **灰度计划**：在发布计划中记录灰度顺序与回退手段（参考 §3.5），确保 SRE 获得监控告警阈值与事件观察面。
- **测试执行**：要求在主干合并前执行 `cargo test -q`、`pnpm test -s`，必要时附加 soak 报告；若涉及传输层改动，附上目标域指纹快照。
- **运维交接**：提供最新 `cert-fp.log` 样例、策略 Summary 截图与 `task_list` 正常输出，帮助运维确认运行状态；新事件字段需同步到监控解析脚本。
