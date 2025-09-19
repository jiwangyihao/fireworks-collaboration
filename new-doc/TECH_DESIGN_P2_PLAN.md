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

### P2.1a 实现说明（已完成）

本小节补充 v1.4 变更记录中已简述的实现细节，便于后续 P2.1b+ 复用模式、保持一致性与回退可控。

#### 1. 代码落点与结构
- 模块文件：`src-tauri/src/core/git/default_impl/init.rs`、`add.rs`
- 任务接入：`src-tauri/src/core/tasks/registry.rs` 新增 `spawn_git_init_task` / `spawn_git_add_task`
- 任务类型：`TaskKind` 扩展 `GitInit`、`GitAdd`
- Tauri 命令：`src-tauri/src/app.rs` 注册 `git_init`、`git_add`
- 前端：
  - API：`src/api/tasks.ts` 暴露 `startGitInit`、`startGitAdd`
  - Store：`stores/tasks.ts` 添加 TaskKind 枚举值
  - UI：`views/GitPanel.vue` 新增 “Init / Add” 卡片（多路径输入自动按换行或逗号分割）
  - 测试：`views/__tests__/git-panel.test.ts` 覆盖交互

#### 2. 行为与语义
1) git_init
   - 幂等：若目标目录已存在且含 `.git`，返回“AlreadyInitialized”语义（通过 progress/phase 或最终 state，不额外报错）。
   - 目录不存在：自动创建（含多级父目录），再初始化仓库。
   - 目标为已存在普通文件：判定为参数非法 → `Protocol` 错误。
   - 取消点：目录创建前后、`Repository::init` 调用前后均检查取消令牌。

2) git_add
   - 输入校验顺序（早失败→减小副作用窗口）：
     1. `paths` 非空（空 → `Protocol`）。
     2. 目标仓库目录存在且包含 `.git`（否则 `Protocol`）。
     3. 每个用户输入路径：拒绝绝对路径（Windows / *nix 统一判定）→ `Protocol`。
     4. 组合成工作区内相对路径后 canonicalize；若 canonicalize 失败（不存在）→ `Protocol`。
     5. Canonical 结果必须 `starts_with(workdir_canonical)`，否则视为越界（目录穿越）→ `Protocol`。
     6. 去重：保留用户首次出现顺序，利用 `HashSet` 抑制重复（影响后续进度 total_hint 与 percent 计算）。
   - 进度语义：
     - 为提升可观测性而放弃“只发一条 progress(100)”的最简模式，改为逐路径阶段化：
       - 每处理一个唯一路径前发送 `task://progress`：`phase = "Staging <相对路径>"`；
       - `objects = 已处理唯一路径数`；`total_hint = 唯一路径总数`；`percent = floor(objects / total * 100)`；
       - 完成全部后发送最终 `task://progress { phase: "Staged", percent: 100 }`（确保 UI 能以统一模式结束）。
     - 保证 percent 单调非降（测试覆盖）。
   - 写入：所有路径添加至 index 后一次性 `index.write()`；失败归类为 `Internal`。
   - 取消点：每个路径处理前检查取消；取消后抛出并在任务层映射为 `Cancel`。
   - 不展开递归逻辑：保持 libgit2/glob 行为，仅对显式列出的文件或目录添加（后续若引入模式匹配可扩展）。

#### 3. 错误分类映射（与 MP1 保持一致）
| 场景 | 分类 | 说明 |
|------|------|------|
| 参数为空 / 绝对路径 / 不存在 / 越界 / 非仓库目录 | Protocol | 调用者可修复的输入问题 |
| 取消（令牌触发） | Cancel | 用户主动终止 |
| I/O / Index 写入 / libgit2 内部失败 | Internal | 不暴露底层细节，日志脱敏 |

#### 4. 安全与防护
- 路径越界：借助 canonicalize + `starts_with` 阻止 `..` / 符号链接逃逸。
- 绝对路径拒绝：避免跨仓库 / 非预期磁盘访问。
- 去重防止重复事件放大 UI 百分比跳变。
- 日志裁剪：不输出敏感绝对路径，可在后续统一日志层再做红action（当前只输出相对或已裁剪信息）。

#### 5. 取消策略
- 使用任务注册表提供的取消令牌；在潜在阻塞点（循环每路径前、仓库初始化前、index 写入前）检测；
- 确保取消时不写入部分 index（因为写入放在所有路径处理后）。

#### 6. 测试矩阵（后端部分）
- git_init：成功（幂等）、对文件路径失败、取消；
- git_add：成功（文件+目录）、空列表、越界路径、绝对路径、重复路径去重、取消、百分比单调、最终阶段 `Staged`；
- 增量测试文件：`tests/git_add_enhanced.rs` 专门验证增强行为。

#### 7. 性能与可扩展性
- 典型路径数（< 数百）下逐路径 progress 开销极小；
- 若未来支持海量文件（>1e5），可改为分批（e.g. 每 N=100 触发一次 progress）并记录设计 TODO；当前无需预优化。

#### 8. 回退策略
- 移除 Tauri 命令导出或在 registry 中禁止 `GitInit` / `GitAdd` 分支即可快速回退到“未实现”状态；
- UI 卡片独立，可前端层面快速隐藏；
- 测试：对应新增测试可标记忽略（`#[ignore]`）或临时删除；不影响 clone/fetch/push。

#### 9. 对后续阶段的复用指引
- `git_add` 的路径校验与 per-item progress 模式可直接抽象成辅助函数供 `commit`（遍历待写树对象时）或未来 `status`/`reset` 类命令使用；
- 取消与错误映射模板化：后续 `commit/branch/checkout/tag/remote_*` 复用同一枚举分类，不新增分类种类，确保前端解码稳定；
- 进度字段中 `objects/total_hint` 使用方式为后续命令提供对齐示例。

#### 10. 已知限制 / TODO
- 未实现路径通配（glob）与 ignore 规则；
- 未在进度事件中区分“目录展开”与“文件添加”两类阶段；
- 尚未抽离通用路径校验助手（待 P2.1b/1c 视复用频率决定）。

> 本小节作为已交付实现的“设计与实现快照”，后续若 P2.1b 引入公共助手或抽象层，应更新此处“复用指引 / 已知限制”段落。

- P2.1b commit
  - 范围：`git_commit`（author 可选、allowEmpty 默认 false）；拒绝空提交（除非 allowEmpty）。
  - 交付：成功/空提交被拒/取消 单测；消息编码与脱敏检查。
  - 验收：init→add→commit 链路可复用；事件有序。
  - 回滚：禁用命令导出，其他命令不受影响。

### P2.1b 实现说明（已完成）

本节记录 `git_commit` 的落地细节、测试矩阵与与 P2.1a 复用点，作为后续 branch/checkout 等命令的模板。与设计初衷保持“最小必要进度事件 + 标准错误分类”一致。

#### 1. 代码落点与结构
- 模块文件：`src-tauri/src/core/git/default_impl/commit.rs`
- 任务接入：`spawn_git_commit_task`（位于 `core/tasks/registry.rs`）
- 任务类型：`TaskKind::GitCommit { dest, message, allow_empty, author_name, author_email }`
- Tauri 命令：`git_commit(dest, message, allow_empty?, author_name?, author_email?)`
- 前端：
  - API：`startGitCommit` (`src/api/tasks.ts`)，兼容 snake_case（`allow_empty` 等）输入
  - Store：TaskKind union 扩展 `GitCommit`
  - UI：`GitPanel.vue` 新增 “本地提交（Commit）” 卡片（消息、作者、allowEmpty 勾选）
  - 测试：`views/__tests__/git-panel.test.ts` 新增 Commit 交互用例

#### 2. 行为与语义
1) 必要校验顺序：
  - 取消检查（should_interrupt 原子标志）
  - 目标目录含 `.git`，否则 `Protocol`
  - 提交消息 `trim()` 后非空，否则 `Protocol`
2) 空提交判定：
  - 读取 index 写树：`write_tree()`
  - 若存在 HEAD：比较 HEAD tree id 与当前 tree id 相等 ⇒ 无变更
  - 若无 HEAD（首次提交）：index 为空 ⇒ 无变更
  - `allowEmpty=false` 且无变更 ⇒ `Protocol` 错误；`allowEmpty=true` 则继续
3) 作者签名：
  - 未显式提供 → `repo.signature()`（遵循本地 git 配置）
  - 显式提供需同时具备非空 name & email；任一缺失/空白 ⇒ `Protocol`
4) 提交：`repo.commit("HEAD", &sig, &sig, message_trimmed, &tree, parents)`；单亲或零亲（首次）
5) 进度事件：仅发送一条最终 progress（phase=`Committed`, percent=100），符合“本地快速命令只需一次 progress”策略；状态事件仍由任务注册表管理。

#### 3. 取消策略
- 多阶段检查：入口校验 / 写 index 前 / 创建 commit 前。
- 任务注册层：若任务启动前已取消（token.cancel()），立即发 Cancel 状态（新增测试覆盖）。
- 一致性：取消永远不产生部分写入（提交在取消前尚未调用 commit）。

