Registry：新增 `spawn_git_tag_task` / `spawn_git_remote_{add,set,remove}_task`

---

### P2.3b 任务级 HTTP 策略覆盖实现说明（已完成）

本阶段为“任务级策略覆盖”初步落地的 HTTP 子集（followRedirects / maxRedirects）实现，拆分为两个小步：

1. 第一阶段（解析+合并+日志）：完成 `strategyOverride` JSON 的结构解析、值范围校验（`maxRedirects <= 20`）、合并全局配置并记录有效值日志。仅在 clone/fetch/push 任务 spawn 时解析，不改变底层网络行为。
2. 第二阶段（事件+changed flag）：引入 `apply_http_override()` 返回 `changed`；当且仅当覆盖导致实际值变更时发出结构化提示事件（非致命）：`TaskErrorEvent.code = http_strategy_override_applied`。

#### 1. 目标与约束
- 精准、最小：只允许显式列出的安全字段；未列出的字段忽略（有 warn 日志）。
- 幂等：单任务生命周期内事件只发一次；无变化不发。
- 零破坏：前端无需新增监听，复用既有 `task://error` 流（与 partial_filter_fallback 一致）。
- 可回退：删除事件分支即可回退为“仅日志”模式；保留解析与合并逻辑。

#### 2. 合并规则
| 步骤 | 描述 |
|------|------|
| 基线 | 复制 `AppConfig::default().http`（后续接入运行时配置） |
| 覆盖 followRedirects | 若提供且不同 → 替换并标记 `changed=true` |
| 覆盖 maxRedirects | 若提供且不同 → clamp(≤20) 后替换并标记 `changed=true` |
| 事件发射 | `changed=true` 时构造 `TaskErrorEvent`，`category=Protocol`，`code=http_strategy_override_applied`，`message="http override applied: follow=<bool> max=<u8>"` |

#### 3. 代码落点
- 函数：`core/tasks/registry.rs::apply_http_override(kind, id, global_cfg, http_override)` → `(follow, max, changed)`。
- 调用位置：`spawn_git_clone_task_with_opts` / `spawn_git_fetch_task_with_opts` / `spawn_git_push_task` 解析参数后。
- 事件：仅在 `changed` 时 `emit_all(app_ref, EV_ERROR, &TaskErrorEvent { code: Some("http_strategy_override_applied"), .. })`。

#### 4. 事件选择策略
备选方案对比：
| 方案 | 优点 | 缺点 | 决策 |
|------|------|------|------|
| 新增 `task://strategy` | 语义清晰 | 前端需新增通道解析 | 否 |
| 使用 progress | 统一事件流 | 语义割裂（非阶段进度） | 否 |
| 复用 error + code | 复用 UI/Store、与其他“协议提示”一致 | 名称上包含 error 需前端样式区分 | 采用 |

#### 5. 测试矩阵（最终）
| 用例文件 | 任务 | 覆盖点 |
|-----------|------|--------|
| `git_http_override_event.rs` | Clone | follow+max 改变触发事件 |
| `git_http_override_no_event.rs` | Clone | 覆盖值与默认相同抑制 |
| `git_http_override_idempotent.rs` | Clone | 单任务事件一次 |
| `git_http_override_clone_only_follow.rs` | Clone | 仅 follow 改变触发 |
| `git_http_override_invalid_max_no_event.rs` | Clone | 解析失败（>20）无事件 |
| `git_http_override_fetch_event_only_max.rs` | Fetch | 仅 max 改变触发 |
| `git_http_override_push_follow_change.rs` | Push | 仅 follow 改变触发 |
| registry 内部单测 | N/A | clamp / changed 逻辑验证 |

#### 6. 幂等 & 失败路径
- 事件发射点唯一：任务启动解析阶段；后续重试不再重新解析覆盖（设计保持简单）。
- 解析错误（结构非法 / maxRedirects>20）直接 Protocol 失败，不触发 override 事件。

#### 7. 日志示例
```
INFO strategy task_kind=GitClone task_id=... follow_redirects=false max_redirects=3 http override applied
INFO git depth=None filter=None has_strategy=true strategy_http_follow=false strategy_http_max_redirects=3 git_clone options accepted (depth/filter/strategy parsed)
```

#### 8. 回退策略
| 操作 | 效果 |
|------|------|
| 移除 `if changed { emit ... }` | 回到仅日志 & 合并值仍可用于后续阶段 |
| 删除新测试 | 只保留解析行为验证 |
| 保留函数不用 | 易于再次开启（低成本开关） |

#### 9. 已知限制 / 后续
- 未实际影响 HTTP 重定向行为（等待自定义 HTTP 客户端接入）。
- 未接入动态全局配置与热加载；后续需覆盖“全局已非默认”差异测试。
- 未处理 `follow=false` 且 `maxRedirects>0` 的语义提示（可在真正应用阶段补充二级提示）。

#### 10. Changelog 建议
```
Added: per-task HTTP strategy override application (followRedirects/maxRedirects) with informative event `http_strategy_override_applied`.
```

---
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
- 确保取消不会留下半完成状态（要么未创建引用，要么 HEAD 尚未切换）。

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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
- 移除 Tauri 命令导出或在任务注册中屏蔽 `GitTag` / `GitRemoteAdd` / `GitRemoteSet` / `GitRemoteRemove` 分支。
- 删除/忽略测试文件 `git_tag_remote.rs`（或标记 `#[ignore]`）。
- 分支名校验可分层回退：只移除 v1.8 增强保持 v1.7，或完全移除校验函数回到最小限制。

