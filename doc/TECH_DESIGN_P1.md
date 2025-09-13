# P1 阶段细化版行动指南与 Roadmap

> 目标：在不引入代理/IP 优选等网络复杂度的前提下，基于 gitoxide 补齐 Git 基本操作（Fetch/Push），落地首版“重试策略（Retry v1）”与更丰富的任务状态/事件模型，确保在常见网络抖动下具备更好的稳定性与可观测性。
>
> 本文框架与 P0 文档一致，重点围绕 P1 的范围、实现步骤、接口契约、测试与验收标准展开，便于开发/测试/文档协同。

---

## 0. 总览节拍（建议 2 周内完成）

| 子阶段 | 名称 | 主要交付 | 预估 | 依赖 |
|--------|------|----------|------|------|
| P1.1 | Git Fetch 基础 | 后端 `git_fetch` 命令 + 任务事件；前端按钮与进度呈现 | 1d | P0 任务/事件骨架 |
| P1.2 | Git Push 基础 | 后端 `git_push` 命令（HTTPS，凭证传入）+ 任务事件；前端表单与最小交互 | 2d | P1.1 |
| P1.3 | 重试策略 v1 | HTTP 客户端与 Git 任务集成的错误分类重试 + 指数退避 | 1.5d | P0 错误分类 |
| P1.4 | 任务状态/事件丰富 | 细化进度（objects/bytes/totalHint/phase）；新增 `task://error` | 1d | P0 进度与事件 |
| P1.5 | 前端整合与配置 | 新增/调整 API 封装、配置项（RetryCfg）与 UI 元素 | 1d | P1.3/P1.4 |
| P1.6 | 测试与发布 | 单测/集成/手动清单 + 文档更新 + tag | 0.5d | 前全部 |

说明：P1 不引入代理与 IP 优选，也不在此阶段将“伪 SNI”接入 Git 传输；这两项在 P3/P4 处理。

---

## 1. 范围边界（只做 / 不做）

| 只做 | 说明 |
|------|------|
| Git Fetch | 通过 gitoxide，支持远端更新获取（不做 shallow/partial）|
| Git Push | HTTPS 方式，凭证以“调用时参数”方式传入（不落盘、不持久化）|
| 重试策略 v1 | 仅对“可安全重试”的错误类别生效；指数退避 + 抖动 |
| 任务事件增强 | 进度字段丰富、独立错误事件 `task://error` |
| 配置扩展 | 新增 `RetryCfg`（max/backoff_ms/factor/jitter_ms）|

| 不做 | 理由 |
|------|------|
| Proxy（HTTP/SOCKS5） | 放在 P4 |
| IP 优选 | 放在 P5 |
| Git 伪 SNI | 放在 P3（统一 transport 替换）|
| Shallow/Partial | 放在 P2 |
| SPKI Pin/指纹事件 | 放在 P7 |
| 凭证持久化/安全存储 | 超出 P1 范围（需专门安全评审）|

---

## 2. 代码结构变化（最小增量）

后端（Rust / Tauri）：
- `src-tauri/src/api/git_api.rs`：新增命令
	- `git_fetch(repo: String, dest: String) -> Uuid`
	- `git_push(repo: String, dest: String, username: Option<String>, password_or_token: Option<String>) -> Uuid`
- `src-tauri/src/core/git/service.rs`（或等价位置）：
	- `start_fetch_task(...)`、`start_push_task(...)` 封装 gix 调用
	- 进度回调对接，填充 objects/bytes/total_hint/phase
- `src-tauri/src/core/util/retry.rs`：
	- `RetryCfg { max: u8, backoff_ms: u64, factor: f32, jitter_ms: u64 }`
	- `should_retry(category: ErrorCategory, attempt: u8, is_idempotent: bool) -> bool`
	- `backoff_delay_ms(attempt, cfg) -> u64`
- `src-tauri/src/core/http/client.rs`：
	- 集成重试（对 Network/Tls 的“早期失败”类别进行重试）