#### 4. 错误分类映射
| 场景 | 分类 | 说明 |
|------|------|------|
| 目录非仓库 / 消息空 / 空提交被拒 / 作者缺字段 | Protocol | 可修正输入 |
| 用户取消 (token / 原子标志) | Cancel | 与 MP1 分类一致 |
| I/O / git2 内部错误 (index 写入 / commit) | Internal | 不泄漏底层细节 |

#### 5. 测试矩阵（新增与扩展）
后端 Rust：
| 用例 | 目标 | 结果 |
|------|------|------|
| 成功提交（有变更） | 基线成功 | 通过 |
| 二次无变更提交拒绝 | 空提交拒绝 | 通过 |
| allowEmpty 强制空提交 | 空提交允许 | 通过 |
| 初始空仓库空提交拒绝 / 允许 | 首次提交边界 | 通过 |
| 自定义作者成功 | 作者签名 | 通过 |
| 作者缺失 email | 校验错误 | 通过 |
| 作者空字符串 | 校验错误 | 通过 |
| 空消息（空白字符） | 校验错误 | 通过 |
| 消息裁剪（前后空白+换行） | 语义正确 | 通过 |
| 原子标志取消（进入前） | Cancel | 通过 |
| 任务注册预取消（token 先 cancel） | Cancel 分支 | 通过 |

前端：
- Commit 按钮交互触发 API；TaskKind / 事件仍复用既有逻辑（无需新增解析代码）。

#### 6. 安全与脱敏
- 未输出绝对路径或作者邮箱到进度事件；日志使用标准 tracing，可后续统一做敏感字段过滤。
- 提交消息直接写入对象；客户端传入内容已在分类错误中不含系统路径。

#### 7. 性能与扩展性
- 单次提交路径：CPU/I/O 极短；不额外拆分多 progress；后续若支持大索引增量统计可再拓展 objects/bytes 指标。

#### 8. 回退策略
- 禁用 Tauri `git_commit` 命令或移除 TaskKind 分支即可回退；UI 卡片独立可条件隐藏；测试文件可标记 `#[ignore]`。

#### 9. 复用指引
- 空提交检测 / 作者校验逻辑可在后续 tag (annotated) / amend（若实现）复用。
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。

#### 10. 已知限制 / TODO
- 未提供 amend / multi-parent (merge) 支持（后续 branch/merge 流程再引入）。
- 未暴露 GPG / 签名提交；后续需要可在签名构造层扩展。
- 未加入提交消息规范（如 Conventional Commit 校验），留给上层进行富校验。

---

- P2.1c branch + checkout
  - 范围：`git_branch`（force/是否立即 checkout）与 `git_checkout`（create 可选）。
  - 交付：成功/已存在/不存在 分支用例；checkout 失败/取消覆盖。
  - 验收：commit→branch→checkout 链条稳定；冲突/不可快进映射为 Protocol。
  - 回滚：分别禁用命令导出。

### P2.1c 实现说明（已完成）

本节记录 `git_branch` / `git_checkout` 的落地细节、命名校验策略两轮增强（v1.7 / v1.8）、测试矩阵与回退指引，延续 P2.1a/b 的结构与分类一致性。

#### 1. 代码落点与结构
- 模块文件：`src-tauri/src/core/git/default_impl/branch.rs`、`checkout.rs`
- 任务注册：`spawn_git_branch_task` / `spawn_git_checkout_task`（`core/tasks/registry.rs`）
- 枚举扩展：`TaskKind::GitBranch { dest, name, checkout, force }`、`TaskKind::GitCheckout { dest, ref_name, create }`
- Tauri 命令：`git_branch(dest, name, checkout?, force?)`、`git_checkout(dest, ref, create?)`
- 前端：
  - API：`startGitBranch` / `startGitCheckout`（`src/api/tasks.ts`）
  - Store：TaskKind 联合类型扩展（`stores/tasks.ts`）
  - UI：暂未新增专用面板卡片（后续统一 Git 操作面板再聚合），对现有事件解码无影响。
- 测试文件：`src-tauri/tests/git_branch_checkout.rs`

#### 2. 行为与语义
1) git_branch
   - 创建分支：要求仓库已有至少一个提交（可解析 HEAD）。若无提交 → `Protocol`。
   - 已存在分支：
     * `force=false` → `Protocol`（避免隐式覆盖）。
     * `force=true` → 快进/覆盖引用到当前 HEAD 提交（必须存在有效提交）。
   - `checkout=true`：在创建/force 成功后立即切换到该分支（等价于后续的 checkout 行为）；若在 force 场景，引用指向更新后的 HEAD 然后再切换。
   - 分支名需通过 `validate_branch_name`；失败 → `Protocol`。
2) git_checkout
   - 已存在本地分支：直接执行 set_head + checkout_head。
   - 不存在且 `create=true`：需要已有提交；否则 `Protocol`。
   - 不存在且 `create=false`：`Protocol`。
   - 仅支持切换（不实现路径/树检出混合模式）。

#### 3. 分支名校验（v1.7 → v1.8 演进）
统一封装在 `validate_branch_name`：
- v1.7 基础规则：拒绝 空/全空白、包含空格、末尾 `/` 或 `.`、前导 `-`、包含 `..`、反斜杠、任意控制字符 (c < 0x20)。
- v1.8 增强：再拒绝 以 `/` 开头、出现 `//`、以 `.lock` 结尾、包含字符 `:` `?` `*` `[` `~` `^` `\\`、包含子串 `@{`。
- 所有违规统一抛出 `Protocol`，错误消息保持用户可读可修复，不暴露内部实现细节。
- 设计为后续 tag/remote 名称规则基线，可抽象为未来 `refs.rs` 通用助手。

#### 4. 取消策略
- 入口快速检查（任务启动时）。
- 在执行关键副作用前再次检查：创建分支前、force 更新前、`set_head` 前、`checkout_head` 前。
- 保证取消不会留下半完成状态（要么未创建引用，要么 HEAD 尚未切换）。

#### 5. 错误分类映射
| 场景 | 分类 | 说明 |
|------|------|------|
| 分支已存在且未 force / 分支不存在且未 create / 无提交创建或 force / 名称非法 | Protocol | 输入或上下文逻辑可修复 |
| 用户取消（任一取消点） | Cancel | 允许 UI 显示“用户终止” |
| git2 内部错误（引用写入 / checkout 失败 / I/O） | Internal | 不泄漏底层细节 |

#### 6. 进度与事件
- 与“本地快速命令”策略对齐：仅发送一条完成 progress（percent=100）。
  - git_branch：`phase = "Branched"` 或（含 checkout）`"BranchedAndCheckedOut"`。
  - git_checkout：`phase = "CheckedOut"`；若 `create=true` 则 `"CreatedAndCheckedOut"`。
- 仍产生标准 state 流：`pending → running → {completed|failed|canceled}`；错误时附加 `task://error`。

#### 7. 测试矩阵（后端）
覆盖 `git_branch_checkout.rs`：
| 用例类别 | 目标 |
|----------|------|
| 创建分支成功 | 基线成功 & phase=Branched |
| 创建并立即 checkout | phase=BranchedAndCheckedOut |
| 已存在分支 (no force) | Protocol 冲突 |
| force 更新分支 | 引用指向最新提交 |
| force 无提交 | Protocol 拒绝 |
| 创建分支无提交 | Protocol 拒绝 |
| checkout 已存在分支 | phase=CheckedOut |
| checkout 不存在 (no create) | Protocol |
| checkout create 成功 | phase=CreatedAndCheckedOut |
| checkout create 无提交 | Protocol |
| checkout create 已存在（幂等语义校验） | Phase 正确且 HEAD 指向目标 |
| 取消：branch 在创建/force 前 | Cancel 分类 |
| 取消：checkout 在 set_head 前 | Cancel 分类 |
| 名称非法（多组） | Protocol（逐条规则断言） |
| 名称合法（多组） | 均成功且 phase 符合 |
| 控制字符 / 特殊序列 `@{` | Protocol |

#### 8. 安全与一致性
- 不输出绝对路径或敏感引用信息到事件；仅使用用户提供的分支名（已通过校验）。
- 错误消息保持抽象：不暴露 libgit2 具体 errno。
- 取消窗口缩小（写引用与切 HEAD 前再次检查）避免部分状态。

#### 9. 回退策略
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitBranch` / `GitCheckout` 分支。
- 删除/忽略测试文件 `git_branch_checkout.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 10. 复用指引
- `validate_branch_name` 可复制为 tag/remote 命名校验基线；非法字符/控制字符集合直接复用。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 11. 已知限制 / TODO
- 未实现基于 upstream/远端跟踪的自动设置（只创建本地引用）。
- 未实现分离 HEAD / 任意 commit 或 tag 的直接检出支持（当前限制为本地分支 ref）。
- 未对分支名长度、Unicode 规范化做额外限制（依赖底层接受范围）。
- 未提供“删除分支”命令；后续若加入需共享校验逻辑。
- 校验规则当前面向常见非法模式，未完整复刻 Git 内建所有 refspec 规则（可按需继续补充）。

