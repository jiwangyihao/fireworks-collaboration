# P2 阶段细化路线图与开发计划（本地 Git 操作 + Shallow/Partial + 任务级策略覆盖）

> 本文在 MP0/MP1 基线（git2-rs 已落地、Push + 自定义 smart subtransport(A) 灰度 + Retry v1 + 事件增强）之上，规划并拆解 P2 的交付目标、阶段划分与工程化实施细节，保持前端命令/事件/任务模型不变、可回退且测试完备。

---

## 0. 目标、范围与成功标准

- 目标
  - 本地 Git 常用操作：提供 init/add/commit/branch/checkout/tag/remote(set-url/add/remove) 等命令，统一事件/错误分类。
  - Shallow/Partial：clone/fetch 支持 `depth`（浅克隆/浅拉取）与 `filter`（部分克隆，如 `blob:none`）；在远端/环境不支持时平滑回退并清晰提示。
  - 任务级策略覆盖：允许在任务级通过 `strategyOverride` 覆盖 `http/tls/retry` 的安全子集，浅合并全局配置；互斥/越权项有护栏与告警。
  - 兼容：保持既有命令/事件/前端 UI 与 Store 结构兼容；新增字段均为可选，输入端容忍 snake_case/camelCase。
- 范围
  - 后端：git2-rs 实现的本地 Git 操作；clone/fetch 的 `depth/filter` 选项与回退；任务级策略覆盖（http/tls/retry 子集）。
  - 前端：命令入参扩展与事件订阅无需破坏性改动；可选增强展示（如“最近错误”已在 MP1.5 落地）。
- 不做（P2）
  - 代理能力与自动降级（P5）；
  - IP 优选与 IP 池（P4）；
  - 凭证安全存储（P6）；
  - LFS（P7）与指标面板（P8）；
  - SSH、系统 git 兜底。
- 成功标准（验收）
  - 单测/集成测试全绿；
  - 本地 Git 操作可在 Windows 上稳定运行（跨平台保持构建通过）；
  - depth/filter 在支持的远端上有效；远端不支持时按“最近原则”回退并给出 `Protocol` 类提示；
  - 任务级策略覆盖能生效、越权与互斥有护栏；
  - 与 MP1 的事件/错误分类保持一致，无敏感信息泄漏。

---

## 1. P2 分阶段与时间线（建议 2–3 周）

P2 拆分为 3 个可验收小阶段，阶段间可独立合入与回滚。

### P2.0 结构对齐与迁移（约 0.25–0.5 天）
- 背景：将所有 Git 功能按“命令=模块”的方式在 `core/git/default_impl/` 下进行平级拆分，避免区分 local/remote；已完成的 clone/fetch/push 也迁移为独立文件，公共助手保留在 helpers.rs/opts.rs。
- 范围：
  - 保留现有 `default_impl/helpers.rs` 与传输层结构不变；
  - 在 `core/git/default_impl/` 下新增独立命令模块文件：`clone.rs`、`fetch.rs`、`push.rs`、`init.rs`、`add.rs`、`commit.rs`、`branch.rs`、`checkout.rs`、`tag.rs`、`remote.rs`；
  - 将 `ops.rs` 中的 clone/fetch/push 内部实现迁移到对应独立文件（初期可先建立空壳并从 ops.rs 调用，以渐进迁移）；
  - 如需聚合 depth/filter/strategyOverride 的解析，新增 `opts.rs`（或先内联在 `helpers.rs`/`ops.rs`，后续再抽出）。
- 交付：
  - 编译通过（cargo build/test）；
  - 对外行为不变（DefaultGitService 仍对外暴露统一入口）；
  - 文档“实施细节与落点”同步为“平级模块化方案”。
  - 实施状态：已按“命令=模块”创建骨架：`clone.rs`、`fetch.rs`、`push.rs`、`init.rs`、`add.rs`、`commit.rs`、`branch.rs`、`checkout.rs`、`tag.rs`、`remote.rs`；其中 clone/fetch 通过桥接复用 `ops.rs` 现有实现，push 已迁移至独立 `push.rs`，其余命令返回 NotImplemented（Protocol）占位，确保后续 P2.1 渐进落地且对现有行为零影响。
