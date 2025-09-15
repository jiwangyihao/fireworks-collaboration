# 技术方案（MP1 阶段路线图）——Push + 自定义 Subtransport(A) + Retry v1 + 事件增强

> 适用范围：以 MP0（git2-rs 迁移完成）为基线，保持前端 API/事件/任务模型不变，补齐 Push 能力，并灰度接入“方式A：自定义 smart subtransport（仅接管连接/TLS/SNI）”，同时引入统一重试（Retry v1）与事件增强；要求全部单测通过且可一键回退。

关联文档：
- 现状与总体设计（git2-rs 版本）：`new-doc/TECH_DESIGN_git2rs.md`
- 旧版 P0 交接稿：`doc/TECH_DESIGN_P0_HANDOFF.md`
- 旧版 P1 设计（历史语境）：`doc/TECH_DESIGN_P1.md`
- MP0 计划与交付：`new-doc/TECH_DESIGN_MP0_PLAN.md`

---

## 0. TL;DR

- 本阶段目标（MP1）：
  - MP1.1：打通 HTTPS Push（凭证回调、进度/取消/错误分类完善）；
  - MP1.2：灰度启用“方式A：自定义 smart subtransport”，仅接管连接与 TLS/SNI，保留自动回退；
  - MP1.3：git push 使用自定义 smart subtransport（方式A），按白名单灰度启用并确保回退；
  - MP1.4：引入统一 Retry v1（指数退避+类别化）；
  - MP1.5：事件增强（push 阶段化进度、标准化错误事件）。
- 不变：任务模型、命令/事件命名、前端现有 UI 和 Store 结构保持兼容；新增字段保持可选，前端容忍未知字段。
- 可回退：
  - Push 可通过功能开关关闭（后端停止暴露命令或返回“未启用”错误）；
  - 自定义 subtransport 默认关闭，可按仓/域白名单灰度；失败自动回退 libgit2 默认路径；
  - Retry v1 可通过配置禁用或调低阈值。

状态更新（2025-09-15，MP1.3 已落地）：
- Push 已对接自定义 smart subtransport（方式A），保持 clone/fetch 行为不变并可灰度启用；
- 在 push 流程的 receive-pack 阶段（GET info/refs 与 POST /git-receive-pack）按需注入 Authorization 头（线程局部存储），避免明文外泄；
- 将 receive-pack 返回的 401 明确映射为 Auth 类错误，避免出现“bad packet length”类误导信息；
- 统一配置读取路径到应用数据目录（app config dir），subtransport 热加载最新配置；
- 新增/完善公共 E2E（clone/fetch）并默认启用，CI 环境可通过环境变量禁用；
- 重构：默认 Git 实现与传输层拆分为模块目录（`default_impl/*` 与 `transport/{mod.rs,register.rs,rewrite.rs,http/{mod.rs,auth.rs,util.rs,stream.rs}}`），并归档 legacy 文件；所有测试绿。
  - 说明：原 `transport/http.rs` 已归档到 `src-tauri/_archive/http.legacy_YYYYMMDD_HHMMSS.rs`；`transport/mod.rs` 统一对外导出 `ensure_registered`/`maybe_rewrite_https_to_custom`/`set_push_auth_header_value`，其中 `set_push_auth_header_value` 由 `http::auth` 模块实现并在 `mod.rs` 中 re-export；为避免历史同名文件造成模块歧义，使用 `#[path = "http/mod.rs"]` 指向目录模块。

---

## 1. 范围与目标

| 子阶段 | 核心目标 | 对用户可见变化 |
|--------|----------|----------------|
| MP1.1 Push | 支持 HTTPS Push（PAT/用户名+令牌），进度、取消与错误分类完善 | 新增 push 按钮/表单（可后置），日志脱敏 |
| MP1.2 Subtransport(A) | 白名单域启用自定义 smart subtransport，仅接管连接/TLS/SNI，失败自动回退 | 默认为关闭，灰度开关后网络兼容性提升 |
| MP1.3 Push+Subtransport(A) | 在 push 流程启用自定义 smart subtransport（仅接管连接/TLS/SNI），失败自动回退 | Push 在白名单域灰度启用，更好网络兼容性 |
| MP1.4 Retry v1 | 统一重试策略（指数退避+类别化），push 遵守“上传前可重试” | 失败时更稳健，错误消息包含重试计数 |
| MP1.5 事件增强 | push 阶段化进度与标准化错误事件（task://error） | UI 可显示更丰富进度/错误（保持兼容） |

