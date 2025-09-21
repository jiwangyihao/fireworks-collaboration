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

### P2.3c 任务级 Retry 策略覆盖实现说明（本次提交）

在 P2.3b 已落地 HTTP 覆盖 (followRedirects / maxRedirects + changed 事件) 的基础上，本阶段实现 `strategyOverride.retry` 的任务级生效：在单个 clone/fetch/push 任务生命周期内使用自定义重试参数，而不影响全局配置或其它并发任务。

#### 1. 目标
- 支持覆盖字段：`max` / `baseMs` / `factor` / `jitter`（均已在 P2.3a 解析层做范围校验）。
- 覆盖只影响当前任务内部计算的重试计划（RetryPlan），不写回全局 config。
- 仅当任意字段实际改变与全局默认值不同时，发送一次提示事件：`code=retry_strategy_override_applied`。
- 行为幂等：同一任务只发一次；值未变化不发事件。

#### 2. 合并规则
| 步骤 | 描述 |
|------|------|
| 基线 | 复制 `AppConfig::default().retry`（后续可换为运行时加载） |
| 覆盖 max | 如果提供且不同 → 替换，标记 changed |
| 覆盖 baseMs | 如果提供且不同 → 替换，标记 changed |
| 覆盖 factor | 如果提供且不同 → 替换，标记 changed |
| 覆盖 jitter | 如果提供且不同 → 替换，标记 changed |
| 事件发射 | `changed=true` 时：`TaskErrorEvent`，`code=retry_strategy_override_applied`，`message="retry override applied: max=<u32> baseMs=<u64> factor=<f64> jitter=<bool>"` |

#### 3. 代码落点
- 函数：`core/tasks/registry.rs::apply_retry_override(global_retry, override_retry)` → `(RetryPlan, changed)`。
- 调用位置：`spawn_git_clone_task_with_opts` / `spawn_git_fetch_task_with_opts` / `spawn_git_push_task` 在解析 `strategyOverride` 后执行（紧邻 HTTP 覆盖逻辑）。
- 重试循环：使用合并后的 `plan` 替换原 `load_retry_plan()` 返回值；Push 仍保持“进入 Upload 后不再自动重试”约束。

#### 4. 事件与幂等
- 事件主题复用 `task://error`，分类 `Protocol`，与 HTTP 覆盖一致以降低前端新增适配成本。
- 每任务仅在合并阶段判定一次；后续重试 attempt 不重复判定。

#### 5. 测试矩阵
| 用例文件 | 场景 |
|-----------|------|
| `git_retry_override_event.rs` | 变更覆盖触发一次事件 + 未变化不触发（合并为单测试串行执行防并发污染） |
| registry 单测 | `retry_override_tests_new` 验证 changed / 不变路径 |
| `git_strategy_override_combo.rs` | clone/fetch/push 六合一：1) http+retry 2) retry-only 3) unchanged 4) invalid(retry.max=0) 5) fetch http+retry 6) push retry-only；校验每任务事件至多一次 |
| `git_retry_override_backoff.rs` | 新增（后续增强）：(a) override 事件在不可重试 Internal 错误下仍一次性出现；(b) factor 上下界 (0.5 / 10.0) 覆盖并在事件 message 中反映 |

#### 6. 失败与范围校验
- 数值范围（`max 1..=20`、`baseMs 10..60000`、`factor 0.5..=10.0`）沿用解析层校验；解析失败仍直接 `Protocol` 失败，不进入合并/事件逻辑（与 P2.3b 对齐）。

#### 7. 日志示例
```
INFO strategy task_kind=GitClone task_id=... retry override applied max=3 base_ms=500 factor=2 jitter=false
INFO strategy retry_max=3 retry_base_ms=500 retry_factor=2.0 retry_jitter=false retry override applied (内部合并函数级别)
```
事件：
```
{ "code": "retry_strategy_override_applied", "message": "retry override applied: max=3 baseMs=500 factor=2 jitter=false" }
```