- 验收：
  - 现有测试全绿；
  - 新增命令文件存在且可编译；已完成功能通过 `ops.rs` → 新文件的桥接调用，或已迁移完毕。
- 回滚：
  - 删除新建命令文件，回到 `ops.rs` 单文件实现；
  - 不影响 `helpers.rs` 与 `transport/*`。

#### 实现说明（已落地）

- 代码落点与结构
  - 目录：`src-tauri/src/core/git/default_impl/`
  - 新增模块（命令=模块，平级）：
    - 已桥接/迁移：
      - `clone.rs`（桥接到 `ops::do_clone`）
      - `fetch.rs`（桥接到 `ops::do_fetch`）
      - `push.rs`（从 `mod.rs` 迁移为独立实现，保持原行为；内部在执行前调用 `ensure_registered`，并沿用 `set_push_auth_header_value` 的授权注入）
    - 骨架占位（返回 NotImplemented/Protocol，待 P2.1 渐进实现）：
      - `init.rs`、`add.rs`、`commit.rs`、`branch.rs`、`checkout.rs`、`tag.rs`、`remote.rs`
  - `mod.rs` 已 `pub mod` 注册上述文件，并将 `DefaultGitService` 的内部路由改为调用对应模块函数（clone/fetch → 新模块桥接；push → 新模块实现）。
  - 兼容保留：`helpers.rs` 与 `ops.rs` 均保留不变；clone/fetch 仍由 `ops.rs` 提供内部实现，确保对外行为零变更。

- 测试与验证（禁公网 E2E 场景）
  - 新增后端测试文件：
    - `tests/git_local_skeleton.rs`：覆盖本地命令骨架当前返回 `Protocol`（NotImplemented）的契约；用于在正式实现前锁住行为。
    - `tests/git_clone_preflight.rs`：覆盖 clone 的快速失败分支（不存在的本地源路径、非法 URL scheme、无效 repo 字符串）。说明：为保证跨平台稳定，避免使用 `https:///missing-host` 这类在部分 Windows 工具链下可能触发崩溃的输入，改用稳定的 `mailto:abc` 无效字符串用例。
    - `tests/git_preconditions_and_cancel.rs`：覆盖取消与前置条件路径（clone 立即取消→`Cancel`；fetch 目标缺少 `.git`→`Internal`；在本地空仓库上 fetch 立即取消→`Cancel`）。
  - 运行方式（PowerShell）：
    - 在 `src-tauri` 目录禁用公网 E2E 后运行：
      ```powershell
      $env:FWC_E2E_DISABLE = 'true'
      cargo test -q
      ```
    - 仅运行 clone 预检测试：
      ```powershell
      $env:FWC_E2E_DISABLE = 'true'
      cargo test --test git_clone_preflight -q
      ```
  - 结果：在禁用公网 E2E 的前提下，后端全部测试通过；前端 `pnpm -s test` 亦全绿。公网 E2E 可在网络可达时移除该环境变量进行验证。

- 兼容性与回滚
  - 对外 API、事件/进度/错误分类不变；仅内部实现与文件结构重构。
  - 回滚可按模块粒度移除新文件并恢复 `mod.rs` 调用路径；测试可保留（骨架测试可临时跳过或调整）。

### P2.1 本地 Git 操作（约 0.5–0.75 周）
- 范围：
  - 新增命令：`git_init`、`git_add`、`git_commit`、`git_branch`、`git_checkout`、`git_tag`、`git_remote_set`、`git_remote_add`、`git_remote_remove`。
  - 事件：以 `task://state` 为主；必要时补充一次 `task://progress`（phase=Running, percent=100）。
  - 错误分类：按现有分类映射 `Internal/Protocol/Cancel` 等；冲突/不可快进 → `Protocol`。