- `src-tauri/src/events/emitter.rs`：
	- 新增 `task_error(task_id, category, message)` 便捷函数

前端（Vue/TS）：
- `src/api/git.ts`：新增 `startGitFetch(...)`、`startGitPush(...)` 封装
- `src/stores/tasks.ts`：
	- `progressById` 增强：合并 objects/bytes/totalHint/phase
	- 订阅 `task://error`，将错误写入 `logs` 与任务快照
- 视图：
	- `GitPanel.vue`：新增 Fetch/Push Tab；Push 表单支持用户名+令牌（或仅令牌）
	- 显示最近一次错误（若有）与重试次数摘要

---

## 3. 配置文件扩展（P1 新增）

在 P0 的基础上，新增 Retry 配置：

```json
{
	"http": {
		"fakeSniEnabled": true,
		"fakeSniHost": "baidu.com",
		"followRedirects": true,
		"maxRedirects": 5,
		"largeBodyWarnBytes": 5242880
	},
	"tls": { /* 同 P0 */ },
	"retry": {
		"max": 3,
		"backoffMs": 300,
		"factor": 1.8,
		"jitterMs": 50
	}
}
```

约束与建议：
- Push 的重试必须更为保守：仅在“未发送请求体/未进入协议关键阶段”的早期错误上允许重试；否则可能造成重复写。
- 凭证通过调用参数传入，不写入配置文件；UI 应提示安全性与隐私风险。

---

## 4. 详细任务拆解（按子阶段）

### P1.1 Git Fetch 基础
目标：在已有 Clone 能力上补充 Fetch，以拉取远端更新。

后端：
1. 在 `service.rs` 中增加 `fetch(repo, dest, token, ...)` 的阻塞实现，放入 `spawn_blocking`；
2. 复用/扩展进度回调：对象计数、字节累计、阶段（Negotiating/Receiving/Resolving/UpdatingRefs）；
3. 取消支持：轮询 `CancellationToken`，并桥接至 gix 的中断回调；
4. 错误分类：网络 → Network；TLS → Tls；认证 → Auth；协议 → Protocol；取消 → Cancel。

前端：
1. `GitPanel.vue` 新增 Fetch Tab：输入 `repo` 与 `dest`；
2. 任务启动后复用任务列表与进度条；
3. 失败时在面板显著展示最近错误（从 `task://error` 抓取）。

验收：对同一仓库的已有工作副本执行 Fetch 成功（若无变更返回快速完成亦视为成功）。

#### P1.1 实际实现说明 (已完成)

