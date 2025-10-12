# P2 阶段技术设计文档

## 1. 概述

本文基于 MP0/MP1 已完成能力（git2-rs 基线、本地仓库操作初始集、Push、自定义 smart subtransport(A) 灰度、Retry v1、事件分类增强）继续扩展至 P2：完善本地 Git 常用操作、浅/部分克隆与策略覆盖（HTTP / TLS / Retry），并引入可观测护栏与环境 gating。目标是在不破坏现有前端任务/事件协议的前提下新增可选字段与信息型事件，保持严格回退路径与充分测试覆盖。

### 目标
1. 本地 Git 常用操作：覆盖 init / add / commit / branch / checkout / tag / remote(set-url/add/remove)，统一事件与错误分类。
2. Shallow / Partial：为 clone / fetch 提供 depth 与 filter 输入，远端或环境不支持时按“最近原则”明确回退并给出结构化提示。
3. 任务级策略覆盖：允许通过 strategyOverride 对 http / tls / retry 子集进行按任务浅合并，提供可观测 applied / conflict / ignored / summary 事件与可控 gating。
4. 兼容性：保持既有前端命令、事件管道与 Store 结构不破坏；新增字段全部可选，输入格式兼容 snake_case 与 camelCase。

### 范围
- 后端：git2-rs 实现的本地仓库与引用操作；clone/fetch 的 depth 与 filter 决策、fallback、能力检测与 gating；按任务策略覆盖（HTTP/TLS/Retry）。
- 前端：命令入参扩展与事件展示，不强制 UI 结构变更；可选增强（最近错误、策略事件分层）。

### 不在本阶段
- 代理与自动降级（计划更后阶段）。
- IP 优选与 IP 池。
- 凭证安全存储。
- LFS 与指标面板。
- SSH 及系统 git 回退路径。

### 成功标准
1. 全量单元与集成测试通过（Windows 作为关键运行环境保持稳定）。
2. 本地 Git 操作在 Windows 上无路径大小写与换行差异导致的失败。
3. depth / filter 在受支持远端生效；不支持路径触发明确 Protocol 类回退事件且仍成功执行主要任务。
4. 任务级策略覆盖可生效；越权与互斥组合被规范化或忽略并以事件反馈。
5. 错误分类与 MP1 既有分类保持一致，无敏感信息泄漏。


## 2. 详细路线图

### P2.0 基线巩固与准备
目标：在进入增量功能（commit/branch 等）前，确保 MP1 基线的稳定性与可扩展性，为后续阶段提供一致的任务注册、事件分类与测试支撑。
范围：
- 统一 TaskRegistry 任务生命周期：pending→running→completed/failed/canceled。
- 标准错误分类枚举与映射（Protocol/Network/Internal/Cancel），建立测试夹具。
- 事件通道约定：state / progress / error 复用；error 支持 code 字段（信息型或致命）。
- 初始化本地仓库操作最小集合：init / add。
- 引入跨平台（Windows 重点）测试运行脚本与临时仓库夹具工具。
交付物：
- 任务注册统一入口与取消原子标志。
- 错误分类辅助函数与单元测试。
- 初始测试夹具（创建临时仓库、写入文件、生成提交作者信息）。
- README/设计文档补充基线事件与分类表。
验收标准：
- 全量测试（至少含 init / add / 基础失败路径）稳定通过。
- 事件与状态顺序确定性，无竞态 flakiness。
- Windows 环境运行通过（CI 或本地）无路径大小写/换行差异导致失败。
回退策略：
- 如后续阶段出现行为变更冲突，可单独还原新增 init/add 实现与分类辅助；其余阶段使用的公共结构保持兼容。
风险与缓解：
- 风险：错误分类过早绑定具体字串 → 后续难扩展；缓解：集中匹配函数表驱动，可增量添加关键字。
- 风险：事件 code 扩展超前；缓解：约定信息型事件必须 category=Protocol 且不改变任务最终状态。

 
### P2.1a 本地操作扩展引导（init/add 基础与测试框架固化）
目标：在基线之上补齐最常用的初始仓库操作，为后续 commit/branch 等操作提供已验证的输入上下文与测试复用点。
范围：
- git init：创建空仓库（含 .git 目录存在性校验、重复初始化幂等处理日志）。
- git add：添加/更新工作区文件到 index（不含 rename / 权限位特殊处理）。
- 统一工作区写入与断言工具：封装写文件、改动文件、读取 HEAD/Index 状态函数。
- 增补进度与事件最小策略：本地快速操作仅在结束时发送一次 progress (percent=100)。
交付物：
- TaskKind 扩展：GitInit / GitAdd。
- 对应 Tauri 命令与前端 API 调用包装。
- 后端测试：初始化成功 / 重复初始化 / 添加单文件 / 添加多文件 / 空 add 不产生提交。
- 前端测试：触发任务、接收完成事件、错误路径（在非空路径上 init 仍成功幂等）。
验收标准：
- GitInit 幂等：二次执行不失败且事件结果可区分第一次与后续。
- GitAdd 对已存在文件修改后再次 add 仍成功，索引状态与工作区内容一致。
- 取消路径：开始前取消能即时终止；执行中文件枚举阶段取消不留下部分索引写入。
回退策略：
- 移除 TaskKind 分支与命令导出即可恢复到 P2.0 基线；测试夹具继续复用。
风险与缓解：
- 大量/大文件性能：暂限制测试规模，后续性能阶段再优化。
- Windows 行尾差异：通过二进制写入与基于 index 元数据断言规避换行差异。

### P2.1b Commit 能力与模板化规范
目标：引入提交（commit）操作，形成后续分支/标签等引用修改类操作的结构化模板（校验→副作用→单进度事件→错误分类）。
范围：
- git commit（支持 allowEmpty、可选作者覆盖）。
- 统一空提交判断逻辑：比较当前 index tree 与 HEAD tree 或首提场景 index 是否为空。
- 任务模型扩展：TaskKind GitCommit；前端 API 和 Store 类型更新。
- 事件：仅最终 progress（phase=Committed）。
交付物：
- GitCommit 任务注册与实现文件。
- 测试：成功提交、空提交拒绝/允许、作者覆盖、消息裁剪、多次重复提交、取消两阶段、输入错误分类。
- 文档：提交参数与错误分类增补。
验收标准：
- allowEmpty=false 时空提交必然返回 Protocol 分类错误；allowEmpty=true 可成功生成提交。
- 作者覆盖需同时提供非空 name 与 email；缺任一字段失败且不产生提交对象。
- 取消路径不产生部分提交（无孤立对象影响 HEAD）。
回退策略：
- 移除 GitCommit TaskKind 与命令导出；测试标记忽略；其余阶段不受影响。
风险与缓解：
- 多平台本地用户 signature 不一致：允许显式作者覆盖保持可重复性。
- 空白消息处理：trim 后再校验，避免多余空格导致意外拒绝。