- 交付：
  - 后端命令实现与 Tauri 注册；
  - 最小单测矩阵：每条命令覆盖成功/参数非法/用户取消各 1 例；
  - 文档与示例。
- 验收：
  - `cargo test` 通过；
  - 在临时目录跑通一组真实操作链（init→add→commit→branch→checkout→tag→remote_set/add/remove）。
- 回滚：
  - 以命令粒度回退；不影响已有 clone/fetch/push 流程。

#### P2.1 微阶段（可独立验收）

- P2.1a init + add
  - 范围：实现 `git_init` 与 `git_add`（含路径校验/工作区范围检查）；state 事件；必要时补 1 条 progress(100)。
  - 交付：成功/参数非法/取消 三例单测；Windows 路径规范化覆盖。
  - 验收：临时目录链路 init→add 跑通；错误分类正确（Protocol/Internal/Cancel）。
  - 回滚：禁用对应命令导出，保留骨架。

- P2.1b commit
  - 范围：`git_commit`（author 可选、allowEmpty 默认 false）；拒绝空提交（除非 allowEmpty）。
  - 交付：成功/空提交被拒/取消 单测；消息编码与脱敏检查。
  - 验收：init→add→commit 链路可复用；事件有序。
  - 回滚：禁用命令导出，其他命令不受影响。

- P2.1c branch + checkout
  - 范围：`git_branch`（force/是否立即 checkout）与 `git_checkout`（create 可选）。
  - 交付：成功/已存在/不存在 分支用例；checkout 失败/取消覆盖。
  - 验收：commit→branch→checkout 链条稳定；冲突/不可快进映射为 Protocol。
  - 回滚：分别禁用命令导出。

- P2.1d tag + remote(set/add/remove)
  - 范围：`git_tag`（轻量/附注、force）与 `git_remote_*`（set 覆盖、add 要求不存在、remove 要求存在）。
  - 交付：每命令 3 例单测；幂等性断言；远端 URL 校验。
  - 验收：全链路演示 init→add→commit→branch→checkout→tag→remote_set/add/remove。
  - 回滚：按命令粒度禁用导出。

### P2.2 Shallow/Partial（约 0.75–1 周）
- 范围：
  - `git_clone`/`git_fetch` 入参扩展：`depth?: number`、`filter?: 'blob:none'|'tree:0'`；
  - 进度桥接沿用 MP0/MP1；
  - 远端不支持 `filter` 时回退（优先保留 depth），并通过 `task://error(Protocol)` 追加一条非阻断提示。
- 交付：
  - 后端实现与参数校验（非法值立即 `Protocol`）；
  - 回退策略与错误事件；
  - 单测/本地集成：对象与字节显著下降；不支持路径的回退用例。
- 验收：
  - 本地/公开小仓库验证 depth/filter；
  - 回退路径稳定，错误可读；
  - 与 Retry v1 协同（不改变参数组合）。
- 回滚：
  - 关闭扩展参数处理，恢复全量路径。

#### P2.2 微阶段（可独立验收）

- P2.2a 入参扩展与校验占位
  - 范围：为 clone/fetch 增加 `depth?/filter?/strategyOverride?` 的解析与校验，但不改变执行行为（暂不生效）。
  - 交付：非法参数立即 Protocol；记录解析日志；最小单测。
  - 验收：现有流程不变；新增参数传入不会影响结果（除非法参数报错）。
  - 回滚：移除解析分支与校验。

- P2.2b Shallow Clone（depth for clone）
  - 范围：在 clone 上实现 depth；进度桥接保持；与 Retry v1 协同。
  - 交付：公开小仓库验证对象/字节显著下降；回归测试通过。
  - 验收：depth=1 成功；0/负值报错；事件正确。
  - 回滚：禁用 depth 分支，回到全量 clone。

