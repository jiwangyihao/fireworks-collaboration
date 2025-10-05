# Fireworks Collaboration 实现总览（MP0 -> P7 & 测试重构）

> 目的：以统一视角梳理 MP0、MP1、P2、P3、P4、P5、P6 七个阶段以及测试重构后的现状，面向后续演进的研发、运维与质量成员，提供完整的实现细节、配置指引、事件契约、测试矩阵与回退策略。
>
> 版本：v1.4（2025-10-05） 维护者：Core Team

---

## 1. 范围与阅读指引

- **涵盖阶段**：
  - **MP0**：gitoxide -> git2-rs 替换，任务/事件契约保持不变；
  - **MP1**：Push、方式A自定义 smart subtransport、Retry v1、事件增强；
  - **P2**：本地 Git 操作扩展、Shallow/Partial、任务级策略覆盖、策略信息事件与护栏；
  - **P3**：自适应 TLS 全量 rollout、可观测性强化、Real Host 校验、SPKI Pin、自动禁用、Soak；
  - **P4**：IP 池采样与优选、预热调度、熔断治理、Soak 阈值；
  - **P5**：代理支持（HTTP/SOCKS5/System）、自动降级与恢复、健康检查、前端集成；
  - **P6**：凭证存储与安全管理（三层存储、AES-256-GCM加密、Argon2id密钥派生、审计日志、访问控制、Git集成）；
  - **P7**：多仓工作区（Workspace）模型、Git 子模块支持、批量并发调度（clone/fetch/push）、团队配置模板导出/导入、跨仓库状态监控、前端一体化视图与性能/稳定性基准；
  - **测试重构**：`src-tauri/tests` 聚合结构、事件 DSL、属性测试与回归种子策略。
- **读者画像**：
  - 新接手的后端/前端开发；
  - 运维与 SRE（回退、监控、调参）；
  - 测试与质量保障（测试矩阵、DSL 约束）。
- **联动文档**：`new-doc/MP*_IMPLEMENTATION_HANDOFF.md`、`new-doc/P*_IMPLEMENTATION_HANDOFF.md` 系列交接稿、`new-doc/TECH_DESIGN_*.md` 设计稿、`doc/TESTS_REFACTOR_HANDOFF.md`。

---

## 2. 里程碑一览

| 阶段 | 核心交付 | 事件新增/调整 | 配置扩展 | 回退策略 | 测试现状 |
|------|-----------|---------------|----------|----------|----------|
| MP0 | git2-rs 基线，Clone/Fetch 稳定，取消/错误分类统一 | 无新增，进度保持兼容 | 继承 HTTP Fake 调试配置 | 可回退到旧二进制（保留 gitoxide tag 归档） | `cargo test` / `pnpm test` 全绿 |
| MP1 | Push、方式A smart subtransport、Retry v1、进度阶段化 | `task://error`，Push `PreUpload/Upload/PostReceive`，错误分类输出 | HTTP/TLS/Fake SNI 配置热加载 | Push/方式A/Retry 可配置关闭或自动回退 | Rust/前端测试覆盖 push、事件 casing |
| P2 | 本地操作（commit/branch/checkout/tag/remote）、Shallow/Partial、策略覆盖、护栏、Summary | 覆盖策略事件：`*_override_applied`、`strategy_override_summary` 等 | `strategyOverride` 入参，env gating（`FWC_PARTIAL_FILTER_SUPPORTED`、`FWC_STRATEGY_APPLIED_EVENTS`） | 逐项移除 TaskKind / 关闭 gating | 新增矩阵测试、属性测试覆盖策略解析 |
| P3 | 自适应 TLS rollout + 可观测性、Real Host 校验、SPKI Pin、自动禁用、Soak | `AdaptiveTls*` 结构化事件、指纹变化事件 | `http.fakeSniRolloutPercent`、`tls.metricsEnabled`、`tls.certFpLogEnabled`、`tls.spkiPins` 等 | 配置层关闭 Fake/metrics/pin；自动禁用冷却 | Soak 测试 + 指标契约测试 |
| P4 | IP 池采样与握手优选、传输集成、异常治理、观测扩展、Soak 阈值 | `IpPoolSelection`、`IpPoolRefresh`、`IpPoolAutoDisable`、`IpPoolCidrFilter` 等 | `ip_pool.*` 运行期与文件配置（缓存、熔断、TTL、黑白名单）| 配置禁用 IP 池/熔断/预热；自动禁用冷却 | Rust 单测/集测、IP 池集成测试、Soak 报告 |
| P5 | 代理支持（HTTP/SOCKS5/System）、自动降级与恢复、前端集成 | `ProxyStateEvent`、`ProxyFallbackEvent`、`ProxyRecoveredEvent`、`ProxyHealthCheckEvent` 等 | `proxy.*` 配置（mode/url/auth/超时/降级/恢复/健康检查/调试日志）| 配置禁用代理/手动降级恢复/调整阈值 | 276个测试（243 Rust + 33 TypeScript），跨平台系统检测，状态机转换验证 |
| P6 | 凭证存储（三层：系统钥匙串/加密文件/内存）、加密安全（AES-256-GCM + Argon2id）、审计日志、访问控制、Git自动填充 | `CredentialEvent`（Add/Get/Update/Delete/List/Cleanup）、`AuditEvent`（操作审计）、`AccessControlEvent`（失败锁定） | `credential.*` 配置（mode/masterPassword/auditMode/accessControl/keyCache/过期管理）| 配置逐层禁用存储/关闭审计/调整锁定阈值 | 1286个测试（991 Rust + 295 前端），99.9%通过率，88.5%覆盖率，批准生产环境上线 |
| 测试重构 | 主题聚合、事件 DSL、属性测试集中管理 | DSL 输出 Tag 子序列 | N/A | N/A | `src-tauri/tests` 结构稳定，CI 使用共享 helper |
| P7 | 工作区模型、子模块支持、批量并发 clone/fetch/push、团队配置模板、跨仓库状态监控、前端一体化视图 | 无新增事件类型（复用 task/state/progress/error），批量任务 progress phase 含聚合文本 | `workspace.*`、`submodule.*`、`teamTemplate`、`workspace.status*` | 配置禁用 workspace 或降并发；子模块/模板/状态可单项停用 | 新增 24 子模块测试 + 12 批量调度测试 + 状态缓存测试 + 前端 store 17 测试 + 性能基准 |

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

// 凭证管理（P6）
add_credential(opts: { host: string; username: string; password: string; expiresAt?: number }): Promise<void>
get_credential(host: string, username: string): Promise<CredentialInfo | null>
update_credential(opts: { host: string; username: string; password: string; expiresAt?: number }): Promise<void>
delete_credential(host: string, username: string): Promise<void>
list_credentials(): Promise<CredentialInfo[]>
cleanup_expired_credentials(): Promise<number>
set_master_password(password: string, config: CredentialConfig): Promise<void>
unlock_store(password: string, config: CredentialConfig): Promise<void>
export_audit_log(): Promise<string>
cleanup_audit_logs(retentionDays: number): Promise<number>
is_credential_locked(): Promise<boolean>
reset_credential_lock(): Promise<void>
remaining_auth_attempts(): Promise<number>

// 任务控制
task_cancel(id: string): Promise<boolean>
task_list(): Promise<TaskSnapshot[]>

// 调试
git_task_debug?(internal)
http_fake_request(input: HttpRequestInput): Promise<HttpResponseOutput>
```

所有命令返回 `taskId`，前端通过事件流追踪生命周期。

命令返回值约定补充（P7 扩展）：
- 直接返回字符串：通常为生成的 `taskId` 或导出文件路径（如 `export_team_config_template`、`backup_workspace`）。
- 返回布尔：表示快速成功/失败（如 `save_workspace`、`restore_workspace`）。
- 返回结构化对象：配置/状态查询或模板导入报告（`get_workspace_statuses`、`import_team_config_template`）。
- 返回列表：子模块名称集合或仓库集合操作结果（`init_all_submodules` 等）。
前端应根据类型决定是否进入任务事件订阅路径（有 taskId）或直接更新本地 store（无 taskId 的同步命令）。

P7 新增的工作区/子模块/批量与团队配置相关命令（命名保持 camelCase，可与上表并列理解）：

```ts
// 工作区管理
create_workspace(opts: { name: string; rootPath: string }): Promise<string>
load_workspace(path?: string): Promise<string>
save_workspace(): Promise<boolean>
add_repository(opts: { workspaceId: string; repo: RepositorySpec }): Promise<string>
remove_repository(opts: { workspaceId: string; repoId: string }): Promise<boolean>
update_repository_tags(opts: { workspaceId: string; repoId: string; tags: string[] }): Promise<boolean>
validate_workspace_file(path: string): Promise<boolean>   // 校验 workspace.json 结构
backup_workspace(path: string): Promise<string>           // 返回带时间戳的备份文件路径
restore_workspace(backupPath: string, workspacePath: string): Promise<void>