| 项目 | 实现情况 | 备注 |
|------|----------|------|
| 依赖与版本 | 使用 gitoxide `gix = 0.73`（禁用默认特性，启用 `blocking-network-client`, `worktree-mutation`, `parallel`），并配合 `gix-transport = 0.48` 开启 `http-client-reqwest` 以支持 HTTPS | 解决“https 未编译支持”的问题；保持与现有 Clone 路径一致 |
| 后端命令 | 在 `src-tauri/src/app.rs` 暴露 Tauri 命令 `git_fetch(repo: String, dest: String, preset: Option<String>) -> Uuid` | `preset` 为可选 RefSpec 预设；返回任务 ID |
| 任务接线 | `TaskRegistry::spawn_git_fetch_task(app, id, token, repo, dest, preset)` 负责状态流转与事件发射 | 状态事件：`Pending → Running → Completed/Failed/Canceled`，并确保所有退出路径均正确收敛 watcher 线程 |
| Fetch 实现 | `core::git::fetch::fetch_blocking_with_progress(repo, dest, should_interrupt, on_progress, preset)` 采用阻塞 API 运行于 `spawn_blocking` 线程 | 预检查 `dest/.git` 存在；通过 `find_fetch_remote` 建立连接，`prepare_fetch` 后 `receive` |
| 取消 | 通过 `CancellationToken` 与 `AtomicBool` 桥接 gix 的 `should_interrupt` 回调 | 取消触发后在 gix 的安全检查点尽快返回；任务状态=Canceled |
| 进度事件 | 桥接阶段：`Negotiating` → `Receiving`；并透传 `objects/bytes/total_hint` | 统一发送 `task://progress`：包含 `percent`（钳制 0..100）、`phase`、`objects`、`bytes`、`totalHint` |
| RefSpec 预设 | 支持 UI 传入 `preset` 注入额外 refspec：<br/>- `branches`: `+refs/heads/*:refs/remotes/origin/*`<br/>- `tags`: `+refs/tags/*:refs/tags/*`<br/>- `branches+tags`: 同时注入两者 | 通过 `gix::refspec::parse` 解析为 `RefSpec` 并设置到 `gix::remote::ref_map::Options.extra_refspecs`；默认 `preset=None` 时依赖远端自身配置（避免 "MissingRefSpecs"） |
| 前端 API | `src/api/tasks.ts` 新增/扩展 `startGitFetch(repo, dest, preset?)` 封装 | 当选择“remote（默认）”时不下发 `preset`，其余选项透传 |
| 前端 UI | `src/views/GitPanel.vue` 新增 Fetch 区域与“RefSpec 预设”下拉（remote/branches/branches+tags/tags） | Fetch 按钮允许 `repo` 为空（从默认 remote 拉取）；进度展示 `phase/percent/objects/bytes`；运行中可取消 |
| Store | `src/stores/tasks.ts` 增强 `progressById` 合并可选字段并钳制百分比 | 兼容字段缺失与重复事件；避免顺序依赖导致的抖动 |
| 测试覆盖（后端） | Rust 单测覆盖：非仓库目录快速失败、取消前即刻中止、参数签名与预设分支 | 避免网络依赖；确保 `spawn_git_fetch_task` 新签名在测试中正确调用 |
| 测试覆盖（前端） | Vitest 覆盖：API 预设透传、View 交互（选择预设→调用参数校验）、事件进度渲染（phase/objs/bytes）、取消按钮 | 为避免事件顺序抖动导致的脆弱性，测试不依赖固定数组下标，且每个用例清理监听调用缓存 |
| 体验对齐 | 为保持一致体验，Clone 亦桥接了 `Negotiating/Receiving/Checkout` 三阶段并复用同一事件模型 | 不改变既有 `git_clone` API；仅增强内部进度回调与事件发射 |

实现要点与边界：
- `.git` 预检查：若 `dest` 不是有效工作副本，`fetch` 会快速失败并返回明确错误，防止误用。
- 远端名称：预设 refspec 默认指向 `origin`；后续可选在 UI/后端参数中开放远端名配置（当前非必须）。
- 进度估算：`total_hint` 可能缺失或不精确，前端以阶段+百分比为主显示，并在 store 侧做百分比钳制。
- 线程收敛：无论正常、失败或取消路径，均确保 watcher 线程通过中断标记与 `join` 正常退出，避免悬挂。

验收状态：
- 本地 `cargo test` 与前端 `pnpm test` 全部通过；新增与修改用例稳定运行。
- UI 手动验证：选择不同预设触发 Fetch，能看到阶段/对象/字节进度；在中途取消，任务状态转为 Canceled 且进度停止。

### P1.2 Git Push 基础
目标：支持最基本的 HTTPS Push（Fast-Forward 或创建新分支）。

后端：
1. `push(repo, dest, auth)`：
	 - 鉴权：支持 Basic（用户名+PAT）或仅 Bearer/PAT；
	 - 保护：不记录凭证到日志；错误信息脱敏；
	 - 进度：复用 Fetch 的进度框架，增加 `phase=Uploading` 与 `bytes_sent`；
2. 取消：与 Fetch 等价路径；
3. 重试：仅在“连接/握手/初始探测”阶段的 Network/Tls 错误允许重试；一旦进入上传与服务端协商阶段则不再重试。