- P2.2c Shallow Fetch（depth for fetch）
  - 范围：在已有仓库上支持浅拉取；与远端配置兼容。
  - 交付：源仓新增提交→目标 fetch depth=1；对象/字节下降；单测。
  - 验收：状态与事件正确；错误分类合理。
  - 回滚：禁用 depth 分支，回到全量 fetch。

- P2.2d Partial Clone（filter for clone）
  - 范围：clone 支持 `filter=blob:none|tree:0`；能力探测/错误回退（优先保留 depth）。
  - 交付：支持路径成功用例；不支持路径触发回退并在完成后追加 Protocol 提示事件。
  - 验收：体积显著下降；回退提示清晰。
  - 回滚：禁用 filter 分支；保留 shallow。

- P2.2e Partial Fetch（filter for fetch）
  - 范围：fetch 支持 filter（若受限则回退 shallow/全量）。
  - 交付：支持/不支持两类用例；回退事件覆盖。
  - 验收：与 clone 一致的回退语义；事件分类正确。
  - 回滚：禁用 filter 分支；保留 shallow。

- P2.2f 兼容性与矩阵收束
  - 范围：整合 depth+filter 叠加；不同远端与平台的小样本矩阵；文档示例。
  - 交付：组合参数用例；边界条件（重试类别/取消/非法）覆盖；文档更新。
  - 验收：矩阵用例全绿；不支持路径均回退且提示一致。
  - 回滚：保留已验证较稳的子能力，关闭不稳项。

### P2.3 任务级策略覆盖（约 0.5–0.75 周）
- 范围：
  - 命令入参新增 `strategyOverride`（安全子集）：
    - `http?: { followRedirects?: boolean; maxRedirects?: number }`
    - `tls?: { insecureSkipVerify?: boolean; skipSanWhitelist?: boolean }`
    - `retry?: { max?: number; baseMs?: number; factor?: number; jitter?: boolean }`
  - 合并语义：浅合并全局配置；非法字段忽略并记录告警。
  - 护栏：若处于代理模式（未来 P5），强制 Real SNI 并追加一次 `Proxy` 类提示（不阻断）。
- 交付：
  - 模型/解析/合并实现；
  - 单测覆盖浅合并/越权字段忽略与告警；
  - 与 clone/fetch/push 贯通（不改变其行为约定）。
- 验收：
  - 覆盖用例通过；运行期修改不影响其他任务；
  - 事件/错误分类保持一致。
- 回滚：
  - 临时禁用覆盖解析，回到全局配置。

#### P2.3 微阶段（可独立验收）

- P2.3a 模型与解析
  - 范围：扩展配置模型与命令入参结构；兼容 snake_case/camelCase；忽略未知字段并告警。
  - 交付：解析与校验单测；无行为变更（尚未应用）。
  - 验收：现有命令不受影响；非法键被忽略且有告警。
  - 回滚：移除解析与模型扩展。

- P2.3b 应用于 HTTP（followRedirects/maxRedirects）
  - 范围：任务内浅合并覆盖 HTTP 策略；仅允许声明字段。
  - 交付：覆盖生效用例（不同任务不同策略）；并发任务互不干扰。
  - 验收：行为只影响本任务；事件与错误不变。
  - 回滚：移除 HTTP 覆盖应用，保留解析。

- P2.3c 应用于 TLS（insecureSkipVerify/skipSanWhitelist）
  - 范围：任务内浅合并 TLS 两个布尔开关；记录护栏日志。
  - 交付：开/关策略的用例；与自适应 TLS 路径兼容。
  - 验收：仅当前任务受影响；日志脱敏。
  - 回滚：移除 TLS 覆盖应用。

- P2.3d 应用于 Retry（max/baseMs/factor/jitter）
  - 范围：任务内覆盖 Retry v1 计划；Push 的“上传前可重试”约束保持。
  - 交付：可重试类别下的重试次数/退避生效；不可重试类别不变。
  - 验收：事件中可选 retriedTimes 正确；不改变参数组合与阶段语义。
  - 回滚：移除 Retry 覆盖应用。