不做（本阶段）：代理/IP 优选、浅克隆/部分克隆、LFS、SSH 兜底、指标面板等（详见 `new-doc/TECH_DESIGN_git2rs.md` 的后续阶段）。

---

## 2. 交付清单（Deliverables）

- 后端（Rust/Tauri，基于 git2-rs）：
  - 新增 git_push 命令；
  - Push 凭证回调（PAT / 用户名+令牌）；
  - Push 进度桥接（objects/bytesSent/percent/phase）；
  - 取消与错误分类（Auth/Network/Tls/Verify/Protocol/Cancel/Internal）；
  - 自定义 smart subtransport(A) 可插拔、白名单域灰度、失败自动回退；
  - 统一重试（Retry v1）：Clone/Fetch 遵守类别化重试，Push 仅在“进入上传前”允许重试；
  - 事件增强：`task://progress`（push 阶段化）与 `task://error`（可选）。
- 前端（保持兼容）：
  - 现有任务/事件模型不变；
  - 可按需增加 push UI（表单：仓库、分支、远端、凭证），非强制；
  - 支持展示新增的可选进度/错误字段（向后兼容）。
- 文档与测试：
  - 新增集成测试（push 至公共试仓库或本地模拟）；
  - 手册用例与回退指南；
  - 安全与脱敏检查。

---

## 3. 设计要点与决策

- Git Push（HTTPS）：
  - 凭证回调：
    - 仅令牌：用户名固定为 `x-access-token`（兼容 GitHub），密码为 token；
    - 用户名 + 令牌（或密码）。
  - 进度：`transfer_progress` +（若可用）`push_transfer_progress`，映射 bytesSent/objects/percent/phase。
  - 取消：在回调/循环中检查取消标记；进入上传后不再自动重试；取消立即中止。
  - 错误分类：401/403→Auth；连接/超时→Network；TLS/证书→Tls/Verify；用户取消→Cancel；其他→Protocol/Internal。
- 自定义 smart subtransport（方式A）：
  - 触发：对白名单域（如 github.com 域族）启用；
  - 行为：仅接管连接与 TLS/SNI；HTTP 语义仍由 libgit2 处理；
  - SNI 策略：默认 Real；可灰度 Fake→失败回退 Real；
  - 验证：保持链验证；可选 Real-Host 复核；
  - 回退：Fake→Real→libgit2 默认；错误归类与事件记录。
- Retry v1：
  - 可重试：超时、连接重置、暂时性网络、5xx（幂等阶段）；
  - 不可重试：证书验证失败、认证失败、用户取消、明确协议错误；
  - Push 特别规则：仅在“进入上传前”允许重试。
- 事件增强：
  - `task://progress` 新增可选字段：`bytesSent`, `phase`（`PreUpload|Upload|PostReceive`）；
  - `task://error` 标准化：`{ category, code?, message, retriedTimes? }`。

---

## 4. 子阶段拆解与验收

### 4.1 MP1.1 Git Push（HTTPS 基础）

- 目标：在 git2-rs 基础上实现 push，具备凭证回调、进度桥接、取消与错误分类；保持命令/事件兼容；日志脱敏。
- 实现要点：
  - 新命令：`git_push(repo: string, remote: string, branch: string, auth?: { token?: string; username?: string; password?: string })`；
  - `RemoteCallbacks::credentials` 回调按上述两种模式提供；
  - 进度：`transfer_progress` + `push_transfer_progress`（若可用），映射到 `task://progress`；
  - 取消：`AtomicBool`/CancellationToken，回调中尽早返回；
  - 错误分类：401/403→Auth；连接/超时→Network；TLS/证书→Tls/Verify；用户取消→Cancel；其他→Protocol/Internal；
  - 脱敏：默认不记录 Authorization/密码/令牌；调试模式下也要做脱敏。