前端：
1. Push Tab：
	 - 输入：`repo`、`dest`、`branch`、`username`、`token`（或仅 `token`）；
	 - UI 提示：凭证仅用于本次调用，不会持久化；
2. 错误展示：明确区分 Auth 失败（401/403）与网络类失败。

验收：对测试仓库（或本地模拟远端）完成一次 push 成功；若权限不足，应给出 Auth 类错误并正确终止。

### P1.3 重试策略 v1
目标：在 HTTP 客户端与 Git 任务上实现统一的“可重试错误”策略与指数退避。

规则：
- 分类允许重试：Network、Tls（握手早期）、部分 HTTP 5xx；
- 分类拒绝重试：Verify、Auth、Protocol、Cancel、Internal；
- Push 特例：仅当 `is_idempotent=false` 且尚未进入上传阶段时允许重试；否则拒绝；
- 退避：`delay = backoff_ms * (factor^(attempt-1)) + rand(0..jitter_ms)`。

实现要点：
1. 在 `util/retry.rs` 提供纯函数与小型状态机，便于单测；
2. HTTP 客户端：封装一次请求的多次尝试（不跨重定向链）；
3. Git 任务：在外层包装“连接/握手/初始协商”阶段的重试循环，进入数据传输后退出重试；
4. 事件：可选发射 `task://progress` 子类型 `phase=Retrying(attempt)` 或在前端侧以错误重试计数汇总展示。

### P1.4 任务状态/事件丰富
目标：补充更细粒度的进度与错误事件，提升可观测性。

交付：
- `task://progress` 载荷扩展：`{ percent, phase, objects, bytes, totalHint, bytesSent? }`；
- `task://error` 新增：`{ taskId, category, message, attempt? }`；
- 失败收尾：任务状态=Failed；取消=Canceled；
- 前端 store：
	- `tasks.updateProgress` 合并可选字段并钳制 `percent`；
	- `logs.push` 写入最近错误，供全局提示与面板显示。

### P1.5 前端整合与配置
目标：使 Fetch/Push 可视化与可配置化。

交付：
- UI：`GitPanel.vue` 新增 Fetch/Push Tab；
- API：`src/api/git.ts` 增加 `startGitFetch` / `startGitPush`；
- 配置：在“设置”区域暴露 RetryCfg 的编辑入口（max/backoff/factor/jitter）；
- 安全：Push 凭证输入控件明确“不持久化”提示，并在发送前端日志中禁用打印。

### P1.6 测试与发布
目标：确保所有单测通过、关键链路手动脚本可复现，然后更新文档并打标。

交付：
- 单元测试：
	- `retry.rs`：允许/拒绝重试、backoff 计算、边界（attempt>max）
	- `tasks` store：扩展字段合并与钳制
- 集成测试（可选或部分条件跳过）：
	- Fetch 成功路径；
	- Push 推荐使用“受控环境”（本地裸仓/私有测试仓库）或增加 `GIT_TEST_REMOTE` 环境变量控制；CI 默认跳过 Push；
- 手动脚本：
	- 在本地搭建裸仓作为远端，完成 clone→commit→push；
	- 禁用网络后重试观察（限 HTTP 调试）
- 文档：更新 README 链接与总设计文档引用；
- 打标：创建 `v0.2.0-P1`（建议）。

---

## 5. 事件与数据格式（P1 增强）

| 事件 | Payload 示例 |
|------|--------------|
| task://progress | `{ "taskId":"...","kind":"GitFetch","phase":"Receiving","objects":240,"bytes":2097152,"totalHint":300 }` |
| task://error | `{ "taskId":"...","category":"Network","message":"connection reset","attempt":2 }` |
| task://state | `{ "taskId":"...","kind":"GitPush","state":"failed" }` |

前端需确保：
- 对未知字段保持向前兼容；
- 在同一任务上多次 `task://error` 时仅展示最近一条并保留计数。

---

## 6. 错误分类与重试决策（细化）