### P2.1c Branch 与 Checkout 引用管理
目标：支持创建分支与检出，形成对引用(ref) 修改的可控模板（名称校验、force 覆盖、安全取消点与单进度事件）。
范围：
- git branch：创建分支、可选 force 更新引用、可选创建后立即 checkout。
- git checkout：检出现有分支或在 create=true 时基于当前 HEAD 创建并检出。
- 分支命名校验规则两阶段增强（基础非法字符 → 追加更严格模式）。
- 取消点：写引用与切换 HEAD 前再次检查。
交付物：
- TaskKind 扩展：GitBranch / GitCheckout。
- 分支命名校验函数与测试（合法/非法用例集合）。
- 集成测试：创建、force 覆盖、已存在冲突、checkout 不存在、create+checkout、取消、非法名称、幂等检出。
验收标准：
- 非 force 创建已存在分支返回 Protocol 错误；force 创建正确更新指向。
- checkout create 在无 HEAD（无提交）场景失败（Protocol）。
- 所有非法名称均返回 Protocol 分类且未写入引用。
回退策略：
- 移除 TaskKind 与命令导出；保留命名校验函数供后续 tag 引用。
风险与缓解：
- 名称规则可能不足覆盖全部 git 约束：以迭代增强策略，通过集中测试列表便捷扩展。
- 并发写引用冲突：当前单进程内序列化操作，后续才考虑并发锁定。

### P2.1d Tag 与 Remote 管理
目标：补齐标签与远程管理操作，完成本地引用与远程配置的最小闭环，为后续 shallow/partial 与策略覆盖提供可复用引用状态前置条件。
范围：
- git tag：轻量与附注标签创建、force 覆盖、消息规范化（换行统一、尾部空行折叠）。
- git remote：add / set-url / remove，URL 基础合法性校验与幂等行为。
- 进度 phase 细化：Tagged / AnnotatedTagged / Retagged / AnnotatedRetagged / RemoteAdded / RemoteSet / RemoteRemoved。
- URL 校验策略：原始含空白直接拒绝；支持 http/https、scp-like、无空格本地路径；其余 Protocol 错误。
交付物：
- TaskKind 扩展：GitTag / GitRemoteAdd / GitRemoteSet / GitRemoteRemove。
- 标签命名与 remote 命名校验（可复用分支规则部分或独立最小集）。
- 测试：标签首次/重复非 force/force 相同内容 OID 稳定、附注缺消息失败、非法 URL、多次 set 同 URL 幂等、remove 不存在失败、取消点、安全回退。
验收标准：
- force 覆盖区分首次与覆盖 phase，且同内容 force 不产生新对象（Annotated）。
- URL 含换行/制表符全部拒绝。
- 删除不存在远程返回 Protocol 分类，无副作用。
回退策略：
- 移除 TaskKind 与命令导出即可，不影响之前 branch/commit 功能。
风险与缓解：
- 附注消息规范化差异：通过测试固定 CRLF→LF 与尾部裁剪规则，避免跨平台差异。
- URL 判定过宽：后续可按需补充白名单 scheme 或引入更严 parser。

### P2.2a Shallow Clone 初始 Depth 支持
目标：在 clone/fetch 中引入 depth 最小可行实现（仅浅度，不含 filter），建立后续 partial/fallback 判定与组合策略基线。
范围：
- 入参扩展：`depth?: number`（>0 整数）。
- 仅 clone / fetch 解析与应用；push 无影响。
- depth 透传 git2 clone/fetch 选项；不改变现有 progress/事件模型。
- 记录浅克隆与 deepen 日志（首次 shallow / 追加 depth 增大）。
交付物：
- 解析与范围校验。
- CloneOpts / FetchOpts 结构扩展。
- 测试：clone depth=1；后续 fetch depth=2 deepen；非法 depth（0/负/非数）Protocol；完整仓库 fetch depth 不破坏历史。
验收标准：
- 合法 depth 成功且存在 `.git/shallow`。
- deepen 后提交数增加且不超真实历史。
- 非法 depth 均 Protocol，未产生残留半成品仓库。
回退策略：
- 去除 depth 字段解析与传递即可回退为完整克隆；相关测试忽略或删除。
风险与缓解：
- 远端差异：断言最小集合（存在 shallow 文件 + 至少 1 提交）。
- 与后续 filter 交互复杂：预抽象判定结构（DepthOnly/Full），降低将来 partial 接入的改动面。

### P2.2b Partial Clone Filter 解析与初始回退逻辑
目标：解析并接入 `filter`（部分克隆，如 `blob:none`）最小路径，在尚未真正下沉对象裁剪的前提下建立回退与决策骨架（与 depth 并存），确保后续引入真实过滤与环境 gating 时无需大量重构。
范围：
- 入参扩展：`filter?: string`；支持集合（首批：`blob:none`、`tree:0`），超出集合返回 Protocol。
- 构建 `DepthFilterDecision`：`Full | DepthOnly | FilterOnly | DepthAndFilter(planned) | FallbackShallow | FallbackFull`。
- 当前阶段不读取远端 advertised capabilities；不做真实过滤下载，仅记录决策并允许任务继续。
- 提前预留 fallback 事件 code 名称（本阶段不发射，下一阶段启用）。
- 与 depth 共存：当同时提供 depth+filter 时置标记（下一阶段实现联合策略）。
交付物：
- filter 字段解析与校验函数。
- 决策函数 `decide_depth_filter(depth, filter)` 返回枚举。
- 测试：
  - 仅 filter（受支持值）→ FilterOnly 决策。
  - depth+filter → 暂返回 DepthOnly 并标注联合待实现（决策枚举区分）。
  - 不支持 filter 值 → Protocol。
  - 空字符串 / 仅空白 → Protocol。
  - 仅 depth → DepthOnly；均缺省 → Full。
验收标准：
- 所有受支持 filter 值解析成功，错误值全部分类为 Protocol。
- 决策枚举覆盖上述六条路径（本阶段不发射相关事件）。
- 无新增事件；现有 clone/fetch 成功率不下降；测试全绿。
回退策略：
- 移除 filter 解析与决策函数；depth 逻辑保持；测试删除或忽略 filter 相关用例。
风险与缓解：
- 风险：后续启用真实过滤需要补充对象裁剪 → 通过分阶段保留 FilterOnly 判定与集中测试降低侵入。
- 风险：过早暴露联合深度+过滤语义 → 使用枚举 planned variant 隔离，未对外宣称事件或状态改变。

### P2.2c Partial/Depth 联合策略与回退事件
目标：完善 depth 与 filter 同时提供时的联合决策及真实回退事件发射，向前端明确告知在当前阶段 filter 尚未生效或被降级，从而为后续真正部分对象裁剪实现提供稳定事件契约。
范围：
- 扩展 `DepthFilterDecision`：实现 `DepthAndFilter` 分支语义；当 filter 支持标记缺失或尚未启用时根据输入组合判定降级：
  - depth+filter → 若环境/远端不支持 filter ⇒ `FallbackShallow`（保留 depth）
  - 仅 filter 且不支持 ⇒ `FallbackFull`