- 接口影响：
  - 新增命令 `git_push`；其余命令/事件保持不变；
  - 事件新增的可选字段前端可无感；后续 UI 可以渐进展示。
- 测试与验收：
  - 单元：凭证回调覆盖两种模式；错误分类映射；取消路径单测；
  - 集成：对公共试仓库或本地裸仓库进行 push（可在 CI 使用本地仓库模拟）；
  - 验收：能成功 push；取消立即生效；错误分类符合预期；日志不泄漏敏感；
  - 回退：通过配置禁用 push 功能或让命令返回“未启用”。

#### MP1.1 实施说明（已落地）

本小节记录当前代码已实现的 MP1.1 具体行为与契约，便于对照验证与后续维护（保持与前端既有事件/任务模型兼容）。

- 命令（当前实现落地签名）
  - Tauri 命令：`git_push(dest: string, remote?: string, refspecs?: string[], username?: string, password?: string)`
  - 说明：
    - `dest`：本地仓库路径（必须是有效的 Git 仓库目录，含 `.git/`）。
    - `remote`：可选，未传则默认使用 `origin`。
    - `refspecs`：可选，形如 `refs/heads/<src>:refs/heads/<dst>`；未传则按远程配置的默认推送行为（与 `git2`/远端配置一致）。
    - `username`/`password`：可选的 HTTPS Basic 凭证。若仅提供令牌且 `username` 为空/未传，则后端自动使用 `x-access-token` 作为用户名以兼容 GitHub。
  - 事件：沿用现有任务事件通道
    - `task://state`：`pending|running|completed|failed|canceled`
    - `task://progress`：push 的阶段化进度（见下）
    - 注：计划中的标准化 `task://error` 事件在 MP1.1 中未启用；失败时通过 `state=failed` 并记录错误分类与消息。

- 进度与阶段（push）
  - Registry 预发：`Starting`（percent=0）
  - 协商阶段：`PreUpload`（来自 `transfer_progress`），附带 `objects/bytes/total_hint` 与 `percent`
  - 上传阶段：`Upload`（来自 `sideband_progress`，作为上传中信号，不保证精确百分比）
  - 服务器处理：`PostReceive`（成功路径上发出）
  - 完成：`Completed`（percent=100）

- 取消与中断
  - 使用 `CancellationToken` + 原子标志在回调中及时检查；用户取消会映射为 `Cancel` 类别并将任务置为 `canceled`。
  - 进入上传后不做自动重试（MP1.4 才引入统一重试策略）。

- 错误分类（当前映射）
  - `Auth`：如 401/403、认证失败相关信息
  - `Network`：超时、连接错误（含 `ErrorClass::Net`）
  - `Tls`：TLS/SSL 相关错误
  - `Verify`：证书链/主机验证错误
  - `Protocol`：HTTP/协议类错误（`ErrorClass::Http`）
  - `Cancel`：用户取消或回调中断
  - `Internal`：其他未归类错误

- 凭证与安全
  - 支持用户名+密码/令牌，或仅令牌（自动用户名 `x-access-token`）。
  - 敏感信息默认脱敏，不记录 Authorization/密码/令牌明文。

- 测试覆盖（示例）
  - 后端集成测试：`src-tauri/tests/git_push.rs`
    - 本地裸仓库 push 成功用例
    - 无效目标快速失败
    - 取消前/中途的取消路径
    - 任务注册表完成/取消语义
    - 阶段事件包含 `PreUpload`/`Completed`
  - 前端：已有 API/视图测试覆盖 push 启动参数透传与按钮交互（Vitest 全绿）。

- 使用与排错要点（GitHub 平台）
  - 使用 HTTPS push 时请使用 Personal Access Token（前缀通常为 `ghp_` 或 `github_pat_`），而非 OAuth App 访问令牌（`gho_`）。后者无法用于 Git push。
  - 将用户名留空或填写 `x-access-token`，密码处粘贴 PAT。
  - 如果组织启用了 SSO，请在 GitHub 的 Token 设置页为目标组织授权该 PAT。
  - 远端需为 HTTPS 形态；如为 SSH（`git@...:`），请改用对应的 HTTPS URL 或切换到 SSH 推送（不在 MP1.1 范围）。
  - 示例 refspec：`refs/heads/main:refs/heads/main`（推送本地 `main` 到远程 `main`）。