- P2.3e 护栏与互斥
  - 范围：代理模式（未来 P5）下强制 Real SNI 的护栏与一次性 `Proxy` 提示事件；越权字段告警。
  - 交付：护栏单测；日志与事件检查。
  - 验收：互斥时覆盖被忽略且有提示；不阻断任务。
  - 回滚：关闭护栏逻辑（保留日志）。

- P2.3f 文档与示例
  - 范围：完善 strategyOverride 示例与前端参数传递说明；
  - 交付：README/技术文档更新；用例脚本。
  - 验收：跟随示例可复现差异化策略执行；CI 文档检查通过。
  - 回滚：保留代码实现，回退文档。


## 2. 技术方案拆解（P2 视角）

### 2.1 本地 Git 操作（git2-rs）
- 命令契约（建议初版）：
  - `git_init({ dest })` → 在目标目录初始化仓库；
  - `git_add({ dest, paths })` → 校验路径存在且在工作区内；
  - `git_commit({ dest, message, author?: { name, email }, allowEmpty?: boolean })` → 空提交默认拒绝；
  - `git_branch({ dest, name, checkout?: boolean, force?: boolean })`；
  - `git_checkout({ dest, ref, create?: boolean })`；
  - `git_tag({ dest, name, message?, annotated?: boolean, force?: boolean })`；
  - `git_remote_set({ dest, name, url })` / `git_remote_add({ dest, name, url })` / `git_remote_remove({ dest, name })`。
- 事件：大多数仅 `task://state`，必要时发一条 `task://progress { phase: "Running", percent: 100 }`。
- 错误分类：
  - 参数/存在性/工作区冲突 → `Protocol`；
  - 文件系统/权限 → `Internal`；
  - 用户取消 → `Cancel`。
- 实现要点：
  - 路径校验与规范化；
  - 幂等性：`remote_set` 覆盖、`remote_add` 要求不存在、`remote_remove` 要求存在；
  - 线程/同步：操作在任务线程中执行，遵守取消令牌。

### 2.2 Shallow/Partial（depth/filter）
- 入参扩展（对象重载，保持向后兼容）：
  - `git_clone({ repo, dest, depth?, filter?, strategyOverride? })`
  - `git_fetch({ repo, dest, preset?, depth?, filter?, strategyOverride? })`
- 参数约束：
  - `depth`: 正整数；`0`/负值→`Protocol` 错误；
  - `filter`: 首版仅允许 `'blob:none'|'tree:0'`；非法值→`Protocol`；
  - 二者可叠加：含义为“浅 + 部分”。
- 支持检测与回退：
  - 若远端/环境不支持 `filter`：
    - 优先保留 `depth`，回退为“仅浅”；
    - 若浅也不被支持（罕见策略限制），回退全量；
    - 任务完成后追加 `task://error { category: "Protocol", message: "partial unsupported; fallback=..." }`（不阻断）。
- 实现提示（git2-rs/libgit2）：
  - `RepoBuilder`/`FetchOptions` 设置 depth；
  - 对 `filter`：依据远端 capabilities 与库支持情况决定是否启用；若当前环境不可直接启用，按上述回退策略处理（后续版本可演进至更细粒度能力探测）。

### 2.3 任务级策略覆盖（strategyOverride）
- 模型：
```
strategyOverride?: {
  http?: { followRedirects?: boolean; maxRedirects?: number },
  tls?: { insecureSkipVerify?: boolean; skipSanWhitelist?: boolean },
  retry?: { max?: number; baseMs?: number; factor?: number; jitter?: boolean }
}
```
- 合并语义：与全局配置浅合并（仅声明字段）；
- 约束：越权/未知字段忽略并记录告警；代理模式（未来 P5）强制 Real SNI，并追加 `Proxy` 类提示。

---

## 3. 命令与事件（契约补充）