- 引入信息型回退事件：`code=partial_filter_fallback`，`message` 包含 `requestedDepth`, `requestedFilter`, `decision`。
- 事件分类仍为 `Protocol`，不改变任务最终状态；单任务至多一次。
- clone / fetch 两类任务均支持；push 无影响。
- 与现有 HTTP/Retry/TLS 策略事件并存，顺序：策略 applied/conflict/ignored 之后、strategy summary 之前（若存在）。
交付物：
- 决策实现与单元测试（覆盖所有输入组合：无输入 / 仅 depth / 仅 filter 支持 / 仅 filter 不支持 / depth+filter 支持 / depth+filter 不支持）。
- 事件发射逻辑与序列测试（验证事件出现次数与顺序）。
- 集成测试：
  - clone depth+filter（远端不支持 filter 模拟）→ fallback shallow 事件。
  - clone 仅 filter （不支持）→ fallback full 事件。
  - fetch deepen + filter（不支持）→ fallback shallow 且 deepen 生效（提交数增加）。
  - 支持场景模拟（通过注入测试钩子）→ 无 fallback 事件。
验收标准：
- 不支持 filter 的两类降级路径均触发单一 fallback 事件，字段准确。
- 支持场景不触发 fallback；决策枚举与测试断言一致。
- fallback 不影响任务成功、进度与已有策略事件（无丢失或乱序）。
回退策略：
- 移除事件发射与降级分支（恢复到 P2.2b 的 FilterOnly/DepthOnly 判定）；测试相应忽略。
风险与缓解：
- 风险：事件顺序与后续 summary 事件潜在竞态 → 通过顺序测试锁定“fallback 先于 summary”。
- 风险：模拟“支持 filter” 钩子与未来真实 capability 检测差异 → 统一抽象 `PartialCapabilityProvider` 接口，后续替换实现即可。

### P2.2d Partial 能力检测与环境 Gating
目标：引入对运行环境与远端是否支持 partial filter 的能力探测与可控 gating（环境变量 + 探测缓存），使 fallback 决策从“静态假设不支持”升级为“基于真实能力与显式开关”。
范围：
- 环境变量 `FWC_PARTIAL_FILTER_SUPPORTED`：未设或=0 表示本进程声明不支持 filter，=1 表示允许尝试；解析为布尔 gating。
- 远端能力探测：首次 clone/fetch（有 filter 请求）时执行最小探测（当前阶段以模拟钩子替代，后续可扩展 ls-remote 或 version 协议特征）。
- 引入缓存：按 remote URL 级别存储探测结果（进程内 HashMap），避免重复探测。
- 决策更新：仅当 gating=true 且探测=支持 时才进入 FilterOnly / DepthAndFilter；否则沿用 fallback 流程。
- 日志：记录 gating 值、探测结果、最终决策；支持调试标记 `strategy.partial.capability`。
交付物：
- `PartialCapabilityProvider` 实现（含 env 读取、缓存、测试注入）。
- 决策函数扩展以调用 provider。
- 测试：
  - gating=0 + filter → FallbackFull/FallbackShallow（与 depth 组合）且无探测调用。
  - gating=1 + provider 返回不支持 → fallback 事件仍触发（单次探测）。
  - gating=1 + provider 支持 → 不触发 fallback，决策进入 FilterOnly / DepthAndFilter。
  - 缓存命中：两次同 URL filter 请求仅一次探测（计数断言）。
  - 不同 URL 独立探测。
验收标准：
- 探测在需要时恰好调用一次并缓存；禁用 gating 完全绕过探测。
- 支持路径无 fallback 事件；不支持路径有且仅一条 fallback 事件。
- 环境变量非法值（非 0/1）按 0 处理并记录警告日志。
回退策略：
- 移除 capability provider 调用，恢复到 P2.2c 静态逻辑；保留结构方便再开启。
风险与缓解：
- 风险：未来真实探测耗时增加初次任务延迟 → 允许异步预热（后续阶段）。
- 风险：缓存污染（不同远端同域名差异）→ 缓存 key 使用规范化完整 URL（scheme+host+path）。

### P2.2e Shallow/Partial 鲁棒性与回归测试强化
目标：在基础与能力检测落地后，补齐边界条件、错误路径与并行场景测试，降低后续引入真实对象裁剪与网络差异时的回归风险。
范围：
- 增补多次 deepen：depth 逐步 1→2→4 验证历史递增且不重复 fallback 事件。
- filter 不支持场景下并行多个 clone（含不同 depth）仅各自一次 fallback，互不污染。
- 本地已完整仓库 + 再传 depth/filter：决策保持 Full，不产生 fallback。
- 非法 filter 与非法 depth 组合输入：优先报告第一个解析错误（明确顺序规则）。
- 软跳过外网依赖：公共仓库网络波动时标记 soft-skip 而非失败。
- 日志字段稳定性测试：抽取 depth/filter/fallback 关键信息匹配正则（防回归改名）。
交付物：
- 新测试文件：`git_shallow_partial_multi_deepen.rs`、`git_partial_parallel_fallback.rs`、`git_shallow_full_repo_noop.rs`、`git_partial_invalid_combo.rs`。
- 日志断言辅助：新增 `assert_log_contains_once` 针对策略模块。
- 文档更新：补充“多次 deepen 与并行” 注意事项。
验收标准：
- 多次 deepen 后提交数量单调递增且 `.git/shallow` 存在。
- 并行 fallback 事件计数 = 任务数；无重复。
- 完整仓库路径无 shallow 文件创建。
- 非法组合明确返回第一错误（测试锁定顺序）。
回退策略：
- 删除新增测试与日志断言，不影响核心功能；其余阶段保持可用。
风险与缓解：
- 风险：并行测试偶发顺序差异 → 仅断言计数与集合，不依赖顺序。
- 风险：外网依赖不稳定 → 使用软跳过策略与本地仓库镜像兜底（可选）。

### P2.2f 文档同步与前端参数透传完善
目标：将已实现的 shallow/partial（depth/filter/回退决策/gating）能力在前端与文档中完整揭示，确保调用方具备明确使用示例、事件解释与回退含义；为后续策略覆盖章节的 summary/gating 逻辑提供一致呈现模式。
范围：
- 前端 API：`startGitClone` / `startGitFetch` 参数说明补充 depth/filter；示例组合（仅 depth、depth+filter、filter-only）。
- UI：Git 面板展示 depth/filter 可选输入（暂文本框/数字输入，不做高级校验）。
- Store：记录 fallback 事件（code=partial_filter_fallback）并与策略类 informational 事件统一展示层次。
- 文档：
  - README 新增 Depth/Partial 使用章节与事件示例 JSON。
  - 技术设计（本文）收束 P2.2 阶段，列出全部决策枚举及其触发条件表。
  - Changelog 条目：Added shallow/partial clone (depth + filter parsing with fallback events and capability gating)。
- 测试：
  - 前端集成：发起多种组合任务并断言事件 code 渲染与顺序。
  - 文档链接校验（可选脚本，确保 README anchors 存在）。