### 4.2 MP1.2 自定义 smart subtransport（方式A）灰度

- 目标：对白名单主机启用仅接管连接/TLS/SNI 的自定义 subtransport；失败自动回退；可一键关闭。
- 实现要点：
  - URL 识别与白名单：命中域名时启用；
  - SNI 策略：默认 Real；灰度控制 Fake→Real 单次回退；
  - TLS 验证：保持链验证；可选 Real-Host 复核；
  - 代理模式下默认禁用 Fake（避免异常指纹叠加）；
  - 事件：在调试级别记录 usedFakeSni/realHost 验证结果（可选字段）；
  - 回退链：Fake→Real→libgit2 默认；确保最终可用或明确失败类别。
- 接口影响：
  - 无破坏性变更；通过配置开启/关闭；
  - 事件可附带可选调试字段，前端容忍未知键。
- 测试与验收：
  - 单元：白名单判定、策略分支、回退链覆盖；
  - 集成：对 github.com 域族仓库进行 clone/fetch/push（若启用），观测回退；
  - 验收：默认关闭时行为与 MP0 等价；开启灰度后失败可自动回退，成功率不降低；
  - 回退：运行时配置关闭后立即恢复 libgit2 默认路径。

#### MP1.2 实施说明（已落地）

本小节汇总当前代码在 MP1.2 的实际实现，供对照验证：

- 自定义 Subtransport 与 URL 改写
  - 注册一次性的 `https+custom` subtransport，并在 clone/fetch/push 前根据配置与运行环境将 `https://` 改写为 `https+custom://`：
    - 命中 SAN 白名单域（`github.com/*.github.com/*.githubusercontent.com/*.githubassets.com/codeload.github.com`）
    - 已启用 Fake SNI 策略（灰度）
    - 当前不处于代理模式（代理下禁用 Fake SNI 与灰度改写）
  - 改写仅改变传输接管点，HTTP 智能协议语义仍由 libgit2 负责。

- SNI 选择与轮换
  - 统一从配置的 `http.fakeSniHosts: string[]` 候选中选择 Fake SNI；配置项 `fakeSniHost` 已从模型中移除。
  - 记录“最近成功 SNI”（last-good per real host），后续优先使用，提高成功率与稳定性。
  - 403 轮换（仅 `GET /info/refs` 阶段）：每个流最多一次，将当前 SNI 从候选中排除，随机选择其它候选；无候选或禁用则回退 Real SNI。
  - 代理存在时强制使用 Real SNI，并跳过改写。

- TLS 验证与开关
  - 默认：默认证书链/主机名校验 + 自定义 SAN 白名单校验；伪 SNI 握手时通过 override 以真实主机名进行白名单匹配，避免假 SNI 影响验证。
  - 新增开关拆分：
    - `tls.insecureSkipVerify`（默认 false）：跳过默认证书验证；
    - `tls.skipSanWhitelist`（默认 false）：跳过自定义 SAN 白名单验证；
    - 组合行为：
      - 两者均关：执行“链/主机名 + SAN 白名单”；
      - 仅关 SAN 白名单：仅执行“链/主机名”；
      - 仅关默认证书：仅执行“SAN 白名单”，作为最小安全闸；
      - 两者全关：完全跳过校验（仅原型调试用途）。

- 动态配置与路径一致性
  - 通过全局 base dir 统一配置加载路径（注入 app_config_dir），subtransport 在运行中也能读取最新配置（无需重启）。

- 观测性
  - HTTP 层有 Header/Body 预览（限长）嗅探，用于定位 Smart HTTP 早期失败；
  - 事件中可附带 usedFakeSni 等可选字段（调试），日志脱敏。

- 前端对齐
  - 前端已移除 `fakeSniHost` 字段，仅保留 `fakeSniHosts` 列表与 403 轮换开关；
  - TLS 设置面板新增“跳过 SAN 白名单校验”复选框。

### 4.3 MP1.3 Git Push 使用自定义 smart subtransport（方式A）