#### 8. 回退策略
| 操作 | 效果 |
|------|------|
| 移除 `apply_retry_override` 调用与事件分支 | 回退为仅使用全局重试计划，不影响 HTTP 覆盖 | 
| 删除测试文件 `git_retry_override_event.rs` | 清除新增覆盖验证，仅保留 HTTP 覆盖 | 

#### 9. 已知限制 / 后续
- 仍未引入动态运行时配置加载（TODO P2.3e）；当前使用 default() 可能与真实用户配置不一致。
- TLS 覆盖尚未应用（计划在后续阶段与自定义传输初始化点统一处理）。
- 没有对“覆盖值降低导致已开始的 backoff 调整”做二次适配（首次加载即定）。

#### 10. Changelog 建议（追加）
```
Added: per-task Retry strategy override application (max/baseMs/factor/jitter) with informative event `retry_strategy_override_applied`.
```

#### 11. 后续增强 / 增补说明（本节为新增）

已在后续补丁中追加的改进与发现：

1) 新测试 `git_retry_override_backoff.rs`：
  - 初版目标是验证实际“Retrying (attempt X of Y)” 进度行，但由于本地 Windows + libgit2 返回的连接失败信息为本地化中文（例如“无法与服务器建立连接”）未命中 `helpers::map_git2_error` 中针对英文关键词 ("connection"/"connect"/timeout) 的 Network 分类分支，被归类为 `Internal` → `is_retryable=false`，导致不进入重试循环。
  - 测试策略调整：改为验证 override 事件出现且未产生重试进度行（符合当前分类逻辑），保证测试稳定性而不依赖具体错误文案。

2) Factor 边界覆盖：在同一测试文件中并行两个 clone 任务（factor=0.5 与 10.0），通过事件 payload 中的 `factor=<value>` 断言上下界值透传无损（打印时 `10.0` 可能序列化为 `10`，测试以包含 `factor=10` 判定）。

3) 事件幂等再确认：所有新增测试保持只读取一次事件缓冲（或使用 peek 不消费）以避免之前出现的“多文件并发读取导致顺序不确定”问题；继续遵循“单任务 override 事件最多一次”约束。

4) 与 P2.1d tag 修复的交互：Annotated tag force 复用 OID 的修复不影响 retry 覆盖逻辑；combo 测试与 backoff 测试均在修复后全量回归通过。

5) 已知限制（Retry 覆盖特有）：
  - 本地化错误文本未被 `map_git2_error` 捕获 → 某些真实的网络错误被归类为 Internal，从而不触发重试进度行；当前仅影响“重试进度可观察性”而不影响 override 事件。
  - Push 任务仍保持“进入 Upload 阶段后不再自动重试” 的语义（计划保持）。
  - 尚未实现“并发多个任务不同 Retry 覆盖的隔离测试”——逻辑已保证（每任务各自 clone 的 override plan），但测试待补。
  - 未覆盖 jitter=true 的统计区间验证（已有 retry.rs 单测覆盖 backoff 范围；任务级未重复）。

6) 计划中的后续改进（若进入 P2.3d / P2.4）：
  - 本地化/多语言 Network 关键字扩展：在 `helpers::map_git2_error` 中增加中文“连接/超时”关键词匹配，或抽象成可配置正则。
  - 可选引入一个测试注入层（MockGitService）直接产出 `ErrorCategory::Network` 以稳定 attempt 进度断言。
  - 增加并发隔离测试：两个并行 clone 任务分别设定截然不同 max/baseMs/factor，断言事件一次且互不污染（尤其 retried_times 计数独立）。
  - 增加 factor/jitter 组合 (极小 baseMs + jitter=true) 的延迟范围抽样统计，确保 backoff 不降为 0 也不过度爆炸。