验收标准：
- 所有前端调用示例与当前实现一致，无未实现条目。
- 事件展示顺序：策略 applied/conflict/ignored → partial fallback → strategy summary（若存在）。
- 决策表与实现一致（测试比对枚举名称）。
回退策略：
- 移除前端 depth/filter UI 与 README 段落；功能仍可由脚本调用（不破坏后端）。
风险与缓解：
- 风险：文档与实现漂移 → 在 CI 添加“决策枚举快照”比对（后续阶段）。
- 风险：前端同时出现多类 informational 事件顺序不稳定 → 在任务启动端集中排序发送（当前已通过顺序约定测试锁定）。

### P2.3a 任务级策略覆盖模型与解析
目标：为后续 HTTP / Retry 策略按任务覆盖奠定统一数据结构、解析与校验基础，保证新增字段最小侵入现有命令与事件协议。（TLS 覆盖在安全审计后取消，详见 §P2.3d 说明。）
范围：
- 扩展任务输入结构：`strategyOverride`（可选，对 clone/fetch/push 生效）。
- 支持字段：
  - http.followRedirects:boolean, http.maxRedirects:number
  - retry.max:number, retry.baseMs:number, retry.factor:number, retry.jitter:boolean
- 解析兼容：camelCase 与 snake_case；未知字段收集（不立即发事件，在后续护栏阶段使用）。
- 校验：数值与范围（maxRedirects<=20, retry.max 1..20, baseMs 10..60000, factor 0.5..10, jitter bool）。
- 不产生任何新事件；仅日志（level=debug/info）记录解析结果与忽略字段集合。
交付物：
- 数据模型：Rust 结构体 + serde 自定义反序列化（双命名支持）。
- 解析辅助：`parse_strategy_override(json)` 返回 (parsed, ignored_top, ignored_nested, errors)。
- 测试：
  - 合法组合全字段。
  - 单字段缺失与可选为空对象 {}。
  - 大小写混用（follow_redirects / followRedirects）。
  - 越界值：maxRedirects=21、factor=0.4/10.1、baseMs=5、retry.max=0 → Protocol。
  - 未知字段：顶层 foo、http.xxx、retry.zz → ignored 集合记录。
  - 空对象与未提供语义等价（后续阶段逻辑一致）。
验收标准：
- 所有合法输入成功解析且无副作用；非法输入分类为 Protocol。
- 忽略字段不影响任务继续；错误与忽略互斥（遇到错误直接失败，不再下发覆盖）。
- 旧调用（不带 strategyOverride）行为不变（回归测试通过）。
回退策略：
- 移除解析模块与结构体，同时删除相关测试；命令仍接受旧参数集合。
风险与缓解：
- 风险：后续字段扩展频繁修改 serde 标签 → 通过集中 `strategy_override.rs` 文件隔离并加快审查。
- 风险：未知字段静默丢失影响可观测 → 后续护栏阶段引入 ignored 事件补足。

### P2.3b 任务级 HTTP 策略覆盖
目标：基于已解析的 strategyOverride，按任务应用 HTTP followRedirects/maxRedirects 覆盖，并在实际生效时通过结构化事件一次性曝光差异；不变更底层实际网络行为（预留后续接入）。
范围：
- 合并规则：仅当提供字段且与全局默认不同才变更；`maxRedirects` 上限 clamp=20。
- 事件：当覆盖值改变时发送一次 `StrategyEvent::HttpApplied { id, follow, max_redirects }`；冲突（follow=false 且 max>0）在 `GitClone` 触发 `StrategyEvent::Conflict { kind:"http", message }`；`GitPush` 仅补充一条信息级 `task://error`，`GitFetch` 只规范化并记录日志。
- 不改变 retry/TLS 或 clone/fetch/push 核心执行；仅在任务 spawn 前阶段合并。
- 结构化事件通过 `events::structured::publish_global` 发出，与 legacy 错误通道解耦。
交付物：
- `apply_http_override` 函数（返回 follow, max, changed）。
- 结构化事件发射逻辑与单元测试（clamp / changed / conflict 判定）。
- 集成测试：改变/不变/仅一字段改变/非法 max/多任务并发 idempotent。
验收标准：
- 仅当至少一项值改变发送一次 `HttpApplied`；冲突场景补充 `Conflict`。
- 非法参数（>20 或类型错误）Protocol 失败且不发送结构化事件。
- 其它策略字段未受影响；前端兼容（无新增解析分支）。
回退策略：
- 移除结构化事件发射；保留合并逻辑；或完全移除函数调用恢复默认行为。
风险与缓解：
- 风险：future 网络栈接入导致语义差异 → `HttpApplied` payload 仅暴露最终 follow/max。
- 风险：多策略先后顺序潜在竞态 → 以固定顺序 HTTP→Retry→TLS 并在测试锁定事件序列。

### P2.3c 任务级 Retry 策略覆盖
目标：为 clone/fetch/push 提供按任务自定义退避计划（max/baseMs/factor/jitter），在保持全局配置不变的同时通过结构化事件曝光差异。
范围：
- 合并规则：仅当任一字段与全局不同才视为 changed；解析层已校验范围。
- 生成独立 `RetryPlan`（不写回全局）。
- 事件：Clone/Push 在 `changed` 时发送一次 `PolicyEvent::RetryApplied { id, code:"retry_strategy_override_applied", changed }`；Fetch 不单独发该事件，但会在最终 `StrategyEvent::Summary` 的 `applied_codes` 中记录差异。
- 与 HTTP 覆盖并列，顺序：HTTP → Retry → TLS。
- 不改变现有重试分类与上限语义（不可重试错误不进入循环）。
交付物：
- `apply_retry_override` 函数与单元测试（changed 判定 / 不变路径）。
- 集成测试：变更/不变/仅一字段变更/边界 factor=0.5 & 10 / jitter=true 透传。
- 组合测试：与 HTTP 同时 changed 仍各发一次事件，次数不超过 1。
验收标准：
- Clone/Push 在 changed 时恰好发送一次 `PolicyEvent::RetryApplied`；Fetch 仅在 Summary `applied_codes` 中体现差异。
- 生成的计划仅影响当前任务；并发任务计划互不干扰（测试比对不同 max/baseMs）。
- 不可重试错误路径仍不会触发 attempt 重试进度，但 override 差异事件可出现。
回退策略：
- 移除 `PolicyEvent::RetryApplied` 发射或函数调用；其余逻辑保持；完全回退删除函数与测试。
风险与缓解：
- 风险：本地化错误文本导致分类 Internal 而非 Network 进而少重试 → 后续 i18n 分类扩展缓解。
- 风险：极端 factor/baseMs 组合导致过长等待 → 范围校验与单测锁定上限。

