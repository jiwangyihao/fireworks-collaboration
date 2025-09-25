# 测试重构实现与维护对接文档 (Implementation Guide)

> 适用读者：核心开发、测试维护、后续功能/性能/事件模型演进贡献者。
> 配套文件：`doc/TESTS_REFACTOR_PLAN.md`（路线图 & 修订记录）
> 当前状态：全部聚合 + 占位清理完成（至 v1.18），结构稳定，可进入“增量特性/低成本扩展”阶段。

---
## 目录
1. 高层概览与成果摘要  
2. 最终目录结构与文件角色  
3. 聚合与命名/分区原则  
4. 公共模块与 DSL 设计演进  
5. 属性测试集中策略与种子管理  
6. 事件断言 DSL 演进路线  
7. 各聚合文件实现摘要（Git / Events / Quality / Tasks / E2E）  
8. 指标基线与监测方法  
9. 新增 / 修改测试操作指南 (How-To)  
10. 技术债与后续优化机会  
11. 风险矩阵与持续缓解  
12. 快速对照速查表  
13. 附录 A：关键词/集中度采集脚本示例  
14. 附录 B：回归种子 (proptest) 维护 SOP  
15. 附录 C：事件 Tag DSL 语义参考  

---
## 1. 高层概览与成果摘要
| 目标维度 | 初始痛点 | 改造后状态 | 度量结果 |
|----------|----------|------------|----------|
| 文件碎片 | 50+ 主题重叠零散 .rs | 14 个聚合主题 + 1 种子文件 | 业务类测试文件数 ≤20 目标满足 |
| 重复逻辑 | init/clone/fetch/retry/partial/shallow 多处复制 | fixtures + matrices + DSL 统一 | 估算重复剪裁 ≥70%（git_impl 系列≥90%）|
| 事件断言 | 大量硬编码全序列、脆弱 | 子序列 + Tag DSL + 结构化桥接占位 | 误失败面收敛（后续 0 观测）|
| 属性测试 | 5 个分散 prop_* 文件 | 1 文件集中 + 回归种子保留 | 集中度 100% |
| 新增模块数 | 无控制风险 | 新增 2 个聚合（tag/remote，task_registry_and_service） | ≤2 约束达成 |
| 行数控制 | 多文件 >800 行或重复 | 聚合文件 <800 行（监控阈值） | 阈值内 |

结论：结构已稳定；后续增量以“扩展现有文件 section”优先，不再新增顶层聚合模块。

---
## 2. 最终目录结构与文件角色
核心根：`src-tauri/tests`

```
common/        <-- 公共构造 & DSL & 矩阵
  event_assert.rs
  fixtures.rs
  git_helpers.rs
  git_scenarios.rs
  http_override_stub.rs
  partial_filter_matrix.rs
  partial_filter_support.rs
  pipeline.rs
  repo_factory.rs
  retry_matrix.rs
  shallow_matrix.rs
  test_env.rs

git/           <-- Git 语义分层聚合 (init/add/branch/clone/fetch/push/strategy/...)
  git_init_and_repo_structure.rs
  git_add_and_commit.rs
  git_branch_and_checkout.rs
  git_clone_core.rs
  git_clone_shallow_and_depth.rs
  git_clone_partial_filter.rs
  git_fetch_core_and_shallow.rs
  git_fetch_partial_filter.rs
  git_push_and_retry.rs
  git_strategy_and_override.rs
  git_preconditions_and_cancel.rs
  git_tag_and_remote.rs

events/        <-- 事件结构与生命周期
  events_structure_and_contract.rs
  events_task_lifecycle_git.rs

quality/       <-- 错误 & i18n & 属性测试 sections
  error_and_i18n.rs

tasks/         <-- 任务/注册表/服务交互
  task_registry_and_service.rs

e2e/
  e2e_public_git.rs

prop_tls_override.proptest-regressions  <-- 回归种子 (proptest)
```