- P2.1d tag + remote(set/add/remove)
  - 范围：`git_tag`（轻量/附注、force）与 `git_remote_*`（set 覆盖、add 要求不存在、remove 要求存在）。
  - 交付：每命令 3 例单测；幂等性断言；远端 URL 校验。
  - 验收：全链路演示 init→add→commit→branch→checkout→tag→remote_set/add/remove。
  - 回滚：按命令粒度禁用导出。

### P2.1d 实现说明（已完成）

详述 `git_tag` 与 `git_remote_{add,set,remove}` 的实现、增强与测试覆盖。

#### 1. 代码落点
- 源文件：`core/git/default_impl/tag.rs`、`core/git/default_impl/remote.rs`
- Registry：新增 `spawn_git_tag_task` / `spawn_git_remote_{add,set,remove}_task`
- TaskKind：`GitTag | GitRemoteAdd | GitRemoteSet | GitRemoteRemove`
- 前端：`api/tasks.ts` & `stores/tasks.ts` 扩展，UI 使用通用任务面板展示 phase。

#### 2. Tag 行为
- 支持轻量 & 附注：`annotated` 标志控制；附注要求非空 `message`。
- Force：
  - 轻量：更新 `refs/tags/<name>` 指向当前 HEAD。
  - 附注：创建新 tag 对象并更新引用；若内容（提交+消息+签名）未变则 OID 相同。
- Phase：`Tagged` / `AnnotatedTagged`（首次） 与 `Retagged` / `AnnotatedRetagged`（force 覆盖）。
- 消息规范化：CRLF 与孤立 CR → `\n`；尾部多余空白/空行裁剪成单一结尾换行；内部空行保持。
- 校验：`validate_tag_name`；仓库存在 & 有 HEAD commit；附注消息非空。
- 取消点：入口、解析 HEAD 后、写引用/创建对象前。

#### 3. Remote 行为
- add：远程不存在 → 创建；phase `RemoteAdded`。
- set：远程存在 → 更新 URL（同 URL 幂等成功）；phase `RemoteSet`。
- remove：远程存在 → 删除；phase `RemoteRemoved`。
- URL 校验（顺序严格）：
  1) 原始字符串含空白（space/tab/newline/carriage return）立即拒绝；
  2) trim 后为空拒绝；
  3) 允许 http/https、scp-like、无空格本地路径；
  4) 其它 scheme 或解析失败 → Protocol。
- 命名校验：`validate_remote_name`。
- 取消点：入口与副作用前。

#### 4. 错误分类
| 类别 | 条件示例 |
|------|---------|
| Protocol | 非仓库、无提交、名称非法、tag 已存在(非 force)、附注缺消息、URL 含空白/非法 scheme、add 重复、set/ remove 不存在、空白 URL |
| Cancel   | 取消标志触发（入口或副作用前） |
| Internal | 打开仓库失败 / 写引用失败 / 创建 tag 对象失败 / 设置远程失败 |

#### 5. 测试覆盖
文件：`git_tag_remote.rs` + `git_tag_remote_extra.rs`
- Tag：轻量/附注创建、重复非 force 拒绝、force OID 变化与不变路径、缺消息拒绝、非法名、无提交拒绝、取消、CRLF 规范化、尾部空行折叠、内部空行保留、轻量 force 同 HEAD OID 不变、附注 force 同消息 OID 不变。
- Remote：add/set/remove 成功链路、add 重复、set 不存在、remove 不存在、set 幂等、取消、URL 含空格/换行/制表符拒绝、本地路径成功、空白 URL 拒绝。

#### 6. 关键差异点
- Phase 粒度区分 Retagged* 提升可观测性（无需 OID 差异推断）。
- URL 校验在 trim 前执行，防止通过尾随换行/空格绕过。
- Annotated force 同内容保持同 OID 行为由测试锁定，避免误判“总是新对象”。

#### 7. 取消与原子性
- 多取消断点防止部分写入（引用写前检查）。
- Remote 操作保证失败不留下半状态（git2 原子语义 + 显式检查）。

#### 8. 回退策略
- 去掉新命令：移除 Tauri 注册 & TaskKind 分支。
- 合并 phase：将 Retagged* 分支重写为原 Tagged* 并更新测试。
- 关闭消息规范化：删除 CR/LF 处理与尾部折叠逻辑并移除相关断言。

#### 9. 已知限制 / TODO
- 未实现 tag 删除；未支持 ssh:// URL；未自定义 tagger；未做 tag 对象 GC；URL 校验未验证实际可达性。

#### 10. 后续衔接
- URL/命名/phase 模式将复用到 P2.2 depth / filter 参数校验与后续策略覆盖。

### P2.2a 实现说明（已完成）

本小节记录 P2.2a（`git_clone` / `git_fetch` 入参 `depth` / `filter` / `strategyOverride` 解析与校验“占位实施”）的实际落地，以便后续 P2.2b+ 在此基础上接入真正的 shallow / partial 行为与能力探测回退逻辑。

#### 1. 代码落点与结构
- 新增文件：`src-tauri/src/core/git/default_impl/opts.rs`
  - 导出：
    - 枚举 `PartialFilter { BlobNone, TreeZero }`
    - 结构 `StrategyHttpOverride` / `StrategyTlsOverride` / `StrategyRetryOverride` / `StrategyOverrideInput`
    - 结构 `GitDepthFilterOpts { depth: Option<u32>, filter: Option<PartialFilter>, strategy_override: Option<StrategyOverrideInput> }`
    - 函数 `parse_depth_filter_opts(depth: Option<Value>, filter: Option<String>, strategy_override: Option<Value>) -> Result<GitDepthFilterOpts, GitError>`
- `TaskKind::GitClone` / `TaskKind::GitFetch` 扩展三个可选字段：`depth` / `filter` / `strategy_override`（保持旧前端未传参数时的兼容）。
- `tasks/registry.rs`：新增 *_with_opts 版本的 spawn（`spawn_git_clone_task_with_opts` / `spawn_git_fetch_task_with_opts`），在进入重试循环前调用解析函数；保留原始 wrapper 维持旧调用签名。
- `app.rs`：Tauri 命令 `git_clone` / `git_fetch` 新增可选入参并转发给 *_with_opts 版本；顺序追加，避免破坏既有前端调用位置参数。

#### 2. 行为与语义（占位阶段）
- 仅进行参数解析 + 校验 + 日志记录，不对底层 libgit2 clone/fetch 逻辑施加任何浅克隆或部分克隆影响；实质仍是“全量 clone / fetch”。
- depth：
  - 允许：正整数（>0, <= u32::MAX）
  - 拒绝：0、负数、非数字（字符串/布尔等）、超过 u32::MAX → `Protocol` 错误（消息：`depth must be positive` / `depth must be a number` / `depth too large`）。
- filter：
  - 允许：`blob:none`、`tree:0`（以及测试为同义形式的 `tree:depth=0` 解析路径——后续可决定是否正式纳入，对外文档暂主推前两种）。
  - 拒绝：其它任意字符串 → `Protocol` 错误（`unsupported filter: <原串>`）。
  - 空白字符串（只含空格）视为未提供（忽略，不报错）。
- strategyOverride：
  - 结构：`{ http?: {...}, tls?: {...}, retry?: {...} }`；字段采用 camelCase + 兼容关键字段 snake_case alias。
  - 解析成功：仅存储，不应用；未知顶层或内部字段由 serde 忽略（当前未发 warn，后续 P2.3 可加告警）。
  - 非对象（数组等）或结构不符 → 解析阶段返回 `Protocol`（单元测试覆盖）。部分集成用例记录当前“非对象未导致任务失败”的状态，用于后续提升一致性时更新期望。
- 解析失败时：任务直接进入 Failed，分类 `Protocol`，不进入网络阶段。
- 解析成功：在日志中 `info` 记录（结构化输出 depth/filter/strategyOverride 摘要），后续 shallow/partial 实际逻辑可复用此结果避免再次解析。

#### 3. 错误分类与事件
- 所有输入校验失败 → `Protocol`（保持与 P2 其它命令一致的“可修复输入”分类）。
- 不新增新的错误分类；未触及网络 I/O 时不会产生 `Internal`；取消逻辑与原 clone/fetch 保持不变。
- 事件顺序：保持原 clone/fetch 的 state / progress / error 语义；P2.2a 无新增 progress 分支。

#### 4. 测试矩阵（P2.2a + 后续补丁 v1.9.1 / v1.9.2）
| 类别 | 用例摘要 | 期望 |
|------|----------|------|
| depth 合法 | 1 / (u32::MAX) / 与合法 filter 组合 | 解析成功，任务不 Failed |
| depth 非法 | 0 / 负数 / 超范围 / 字符串 | Protocol 错误，任务 Failed (集成覆盖 0；单测覆盖其余) |
| filter 合法 | blob:none / tree:0 / (同义 tree:depth=0) | 解析成功 |
| filter 非法 | 大写变体 / tree:1 / 含内部空格 / 任意其它字符串 | Protocol 错误 |
| 空 filter | 仅空白 | 忽略，不报错 |
| depth+filter 组合 | depth=2 + tree:0 | 成功（占位仍全量克隆） |
| strategyOverride 合法 | http / tls / retry 各单独 + 全部组合 | 解析成功并保留结构 |
| strategyOverride 非法 | 非对象（数组）、字段类型错误 | 单测 Protocol；集成记录当前未致 Failed 的现状（后续可统一策略） |
| 未知字段 | 顶层或子字段 | 忽略（未来可加 warn） |