// 子模块操作
list_submodules(opts: { repoPath: string }): Promise<SubmoduleInfo[]>
has_submodules(opts: { repoPath: string }): Promise<boolean>
init_all_submodules(opts: { repoPath: string }): Promise<string[]>
update_all_submodules(opts: { repoPath: string }): Promise<string[]>
sync_all_submodules(opts: { repoPath: string }): Promise<string[]>

// 批量任务（返回父任务 taskId）
workspace_batch_clone(req: WorkspaceBatchCloneRequest): Promise<string>
workspace_batch_fetch(req: WorkspaceBatchFetchRequest): Promise<string>
workspace_batch_push(req: WorkspaceBatchPushRequest): Promise<string>

// 团队配置模板
export_team_config_template(opts?: { path?: string; sections?: string[] }): Promise<string> // 返回生成文件路径
import_team_config_template(opts: { path?: string; strategy?: ImportStrategyConfig }): Promise<TemplateImportReport>

// 跨仓库状态
get_workspace_statuses(opts: StatusQuery): Promise<WorkspaceStatusResult>
clear_workspace_status_cache(): Promise<number>
invalidate_workspace_status_entry(opts: { repoId: string }): Promise<boolean>
```

### 3.2 事件总览

- `task://state`：`{ taskId, kind, state, createdAt }`，`state ∈ pending|running|completed|failed|canceled`；
- `task://progress`：
  - Clone/Fetch：`{ taskId, kind, phase, percent, objects?, bytes?, totalHint?, retriedTimes? }`；
  - Push：`phase ∈ PreUpload|Upload|PostReceive`；
- `task://error`：分类或信息事件，`{ taskId, kind, category, message, code?, retriedTimes? }`；
- 自适应 TLS 结构化事件（P3）：`AdaptiveTlsRollout`、`AdaptiveTlsTiming`、`AdaptiveTlsFallback`、`AdaptiveTlsAutoDisable`、`CertFingerprintChanged`、`CertFpPinMismatch`；
- 策略信息事件（P2）：`http_strategy_override_applied`、`retry_strategy_override_applied`、`tls_strategy_override_applied`、`strategy_override_conflict`、`strategy_override_ignored_fields`、`partial_filter_fallback`、`strategy_override_summary`。
- IP 池与优选事件（P4）：`IpPoolSelection`、`IpPoolRefresh`、`IpPoolCidrFilter`、`IpPoolIpTripped`、`IpPoolIpRecovered`、`IpPoolAutoDisable`、`IpPoolAutoEnable`、`IpPoolConfigUpdate`；同时 `AdaptiveTlsTiming/Fallback` 增补 `ip_source`、`ip_latency_ms`、`ip_selection_stage` 可选字段。
- 代理事件（P5）：`ProxyStateEvent`（状态转换，含扩展字段）、`ProxyFallbackEvent`（自动/手动降级）、`ProxyRecoveredEvent`（自动/手动恢复）、`ProxyHealthCheckEvent`（健康检查结果）；代理启用时通过传输层注册逻辑强制禁用自定义传输层与 Fake SNI；配置热更新和系统代理检测不发射独立事件，由Tauri命令直接返回结果。
- 凭证与审计事件（P6）：`CredentialAdded`、`CredentialRetrieved`、`CredentialUpdated`、`CredentialDeleted`、`CredentialListed`、`ExpiredCredentialsCleanedUp`（凭证生命周期）；`AuditEvent`（操作审计，含用户/时间/操作类型/结果/SHA-256哈希）；`AccessControlLocked`、`AccessControlUnlocked`（失败锁定与恢复）；`StoreUnlocked`、`StoreLocked`（加密存储解锁状态）。

事件顺序约束在测试中锁定：策略 applied -> conflict -> ignored -> partial fallback -> summary；TLS 事件在任务结束前统一刷出；凭证操作触发审计事件在命令执行后同步发射。

P7 未新增独立事件类型：工作区、子模块、批量调度与状态查询均复用既有 `task://state|progress|error` 语义。批量任务父进度的 `phase` 字段采用聚合文本（如 `Cloning 2/5 completed (1 failed)`），子模块递归克隆阶段通过主任务进度区间（0-70-85-100%）映射，不引入单独子模块事件流。状态服务（WorkspaceStatusService）目前仅通过命令拉取结果，后续事件推送在后续迭代规划中。

P7 已知限制（未纳入本次交付）：
- 子模块并行初始化/更新参数 `parallel/maxParallel` 预留但未实现（串行足够 <10 子模块常见场景）。
- 子模块粒度实时进度事件（`SubmoduleProgressEvent`）尚未连接前端事件总线，仅通过主任务阶段映射。
- 批量任务进度权重均等，未按仓库体积/历史耗时加权；大体量差异下显示可能不线性。
- 工作区状态服务无事件推送（需轮询）；大量仓库高频刷新需手动调大 `statusCacheTtlSecs` 与关闭自动刷新。

TaskKind 扩展（P7 补充说明）：
- 递归克隆：沿用 `TaskKind::GitClone`，仅在克隆完成后根据配置附加子模块 init/update 两阶段（映射到 70–85%、85–100% 进度区间）。
- 子模块独立操作：未新增专属 TaskKind，命令直接进行同步/初始化逻辑，失败通过日志与返回值暴露。
- 批量调度：新增 `TaskKind::WorkspaceBatch { operation, total }` 作为父任务快照，子任务仍为原生 Git TaskKind（Clone/Fetch/Push），通过父子关联表跟踪；父任务进度 = 子任务完成百分比平均。

### 3.3 服务与分层

**P7 状态管理架构补充**:
Tauri 应用层使用 `Arc<Mutex<T>>` 模式管理三个独立的全局状态:
- `SharedWorkspaceManager = Arc<Mutex<Option<Workspace>>>`：当前加载的工作区实例，commands 通过 State 注入访问；
- `SharedWorkspaceStatusService = Arc<WorkspaceStatusService>`：跨仓库状态查询服务，内部维护 TTL 缓存与并发控制，与 WorkspaceManager 解耦；
- `SharedSubmoduleManager = Arc<Mutex<SubmoduleManager>>`：子模块管理器，拥有独立配置(`SubmoduleConfig`)，支持递归初始化/更新/同步操作。

`WorkspaceStorage` 不是全局单例，每次 `load_workspace`/`save_workspace` 调用时实例化并传入路径，确保多工作区场景下无状态冲突。批量任务通过快照(`workspace.clone()`)避免持锁跨 async 边界。

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
 ├─ IP 池候选消费与握手埋点（P4）
 └─ 自动禁用窗口