7) 回退再补充：若需暂时关闭 Retry 覆盖事件，可仅删除 `if changed { emit ... retry_strategy_override_applied }` 分支；功能仍保留（使用覆盖后的 plan），进一步回退则移除 `apply_retry_override` 调用即可恢复旧逻辑（使用 `load_retry_plan()`）。

> 本节作为 P2.3c 的“增量追踪”，若后续补上并发与本地化改进，请把新增测试文件 / keyword 匹配策略附加到此段落，保持历史演进透明。

```

### P2.3e 任务级策略覆盖护栏（忽略字段 + 冲突规范化）实现说明（已完成）

本阶段统一交付两类护栏能力：
1) 未知/越权字段收集与一次性提示（ignored fields）。
2) 冲突/互斥字段自动规范化与冲突事件提示（conflict normalization）。

目标：在不阻断任务的前提下提升策略覆盖可观测性，确保进入底层网络/TLS 实现前的参数组合自洽，可快速定位调用方配置问题，并保持易回退。

#### 1. 功能范围
- 适用任务：`GitClone` / `GitFetch` / `GitPush`。
- 覆盖对象：`strategyOverride` 顶层与 `http` / `tls` / `retry` 子对象。
- 护栏事件：
  - 忽略字段：`strategy_override_ignored_fields`
  - 冲突规范化：`strategy_override_conflict`
  - 已有应用事件：`http|retry|tls_strategy_override_applied`（仅值真实改变时）

#### 2. 忽略字段（Ignored Fields）
- 解析时收集：
  - 顶层未知键 → `ignored_top_level`
  - 分节未知键（记录为 `section.key`，如 `http.foo`）→ `ignored_nested`
- 若任一集合非空 ⇒ 发射一次事件：
```
code: strategy_override_ignored_fields
category: Protocol
message: strategy override ignored unknown fields: top=[a/b] sections=[http.x/tls.y]
```
- 幂等：任务仅解析一次；事件至多一次；与其它事件并存。

#### 3. 冲突规范化（Conflict Normalization）
当前支持规则：
| 类别 | 条件 | 规范化 | 说明 |
|------|------|--------|------|
| HTTP | followRedirects=false 且 maxRedirects>0 | maxRedirects=0 | 否则语义悖论（禁止跟随却限制次数>0） |
| TLS  | insecureSkipVerify=true 且 skipSanWhitelist=true | skipSanWhitelist=false | 放宽验证已跳过完全验证，名单开关无意义 |

检测到冲突 ⇒ 修改局部值后再进入后续逻辑，并发射冲突事件：
```
code: strategy_override_conflict
message 示例: http conflict: followRedirects=false => force maxRedirects=0 (was 3)
```
每个冲突规则独立事件（当前实现至多 2 条）；可未来聚合。

`changed` 语义：仅比较“最终规范化后值” 与全局默认；因此：
- 若用户提供冲突组合导致规范化后仍与默认不同 → 同时出现 `*_applied` 与 `strategy_override_conflict`。
- 若规范化后回到默认 → 仅出现冲突事件，不出现 applied。

#### 4. 事件顺序与并发
同一任务内顺序：
1. `*_strategy_override_applied`（HTTP / TLS / Retry 各自独立，按应用顺序）
2. `strategy_override_conflict`（按检测顺序 HTTP→TLS）
3. `strategy_override_ignored_fields`

并发任务互不影响：各自解析与事件缓冲独立，已在组合测试中验证。

#### 5. 代码落点与数据结构
- `opts.rs`: `StrategyOverrideParseResult { parsed, ignored_top_level, ignored_nested }` 返回额外集合。
- `GitDepthFilterOpts`: 承载忽略字段集合以便 clone/fetch 路径复用。
- `registry.rs`:
  - `apply_http_override` / `apply_tls_override` 签名扩展返回 `(values..., changed, conflict: Option<String>)`。
  - 任务 spawn 流程：解析 → HTTP 应用 → TLS 应用 → Retry 应用 → emit applied → emit conflict(s) → emit ignored。

#### 6. 测试矩阵（合并后）
| 类别 | 文件 | 关注点 |
|------|------|--------|
| 忽略字段 | `git_strategy_override_guard_ignored.rs` | 有/无未知字段事件一次性行为 |
| HTTP 冲突 | `git_strategy_override_conflict_http.rs` | follow=false + max>0 规范化 & 冲突事件 |
| TLS 冲突 | `git_strategy_override_conflict_tls.rs` | insecure=true + skipSan=true 规范化 & 冲突事件 |
| 组合冲突+忽略 | `git_strategy_override_conflict_combo.rs` | HTTP+TLS 冲突 + ignored 同时出现次数准确 |
| 无冲突路径 | `git_strategy_override_no_conflict.rs` | 合法组合无 conflict/ignored；仅必要 applied |
| 回归（已存在） | HTTP/TLS/Retry 各 applied 测试 | 未因护栏改变原有触发条件 |
| 单元（registry） | 覆盖函数测试 | changed / conflict 分支与规范化正确 |

全部测试（后端 + 前端）绿：冲突/忽略新增后无 flakiness。

#### 7. 回退策略
| 目标 | 操作 | 副作用 |
|------|------|--------|
| 仅关闭冲突事件 | 移除 conflict emit 分支 | 仍执行规范化（静默） |
| 关闭规范化 | 移除冲突检测与局部值调整 | 可能向后传播矛盾组合（后续实现需自保） |
| 仅关闭忽略字段事件 | 移除 ignored emit 分支 | 日志仍可定位未知字段 |
| 完全回退护栏 | 恢复旧函数签名 + 移除结构字段与测试 | 回归到仅应用策略，无可观测护栏 |

#### 8. 安全与观测
- 事件仅暴露键名与简单布尔/数字差异，不携带敏感值（证书、Token 等未包含）。
- 冲突信息包含原值（`was X`）辅助调试；未记录完整原始 JSON，避免过度噪声。
- 统一 `category=Protocol`，前端无需新增分类通道。

#### 9. 已知限制 / 后续展望
- 冲突规则为硬编码列表：未来新增策略字段需同步扩展或抽象规则表。
- 目前将 HTTP/TLS 冲突分别发事件；若冲突种类增多可聚合为单条结构化 payload。
- 未对 retry 与其它策略之间潜在耦合（例如极端 backoff 与禁用 redirect 组合）做跨域冲突检测。
- 未提供“dry-run diff” 汇总事件（可后续新增 summary 事件减少多条提示）。

#### 10. Changelog 与完成度
```
Added: per-task strategy override guard (ignored fields) + conflict normalization with events `strategy_override_ignored_fields` & `strategy_override_conflict` (HTTP follow=false => maxRedirects=0; TLS insecureSkipVerify=true => skipSanWhitelist=false).
```
完成度：功能、生效路径、事件、测试、文档与回退策略全部落地。

#### 11. 结论
护栏体系（忽略字段 + 冲突规范化）显著提升策略覆盖的透明性与安全性，不改变既有任务核心语义，具备细粒度回退开关，风险低、收益高，可作为后续引入更多策略字段的基础模板。



---
### P2.3 任务级策略覆盖路线图（约 0.5–0.75 周）
> 补充：本节后追加的 P2.3d TLS 实现摘要（快速阅读版），详尽内容见本文后续 "P2.3d 任务级 TLS 策略覆盖实现说明" 章节。

#### P2.3d TLS 实现摘要（快速版）
| 维度 | 内容 |
|------|------|
| 目标 | 为 clone/fetch/push 提供 per-task `strategyOverride.tls`（仅 `insecureSkipVerify` / `skipSanWhitelist` 布尔覆盖），不触及全局 `san_whitelist`，改变值时发一次提示事件。 |
| 关键函数 | `TaskRegistry::apply_tls_override(kind,id,global,tls_override)` 返回 `(insecure, skipSan, changed)`；不修改 `global`。 |
| 集成点 | 三个任务 spawn 解析 override 后按顺序应用：HTTP → TLS → Retry（顺序只影响日志展示，无逻辑耦合）。 |
| 事件 | `task://error`，`category=Protocol`，`code=tls_strategy_override_applied`，`message="tls override applied: insecureSkipVerify=<bool> skipSanWhitelist=<bool>"`，单任务最多一次。 |
| 安全护栏 | 不允许任务级覆盖 `san_whitelist`；仅两布尔开关；全局对象不可变；空对象 / 未知字段忽略并 warn。 |
| 测试矩阵摘要 | clone insecure=true；clone unchanged；fetch skipSan=true；push 双开关；push 仅 insecure；并行组合 (http+tls+retry / tls-only / unchanged)；mixed (insecure + tls 空 + tls 未知 + skipSan)；单元：所有合并分支 & 全局未变。 |
| 回退 | 删除事件发射分支 → 仅日志；移除函数调用 → 停用 TLS 覆盖；删除函数与测试 → 完全回退。 |
| 幂等/并发 | `changed` 判定一次；多任务并发互不影响；值未变不发事件。 |
| Changelog | Added: per-task TLS strategy override (insecureSkipVerify/skipSanWhitelist) with informative event `tls_strategy_override_applied`. |

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
（编号校正说明：原文档初稿中 Retry 与 TLS 子阶段编号存在对调；现统一采用 a=模型解析, b=HTTP, c=Retry, d=TLS, e=护栏, f=文档。此前“P2.3c 应用于 TLS” 描述已在后续实现中落地为 P2.3d，本节已同步更正。）

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