文件角色速览（详见第 7 节）：
- Git 聚合：主题+section 内部语义分块，限制行数，集中同类参数组合逻辑。
- common：纯逻辑复用层。禁止放置具体业务断言（除事件 DSL）。
- quality：错误、国际化、属性测试集中 & 回归 seed 再利用。
- events：结构契约 + 生命周期（与 DSL 协作）。
- tasks：面向内部任务系统与 git service 交集行为。
- e2e：端到端流水线（保持最小覆盖、避免与组合测试重复）。

---
## 3. 聚合与命名/分区原则
1. 命名模式：`git_*`, `events_*`, `error_* | *_and_*`, 保持可读语义 + 一致前缀便于过滤。  
2. 行数阈值：单文件 < 800 行；超过 700 行需评估拆分或进一步抽象 helper。  
3. Section 组织：文件内使用内部 `mod section_xxx` 分块；命名从业务语义（basic/edge/matrix/fallback/deepen/capability）。  
4. 新增模块限制：后重构阶段新增顶层聚合文件需满足 (a) 现有文件逼近阈值 & 难以再抽象, (b) 与现有主题低重合度。默认优先扩展现有 sections。  
5. 参数矩阵策略：只为高重复维度建立显式枚举（ShallowCase / PartialFilterCase / RetryCase），不做组合笛卡尔积生成。  
6. 重复消除优先级：抽象 → 参数化 → 剪裁冗余 case；禁止“复制改变量”式新增。  
7. 回归保留策略：仅保留必需的 proptest 种子文件；无业务逻辑测试存根。  

---
## 4. 公共模块与 DSL 设计演进
| 模块 | 功能 | 设计要点 | 演进阶段 |
|------|------|----------|----------|
| test_env.rs | 全局一次性初始化 (日志/环境变量) | 通过 Once 防抖；聚合文件首行调用 | 12.1 引入，后续稳定 |
| fixtures.rs | 轻量仓库构造 & 文件操作 | 语义函数：create_temp_repo / write_file / commit | 12.x 渐进补全 |
| repo_factory.rs | 复杂结构仓库（多分支/标签） | 明确输出描述（branches/tags 列表） | 12.3 起 |
| git_helpers.rs | Git 操作辅助 / 错误分类 | 隔离 from prod 的最小桥接；保证测试侧错误枚举稳定 | 12.1+ |
| git_scenarios.rs | 场景级复合操作 (clone/push/fetch) | 返回 Outcome 结构 + 收集事件 | 12.4+ |
| shallow_matrix.rs | 浅克隆/加深 case | 显式枚举 + Display | 12.5 |
| partial_filter_matrix.rs | partial filter case (Clone/Fetch) | Op + FilterType + depth | 12.6/12.8 |
| partial_filter_support.rs | capability / fallback 判定 | SupportLevel 枚举 (Supported/Unsupported/Invalid) | 12.6+ 扩展 |
| retry_matrix.rs | 重试策略 | attempts/backoff 形状（不包含真实时间） | 12.9 |
| http_override_stub.rs | HTTP override 组装 / 变体注入 | 统一 case -> expected events subset | 12.10 |
| pipeline.rs | E2E 流水线 orchestrator | PipelineSpec + run_pipeline_with | 12.15 |
| event_assert.rs | 序列 / Tag DSL / 结构化辅助 | expect_subsequence / expect_tags_subsequence / structured helpers | 多阶段迭代 (12.9~12.12 合并 support) |

演进原则：
- 仅当 ≥2 聚合文件出现重复语义时抽到 common。
- Helper 输出使用结构体/枚举，避免返回多元组不透明值。
- 断言 helper 的 panic 信息统一包含：锚点标签 + index + 摘要。 

---
## 5. 属性测试集中策略与种子管理
1. 集中落点：`quality/error_and_i18n.rs` 四个 sections (strategy_props / retry_props / partial_filter_props / tls_props)。  
2. 目标：在不引入笛卡尔积爆炸的前提下覆盖参数边界 & 退化路径；倾向生成与手工 matrix 混合。  
3. 回归种子：`prop_tls_override.proptest-regressions` 保持与计划文件同级；header 说明用途/清理条件。  
4. 种子维护 SOP（详见附录 B）：
   - 出现 CI flake 且 proptest 提供最小化案例 → 自动 append（不覆盖历史）
   - 稳定 ≥30 天未触发相同路径，评估裁剪旧 seed（保留最后 3 条）
   - 修改生成器逻辑需：运行一次无 seed 模式确认无新 panic，再回填 seed 适配
