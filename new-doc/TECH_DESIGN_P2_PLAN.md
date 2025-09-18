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
      * force move 后验证分支引用已指向最新提交；
    - 回归：后端测试总数增加且全部通过；保持前端用例不变仍全绿；
    - 回退影响：删除校验函数或恢复旧逻辑即可，不影响其它命令；测试可标记忽略对应新增用例。
 
     - v1.8: P2.1c 第二轮增强（更严格的分支名校验 & 进一步测试覆盖）:
      - 校验规则提升：`validate_branch_name` 扩展拒绝范围，新增：
        * 以 `/` 开头；
        * 含有双斜杠 `//`；
        * 结尾为 `.lock`（防止与引用锁文件冲突）；
        * 包含下列任一非法字符：`:` `?` `*` `[` `~` `^` `\\`；
        * 包含序列 `@{`（与引用语法冲突）；
        * 任意控制字符 (c < 0x20)；
        * 维持既有拒绝：空/空白、空格、结尾 `/` 或 `.`、前导 `-`、包含 `..`、反斜杠、前述字符的超集。
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