- P2.3c 应用于 Retry（max/baseMs/factor/jitter）
  - 范围：任务内覆盖 Retry v1 计划；Push 的“上传前可重试”约束保持。
  - 交付：可重试类别下的重试次数/退避生效；不可重试类别不变。
  - 验收：事件中可选 retriedTimes 正确；不改变参数组合与阶段语义。
  - 回滚：移除 Retry 覆盖应用。

- P2.3d 应用于 TLS（insecureSkipVerify/skipSanWhitelist）
  - 范围：任务内浅合并 TLS 两个布尔开关；记录护栏日志（当前阶段仅日志+事件，不改变真实验证逻辑）。
  - 交付：开/关策略用例；并发任务隔离；事件仅在 changed 时一次。
  - 验收：多任务并行事件幂等；不修改全局 san_whitelist；日志脱敏。
  - 回滚：移除 TLS 覆盖应用或事件分支。

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

### P2.3d 任务级 TLS 策略覆盖实现说明（已完成）

本阶段在已落地的 HTTP (`http_strategy_override_applied`) 与 Retry (`retry_strategy_override_applied`) 覆盖基础上，引入 `strategyOverride.tls`（`insecureSkipVerify` / `skipSanWhitelist`） 的任务级浅合并与一次性提示事件：`tls_strategy_override_applied`。