- 新增命令：`git_init/git_add/git_commit/git_branch/git_checkout/git_tag/git_remote_*`；
- 扩展命令：`git_clone`/`git_fetch` 支持 `depth`/`filter` 与 `strategyOverride`；
- 事件：
  - `task://state`：`pending|running|completed|failed|canceled`；
  - `task://progress`：保持既有字段；回退提示通过 `task://error` 追加；
  - `task://error`：`{ category, message, retriedTimes? }`（与 MP1.5 一致）。
- 兼容性：
  - 新字段均为可选；输入端容忍 snake_case/camelCase（内部统一 camelCase）。

---

## 4. 实施细节与落点（后端）

- 平级模块化结构（命令=模块）：
  - 保留现有：
    - `src-tauri/src/core/git/errors.rs`、`service.rs`、`default_impl/{helpers.rs,ops.rs,mod.rs}`、`transport/{mod.rs,register.rs,rewrite.rs,http/*}`
  - 新增（按 P2.0 引入骨架）：
    - `src-tauri/src/core/git/default_impl/{clone.rs,fetch.rs,push.rs,init.rs,add.rs,commit.rs,branch.rs,checkout.rs,tag.rs,remote.rs}`（逐步迁移，初期 NotImplemented 或从 ops.rs 转发）
    - 选择性新增 `src-tauri/src/core/git/default_impl/opts.rs`（聚合 depth/filter/strategyOverride 的解析），若改动小可先内联 `ops.rs`
  - 入口与导出：
    - `DefaultGitService` 保持对外统一入口，内部调用拆分后的命令模块函数；
    - 不改动 `service.rs` trait 与事件模型，避免上层改动。
  - 相关位置：
    - `src-tauri/src/core/config/model.rs`（扩展 Strategy 模型）
    - `src-tauri/src/core/tasks/registry.rs`（注册与事件发射、覆盖合并）
- 取消与并发：沿用任务注册表与取消令牌；本地命令路径在关键步骤检查取消。
- 日志与脱敏：不输出敏感信息；参数回显做裁剪。

---

## 5. 测试计划（最小可行）

- 单元/集成（后端）
  - 本地 Git 命令：每条覆盖“成功/参数非法/用户取消”；
  - Shallow：clone depth=1 与 fetch depth=1 对象/字节下降；
  - Partial：`filter=blob:none` 成功；远端不支持→回退并追加 `Protocol` 提示；
  - 覆盖：`strategyOverride` 合并与越权告警；
  - 事件：state 时序、progress/ error 字段与分类；
  - Retry：与 shallow/partial 组合时不改变参数，只在可重试类别下生效。
- 前端（保持兼容）
  - 命令入参与事件订阅无需破坏性改动；
  - 如新增 UI 钩子（可选），补充 Vitest。

---

## 6. 质量门禁与交付清单

- 质量门禁
  - Build: PASS（各平台 CI）
  - Lint/Typecheck: PASS
  - Unit/Integration: PASS
  - E2E 冒烟：在公开小仓库与本地仓库通过
  - 回滚预案：已记录
- 交付清单
  - 代码：本地 Git 命令 + depth/filter + 任务级策略覆盖
  - 文档：本计划、命令与参数说明、回退策略
  - 测试：新增/替换用例与说明

---

## 7. 风险清单与缓解

| 风险 | 表现 | 缓解 |
|------|------|------|
| 本地命令幂等/一致性 | remote/set 等产生意外覆盖 | 明确幂等语义与参数校验，单测覆盖 |
| partial 支持差异 | 不同远端实现不一致 | 能力探测 + 回退为 shallow 或全量，并追加提示 |
| 事件时序差异 | UI 进度显示异常 | 遵循既有时序；本地命令仅发 state/一次 progress |
| 覆盖策略越权 | 覆盖无效或误导 | 忽略越权 + 告警；文档清晰列出允许字段 |
| 与 Retry 协同 | 重试改变参数或阶段 | 参数固定；仅在可重试类别下尝试 |
| Windows 路径问题 | 反斜杠/编码差异 | 路径规范化与测试覆盖 |