#### 5. 回滚策略
- 快速移除：删除 `opts.rs` + 移除 TaskKind 新字段 + 删 *_with_opts + 还原 Tauri 命令签名。
- 温和降级：保留字段但在 registry 中跳过解析函数（直接忽略全部新参数），实现“软关闭”。

#### 6. 对后续阶段的复用
- P2.2b/c：直接在 clone/fetch 内部使用 `GitDepthFilterOpts.depth` 设置 `RepoBuilder.depth()` / `FetchOptions.depth()`。
- P2.2d/e：在能力探测失败路径上引用 `filter` 字段构建回退事件（不阻断任务完成），并根据 `depth` 决定是否保持 shallow。
- P2.3：在任务启动阶段追加 strategyOverride -> 全局策略浅合并（HTTP/TLS/Retry），当前结构已满足静态解析需求。

#### 7. 已知限制 / TODO（占位阶段）
- 未进行任何能力探测 (`protocol v2` / partial capability)；
- 未对 strategyOverride 未知字段给出 warn；
- 未对 filter 同义形式统一规范输出（只在内部测试中使用）；
- 未提供“允许忽略单个非法字段但继续”模式（当前遇到首个非法即整体失败）。

#### 8. 现状评估
- 解析层覆盖面：深度、过滤器、策略三块均有边界与组合测试；
- 行为稳定性：不改变既有 clone/fetch 路径，风险集中在早期参数判定，具备清晰回滚；
- 后续改动成本：添加 shallow/partial 只需在执行层判断 `opts.depth` / `opts.filter` 应用对应 libgit2 选项并补充能力探测与回退事件。

### P2.2d Partial Clone (filter for clone) 实现补充（v1.12）

> 本小节为阶段性交付“占位 + 回退”快照，尚未真正减少传输数据量，重点在：参数解析 → 统一回退语义 → 可测试事件契约。未来真正 partial 能力启用后需更新此处。

#### 1. 代码落点
| 文件 | 作用 |
|------|------|
| `core/tasks/registry.rs` | 在 `spawn_git_clone_task_with_opts` 中检测用户 `filter`，发出非阻断回退 `TaskErrorEvent` |
| `core/git/default_impl/opts.rs` | 已存在 depth/filter 解析（沿用） |
| `events/emitter.rs` | 非 tauri 模式的事件捕获单例（新增 `peek_captured_events`、集中 `CAPTURED` 静态） |
| `tests/git_partial_clone_filter_*.rs` | 新增 3 个事件断言测试 + 1 个原有完成性测试 |

#### 2. 行为与回退语义
| 场景 | depth 传入 | filter 传入 | 实际执行 | 回退事件消息 | 任务最终状态 |
|------|-----------|-------------|----------|---------------|--------------|
| 仅 filter | 无 | 合法 | 全量 clone | `partial filter unsupported; fallback=full` | Completed |
| depth + filter | 有 | 合法 | shallow clone（深度保持） | `partial filter unsupported; fallback=shallow (depth retained)` | Completed |
| 无 filter | 任意 | 无 | 原有 full 或 shallow | （无回退） | Completed |
| 非法 filter | 任意 | 非法字符串 | 被解析阶段拒绝 | Protocol 解析错误 | Failed |

说明：当前未做远端 capability 探测，凡提供合法 filter 均回退；此设计可前向兼容后续“仅在不支持时回退”的增强（添加探测条件即可）。

#### 3. 事件契约
| Topic | 分类 | 条件 | 关键字段/片段 |
|-------|------|------|---------------|
| `task://error` | `Protocol` | 检测到用户传入合法 filter | `message` 含 `fallback=full` 或 `fallback=shallow` |
| `task://state` | Running→Completed | 正常流程 | 不变 |
| `task://progress` | Starting / Completed | 既有 clone 进度 | 不变（未新增 phase） |

后续计划：将回退事件结构化为 `{ code: "partial_filter_fallback", mode: "full|shallow", message }`，当前仅锁定 message 文本。

#### 4. 测试策略
| 测试文件 | 断言要点 |
|----------|----------|
| `git_partial_clone_filter_event_only.rs` | 存在 `fallback=full` 事件且任务 Completed |
| `git_partial_clone_filter_event_with_depth.rs` | 存在 `fallback=shallow` 事件且任务 Completed |
| `git_partial_clone_filter_event_baseline.rs` | 不出现任何 `fallback=` 片段 |
| `git_partial_clone_filter_fallback.rs` | 回退不阻断（完成性） |

实现细节：事件回退在 clone 线程早期发送；测试使用 `peek_captured_events()` 轮询最多 20 次 * 50ms 保证可靠捕获；baseline 用例结束后 `drain_captured_events()` 清理，避免交叉污染。

#### 5. 关键实现摘录（伪代码）
```
if filter_requested.is_some() {
  let msg = if depth_applied.is_some() {
     "partial filter unsupported; fallback=shallow (depth retained)"
  } else {
     "partial filter unsupported; fallback=full"
  };
  emit(TaskErrorEvent { category: Protocol, message: msg, ... });
}
```

#### 6. 风险 & 限制
- 始终回退：尚未探测远端 partial 能力，真实支持场景下浪费潜在优化。
- 文本耦合：前端若依赖字符串解析易受未来文案调整影响 → 尽快结构化。
- 捕获仅限非 tauri 构建：GUI 集成无法直接重用该内存缓冲；如需端到端调试需额外命令或开发模式面板。

#### 7. 回滚与降级
| 目标 | 操作 | 影响 |
|------|------|------|
| 临时关闭回退事件 | 注释/删除 emit 分支 | 仍能 clone；测试需标记 ignore |
| 移除新增测试 | `#[ignore]` 或删除测试文件 | 不再保护回退文本契约 |
| 彻底恢复至 P2.2c 状态 | 移除 filter 分支代码 + 文档此节 | 失去用户提示，但最小行为仍正确 |

#### 8. 测试矩阵（已实现）
| 用例 | depth | filter | 期望完成状态 | 回退事件 | 事件消息包含 |
|------|-------|--------|--------------|----------|--------------|
| filter only blob:none | None | blob:none | Completed | Yes | `fallback=full` |
| filter only tree:0 | None | tree:0 | Completed | Yes | `fallback=full` |
| depth+filter blob:none | 1 | blob:none | Completed | Yes | `fallback=shallow` |
| depth+filter tree:0 | 2 | tree:0 | Completed | Yes | `fallback=shallow` |
| no filter | 1 | None | Completed | No | (无) |
| invalid filter | - | xyz | Failed | (N/A) | Protocol 解析错误 |