### P2.3d 任务级 TLS 策略覆盖（已取消）
最初计划在 P2 为单任务暴露 `insecureSkipVerify` / `skipSanWhitelist` 两个布尔开关，用于临时放宽 TLS 校验。然而在 v1.8 的安全审计中确认：
- `RealHostCertVerifier` 已强制在所有 Fake SNI 场景使用真实域名做链路与主机名校验，无法再回退到不安全路径；
- 配置模型同步移除了 `tls.insecureSkipVerify`、`tls.skipSanWhitelist` 开关，仅保留 SPKI Pin 与可观测性（`metricsEnabled`/`certFpLogEnabled`/`certFpMaxBytes`）；
- 任务输入 `strategyOverride` 未实现 TLS 分支，后端只接受 HTTP / Retry 字段；相关事件也未上线。

结论：任务级 TLS 覆盖功能在实现前即被取消，文档保留此章节用于说明决策背景，实际产品行为不支持任何 TLS 放宽开关。若需排障只能通过 SPKI Pin/可观测字段诊断或临时关闭 Fake SNI。P2 交付的策略覆盖仅包含 HTTP 与 Retry 子集。

### P2.3e 策略覆盖护栏（结构化 ignored/conflict 事件）
目标：在不阻断任务的前提下提供未知字段与互斥组合的可观测提示，保障策略配置可调试性与未来字段演进空间。
范围：
- 忽略字段事件：`StrategyEvent::IgnoredFields { top_level, nested }`，每任务至多一次。
- 冲突事件：`StrategyEvent::Conflict { kind, message }`，当前规则仅保留 HTTP follow=false & max>0；仅 Clone 发结构化事件，Push 额外复用 `task://error` 兼容旧 UI。
- 规范化：冲突发生时自动将 max 调整为 0，可能同时发出冲突与 applied 事件。
- 事件顺序：`HttpApplied`* → `Conflict`* → `IgnoredFields`? → 后续 Summary。
交付物：
- 解析结果返回 ignored 集合；合并函数返回 conflict 描述；
- 结构化事件发射逻辑与单元测试（含多冲突、多未知字段）；
- 集成测试：冲突（HTTP）、仅 ignored、无冲突。
验收标准：
- 每任务 ignored 事件至多一次；冲突事件数量 = 触发规则数；
- 规范化后值参与差异判定：若回到全局默认则只发 Conflict 不追加 `HttpApplied`；
- 顺序测试稳定通过。
回退策略：
- 移除 Conflict emit 保留规范化；或移除规范化恢复原值（高风险）；或全部移除回到仅 Summary。
风险与缓解：
- 风险：规则集合增加导致事件噪声上升 → 已通过 Summary 聚合 `applied_codes` 降噪；
- 风险：忽略字段误写难定位 → 事件 payload 保留字段列表与分组区分。

### P2.3f 策略覆盖汇总事件与前端集成收束
目标：通过结构化 Summary 完成策略覆盖可观测闭环，前端统一展示并文档化回退路径。
范围：
- Summary 事件：`StrategyEvent::Summary { http_*, retry_*, applied_codes, filter_requested }`，TLS 字段固定等同全局配置后不再包含；
- 独立 applied 事件沿用结构化变体（`Strategy::HttpApplied`、`Policy::RetryApplied`），未实现 runtime gating；
- retriedTimes 合并策略：信息型事件不降低已记录重试次数；
- 前端：推荐订阅结构化事件；Push legacy `task://error` 仅保留冲突提示；
- 文档：README / 设计文档更新事件矩阵与回退表；Changelog 追加条目。
交付物：
- `emit_strategy_summary` 实现 + 顺序测试（applied/conflict/ignored → summary）；
- 前端事件存储逻辑与测试（顺序 / retriedTimes 保留 / applied_codes 展示）。
验收标准：
- Summary 始终发送；`applied_codes` 列表包含 HTTP/Retry 差异；
- 所有信息事件不改变任务最终状态；失败语义与前版本一致；
- 回退矩阵清晰（禁用 Summary / 移除覆盖函数 / 停用事件总线）。
回退策略：
- 删除 Summary emit → 依赖独立 applied 事件；
- 或直接跳过策略解析，回到全局配置行为。
风险与缓解：
- 风险：事件洪水（多策略）→ 通过结构化事件 + Summary 聚合降低噪声；
- 风险：前端排序波动 → 发送顺序测试锁定，并可按 timestamp 排序兜底。

## 3. 实现说明（按阶段）

### P2.1b Commit 实现说明
本节记录 git_commit 的落地细节、测试矩阵与与 P2.1a 复用点，作为后续 branch/checkout 等命令的模板，保持“最小必要进度事件 + 标准错误分类”原则。

#### 1. 代码落点与结构
- 模块文件：`src-tauri/src/core/git/default_impl/commit.rs`
- 任务接入：`spawn_git_commit_task`（`core/tasks/registry.rs`）
- 枚举：`TaskKind::GitCommit { dest, message, allow_empty, author_name, author_email }`
- Tauri 命令：`git_commit(dest, message, allow_empty?, author_name?, author_email?)`
- 前端：API `startGitCommit` (`src/api/tasks.ts`)；Store 扩展 TaskKind；UI `GitPanel.vue` 提交卡片；测试 `views/__tests__/git-panel.test.ts`。

#### 2. 行为流程
1) 取消检查 → 仓库存在（含 .git） → 消息 trim 非空 → 空提交判定 → 作者签名组装 → 执行 commit → 发最终 progress。
2) 空提交判定：写 tree；有 HEAD 则比较 tree id；无 HEAD 则 index 为空即“无变更”。
3) 作者：显式覆盖需 name+email 均非空，否则 Protocol；未提供使用 `repo.signature()`。
4) 进度事件：仅一条（phase=Committed, percent=100）。

#### 3. 取消策略
- 入口、写 tree 前、commit 前多点检查；任务预取消（启动前 token 已取消）直接进入 Cancel 状态；保证不产生部分提交对象。

#### 4. 错误分类
| 场景 | 分类 |
|------|------|
| 非仓库 / 空消息 / 空提交被拒 / 作者字段缺失 | Protocol |
| 用户取消 | Cancel |
| 底层 git2 / I/O 失败 | Internal |

#### 5. 测试矩阵（后端）
成功提交 / 二次无变更拒绝 / allowEmpty 空提交成功 / 首次仓库空提交拒绝与允许 / 自定义作者 / 作者缺失或空字符串失败 / 空白消息失败 / 消息裁剪 / 原子标志取消 / 任务注册预取消。

#### 6. 前端测试
交互触发提交后捕获完成事件；无新增解析分支（沿用既有事件管道）。

#### 7. 安全
事件不包含绝对路径或邮箱；日志使用标准 tracing；消息未做敏感过滤（由调用方控制输入）。

#### 8. 性能
操作本地对象极短；无需多 progress；后续大索引统计可扩展对象与字节指标。

#### 9. 回退
移除命令导出或屏蔽 TaskKind；测试 `#[ignore]`；UI 卡片可条件隐藏。

#### 10. 复用
空提交检测 / 作者校验逻辑供 tag annotated 与未来 amend 复用；错误分类模式与 init/add 一致减少前端分支。