5. 属性测试约束：
   - 单属性运行时间 < 2s；避免深度 I/O 真实网络
   - 断言失败输出必须含输入摘要（Display 实现）

---
## 6. 事件断言 DSL 演进路线
阶段演进：
1. 初始：硬编码 `assert!(events[i].contains("xxx"))` → 频繁脆弱
2. Tag 序列：`tagify(events) -> Vec<String>` + `expect_tags_subsequence(["task:start","push:op:Push",...])`
3. 结构化辅助：对关键事件类型 (Policy/Strategy/Transport/TaskLifecycle) 解析 -> 聚合 snapshot / selective 字段断言
4. 支撑函数（现状 `event_assert.rs`）：
   - `expect_subsequence(lines, subset)` 宽松文本子序列
   - `expect_tags_subsequence(tags, expected)` 锚点化
   - `snapshot_events(events_json_like)` 可选结构校验 (id 唯一性)
   - 针对策略/TLS/partial filter 的特化 asserts（复用枚举分类）
5. 下一步潜在升级（技术债节列出）：引入统一结构化枚举 → 强类型匹配 → JSON snapshot 按字段白名单。

---
## 7. 各聚合文件实现摘要
(每条含：目的 / 主要 sections / 关键断言 / 潜在精简机会)

### git_init_and_repo_structure.rs
- 目的：仓库基础 init / layout / preflight
- Sections: basic_init / repo_layout / preflight
- 关键断言：HEAD/refs 结构一致；空仓约束
- 精简机会：HEAD 读取逻辑与其它文件合并（若重复增多）

### git_add_and_commit.rs
- 目的：暂存/提交生命周期
- Sections: add_basic / add_edge / commit_basic / commit_edge / task_wrapper
- 关键断言：index 内容，空提交拒绝
- 精简机会：合并重复 modify+stage 流程 → helper pipeline（影响小暂缓）

### git_branch_and_checkout.rs
- 目的：分支创建/删除/checkout/dirty 状态
- Sections: branch_create / branch_delete / checkout_basic / checkout_dirty / checkout_detached
- 关键断言：current_branch / HEAD 指向
- 精简机会：dirty 构造统一封装

### git_clone_core.rs
- 目的：Clone 参数合法性 + 基础行为
- Sections: params_validation / params_matrix / preflight / behavior_basic
- 关键断言：错误分类；矩阵代表性覆盖
- 精简机会：预检与 shallow 部分 helper 合并

### git_clone_shallow_and_depth.rs
- 目的：浅克隆 + deepen / invalid / ignore / file url
- Sections: basic_shallow / invalid_depth / deepen / local_ignore / file_url
- 关键断言：.git/shallow 内容/行数变化
- 精简机会：以对象计数替代浅文件行宽松校验（待实现）

### git_clone_partial_filter.rs
- 目的：Partial clone filter (capability / variants / depth / fallback)
- Sections: capability / filter_event / filter_depth / fallback
- 关键断言：SupportLevel 分类 + 事件锚点
- 精简机会：与 fetch partial 共享 case 输出展示

### git_fetch_core_and_shallow.rs
- 目的：Fetch 基础 + shallow fetch deepen/invalid/ignore
- Sections: fetch_basic / fetch_shallow / fetch_deepen / fetch_invalid / fetch_ignore
- 关键断言：updated refs / shallow state 转换
- 精简机会：与 clone deepen 共用逻辑抽象提升

### git_fetch_partial_filter.rs
- 目的：Fetch partial filter 变体 + fallback
- Sections: capability / filter_variants / filter_depth / fallback
- 关键断言：SupportLevel 区分 invalid vs unsupported
- 精简机会：事件子序列模板化

### git_push_and_retry.rs
- 目的：Push 行为 + retry/backoff 策略
- Sections: push_basic / push_conflict / retry_policy / retry_event
- 关键断言：attempt 序列 + 成功/冲突路径互斥
- 精简机会：backoff 序列断言进一步抽象（currently minimal）