- 目标：在 push 流程中灰度启用仅接管连接/TLS/SNI 的自定义 smart subtransport（方式A），保障失败自动回退至 Real SNI 或 libgit2 默认路径，确保与现有 push 命令/事件兼容。
- 实现要点：
  - 对齐实现：参照 clone/fetch 的实现方式与约束，复用相同的 URL 改写、SNI 轮换/回退、代理互斥、TLS 校验与回退链路径，保证代码路径与配置项一致。
  - URL 改写：在 push 流程对命中白名单域的 `https://` 远端改写为 `https+custom://`，仅改变传输接管点；HTTP 智能协议仍由 libgit2 处理。
  - SNI 策略：优先使用 Fake SNI（来自 `http.fakeSniHosts` 候选，优先“最近成功”），若早期阶段（如 info/refs）出现 403/握手异常，轮换一次；仍失败则回退 Real SNI。
  - 代理互斥：检测到代理时强制使用 Real SNI，并跳过改写与 Fake SNI。
  - TLS 验证：保持链/主机名校验与 SAN 白名单策略；在 Fake SNI 握手中以真实主机名进行白名单匹配，避免假 SNI 干扰验证；尊重现有 `tls.*` 开关。
  - 事件与脱敏：可在调试级别附带 `usedFakeSni`、`sniCandidate` 等可选字段；敏感信息（Authorization/密码/令牌）一律脱敏。
  - 回退链：Fake → Real → libgit2 默认；发生回退时记录类别化错误，最终失败时清晰上报。
- 接口影响：
  - 无破坏性变更；通过配置开关与白名单控制启用范围；push 命令入参与事件保持既有契约。
- 测试与验收：
  - 单元：白名单判定、SNI 轮换与回退分支、代理分支；
  - 集成：对本地裸仓库与命中白名单域的远端进行 push，验证 Fake→Real→默认回退链；
  - 验收：默认关闭时与 MP1.1 行为一致；开启灰度后成功率不降低，失败有明确分类；支持一键关闭回退。

#### MP1.3 实施说明（已落地）

本小节记录当前代码在 MP1.3 的具体实现与契约，保持与 clone/fetch 路径一致，并确保灰度与回退：

- 接入与改写
  - 在 push 前对远端 URL 进行条件改写：命中 SAN 白名单且启用了 Fake SNI、且未启用代理时，将 `https://` 改写为 `https+custom://`，仅改变传输接管点；HTTP 智能协议仍由 libgit2 负责。
  - 代码位置：`src-tauri/src/core/git/transport/{mod.rs,register.rs,rewrite.rs,http/{mod.rs,auth.rs,util.rs,stream.rs}}`；对外导出 `ensure_registered`、`maybe_rewrite_https_to_custom` 与 `set_push_auth_header_value`（通过 `transport/mod.rs` 统一 re-export）。
  - 归档：原单文件 `transport/http.rs` 已迁移为目录模块并归档至 `src-tauri/_archive/`，避免与 `http/mod.rs` 重名导致的编译歧义。

##### 维护者速览（模块职责）

- `transport/mod.rs`
  - 模块装配与对外导出；通过 `#[path = "http/mod.rs"]` 绑定目录模块；re-export `set_push_auth_header_value`。
- `transport/register.rs`
  - 自定义 subtransport 的注册与一次性初始化（幂等）。
- `transport/rewrite.rs`
  - URL 改写逻辑（`https://` → `https+custom://`）与单元测试；白名单与代理判断。
- `transport/http/mod.rs`
  - 自定义 HTTPS smart subtransport 主体（`SmartSubtransport` 的 `action/close` 实现）、SNI 选择与 TLS 握手封装；依赖下述子模块。
- `transport/http/auth.rs`
  - 仅 Push 场景的 Authorization 线程局部注入（receive-pack 阶段）；对外通过 `transport/mod.rs` re-export。
- `transport/http/util.rs`
  - HTTP 解析与日志辅助（CRLF/双 CRLF 查找、首行与 Host 解析、Body 预览脱敏）。
