# MP0 实现说明（交接稿）— 从 gitoxide 迁移到 git2-rs，保持前端契约不变

> 目的：面向 MP1 研发与联调，完整说明 MP0 的实现细节、接口契约、事件与进度、错误与取消、配置与测试现状，以及对后续 Push/Subtransport/Retry 的对接点与注意事项。
>
> 版本：v1.0（2025-09-14） 维护者：Core Team

---

## 1. 范围与目标

- 范围：在不改变前端 API/事件/任务模型的前提下，将后端 Git 实现从 gitoxide（gix）全面替换为 git2-rs（libgit2 绑定）。
- 维持不变：
  - 命令：`git_clone`、`git_fetch`、`task_cancel`、`task_list`、`task_snapshot`；
  - 事件：`task://state`、`task://progress`（字段名与含义保持兼容，可附加可选字段）；
  - 任务模型：`Pending|Running|Completed|Failed|Canceled`；
  - HTTP 伪 SNI 调试 API：`http_fake_request`（白名单校验、重定向处理、可选脱敏）。
- 新能力与修复：
  - Git 实现统一为 git2-rs，提供等价的 clone/fetch；
  - 进度桥接更稳定（对象数/字节、Checkout 映射）；
  - 取消响应：在传输与 checkout 回调中检查中断，返回 `User` 错误即视为取消；
  - 错误分类统一：Network/Tls/Verify/Protocol/Auth/Cancel/Internal；
  - Rust/前端所有测试通过（详见 §8）。

---

## 2. 代码结构与关键文件

后端（Rust/Tauri）：
- `src-tauri/Cargo.toml`
  - 依赖变更：移除 gix（gitoxide）相关；新增并固定 `git2 = "0.19"`。
  - Tauri 相关依赖通过可选特性 `tauri-app` 控制，便于 `cargo test` 纯后端构建。
- `src-tauri/src/core/git/`
  - `service.rs`：定义统一接口 `GitService` 与进度负载 `ProgressPayload`。
  - `default_impl.rs`：基于 git2-rs 的默认实现，提供 `clone_blocking` 与 `fetch_blocking`；桥接进度、支持取消、错误分类。
  - `errors.rs`：错误类别与 `GitError` 封装。
- `src-tauri/src/core/tasks/registry.rs`
  - 任务注册、状态管理、事件发射；`spawn_git_clone_task`/`spawn_git_fetch_task` 调用 `DefaultGitService` 并桥接进度事件。
- `src-tauri/src/events/emitter.rs`
  - 事件统一发射；在无 `tauri-app` 特性时为 no-op，便于核心测试。
- `src-tauri/src/app.rs`
  - Tauri 命令暴露：`git_clone`/`git_fetch`/任务命令/`http_fake_request` 等；维持既有签名不变。

前端（Vite/Vue）：
- 不涉及破坏性改动；所有 API 与事件契约保持不变，测试已覆盖。
- 新增 `src/api/tauri-fetch.ts` 作为 Fetch 兼容层，内部调用 `http_fake_request`：默认注入 `User-Agent: fireworks-collaboration/tauri-fetch`，并完整保留调用方提供的 Authorization 等头部以确保 GitHub API 正常认证。

---

## 3. Git 实现细节（git2-rs）

### 3.1 Clone
- 使用 `git2::build::RepoBuilder` + `FetchOptions` + `RemoteCallbacks`：
  - `transfer_progress` 映射：
    - `objects` = `stats.received_objects()`
    - `bytes` = `stats.received_bytes()`
    - `totalHint` = `stats.total_objects()`（若为 0 则省略）
    - `percent` = `received/total*100`（上限 100）
    - `phase` = `Receiving`
  - `CheckoutBuilder::progress`：
    - 将 checkout 阶段映射为整体的 90%~100%（`map_checkout_percent_to_overall`）；
    - `phase` = `Checkout`；
- 开始与结束：
  - 在任务层（`TaskRegistry`）会预发 `Starting`，git 完成后补发 `Completed`（`percent=100`）。

### 3.2 Fetch
- 打开现有仓库，解析远程：
  - 传入 `repo` 参数可为远程名或 URL；为空时尝试 `origin`，再退到第一个远程；均无则报错。
- 使用 `Remote::fetch(&[], Some(&mut fo), None)` 复用远程配置的 refspecs。
- 进度同 `clone` 的 `transfer_progress`，phase 统一为 `Receiving`。

### 3.3 取消机制
- 任务注册时创建 `CancellationToken`；后台线程监听取消信号，并设置 `AtomicBool`。
- 在 `transfer_progress`/`checkout` 回调中检查该标志，返回 `false` 触发 `git2::ErrorCode::User`；
- 错误映射为 `ErrorCategory::Cancel`，任务层据此设置为 `Canceled`。

### 3.4 错误分类
- 规则见 `default_impl.rs::map_git2_error`：
  - `Network`：连接/超时/Net 类；
  - `Tls`：SSL/TLS 相关；
  - `Verify`：证书/X509；
  - `Auth`：401/403/auth 关键字（为后续 push 预热）；
  - `Protocol`：HTTP/协议错误；
  - `Cancel`：`ErrorCode::User`；
  - 其他 `Internal`。