### git_strategy_and_override.rs
- 目的：Strategy 组合 + HTTP override 变体 + Summary/TLS gating
- Sections: http_basic / http_limits / http_events / strategy_summary_multiop / override_no_conflict / override_empty_unknown / override_invalid_inputs / tls_mixed_scenarios / summary_gating
- 关键断言：策略应用标记、override 数量限制、Summary/TLS gating 事件
- 精简机会：策略与 HTTP case 合并成统一枚举（若后续增加策略）

### git_preconditions_and_cancel.rs
- 目的：前置校验失败/取消/超时/传输层 fallback
- Sections: preconditions / cancellation / timeout / transport_fallback / transport_timing
- 关键断言：Outcome kind 互斥；Fallback stage 顺序；TimingRecorder 幂等
- 精简机会：mock clock 接入（planned）

### git_tag_and_remote.rs
- 目的：标签/远端引用管理 & refname 验证
- Sections: tag_basic / tag_edge / remote_listing / refname_validation
- 关键断言：符合命名规则；引用更新事件
- 精简机会：refname pattern 表达式集中 `git_helpers`

### events_structure_and_contract.rs
- 目的：事件 schema / 序列最小集 / TLS 结构化观测
- Sections: schema_basic / sequence_minimal / legacy_absence / contract_snapshot / adaptive_tls_metrics / tls_fingerprint_log / tls_pin_enforcement
- 关键断言：字段存在性 + id 唯一 + TLS 事件/日志一致性
- 精简机会：字段白名单/忽略机制参数化

### events_task_lifecycle_git.rs
- 目的：Git 任务生命周期成功/失败/取消/指标
- Sections: success_flow / failure_flow / push_flow / metrics
- 关键断言：终态互斥 + 指标字段存在
- 精简机会：进度事件锚点自动推导

### error_and_i18n.rs
- 目的：错误枚举桥接 & i18n + 属性测试集中
- Sections: error_mapping / i18n_locale_basic / i18n_fallback / integration_edge / strategy_props / retry_props / partial_filter_props / tls_props
- 关键断言：locale key 存在性 / fallback / 属性不变量
- 精简机会：属性 sections 拆至独立文件（当前行数未超阈值暂不做）

### task_registry_and_service.rs
- 目的：TaskRegistry 行为 + GitService 边缘场景
- Sections: registry_basic / registry_edge / service_impl_edges
- 关键断言：注册/取消/状态转移一致性
- 精简机会：等待 predicate 统一化（已部分完成）

### e2e_public_git.rs
- 目的：端到端真实（或模拟）流水线
- Sections: scenario_clone_build_push / scenario_read_only / scenario_error_boundary
- 关键断言：提交计数差异 + 关键事件锚点
- 精简机会：与 pipeline.rs 进一步对齐 Outcome 结构

---
## 8. 指标基线与监测方法
| 指标 | 基线定义 | 工具 / 采集方式 | 触发更新条件 |
|------|----------|----------------|--------------|
| 文件行数 | 每聚合文件 LoC | `wc -l`/Rust analyzer | 某文件 >700 行 |
| 关键词频次 | override/tls/retry/... | PowerShell Select-String | 单关键词 ±20% 变化 |
| 属性集中度 | 属性测试仅 1 文件 | 目录扫描 | 增加第二个属性文件 |
| 重复剪裁率 | git_impl 剪裁 ≥90% | Diff/loc 记录 | 再次大规模重构 |
| 事件 DSL 使用率 | Tag DSL 覆盖策略/重试/TLS/partial | 搜索 `expect_tags_subsequence` | 新场景未使用 DSL |
| 种子文件数量 | 1 | 手动 | 新增/删除 seed |

示例关键词统计（PowerShell）：
```
Get-ChildItem src-tauri/tests -Recurse -Include *.rs | \
  Select-String -SimpleMatch 'override' | Measure-Object | % {$_.Count}
```