---

## 8. 回退策略

- 本地命令：逐条关闭或隐藏；不影响 clone/fetch/push。
- Shallow/Partial：关闭参数解析或强制回退到全量。
- 覆盖策略：禁用覆盖解析，回到全局配置。
- 文档与变更日志保留回退指引。

---

## 9. 任务分解（WBS）

0) P2.0 结构对齐与迁移（无行为改动）
- [ ] 在 `core/git/default_impl/` 下创建独立命令文件骨架（clone/fetch/push/init/add/commit/branch/checkout/tag/remote）（DoD：编译通过，函数返回 NotImplemented 或由 ops.rs 转发）
- [ ] 将 `ops.rs` 的 clone/fetch/push 调用改为桥接到对应新文件（DoD：对外行为不变，测试全绿）
- [ ] 视需要新增 `default_impl/opts.rs` 或临时内联在 `ops.rs`（DoD：现有实现与测试不变）
- [ ] 更新本文件“实施细节与落点”为“平级模块化方案”（DoD：文档与代码一致）

1) P2.1 本地 Git 操作
- [ ] P2.1a `git_init`/`git_add`（DoD：成功/非法/取消用例通过）
- [ ] P2.1b `git_commit`（DoD：空提交默认拒绝，三例用例通过）
- [ ] P2.1c `git_branch`/`git_checkout`（DoD：存在性/冲突与取消覆盖）
- [ ] P2.1d `git_tag`/`git_remote_*`（DoD：幂等/存在性规则与单测覆盖）

2) P2.2 Shallow/Partial
- [ ] P2.2a 入参扩展与校验占位（DoD：非法参数立即报错；无行为变更）
- [ ] P2.2b Shallow Clone（DoD：对象/字节显著下降）
- [ ] P2.2c Shallow Fetch（DoD：增量浅拉取成功）
- [ ] P2.2d Partial Clone（DoD：支持时成功；不支持时回退并追加提示事件）
- [ ] P2.2e Partial Fetch（DoD：与 clone 路径一致的回退语义）
- [ ] P2.2f 组合矩阵与文档（DoD：矩阵全绿与示例完备）

3) P2.3 任务级策略覆盖
- [ ] P2.3a 模型与解析（DoD：解析/校验单测通过）
- [ ] P2.3b 应用于 HTTP（DoD：仅当前任务生效，互不影响）
- [ ] P2.3c 应用于 TLS（DoD：仅当前任务生效，日志脱敏）
- [ ] P2.3d 应用于 Retry（DoD：重试计数正确，类别约束符合）
- [ ] P2.3e 护栏与互斥（DoD：代理下强制 Real SNI 并提示）
- [ ] P2.3f 文档与示例（DoD：示例可复现差异化策略）

4) 收尾
- [ ] 文档/变更日志；回退演练；CI 观察

---

## 10. 与后续阶段的衔接

- P3：自适应 TLS 传输层全量推广（默认启用，仍可关闭），沿用本阶段的任务级覆盖机制；
- P4：IP 优选/池对接，在连接前选择评分最高 IP；
- P5：代理支持与自动降级，互斥护栏与事件提示沿用；
- P6：凭证安全存储与脱敏检查；
- P7：LFS 基础与缓存；
- P8：可观测性面板与指标汇聚。

---

## 附：变更记录（本文件）
- v1: 初版（P2 细化拆解与计划）
- v1.1: 增加微阶段拆分（P2.1a–P2.1e，P2.2a–P2.2f，P2.3a–P2.3f）与细化 WBS/DoD，支持更小粒度的开发与验收
 - v1.2: 新增 P2.0 “结构对齐与迁移”，提出平级模块化方案，并将实施细节/WBS 与之对齐
 - v1.3: 结构方案调整为“所有 Git 功能平级模块”，包含 clone/fetch/push 拆分为独立文件与渐进迁移步骤