- `transport/http/stream.rs`
  - `SniffingStream` 实现：GET/POST framing、分块/Content-Length/EOF 解码、早期 403 轮换与 401→Auth 映射、读写/seek 实现与调试嗅探。
  - 默认实现（`default_impl`）在 push 入口按与 clone/fetch 相同逻辑调用上述函数，保证行为一致与可灰度。

- Authorization 注入（仅 push）
  - 通过线程局部存储在进入 push 前设置 Authorization 值（Basic/PAT），由自定义 subtransport 在以下请求自动注入：
    - `GET /info/refs?service=git-receive-pack`
    - `POST /git-receive-pack`
  - clone/fetch（upload-pack）路径不注入，保持既有行为不变；所有日志默认脱敏。

- 错误映射与用户体验
  - 对 receive-pack 流程中返回的 401 显式映射为 `Auth`（PermissionDenied）类别，从而避免 libgit2 上层出现“bad packet length”等误导性错误；
  - 403 在 `GET /info/refs` 阶段触发一次性 SNI 轮换（排除当前候选、随机其余），仍失败回退 Real SNI；上传阶段不做自动重试。

- SNI 策略与回退
  - 候选来源 `http.fakeSniHosts: string[]`，优先使用“最近成功 SNI”（last-good per real-host）；
  - 握手失败自动回退 Real SNI；代理存在时强制 Real SNI 并跳过改写；
  - 回退链保持与 clone/fetch 一致：Fake → Real → libgit2 默认。

- TLS 与安全
  - 维持默认证书链/主机名校验；在 Fake SNI 握手时按真实主机名执行 SAN 白名单匹配；
  - 尊重 `tls.insecureSkipVerify` 与 `tls.skipSanWhitelist` 的组合开关；调试日志默认脱敏。

- 配置与热加载
  - 统一从应用数据目录加载配置（通过 Tauri 注入的 base dir），传输层实时读取，修改后即时生效；
  - Subtransport 注册（`ensure_registered`）为幂等操作，可重复调用；URL 改写函数具备单元测试覆盖。

- 测试与验证
  - Rust：全部单元与集成测试通过，包含 URL 改写、SNI 轮换与错误映射用例；
  - 前端：Vitest 全绿；公共 E2E（clone/fetch）默认启用，CI 下通过环境变量禁用；
  - 手动：面向 GitHub 的 push 验证了 Authorization 注入与 401 体验提升。

- 代码整洁度
  - 传输层从单文件重构为模块（`http/register/rewrite`），默认实现拆分为 `default_impl/*`；
  - 归档 legacy `transport.rs`，消除模块重名；
  - 调整内部可见性以消除 `private_interfaces` 编译警告，不影响行为。

### 4.4 MP1.4 Retry v1（统一重试策略）

- 目标：为 Clone/Fetch/Push 建立统一、可配置的重试策略；Push 仅在上传前允许重试。
- 实现要点：
  - 类别化：`{ retryable: Network/5xx | non-retryable: Verify/Auth/Cancel/Protocol }`；
  - 策略：指数退避 + 抖动（如 base=300ms, factor=1.5, max=6），可通过配置调整；
  - Push：若已开始上传 pack，不再自动重试；
  - 事件：在 progress 或 error 中附加 `retriedTimes`（可选）。
- 接口影响：
  - 无破坏性变更；新增配置项 `retry.*`；
  - 事件可增加可选字段。
- 测试与验收：
  - 单元：重试判定函数的分类与边界；退避计算；
  - 集成：模拟网络抖动/超时，观察重试次数与退出条件；
  - 验收：失败类别正确分类；最大重试后给出清晰错误；
  - 回退：配置中关闭或调低重试。

#### MP1.4 实施说明（已落地）

本小节记录当前代码在 MP1.4 的具体实现与契约，确保与既有任务/事件模型兼容：

- 配置与默认值
  - 模型：`retry: { max: number, baseMs: number, factor: number, jitter: boolean }`
  - 默认：`{ max: 6, baseMs: 300, factor: 1.5, jitter: true }`
  - 位置：`src-tauri/src/core/config/model.rs`，通过 `loader::load_or_init` 读取，任务层实时生效。