#### 1. 目标与范围
- 仅允许两个布尔开关的按任务覆盖；不允许覆盖 `san_whitelist` 列表（保持全局安全基线）。
- 覆盖后仅影响该任务生命周期内的“有效 TLS 行为标志”（当前阶段尚未真正接入底层 TLS 逻辑，作为占位与观测），并发任务互不干扰。
- 当且仅当至少一个值与全局配置不同，发送一次非致命提示事件（`category=Protocol`，`code=tls_strategy_override_applied`）。
- 未变化不发事件；解析错误仍走现有 Protocol 失败路径，不发送 override 事件。

#### 2. 合并规则
| 步骤 | 描述 |
|------|------|
| 基线 | 复制 `AppConfig::default().tls`（后续阶段可替换为运行时加载配置） |
| 覆盖 insecureSkipVerify | 提供且不同则替换，`changed=true` |
| 覆盖 skipSanWhitelist | 提供且不同则替换，`changed=true` |
| 不可覆盖 | `san_whitelist` 永远保持全局（安全基线） |
| 事件 | `changed=true` 时发送一次 `tls_strategy_override_applied` |

#### 3. 代码落点
- 函数：`TaskRegistry::apply_tls_override(kind, id, global_cfg, tls_override)` → `(insecure_skip_verify, skip_san_whitelist, changed)`；放置于 `registry.rs` 与 HTTP/Retry 同级。
- 集成：在 `spawn_git_clone_task_with_opts` / `spawn_git_fetch_task_with_opts` / `spawn_git_push_task` 中解析 `strategy_override` 后调用，位置紧随 HTTP 覆盖之前或之后（当前实现：HTTP→TLS→Retry，一次性顺序，不影响逻辑）。
- 日志：`tracing target="strategy" ... "tls override applied"`，包含 taskKind 与 taskId。
- 任务 options 接受日志：扩展 clone/fetch(push) 的 `tracing::info!(strategy_tls_insecure=?, strategy_tls_skip_san=?)` 字段。