| 分类 | 示例 | 重试 |
|------|------|------|
| Network | connect timeout/reset, EOF | 允许（≤max）|
| Tls | handshake 超时/EOF | 允许（仅早期）|
| Verify | SAN mismatch | 禁止 |
| Auth | 401/403（Push 权限不足） | 禁止 |
| Protocol | git pack 解码错误 | 禁止 |
| HTTP5xx | 500/502/503/504 | 允许（≤max）|
| Cancel | 用户取消 | 禁止 |
| Internal | 代码错误 | 禁止 |

Push 额外规则：一旦进入 `Uploading` 阶段，后续错误不再自动重试。

---

## 7. 测试清单（P1）

| 类别 | 用例 | 预期 |
|------|------|------|
| Retry | attempt=1..max 退避时间单调递增（含 jitter） | OK |
| Retry | Verify/Auth/Protocol 不重试 | OK |
| Fetch | 已有工作副本拉取 | Completed |
| Push | 权限不足时分类=Auth | Failed(Auth) |
| Push | 受控远端成功 push（或本地裸仓） | Completed |
| 事件 | 连续 error 仅保留最近消息 + 计数 | OK |
| 前端 | Fetch/Push Tab 表单与展示 | OK |

---

## 8. 验收指标（P1 Done Definition）

| 指标 | 标准 |
|------|------|
| 功能 | Git Fetch/Push 可用（受控环境），HTTP 客户端具备 Retry v1 |
| 任务 | 进度更细粒度、错误事件可见，取消行为一致 |
| 稳定 | 常见网络抖动下可自动重试并成功；失败日志清晰 |
| 安全 | 凭证不落盘、不打印，日志脱敏延续；对 Auth 失败明确提示 |
| 文档 | README/TECH_DESIGN/本文件对齐并可指导新贡献者 |

---

## 9. 风险与即时缓解（仅 P1）

| 风险 | 触发 | 缓解 |
|------|------|------|
| Push 非幂等重试 | 不当重试导致重复写 | 限定仅在早期错误重试；进入上传后不再重试 |
| 凭证泄漏 | 日志或崩溃栈暴露 | 全面脱敏；从不写入配置；内存仅短期持有 |
| 进度估算波动 | totalHint 不准确 | 以阶段 + 百分比为主，提示“估算值” |
| CI 不可用 Push | 无公网/权限 | 将 Push 集成测试置为可选，默认跳过 |

---

## 10. 进入 P2 的准备点

| 未来点 | P1 准备 |
|--------|---------|
| Shallow/Partial | gix 能力调研与参数面板占位（depth/filter）|
| 任务级策略覆盖 | 在任务启动参数中允许覆盖 RetryCfg（不落盘）|
| Redirect 安全策略 | P1 收集案例，P2 统一策略（仅同域/白名单）|
| 进度展示优化 | 前端为对象/字节/速率预留展示位 |

---

## 11. 开发顺序（建议）

1. Fetch 后端 + 事件接线 + 前端按钮
2. Push 后端（凭证参数）+ UI 表单 + 基础错误分类
3. Retry v1 工具函数 + HTTP 客户端接入 + Git 任务早期阶段接入
4. 进度/错误事件增强 + 前端 store 与面板展示
5. 单测 + 手动脚本 + 文档修订
6. 打标发布：`v0.2.0-P1`

---

## 12. 快速手动验收脚本（建议）

```powershell
# 1) 准备一个本地裸仓作为远端（建议在临时目录）
# 2) Clone -> 在工作副本做一次提交 -> Fetch -> Push（受控环境）
# 3) 断网/限速 -> 触发 HTTP 重试（仅调试）
# 4) 检查前端：任务状态/进度/错误展示与计数
# 5) 检查日志：无凭证明文，错误分类合理
```

---

## 13. 关联文档

- P0 阶段细化版：`doc/TECH_DESIGN_P0.md`
- 综合技术方案：`doc/TECH_DESIGN.md`