- 重试核心模块与行为
  - 模块：`src-tauri/src/core/tasks/retry.rs`
    - `RetryPlan`（由配置派生）、`backoff_delay_ms`（指数退避 ±50% 抖动）、`is_retryable`（按类别判断可重试）
  - 可重试类别：`Network`；`Protocol` 中的 HTTP 5xx（基于错误消息启发式匹配）
  - 不可重试：`Auth`/`Tls`/`Verify`/`Cancel`/`Internal` 等
  - 错误分类来源：`src-tauri/src/core/git/default_impl/helpers.rs::map_git2_error`；已覆盖 timeout/connect 等常见网络错误为 `Network`

- 任务注册表集成
  - 位置：`src-tauri/src/core/tasks/registry.rs`
    - `spawn_git_clone_task` / `spawn_git_fetch_task` / `spawn_git_push_task` 内部加入统一重试循环
    - 取消：每次尝试都有独立的原子中断标志与 watcher 线程，确保取消即时生效
    - 事件：重试前会通过 `task://progress` 发出“Retrying ...”阶段，并附带可选字段 `retriedTimes`

- Push 特别规则（仅上传前重试）
  - 进入上传阶段（`phase == "Upload"`）后不再自动重试；通过回调中标记 `upload_started` 实现
  - 仍保持用户手动取消立即生效

- 无效输入快速失败（不触发重试）
  - `DefaultGitService::clone_blocking` 在进入底层 clone 前进行输入校验：
    - 本地路径：若形如路径且不存在，直接返回 `Internal` 错误
    - URL：仅接受 `http/https`，或 scp-like（`user@host:path`）；其他（如 `ftp://...`、`not-a-valid-url!!!`）立即返回 `Internal`
  - 该路径保证明显错误不会进入重试循环，符合“invalid url should fail quickly”的预期

- 事件契约
  - `task://progress` 新增可选字段 `retriedTimes`（仅在重试阶段事件中出现，前端可无感）
  - 其余字段与语义保持不变

- 测试与验证
  - 单元：`retry.rs` 增加 5xx 可重试与退避抖动范围测试
  - 集成：注册表层新增 `invalid url/scheme` 快速失败用例，验证不进入重试
  - 公网 E2E：默认启用；在 CI 或无外网环境可通过环境变量禁用以保持稳定

### 4.5 MP1.5 事件增强与错误分类

- 目标：丰富 push 进度信息并标准化错误事件，保持兼容。
- 实现要点：
  - `task://progress`：push 增加 `bytesSent`, `objects`, `percent`, `phase`（`PreUpload|Upload|PostReceive`）；
  - `task://error`：`{ category, code?, message, retriedTimes? }`，其中 `category ∈ { Network, Tls, Verify, Protocol, Proxy, Auth, Lfs, Cancel, Internal }`；
  - 保持现有字段不变，新字段为可选。
- 测试与验收：
  - 单元：事件构造与脱敏；
  - 集成：推送与失败路径均能产出预期事件；
  - 验收：前端不需变更亦能正常运行；可选增强展示正常工作。

---

## 5. 命令与事件（契约）

- 新命令：`git_push`
  - 入参（示例）：
    - `repo: string`（本地仓库路径）
    - `remote: string`（默认 `origin`）
    - `branch: string`（要推送的本地分支，或 `refs/heads/<name>`）
    - `auth?: { token?: string; username?: string; password?: string }`
    - 可选：`force?: boolean`（P2+ 再评估）
  - 事件：
    - `task://state`：`pending|running|completed|failed|canceled`
    - `task://progress`：push 增强字段（可选）
    - `task://error`：标准化错误对象（可选）

---

## 6. 配置模型（MP1 初版）

- `retry`: `{ max: number, baseMs: number, factor: number, jitter: boolean }`
- `httpStrategy`: `{ fakeSniEnabled: boolean, enforceDomainWhitelist: boolean }`
- `tls`: `{ realHostVerify: boolean }`
- `logging`: `{ debugAuthLogging: boolean }`（默认脱敏）

注：策略可在后续阶段（P2+）支持任务级覆盖，MP1 暂不强制。

---

## 7. 安全与隐私