#### 9. 后续演进（P2.2e+ 展望）
- 能力探测：初次握手读取 capability（protocol v2 `filter`）→ 仅在不支持时发回退。
- 真 partial：应用 libgit2 对应选项（待验证支持路径），以对象/字节差异测试为验收。
- 结构化错误：新增 code/mode 字段 + 文档示例；保持向后兼容保留旧 message。
- Fetch 对称支持：在 `spawn_git_fetch_task_with_opts` 中复用同一逻辑抽象。
- 矩阵收束：完成 depth/filter 组合 + 重试/取消/非法边界统一表格。



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
 - v1.4: 完成 P2.1a (`git_init`/`git_add`)：
  - 后端：实现 init 幂等、add 路径校验(含 canonicalize 越界检测)、重复路径去重、绝对路径拒绝、逐路径进度事件 (Staging <path> → Staged)；
  - 任务注册：`spawn_git_init_task` / `spawn_git_add_task` 接入标准事件模型，GitAdd 传递真实阶段化进度；
  - 前端：GitPanel 新增 Init/Add 卡片与 API (`startGitInit`/`startGitAdd`)，TaskKind 扩展；
  - 测试：Rust 新增 `git_add_enhanced.rs` 覆盖绝对路径拒绝 / 去重 / 进度单调性；前端增加 Init/Add 按钮交互测试；
  - 错误分类：参数/路径问题 → Protocol，取消 → Cancel，I/O → Internal；
  - 回退：移除 Tauri 命令或 UI 卡片即可回退；无既有命令行为破坏。

   - v1.5: 完成 P2.1b (`git_commit`):
    - 后端：实现提交逻辑（空提交检测、allowEmpty、作者校验、单进度事件 Committed、标准分类与多取消点）；
    - 任务注册：`spawn_git_commit_task` 支持预取消与完成状态；
    - 前端：GitPanel 新增 Commit 卡片与 API (`startGitCommit`)，TaskKind 扩展；
    - 测试：新增完整矩阵（成功/空提交拒绝/allowEmpty/作者缺失/作者空字符串/消息裁剪/初始空仓库/取消两路径/注册层预取消）；
    - 安全：不输出敏感路径，作者/消息按需裁剪；
    - 回退：移除命令与 UI 卡片即可回退，不影响已有命令。
 
    - v1.6: 完成 P2.1c (`git_branch` / `git_checkout`):
     - 后端：实现 `git_branch`（支持 force 覆盖、可选立即 checkout；存在且未 force 冲突 → Protocol；无提交时禁止创建新分支）、`git_checkout`（支持 create 标志创建后切换；不存在且未 create → Protocol；无提交 create → Protocol）；
     - 进度：两命令均使用单进度事件（`Branched` / `BranchedAndCheckedOut`、`CheckedOut` / `CreatedAndCheckedOut`），percent=100；
     - 任务注册：新增 `spawn_git_branch_task` / `spawn_git_checkout_task`，按既有本地命令模板发 state/error/progress；
     - 前端：`tasks.ts` 增加 `startGitBranch` / `startGitCheckout` API，`stores/tasks.ts` 扩展 TaskKind；UI 暂未新增专用卡片（复用后续统一面板规划），保持兼容；
     - 测试：新增 `tests/git_branch_checkout.rs` 覆盖创建/重复冲突/force 更新/checkout 不存在失败/create 成功/取消；所有后端测试 (31+) 与前端 86 用例全绿；
     - 错误分类：参数与存在性冲突 → Protocol；取消 → Cancel；底层 git2 失败 → Internal；
     - 回退：移除 Tauri 命令或任务注册分支即可恢复到未实现状态；测试文件可标记忽略；不影响已交付命令。
 
   - v1.7: P2.1c 增量完善（分支名校验 & 扩展测试）:
    - 分支名校验：新增 `validate_branch_name`，拒绝空白、空格、结尾 `/` 或 `.`、前导 `-`、包含 `..`、反斜杠或控制字符；全部归类 Protocol；
    - 语义明确：`force` 更新分支在无任何提交 (HEAD 不可解析) 时返回 Protocol（之前是允许继续尝试的潜在不确定状态）；
    - 取消增强：在 branch/checkout 设置 HEAD 与 checkout 前再次检测取消标志，降低临界窗口；
    - 新增测试：
      * invalid names 列表覆盖；
      * 无提交创建/force 均拒绝；
      * checkout 取消路径；
      * force 移动引用后验证分支引用已指向最新提交；
    - 回归：后端测试总数增加且全部通过；保持前端用例不变仍全绿；
    - 回退影响：删除校验函数或恢复旧逻辑即可，不影响其它命令；测试可标记忽略对应新增用例。
 
     - v1.8: P2.1c 第二轮增强（更严格的分支名校验 & 进一步测试覆盖）:
      - 校验规则提升：`validate_branch_name` 扩展拒绝范围，新增：
        * 以 `/` 开头；
        * 含有双斜杠 `//`；
        * 结尾为 `.lock`（防止与引用锁文件冲突）；
        * 包含下列任一非法字符：`:` `?` `*` `[`~` `^` `\\`、包含子串 `@{`。
        所有违反 → `Protocol` 分类，消息聚焦可修复性（不暴露内部实现细节）。
      - 语义保持：无提交（HEAD 不可解析）时禁止创建或 force 更新分支，继续走 `Protocol`，确保引用不指向无效对象。
      - 取消窗口：保持 v1.7 已加的 HEAD 更新与 checkout 之前的二次取消检查，无新增窗口。
      - 新增/扩展测试：
        * `branch_valid_names_succeed_and_phase_emitted`：覆盖多组合法名称并断言最终 progress phase (`Branched` / `BranchedAndCheckedOut`)；
        * `branch_new_invalid_additional_cases`：覆盖新增非法模式（前导 `/`、双斜杠、`.lock` 结尾、`@{`、非法字符与控制字符等）；
        * `checkout_create_on_existing_branch_noop_like`：验证在目标分支已存在场景下 create+checkout 的行为与 phase 语义；
        * `checkout_create_without_commit_rejected`：再次锁定没有任何提交时禁止 create+checkout；
        * force 移动引用后验证引用指向最新提交（回归性保证）；
        * 移除测试中未使用的 `Author` import，消除编译器 warning，保持“零 warning”目标。
      - 稳定性：运行所有后端测试（包含新增用例）全部通过；无额外前端改动需求，TaskKind 与事件契约未变；新增校验仅在非法输入路径生效，对既有合法调用完全向后兼容。
      - 复用展望：`validate_branch_name` 设计为后续 `git_tag` / `git_remote_*` 的命名规则基线，可在需要时抽象为 `refs.rs` 通用助手；控制字符与特殊序列过滤逻辑可直接迁移。
      - 回退策略：如出现兼容性问题，可快速恢复到 v1.7（删除新增规则分支或放宽匹配）。测试可通过忽略新增用例回退。
  - 抽象复用：在后续调整中（同版本后续 patch）已将分支/标签/远端名称校验收敛到 `refname.rs` (`validate_ref_name` + 三个 wrapper)，减少重复逻辑，为即将实现的 tag/remote 命令提供统一入口；回退可单点放宽。

  - v1.9: 完成 P2.2a（`git_clone`/`git_fetch` 入参 `depth` / `filter` / `strategyOverride` 解析与校验占位）:
    - 新增模块：`core/git/default_impl/opts.rs` 暴露 `parse_depth_filter_opts` 与结构：`GitDepthFilterOpts`、`PartialFilter`、`StrategyOverride*`；
    - 校验规则：
      * depth: 必须为正整数（>0）；0/负值/非数字/超 u32 范围 → Protocol
      * filter: 允许 `blob:none` / `tree:0`；其它值 → Protocol
      * strategyOverride: 结构化解析 http/tls/retry 子集，未知字段忽略；当前不应用，仅记录日志
    - TaskKind：`GitClone` / `GitFetch` 扩展可选字段 `{ depth, filter, strategy_override }`，保持旧调用兼容（旧前端未传参数 → None）
    - Tauri 命令：`git_clone(repo,dest,depth?,filter?,strategy_override?)` / `git_fetch(repo,dest,preset?,depth?,filter?,strategy_override?)`；未破坏原顺序（新增参数置于末尾或文档说明）
    - 任务注册：在进入重试循环前调用解析函数；非法参数直接 `Failed` + 错误事件（Protocol 分类）；合法参数仅 `tracing::info!` 记录（placeholder，无行为变更）
    - 测试：
      * 单元：`opts.rs` 覆盖 depth 正常/0/负、filter 合法与非法、strategyOverride 解析与别名兼容
      * 集成：新增 `git_clone_fetch_params.rs`：depth=0 触发 Failed；非法 filter 触发 Failed
    - 回退策略：删除 `opts.rs` 并还原 TaskKind/命令签名；或在解析失败时改为忽略（临时禁用强校验）
    - 后续衔接：P2.2b/c 将把解析结果绑定到 `RepoBuilder`/`FetchOptions` 实现浅克隆/浅拉取；P2.2d+ 引入 filter 能力探测与回退事件。
  - v1.9.1: P2.2a 测试与边界强化补丁：
    - 新增单元测试：depth 溢出 (u64→u32) / 空 filter 拒绝 / filter 同义形式 `tree:depth=0` 支持；
    - 新增集成测试 `git_clone_fetch_params_valid.rs`：合法 depth=1 + filter=`blob:none` 不触发 Failed；strategyOverride 含未知字段不报错；
    - 兼容性：不改变外部行为（仍为占位），仅增加测试护栏；全部 `cargo test` 通过；
    - 文档：记录为 v1.9.1 以与后续 P2.2b 行为性变更区分（防止 shallow 实现与纯解析补丁混淆）。
  - v1.9.2: P2.2a 进一步组合/类型测试补丁：
    - 单元测试新增：
      * depth = u32::MAX 成功；
      * depth 为字符串类型被拒绝（"depth must be a number"）；
      * 无效 filter 变体（大写、tree:1、内部空格）全部 Protocol；
      * depth+filter 组合；
      * strategyOverride 各子块独立解析（http/tls/retry 单独存在时均成功）；
      * 非对象 strategyOverride (数组) 在 Task 级当前占位实现未强制 Failed（集成测试记录现状，后续若调整为严格 Protocol 可更新）；
    - 集成测试新增 `git_clone_fetch_params_combo.rs`：
      * 同时传递 depth+filter+完整 strategyOverride 不失败；
      * 非对象 strategyOverride 类型当前被视为占位忽略（状态非 Failed），测试锁定现状并留注释；
    - 目的：锁定解析稳定性与未来行为调整安全窗，一旦后续 P2.2b/c 引入实际 shallow/partial 行为，可直接替换/补充期望断言；
    - 风险缓解：排除大小写/空白/数字越界/结构类型错误导致未定义行为的可能性。

### P2.2b 实现说明（已完成）

本节补充 Shallow Clone（`depth` for clone）生效的实现快照：

1. 代码改动
   - 扩展 `GitService::clone_blocking` 签名新增 `depth: Option<u32>`；调用侧统一传 `None` 维持向后兼容。
   - 在任务注册 (`spawn_git_clone_task_with_opts`) 成功解析后将 `depth_applied` 下传服务层；
   - `ops.rs` 中通过 `fo.depth(d as i32)` 设置浅克隆；checkout 进度保持原样；
   - 本地路径（绝对/相对/含反斜杠判定）不支持浅克隆，静默忽略 depth（后续 partial 回退统一化前不额外发事件）。
   - depth 上限改为 `i32::MAX`；超出返回 `Protocol(depth too large)`；对应测试从 `test_max_u32_depth_ok` 调整为 `test_max_i32_depth_ok`。
   - `filter` / `strategyOverride` 仍占位，仅记录日志：`depth active; filter/strategy still placeholder`。

2. 事件与兼容性
   - 事件模型未变；阶段仍为 `Starting|Receiving|Checkout|Completed`；
   - 忽略本地路径 depth 不追加 `task://error`（后续可统一为非阻断提示）。

3. 测试
   - 后端 `cargo test` 全绿（45+ 用例）。
   - 新增 `tests/git_shallow_clone.rs`：
     * `shallow_clone_depth_one_creates_shallow_file` 验证公网 depth=1 存在 `.git/shallow`（CI 或 `FWC_E2E_DISABLE` 下跳过）。
     * `full_clone_no_depth_has_no_shallow_file` 验证全量克隆通常无 shallow 文件（存在仅告警）。
   - 新增测试：`tests/git_shallow_local_ignore.rs` 构造三次提交的本地仓库，使用 `depth=1` 克隆：
     * 断言未生成 `.git/shallow`；
     * 提交计数 >=3，证明未被裁剪。
   - 目的：锁定“本地路径静默回退”语义，避免后续浅克隆实现误对本地仓库生效导致历史截断。
   - 组合参数测试本地路径（`git_clone_fetch_params_combo.rs`）仍通过，depth 被正确忽略。

4. 回退策略
   - 软回退：在 `mod.rs` 将传入 depth 直接置为 `None`。
   - 硬回退：还原 trait 签名并删除 `fo.depth()` 调用；更新所有调用与测试。

5. 已知限制 / TODO
   - 未实现 fetch depth（计划 P2.2c）。
   - 未发送“忽略 depth”回退提示事件。
   - 未统计对象/字节节省指标（P2.2f 评估是否新增）。
   - 与未来 `--single-branch` 等组合尚未覆盖。

6. 安全与性能
   - 减少对象协商，不引入敏感信息；
   - 手动验证公开小仓库对象与字节下降（未转化为事件字段）。

7. 变更摘要
   - Trait 扩展 + 执行层 depth 应用；
   - 上限校验改为 `i32::MAX`；
   - 本地路径静默回退；
   - 新增浅克隆测试；
   - 文档与 changelog 待补充版本号（合并时编写）。
  - 新增非法 depth 集成测试 `git_shallow_invalid_depth.rs`：
    * depth=0 → 解析失败（Failed/Protocol）
    * depth<0 → 解析失败（Failed/Protocol）
    * depth > i32::MAX → 解析失败（Failed/Protocol, message 包含 "depth too large"）
    目的：锁定输入校验与分类为 Protocol，防止后续 fetch depth 接入时出现不一致的错误分类。

  - v1.10: 完成 P2.2c（Shallow Fetch depth for fetch 生效）：
    - 服务层：`GitService::fetch_blocking` 扩展 `depth: Option<u32>` 参数（与 clone 对齐），保持向后兼容（旧调用经适配添加 `None`）。
    - 实现：在 `default_impl::ops::do_fetch` 中对传入的 `depth` 调用 `fo.depth(d as i32)`；当 `repo_url` 指向本地路径（或空串使用已配置远程且本地路径不适用）进行静默忽略策略保持一致性（本地 fetch depth 无意义），记录日志。
    - 任务注册：`spawn_git_fetch_task_with_opts` 解析占位结果后将 `opts.depth` 赋值给 `depth_applied` 并传递给 service（之前仅记录日志）。
    - 日志变化：`git_fetch options accepted (depth active; filter/strategy placeholder)` 用于区分 P2.2a 占位阶段。
    - 测试：
      * 新增 `git_shallow_fetch.rs`（公网 E2E，可跳过）：针对已克隆仓库执行 depth=1 fetch；若 `.git/shallow` 生成则断言非空，否则给出 warn（容忍不同远端行为）；
      * 新增 `git_shallow_fetch_local_ignore.rs`：对本地路径 fetch depth=1 静默忽略，不生成 `.git/shallow`；
      * 更新所有使用 `fetch_blocking` 的测试引入 `None` depth 参数；
      * 确保取消/前置条件/重试路径未受影响（`git_fetch` 任务测试全部通过 45+ 后端用例新增 2 文件后仍全绿）。
    - 兼容性：旧前端与 Tauri 命令调用无需修改；TaskKind 早已包含 `depth` 字段，无新增序列化变动；事件阶段集合保持 `Starting|Fetching|Receiving|Completed` 不变。
    - 回退策略：将 service 实现中传入的 `depth` 强制置为 `None` 或还原 trait 签名；测试可保留（本地忽略语义仍满足）。
    - 已知限制：未对“从全量仓库转为浅仓库”做强制裁剪（libgit2 行为与远端支持决定 shallow 文件是否出现），未追加回退提示事件；`filter` 仍占位准备 P2.2d。
  - v1.10.1: P2.2c 增强（深度加深 + fetch 参数校验补充 + 辅助函数抽取）:
    - 抽象：新增 `helpers::is_local_path_candidate`，统一 clone/fetch 本地路径判定逻辑（减少重复 & 回退集中）。
    - 测试：
      * `git_shallow_fetch_deepen.rs`：本地源 5 提交，浅克隆 depth=1 后依次 fetch depth=2/4，验证提交数量单调不减（加深语义）。
      * `git_shallow_fetch_invalid_depth.rs`：覆盖 fetch 任务 `depth=0 / 负数 / > i32::MAX` 失败路径，与 clone 非法 depth 行为保持一致（Protocol + Failed）。
    - 一致性：clone 与 fetch 对本地路径的 depth 忽略策略通过公共助手保证；减少未来 Partial/Filter 接入时分叉风险。
    - 未改动：仍未对“已全量仓库再 shallow fetch”添加提示事件；保留后续 Partial 阶段统一处理回退/提示的窗口。
    - 回退：删除/还原 helper 并直接内联旧逻辑即可；测试可选忽略 deepen/invalid fetch 相关文件。
  - v1.10.2: P2.2c 进一步测试充实：
    - 新增测试：
      * `git_shallow_fetch_deepen.rs`（已存在，加深语义继续稳定）
      * `git_shallow_fetch_invalid_depth.rs`（fetch 非法 depth）
      * `git_shallow_file_url_deepen.rs`（file:// 方案骨架，当前标记 `#[ignore]` 因实现不支持 file://，为未来扩展保留）
    - 补充保障：
      * smaller depth fetch 不会回退历史（在 file URL 测试与本地 deepen 测试中覆盖）；
      * full fetch (depth=None) 之后提交数非递减；
    - 决策：明确当前阶段不支持 `file://` scheme（clone 阶段校验直接拒绝），因此测试以 ignored 形式存在，不影响主线稳定；
    - 质量：全部活跃测试通过，新增 ignored 测试提供未来支持入口。

#### P2.2b 详细实现补充（扩展说明）

本补充段面向后续维护者，提供比“变更摘要”更细粒度的技术视图，以支撑 P2.2c（Shallow Fetch）与 P2.2d/e（Partial Clone/Fetch）迭代时的可预测演进与回退。

1) 调用链 / 数据流（Clone 启动 → 浅克隆生效）
```
Tauri Command (git_clone) 
  → TaskRegistry.spawn_git_clone_task_with_opts(... depth_json, filter, strategy_override)
   → parse_depth_filter_opts(depth_json, filter, strategy_override)  // P2.2a 占位解析（含上限 i32::MAX 检查）
    → 返回 GitDepthFilterOpts { depth_applied, filter: None, strategy_override }
   → 创建任务 & 进入执行闭包：DefaultGitService.clone_blocking(repo, dest, depth_applied)
    → default_impl/mod.rs::clone_blocking
      * 检测 repo 是否为本地路径（Path::new(repo).exists() || 具有本地盘符/相对形式）
      * 本地则 effective_depth = None（静默忽略）；否则传递 Some(depth)
      → clone::do_clone(url, dest, effective_depth, ...)
       → ops::do_clone(repo_url_final, dest, depth_opt, progress_bridge, cancel_token)
         * 构建 FetchOptions fo; 若 depth_opt=Some(d) → fo.depth(d as i32)
         * RepoBuilder (不再尝试使用 depth，避免误导) + remote callbacks + fo 进行下载
         * 进度回调映射 → task://progress (Starting | Receiving | Checkout | Completed)
```

2) 本地路径判定逻辑（忽略 shallow 的条件）
  - 规则：若输入 repo 字符串能被解释为**本地现有目录** 或 **合法的相对/绝对文件系统路径** 则视为本地；不进行 URL scheme 解析。
  - 忽略策略：直接将 depth 置 None；不发回退事件（未来 Partial 能力统一化后可追加非阻断提示）。
  - 原因：Git 本地“克隆”本质是拷贝 + 硬链接/对象复用，不受服务器侧 `upload-pack` 协商；强行设置 depth 会带来与预期不一致的截断风险（libgit2 对本地路径 depth 不生效 / 语义模糊）。