#### 4. 事件结构
```json
{
  "taskId": "<uuid>",
  "kind": "GitClone|GitFetch|GitPush",
  "category": "Protocol",
  "code": "tls_strategy_override_applied",
  "message": "tls override applied: insecureSkipVerify=<bool> skipSanWhitelist=<bool>",
  "retriedTimes": null
}
```

#### 5. 测试矩阵
测试文件：`git_tls_override_event.rs`
| 用例 | 说明 |
|------|------|
| clone insecureSkipVerify=true | 触发事件一次 |
| clone unchanged (false/false) | 不触发事件 |
| fetch skipSanWhitelist=true | 触发事件一次 |
| push insecure=true+skipSan=true | 触发事件一次 |
| push insecure=true (only) | 触发事件一次（新增 `git_tls_push_insecure_only.rs`） |
| 并行 clone (http+tls+retry / tls-only / unchanged) | 各任务事件计数分别 1 / 1 / 0（`git_strategy_override_tls_combo.rs`） |
| mixed clone(insecure) + fetch(tls空对象) + fetch(tls含未知字段) + push(skipSan) | 仅 clone / push 触发，空对象与未知字段不触发（`git_strategy_override_tls_mixed.rs`） |
| apply_tls_override 单元：无覆盖/单字段/双字段/不变/全变/全局未变 | 覆盖全部合并分支与不变幂等（`tls_override_tests_new`） |

（未添加“无效值”测试：布尔解析错误会在上层 serde 层直接失败并归类为 Protocol，与 HTTP/Retry 行为一致。）

#### 6. 幂等与并发
- 每任务仅在解析阶段调用一次 `apply_tls_override`；`changed` 判定为 true 时发事件一次；后续重试循环不会再次发射（与 HTTP/Retry 逻辑一致）。
- 不修改全局配置对象；不同任务可独立覆盖不同组合值互不影响。

#### 7. 回退策略
| 操作 | 效果 |
|------|------|
| 删除事件发射分支 | 保留合并与日志，不再对前端显示提示 |
| 移除 `apply_tls_override` 调用 | 回到“仅解析不应用”状态（保持模型兼容） |
| 移除函数与测试文件 | 完全回退 TLS 覆盖实现（保留 HTTP/Retry） |

#### 8. 安全与限制
- 不允许任务级改变 SAN 白名单列表，避免规避域名验证策略；未来若需调试能力，可在受控构建引入显式危险开关。
- 未实际改变底层证书验证逻辑（后续引入自定义传输/TLS 层时再接入）。
- 若将来代理模式护栏（P2.3e/P5）要求强制 Real SNI，可在此基础添加：`insecureSkipVerify=true` 时附加二次提示。

#### 9. Changelog 建议追加
```
Added: per-task TLS strategy override application (insecureSkipVerify/skipSanWhitelist) with informative event `tls_strategy_override_applied`.
```