- 不记录 Authorization/密码/令牌等敏感信息；
- 调试日志也需要脱敏（仅显示掩码/长度）；
- 证书验证不降低安全基线：自定义 subtransport 仅改变 SNI，链验证照常；
- 代理模式下禁用 Fake SNI（减少可识别异常特征）。

---

## 8. 可观测性与诊断

- 事件已满足实时显示需求；
- 可在 debug 日志中附加：usedFakeSni、realHost 验证结果、retriedTimes（均为可选）；
- 指标/面板在后续阶段推进（参考 `new-doc/TECH_DESIGN_git2rs.md` §13/§19）。

---

## 9. 测试计划

- 单元测试：
  - 凭证回调路径（仅 token / 用户名+令牌）；
  - 取消早退；
  - 错误分类映射；
  - Retry 判定与退避；
  - Subtransport 白名单与回退链。
- 集成测试：
  - Push 至本地裸仓库（CI 友好）；
  - 网络错误模拟（连接复位/超时）验证重试；
  - 开/关 subtransport(A) 的行为对比；
  - 事件流完整性（state/progress/error）。
- 手动测试（补充）：
  - 对公共试仓库的 Clone/Fetch/Push；
  - 断网/代理等边界场景；
  - 脱敏校验与日志审阅。

---

## 10. 回退策略

- Push：配置开关禁用；命令立即返回“未启用”或在 UI 层隐藏入口；
- Subtransport(A)：默认关闭；灰度开启后如失败自动回退 Real / 默认 libgit2 路径；
- Retry v1：配置关闭或降低重试次数；
- 事件增强：保留可选字段，回退不影响既有消费端。

---

## 11. 里程碑与节奏（相对）

- W1-W2：MP1.1 Push 实现与单测；本地集成测试跑通；
- W3：MP1.2 Subtransport(A) 原型与灰度开关；回退链与单/集成测试；
- W4：MP1.3 Push 启用自定义 subtransport（A）灰度；Push 回退链与观测验证；
- W5：MP1.4 Retry v1 接入与分类打通；
- W6：MP1.5 事件增强与收尾；文档与回退演练；
- 持续：修复反馈、补齐边界；确保现有测试全绿。

（说明：具体节奏视 CI 与多平台联调进展微调，保持可回退。）

---

## 12. 风险与缓解

| 风险 | 等级 | 描述 | 缓解/回退 |
|------|------|------|-----------|
| Push 上传中断重复写入 | 中 | 自动重试导致重复写入 | 仅在上传前允许重试；清晰错误提示 |
| 自定义 subtransport 兼容性 | 中 | 某些网络策略/代理不兼容 | 默认关闭；Fake→Real→默认回退链；代理禁用 Fake |
| 错误分类不准 | 低 | 错误提示误导 | 分类表驱动+测试覆盖；保留原始 message |
| 凭证日志泄漏 | 高 | 敏感信息外泄 | 默认脱敏；审阅日志面板与测试 |
| 重试导致等待过长 | 低 | 用户体验下降 | 配置可调；最高重试次数限制；即时取消 |

---

## 13. 完成定义（DoD）

- 功能：Push + Subtransport(A) 灰度 + Push 启用自定义 Subtransport(A) + Retry v1 + 事件增强均可用；
- 兼容：前端无需改动即可运行；新增仅为可选字段；
- 质量：所有现有与新增测试全绿；无敏感日志；
- 可回退：任一子功能可单独关闭并恢复 MP0 行为；
- 文档：开发/测试/回退指南完整，关键配置有说明。

---

## 14. 实施 Checklist（开发者视角）

- [ ] 新增 `git_push` 命令与参数校验；
- [ ] 凭证回调（仅 token / 用户名+令牌）与脱敏；
- [ ] Push 进度桥接与取消；
- [ ] 错误分类与 `task://error` 事件；
- [ ] Subtransport(A) 白名单识别、SNI 策略与回退链；
- [ ] Retry v1：类别化判定与退避器；
- [ ] 配置开关：`retry.*`/`httpStrategy.fakeSniEnabled`/`tls.realHostVerify`；
- [ ] 单元/集成测试用例齐备（含本地裸仓库 push）；
- [ ] 文档更新与手册用例；
- [ ] 回退演练（开/关各子功能）。

---

（完）