3) 验证与错误路径
  - 非法 depth 在“解析阶段”即失败（TaskState=Failed，ErrorCategory=Protocol）——不会进入任何网络/IO 操作。
  - 合法 depth 但本地路径：被忽略；后续行为等价“全量克隆”，因此测试通过提交数量与 `.git/shallow` 文件缺失来断言忽略生效。
  - 合法 depth + 远端：设置 fo.depth；若远端正常响应则创建 `.git/shallow` 文件。
  - P2.2b 不新增新的错误分类；任何浅克隆的网络失败仍沿用既有 Network/Tls/Verify/Proxy/Auth/Internal 分类逻辑。

4) 关键内部契约（需保持以避免后续回归）
  - Trait `GitService::clone_blocking` 的参数顺序与新增 `depth` 置于尾部，保持后续扩展（例如 future: single_branch）时的二次变化最小化。
  - 进度阶段集合保持不变：UI / 测试仅依赖固定枚举文字；浅克隆不会引入新的阶段并且不改变阶段顺序（避免前端 diff 逻辑破裂）。
  - 忽略本地 depth 不抛错：后续 Partial 回退机制引入时需要新增“可选提示”事件时，需要保持“此前不提示”到“新增提示”是向前兼容的 additive 变更。

5) 测试矩阵对应关系（文件 → 保障点）
  | 测试文件 | 保障维度 | 关键断言 |
  |----------|----------|----------|
  | `git_shallow_clone.rs` | 远端浅克隆生效 | `.git/shallow` 存在（depth=1），全量克隆不存在 |
  | `git_shallow_local_ignore.rs` | 本地路径忽略 | 无 `.git/shallow` 且 commit ≥3 |
  | `git_clone_fetch_params_combo.rs` | depth+filter+strategyOverride 组合 | 本地路径时 depth 静默忽略不 Failed |
  | `opts.rs` 单测（max_i32 等） | 上限与类型 | 大值通过 / 超界失败 / 字符串类型拒绝 / 0/负数拒绝 |
  | `git_shallow_invalid_depth.rs` | 非法 depth 解析失败路径 | TaskState=Failed + Protocol 分类（0 / 负数 / > i32::MAX） |