#### 10. 现状结论
- 函数与事件已落地；测试矩阵全部通过；与既有 HTTP/Retry 覆盖事件并存，无字段冲突；前端无需新增订阅。

#### 11. 测试增强补充（本次追加）
- 新增单元测试模块：`tls_override_tests_new`（覆盖无覆盖/单字段变化/双字段变化/值不变）。
- 新增并发/组合集成测试：`git_strategy_override_tls_combo.rs`（并行 3 个 clone 任务分别 http+tls+retry 全变 / tls-only / 全部不变，验证事件幂等与互不串扰）。
- 原 `git_tls_override_event.rs` 继续保留基础串行路径；组合测试验证并行下事件次数精确为 1。
- 所有新增测试总计 +6（单元 5 + 集成 1）均通过；全套测试总数提升，未引入 flakiness（在 Windows 环境 <1s 对应单元模块执行）。

### P2.3f 任务级策略覆盖文档与前端支持（本次完成）

> 本章节为最终收束：在既有解析 / 应用 / 护栏与事件完成后，补齐前端 API 透传、事件 code 存储与文档示例。

#### 1. 交付内容
- 前端 API：`startGitClone` / `startGitFetch` / `startGitPush` 支持 `strategyOverride`（与可选 depth/filter）。
- 公共类型：`StrategyOverride`（http/tls/retry 三子集）。
- Store：`lastErrorById` 增加 `code` 字段；用于区分 informational 提示与真正失败（后续可在 UI 过滤分类）。
- 新前端测试：`strategy-override.events.test.ts` 覆盖 applied/conflict/ignored 事件 code 记录。
- README：新增使用示例与事件代码表；本设计文档追加总结章节。

#### 2. 事件代码（最终矩阵）
| code | 触发条件 | 幂等 | 分类 |
|------|----------|------|------|
| http_strategy_override_applied | follow/max 至少一项与全局不同 | 单任务一次 | Protocol |
| tls_strategy_override_applied | insecure/skipSan 至少一项不同 | 单任务一次 | Protocol |
| retry_strategy_override_applied | 任一重试参数不同 | 单任务一次 | Protocol |
| strategy_override_conflict | 互斥组合被规范化（HTTP 或 TLS） | 规则数上限（≤2） | Protocol |
| strategy_override_ignored_fields | 出现未知字段（顶层或子节） | 单任务一次 | Protocol |

顺序：applied → conflict → ignored；三类 applied 互不排斥；conflict/ignored 可与任意 applied 并存。

#### 3. 回退策略归档
| 目标 | 操作 | 保留影响 |
|------|------|----------|
| 关闭 informational 事件 | 移除 emit 分支 | 覆盖仍生效 |
| 关闭单类覆盖 | 移除对应 apply_* 调用 | 其它仍可用 |
| 关闭冲突规范化 | 移除冲突检测修改 | 可能传播矛盾组合 |
| 关闭忽略字段事件 | 移除 ignored emit | 日志仍可定位 |
| 全量回退 | 移除解析与全部 apply | 恢复全局配置行为 |

#### 4. 不变性
- 所有新增参数可选；旧调用无改动。
- 事件通道未增加；仅附加 code 字段。
- Informational 事件不改变任务 state，失败语义不变。

#### 5. 风险与验证
- 仅前端透传与显示层改动；核心判定逻辑早期阶段已由后端测试矩阵覆盖。
- 新增单测验证 code 存储，降低回归风险。

#### 6. 结论
P2.3f 标记完成；策略覆盖功能对调用方“自描述”闭环形成。后续增加新策略字段可沿用相同模式（解析→应用→事件→文档补充）。