#### 9. 复用指引
- 错误分类与取消模板与 init/add 对齐，保证前端无需新增分支。
- 进度 phase 命名模式（动作过去式 + 可选合并语义）为后续 tag (`Tagged` / `AnnotatedTagged`) 提供模板。
- 取消点布局（副作用前检查）可移植到 tag/remote 修改引用场景。

#### 10. 已知限制 / TODO
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
- set：远程存在 → 更新 URL（同 URL 幺等成功）；phase `RemoteSet`。
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
| Protocol | 非仓库、无提交、名称非法、tag 已存在(非 force)、附注缺消息、URL 含空白/非法 scheme、add

# TECH_DESIGN_P2_PLAN 补充：P2.3b（二阶段）HTTP 策略覆盖事件与 changed flag

> 本补充文件与主文档 `TECH_DESIGN_P2_PLAN.md` 中的 “P2.3b 实现说明（已完成）” 章节配套，聚焦第二阶段新增的事件、幂等与测试强化。若后续将 TLS / Retry 覆盖一并应用，可在此文件继续增补相似章节，主文档保持概要。

## 1. 目标概述
- 在 clone / fetch / push 任务解析 `strategyOverride` 后应用 HTTP 子集（followRedirects, maxRedirects）。
- 新增 changed 判定：仅当有效值与全局不同才算“应用”。
- 通过结构化 TaskErrorEvent（code=`http_strategy_override_applied`）发射一次非致命提示，提升前端可观测性而不引入新事件主题。
- 保持零破坏：未改变底层 HTTP 行为（后续阶段再实际接入网络层）。

## 2. 合并与事件逻辑
```rust
// registry.rs
let (f, m, changed) = apply_http_override("GitClone", &id, &global_cfg, opts.strategy_override.as_ref().and_then(|s| s.http.as_ref()));
if changed {
    if let Some(app_ref)=&app {
        let evt = TaskErrorEvent { task_id:id, kind:"GitClone".into(), category:"Protocol".into(), code:Some("http_strategy_override_applied".into()), message: format!("http override applied: follow={} max={}", f, m), retried_times:None };
        this.emit_error(app_ref,&evt);
    }
}
```

规则回顾：
| 步骤 | 说明 |
|------|------|
| 基线 | 复制 `AppConfig::default().http` 值（后续换成运行时配置） |
| 覆盖 | 若提供 followRedirects / maxRedirects 且不同则替换并标记 changed=true；maxRedirects clamp ≤ 20 |
| 发射 | changed=true 时，仅一次事件（spawn 时） |
| 日志 | tracing target="strategy" 同步 info 行（含 follow/max） |

## 3. 事件结构
```json
{
  "taskId": "<uuid>",
  "kind": "GitClone|GitFetch|GitPush",
  "category": "Protocol",
  "code": "http_strategy_override_applied",
  "message": "http override applied: follow=<bool> max=<u8>",
  "retriedTimes": null
}
```
选择原因：
- 复用错误通道（前端已统一消费）；
- 与 partial_filter_fallback 一致，形成“协议提示”类别；
- 避免新增主题造成前端监听扩散。

## 4. 测试矩阵（新增）
| 文件 | 断言要点 |
|------|----------|
| `git_http_override_event.rs` | 覆盖值改变 → 存在事件（包含 code 与 taskId） |
| `git_http_override_no_event.rs` | 覆盖值与默认相同 → 不存在事件 |
| `git_http_override_idempotent.rs` | 单任务事件次数恰为 1 |
| registry 内部单测 | clamp / changed 判定逻辑 |

实现细节：
- 测试需传入 `Some(AppHandle)`（非 tauri feature 下为空占位 struct）才能捕获事件。
- 使用 `peek_captured_events()` 收集全部事件再过滤 code。

## 5. 幂等与回退
| 场景 | 行为 |
|------|------|
| 重复调用 apply（当前不会发生） | 若未来出现，需在上层调用点防抖；现阶段单次调用保证幂等 |
| 回退需求 | 删除 `if changed { emit ... }` 分支和 3 个测试文件即可；合并逻辑保留 |

## 6. 风险评估
- 只读 → 局部变量；无共享状态写入；
- 事件数量受限（最多 1/任务），前端无性能压力；
- 若未来引入真实 redirect 行为，需确认 follow=false 与 max>0 的组合语义（可能追加提示）。

## 7. 后续扩展建议
1. 注入真实运行时配置（替换默认值）并支持热加载。
2. 扩展 TLS / Retry 应用：沿用 `*_strategy_override_applied` code 规范。
3. 前端 UI 增加“提示”标签区分致命 vs 信息事件。
4. 统一策略事件聚合：在任务详情面板聚合展示一次性策略差异摘要。

## 8. Changelog 建议条目
```
Added: per-task HTTP strategy override application + informative event `http_strategy_override_applied` (emitted once when override changes followRedirects/maxRedirects).
```

## 9. 现状结论
- 覆盖逻辑与事件已落地并通过 3 个集成 + 1 组单元测试；
- 后端、前端全部测试通过；
- 回退与后续拓展路径清晰。

---