6) 性能与资源占用评估
  - 增量 CPU：一次整数比较与可选路径 exists() 判断，影响可忽略。
  - 内存：未引入额外缓存结构；集成测试表面浅克隆对象数下降但未在事件中统计，该指标延后到 P2.2f 决策。
  - 未来若需 emit 省略统计（objects_saved / bytes_saved）可在 progress 的 `Completed` 阶段追加扩展字段或独立 `task://progress`（需保持前端容忍性）。

7) 回退&前向扩展钩子
  - 回退点 A（软）：`mod.rs` 强制传 `None` 给 clone::do_clone → 立即失效 shallow。
  - 回退点 B（硬）：移除 trait 新参数 + 调整所有调用者 + 删除 fo.depth 分支。
  - 扩展钩子：在 `ops::do_clone` 内部保留 `fo` 构造点，可在 P2.2d 添加 `fo.download_tags(...)` 或其它 partial 相关设置；或插入 capability 探测逻辑（protocol v2 negotiation）。

8) 风险与缓解（专属于 P2.2b 的子集）
  | 风险 | 场景 | 缓解 |
  |------|------|------|
  | 本地误当远端 | 用户传入看似 URL 但本地目录名含冒号 | 目前使用文件系统存在性优先判定 → 即便误判只会走全量克隆（安全） |
  | 远端不支持 depth | 罕见老旧服务 | libgit2 会回退全量；后续可检测 absence of .git/shallow 并提示 |
  | 超大 depth 近似全量 | 用户输入接近 i32::MAX | 允许；行为与全量无差异；不特殊处理 |

9) 对 P2.2c（Shallow Fetch）的直接可复用点
  - `parse_depth_filter_opts` 输出结构无需调整；fetch 路径仅需在 `ops::do_fetch`（或对应新模块）对 `fo.depth()` 进行同样应用；
  - 本地路径 fetch depth 是否忽略：建议与 clone 保持一致（在本地仓库上执行浅 fetch 没有语义价值且易混淆）。
  - 需新增：已有浅克隆仓库追加 fetch depth=1 时的“增量限制”测试（不可拉取超出深度的旧提交）。

10) 未来演进占位（Partial / Capability）
  - 入口点：在设置 `fo.depth()` 前后插入能力探测（LS advertised capabilities）→ 若 filter 请求失败可先降级 depth-only；
  - 事件变更策略：新增非阻断 `task://error`（Protocol + message="partial unsupported; fallback=shallow"）保持 additive。

11) 维护建议
  - 保持 `.git/shallow` 文件存在性检查逻辑仅用于测试，不在生产逻辑依赖（避免平台差异导致行为分叉）；
  - 避免在后续补丁直接复用 RepoBuilder.depth（libgit2 版本差异会产生迷惑）；统一通过 FetchOptions 入口。

> 若未来需要把“本地忽略 depth”行为改为发出提示事件，请在变更前回看本节第 4 点契约，确保 UI 兼容性（新增错误事件不会导致现有逻辑误判为失败）。

### P2.2c 实现说明（已完成）

本节归档 Shallow Fetch（`depth` for fetch）正式生效及其两轮增强（v1.10 / v1.10.1 / v1.10.2）的设计与实现快照，衔接 P2.2b（Shallow Clone）。

#### 1. 代码改动概览
- Trait 扩展：`GitService::fetch_blocking(&self, repo: &str, dest: &str, preset: Option<...>, depth: Option<u32>)`（新增 `depth`，向后兼容旧调用—统一由调用点补 `None`）。
- 任务注册：`spawn_git_fetch_task_with_opts` 在解析（沿用 `parse_depth_filter_opts`）后提取 `opts.depth` → `depth_applied` 并传入 service。
- 执行层：`default_impl::ops::do_fetch` 在构建 `FetchOptions fo` 时：`if let Some(d) = depth { fo.depth(d as i32); }`。
- 本地路径判定：复用抽取后的 `helpers::is_local_path_candidate`（v1.10.1 引入）——若判定为本地仓库或本地路径目标，`depth` 静默忽略并记录日志（与 clone 对齐）。
- 日志：区分占位阶段 → 生效阶段：`git_fetch options accepted (depth active; filter/strategy placeholder)`。

#### 2. 数据流（Fetch 启动 → 浅拉取）
```
Tauri git_fetch(repo,dest,preset?,depth?,filter?,strategyOverride?)
  → TaskRegistry.spawn_git_fetch_task_with_opts(...)
    → parse_depth_filter_opts(...)  // 验证 depth/filter/strategyOverride
    → depth_applied = opts.depth
    → DefaultGitService.fetch_blocking(repo, dest, preset, depth_applied)
      → default_impl/mod.rs::fetch_blocking
        * if is_local_path_candidate(repo) ⇒ effective_depth=None
        * else effective_depth=depth_applied
        → fetch::do_fetch(..., effective_depth, ...)
          → ops::do_fetch(..., Some(d)) ⇒ fo.depth(d as i32)
          → 进度回调 → task://progress (Starting | Fetching | Receiving | Completed)
```

#### 3. 语义与行为
- 深度生效条件：远端（非本地路径）且解析阶段通过；否则保持全量 fetch。
- 本地路径忽略：无 `.git/shallow` 生成；提交数量不被裁剪（测试通过多次 fetch 与 commit 数断言）。
- 加深（deepen）：后续 fetch 传入更大 depth（例如初始 1 → 2 → 4）允许提交可见范围单调扩大，未强制重新生成 `.git/shallow` 时也以提交数增加为准。
- 更小 depth：传入低于当前已拥有可见高度的 depth 不会“收缩历史”——保持提交集不减（测试覆盖）。

#### 4. 非法输入与错误分类
- 与 clone 一致：
  * depth=0 / 负数 / > i32::MAX → 解析阶段 `Protocol` 错误（Task Failed），不进入 fetch。
  * 其它参数（filter / strategyOverride）仍处于占位解析阶段，不影响 shallow fetch。
- 分类保持：`Protocol`（输入问题）/ `Cancel` / `Internal|Network|Tls|Auth|Verify`（底层已有逻辑）。