#### 7. 兼容性与调用护栏（补充合并）
本阶段在不破坏既有调用的基础上引入多项兼容与容错：
- 旧版 `startGitFetch(repoPath, remote)` （第二参数为远端名字符串） 仍受支持；新版对象式签名 `startGitFetch({ repo, remote, depth, filter, strategyOverride })` 检测到第二参数为字符串时回退旧路径（测试：`git.fetch.compat.test.ts`）。
- `preset=remote` 省略时不显式传入（测试：`git.fetch.remote-omit.test.ts`），确保参数最小化与后端期望一致。
- 空对象 `strategyOverride: {}` 会被透传（测试：clone 空 override 用例），不会触发任何 applied/conflict/ignored 事件，保证“显式声明为空” 与 “未提供” 语义相同。
- “仅 override” / “credentials+override 组合” / “override + depth/filter 组合” 等多种排列均已在前端 API 测试中覆盖（文件：`git.api.test.ts` 中组合场景 + push credentials+override）。
- 未知字段与互斥组合在后端被护栏事件捕获，不影响任务主流程（参见 P2.3e 章节），前端无需额外分支。

#### 8. retriedTimes 语义完善
为避免信息型（informational）策略事件覆盖真实重试进度导致的 `retriedTimes` 可观测性下降，Store 合并逻辑采用“保留与提升”策略：
- 若新到达的 `task://error` 事件缺失 `retriedTimes` 字段，则沿用先前已记录的数值。
- 若携带 `retriedTimes` 且值更大，则更新（提升）；更小则忽略，防止回退。
相关测试：`tasks.error.retried-preserve.test.ts` 覆盖“信息事件不清空” 与 “更大值提升” 两条路径。此语义确保：
1) 策略 applied/conflict/ignored 等信息提示不会让界面回退到“未重试”状态；
2) 后续真实重试（若分类为可重试）仍可正确累进展示。

#### 9. 测试矩阵增量汇总（P2.3f 相对 P2.3e）
前端新增 / 扩展测试类别：
- 事件 code 存储与顺序：`strategy-override.events.test.ts`、`tasks.strategy-order.test.ts`、`tasks.strategy-multi-applied.test.ts`。
- 兼容性：`git.fetch.compat.test.ts`、`git.fetch.remote-omit.test.ts`。
- 参数组合：`git.api.test.ts` 扩展（empty override / override-only / http+tls+retry / retry-only / credentials+override / depth+filter+override 交叉）。
- retriedTimes 逻辑：`tasks.error.retried-preserve.test.ts`。

后端（Rust）在原有 HTTP/TLS/Retry/护栏测试基础上无需新增代码路径，因此本阶段未再添加新的后端文件；但通过前端集成测试间接验证：
- 事件幂等（单任务 applied* ≤1，冲突规则 ≤2，ignored ≤1）。
- 冲突与规范化不影响 applied 触发条件（normalized 后仍差异则双事件）。
- 旧 fetch 签名路径与新签名路径行为一致（仅参数封装差异）。

总体当前统计（参考最近一次全量运行）：
- 前端测试：≈108 用例（新增/扩展用例集中在策略事件与 API 兼容面）。
- 后端测试：≈66 用例（含策略覆盖、护栏、git 基础命令）。

#### 10. 质量与回归护栏
本阶段引入的文档与测试强化了以下不变量（由测试锁定）：
- 调用兼容：旧 fetch 签名不抛异常；空 override 与缺省等价；remote preset 省略不改变行为。
- 事件顺序：applied → conflict → ignored；无逆序交叉。（顺序测试确保新增逻辑插入点固定）。
- 幂等：单任务每类 applied 事件至多一次；信息事件不会降低 retriedTimes；冲突事件数量受规则集大小约束。
- 安全：策略事件仅承载布尔/数字差异，不含敏感凭证或路径；凭证 + override 组合不泄露凭证到事件 payload。
- 回退：任一覆盖或事件类别可通过移除对应 emit/调用点有界撤销，不需要迁移或清理存量数据。

风险评估：剩余主要风险集中在未来新增策略字段时的规则扩展一致性；当前通过事件 code 矩阵与回退策略（章节 3 与 7）形成明确边界。建议后续新增字段时：先增补矩阵 & 护栏测试，再接入前端透传，保持与 P2.3f 模式一致。