---
## 9. 新增 / 修改测试操作指南 (How-To)
### 9.1 新增 Git 行为测试
1. 选择对应聚合文件（如 clone 参数 → `git_clone_core.rs`）。
2. 若为现有 section 子类变体：追加 case 到参数枚举；循环中自动断言。
3. 若出现新维度且重复≥3：考虑建立 matrix 枚举（先在当前文件内部定义，≥2 文件复用再上移 common）。
4. 使用 `test_env::init_test_env()`；避免自建全局 logger。

### 9.2 新增属性测试
1. 在 `error_and_i18n.rs` 末尾新增 section：`mod section_<feature>_props`。
2. 生成器：实现 `Strategy` 或构造函数，Display 输出关键字段。
3. 失败后若 proptest 最小化生成新的稳定 panic 用例：写入 seed 文件尾部。
4. 保证运行时间：设置 `prop::test_runner().set_cases(N)` 控制案例数。

### 9.3 更新 / 清理回归种子
1. 评估近期 CI 是否仍出现同栈。
2. 若无：可删除最早种子（保留最近 3 条）。
3. 删除后运行属性测试确认无回归。

### 9.4 新增事件断言
1. 优先 Tag DSL：加入期望锚点最小子序列（不要整个列表）。
2. 需要结构字段：使用结构化 helper（collect_policy / assert_policy_code 等）。
3. 大量新增字段 → 启动结构化枚举提案（见技术债）。

### 9.5 删除冗余用例
1. 判断是否覆盖与其它 case 等价（输出相同 Outcome 类型 + 无额外分支）。
2. 删除后：执行关键词计数；如关键字下降 >20% 需补文档说明或保留代表锚点。

### 9.6 引入新错误 / locale key
1. 更新错误枚举 & 映射（生产侧）。
2. 在 `error_mapping` section 添加断言；locale 每种语言加入 key。
3. 运行质量测试确认 fallback 不变。

### 9.7 调整超时/取消测试
1. 不允许使用真实长 `sleep`；用 mock 时钟或逻辑分支。
2. 终态断言使用互斥 helper；确认无 success 与 fail 并存。

---
## 10. 技术债与后续优化机会
| 项 | 描述 | 价值 | 估计成本 | 优先级 |
|----|------|------|----------|--------|
| 结构化事件枚举 | 利用 serde 解析统一类型，减少字符串 tag 依赖 | 提高稳健性 | 中 | 中 |
| 事件字段白名单 snapshot | 可配置忽略非关键新增字段 | 减少噪音 | 低 | 中低 |
| 对象计数 helper (shallow/fetch) | 替换 shallow 文件行数宽松断言 | 精准度提升 | 中 | 中 |
| mock clock 接入 (timeout) | 去除潜在真实时间依赖 | 稳定/速度 | 中 | 中 |
| 属性生成器抽象层 | 统一常用策略组合 | 降重复 | 中 | 低中 |
| 关键词统计脚本自动化 | 一键输出基线 diff | 过程可追踪 | 低 | 低 |
| DSL 锚点提取工具 | 自动建议最小子序列 | 减编写成本 | 中 | 中 |
| Seed 过期自动提示 | 根据 git blame + 日期额度提醒清理 | 清爽 | 低 | 低 |

---
## 11. 风险矩阵与持续缓解
| 风险 | 现状 | 可能影响 | 缓解策略 |
|------|------|----------|----------|
| 事件 DSL 与未来结构变化 | Tag 仍基于字符串 | 结构调整需批量替换 | 尽快引入枚举 + serde 解析 |
| 属性测试执行时间膨胀 | 目前集中单文件 | CI 变慢 | 建阈值报警（>5s 总耗时） |
| Shallow/Partial 行为扩展 | helper 精度不足 | 断言误判 | 引入对象计数与 capability 检验强化 |
| Seeds 过度累积 | 单文件管理 | 噪音、误导 | 定期清理 SOP (附录 B) |
| 错误枚举漂移 | 枚举扩展未同步测试 | 覆盖缺口 | PR 模板中加入“更新 error_and_i18n?” checklist |
| 多文件行数接近阈值 | strategy/override 聚合最接近 | 可读性下降 | 技术债任务：枚举化策略 case |