#### 11. 已知限制
不支持 amend、多父提交、GPG 签名、消息规范（Conventional Commit）校验；后续阶段按需添加。

### P2.1c Branch 与 Checkout 实现说明
记录 git_branch / git_checkout 落地细节、命名校验两轮增强、测试矩阵与回退指引。

#### 1. 代码落点与结构
- 模块：`core/git/default_impl/branch.rs`、`checkout.rs`
- Registry：`spawn_git_branch_task` / `spawn_git_checkout_task`
- 枚举：`TaskKind::GitBranch { dest, name, checkout, force }`、`TaskKind::GitCheckout { dest, ref_name, create }`
- 命令：`git_branch(dest,name,checkout?,force?)`、`git_checkout(dest,ref,create?)`
- 前端：API `startGitBranch` / `startGitCheckout`；Store 扩展；测试 `git_branch_checkout.rs`。

#### 2. 行为语义
branch：需已有提交；已存在分支 force=false ⇒ Protocol；force=true 覆盖引用；checkout=true 则创建后立即切换。
checkout：存在则切换；不存在且 create=true 且有提交则创建+切换；其它为 Protocol。

#### 3. 分支名校验
`validate_branch_name`：v1.7 基础非法字符与控制字符；v1.8 增强（前导/尾部/双斜杠/.lock/特殊符号/`@{` 等）；全部非法统一 Protocol。

#### 4. 取消策略
关键副作用（创建引用、force 更新、set_head、checkout_head）前检查；入口预取消直接 Cancel；确保无半完成状态。

#### 5. 错误分类
| 场景 | 分类 |
|------|------|
| 已存在且未 force / 不存在且未 create / 无提交创建或 force / 名称非法 | Protocol |
| 用户取消 | Cancel |
| 引用写入 / checkout 失败 | Internal |

#### 6. 进度与事件
单一 progress：`Branched` / `BranchedAndCheckedOut` / `CheckedOut` / `CreatedAndCheckedOut`；标准 state 流；无额外信息事件。

#### 7. 测试矩阵
创建 / 创建+checkout / 已存在冲突 / force 更新 / force 无提交 / 无提交创建失败 / checkout 成功 / checkout 不存在 / checkout create 成功 / create 无提交失败 / create 已存在幂等 / 取消多断点 / 非法名称集合 / 合法名称集合 / 控制字符与 `@{`。

#### 8. 安全
不暴露绝对路径；错误消息抽象；多点取消减少部分状态。

#### 9. 回退
屏蔽命令或移除 TaskKind；测试忽略；可回退仅 v1.8 校验保持 v1.7。

#### 10. 复用
命名校验逻辑可供 tag/remote；进度 phase 命名模板可复用；副作用前取消模式可移植。

#### 11. 已知限制
未实现 upstream 追踪、任意 commit/tag 检出、删除分支、Unicode 进一步规范、全部 refspec 规则。

### P2.1d Tag 与 Remote 实现说明
详述 git_tag 与 git_remote_{add,set,remove} 的实现、测试与回退策略。

#### 1. 代码落点
- 模块：`core/git/default_impl/tag.rs`、`core/git/default_impl/remote.rs`
- Registry：`spawn_git_tag_task` / `spawn_git_remote_{add,set,remove}_task`
- 枚举：`GitTag | GitRemoteAdd | GitRemoteSet | GitRemoteRemove`
- 前端：`api/tasks.ts` + `stores/tasks.ts` 展示 phase（通用任务面板）。

#### 2. Tag 行为
- 轻量与附注：`annotated` 控制；附注需非空 message。
- force：轻量直接更新引用；附注创建新 tag 对象（内容完全相同 OID 不变）。
- Phase：`Tagged` / `AnnotatedTagged`；force 覆盖：`Retagged` / `AnnotatedRetagged`。
- 消息规范化：CRLF / CR → LF；裁剪尾部多余空行与空白；内部空行保留。
- 校验：`validate_tag_name`；需存在 HEAD commit；附注消息非空。

#### 3. Remote 行为
- add：不存在则创建（phase=RemoteAdded），存在返回 Protocol。
- set：存在则更新 URL（同 URL 幂等成功 phase=RemoteSet），不存在 Protocol。
- remove：存在删除（phase=RemoteRemoved），不存在 Protocol。
- URL 校验顺序：含空白立即拒绝 → trim 结果非空 → 允许 http/https、scp-like、本地无空格路径 → 其它 Protocol。
- 命名：`validate_remote_name`（与分支/标签命名规则基线复用部分类似策略）。

#### 4. 取消策略
入口、解析 HEAD、写引用/创建对象前多次检查，保证无半写引用或悬挂 tag 对象。

#### 5. 错误分类
| 场景 | 分类 |
|------|------|
| 非仓库 / 无提交 (tag) / 名称非法 / tag 已存在且非 force / 附注缺消息 / URL 含空白或非法 / add 重复 / set/remove 不存在 / 空白 URL | Protocol |
| 用户取消 | Cancel |
| 写引用 / 创建 tag 对象 / 设置远程 失败 | Internal |

#### 6. 测试矩阵
Tag：轻量首次 / 附注首次 / 重复非 force 拒绝 / force OID 不变与变化 / 缺消息失败 / 非法名 / 无提交失败 / 取消 / CRLF 规范化 / 尾部空行折叠。
Remote：add/set/remove 成功链路 / add 重复 / set 不存在 / remove 不存在 / set 幂等 / 取消 / 含空格换行或制表符 URL 拒绝 / 本地路径成功 / 空白 URL 拒绝。

#### 7. 差异化与可观测性
区分 Retagged / AnnotatedRetagged 提升可观测粒度；URL 校验在 trim 前进行防绕过；附注同内容 force 保持 OID 不变（测试锁定）。

#### 8. 取消与原子性
多取消断点 + git2 原子更新确保失败不留下半引用；远程操作成功/失败全或无。

#### 9. 回退
屏蔽相关 TaskKind 或命令导出；忽略/删除 `git_tag_remote*.rs` 测试；可选择性保留命名校验辅助供后续引用。

#### 10. 复用
错误分类 / 取消模板与 init/add/commit/branch 一致；Phase 过去式命名为后续扩展（如 delete）提供模式；消息规范化策略可复用至未来注释对象。

#### 11. 已知限制
未实现 upstream 远端跟踪设置；未支持检出任意 tag 直接工作副本；未实现标签删除；未做 Unicode 归一化；命名校验未完全复刻所有 refspec 规则。

### P2.3b HTTP 策略覆盖实现说明
聚焦任务级 HTTP 策略覆盖（followRedirects / maxRedirects）双阶段实现：第一阶段仅解析+合并并记录日志，第二阶段增加 changed 判定与结构化事件。仅影响单个 clone/fetch/push 任务，不修改全局配置。