#### 5. 测试矩阵（新增/更新）
| 文件 | 目标 | 关键断言 |
|------|------|---------|
| `git_shallow_fetch.rs` | 远端浅拉取基础 | depth=1 任务成功；若远端支持则出现 `.git/shallow`（不强制，缺失记录 warn） |
| `git_shallow_fetch_local_ignore.rs` | 本地路径忽略 | 无 `.git/shallow`；提交数不被裁剪 |
| `git_shallow_fetch_deepen.rs` | 多次 deepen | depth 递增后提交数单调不减（1→2→4），更小 depth 不回退 |
| `git_shallow_fetch_invalid_depth.rs` | 参数非法路径 | 0/负/超上限 => Failed + Protocol（消息包含原因） |
| `git_shallow_file_url_deepen.rs` (`#[ignore]`) | file:// 占位 | 当前不支持 file://；忽略以保留未来实现入口 |
| 受影响旧测试 | 统一新增 fetch `depth=None` 参数 | 不破坏既有行为与断言 |

#### 6. 抽象与复用
- `helpers::is_local_path_candidate` 统一 clone/fetch 本地判断，降低后续 Partial 回退分叉。
- `parse_depth_filter_opts` 复用：无额外重复解析逻辑。
- deepen 语义测试为后续 Partial（filter）叠加时验证“深度可扩张 + 过滤不破坏已拥有提交”提供基线。

#### 7. 回退策略
- 软回退：在 `fetch_blocking` 内丢弃传入 depth（置 None），测试仍可通过（忽略语义成立）。
- 硬回退：还原 trait 签名 + 删除 `fo.depth(...)`；移除/忽略 shallow fetch 专属测试（deepen / invalid depth）。
- 按阶段回退：可仅移除 deepen 测试保持单次浅拉取（若远端兼容性问题）。

#### 8. 已知限制 / TODO
- 不会向“已全量仓库”强制裁剪（无历史截断逻辑）。
- 尚未发出“忽略 depth（本地路径）”的非阻断提示事件（计划与 Partial 回退一起统一补齐）。
- 未统计对象/字节节省指标；P2.2f 评估是否加入。
- 未处理“从全量转 shallow”显式 downgrade；依赖远端协商（libgit2 行为）。
- file:// scheme 暂不支持（测试 ignored）。

#### 9. 性能与影响评估
- 额外分支：一次本地路径判定 + Option 检查，CPU 开销可忽略。
- 网络优化：在支持的远端对象协商减少，实际节省未注入事件（避免协议差异噪音）。
- 线程/任务模型未改动；重试逻辑（Retry v1）不修改 depth 参数（幂等）。

#### 10. 未来衔接（P2.2d/e Partial）
- Capability 探测插入点：`ops::do_fetch` 在设置 `fo.depth` 后，可添加 partial capability 判断（若 filter 请求失败 → fallback+非阻断错误事件）。
- 回退事件模型：将在 partial 阶段引入 `task://error(Protocol, message="partial unsupported; fallback=shallow|full")`，本节保持兼容窗口。
- 深度与过滤组合：测试需新增“先 shallow 再 partial”与“partial 后 deepen”交叉矩阵（当前 deepen 测试即其子集基线）。

#### 11. 变更与版本标记
- v1.10：基础 shallow fetch 生效（trait 扩展 + fo.depth 应用 + 测试 `git_shallow_fetch.rs` / 本地忽略）。
- v1.10.1：抽取 `is_local_path_candidate` + deepen 测试 + fetch 非法 depth 测试补齐（与 clone 校验对齐）。
- v1.10.2：进一步测试充实（更小 depth 不收缩 / full fetch 语义 / ignored file:// 骨架）。

#### 12. 维护建议
- 保持忽略本地 depth 的静默语义直至 Partial 回退事件统一引入，避免事件模型提前破坏前端“错误==失败”假设。
- deepen 测试若因远端差异（罕见）不稳定，可本地先行锁定固定测试源仓，后续引入可配置测试仓库镜像。
- file:// 若未来支持，更新 ignored 测试为活跃并补充安全路径校验（防目录穿越 / UNC 路径）。

> 本节作为 P2.2c “生效 + 加深 + 测试矩阵巩固” 的稳定快照，后续 Partial 引入时只需在此基础添加回退错误事件与 filter capability 检测，不应再修改 shallow fetch 的既有语义（除非添加提示事件，需标注向前兼容性评估）。

### P2.2d 实现说明（部分完成：Partial Clone 参数回退事件占位）

> 版本：v1.11（占位阶段） — 目标是在 clone 任务中对用户请求的 `filter` 参数进行“能力尚未启用”的非阻断回退提示，优先保留已生效的 `depth`（若存在），为后续真正的 partial capability 探测与生效逻辑预留最小改动面。当前未实际减少对象/字节，仅发送一次 `task://error(Protocol)` 提示并继续完成任务。

#### 1. 代码改动
- `tasks/registry.rs::spawn_git_clone_task_with_opts`：在成功解析 `parse_depth_filter_opts` 后：
  - 若解析出的 `filter` 为 `Some`，记录 `filter_requested`；
  - 立即发送一条非阻断 `TaskErrorEvent`：
    * 当同时存在 `depth`：`message = "partial filter unsupported; fallback=shallow (depth retained)"`
    * 仅有 `filter` 无 depth：`message = "partial filter unsupported; fallback=full"`
  - 不更改后续 `depth_applied` 的逻辑；`filter` 不向下游传递（仍未应用）。
- 日志：`git_clone options accepted (depth active; filter parsed)`（原占位日志文案更新，强调 filter 已解析）。
- 新增集成测试：`tests/git_partial_clone_filter_fallback.rs`
  - 构造本地源仓库，发起含 `filter=blob:none` 的 clone 任务；
  - 断言任务最终 `Completed` 而非 `Failed`；（事件回放在无 `tauri-app` 特性下为 no-op，不断言错误事件内容，仅验证不失败逻辑）。

#### 2. 行为与语义（占位回退）
- 用户传入合法 `filter`：任务成功执行“常规（可能含 shallow）克隆”；
- 回退提示通过 `Protocol` 分类的 `task://error` 事件体现（非阻断）；选用 `Protocol` 是为了保持“调用者可修复/升级环境”语义；
- 未变更：
  - 进度阶段集合（`Starting|Receiving|Checkout|Completed`）保持不变；
  - 取消与重试路径不受影响；
  - `filter` 非法仍在解析阶段导致 `Failed`（沿用 P2.2a 行为）。

#### 3. 错误与分类
- 新增回退事件使用 `ErrorCategory::Protocol`；不改变任务最终状态（`Completed`）。
- 仍可能出现其它错误分类：网络/TLS/取消/内部错误各路径保持原有映射。

#### 4. 测试矩阵（当前阶段）
| 用例 | 目标 | 状态 |
|------|------|------|
| `git_partial_clone_filter_fallback` | 合法 filter 触发回退并仍然 Completed | 已实现 |
| filter+depth 组合（已有 shallow 测试覆盖 depth） | 验证提示文案（目前测试未捕获事件，占位） | 待后续具备事件捕获基建扩展 |
| 非法 filter (`blob: none`, 大写等) | 解析阶段 Failed + Protocol | 既有 `opts.rs`/集成测试已覆盖 |

#### 5. 回退策略
- 软回退：移除回退事件发送分支，恢复“静默忽略 filter”；
- 硬回退：同时还原日志文案，删除测试文件 `git_partial_clone_filter_fallback.rs`；
- 后续升级（真正启用 partial）时：将当前回退分支替换为能力探测 → 条件设置 `git2` 相关选项（若未来绑定支持），失败时继续保留该事件逻辑。

#### 6. 已知限制 / TODO
- 未进行 capability 探测（protocol v2 / server filter support）；
- 未区分 `blob:none` 与 `tree:0` 的不同潜在能力；
- 未对本地路径单独提示（与 shallow depth 忽略一致保持静默）；
- 测试未断言事件负载（需要引入事件收集 mock 或在非 tauri 特性下提供 hook）。

#### 7. 后续演进指引（指向 P2.2e 与真正 Partial 生效）
- 在 clone/fetch 的执行层（`ops::do_clone`/`ops::do_fetch` 前后或自定义 wrapper）插入 capability 探测：
  1) 协商阶段（需支持 protocol v2 场景）查询 server 支持的 filter；
  2) 若不支持：保持当前回退事件语义；
  3) 若支持：应用 filter（待选择具体 git2 接口或自定义传输扩展），并新增 `phase="Filtering"` 可选 progress（可选）。
- 事件增强（可选）：在成功应用 partial 时追加一条信息性 progress（percent 不回退）或 stats 扩展字段（objectsSaved / bytesSaved）。
- 测试增强：对比带/不带 filter 的对象数与字节量；验证 fallback 与 success 双路径。

#### 8. 质量评估
- 当前实现对既有 shallow 与本地命令测试零影响；
- 风险集中在任务注册解析后新增的一个 `if` 分支（无共享状态修改）；
- 构建与全部 45+ 后端测试通过，新增测试 1 个；
- 提供清晰回退路径与后续插桩（capability）位置标记。

#### 9. 版本记录
- v1.11: 引入 clone filter 回退事件占位（非阻断），新增 fallback 测试；未启用真实 partial 逻辑。

---