---
## 12. 快速对照速查表
| 需求 | 去哪里改 | Helper/枚举 |
|------|----------|--------------|
| 新增 shallow case | `shallow_matrix.rs` | ShallowCase |
| 新增 partial filter fetch 变体 | `partial_filter_matrix.rs` | PartialFilterCase (Op=Fetch) |
| 新增重试策略 | `retry_matrix.rs` | RetryCase / compute_backoff_sequence |
| 覆盖 HTTP override 新限制 | `http_override_stub.rs` & `git_strategy_and_override.rs` | HttpOverrideCase |
| 新增任务取消分支 | `git_preconditions_and_cancel.rs` | 终态互斥 helper |
| 新增生命周期指标 | `events_task_lifecycle_git.rs` | metrics section + Tag DSL |
| 添加 locale | `error_and_i18n.rs` | LOCALE key 表 |
| 调整 E2E 步骤 | `pipeline.rs` + `e2e_public_git.rs` | PipelineSpec |

命名规范：`test_<动作>_<场景>_<期望>`；参数化循环利用 `case.describe()`。

---
## 13. 附录 A：关键词/集中度采集脚本示例
PowerShell 简版：
```
$keywords = 'override','tls','retry','partial_filter','shallow','capability'
$results = @{}
Get-ChildItem src-tauri/tests -Recurse -Include *.rs | ForEach-Object {
  $path = $_.FullName
  $content = Get-Content $path -Raw
  foreach($k in $keywords){
    if(-not $results.ContainsKey($k)){ $results[$k] = 0 }
    $results[$k] += ([regex]::Matches($content, [regex]::Escape($k))).Count
  }
}
$results.GetEnumerator() | Sort-Object Name | Format-Table -AutoSize
```

---
## 14. 附录 B：回归种子 (proptest) 维护 SOP
| 步骤 | 场景 | 动作 |
|------|------|------|
| 1 | CI 出现 proptest 失败并最小化生成 | 复制 minimal case 到 seed 文件末尾 |
| 2 | 种子过期评估 | 查 commit 时间 / 近30天无命中 | 可删除最早 seed |
| 3 | 生成器逻辑调整 | 先移除 seed 临时运行；再重新引入（若仍需） |
| 4 | 种子文件冲突 | 合并时按时间排序（旧→新） | 追加不覆盖 |
| 5 | 大规模生成器重写 | 暂时备份旧 seed 到 issue 记录 | 再清空文件 |

文件头部需要：用途 / 清理条件 / 操作人手册（已存在）。

---
## 15. 附录 C：事件 Tag DSL 语义参考
| Tag 前缀 | 示例 | 语义 |
|----------|------|------|
| task: | task:start / task:end | 任务生命周期锚点 |
| push: | push:op:Push / push:result:Conflict | Push 操作与结果 |
| retry: | retry:attempt:1 | 重试尝试序号 |
| strategy: | strategy:apply:<name> | 策略应用 / 组合 |
| http: | http:override:follow / http:limit:max | HTTP override 行为锚点 |
| tls: | tls:adaptive:rollout / tls:mode:mixed | TLS 策略事件 |
| filter: | filter:partial:event-only | Partial filter 类型 |
| capability: | capability:partial:unsupported | 能力检测结果 |
| shallow: | shallow:deepen / shallow:invalid | 浅克隆/加深/非法场景 |
| precondition: | precondition:fail:<type> | 前置失败分类 |
| cancel: | cancel:requested | 取消流程触发 |
| metric: | metric:throughput:<bucket> | 指标/度量事件 |

最小子序列策略：只包含必要锚点（开始/关键转折/终态），避免耦合中间非关键噪音。

---
## 16. 交付与使用注意
- 本文档与 `TESTS_REFACTOR_PLAN.md` 联合作为未来变更审查基线；新增大规模测试结构变动需同时修订两者。  
- 若引入结构化事件枚举：更新第 6、15 节并增加数据类型说明。  
- 建议在 PR 模板增加 Checklist：
  - [ ] 是否新增了聚合文件？若是，评估行数/重合度。
  - [ ] 是否更新关键词基线（必要时）？
  - [ ] 是否需要新增/清理 seeds？
  - [ ] 是否复用 Tag DSL 而非硬编码字符串？
  - [ ] 是否保持属性测试集中？

Done.