---

## 4. 事件与进度契约（保持兼容）

- 事件通道：
  - `task://state`：`{ id, kind, state }`，state ∈ `pending|running|completed|failed|canceled`。
  - `task://progress`：
    - Clone/Fetch：`{ task_id, kind, phase, percent, objects?, bytes?, totalHint? }`
    - 兼容已有前端解析，允许附加可选字段。
- 阶段命名：`Starting|Negotiating|Receiving|Checkout|Completed`（任务层会补发 `Starting` 与 `Completed`）。

---

## 5. HTTP 伪 SNI 调试 API（MP0 基线）

- 命令：`http_fake_request(input)`，仅支持 https；
- 白名单：在 `AppConfig.tls.san_whitelist` 中配置，默认包含 `github.com` 与常见子域通配；
- 重定向：支持 301/302/303/307/308，最多 `max_redirects` 次，301/302/303 切换为 GET；
- 前端调用：通过 `tauriFetch` 封装触发，若未提供 `User-Agent` 会自动注入 `fireworks-collaboration/tauri-fetch`，同时保留 Authorization 等原始头部；
- 日志脱敏：`logging.auth_header_masked`（默认开启）将 Authorization 头替换为 `REDACTED`；
- 错误分类：根据错误消息映射为 `Verify/Tls/Network/Input/Internal` 前缀字符串（见 `app.rs::classify_error_msg`）。

---

## 6. 配置与默认值

- 配置文件：`AppConfig`（由 `config/loader.rs` 读写），默认路径位于系统应用目录。
- 关键项（与 MP0 相关）：
  - `tls.san_whitelist`: 域白名单（默认包含 github 域族）；
  - `logging.auth_header_masked`: true；
  - （MP0 未引入 Fake SNI for Git，MP1/P3 再接入自定义 subtransport）。

---

## 7. 已知限制与兼容性

- MP0 不提供 push；相关凭证回调、上传进度将在 MP1 引入。
- 浅克隆/部分克隆参数暂不暴露；规划于 P2 开启。
- 代理策略、IP 优选、证书 pin 等后续阶段推进；MP0 仅提供 HTTP 调试 API 的白名单与重定向控制。

---

## 8. 测试与验收

- 前端测试：`pnpm test` 全部通过（19 文件 / 75 用例）。
- 后端测试：`cargo test` 全部通过（多模块 0 失败）。
- 手动冒烟：
  - `git_clone` 本地临时仓库（Rust 集成测试已覆盖 git 工作流）
  - `http_fake_request` 对 github 域族进行 GET 并观察重定向链与脱敏日志。

---

## 9. 面向 MP1 的对接点

- Push（MP1.1）：
  - 在 `GitService` 中新增 `push_blocking`（带凭证回调）；
  - 进度桥接：`RemoteCallbacks::push_transfer_progress`（若可用），phase 拟定 `PreUpload|Upload|PostReceive`；
  - 错误分类沿用 `map_git2_error`，新增服务器拒绝与权限不足的协议级分类；
  - 取消策略：进入上传后不再自动重试（重试窗口在“进入上传前”）。

- 自定义 smart subtransport（MP1.2/方式A）：
  - 保持当前 `GitService` 抽象；
  - 在构建 `FetchOptions`/`Remote` 时注册自定义子传输，仅接管连接/TLS/SNI，HTTP 仍由 libgit2 处理；
  - 与 HTTP 调试模块共享白名单/证书验证策略；失败回退到默认传输。

- Retry v1（MP1.4）：
  - 根据 `ErrorCategory` 分类进行指数退避重试；
  - Push 仅在上传前允许重试；
  - 事件中可选上报 `retriedTimes`。

---

## 10. 迁移回顾与风险

- 已清理 gix 依赖，统一使用 git2-rs；
- 构建维度差异（Windows 下 libgit2 链接）已在 CI/本地验证通过；
- 若后续平台兼容问题出现：优先回退 git2 版本小更新；不再保留 gix 双栈开关。

---

## 11. API 速览（对前端保持不变）

- `git_clone(repo: string, dest: string): Promise<string /* taskId */>`
- `git_fetch(repo: string, dest: string, preset?: string): Promise<string /* taskId */>`
- `task_cancel(id: string): Promise<boolean>`
- `task_list(): Promise<TaskSnapshot[]>`
- `task_snapshot(id: string): Promise<TaskSnapshot | null>`
- `http_fake_request(input: HttpRequestInput): Promise<HttpResponseOutput>`

事件：
- `task://state` -> `{ id, kind, state }`
- `task://progress` -> `{ task_id, kind, phase, percent, objects?, bytes?, totalHint? }`

---

## 12. 附：与设计文档对齐

- 设计依据：`doc/TECH_DESIGN_git2rs.md` 的 MP0 章节；本实现严格按“保留前端契约、替换后端 git 实现、清理 gix 依赖、测试全绿”交付。
- 后续文档：MP1 详细设计与子传输方案见 `doc/TECH_DESIGN_P1A_git2rs_custom_transport.md`（占位，MP1 阶段填充）。