#### 1. 代码落点
- 解析与应用：`core/tasks/registry.rs::apply_http_override(kind, id, global_http, override_http)`
- 调用：`spawn_git_{clone,fetch,push}_task*_with_opts` 在参数解析和 shallow/partial 判定之后、真正 git 操作之前
- 事件发射：任务文件中按 changed 判定调用 `publish_global(Event::Strategy(StrategyEvent::HttpApplied { ... }))`

#### 2. 覆盖字段与约束
- 允许字段：`followRedirects: bool`，`maxRedirects: u8 (≤20)`
- 未出现字段保持全局基线；未知字段忽略并记录 info 日志（不失败）
- `maxRedirects` 超过 20 或类型不符 → 解析阶段 `Protocol` 失败（不进入合并）
- 失败不写入任何任务局部 HTTP 状态，直接终止任务启动流程

#### 3. 合并与 changed 逻辑
1. 基线拷贝：`let mut eff = global_http.clone()`（未来可替换为运行时配置）
2. 若提供 followRedirects 且与 eff 不同：更新并 `changed=true`
3. 若提供 maxRedirects 且与 eff 不同：clamp(≤20) 后更新并 `changed=true`
4. 返回 `(eff.follow, eff.max_redirects, changed)`
5. 仅当 changed 为 true 发出事件；未变不发（满足幂等与低噪声）

#### 4. 事件语义
- 通道：结构化事件总线（`StrategyEvent::HttpApplied`），不再复用 `task://error`
- payload：`follow` / `max_redirects` 仅包含最终覆盖后的值，便于前端直读
- 冲突：`StrategyEvent::Conflict { kind:"http", message }`（仅 Clone 发射；Push/Fetch 暂未下发结构化冲突事件）
- Push 兼容旧 UI：在冲突时额外通过 `task://error` 通道发信息级提示，其它任务不再发送 legacy code
- 单任务生命周期 `HttpApplied` 最多 1 次（按 changed 判定）

#### 5. 幂等与安全
- 解析与应用在任务 spawn 期间执行一次；后续重试（内部网络重试）不再重新计算覆盖
- 解析失败直接终止任务，无部分覆盖状态残留
- 不写全局静态配置，任务结束即失效，避免串扰并发任务

#### 6. 测试矩阵（核心用例）
- follow 与 max 同时改变 → 发事件
- 仅 follow 改变 → 发事件；仅 max 改变 → 发事件
- 覆盖值与默认完全相同 → 不发事件
- 同任务多次内部重试（模拟） → 事件只出现一次
- maxRedirects 越界（>20）→ Protocol 失败不发事件
- fetch / push 任务各自触发 follow 或 max 改变路径

#### 7. 回退策略
- 仅移除 `if changed { emit ... }` 代码块 → 回到“仅解析+日志”模式
- 或整体删除 `apply_http_override` 调用与测试文件（其余任务逻辑不受影响）

#### 8. 复用与扩展
- changed 判定/结构化事件模式在后续 Retry 扩展中复用（`PolicyEvent::RetryApplied` 保持同源 `applied_codes` 命名）。
- 单点应用函数便于组合 Summary 聚合事件（含 `applied_codes` 与最终 HTTP/Retry 值）。

#### 9. 已知限制
- 未真正驱动底层 HTTP 客户端（当前 redirect 行为保持基线）
- GitFetch 解析到冲突仅在日志层记录，暂未发射 `Conflict` 事件
- 仅限两个字段；其它策略（Retry/TLS）在后续阶段实现

#### 10. 示例事件
```
Event::Strategy(StrategyEvent::HttpApplied { id:"...", follow:false, max_redirects:0 })
Event::Strategy(StrategyEvent::Conflict { id:"...", kind:"http", message:"followRedirects=false => force maxRedirects=0 (was 3)" })
Event::Strategy(StrategyEvent::Summary { http_follow:false, http_max:0, applied_codes:["http_strategy_override_applied"], .. })
```

#### 11. Changelog 建议
Added: per-task HTTP strategy override (followRedirects/maxRedirects) with structured events `StrategyEvent::HttpApplied` / `StrategyEvent::Conflict` / `StrategyEvent::Summary`。

### P2.3c Retry 策略覆盖实现说明
为 clone/fetch/push 任务引入局部 Retry 参数覆盖（max/baseMs/factor/jitter），不写回全局配置；仅在任一值相对全局默认发生变更时通过结构化事件与 Summary `applied_codes` 暴露差异。

#### 1. 代码落点
- 应用函数：`core/tasks/registry.rs::apply_retry_override(global_retry, override_retry)` → `(RetryPlan, changed)`
- 调用：`spawn_git_{clone,fetch,push}_task_with_opts` 解析 strategyOverride 后、HTTP 覆盖之后
- 事件：Clone/Push 在 `changed` 时调用 `publish_global(Event::Policy(PolicyEvent::RetryApplied { id, code:"retry_strategy_override_applied", changed }))`；Fetch 仅依赖最终的 `StrategyEvent::Summary.applied_codes`

#### 2. 合并逻辑
1. 基线拷贝：`let mut plan = global_retry.clone().into()`（保持与运行时配置同步）
2. 逐字段（max/baseMs/factor/jitter）若提供且不同 → 覆盖并 `changed=true`
3. 返回覆盖后 RetryPlan 与 changed；未变不发结构化事件（但 Summary 仍会包含空 `applied_codes`）

#### 3. 约束与校验
- 数值合法性（`max 1..=20`, `baseMs 10..60000`, `factor 0.5..=10.0`）在解析阶段完成；解析失败直接 Protocol 终止
- 覆盖仅影响当前任务内部 backoff 计算；不影响其它并发任务
- Push 仍保持进入上传阶段后不再自动重试的既有语义

#### 4. 幂等与可观测性
- 单任务仅在 spawn 阶段判定一次；后续 attempt 不重复计算
- Clone/Push 的结构化事件通过全局事件总线发送；Fetch 仅在 `Summary.applied_codes` 中体现差异
- Summary 聚合 `retry_*` 数值，便于前端一次性展示最终计划

#### 5. 测试要点
- 覆盖值改变 → Clone/Push 收到一次 `PolicyEvent::RetryApplied`；Fetch 的 Summary `applied_codes` 包含 `"retry_strategy_override_applied"`
- 值与默认相同 → 无结构化事件，Summary `applied_codes` 为空
- http+retry 组合任务：各自事件最多一次，顺序按 HTTP → Retry → Summary
- 越界或无效值（max=0 等）→ 解析阶段 Protocol 失败，无事件
- Backoff 边界（factor=0.5 / 10.0）`changed` 集合包含相应字段

#### 6. 回退策略
- 删除结构化事件发射：逻辑仍覆盖但仅在 Summary 中体现
- 删除 `apply_retry_override` 调用：完全回到全局计划

#### 7. 已知限制
- 未引入动态运行时配置加载；默认值差异无法测试
- 中文本地化网络错误分类可能导致少量 retryable 场景被视为 Internal（不影响 override 事件）
- 未对 jitter=true 的统计分布在任务级重复验证（核心在单元测试覆盖）