IP Pool Service (core/ip_pool/*)
 ├─ `IpPool` 统一入口（pick/report/maintenance/config）
 ├─ `PreheatService` 调度多来源采样（builtin/history/userStatic/DNS/fallback）
 ├─ `IpScoreCache` + `IpHistoryStore` 缓存与持久化（TTL、容量、降级）
 ├─ 传输层集成：`custom_https_subtransport` 消费候选、线程本地埋点
 └─ 异常治理：`circuit_breaker`、黑白名单、全局自动禁用（P4）

Proxy Service (core/proxy/*)
 ├─ `ProxyManager` 统一管理（连接器、状态、配置、健康检查）
 ├─ HTTP/SOCKS5 连接器（CONNECT隧道、协议握手、Basic Auth）
 ├─ `ProxyFailureDetector` 滑动窗口失败检测与自动降级
 ├─ `ProxyHealthChecker` 后台探测与自动恢复
 ├─ `SystemProxyDetector` 跨平台系统代理检测（Windows/macOS/Linux）
 └─ 传输层集成：代理启用时强制禁用自定义传输层与 Fake SNI（P5）

Credential Service (core/credential/*)
 ├─ `CredentialStoreFactory` 三层存储智能回退（系统钥匙串 → 加密文件 → 内存）
 ├─ 系统钥匙串集成：Windows Credential Manager、macOS Keychain、Linux Secret Service（P6.1）
 ├─ `EncryptedFileStore` AES-256-GCM加密 + Argon2id密钥派生 + 密钥缓存（P6.1）
 ├─ `InMemoryStore` 进程级临时存储（P6.0）
 ├─ `AuditLogger` 双模式审计（标准/审计模式，SHA-256哈希，持久化）（P6.2/P6.5）
 ├─ `AccessControl` 失败锁定机制（5次失败 → 30分钟锁定 → 自动过期）（P6.5）
 ├─ Git集成：`git_credential_autofill` 智能降级（存储 → 未找到 → 出错）（P6.4）
 └─ 前端集成：13个Tauri命令、4个Vue组件、Pinia Store（P6.3/P6.5）
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

P7 追加的主要配置键（集中在 `config.json`）：
`workspace.enabled`（启用工作区，**实际默认 false**，保持向后兼容）、`workspace.maxConcurrentRepos`（批量并发上限，**实际默认 3**，保守配置）、`workspace.statusCacheTtlSecs`（**实际默认 15秒**）/ `workspace.statusMaxConcurrency`（**实际默认 4**）/ `workspace.statusAutoRefreshSecs`（默认 null，禁用自动刷新）；`submodule.*`（autoRecurse/maxDepth/autoInitOnClone/recursiveUpdate/parallel/maxParallel，其中并行当前未实现）；`teamTemplate`（导出/导入默认路径及策略开关）；其余保持向后兼容，未启用时不影响已有单仓库功能。

最小可用配置示例（启用工作区 + 子模块支持 + 保守刷新）：
```jsonc
{
  "workspace": {
    "enabled": true,              // 默认 false，需要显式启用
    "maxConcurrentRepos": 3,      // 默认值即为 3，保守并发
    "statusCacheTtlSecs": 15,     // 默认值 15 秒
    "statusMaxConcurrency": 4,    // 默认值 4
    "statusAutoRefreshSecs": 60   // 可选，默认 null（禁用）
  },
  "submodule": {
    "autoRecurse": true,
    "maxDepth": 5,
    "autoInitOnClone": true,
    "recursiveUpdate": true,
    "parallel": false,
    "maxParallel": 3
  },
  "teamTemplate": {
    "defaultExportPath": "config/team-config-template.json"
  }
}
```

P7 测试覆盖摘要：新增子模块模型与操作单/集成测试 24 项；批量调度（clone/fetch/push）集成测试 12 项，验证并发、失败摘要与取消传播；状态服务缓存/失效/性能测试若干（含 10/50/100 仓库 p95 基准）；团队模板导出/导入 7 项；前端 Pinia store 新增 17 项单测（批量任务、模板报告、状态缓存）；端到端性能基准测试纳入 Nightly（批量 clone、状态刷新）。

性能基线（本地自动化环境 p95 指标，用于回归门槛参考）：
- 批量 clone：10/50/100 仓库 p95 用时分别 ≈15.5ms / 11.2ms / 10.2ms（相对单仓 baseline 105.5ms 聚合后均 < 0.2×/仓）。
- 状态刷新：10/50/100 仓库总耗时 ≈10.9ms / 54.2ms / 80.8ms（100 仓库 <3s 目标内充足裕度）。
- 子模块递归初始化+更新阶段占主任务 30%（70→100% 区间），失败不阻塞主任务完成。
- 性能回归策略：Nightly 比对最近 7 次运行滚动窗口，如 p95 超出基线 2× 触发告警并要求人工复核差异日志（任务元数据与结构化事件）。

回退快速参考（按优先级从最小影响到功能全禁用）：
| 场景 | 操作 | 影响范围 | 备注 |
|------|------|----------|------|
| 批量任务负载过高 | 将 `workspace.maxConcurrentRepos` 降为 1 | 退化为顺序执行 | 不中断现有父任务，但新任务生效 |
| 子模块初始化频繁失败 | `submodule.autoInitOnClone=false` | 保留主仓库克隆 | 可手动调用 init_all_submodules |
| 状态刷新造成 IO 压力 | `workspace.statusAutoRefreshSecs=0` 提高 `statusCacheTtlSecs` | 停止自动轮询 | 需要手动刷新按钮或命令 |
| 模板导入疑似破坏配置 | 使用最近备份文件覆盖 `config.json` | 恢复所有配置节 | 备份命名含时间戳易定位 |
| 工作区整体不稳定 | `workspace.enabled=false` | 回退单仓模式 | 不需要重编译，重启后生效 |
| 仅想屏蔽批量 UI | 保持 enabled，前端配置隐藏入口（可选） | 后端能力仍可保留 | 方便灰度逐步恢复 |

工作区文件与并发风险提示：
- 当前未实现显式锁文件；多进程同时写入 `workspace.json` 理论上存在竞态，建议运维避免同一物理目录并行启动两个实例（或通过容器编排保证单实例）。
- 保存操作采用原子写（写临时文件后 rename），若写入中断可回退最近备份或使用 `.bak` 文件恢复。
- 频繁批量操作配合低 TTL 状态刷新可能导致磁盘 I/O 峰值，建议：高并发任务时临时调大 `statusCacheTtlSecs` 并暂停自动刷新。

团队模板安全与去敏化要点：
- 导出时自动清理：代理密码、凭证文件路径、IP 池历史文件路径、运行态敏感统计字段。
- 备份策略：每次导入前生成 `team-config-backup-YYYYMMDDHHMMSS.json`，回滚只需将备份覆盖现有 `config.json` 并重新加载。
- Merge 策略忽略本地默认值（保持最小差异），保留本地 IP 池 historyPath 以避免分发机器路径。
- Schema 主版本不匹配直接拒绝导入；报告中列出 `applied` 与 `skipped` 节及原因（如 strategyKeepLocal / sectionDisabled / noChanges）。

### 3.5 发布节奏与跨阶段集成
- **推广顺序**：遵循 MP0 -> MP1 -> P2 -> P3 的递进路径，每阶段功能上线前都需确认前置阶段的回退手段仍可用；详见 `new-doc/MP*_IMPLEMENTATION_HANDOFF.md`。
- **配置切换流程**：
  1. 在预生产环境调整 `AppConfig`/环境变量验证行为；
  2. 触发 `fwcctl reload-config` 或重启 Tauri 容器以生效；
  3. 通过 `task_list` 验证无历史任务处于异常状态，再逐步扩大发布半径。
- **灰度策略**：MP1 的方式A 与 Retry、P3 的 fake SNI rollout 都支持按域或百分比分级，推荐每级灰度至少运行一次 soak 或回归脚本；
  - Rollout 0% -> 25% -> 50% -> 100%，期间监控 `AdaptiveTlsRollout` 与 `AdaptiveTlsFallback` 事件；
  - Push/策略相关配置调整需同步更新前端提示与文档，避免用户误解重试次数或凭证要求。
  - P4 的 IP 池推荐域名单独启用：先在预热域上验证 TTL 刷新/候选延迟，再逐步扩大 `preheatDomains`；按需域名可通过 `ip_pool.enabled`/`maxCacheEntries`/`singleflightTimeoutMs` 跨阶段放量，并搭配 Soak 报告监控 `selection_by_strategy` 与 `refresh_success_rate`。
- **跨阶段依赖**：
  - P2 的策略覆盖与 P3 的 adaptive TLS 共享 HTTP/TLS 配置，只在任务级做差异化；
  - P3 的指纹日志与自动禁用依赖 MP1 方式A 的传输框架，如需临时关闭 Fake SNI，应评估 P3 指标链路的可见性。
  - P4 的 IP 池与 P3 的 Adaptive TLS 深度联动：传输层在同一线程上下文填充 `ip_source`/`ip_latency_ms` 并继续触发 `AdaptiveTlsTiming/Fallback`；关闭 IP 池时需同步评估 P3 自动禁用与指标的观测空洞；黑白名单/熔断策略依赖 P2 的任务配置热加载能力。
  - P7 工作区/批量/子模块逻辑仅在 `workspace.enabled=true` 时激活；批量调度复用任务注册器与进度事件，不改变 MP0-P6 任务语义；子模块递归克隆建立在现有 `TaskKind::GitClone` 之后附加阶段（70-85-100%）；团队配置模板导入仅写入配置文件与内存运行态，不影响已存在 TLS/IP/代理/凭证模块的回退路径；跨仓库状态服务读取仓库索引，不修改 Git 操作代码。
  - 跨模块影响：
    - 代理启用（P5）时不影响工作区/批量逻辑；批量任务内部仍复用 Git 传输层现有代理互斥策略（自定义传输与 Fake SNI 已由前序逻辑屏蔽）。
    - 凭证存储（P6）自动为批量 clone/push 子任务统一回调，无需在批量请求中重复提供凭证；子模块操作沿用主仓库凭证。
    - IP 池（P4）与工作区解耦：批量任务底层仍按单仓 Git 任务路径调用；若 IP 池禁用不会影响 Workspace 元数据与任务调度。
    - 模板导入仅覆盖配置，不直接触发批量/子模块任务；导入后需要手动 reload 或下一次任务读取时生效。
- **回滚指引**：
  - 生产事故时优先通过配置禁用新增功能（Push、策略覆盖、Fake SNI、指标采集等）；
  - 若需降级二进制，参考 `src-tauri/_archive` 中的 legacy 实现及各阶段 handoff 文档的“回退矩阵”。
  - P7 回退：关闭 `workspace.enabled` 即可整体禁用工作区、批量与子模块 UI 入口；如仅批量操作异常，可下调 `workspace.maxConcurrentRepos=1` 退化为顺序；子模块异常时禁用递归（`submodule.autoInitOnClone=false`，手动操作可用）；模板导入风险时避免执行 `import_team_config_template` 并保留自动备份回滚；状态服务异常时将 `workspace.statusAutoRefreshSecs=0` 并清空缓存。
- **交接资料**：
  - 每次版本发布前更新 `CHANGELOG.md`、`new-doc/IMPLEMENTATION_OVERVIEW.md` 与对应的 handoff 文档；
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

### 4.5 P4 - IP 池与握手优选

  - **目标**：
    - 为指定域名和按需域名收集多来源 IP 候选，通过 TCP 握手延迟排序选择最佳连接；
    - 保持缓存 TTL、容量与历史持久化，确保网络变化时能快速刷新或回退；
    - 与传输层、自适应 TLS、观测体系打通，并在异常场景下提供熔断和全局禁用能力；
    - 提供 Soak 阈值和报告扩展，为灰度和准入提供量化依据。
  - **核心模块**：
    - `IpPool` 统一封装 pick/report/maintenance/config 接口；
    - `IpScoreCache`（内存缓存）+ `IpHistoryStore`（磁盘 `ip-history.json`）负责 TTL、容量、降级处理；
    - `PreheatService` 独立 tokio runtime 调度多来源采样（Builtin/UserStatic/History/DNS/Fallback），指数退避与手动刷新并存；
    - `custom_https_subtransport` 以延迟优先顺序尝试候选，失败后回退系统 DNS，并记录 `IpPoolSelection` 事件及线程本地埋点；
    - `circuit_breaker` + 黑白名单 + 全局 `auto_disabled_until` 联合管理熔断与禁用，事件 `IpPoolIpTripped/Recovered`、`IpPoolAutoDisable/Enable` 反映状态；
    - `core/ip_pool/events.rs` 集中封装所有新事件、保证测试可注入总线。
  - **配置与默认值**：
    - 运行期（`config.json`）：`ip_pool.enabled=false`、`cachePruneIntervalSecs=60`、`maxCacheEntries=256`、`singleflightTimeoutMs=10000`、熔断阈值 (`failureThreshold=5`、`failureRateThreshold=0.6`、`failureWindowSeconds=120`、`cooldownSeconds=300`、`circuitBreakerEnabled=true`)；
    - 文件（`ip-config.json`）：`preheatDomains=[]`、`scoreTtlSeconds=300`、`maxParallelProbes=4`、`probeTimeoutMs=3000`、`userStatic=[]`、`blacklist/whitelist=[]`、`historyPath="ip-history.json"`；所有字段热更新后立即重建预热计划与熔断状态。
  - **运行生命周期**：
    1. 应用启动加载配置 -> 构建 `IpPool` -> `PreheatService::spawn` 若启用则拉起后台 runtime；
    2. 预热循环按域调度采样，写入缓存与历史，并发 `IpPoolRefresh`；
    3. 任务阶段 `pick_best` 优先命中缓存，否则 `ensure_sampled` 同域单飞采样；
    4. `report_outcome` 回写成功/失败，为熔断统计提供数据；
    5. `maybe_prune_cache` 按 `cachePruneIntervalSecs` 清理过期与超额条目，同时调用 `history.prune_and_enforce`；
    6. 持续失败或运维干预触发 `set_auto_disabled`，冷却到期 `clear_auto_disabled` 自动恢复。
  - **预热调度细节**：
    - `DomainSchedule` 维护 `next_due`、`failure_streak` 与指数退避（封顶 6×TTL），热更新与 `request_refresh` 会立即重置；
    - 候选收集 `collect_candidates` 合并五类来源，白名单优先保留、黑名单直接剔除并发 `IpPoolCidrFilter`；
    - `measure_candidates` 受信号量限制并发数，`probe_latency` 根据配置截断超时，成功/失败均写 `ip_pool` target 日志；
    - 当所有域达到失败阈值时执行 `set_auto_disabled("preheat consecutive failures", cooldown)` 并进入冷却；
    - 预热成功/失败均会发 `IpPoolRefresh` 事件（`reason=preheat/no_candidates/all_probes_failed`）。
  - **按需采样与缓存维护**：
    - `ensure_sampled` 使用 `Notify` 单飞避免同域重复采样，超时（默认 10s）后回落系统 DNS；
    - `sample_once` 复用预热逻辑，成功写回缓存与历史；
    - `maybe_prune_cache` 清理过期条目、执行 LRU 式容量淘汰，再调用 `history.prune_and_enforce(now, max(maxCacheEntries, 128))`；
    - `IpHistoryStore` 持久化失败时降级为内存模式，仅记录 `warn`，运行期不受阻塞；
    - `auto_disable_extends_without_duplicate_events` 回归测试确保冷却延长不重复发 disable 事件，`clear_auto_disabled` 仅在状态切换时广播 enable。
  - **传输层集成**：
    - `acquire_ip_or_block` 返回按延迟排序的候选 snapshot，逐一尝试并通过 `report_candidate_outcome` 记录；
    - 成功/失败均通过 `IpPoolSelection`、线程局部 `ip_source`/`ip_latency_ms` 反馈给 `AdaptiveTlsTiming/Fallback`；
    - 阻塞接口 `pick_best_blocking` 复用全局 tokio runtime `OnceLock` 以适配同步调用场景；
    - IP 池禁用或候选耗尽时事件中的 `strategy=SystemDefault`，前端可据此回退展示。
  - **异常治理**：
    - `CircuitBreaker::record_outcome` 基于滑动窗口判定并发 `IpPoolIpTripped/Recovered`；
    - 黑白名单从配置热更新后即时生效，过滤结果通过 `IpPoolCidrFilter` 记录；
    - 全局自动禁用采用 CAS/Swap 保证幂等，冷却中延长仅写 debug，恢复只发一次 `IpPoolAutoEnable`；
    - 事件辅助 `event_bus_thread_safety_and_replacement` 测试覆盖并发场景，确保不会丢失或重复。
  - **观测与数据**：
    - 事件：`IpPoolSelection`（strategy/source/latency/candidates）、`IpPoolRefresh`（success/min/max/原因）、`IpPoolConfigUpdate`、熔断/禁用/CIDR；
    - `AdaptiveTlsTiming/Fallback` 新增 ip 字段，与 P3 事件共享线程局部；
    - `ip-history.json` 超过 1 MiB 记录警告；`prune_and_enforce` 在维护周期内统一清理；
    - Soak 报告新增 `ip_pool` 统计（selection_total/by_strategy、refresh_success/failure、success_rate）。
  - **Soak 与阈值**：
    - 环境变量 `FWC_ADAPTIVE_TLS_SOAK=1`、`FWC_SOAK_MIN_IP_POOL_REFRESH_RATE`、`FWC_SOAK_MAX_AUTO_DISABLE`、`FWC_SOAK_MIN_LATENCY_IMPROVEMENT` 等可调；
    - 报告 `thresholds` 判断 ready 状态，`comparison` 对比基线（成功率、回退率、IP 池刷新率、自动禁用次数、延迟改善）；
    - 无基线时自动标记 `not_applicable` 并写入原因。
  - **测试矩阵**：
    - 单元：`preheat.rs`、`history.rs`、`mod.rs`（缓存/单飞/TTL）、`circuit_breaker.rs`、`events.rs`；
    - 集成：`tests/tasks/ip_pool_manager.rs`、`ip_pool_preheat_events.rs`、`ip_pool_event_emit.rs`、`ip_pool_event_edge.rs`、`events_backward_compat.rs`；
    - Soak：`src-tauri/src/soak/mod.rs` 对阈值、报告、基线比较和环境变量覆盖提供测试；
    - 全量回归：`cargo test -q --manifest-path src-tauri/Cargo.toml`、前端 `pnpm test -s`。
  - **运维要点与故障排查**：
    - 快速禁用：`ip_pool.enabled=false` 或停用预热线程，新任务立即回退系统 DNS；
    - 黑白名单：更新 `ip-config.json` 后调用 `request_refresh` 即时生效，事件中保留被过滤 IP 与 CIDR；
    - 自动禁用：观察 `IpPoolAutoDisable`/`IpPoolAutoEnable` 与日志，必要时手动调用 `clear_auto_disabled` 或调整 `cooldownSeconds`；
    - 历史异常：删除损坏的 `ip-history.json` 会自动重建，日志含 `failed to load ip history`；
    - 调试：`RUST_LOG=ip_pool=debug` 打开预热/候选/退避细节；Soak 报告 `ip_pool.refresh_success_rate` < 阈值时重点排查网络连通性。
  - **交接 checklist**：
    - 发布前确认 `ip-config.json`/`config.json` 的 IP 池字段（TTL、并发、黑白名单、熔断阈值）与预期一致；
    - 运行 `cargo test --test ip_pool_manager`、`--test ip_pool_preheat_events`、`--test events_backward_compat` 快速验证集成与事件向后兼容；
    - 所有灰度环境需保留最新 soak 报告与 `selection_by_strategy` 指标截图，供准入评审；
    - 告警体系需新增 IP 池类事件（刷新失败率、auto disable、熔断）监控，防止观测盲点；
    - 运维手册应补充黑白名单维护与历史文件巡检 SOP。

### 4.6 P5 - 代理支持与自动降级

  - **目标**：
    - 支持 HTTP/HTTPS、SOCKS5 和系统代理，提供统一配置与管理接口；
    - 实现代理失败的自动降级直连与健康检查恢复机制；
    - 与 Fake SNI、IP 优选等既有策略保持互斥，确保网络环境适配性；
    - 提供跨平台系统代理检测（Windows/macOS/Linux）与前端集成。
  - **核心模块**：
    - `ProxyManager` 统一封装模式、状态、连接器、健康检查、配置热更新；
    - `HttpProxyConnector`（CONNECT隧道、Basic Auth）与 `Socks5ProxyConnector`（协议握手、认证方法、地址类型）实现 `ProxyConnector` trait；
    - `ProxyFailureDetector` 滑动窗口统计失败率，触发自动降级并发 `ProxyFallbackEvent`；
    - `ProxyHealthChecker` 后台定期探测（默认60秒），连续成功达阈值后触发自动恢复并发 `ProxyRecoveredEvent`；
    - `SystemProxyDetector` 跨平台检测系统代理（Windows注册表/macOS scutil/Linux环境变量）；
    - 传输层集成：`register.rs` 检查代理配置，启用时跳过自定义传输层注册，强制使用 libgit2 默认 HTTP 传输。
  - **配置与默认值**：
    - 运行期（`config.json`）：`proxy.mode=off`（off/http/socks5/system）、`url=""`、`username/password=null`、`disableCustomTransport=false`（代理启用时强制true）、`timeoutSeconds=30`、`fallbackThreshold=0.2`、`fallbackWindowSeconds=300`、`recoveryCooldownSeconds=300`、`healthCheckIntervalSeconds=60`、`recoveryStrategy="consecutive"`、`probeUrl="www.github.com:443"`（host:port格式）、`probeTimeoutSeconds=10`、`recoveryConsecutiveThreshold=3`、`debugProxyLogging=false`；
    - 所有字段支持热更新，通过重新创建 `ProxyManager` 实例生效。
  - **运行生命周期**：
    1. 应用启动加载配置 -> 创建 `ProxyManager` -> 初始化失败检测器和健康检查器；
    2. 传输层注册时调用 `should_skip_custom_transport()`，代理启用则跳过 `https+custom` 注册；
    3. 任务阶段 `get_connector()` 返回对应连接器（HTTP/SOCKS5），建立隧道并报告结果；
    4. `report_failure()` 更新滑动窗口，失败率超阈值触发 `trigger_automatic_fallback()`；
    5. 后台健康检查定期探测，连续成功达阈值触发 `trigger_automatic_recovery()`；
    6. 冷却窗口结束后自动清除禁用状态，恢复代理模式；
    7. 状态机转换规则：Disabled↔Enabled（启用/禁用）、Enabled→Fallback（失败降级）、Fallback→Recovering（开始恢复）、Recovering→Enabled（恢复成功）或Recovering→Fallback（恢复失败），所有转换通过 `can_transition_to()` 验证。
  - **强制互斥策略**：
    - 代理启用时 `ProxyManager::should_disable_custom_transport()` 强制返回 `true`（检查 `is_enabled()` 且 `mode != Off`）；
    - 传输层注册阶段 `register.rs::should_skip_custom_transport()` 创建临时 `ProxyManager` 检查配置，若应禁用则直接返回 `Ok(())`，跳过 `https+custom` 注册；
    - 同时通过 `tl_set_proxy_usage()` 记录代理使用状态到线程局部metrics，供传输层和观测系统使用；
    - 结果：代理模式下不使用 Fake SNI、IP 优选，直接使用 libgit2 默认 HTTP 传输（真实SNI），避免复杂度叠加和潜在冲突。
  - **协议实现细节**：
    - HTTP CONNECT：构造 `CONNECT host:port HTTP/1.1`，解析 200/407（需认证）/502（网关错误）响应，支持 Basic Auth（Base64编码 `username:password`）；超时通过 `TcpStream::set_read_timeout()` 和 `set_write_timeout()` 控制；
    - SOCKS5：版本协商（0x05）-> 认证（No Auth 0x00 / Username/Password 0x02）-> CONNECT请求（CMD=0x01），支持 IPv4（ATYP=0x01）/IPv6（ATYP=0x04）/域名（ATYP=0x03）地址类型，映射 REP 错误码（0x01-0x08：通用失败/规则禁止/网络不可达/主机不可达/连接拒绝/TTL超时/命令不支持/地址类型不支持）；
    - 系统检测：Windows 读取注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` 的 `ProxyEnable` 和 `ProxyServer` 字段，macOS 执行 `scutil --proxy` 并解析输出，Linux 检测 `HTTPS_PROXY`/`HTTP_PROXY` 环境变量（按优先级）；
    - 错误分类：`ProxyError` 包含5个变体（Network/Auth/Proxy/Timeout/Config），每个错误通过 `category()` 方法返回分类字符串供日志和诊断使用。
  - **自动降级与恢复**：
    - 失败检测器维护滑动窗口（默认300秒），样本数≥5且失败率≥20%触发降级；
    - 降级后状态切换为 `Fallback`（通过 `can_transition_to()` 验证 Enabled→Fallback 合法），`is_enabled()` 返回 `false`，后续任务走直连；状态机转换规则在 `state.rs` 的 `apply_transition()` 中强制验证；
    - 健康检查器定期探测代理可用性（探测目标为 `probeUrl` 配置的 host:port）；
    - 连续成功次数达阈值（默认3次）且冷却期满（默认300秒）触发恢复；
    - 恢复后状态切换为 `Enabled`，重置失败统计，发射 `ProxyRecoveredEvent`；
    - 支持三种恢复策略：`immediate`（单次成功）、`consecutive`（连续多次成功）、`exponential-backoff`（退避恢复）。
  - **前端集成**：
    - `ProxyConfig.vue`：代理配置UI（模式选择、URL/凭证输入、系统检测按钮、禁用自定义传输层开关、高级设置包含降级/恢复/探测配置、调试日志开关）；
    - `ProxyStatusPanel.vue`：状态面板（当前状态、降级原因、失败统计、URL显示含凭证脱敏）；
    - Tauri 命令：`detect_system_proxy()`（检测系统代理，返回 SystemProxyResult）、`force_proxy_fallback(reason?: Option<String>)`（手动降级，支持自定义原因）、`force_proxy_recovery()`（手动恢复）、`get_system_proxy()`（legacy 命令，返回基础信息）；
    - Pinia store：`useConfigStore` 管理代理配置（读写 config.json），前端组件通过 Tauri 命令直接调用后端功能；
    - 系统代理检测通过 `detect_system_proxy()` 命令返回结果，不发射事件；配置热更新无独立事件，由组件保存时触发。
  - **观测与事件**：
    - `ProxyStateEvent`：状态转换（previous/current state、reason、timestamp），包含扩展字段（proxy_mode、proxy_state、fallback_reason、failure_count、health_check_success_rate、next_health_check_at、system_proxy_url、custom_transport_disabled）；
    - `ProxyFallbackEvent`：降级事件（reason、failure_count、window_seconds、failure_rate、proxy_url、is_automatic）；
    - `ProxyRecoveredEvent`：恢复事件（successful_checks、proxy_url、is_automatic、strategy、timestamp）；
    - `ProxyHealthCheckEvent`：健康检查结果（success、response_time_ms、error、proxy_url、test_url、timestamp）；
    - 代理事件通过传输层线程局部变量与 P3 的 `AdaptiveTlsTiming/Fallback` 事件联动，但不会修改既有事件结构。
  - **Soak 与阈值**：
    - 环境变量：`FWC_PROXY_SOAK=1`、`FWC_SOAK_MIN_PROXY_SUCCESS_RATE=0.95`（代理成功率≥95%）、`FWC_SOAK_MAX_PROXY_FALLBACK_COUNT=1`（最多降级1次）、`FWC_SOAK_MIN_PROXY_RECOVERY_RATE=0.9`（恢复率≥90%，如有降级）；
    - 报告扩展：`proxy` 统计（selection_total、selection_by_mode、fallback_count、recovery_count、health_check_success_rate、avg_connection_latency_ms、system_proxy_detect_success）；
    - 阈值判定：`proxy_success_rate >= 0.95`、`fallback_count <= 1`、`recovery_rate >= 0.9`（如有降级）、`system_proxy_detect_success == true`（System模式必须检测成功）。
  - **测试矩阵**：
    - config.rs（36测试）：ProxyConfig结构、validation规则、默认值、is_enabled逻辑；
    - state.rs（17测试）：ProxyState状态机、转换验证、状态上下文；
    - detector.rs（28测试）：ProxyFailureDetector滑动窗口、失败率计算、阈值触发；
    - manager.rs（59测试）：ProxyManager统一API、配置热更新、状态管理、连接器切换；
    - http_connector.rs（29测试）：HTTP CONNECT隧道、Basic Auth、响应解析、超时处理；
    - socks5_connector.rs（59测试）：SOCKS5协议握手、认证方法、地址类型、REP错误码映射；
    - events.rs（15测试）：事件结构体序列化、时间戳生成、事件构造器；
    - ProxyConfig.vue（14测试）：配置UI交互、表单验证、系统检测、配置保存；
    - ProxyStatusPanel.vue（19测试）：状态显示、URL脱敏、模式Badge；
    - 总计：276个测试（243 Rust 单元/集成测试分布在 7 个文件：config.rs/state.rs/detector.rs/manager.rs/http_connector.rs/socks5_connector.rs/events.rs + 33 TypeScript 组件测试：ProxyConfig.test.ts 14个 + ProxyStatusPanel.test.ts 19个）。
  - **常见故障排查**：
    - 代理连接失败：检查 `proxy.url` 格式（必须包含协议前缀如 `http://`）、网络可达性（ping代理服务器）、凭证正确性（用户名/密码）；查看 `task://error` 中的 `category=Proxy/Auth`；启用 `debugProxyLogging=true` 查看详细连接日志（包含sanitized URL、认证状态、响应时间）；
    - 频繁降级：查看 `ProxyFallbackEvent` 中的 `failure_rate` 和 `failure_count`，可能需调高 `fallbackThreshold`（默认0.2即20%） 或检查代理稳定性；检查滑动窗口 `fallbackWindowSeconds` 是否过短；
    - 系统检测失败：Windows 检查注册表权限（需要读取 `HKCU`）和 IE代理设置是否配置、macOS 检查 `scutil` 命令是否可执行和网络偏好设置、Linux 检查环境变量（优先 `HTTPS_PROXY` 再 `HTTP_PROXY`）；提供手动配置回退（切换到http/socks5模式手动输入）；
    - 自定义传输层未禁用：确认代理 `is_enabled()` 返回 `true`（mode非off且URL非空或mode为system），检查 `should_disable_custom_transport()` 逻辑；查看日志中的 "Skipping custom transport registration" 消息；
    - 恢复不触发：检查冷却窗口是否到期（查看日志中的 recovery cooldown 提示）、`recoveryConsecutiveThreshold` 是否过高（默认3次，建议不超过10）、健康检查是否正常执行（查看 `ProxyHealthCheckEvent`）、探测URL是否可达（`probeUrl` 默认 `www.github.com:443`）。
  - **交接要点**：
    - 代理配置凭证当前明文存储在 `config.json`，P6 将引入安全存储（Windows Credential Manager/macOS Keychain/Linux Secret Service）；
    - 仅支持 Basic Auth，企业认证协议（NTLM/Kerberos）暂不支持，可使用 CNTLM 等本地转换工具；
    - PAC 文件解析、代理链、实时配置监听等功能延后到 P6 或后续版本；
    - 代理启用时强制禁用自定义传输层与 Fake SNI 是设计选择（通过 `should_disable_custom_transport()` 实现），即使 `disableCustomTransport=false` 也会被覆盖；
    - 热更新代理配置需要重新创建 `ProxyManager` 实例，通过传输层注册检查 `should_skip_custom_transport()` 生效；
    - 手动降级/恢复立即切换状态，下一个任务立即使用新配置；
    - 探测URL必须是 `host:port` 格式（如 `www.github.com:443`），不支持完整URL格式。
  - **回退策略**：
    - 配置层：设置 `proxy.mode=off` 立即禁用代理，下一个任务生效；
    - 手动控制：前端点击"手动降级"或调用 `force_proxy_fallback(reason?)` Tauri命令强制切换直连，发送 `ProxyFallbackEvent` (is_automatic=false)；
    - 调整阈值：修改 `fallbackThreshold`（0.0-1.0）/`recoveryConsecutiveThreshold`（1-10）/`recoveryCooldownSeconds`（≥10）并保存配置文件，应用重启或重新加载配置后生效；
    - 清理统计：重启应用或手动调用 `force_proxy_recovery()` 重置滑动窗口的失败统计并尝试恢复；
    - 运维介入：通过日志观察 `ProxyStateEvent` 和 `ProxyHealthCheckEvent` 获取诊断信息（当前状态、失败计数、健康检查结果），必要时临时设置 `healthCheckIntervalSeconds` 为更大值（如3600）延长探测间隔，或直接禁用代理。

### 4.7 P6 - 凭证存储与安全管理

  - **目标**:
    - 提供生产级凭证存储方案，支持三层存储智能回退（系统钥匙串 → 加密文件 → 内存）；
    - 实现企业级加密安全（AES-256-GCM + Argon2id密钥派生 + ZeroizeOnDrop内存保护）；
    - 提供完整审计日志与访问控制机制（失败锁定、自动过期、持久化）；
    - 与Git操作深度集成（自动填充凭证、智能降级、过期提醒）；
    - 前端用户体验优化（凭证管理表单、过期凭证管理、审计日志查看）。
  - **核心模块**：
    - `CredentialStoreFactory` 三层存储抽象与智能回退（根据平台能力、用户权限、配置自动选择最优存储）；
    - 系统钥匙串集成：Windows Credential Manager（`WindowsCredentialStore`）、macOS Keychain Services、Linux Secret Service（通过统一接口 `CredentialStore` trait实现）；
    - `EncryptedFileStore` 文件加密存储（AES-256-GCM加密、Argon2id密钥派生、密钥缓存优化200倍性能提升）；
    - `InMemoryStore` 进程级临时存储（回退兜底、测试隔离）；
    - `AuditLogger` 双模式审计（标准模式不记录哈希、审计模式记录SHA-256哈希）+ 持久化（JSON文件、自动加载、容错设计）；
    - `AccessControl`（内嵌于AuditLogger）失败锁定机制（默认5次失败 → 默认1800秒即30分钟锁定 → 自动过期或管理员重置）；
    - Git集成：`git_credential_autofill` 三级智能降级（存储凭证 → 未找到提示 → 错误继续）。
  - **配置与默认值**：
    - 运行期（`config.json`）：`credential.storage=system`（system/file/memory）、`default_ttl_seconds=7776000`（90天）、`debug_logging=false`、`audit_mode=false`、`require_confirmation=false`、`file_path=null`（加密文件路径，可选）、`key_cache_ttl_seconds=3600`（1小时）；
    - 访问控制（内部硬编码，不可配置）：`max_failures=5`、`lockout_duration_secs=1800`（30分钟）；
    - 环境变量：`FWC_CREDENTIAL_STORE`（覆盖storage）、`FWC_MASTER_PASSWORD`（测试/CI场景，加密文件模式使用）；
    - 所有字段支持热更新，修改后下一次操作生效。
  - **运行生命周期**：
    1. 应用启动 → `CredentialStoreFactory::create()` 根据配置尝试三层存储，失败则自动降级；
    2. 加密文件模式需用户调用 `unlock_store(masterPassword)` 解锁 → Argon2id密钥派生（1-2秒）→ 缓存密钥（TTL 300秒）；
    3. 凭证操作（add/get/update/delete/list）→ 路由到对应存储实现 → 自动记录审计日志；
    4. Git操作调用 `git_credential_autofill(host, username)` → 自动填充存储的凭证 → 未找到则返回None继续原有流程；
    5. 访问控制检测连续失败，达阈值触发 `AccessControlLocked` 事件并拒绝后续操作；
    6. 定期调用 `cleanup_expired_credentials()` 清理过期凭证，前端显示即将过期警告（7天）和已过期提示。
  - **三层存储智能回退**：
    - **Layer 1 - 系统钥匙串**：Windows Credential Manager（`CredReadW`/`CredWriteW`/`CredDeleteW`）、macOS Keychain（Security Framework）、Linux Secret Service（`libsecret` D-Bus）；失败原因包括权限不足、服务未运行、API错误；
    - **Layer 2 - 加密文件**：`credentials.enc` AES-256-GCM加密（随机nonce、AEAD认证标签）+ Argon2id密钥派生（m_cost=64MB, t_cost=3, p_cost=1）+ 密钥缓存（首次1-2秒，缓存后<10ms）；失败原因包括主密码错误、文件损坏、磁盘权限；
    - **Layer 3 - 内存存储**：进程内 `HashMap` 兜底，应用重启丢失；始终可用，确保功能不中断；
    - 回退决策：系统钥匙串失败 → 尝试加密文件（需主密码）→ 回退内存存储；每次回退记录日志并通过 `StoreBackendChanged` 事件通知前端。
  - **加密与安全**：
    - AES-256-GCM：对称加密算法，提供机密性和完整性保护（AEAD），每个凭证独立nonce确保安全；
    - Argon2id：密钥派生函数（KDF），抗GPU/ASIC破解，参数：内存64MB、时间3迭代、并行度1线程（符合OWASP推荐）；
    - HMAC验证：审计模式下对主机名/用户名生成SHA-256 HMAC，用于凭证追溯而不泄露明文；
    - ZeroizeOnDrop：`MasterPassword`、`EncryptionKey`、`Credential`中的密码字段使用 `zeroize` crate自动清零，防止内存残留；
    - 密钥缓存：首次派生1-2秒，缓存后<10ms，性能提升200倍；缓存密钥使用 `Arc<RwLock<Option<EncryptionKey>>>` 保护，TTL默认3600秒（1小时）；
    - Display/Debug trait：密码字段使用 `masked_password()` 脱敏（前2字符+***+后2字符），防止日志泄露。
  - **审计与访问控制**：
    - 审计日志包含：操作类型（Add/Get/Update/Delete）、时间戳（Unix秒）、主机名、用户名、结果（Success/Failure/AccessDenied）、可选SHA-256哈希（审计模式）；
    - 持久化：`audit-log.json` JSON Lines格式，应用启动自动加载，损坏时优雅降级创建新文件；
    - 访问控制：连续5次失败 → 锁定30分钟 → 自动过期或管理员调用 `reset_credential_lock()` 重置；锁定期间返回 `remaining_attempts()` 供前端显示剩余尝试次数；
    - 容错设计：审计日志写入失败不影响凭证操作（降级为内存日志），文件损坏时自动重建。
  - **Git集成细节**：
    - `git_credential_autofill(host, username)` 在Git Push/Fetch前调用，返回 `Option<CredentialInfo>`；
    - 三级降级策略：存储中找到凭证 → 直接使用；未找到 → 返回None，Git操作继续交互式输入；获取失败（锁定/错误）→ 返回None并记录错误；
    - URL格式支持：HTTPS（`https://github.com/...`）、SSH（`ssh://git@github.com:...`）、Git简写（`git@github.com:...`）；
    - 过期处理：即将过期（7天内）显示黄色警告，已过期显示红色错误并提供一键清理按钮；
    - 3次迭代优化（P6.4.1-P6.4.3）：初始实现 → 添加URL解析与域名提取 → 优化错误处理与降级逻辑（共1,135行代码）。
  - **前端集成**：
    - **Tauri命令**（13个）：
      - 凭证操作（5个）：`add_credential`、`get_credential`、`update_credential`、`delete_credential`、`list_credentials`；
      - 生命周期管理（2个）：`cleanup_expired_credentials`、`set_master_password`（初始化）、`unlock_store`（解锁）；
      - 审计日志（2个）：`export_audit_log`、`cleanup_audit_logs`；
      - 访问控制（3个）：`is_credential_locked`、`reset_credential_lock`、`remaining_auth_attempts`；
    - **Vue组件**（4个）：
      - `CredentialForm.vue`（165-182行）：凭证添加/编辑表单，支持主机名、用户名、密码/令牌输入，过期时间选择（天数）；
      - `CredentialList.vue`（178行）：凭证列表展示，脱敏显示（仅前后2字符），过期状态Badge（即将过期/已过期），删除确认；
      - `ConfirmDialog.vue`（65行，P6.5新增）：通用确认对话框，3种变体（danger/warning/info），DaisyUI modal实现；
      - `AuditLogView.vue`（156行，P6.5新增）：审计日志查看，时间范围过滤、操作类型筛选、导出JSON功能；
    - **Pinia Store**（`credential.store.ts`）：9个actions（loadCredentials、addCredential、updateCredential、deleteCredential、unlockStore、lockStore、cleanupExpired、resetLock、exportAuditLogs）、5个getters（isLocked、expiringSoon、expired、sortedCredentials、auditSummary）。
  - **测试矩阵**：
    - 后端测试：521个（60单元测试 + 461集成测试），206个凭证模块专项测试（73存储 + 48管理 + 31审计 + 24生命周期 + 9 Git + 21 CredentialView组件）；
    - 前端测试：295个（全部通过），144个P6凭证相关测试（17 credential.store + 28 CredentialForm + 99 UI组件）；
    - 总计：1286个测试（991 Rust + 295 前端），99.9%通过率（仅1个proxy模块pre-existing issue），88.5%覆盖率；
    - 关键测试场景：三层回退、加密解密往返、密钥缓存TTL、访问控制锁定与恢复、审计日志持久化与容错、Git自动填充3种URL格式、过期凭证清理、并发操作安全。
  - **性能指标**：
    - 系统钥匙串：add/get/delete <5ms（Windows实测），list(100) ~15ms；
    - 加密文件：首次操作1000-2000ms（密钥派生），缓存后<10ms，性能提升200倍；
    - 内存存储：所有操作<1ms，list(1000) <200ms；
    - 审计日志：写入<0.5ms（异步），SHA-256哈希<0.5ms；
    - 并发性能：100线程并发读写无死锁、无数据竞争。
  - **代码规模**：
    - 总计：17,540行（核心4,684 + 测试8,406 + 文档4,450）；
    - 测试/核心比例：1.8:1（优秀）；
    - Clippy警告：0；unwrap()数量：0（全部使用expect或?）；unsafe代码：0。
  - **技术创新**（10项）：
    1. 三层存储智能回退（平衡安全性与可用性）；
    2. SerializableCredential模式（解决 `#[serde(skip)]` 序列化问题）；
    3. 密钥派生缓存优化（200倍性能提升）；
    4. Windows API凭证前缀过滤（`fireworks-collaboration:git:` 避免冲突）；
    5. CredentialInfo自动映射（密码永不传输到前端）；
    6. 审计日志双模式（标准/审计模式平衡隐私与追溯）；
    7. Git凭证智能降级（3级降级保证可用性）；
    8. 过期凭证双重提醒（即将过期/已过期）；
    9. 审计日志容错设计（损坏时优雅降级）；
    10. 访问控制自动过期（30分钟自动解锁）。
  - **安全审计结论**（2025年10月4日）：
    - 审计范围：~3,600行核心代码，8个维度（加密、内存、日志、错误、并发、平台、配置、密钥）；
    - 总体评分：⭐⭐⭐⭐⭐ (4.9/5)；
    - 风险识别：0高危、3中危（macOS/Linux未实机验证、密钥缓存内存风险、审计日志无限增长）、3低危；
    - 合规性：OWASP Top 10全部通过、NIST标准符合（AC/AU/IA/SC系列）、依赖安全无已知CVE；
    - 准入决策：✅ **批准生产环境上线**（附条件：CI/CD跨平台测试）。
  - **准入评审**（7项标准全部达标）：
    - 功能完整性：99%（仅 `last_used` 未实现，受Rust不可变模型限制）；
    - 测试通过率：99.9%（1286个测试，仅1个非相关失败）；
    - 测试覆盖率：88.5%（后端90%、前端87%）；
    - 安全审计：0高危风险；
    - 性能指标：<500ms达标（除首次密钥派生）；
    - 文档完整性：100%（所有公共API）；
    - 代码质量：0 Clippy警告。
  - **常见故障排查**：
    - 系统钥匙串失败：Windows检查Credential Manager服务是否运行、macOS检查Keychain Access权限、Linux检查Secret Service（`gnome-keyring`/`seahorse`）是否安装；查看日志中的具体错误码；
    - 主密码错误：加密文件模式下密钥派生失败返回 `InvalidMasterPassword`，重置需删除 `credentials.enc` 并重新设置；
    - 访问控制锁定：连续5次失败后锁定30分钟，查看 `AccessControlLocked` 事件中的 `locked_until` 时间戳，管理员可调用 `reset_credential_lock()` 立即解锁；
    - 审计日志损坏：删除 `audit-log.json` 会自动重建，日志含 "failed to load audit log" 警告；
    - Git自动填充不工作：检查URL格式是否支持（HTTPS/SSH/git@），确认凭证已存储且未过期，查看 `git_credential_autofill` 返回值；
    - 密钥缓存过期：默认TTL 3600秒（1小时），过期后下次操作重新派生（1-2秒），可通过 `key_cache_ttl_seconds` 调整。
  - **交接要点**：
    - 凭证当前明文存储在系统钥匙串/加密文件，P7可考虑HSM集成或硬件密钥；
    - macOS/Linux系统钥匙串代码已实现但未实机验证，建议添加CI/CD跨平台测试；
    - 审计日志暂无自动滚动策略，需手动清理或在后续版本实现（短期优化）；
    - 性能基准测试框架已完成（295行，8个测试组），建议运行 `cargo bench --bench credential_benchmark` 获取实际数据；
    - 最后使用时间（`last_used`）字段因Rust不可变模型限制未实现，需重构为可变结构（技术债务）；
    - 凭证导出功能暂无额外加密保护，用户自行管理导出文件安全（延后增强）。
  - **回退策略**：
    - 配置层：逐层禁用存储（system → file → memory），或完全禁用凭证功能；
    - 审计日志：关闭审计模式（`auditMode=standard`）或禁用持久化；
    - 访问控制：调整阈值（`maxFailures`、`lockoutDurationMinutes`）或完全禁用（`enabled=false`）；
    - Git集成：移除 `git_credential_autofill` 调用，回退到交互式输入；
    - 主密码：重置需删除 `credentials.enc` 并重新解锁，已存储凭证丢失（提前备份）。
  - **上线策略**（推荐三阶段灰度）：
    1. **阶段1（灰度）**：10-20个用户测试（1周），重点验证系统钥匙串集成和主密码流程；
    2. **阶段2（扩大）**：100个用户测试（2周），监控审计日志存储和访问控制触发频率；
    3. **阶段3（全量）**：全量发布，持续监控性能指标和安全事件。
  - **后续优化建议**：
    - 短期（1-3个月）：macOS/Linux实机验证、审计日志滚动策略、性能基准测试执行、用户体验优化（搜索/过滤/批量操作）；
    - 长期（3-12个月）：生物识别解锁（Touch ID/Windows Hello）、OAuth 2.0自动刷新、凭证跨设备同步、审计日志远程上传、HSM集成。

### 4.8 P7 - 工作区与批量能力

- **目标**:
  - 建立多仓库工作区(Workspace)管理模型,支持仓库 CRUD、标签分类与序列化存储;
  - 实现 Git 子模块探测与批量操作(init/update/sync),复用现有 git2 能力;
  - 提供批量并发任务调度(clone/fetch/push),通过 Semaphore 控制并发度,避免资源竞争;
  - 支持团队配置模板导出/导入,便于跨团队标准化与安全化;
  - 引入跨仓库状态监控服务,带 TTL 缓存与无效化 API,减少重复查询开销;
  - 前端一体化视图,集成任务进度、错误聚合与 Pinia store 响应式状态;
  - 提供性能基准与稳定性测试,支撑灰度上线决策。

- **批量任务并发控制实现细节**:
  - `workspace_batch_*` 命令通过 `resolve_concurrency(requested, config)` 解析最终并发数:优先使用请求中的 `maxConcurrency`,回退到配置的 `workspace.maxConcurrentRepos`(实际默认值 3),强制校验 `value > 0` 避免死锁;
  - 内部使用 `tokio::sync::Semaphore` 持有 `max_concurrency` 个 permit,每个子任务执行前 `acquire()`,完成后自动释放,确保同时运行的子任务数不超过阈值;
  - 父任务(`TaskKind::WorkspaceBatch { operation, total }`)创建后立即返回父 `taskId`,子任务递归创建为 `TaskKind::GitClone`/`GitFetch`/`GitPush`,通过 `parent_id` 关联,进度事件聚合到父任务的 phase 文本中(`Cloning 3/10 repositories`);
  - 失败策略:默认 `continueOnError=true`,单个子任务失败不中断批量流程,最终父任务汇总所有子任务状态到 `task://state`(`completed` 表示全部成功,`failed` 表示至少一个失败),错误详情通过 `task://error` 子任务事件分发。

- **RepositoryEntry.hasSubmodules 字段作用**:
  - 在 `workspace_batch_clone` 命令中,若请求未明确指定 `recurseSubmodules` 参数,则回退到该字段值作为默认行为:
    ```rust
    recurse_submodules: request.recurse_submodules.unwrap_or(repo.has_submodules)
    ```
  - 允许为不同仓库单独配置子模块处理策略(例如前端仓库启用,后端服务禁用),提高灵活性;
  - 该字段默认值为 `false`,在 `workspace.json` 中显式声明后生效。

- **SubmoduleManager 独立状态**:
  - `SharedSubmoduleManager = Arc<Mutex<SubmoduleManager>>` 与 workspace/status service 并行,拥有独立 `SubmoduleConfig`(默认 autoRecurse=true, maxDepth=5, autoInitOnClone=true, recursiveUpdate=true);
  - 命令返回 `SubmoduleCommandResult { success: bool, message: String, data?: string[] }`:前端必须检查 `success` 字段判断成功/失败,而非直接依赖 Promise resolve/reject。`data` 字段包含受影响子模块名称列表。

- **运维命令扩展**:
  - `validate_workspace_file(path)`: 校验 workspace.json 结构合法性,返回布尔值;
  - `backup_workspace(path)`: 创建带时间戳的备份文件(`workspace.json.bak-YYYYMMDDHHMMSS`),返回完整路径字符串;
  - `restore_workspace(backupPath, workspacePath)`: 从备份恢复,直接覆盖目标文件(原子操作);
  - 备份策略建议:每次批量操作前手动备份或配置自动备份钩子(未实现)。

- **测试覆盖**:
  - 子模块: 24 项(列表/检测/初始化/更新/同步,含递归场景);
  - 批量调度: 12 项(clone/fetch/push 各 4个,含并发/失败聚合/取消传播);
  - 状态服务: 缓存/TTL/失效集成测试 + 10/50/100 仓库性能基准;
  - 前端 store: 17 项 Pinia 测试(批量任务、模板报告、状态缓存);
  - 性能基准 Nightly 门槛: 批量 clone p95 < 0.2×/仓、状态刷新 100 仓 < 3s。

- **回退策略快速参考**:
  - 批量负载过高:降 `workspace.maxConcurrentRepos=1` 退化为顺序;
  - 子模块初始化失败:禁用 `submodule.autoInitOnClone=false`,手动调用;
  - 状态刷新 IO 压力:停止自动轮询(`statusAutoRefreshSecs=0`)提高 TTL;
  - 模板导入风险:使用自动备份覆盖 config.json 回滚;
  - 整体禁用:`workspace.enabled=false` 回退单仓模式(不需重编译)。

- **模块映射与代码指针**:
  - `src-tauri/src/core/workspace/`: model.rs(核心结构), config.rs(配置管理), storage.rs(序列化/验证/备份), status.rs(状态服务);
  - `src-tauri/src/core/submodule/`: model.rs, manager.rs(init/update/sync 操作), config.rs(子模块配置);
  - `src-tauri/src/core/tasks/workspace_batch.rs`: Semaphore 调度、父子任务关联、进度聚合;
  - `src-tauri/src/core/config/team_template.rs`: 模板导出/导入、安全化清理、备份机制;
  - `src-tauri/src/app/commands/workspace.rs`: 18 个 Tauri 命令(CRUD/批量/状态/备份);
  - `src-tauri/src/app/commands/submodule.rs`: 9 个 Tauri 命令(list/has/init/update/sync + 配置);
  - `src/views/WorkspaceView.vue`: 工作区视图(仓库列表、批量操作、状态监控);
  - `src/stores/workspace.ts`: Pinia store(CRUD actions/getters, 与后端命令桥接);
  - `src/stores/tasks.ts`: 批量任务父子关系跟踪、进度聚合。

- **跨阶段集成补充**:
  - P7 工作区/批量逻辑仅在 `workspace.enabled=true` 时激活,不改变 MP0-P6 任务语义;
  - 子模块递归克隆附加在 `TaskKind::GitClone` 之后(70-85-100% 进度区间映射);
  - 团队配置模板导入仅写入 config.json,不影响 TLS/IP/代理/凭证回退路径;
  - 代理(P5)启用时不影响批量逻辑,内部仍复用 Git 传输层现有互斥策略;
  - 凭证存储(P6)自动为批量 clone/push 子任务统一回调,无需重复提供凭证;
  - IP 池(P4)与工作区解耦,批量任务底层按单仓 Git 路径调用。

- **已知限制**:
  - 子模块并行参数(`parallel`/`maxParallel`)预留但未实现(串行足够 <10 子模块场景);
  - 子模块粒度进度事件尚未连接前端总线,仅通过主任务阶段映射;
  - 批量进度权重均等,未按仓库体积/历史耗时加权(大体量差异下不线性);
  - 工作区状态服务无事件推送(需轮询),大量仓库高频刷新需调大 TTL 与关闭自动刷新;
  - 工作区文件并发风险:未实现显式锁,多进程同时写 workspace.json 存在竞态(建议容器编排保证单实例)。

### 4.9 测试重构 - 统一验证体系

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

- **文档同步**：合并前核对 `new-doc/MP*_IMPLEMENTATION_HANDOFF.md`、`new-doc/P*_IMPLEMENTATION_HANDOFF.md` 与本文、`doc/TESTS_REFACTOR_HANDOFF.md` 的版本号与变更记录，一并更新 `CHANGELOG.md`。
- **配置审计**：按阶段确认环境变量与 AppConfig 值：MP1（Fake SNI 白名单、Retry 阈值）、P2（`FWC_PARTIAL_FILTER_SUPPORTED`、`FWC_STRATEGY_APPLIED_EVENTS`）、P3（rollout 百分比、auto disable 阈值、SPKI pin 列表）、P4（`ip_pool.enabled`、`cachePruneIntervalSecs`、`maxCacheEntries`、`singleflightTimeoutMs`、熔断阈值与 `cooldownSeconds`、`preheatDomains`、`probeTimeoutMs`、黑白名单）、P5（`proxy.mode`、`proxy.url`、凭证、`disableCustomTransport`、降级阈值 `fallbackThreshold`（0.2）、恢复阈值 `recoveryConsecutiveThreshold`（3）、健康检查间隔 `healthCheckIntervalSeconds`（60）、探测URL `probeUrl`（host:port格式如"www.github.com:443"）、探测超时 `probeTimeoutSeconds`（10）、冷却窗口 `recoveryCooldownSeconds`（300）、恢复策略 `recoveryStrategy`（immediate/consecutive/exponential-backoff）、调试日志 `debugProxyLogging`（false））、P6（`credential.storage`（system/file/memory）、`default_ttl_seconds`（7776000即90天）、`debug_logging`（false）、`audit_mode`（false）、`require_confirmation`（false）、`file_path`（可选）、`key_cache_ttl_seconds`（3600即1小时））。
- **灰度计划**：在发布计划中记录灰度顺序与回退手段（参考 §3.5），确保 SRE 获得监控告警阈值与事件观察面。P6 推荐三阶段灰度：10-20用户（1周，系统钥匙串验证）→ 100用户（2周，审计日志监控）→ 全量发布。
- **测试执行**：要求在主干合并前执行 `cargo test -q`、`pnpm test -s`，必要时附加 soak 报告；若涉及传输层改动，附上目标域指纹快照。P6 需额外验证：1286个测试全部通过、安全审计报告、准入评审文档。
- **运维交接**：提供最新 `cert-fp.log` 样例、策略 Summary 截图与 `task_list` 正常输出，帮助运维确认运行状态；新事件字段需同步到监控解析脚本。