#### 8. 示例事件
```
Event::Policy(PolicyEvent::RetryApplied { id:"...", code:"retry_strategy_override_applied", changed:["max","baseMs"] })
Event::Strategy(StrategyEvent::Summary { retry_max:3, retry_base_ms:500, applied_codes:["retry_strategy_override_applied"], .. })
```

#### 9. Changelog 建议
Added: per-task Retry strategy override (max/baseMs/factor/jitter) with structured events `PolicyEvent::RetryApplied` + Summary `applied_codes`。

### P2.3d TLS 策略覆盖实现说明
本阶段未实现任何任务级 TLS 覆盖逻辑：`apply_tls_override` 等函数从未落地，相关事件/测试也未编写。最新实现沿用全局配置的强制 Real-Host 验证与 SPKI Pin 支持，不存在临时放宽路径，Changelog 亦无需记录 TLS 覆盖条目。

### P2.3e 策略覆盖护栏实现说明（忽略字段 + 冲突规范化）
提供两类非阻断护栏：1) 未知字段收集并一次性提示；2) 冲突组合规范化并发冲突事件，确保任务级策略覆盖透明且自洽。

#### 1. 功能范围
- 任务：Clone / Fetch / Push
- 作用对象：`strategyOverride` 顶层 + 子对象 http / retry
- 新增结构化事件：`StrategyEvent::IgnoredFields`（列出顶层与嵌套未知字段）、`StrategyEvent::Conflict { kind:"http", ... }`；`applied_codes` 继续通过 `StrategyEvent::Summary` 聚合

#### 2. 逻辑
1. 解析阶段：收集未知顶层键与子节未知键（记录为 `section.key`）。
2. 冲突检测：
  - HTTP：followRedirects=false 且 maxRedirects>0 → 规范化 maxRedirects=0
3. 规范化后继续后续覆盖；Clone 在冲突时发出 `StrategyEvent::Conflict`，Push 仅写入 legacy `task://error` 信息事件，Fetch 当前仅记录日志。
4. 忽略字段：若集合非空，生成一次 `StrategyEvent::IgnoredFields` 事件，列出 top 与 sections。
5. changed 判定：基于规范化后最终值与全局比较（导致“规范化回到默认”时仅发 conflict，不发 applied）。

#### 3. 事件顺序（单任务）
applied(HTTP→Retry) → conflict(仅 Clone 结构化；Push legacy 提示) → ignored（若有） → summary

#### 4. 代码落点
- 解析返回结构扩展：`StrategyOverrideParseResult { parsed, ignored_top_level, ignored_nested }`
- 应用函数扩展返回 conflict 描述（Option<String>）
- Spawn 流程：解析 → HTTP 应用 → Retry 应用 → emit applied* → emit conflict* → emit ignored

#### 5. 测试要点
- 忽略字段：含/不含未知键事件出现 0/1 次
- HTTP 冲突：follow=false + max>0 → Clone 发结构化事件并将 max 归零；Push 仅发 legacy `task://error` 提示；Fetch 仅规范化
- 组合：HTTP 冲突 + 忽略字段并存（计数精确）
- 无冲突合法路径：仅必要 applied 事件
- 单元：changed / conflict / 忽略各分支

#### 6. 回退策略
- 仅关冲突事件：移除 conflict emit（保留规范化）
- 关规范化：移除规则逻辑（可能传播矛盾组合）
- 仅关忽略字段事件：移除 ignored emit（日志仍可见）
- 全回退：移除扩展字段与 emit 分支，函数签名恢复

#### 7. 安全与限制
- 事件不含敏感数据，仅字段名与少量布尔/数字
- 规则为硬编码列表；新增策略字段需扩展规则表与测试
- 未跨域检测 Retry 与其他策略组合

#### 8. Changelog 建议
Added: per-task strategy override guard (ignored fields + conflict normalization) with structured events `StrategyEvent::IgnoredFields` & `StrategyEvent::Conflict`.

### P2.3f 策略覆盖前端与文档支持实现说明
为策略覆盖闭环补齐前端透传、事件存储、示例文档与回退矩阵，确保调用方可稳定使用 HTTP/Retry + 护栏全套能力（TLS 覆盖已移除）。

#### 1. 范围
- 前端 API：`startGitClone/Fetch/Push` 支持 `strategyOverride`（与 depth/filter 并存）
- 公共类型：`StrategyOverride`（仅 http|retry 子对象）
- Store：错误事件存储新增 code 保留；信息型策略事件不覆盖已有 retriedTimes
- 测试：事件顺序、组合、兼容旧 fetch 签名、参数排列、retriedTimes 保留
- 文档：README + 设计文档事件代码表 & 示例更新

#### 2. 事件矩阵（最终）
`StrategyEvent::HttpApplied` / `PolicyEvent::RetryApplied` / `StrategyEvent::Conflict` / `StrategyEvent::IgnoredFields`（顺序：applied* → conflict → ignored → summary；`StrategyEvent::Conflict` 当前仅由 GitClone 发射，Push 仅保留 legacy `task://error` 提示）；字符串 `applied_codes` 保留旧 code（例如 `http_strategy_override_applied`、`retry_strategy_override_applied`）供前端聚合展示。

#### 3. retriedTimes 语义
信息事件缺少 retriedTimes 不清零；仅更大值提升，保持重试进度可观测连续性

#### 4. 兼容性
- 旧 `startGitFetch(repo, remote)` 签名仍接受；对象式新签名分支自动判定
- 空 `{}` override 与缺省语义等价（不触发任何策略事件）
- credentials / depth+filter 与 override 任意组合已测试

#### 5. 回退矩阵
| 目标 | 操作 | 影响 |
|------|------|------|
| 关闭信息事件 | 移除 emit 分支 | 覆盖仍生效 | 
| 关闭单策略 | 移除对应 apply_* 调用 | 其它不受影响 |
| 关闭冲突规范化 | 移除规则 | 可能传播矛盾值 |
| 关闭忽略字段事件 | 移除 ignored emit | 日志仍可定位 |
| 全量回退 | 移除解析与全部 apply | 恢复仅全局配置 |

#### 6. 测试要点（增量）
- 顺序：applied→conflict→ignored（无逆序）
- 幂等：单任务每类 applied ≤1，conflict ≤规则数，ignored ≤1
- retriedTimes 保留：信息事件不降低数值
- 旧 fetch 签名兼容：与新签名结果一致

#### 7. 安全
事件不含敏感凭证；策略字段仅布尔与数字

#### 8. 已知限制
- 新增策略字段需同步扩展前端类型与事件代码表
- 未提供统一单条 diff 以替换多事件（summary 另述）

#### 9. Changelog 建议
Docs: added StrategyOverride usage examples and event code reference; Frontend: support per-task strategy overrides with stored codes and retry progress preservation.

#### 10. 结论
前端与文档支持完成，策略覆盖闭环；后续新增策略按统一模板迭代即可。

#### 9. 结论
护栏提升策略可观测性与安全，零中断、幂等、低成本回退，为后续策略扩展提供模板。
