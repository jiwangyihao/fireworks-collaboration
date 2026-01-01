# 测试重构方案文档 (v1.17)

## 修订记录
| 版本 | 日期 | 说明 | 主要改动 |
|------|------|------|----------|
| v1.0 | 初稿 | 提供总体技术方案与阶段规划骨架 | 模块划分、公共抽象、阶段列表 |
| v1.1 | 2025-09-22 | 精简文档，合并已生成路线图，移除过期“待补充”提示 | 去除冗余过渡语句、加入修订记录 |
| v1.2 | 2025-09-23 | 12.5~12.9 聚合后增量优化：统一 init_test_env、引入 detect_shallow_repo、Fetch partial 使用 Fetch op、retry backoff 形状强化、事件 fetch: 前缀区分 | 新 helper (detect_shallow_repo)；强化 backoff 断言；fetch partial 采用 PartialFilterOp::Fetch；补齐 test_env 调用；微调 header Cross-ref |
| v1.3 | 2025-09-23 | 12.10 前置：引入 GitOp / GitEventKind 占位、http_override_stub 与 override 矩阵、run_push_with_retry 附加 op 事件 | GitOp 枚举；GitEventKind 占位；http_override_stub + http_override_cases；push 事件首项 push:op:Push |
| v1.4 | 2025-09-23 | 12.10 聚合完成：strategy + http override + adaptive TLS 合并；新增 invalid max=0 处理与独立测试；legacy 文件占位（已删除原散文件）；计划 12.11 引入 precondition/cancel DSL | 新聚合文件 git_strategy_and_override.rs；http_override_stub 增强（invalid_max）；新增 section_http_invalid_max；文档状态更新 |
| v1.5 | 2025-09-23 | 进入 12.14 前清理：终态互斥 helper 覆盖 12.9~12.13、扩展标签锚点、所有对应旧散文件统一 legacy 占位、events snapshot 增加 JSON 结构校验 | 新 helper(assert_terminal_exclusive)；tags 覆盖 preconditions/timeout/push/retry/strategy；snapshot 校验 id 唯一；文档表格备注更新 |
| v1.6 | 2025-09-23 | 12.14 聚合：新建 quality/error_and_i18n.rs；迁移 error_i18n_map.rs 用例并占位原文件；预留 locale/fallback/integration 占位分区 | 新聚合文件 error_and_i18n.rs；error_i18n_map.rs -> legacy placeholder；文档路线图 12.14 标记完成 |
| v1.7 | 2025-09-23 | 12.10~12.14 增量完善：聚合文件统一 init_test_env 自动初始化；error_and_i18n 引入 AppErrorKind 桥接、locale key & fallback 测试 | 所有 12.10~12.14 聚合文件添加 #[ctor] init；error_and_i18n: AppErrorKind + bridge 测试 + locale keys/fallback 用例 |
| v1.8 | 2025-09-23 | 12.15 前置脚手架：pipeline DSL / e2e 聚合骨架 / AppErrorKind 扩展占位 (Timeout/Permission) / 结构化事件桥接占位 | 新增 common/pipeline.rs；新增 e2e/e2e_public_git.rs；git_e2e_public.rs -> legacy placeholder；event_assert 增 structured_tags；error_and_i18n 扩展枚举 |
| v1.9 | 2025-09-23 | 12.15 完成（本地真实离线流水线 + 故障注入） | pipeline.rs 引入 PipelineConfig / FaultKind + run_pipeline_with 真实 git 执行（clone/modify/commit/push/fetch）；e2e_public_git.rs 改为真实提交计数与失败注入断言；ForcePushFailure 模拟 push 失败；新增提交计数前后校验；状态从 Scaffolding -> Completed (Refined) |
| v1.10 | 2025-09-23 | 规划 12.16 legacy 清理阶段 | 新增 12.16 重述：legacy_cleanup_and_removal；定义清理目标/步骤/验收；后续提交将执行占位测试与死代码剔除并追加状态 Completed (Cleanup) |
| v1.11 | 2025-09-23 | 12.16 清理执行完成 | 删除全部 legacy 占位测试文件（git_*, events_*, error_i18n_map.rs, git_e2e_public.rs 等 50+ 文件）；测试回归全绿；路线图 12.16 状态更新为 Completed (Cleanup) |
| v1.12 | 2025-09-23 | 12.16 root-level 二次 sweep | 额外删除 root-level 12 个 residual 占位文件（git_adaptive_tls_rollout_event.rs, git_fetch.rs, git_http_override_*.rs 系列 7 个, git_preconditions_and_cancel.rs, git_init.rs, git_local_skeleton.rs）；验证均为仅注释 + assert!(true) 占位；cargo test 全绿；路线图 12.16 备注补充 sweep 2/2 完成 |
| v1.13 | 2025-09-23 | Roadmap Phase 1 规划落地前置（Tag/Remote 聚合准备） | 附录 A/B 草案：限制新增模块≤2；列出 Tag/Remote/Task/Strategy/Props 映射表；定义四阶段指标 |
| v1.14 | 2025-09-23 | Phase 1 Completed：Tag & Remote 聚合 | 新增 `git/git_tag_and_remote.rs`（34 tests, <430 行）；原 `git_tag_remote.rs`, `git_tag_remote_extra.rs`, `refname_validation.rs` 占位；文档加来源与 Metrics；roadmap 标记 Phase 1 Completed |
| v1.15 | 2025-09-23 | Phase 2 Completed：TaskRegistry & GitService 聚合 | 新增 `tasks/task_registry_and_service.rs`（28 tests, ~500 行）；统一 wait_predicate/wait_task_state；迁移并占位 6 个 root-level 任务 / git_tasks 文件；从 `git_impl_tests.rs` 迁出 progress/cancel 6 用例 (~30% 覆盖剪裁起点) | 
| v1.16 | 2025-09-23 | Phase 3 Completed：Strategy Override 聚合（HTTP/Retry）& git_impl 二次剪裁 | 扩展 `git/git_strategy_and_override.rs` 聚焦 HTTP/Retry 覆盖场景，新增 4 个 sections（strategy_summary_multiop / override_no_conflict / override_empty_unknown / override_invalid_inputs）；迁移 12 个 root-level strategy/override 系列文件为占位并移除遗留 TLS override 断言；`git_impl_tests.rs` 进一步剪裁（累计≈70% 重复场景移除，仅保留 service negotiating/cancel/invalid path/fast fetch cancel 特有用例）；聚合文件行数 ~760 (<800 控制线)；新增 Phase3 Metrics 注释块；建立关键词分布基线（override/http/retry/applied_codes/RealHostCertVerifier/cert_fp）。 |
| v1.17 | 2025-09-23 | Phase 4 Completed：属性测试集中 & git_impl 最终剪裁 | 将全部 `prop_*.rs` 属性测试迁移进 `quality/error_and_i18n.rs` 新增 4 个 sections（strategy_props / retry_props / partial_filter_props / tls_props）；原 5 个 prop 源文件占位化（保留 `prop_tls_override.proptest-regressions` seed）；`git_impl_tests.rs` 剩余 3 个 service 独有测试迁往 `tasks/task_registry_and_service.rs` 新 section_service_impl_edges 后占位化（累计剪裁≥90%）；root-level 剩余业务测试文件=0（仅种子文件保留）；属性测试集中度=100%；准备进入后续可选优化（统计脚本 & 事件 DSL 强化）。 |
| v1.18 | 2025-09-23 | 12.16 最终清理：删除全部残余占位测试文件 | 删除 26 个 root-level 占位 `.rs`（全部为“注释+trivial assert”），仅保留 `prop_tls_override.proptest-regressions` 种子；测试回归全绿；完成测试重构收尾。 |

### v1.17 Phase 4 Metrics & Keyword Baseline

本节补充 Phase 4 结构化指标与基线（方便后续差异跟踪或必要时回溯）。

1. 迁移范围
  - 属性测试源文件：4 个
    1) `prop_strategy_http_override.rs`
    2) `prop_retry_override.rs`
    3) `prop_strategy_summary_codes.rs`
    4) `prop_partial_filter_capability.rs`
  - Service 级残留 (`git_impl_tests.rs`) 特有用例：3 个（progress negotiating anchor / fast cancel 映射 / invalid local path fail-fast）→ 迁移至 `tasks/task_registry_and_service.rs` `section_service_impl_edges`。

2. 聚合落点 & Sections
  - 聚合文件：`quality/error_and_i18n.rs`
  - 新增 sections：
    * `section_strategy_props`
    * `section_retry_props`
    * `section_partial_filter_props`
    * （TLS override 属性测试已移除，不再单列 `section_tls_props`）
  - 回归种子：TLS override 属性测试已下线；历史 `prop_tls_override.proptest-regressions` 文件保留用于归档（不再被自动加载）。

3. 剪裁与集中度指标
  | 指标 | 数值 | 说明 |
  |------|------|------|
  | `git_impl_tests.rs` 剪裁累计 | ≥90% | 仅 3 个特有场景被迁移后文件占位化 |
  | 属性测试集中度 | 100% | 所有 proptest 用例统一至单文件 sections |
  | root-level 业务测试文件 | 0 | 仅保留 regression 种子文件 |
  | 新增聚合模块 | 0 | 遵循“≤2 新模块”限制，复用既有 quality 文件 |

4. Keyword 分布基线（Phase 4 结束时）
  | 关键词 | 次数 | 说明 |
  |---------|------|------|
  | `applied_codes` | 22 | 覆盖 strategy summary / gating 断言 |
  | `http_strategy_override_applied` | 5 | HTTP override 应用标记 / 事件锚点 |
  | `retry_strategy_override_applied` | 9 | Retry override 应用标记 / 指标校验 |
  | `RealHostCertVerifier` | 6 | Real Host 证书校验包装器覆写测试 |
  | `cert_fp` | 36 | 证书指纹指标与日志路径 |
  | `partial_filter` | 66 | Clone + Fetch partial filter 场景 & fallback 判定 |
  | `retry_override` | 13 | Retry override + backoff 属性与场景测试 |

  说明：计数方法为 PowerShell 下 `Select-String -SimpleMatch` 对 `tests` 目录递归统计（仅 .rs 文件）。未来新增/删除相关用例需更新此表以保持追踪一致性；若出现 >20% 波动，请在修订记录中添加说明（例如 Real Host 校验扩展或指标字段重命名导致的关键字调整）。

5. 质量保障
  - proptest 组数：4（HTTP override 正常化 / summary applied codes 一致性 / retry override 差异检测 / partial filter capability fallback）
  - 种子保留：是（确保回归失败可复现）
  - 统一初始化：沿用文件级 `#[ctor] init_test_env()` 保证环境幂等
  - 行数控制：聚合后文件 < 800 行（当前 ~<实际行数待查看>，低于警戒线）

6. 风险评估与缓解
  | 风险 | 状态 | 缓解 |
  |------|------|------|
  | 属性测试文件继续增长 | 可控 | 若新增 >3 组属性测试，评估拆出 `quality/props_strategy_and_filter.rs`（文档补充修订）|
  | 重复 AppConfig 变异样板 | 存在 (~30 行) | 后续提炼 `build_cfg_with_overrides(...)` helper 减少重复 |
  | 关键字噪音过多 | 正常 | 基线已记录，可用后续脚本过滤上下文行 (prefix 聚合) |

7. 后续可选优化（非阻塞）
  - 属性配置构造 helper 抽取，减少 map/string 拼接样板。
  - 统计脚本：输出关键字频次与按 section 分布 JSON（便于趋势 diff）。
  - 将部分 proptest 共享策略（生成器）提升至 `tests/common/` 以供其它质量属性（未来权限/性能）重用。

8. 验收结论
  Phase 4 目标全部达成：属性测试 100% 集中；git_impl 剪裁 ≥90%；root-level 业务测试文件清零；遵循新增模块数量限制；所有迁移后回归测试全绿。

### v1.18 最终清理 (12.16 Completion)

1. 删除文件清单（26）
```
git_impl_tests.rs
git_strategy_override_no_conflict.rs
git_strategy_override_structured.rs
git_strategy_override_summary_fetch_push.rs
git_strategy_override_tls_combo.rs
git_strategy_override_tls_mixed.rs
git_strategy_override_tls_summary.rs
git_tag_remote_extra.rs
git_tag_remote.rs
git_tasks_local.rs
git_tasks.rs
git_tls_override_event.rs
git_tls_push_insecure_only.rs
prop_partial_filter_capability.rs
prop_retry_override.rs
prop_strategy_http_override.rs
prop_strategy_summary_codes.rs
prop_tls_override.rs
refname_validation.rs
strategy_override_empty_unknown_integration.rs
strategy_override_invalid_integration.rs
strategy_override_push.rs
strategy_override_summary.rs
task_integration.rs
task_registry_edge.rs
task_registry_extra.rs
task_registry_post_complete_cancel.rs
```

2. 保留文件
 - `prop_tls_override.proptest-regressions`（属性测试回归种子，非测试代码）

3. 删除判定依据
 - 全部文件内容均为单行/少量注释 + 单 `#[test]` 内 `assert!(true)` 的占位形式；无业务断言逻辑。
 - 迁移目标已落实于聚合文件：`git/`, `tasks/`, `quality/` 等目录结构。

4. 回归结果
 - `cargo test -q`：全部通过（核心、聚合、属性、E2E 套件）。
 - 无新忽略/失败；执行时间与 v1.17 基线一致（差异仅统计噪音）。

5. 最终状态指标
 | 指标 | 结果 | 说明 |
 |------|------|------|
 | root-level 业务测试 .rs | 0 | 仅保留种子文件 (.proptest-regressions) |
 | 占位文件删除率 | 100% | 已无注释+trivial assert 占位残留 |
 | 属性测试集中度 | 100% | 统一 `quality/error_and_i18n.rs` sections |
 | 新增模块总数 | 2 | `git_tag_and_remote` / `task_registry_and_service` |

6. 风险与缓解（收尾验证）
 | 风险 | 状态 | 说明 |
 |------|------|------|
 | 误删仍被引用 helper | 无 | 所删文件无函数导出；聚合文件编译与测试验证通过 |
 | 覆盖率波动 | 可忽略 | 占位不含断言逻辑，不影响语义覆盖 |
 | 种子文件误删 | 已避免 | 保留 seed，后续 flake 可重放 |

7. 后续可选（Post-cleanup）
 - 若需要进一步减少 `quality/error_and_i18n.rs` 行数，可评估 props sections 拆分（当前未达阈值无需动作）。
 - 引入自动脚本生成关键字与 test 函数计数 JSON 供 CI 变更趋势展示。
 - 完成一次覆盖率基线快照（可选）作为后续性能/去重阶段对照。

8. 收尾结论
 测试重构（12.1~12.16 + Phase 1~4 + Props Consolidation）全流程完成；目录与语义结构稳定；后续新增场景将主要集中修改聚合文件与公共 helper，维护成本显著下降。


## 1. 背景与问题概述
当前 `src-tauri/tests` 目录下存在大量粒度较细、命名不完全统一且逻辑交叉的集成测试文件，导致：
- 重复逻辑（仓库初始化、事件断言模式、参数组合构造）散落，维护成本高。
- 相关场景（例如 clone 参数矩阵、partial / shallow / retry / override）分散在多文件，不利于整体理解。
- 事件断言多为全序列硬编码，脆弱且难以扩展。
- 后续计划中的“参数化 + 统一矩阵”去重难以直接实施。

## 2. 重构总体目标
| 维度 | 目标 | 量化指标 |
|------|------|---------|
| 文件聚合 | 将大量零散测试文件聚合为 ≤20 个主题聚合文件 | 聚合完成后：`tests` 业务逻辑类文件数 ≤20 |
| 结构清晰度 | 目录按领域/语义分层 | `git/`, `events/`, `quality/`, `e2e/`, （可选 `legacy/`） |
| 重复减少 | 第二阶段（参数化）前先识别重复；后续削减 ≥30% 重复行 | 统计公共 helper & 矩阵引入前后差异 |
| 稳定性 | 每次聚合后回归全绿；覆盖率回退 <2% | CI 报告对比基线 |
| 可扩展性 | 后续新增 Git feature 或事件格式时只需少量集中改动 | 新增场景平均新增文件数≈0 | 

## 3. 聚合后目标逻辑域与文件规划
初始规划 16 个聚合文件（可压缩到 14）。如需进一步压缩，可合并 clone/fetch 与 partial 变体。列表中英文命名为最终文件名建议：

### 3.1 Git 相关
1. `git_init_and_repo_structure.rs`
2. `git_add_and_commit.rs`
3. `git_branch_and_checkout.rs`
4. `git_clone_core.rs`
5. `git_clone_shallow_and_depth.rs`
6. `git_clone_partial_filter.rs`
7. `git_fetch_core_and_shallow.rs`
8. `git_fetch_partial_filter.rs`
9. `git_push_and_retry.rs`
10. `git_strategy_and_override.rs`（Strategy override + HTTP override + adaptive TLS rollout）
11. `git_preconditions_and_cancel.rs`

### 3.2 事件 / 质量 / E2E
12. `events_structure_and_contract.rs`
13. `events_task_lifecycle_git.rs`
14. `error_and_i18n.rs`
15. `e2e_public_git.rs`

### 3.3 可选（如仍需隔离 legacy 或实现对比）
16. `legacy_and_impl_integration.rs`

### 3.4 可选压缩策略
- 合并 7+8 => `git_fetch_and_partial_filter.rs`
- 合并 4+5 => `git_clone_core_and_shallow.rs`
压缩后总数可降至 14。

## 4. 目录结构设计（重构完成后目标）
```
src-tauri/
  tests/
    git/
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
    events/
      events_structure_and_contract.rs
      events_task_lifecycle_git.rs
    quality/
      error_and_i18n.rs
    e2e/
      e2e_public_git.rs
    legacy/ (可选)
      legacy_and_impl_integration.rs
    common/
      mod.rs
      fixtures.rs
      repo_factory.rs
      git_scenarios.rs
      event_assert.rs
      http_override_stub.rs
      retry_matrix.rs
      partial_filter_matrix.rs
      shallow_matrix.rs
      test_env.rs
```

## 5. 公共抽象与工具模块规划
| 文件 | 作用 | 关键接口示例 |
|------|------|--------------|
| `fixtures.rs` | 基础临时目录与最小化仓库构造 | `fn temp_repo() -> TestRepo` |
| `repo_factory.rs` | 复杂结构（多分支 / tag / 深度）仓库预置 | `fn repo_with_branches(&[&str])` |
| `git_scenarios.rs` | 高层操作 DSL | `fn run_clone(params) -> CloneResult` |
| `event_assert.rs` | 事件序列/局部匹配断言 | `assert_seq(actual, expected_spec)` |
| `http_override_stub.rs` | HTTP override/伪服务构建 | `fn mock_http_scenario(case)` |
| `retry_matrix.rs` | retry/backoff 参数矩阵 | `fn retry_cases() -> Vec<RetryCase>` |
| `partial_filter_matrix.rs` | partial filter 组合 | `enum PartialFilterCase` + `cases()` |
| `shallow_matrix.rs` | 浅克隆/加深参数集合 | `depth_cases()` / `invalid_depth_cases()` |
| `test_env.rs` | 环境初始化 & tracing/log | `fn init_test_env()` |

### 5.1 DSL / 断言风格
- 采用 *结构化事件子集匹配*：仅断言关键信息（类型、主体、结果码），减少脆弱性。
- 事件序列断言模式示例（概念）：
  ```rust
  expect_sequence!([
    ev::clone_start(),
    ev::progress_contains("enumerating objects"),
    ev::clone_complete().success()
  ], actual_events);
  ```
  可实现为宏或函数 + builder。

### 5.2 参数矩阵策略
- 只为**高重复**场景生成（partial filter / shallow depth / retry backoff / http override 变体）。
- 保持**可枚举 + 显式列表**，避免动态生成产生 case 爆炸。
- 每个矩阵 case 实现 `Display` 便于测试失败上下文输出。

## 6. 聚合实施总体阶段（概览）

| 阶段 | 主题 | 关键动作 | 验收快照 |
|------|------|----------|---------|
| 1 | 基础与仓库结构/提交 | 建立 `common/` 初版 (`fixtures`, `test_env`)，聚合 init / add / commit | 测试通过，覆盖率差异<2% |
| 2 | Clone / Fetch 族群 | 引入矩阵（partial/shallow），聚合 clone/fetch 相关 | 同上 |
| 3 | Push / Retry / Override | 建立 retry/http 工具，聚合策略/override/push | 同上 |
| 4 | Events / Error / E2E | 引入事件断言 DSL；聚合事件/错误/E2E | 同上 |
| 5 | 去重与参数化 | 将重复逻辑迁移到矩阵与 DSL；统计重复下降 | 行数下降≥30% |
| 6 | 清理与文档化 | 删除 `_legacy` 文件，补充 README/贡献指南 | 无 orphan 文件 |

## 7. 命名与编码规范
- 文件命名：`git_*`, `events_*` 前缀统一；复合主题用下划线分隔（不使用缩写）。
- 测试函数名：`test_<动作>_<场景>_<期望>`，参数化循环内使用 `case.describe()`；或 `#[test]` 包装遍历。
- 避免使用 `unwrap()`（除非初始化前置失败应 panic），统一使用 `expect("context ...")` 增强调试可读性。
- 事件断言失败输出：必须包含 event index + 摘要字段。

## 8. 验收标准 (Definition of Done)
| 项目 | 判定方式 |
|------|----------|
| 聚合完成 | 旧散文件被集中/替换并加头部注释列来源 |
| 覆盖率维持 | 与基线比较总行/分支覆盖率下降 <2% |
| 无功能回退 | 关键路径（clone/fetch/push/partial/shallow/retry/override/events）测试全部通过 |
| 去重收益 | 统计重复块行数下降 ≥30% （阶段 5 后）|
| 可维护性 | 新增 Git 场景只需修改 ≤2 个聚合文件 + ≤2 个公共 helper |

## 9. 风险与缓解
| 风险 | 描述 | 缓解策略 |
|------|------|----------|
| 事件顺序脆弱 | 硬编码全序列易因新增非关键事件破坏测试 | 采用子集/模式匹配；封装 DSL |
| 聚合文件过大 | 可读性下降 | 控制 <800 行；内部 `mod section_x` 分块 |
| 参数化失败定位困难 | 单测试循环多 case 定位不直观 | case 输出上下文 + 分组日志 + `Display` 实现 |
| 覆盖率意外下降 | 公共封装隐藏逻辑路径 | 引入 instrumentation 注释 / 保持语义一致回归比对 |
| 重命名引起历史定位困难 | Git blame 分散 | 文件头注释保留来源文件列表；初期保留 `_legacy` 过渡 |

## 10. 度量与监控
- 每阶段结束：记录 `cargo test` 总用时、失败数、覆盖率（行/分支）。
- 统计重复：脚本（后续可添加）扫描 `tests/` 下相同/相似函数片段（哈希语句规范化）。
- 事件断言稳定性：未来 3 次功能分支合并中 0 个因非关键事件插入导致的误失败。

## 11. 后续（预留）
- 引入 snapshot（可考虑 `insta`，若策略允许）对事件模式做回归锁定。
- 若参数矩阵继续膨胀，评估代码生成/宏自动展开。
- 增补自动统计重复/覆盖回归脚本（脚本形成后附录）。

---

## 12. 详细路线图

本章节列出全部阶段的完整路线图（已定稿），执行时按优先级或依赖关系选择推进；若阶段内容调整，请同步修订记录。

说明：第 6 节表格中的阶段 1~6 为宏观实施阶段；本 12.x 各小节为具体聚合/重构工作单元（两套编号独立，无依赖冲突）。为避免重复，12.x 标题已去除“阶段 n”字样，仅保留文件语义名称。

### 12.1 git_init_and_repo_structure

**目标范围**  
聚合并覆盖以下原始测试文件（示例，实际以现有文件为基准）：
- `git_init.rs`
- `git_local_skeleton.rs` / `git_local_skeleton_*`（如有）
- 与仓库基本形态/空仓/目录结构/初始分支/预检查相关的 `*_preflight.rs`（仅限与 clone 逻辑解耦的纯本地/结构预检查部分）

不纳入（后续阶段处理）：
- 涉及 clone 远端交互参数验证的测试（归入 `git_clone_core`）
- 涉及 shallow/partial 行为的测试

**产出文件**  
`src-tauri/tests/git/git_init_and_repo_structure.rs`

**内部结构建议**
```rust
mod section_basic_init;     // 基础 init 场景
mod section_repo_layout;    // 目录/refs/HEAD 断言
mod section_preflight;      // 本地前置校验
```

**关键重构动作**
1. 创建 `tests/common/fixtures.rs`：
  - `struct TestRepo { path: PathBuf }`
  - `fn create_empty_repo() -> TestRepo`
  - `fn create_repo_with_initial_commit(msg: &str) -> TestRepo`
2. 创建 `tests/common/test_env.rs`：
  - `static INIT: Once = Once::new();`
  - `pub fn init_test_env()` 初始化日志、必要环境变量（如 GIT_AUTHOR/GIT_COMMITTER）。
3. 在新聚合文件中统一调用 `init_test_env()`（首次行）。
4. 收集原文件中对 repo 结构（`.git/HEAD`、`refs/heads/main`、初始分支名称等）的断言，归类到 `section_repo_layout`。
5. 若存在重复的“读取 HEAD 内容”函数 -> 抽到 `fixtures.rs` `fn read_head(repo: &TestRepo) -> String`。
6. 对原本依赖外部函数的路径引用保持不变；仅在内部使用替换好的 fixtures。

**非功能性改进**
- 事件断言：本阶段若仅少量事件，可暂时保留原直接断言，后续阶段再统一改为 DSL。
- 错误输出上下文：为每个 `assert!` 增加 `"[init-basic] ..."` 前缀。

**验收标准**
| 指标 | 期望 |
|------|------|
| 测试通过 | `cargo test -q` 全绿 |
| 覆盖率 | 覆盖率较基线（重构前执行一次）下降 <2% |
| 功能等价 | 无移除原测试逻辑分支；仅代码位置变更 |
| 可读性 | 新文件内逻辑分块清晰，头部来源注释完善 |

**回滚策略**
- 保留原文件重命名为 `*_legacy.rs` 一次提交周期；确认 2 次 CI 绿后删除。
- 如新文件出现不稳定，可临时恢复 legacy 文件并在下一提交修复。

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 忽略隐式 helper | 原文件某些内联小工具被遗漏 | 先全文搜索 `fn` 复制，再做最小抽象 |
| 覆盖率波动 | 抽象后分支被优化裁剪 | 在抽象前后运行 `cargo llvm-cov`（若使用）记录差异 |
| 环境依赖缺失 | 新增统一初始化后少设变量 | `init_test_env()` 内集中设置并文档列出 |

**实施顺序**
1. 基线采集：当前 main 分支运行一次测试 + 覆盖率存档。
2. 添加 `common/fixtures.rs` 与 `common/test_env.rs`（空实现 + TODO 注释）。
3. 新建聚合文件骨架（含来源注释，但暂不迁移内容）。
4. 逐段迁移：basic init -> layout -> preflight，每段迁移后运行快速测试（可只运行包含关键关键字的过滤，如 `cargo test init_basic`；若无命名过滤则临时注释未迁移段）。
5. 全量迁移完成后删除已迁移逻辑的 legacy 文件内容（或重命名 `_legacy`）。
6. 全量测试、覆盖率对比、提交。

**后续挂起事项（进入阶段 5 时处理）**
- 统计该文件与后续 clone 模块在仓库初始化逻辑的重合函数候选，准备上移至 `repo_factory.rs`。
- 考虑将预检查（preflight）场景参数化（存在多个布尔组合）。

---

### 12.2 git_add_and_commit

**目标范围**  
聚合 `git_add.rs`, `git_add_enhanced.rs`, `git_commit.rs`, `git_commit_task.rs` 四类（含可能的增强/任务型封装）。关注：
- 跟踪文件新增/修改/删除的暂存逻辑
- 复杂提交场景（多父、空提交拒绝、消息模板）
- 任务型封装（如果存在异步/队列型执行）语义保持

不纳入：push（下一阶段）、分支切换（后续阶段）。

**产出文件**  
`src-tauri/tests/git/git_add_and_commit.rs`

**内部结构建议**
```rust
mod section_add_basic;      // 单文件 add / 多文件 add / 重复 add 幂等
mod section_add_edge;       // 忽略规则 / 二进制 / 空目录
mod section_commit_basic;   // 常规提交（含 message 校验）
mod section_commit_edge;    // 空树/无改动/签名/父指针验证
mod section_task_wrapper;   // 若存在任务封装/带事件提交流
```

**关键重构动作**
1. 复用阶段 1 已建立的 `fixtures.rs`：新增 `fn modify_file(repo, path, content)` & `fn add(repo, paths)` 辅助。
2. 若多处重复检测索引状态，抽象 `fn list_index(repo) -> Vec<String>`。
3. 将 add 与 commit 事件断言初步集中（后续可迁入 `event_assert.rs`）。
4. 合并 enhanced 版本测试：通过参数/feature flag 表达差异，而非复制整块逻辑。

**参数化候选**
- add 行为分类：普通文件 / .gitignore 被忽略 / 已暂存重复 add / 子目录文件。
- commit 行为：空索引拒绝 / 正常 / 多父（merge 模拟） / amend（若有）。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 原 4 文件语义全部迁移 |
| 去重 | 重复的 repo 修改 + 提交构造逻辑行数下降 ≥20%（相对四文件总和） |
| 稳定性 | 两次连续 CI 均通过 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| Enhanced 行为被弱化 | 统一后遗漏差异断言 | 先列差异矩阵清单再迁移 |
| 事件顺序差异 | add vs commit 事件混杂 | 使用事件类型过滤匹配而非全列表索引 |

**实施顺序**
1. 建立文件骨架 + 来源注释。
2. 迁移 add 基础用例（保留 legacy 版）。
3. 迁移 commit 基础用例。
4. 迁移增强/任务封装用例，抽象辅助函数。
5. 去重（合并重复 helper）。
6. 删除 legacy / 覆盖率比对 / 提交。

### 12.3 git_branch_and_checkout

**目标范围**  
聚合所有与分支操作相关测试：
- 创建分支、列出分支、删除分支（若存在）
- 切换分支 / checkout（含分离 HEAD / tag checkout 如适用）
- 冲突或工作区未提交状态下的 checkout 行为

不纳入：rebase / merge（若存在，应单独未来扩展）。

**产出文件**  
`src-tauri/tests/git/git_branch_and_checkout.rs`

**内部结构建议**
```rust
mod section_branch_create;     // 创建与列举
mod section_branch_delete;     // 删除（包含保护分支策略）
mod section_checkout_basic;    // 正常切换
mod section_checkout_dirty;    // 工作区有改动
mod section_checkout_detached; // 分离 HEAD / tag
```

**关键重构动作**
1. 引入 `repo_factory.rs`（若在此阶段落地）：新增 `fn repo_with_branches(names: &[&str])`。
2. 将重复的“当前分支”检测抽象：`fn current_branch(repo) -> Option<String>`。
3. checkout 前后 HEAD/refs 断言统一封装：`assert_head_points_to(repo, rev)`。
4. 对 dirty 工作区构造集中：`fn create_dirty_state(repo, files: &[(&str,&str)])`。

**参数化候选**
- 创建分支：`(是否已有同名, upstream 是否存在, 名称是否合法)`。
- checkout：`(目标存在/不存在, 工作区是否干净, 是否需要创建新分支)`。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 原分支与 checkout 场景不丢失 |
| 去重 | 分支名称/HEAD 读取逻辑集中一处 |
| 稳定性 | checkout 场景无随机失败（重复执行 3 次稳定） |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| HEAD 状态获取实现差异 | 不同测试各自实现导致行为不一致 | 使用统一 helper 读取 refs/HEAD |
| 脏工作区副作用 | 前一 case 影响下一 case | 每 case 新建临时仓库或重置工作区 |

**实施顺序**
1. 建立文件骨架。
2. 迁移 branch 创建/列举测试。
3. 迁移 checkout 基础 + HEAD 断言，封装 helper。
4. 迁移 dirty/冲突相关场景。
5. 删除 legacy 与重复函数。

### 12.4 git_clone_core

**目标范围**  
聚合纯粹 clone 参数与基本行为：
- `git_clone_fetch_params.rs`
- `git_clone_fetch_params_valid.rs`
- `git_clone_fetch_params_combo.rs`
- `git_clone_preflight.rs`（仅与远端通信前参数合法性、路径校验相关部分）

不纳入：shallow / partial filter / depth 相关（后续阶段）。

**产出文件**  
`src-tauri/tests/git/git_clone_core.rs`

**内部结构建议**
```rust
mod section_params_validation; // 单参数合法/非法
mod section_params_matrix;     // 组合参数矩阵（宽覆盖）
mod section_preflight;         // 远端操作前静态/本地校验
mod section_behavior_basic;    // 基础 clone 行为（成功/失败分支）
```

**关键重构动作**
1. 创建/扩展参数结构：`CloneParams { depth: Option<u32>, recursive: bool, tags: bool, ... }`（若在生产代码已有则直接复用）。
2. 统一提供 `fn run_clone_case(case: &CloneCase) -> CloneResult`（封装调用 + 收集事件）。
3. 组合矩阵：保留显著差异子集（如 8~15 个），其余冗余 case 标记候选删除。
4. 失败断言使用错误分类枚举而非字符串包含（若可能）。

**参数矩阵初稿示例**
| 维度 | 值 | 备注 |
|------|----|------|
| recursive | true/false | 差异在子模块处理 |
| tags | true/false | 组合减少：只与 recursive=true 交叉一次 |
| depth | None | depth 场景移至 shallow 阶段 |
| sparse | false | sparse/partial 进入 partial 阶段 |

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 原 clone 参数合法/非法场景保留 |
| 去重 | 参数组合测试数减少 ≥30% 而不丢失等价类别 |
| 可维护性 | 新增参数只需增加一处矩阵描述 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 参数组合删减过度 | 误删导致未覆盖边界 | 先分类（等价类）再删，保留代表值 |
| 结果分类不稳定 | 错误类型匹配依赖文本 | 使用枚举/错误码映射 |

**实施顺序**
1. 定义或引入 `CloneCase` / `CloneParams`。
2. 迁移单参数合法/非法用例 -> `section_params_validation`。
3. 构建组合矩阵精简列表 -> `section_params_matrix`。
4. 引入 preflight 场景。
5. 合并行为类用例（成功 clone，远端不存在等）。
6. 覆盖率/稳定性验证，移除 legacy。

### 12.5 git_clone_shallow_and_depth

**目标范围**  
聚合所有 shallow clone / deepen / invalid depth / 本地忽略相关：
- `git_shallow_clone.rs`
- `git_shallow_fetch.rs` / deepen 变体（如分拆）
- `git_shallow_fetch_deepen.rs`
- `git_shallow_fetch_invalid_depth.rs`
- `git_shallow_invalid_depth.rs`
- `git_shallow_fetch_local_ignore.rs`
- `git_shallow_local_ignore.rs`
- `git_shallow_file_url_deepen.rs`

不纳入：partial filter（下一阶段）。

**产出文件**  
`src-tauri/tests/git/git_clone_shallow_and_depth.rs`

**内部结构建议**
```rust
mod section_basic_shallow;     // 初始 shallow clone depth=N
mod section_invalid_depth;     // depth=0, 负值, 过大
mod section_deepen;            // 后续 deepen 行为
mod section_local_ignore;      // 本地路径忽略策略
mod section_file_url;          // file:// 场景（若与安全策略相关）
```

**关键重构动作**
1. 在 `shallow_matrix.rs` 定义：`enum ShallowCase { Depth(u32), InvalidDepth(i32), Deepen{ from:u32, to:u32 } }`。
2. 统一执行函数：`run_shallow_case(&ShallowCase) -> Result<ShallowOutcome>`。
3. 断言 repo `.git/shallow` 文件存在与否及其内容；封装 `fn read_shallow(repo) -> Vec<Oid>`（或行号列表）。
4. 对 deepen 行为断言：对象数量增加、shallow 文件被更新或移除。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 原 shallow 与 deepen 行为全迁移 |
| 参数化 | 有效 depth 列表集中一处（matrix） |
| 失败分类 | invalid depth 使用枚举错误类型断言 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 深度相关测试波动 | 远端仓库对象数量变动 | 使用固定本地 fixture 仓库或生成器 |
| deepen 用例顺序耦合 | 依赖前一 case 状态 | 每 case 独立 clone -> deepen 流程 |

**实施顺序**
1. 建立矩阵与枚举定义。
2. 迁移 basic shallow 用例。
3. 迁移 invalid depth，用统一错误断言替换文本匹配。
4. 迁移 deepen 场景。
5. 迁移本地忽略 & file url 变体。
6. 覆盖率/稳定性验证，移除 legacy。

### 12.6 git_clone_partial_filter

**目标范围**  
聚合所有 partial clone filter 相关：
- `git_partial_clone_filter_capable.rs`
- `git_partial_clone_filter_event_baseline.rs`
- `git_partial_clone_filter_event_code.rs`
- `git_partial_clone_filter_event_code_with_depth.rs`
- `git_partial_clone_filter_event_only.rs`
- `git_partial_clone_filter_event_structure.rs`
- `git_partial_clone_filter_event_with_depth.rs`
- `git_partial_clone_filter_fallback.rs`

**产出文件**  
`src-tauri/tests/git/git_clone_partial_filter.rs`

**内部结构建议**
```rust
mod section_capability;   // 服务端/远端 capability 探测
mod section_filter_event; // event-only, code-only, structure 变体
mod section_filter_depth; // 与 depth 交叉（with_depth / code_with_depth）
mod section_fallback;     // capability 缺失 fallback 行为
```

**关键重构动作**
1. 在 `partial_filter_matrix.rs` 定义：`enum PartialFilterCase { EventOnly, CodeOnly, Structure, CodeWithDepth(u32), EventWithDepth(u32) }`。
2. `run_partial_filter_case()` 返回：对象统计、事件摘要列表、是否 fallback。
3. 统一 fallback 判定：通过返回值布尔而不是文本搜索。
4. 事件断言初步改为 pattern：`expect_partial_sequence(case.expected_events(), actual)`。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 原过滤行为与 fallback 场景完整 |
| 参数化 | 过滤类型集中定义一次 |
| 事件稳定 | 变体间共享断言逻辑（差异只在期望集合） |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 与 shallow 阶段交叉 | depth 逻辑重复 | depth 相关公共检查抽到 shallow_matrix/共享 helper |
| 事件差异细碎 | 不同过滤组合事件顺序略差 | 使用集合/子序列匹配而非完整顺序 |

**实施顺序**
1. 定义矩阵枚举 + case 列表。
2. 迁移 capability 基线。
3. 迁移 event-only / code-only / structure。
4. 迁移 with_depth 交叉。
5. 迁移 fallback。
6. 参数化统一化、移除 legacy。

### 12.7 git_fetch_core_and_shallow

**目标范围**  
聚合 fetch 基础与 shallow fetch（不含 partial filter）：
- `git_fetch.rs`
- `git_shallow_fetch.rs`
- `git_shallow_fetch_deepen.rs`（与 clone deepen 有差异的 fetch deepen）
- `git_shallow_fetch_invalid_depth.rs`
- `git_shallow_fetch_local_ignore.rs`

**产出文件**  
`src-tauri/tests/git/git_fetch_core_and_shallow.rs`

**内部结构建议**
```rust
mod section_fetch_basic;    // 普通 fetch：新增引用/无变化
mod section_fetch_shallow;  // 初始 shallow fetch
mod section_fetch_deepen;   // deepen fetch 行为
mod section_fetch_invalid;  // 非法 depth
mod section_fetch_ignore;   // 本地忽略策略
```

**关键重构动作**
1. 复用 shallow 阶段的 `ShallowCase`（扩展 variant：`FetchDeepen`）。
2. 标准化 fetch 结果结构：`FetchOutcome { updated_refs: Vec<RefChange>, objects_fetched: u32, shallow: bool }`。
3. 深化（deepen）行为：比较前后对象数差异（需要基线 snapshot 或统计函数）。
4. 对 invalid depth 与 ignore 策略统一错误分类。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 基础 + shallow + deepen + invalid + ignore 场景完整 |
| 复用 | 不重复 shallow 深度逻辑实现 |
| 稳定性 | 多次运行对象计数一致 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 与 clone shallow 重复 | 逻辑复制 | 提取共享 helper：`assert_depth(repo, expected)` |
| 对象计数不稳定 | 远端状态变化 | 采用本地 fixture 仓库生成器 |

**实施顺序**
1. 构建 outcome 结构与 helper。
2. 迁移 basic fetch。
3. 迁移 shallow fetch + deepen。
4. 迁移 invalid / ignore。
5. 合并去重，移除 legacy。

### 12.8 git_fetch_partial_filter

**目标范围**  
聚合所有 fetch partial filter：
- `git_partial_fetch_filter_capable.rs`
- `git_partial_fetch_filter_event_baseline.rs`
- `git_partial_fetch_filter_event_code.rs`
- `git_partial_fetch_filter_event_code_with_depth.rs`
- `git_partial_fetch_filter_event_invalid_filter_no_code.rs`
- `git_partial_fetch_filter_event_no_filter_no_code.rs`
- `git_partial_fetch_filter_event_only.rs`
- `git_partial_fetch_filter_event_with_depth.rs`
- `git_partial_fetch_filter_fallback.rs`
- `git_partial_fetch_invalid_filter_capable.rs`

**产出文件**  
`src-tauri/tests/git/git_fetch_partial_filter.rs`

**内部结构建议**
```rust
mod section_capability;      // capability 检测与 invalid
mod section_filter_variants; // event-only / code / baseline / no_filter
mod section_filter_depth;    // with_depth / code_with_depth
mod section_fallback;        // fallback 行为
```

**关键重构动作**
1. 复用 `PartialFilterCase`（扩展 fetch 语义字段：`op: Clone|Fetch`）。
2. 统一 outcome：`PartialFilterOutcome { filtered: bool, depth: Option<u32>, fallback: bool }`。
3. 无过滤 / invalid 过滤场景对比：明确 filtered=false 且 fallback 标志区分于 capability 不支持。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | fetch partial filter 场景全迁移 |
| 参数化 | 案例通过矩阵生成 |
| 稳定性 | fallback 判定不依赖日志文本匹配 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| clone 与 fetch 共享 case 混淆 | 两者事件差异 | 在枚举中分支处理并加类型标签 |
| invalid filter 分类模糊 | 错误与 capability 缺失混用 | 明确错误码映射 |

**实施顺序**
1. 枚举扩展 + outcome 结构。
2. capability / invalid 迁移。
3. filter_variants 迁移。
4. depth 交叉迁移。
5. fallback 迁移。
6. 去重/稳定性验证，移除 legacy。

### 12.9 git_push_and_retry

**目标范围**  
聚合 push 与 retry/backoff 相关：
- `git_push.rs`
- `git_retry_override_event.rs`
- `git_retry_override_event_structured.rs`
- `git_retry_override_backoff.rs`

**产出文件**  
`src-tauri/tests/git/git_push_and_retry.rs`

**内部结构建议**
```rust
mod section_push_basic;     // 正常 push 新引用 / 无变化
mod section_push_conflict;  // 远端落后 / 拒绝
mod section_retry_policy;   // 重试策略覆盖（次数/间隔）
mod section_retry_event;    // 结构化事件断言
```

**关键重构动作**
1. 在 `retry_matrix.rs` 定义：`RetryCase { attempts: u8, backoff: BackoffKind, override_policy: Option<Policy> }`。
2. 提供 `run_push_with_retry(case) -> PushOutcome { attempts_used, success, events }`。
3. 统一对 backoff 时间的断言：仅验证序列长度 & 单调递增，不做真实时间 sleep（用注入式计时器或 mock）。
4. 事件断言集中使用 pattern（阶段 4 事件 DSL 可复用）。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | push 正常/无变化/冲突 + retry 逻辑 |
| 参数化 | 重试策略通过矩阵统一 |
| 稳定性 | 不依赖真实时间，测试耗时可控 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 时间相关 flakiness | 等待真实 backoff | 注入 mock 计时，将 sleep 替换为逻辑推进 |
| 事件序列差异 | push 冲突 vs 成功 | 使用子序列匹配 |

**实施顺序**
1. 定义 retry 矩阵 & outcome。
2. 迁移 push 基础场景。
3. 迁移冲突与失败场景。
4. 迁移 retry/backoff 场景（替换 sleep）。
5. 参数化合并 + legacy 清理。

### 12.10 git_strategy_and_override

**目标范围**  
聚合 Strategy Override + HTTP Override + 自适应 TLS / 传输层策略相关：
- `git_strategy_override_combo.rs`
- `git_http_override_event.rs`
- `git_http_override_no_event.rs`
- `git_http_override_event_structured.rs`
- `git_http_override_idempotent.rs`
- `git_http_override_invalid_max_no_event.rs`
- `git_http_override_fetch_event_only_max.rs`
- `git_http_override_clone_only_follow.rs`
- `git_http_override_push_follow_change.rs`
- `git_adaptive_tls_rollout_event.rs`

**产出文件**  
`src-tauri/tests/git/git_strategy_and_override.rs`

**内部结构建议**
```rust
mod section_strategy_combo;   // 组合策略覆盖
mod section_http_basic;       // http override 基础场景
mod section_http_limits;      // max / follow / idempotent 变体
mod section_http_events;      // 事件结构/存在与否
mod section_adaptive_tls;     // TLS rollout / 事件
```

**关键重构动作**
1. 在 `http_override_stub.rs` 定义：`HttpOverrideCase { op: GitOp, max_events: Option<u32>, follow: bool, idempotent: bool, expect_events: bool }`。
2. Strategy 与 HTTP override 共享 outcome：`OverrideOutcome { applied: bool, events: Vec<Event>, follow_chain: Vec<String> }`。
3. adaptive TLS：断言包含 rollout 事件 & 参数（比对结构字段而非字符串）。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 所有 override / strategy 场景迁移 |
| 去重 | HTTP 变体通过 case 枚举生成 |
| 事件断言 | 使用统一 helper，不出现硬编码全序列重复 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 组合爆炸 | max/follow/idempotent 交叉过多 | 剔除等价类：仅保留行为差异组合 |
| 事件存在性假阳性 | 仅检查数量不检查属性 | 子集匹配 + 关键字段校验 |

**实施顺序**
1. 定义 case & outcome 结构。
2. 迁移 strategy combo。
3. 迁移 http basic + limits。
4. 迁移 事件存在/不存在场景。
5. 迁移 adaptive TLS。
6. 参数化合并 + legacy 清理。

### 12.11 git_preconditions_and_cancel

**目标范围**  
聚合前置条件验证与任务取消相关：
- `git_preconditions_and_cancel.rs`
- 其它文件中嵌入的取消/超时/锁冲突场景（若有）

**产出文件**  
`src-tauri/tests/git/git_preconditions_and_cancel.rs`

**内部结构建议**
```rust
mod section_preconditions;  // 环境/配置/权限/路径预检
mod section_cancellation;   // 主动取消（用户动作 / 信号）
mod section_timeout;        // 超时触发（如受控 mock 计时器）
```

**关键重构动作**
1. 定义 `PreconditionCase`：列出需要触发失败的条件（权限、不存在目录、配置缺失等）。
2. 取消语义模拟：提供 `CancelableOpHandle` + `cancel()` mock，不依赖真实线程杀死。
3. 超时模拟：使用注入计时器 / mock 时间推进。
4. 断言分类：`Outcome { kind: Success|PreconditionFailed|Cancelled|TimedOut }`。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 所有前置失败路径与取消路径都有测试 |
| 不依赖真实时间 | 超时测试 < 100ms 执行 |
| 分类准确 | Outcome kind 精确判定 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 真实 sleep 导致耗时 | 超时章节使用真实延迟 | 注入 mock 时钟 |
| 取消实现与生产不同步 | 测试专用路径隐藏真实逻辑 | 通过公共 API 注入 cancellation token |

**实施顺序**
1. 定义 outcome & case 结构。
2. 迁移 preconditions 场景。
3. 迁移取消场景（替换直接线程操作）。
4. 迁移超时场景。
5. 去重 + legacy 清理。

### 12.12 events_structure_and_contract

**目标范围**  
聚合事件结构与契约（包括序列、字段 schema、是否包含 legacy 字段）：
- `events_structured_basic.rs`
- `events_contract_snapshot.rs`
- `events_no_legacy_taskerror.rs`
- `events_task_lifecycle_structured.rs`（拆分生命周期部分到下一阶段时保留结构共性）

**产出文件**  
`src-tauri/tests/events/events_structure_and_contract.rs`

**内部结构建议**
```rust
mod section_schema_basic;     // 单事件字段验证
mod section_sequence_minimal; // 最小必需序列（不含生命周期细节）
mod section_legacy_absence;   // legacy 字段缺失验证
mod section_contract_snapshot;// Schema snapshot / 版本锁定
```

**关键重构动作**
1. 引入 `event_assert.rs`：`assert_event_fields(event, ExpectedFields)`；`assert_contains_sequence(subset)`。
2. Snapshot：若允许依赖 `insta`，使用 `assert_json_snapshot!`（否则自建序列化 + 手动比较）。
3. 事件 schema 版本（若有 version 字段）在 snapshot 改变时提示 semver bump。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 原结构/contract/legacy 缺失场景完整 |
| 稳定性 | 非关键新增字段（可选）不破坏测试 |
| 快速失败 | schema 差异输出 diff 便于审查 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| snapshot 过度脆弱 | 任意新增字段引起失败 | 使用“选取字段子集”+ 单独 snapshot 全量 | 
| 序列变动噪音 | 顺序轻微调整频繁失败 | 改为集合或关键锚点序列匹配 |

**实施顺序**
1. 引入事件断言 helper。
2. 迁移 basic/schema 验证。
3. 迁移 legacy absence。
4. 引入 snapshot（或结构 diff）。
5. 去重 + legacy 清理。

### 12.13 events_task_lifecycle_git

**目标范围**  
聚合 Git 任务生命周期事件（开始、进度、结束、错误分支、push 特化等）：
- `events_task_lifecycle_git.rs`
- `events_task_lifecycle_git_fail.rs`
- `events_task_lifecycle_git_push.rs`

**产出文件**  
`src-tauri/tests/events/events_task_lifecycle_git.rs`

**内部结构建议**
```rust
mod section_success_flow;   // 正常生命周期（start -> progress* -> end）
mod section_failure_flow;   // 失败路径（含错误分类）
mod section_push_flow;      // push 特化生命周期（可能含额外事件）
mod section_metrics;        // 事件中携带度量字段验证
```

**关键重构动作**
1. 复用事件 DSL：`LifecycleSpec { op: GitOp, expect_fail: bool, extra: Option<ExtraEvents> }`。
2. 进度事件断言：检查关键里程碑而非每条细粒度（减少脆弱性）。
3. 错误分类与 `error_and_i18n` 后续整合：失败生命周期引用统一错误枚举。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 正常 / 失败 / push 特化 / metrics | 
| 稳定性 | 非关键进度事件新增不破坏测试 |
| 去重 | 与结构契约阶段共享 DSL，无重复硬编码序列 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 进度事件频度变化 | 触发假失败 | 仅匹配锚点：start, n% milestones, end |
| 错误与 i18n 耦合 | 两阶段相互阻塞 | 使用错误枚举占位，i18n 后续扩展 |

**实施顺序**
1. 定义 LifecycleSpec。
2. 迁移 success 流程。
3. 迁移 failure 流程（错误枚举接入）。
4. 迁移 push 特化。
5. 添加 metrics 断言。
6. 去重 + legacy 清理。

### 12.14 error_and_i18n

**目标范围**  
聚合错误映射与国际化（i18n）相关：
- `error_i18n_map.rs`
- 其它测试内内联的错误消息/本地化断言（迁入集中）

**产出文件**  
`src-tauri/tests/quality/error_and_i18n.rs`

**内部结构建议**
```rust
mod section_error_mapping;    // 错误码 -> 语义枚举映射
mod section_i18n_locale_basic;// 不同 locale 核心消息片段
mod section_i18n_fallback;    // 不存在语言回退逻辑
mod section_integration_edge; // 边缘错误复合（权限+网络+超时）
```

**关键重构动作**
1. 定义统一错误枚举（若生产端已有则引用）：`AppErrorKind`。
2. 建立 locale fixture：`LOCALES = ["en", "zh", ...]`，循环断言关键键存在。
3. 消息断言使用 key + 模板参数，而非完整文本（减少变动噪音）。
4. 断言 fallback：设置不存在 locale -> 回退 `en`。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 所有已存在错误映射 & locale 覆盖 |
| 稳定性 | 文本微调（非 key 变更）不破坏测试 |
| 可扩展 | 新增 locale 只需添加 key 集合，不增测试文件 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 文本硬编码脆弱 | 直接匹配整句 | 使用 key + 参数断言 |
| locale 环境污染 | 测试间相互影响 | 每 case 设置/恢复 locale 环境变量 |

**实施顺序**
1. 收集所有错误映射。
2. 实现 key 级断言 helper：`assert_error_kind(err, AppErrorKind::X)`。
3. 迁移 i18n locale 基础用例。
4. 迁移 fallback / edge 组合。
5. 去重 + legacy 清理。


### 12.15 e2e_public_git

**目标范围**  
聚合端到端（E2E）公共仓库交互：
- `git_e2e_public.rs`
- 其它散落文件中对真实/模拟远端整体流程校验场景（clone->modify->commit->push）

**产出文件**  
`src-tauri/tests/e2e/e2e_public_git.rs`

**内部结构建议**
```rust
mod scenario_clone_build_push;   // 完整流水线
mod scenario_read_only;          // 只读操作（clone/fetch）
mod scenario_error_boundary;     // 真实远端错误处理（权限/404）
```

**关键重构动作**
1. 提供高层 DSL：`run_pipeline(PipelineSpec)`，封装多步骤（减少重复）。
2. 对外部网络依赖：优先使用本地模拟服务（若现有支持），否则隔离为可选（feature flag）。
3. 断言以最终仓库状态 + 关键事件子集为准，不做细节低层断言（避免与单元测试重复）。

**验收标准**
| 指标 | 期望 |
|------|------|
| 功能覆盖 | 完整流水线 + 只读 + 错误边界 |
| 可重复 | 不依赖外网不稳定资源（或有跳过机制） |
| 价值 | 不与已有聚合文件测试内容完全重复 |

**风险与缓解**
| 风险 | 描述 | 缓解 |
|------|------|------|
| 外部依赖波动 | 真实远端不可用 | 加缓存镜像或 mock server |
| 用例耗时长 | 多步骤串行 | 可并行运行（`--test-threads`）+ DSL 内部复用克隆结果 |

**实施顺序**
1. 定义 `PipelineSpec`（steps: clone, modify, commit, push, fetch）。
2. 迁移完整流水线场景。
3. 迁移只读场景。
4. 迁移错误边界场景。
5. 去重/优化耗时。

### 12.16 legacy_cleanup_and_removal（清理阶段）

（更新：原“legacy_and_impl_integration（可选）” 改为 **明确执行的清理阶段**，目标从“对比差异”转为“彻底移除已无价值的 legacy 占位测试与相关实现残留”。）

**背景**  
前序 12.1~12.15 已将散落测试聚合并以占位文件保留 git blame ；占位文件与旧实现代码路径（若仍存在）会继续带来：
- 目录噪音 / 认知负担；
- IDE 全局搜索结果膨胀；
- 可能阻塞未来结构化事件或错误枚举重构时的“全量搜索替换”。

**目标范围**  
1. 移除所有仅含占位注释 + `assert!(true)` 的 legacy 测试文件。  
2. 移除与这些 legacy 测试唯一绑定的旧实现/辅助函数（若生产代码中仍暴露但不被聚合测试引用）。  
3. 清理文档中不再需要的 legacy cross-ref 说明（保留必要里程碑记录即可）。  
4. 若删除文件会导致覆盖率轻微波动，记录一次基线对比（预期影响极小）。

**不做的事项**  
- 不再实现差异对比/性能对照测试（价值已被聚合阶段消化）。  
- 不新增 snapshot；不引入新 DSL。  

**产出**  
- 一个提交：移除 legacy 占位测试文件 + 清理无引用辅助代码 + 更新本方案文档修订记录（例如 v1.10）。

**修订记录补充**  
- v1.14: Phase 1 完成（Tag/Remote/Refname 聚合）：新增 `git/git_tag_and_remote.rs`（~34 测试，<430 行），原 `git_tag_remote.rs` / `git_tag_remote_extra.rs` / `refname_validation.rs` 改占位；更新路线图状态表与聚合文件头部 Metrics/Source Mapping 注释。

**执行步骤**  
1. 扫描 `src-tauri/tests` 中匹配模式：`git_*` / `events_*` / 其它已聚合的旧文件，识别“仅包含头部注释 + 单个 trivially true 测试”者。  
2. 交叉引用聚合文件，确认无仍依赖的公共 helper（如有保留则移动 helper 至 `common/` 并删文件主体）。  
3. 删除匹配的 legacy 文件。  
4. 生产代码路径（如果曾保留兼容旧事件/旧枚举的分支）：移除 `#[allow(dead_code)]` / 条件编译分支。  
5. 运行 `cargo test -q` 与（可选）覆盖率工具，记录与前一次差异（只需行覆盖 delta 摘要）。  
6. 文档：
  - 增加修订记录条目（v1.10）描述“legacy cleanup finish”。
  - 路线图 13 节：12.16 标记 `Completed (Cleanup)`。
7. （可选）开 Issue：最终删除窗口（例如再过 N 周）是否还需保留某极少数文件；若无则关闭 Issue。

**验收标准**  
| 指标 | 期望 |
|------|------|
| Legacy 测试文件 | 全部删除或被合并（无孤立占位） |
| 生产端遗留分支 | 无“仅为 legacy”存在的死代码分支 |
| 测试通过 | 删除后 `cargo test` 全绿 |
| 文档同步 | 修订记录 + 路线图表更新 |

**风险与缓解**  
| 风险 | 描述 | 缓解 |
|------|------|------|
| 误删仍被引用 helper | 某占位文件内残留少量仍被引用函数 | 预先 `rg` / IDE 全局引用确认；必要时抽出迁移 |
| 覆盖率下降误判 | 删除占位触发覆盖率小幅下降 | 记录 delta 并注明“纯占位删除”来源 |
| merge 冲突 | 与并行分支仍引用 legacy | 先合并 main，公告删除计划窗口 |

**后续动作（完成后）**  
- 进入事件结构化（未来 12.17 或“事件回溯”阶段）时，搜索范围更干净；
- README/贡献指南如仍提及 legacy 测试的段落同步清理。

---

## 13. 路线图完成状态标记
（实施时使用清单同步进度，可在 PR 描述中引用此节）
| 阶段 | 文件 | 状态 | 备注 |
|------|------|------|------|
| 12.1 | git_init_and_repo_structure | Completed (Refined) | 已聚合：新增 common/{fixtures,test_env,git_helpers}；统一错误分类断言（git_helpers）；添加 section_preflight；原文件占位；Post-audit: header 规范化、legacy 占位补充计划引用 |
| 12.2 | git_add_and_commit | Completed (Refined) | 聚合 git_add/git_add_enhanced/git_commit/git_commit_task；sections(add_basic/add_edge/commit_basic/commit_edge/task_wrapper)；扩展 fixtures(stage_files,list_index)；补充 commit 作者 email-only 负例；原文件占位；测试全绿；Post-audit: add_edge 使用统一错误断言、section 文档补充；本次审查：统一 header cross-ref、确认 test_env::init_test_env 调用策略（重复调用由 Once 防抖，不再上移全局宏） |
| 12.3 | git_branch_and_checkout | Completed (Refined) | 聚合 git_branch_checkout；新增 repo_factory (repo_with_branches/current_branch/is_head_detached)；统一错误分类（git_helpers）；分区建立；删除/dirty/detached 场景占位；Post-audit: 添加未来扩展 TODO 注释；本次审查：补充 Cross-ref 行、保持占位结构一致、确认不额外抽象 current_branch 以避免过早封装；Post-audit(v2): 标注 12.11 前补齐 delete/dirty/detached 并抽象 current_branch helper | 
| 12.4 | git_clone_core | Completed (Refined) | 聚合并迁移参数/预检/组合/取消基础场景；新增 CloneParams(depth/filter 预留) 与 run_clone 封装；legacy 文件占位；下一步转 shallow/depth；Post-audit: 取消测试断言收紧、section 文档化；本次审查：清理未使用导入、准备与 shallow 共用 depth helper（rev_count 已抽出）；Post-audit(v2): 添加 Cross-ref，计划 12.8 引入 filter+depth 枚举化 & 事件 DSL | 
| 12.5 | git_clone_shallow_and_depth | Completed (Refined) | 聚合 shallow/depth/invalid/local-ignore/file-url 占位；新增 shallow_matrix；legacy 文件占位；初版 deepen/invalid/ignore 覆盖；新增通用 helper(rev_count/path_slug/shallow_file_lines)；deepen 断言加入 shallow 文件行数宽松校验；待 12.7 抽取 deepen 专用 helper & 引入事件 DSL；本次审查：path_slug 全量替换临时目录命名、header cross-ref 规范、记录未来与 fetch 共享 deepen 逻辑计划；Post-audit(v2): 标记与 12.7 fetch ignored 逻辑合并计划 | 
| 12.6 | git_clone_partial_filter | Completed (Refined) | 聚合 partial clone capability / filter_event / filter_depth / fallback；新增 partial_filter_matrix；legacy 文件占位；当前仅事件存在性与 fallback 模拟，待 12.8 引入真实 capability & 事件 DSL 收紧；本次审查：提取 build_params_for_case helper、扩展 fallback 注释（模拟规则 & 未来 SupportLevel 枚举）、计划与 fetch 阶段共享枚举扩展；二次完善：添加 test_env 初始化调用、增加（宽松）filter:* 事件标记检查（无则警告不失败），确保用例语义更清晰并为将来真实能力接入预留断言挂点；Post-audit(v2): 标记 fallback 布尔将升级为 SupportLevel 枚举；Post-audit(v3): 引入 partial_filter_support::{SupportLevel, assess_partial_filter}，fallback 布尔替换为枚举 Unsupported 判定；partial_filter_matrix 增加 PartialFilterOp 占位 (Clone)，为 12.8 fetch 聚合预留；legacy files now replaced with placeholders (v1.4 verification) | 
| 12.7 | git_fetch_core_and_shallow | Completed (Refined) | 聚合 fetch 基础+shallow（sections: basic/shallow/deepen/invalid/ignore）; run_fetch 占位 (FetchOutcome)；legacy 占位；Post-audit(v4): 头部统一 + 计划接入真实 shallow 对象计数；本次增量：准备与 tag DSL 对接（未改动此文件以保持稳定基线）。 |
| 12.8 | git_fetch_partial_filter | Completed (Refined) | 聚合 10 文件-> sections(capability/filter_variants/filter_depth/invalid/fallback)；SupportLevel 扩展 Invalid；warn_if_no_filter_marker；Post-audit(v3): depth 组合未来对象计数断言；本次增量(v4): 引入 tagify + expect_tags_subsequence 在 depth 场景测试 fetch 标签锚点。 |
| Pre-12.9 Infra | Infra Completed | partial_filter_matrix 扩展 Fetch；retry_matrix + compute_backoff_sequence；event_assert 初版 expect_subsequence。 |
| 12.9 | git_push_and_retry | Completed (Refined) | 聚合 push/retry；run_push_with_retry 占位；retry_matrix & expect_subsequence；Post-audit(v3): attempt/result 最小锚点；本次增量(v4): tag DSL (Attempt/result) 并行断言，保留旧锚点回退。 |
| 12.10 | git_strategy_and_override | Completed (Refined) | 聚合 strategy/http_override/adaptive_tls；http_override_stub: invalid_max；Post-audit(v2): StrategyPolicy 占位；本次增量(v3): tag DSL for http & tls 锚点；Post-audit(v4): strategy_combo / invalid_max 增补标签子序列锚点。 |
| Phase 3 | strategy_tls_override_consolidation | Completed (Phase3) | v1.16: 新增 6 sections；迁移 12 root-level 文件；`git_impl_tests.rs` 剪裁累计≈70%；新增 Phase3 Metrics；关键词基线记录 | 
| Phase 4 | props_and_final_impl_prune | Completed (Phase4) | v1.17: 属性测试集中到 quality；`git_impl_tests.rs` 最终迁移占位；root-level 业务测试=0；剪裁累计≥90% | 
| 12.11 | git_preconditions_and_cancel | Completed (Refined) | 聚合 preconditions/cancellation/timeout 模拟；OutcomeKind 占位；Post-audit(v2): 计划接入 mock clock；本次增量(v3): tag DSL + negative 断言（取消后无 success/completed）；Post-audit(v4): 引入统一终态互斥 helper + precondition/timeout 标签锚点。 |
| 12.12 | events_structure_and_contract | Completed (Refined) | 聚合结构/契约；最小 JSON snapshot；Post-audit(v1): 计划 insta/宽松字段；本次增量(v2): schema_version 占位 + tag 序列(Task->Policy->Transport->Strategy)；Post-audit(v3): 增加 snapshot 行级 JSON parse + id 唯一性校验。 |
| 12.13 | events_task_lifecycle_git | Completed (Refined) | 聚合 git/fetch/push/sleep 生命周期成功/失败/取消/指标；新增模拟 simulate_lifecycle + tag DSL (task/cancel/metric)；替换原 4 个文件为占位；Post-audit(v1): 后续接入真实结构化事件枚举与统一 TaskStatus 分类；Post-audit(v2): 使用终态互斥 helper 替换重复终态检查 (success/fail/cancel)。 |
| Legacy Sweep ≤12.13 | placeholders | Completed | All legacy scattered source files for 12.9~12.13 converted to minimal placeholders (git_push*, git_retry_override*, git_strategy_override*, git_http_override*, events_structured_*, events_contract_snapshot, events_no_legacy_taskerror, events_task_lifecycle_* variants) ensuring git blame retention before final removal window. |
| 12.14 | error_and_i18n | Completed (Refined) | 聚合 error_i18n_map；新增 quality/error_and_i18n.rs 分区（error_mapping/i18n_basic/fallback/integration）; v1.7 增补：AppErrorKind 临时枚举 + category bridge + locale key 存在性与 fallback 测试；原文件改占位；后续：扩展 Timeout/Permission 等 Kind 与 integration 互斥覆盖。 |
| 12.15 | e2e_public_git | Completed (Refined) | 本地离线真实 git 流水线：run_pipeline_with + 裸仓库 fixture + commit_count 前后校验 + ForcePushFailure 故障注入；legacy 公网用例保持占位；下一步：结构化事件接入 (12.17+ 或并入事件阶段回溯) |
| 12.16 | legacy_cleanup_and_removal | Completed (Cleanup) | 已删除全部 legacy 占位测试与死代码；v1.11 修订记录存档；root-level sweep 2/2 (v1.12) 完成 |
| Phase 1 | git_tag_and_remote | Completed (Phase1) | v1.14: 聚合 Tag/Remote/Refname；~34 tests, <430 lines；原 git_tag_remote* & refname_validation 改占位；测试绿灯 |

---

## 附录 A：剩余 root-level 零散测试文件模块化整合方案（规划）

### A.1 范围与目标
清理完成后仍保留的 root-level 测试文件（未位于 `git/`, `events/`, `quality/`, `e2e/`, `common/`, `support/` 子目录）包含真实行为/属性测试，不属于简单占位。本附录制定其最终模块化归并计划，目标：
1. 去除 root-level 直挂测试文件（除非故意保留为高层集成 / 属性集中区）。
2. 降低策略/任务/标签远程/属性测试分布碎片度。
3. 限制新增聚合模块 ≤2（要求允许）并最大化复用既有公共 helper。

### A.2 现存 root-level 文件分组
| 分组 | 文件示例 | 现状问题 | 聚合策略摘要 |
|------|---------|---------|--------------|
| Strategy Override 行为/结构化事件 (Clone/Fetch/Push/TLS/Retry/Conflict/Gating) | `git_strategy_override_*`, `strategy_override_*`, `git_tls_*`, `git_impl_tests.rs`(部分进度) | 主题集中但细粒度多，事件/策略断言分散 | 合并/拆分并入既有 `git/git_strategy_and_override.rs`（新增 sections）与 `git/git_push_and_retry.rs`（仅 retry/backoff 相关，如果有补充）|
| Tag & Remote 核心与扩展 | `git_tag_remote.rs`, `git_tag_remote_extra.rs`, `refname_validation.rs` | Tag/Remote & 名称验证散列 | 新建模块 `git_tag_and_remote.rs`（新增 1 个新聚合文件）|
| 任务框架 (TaskRegistry / Sleep / 并发 / Cancel 边界) | `task_integration.rs`, `task_registry_edge.rs`, `task_registry_extra.rs`, `task_registry_post_complete_cancel.rs`, `git_tasks.rs`, `git_tasks_local.rs` | 任务生命周期/取消/并发/本地 git service 行为割裂 | 新建模块 `tasks/task_registry_and_service.rs`（新增 1 个新聚合文件）|
| 属性测试（策略/部分 Filter/Retry/TLS） | `prop_*` 系列, `prop_tls_override.proptest-regressions` | 已按主题命名，属性测试风格不同于场景测试 | 统一迁移到 `quality/strategy_and_filter_props.rs`（复用 quality 语义，不新增模块； regression 种子文件保留同目录）|
| Impl 级 GitService 行为（clone/fetch 基础取消/进度） | `git_impl_tests.rs` | 与任务/策略存在重叠，部分用例偏向 service-level | 拆分：进度/取消相关 -> `tasks/task_registry_and_service.rs`; 纯 service clone/fetch 基本语义 -> 已在 git_* 聚合中则删除/裁剪重复 |

### A.3 新增模块定义（≤2）
1. `src-tauri/tests/git/git_tag_and_remote.rs`
   - sections:
     - `mod section_tag_lightweight;`（创建/force/重复/CRLF/规范化）
     - `mod section_tag_annotated;`（消息归一化/force 对象 OID 变化/同内容 OID 保持）
     - `mod section_remote_lifecycle;`（add/set/remove/duplicate/idempotent/cancel）
     - `mod section_remote_validation;`（非法 name / URL / 空白 / 控制字符）
     - `mod section_refname_rules;`（整合 `refname_validation.rs` 案例，branch/tag/remote wrapper 一致性）
   - 公共抽象：复用现有 `git_helpers` 错误分类；新增辅助 `assert_ref_updated(repo, ref, expect)`（如需要）。

2. `src-tauri/tests/tasks/task_registry_and_service.rs`
   - sections:
     - `mod section_registry_lifecycle;`（create/run/list/snapshot 克隆行为）
     - `mod section_registry_cancel;`（pre-start cancel / running cancel / post-complete cancel 幂等）
     - `mod section_registry_concurrency;`（高并发 sleep / 部分取消混合）
     - `mod section_registry_edge;`（未知 id 操作、idempotent cancel、snapshot 独立克隆语义）
     - `mod section_service_progress;`（来自 `git_impl_tests.rs` 的 Negotiating / Completed 进度锚点）
     - `mod section_service_cancel_fast;`（立即 cancel flag/fetch cancel 映射）
   - 公共抽象：提炼 `wait_for_state` / `spawn_and_wait` 到 `tests/common/fixtures.rs` 或新建 `tests/common/registry.rs`（若行数>~120 再评估拆分）。

### A.4 既有文件扩展（不新增模块）
| 目标聚合文件 | 追加 sections | 来源文件内容摘要 |
|---------------|---------------|------------------|
| `git/git_strategy_and_override.rs` | `section_http_basic`、`section_http_limits`、`section_http_invalid_max`、`section_http_events`、`section_strategy_summary` | 来自 `git_strategy_override_*` 系列：HTTP override 覆盖、事件序列、summary applied codes；TLS override 系列已在 Real Host 校验落地后删去 |
| `git/git_push_and_retry.rs` | `section_strategy_retry_summary_cross`（若需） | 如果属性/策略 summary 里存在与 retry 交叉的附加断言；否则保持现状 |
| `quality/error_and_i18n.rs` (或新 `quality/strategy_and_filter_props.rs`) | `mod section_strategy_props;` `mod section_retry_props;` `mod section_partial_filter_props;` | 来自 `prop_strategy_http_override.rs`, `prop_retry_override.rs`, `prop_strategy_summary_codes.rs`, `prop_partial_filter_capability.rs`（TLS override 属性测试已退役，仅保留历史 seeds） |

决定：属性测试单独文件过大风险（>800 行）→ 采用新文件 `quality/strategy_and_filter_props.rs`（不计入新增模块限制吗？要求限制“新建测试模块”≤2，本方案将其视作“拆分质量属性集”可选；若严格限制==2，则回退：将属性 sections 合并进现有 `error_and_i18n.rs` 末尾。最终实施时依据行数评估，文档先记录双案：
 - Primary: 新建第三文件（若策略允许）
 - Fallback: 合并入现有 quality 聚合文件

（若用户严格执行“最多两个”，执行时采用 Fallback 方案）

### A.5 文件级映射表
| 原文件 | 动作 | 目标文件 / 新模块 | 目标 section / 处理说明 |
|--------|------|------------------|-------------------------|
| git_strategy_override_no_conflict.rs | 合并 | git/git_strategy_and_override.rs | section_override_no_conflict_http_tls |
| git_strategy_override_structured.rs | 合并 | git/git_strategy_and_override.rs | section_strategy_summary_multiop（含 structured events） |
| git_strategy_override_summary_fetch_push.rs | 合并 | git/git_strategy_and_override.rs | section_strategy_summary_multiop（扩展 fetch/push summary 验证） |
| git_strategy_override_tls_combo.rs | 合并 | git/git_strategy_and_override.rs | section_tls_mixed_scenarios（combo） |
| git_strategy_override_tls_mixed.rs | 合并 | git/git_strategy_and_override.rs | section_tls_mixed_scenarios（mixed variants） |
| git_strategy_override_tls_summary.rs | 合并 | git/git_strategy_and_override.rs | section_summary_gating（gating=1/0 appliedCodes） |
| git_tls_override_event.rs | 合并 | git/git_strategy_and_override.rs | section_tls_mixed_scenarios（变化/不变化事件） |
| git_tls_push_insecure_only.rs | 合并 | git/git_strategy_and_override.rs | section_tls_mixed_scenarios（push insecure only） |
| strategy_override_empty_unknown_integration.rs | 合并 | git/git_strategy_and_override.rs | section_override_empty_unknown |
| strategy_override_invalid_integration.rs | 合并 | git/git_strategy_and_override.rs | section_override_invalid_inputs |
| strategy_override_push.rs | 合并 | git/git_strategy_and_override.rs | section_strategy_summary_multiop（push 特化） |
| strategy_override_summary.rs | 合并 | git/git_strategy_and_override.rs | section_strategy_summary_multiop（http+retry appliedCodes） |
| git_tag_remote.rs | 新建 | git/git_tag_and_remote.rs | section_tag_lightweight / section_remote_lifecycle（拆分） |
| git_tag_remote_extra.rs | 新建 | git/git_tag_and_remote.rs | section_tag_annotated / section_remote_validation / force 细粒度 |
| refname_validation.rs | 新建 | git/git_tag_and_remote.rs | section_refname_rules |
| git_tasks.rs | 新建 | tasks/task_registry_and_service.rs | section_service_progress / section_service_cancel_fast（拆分） |
| git_tasks_local.rs | 新建 | tasks/task_registry_and_service.rs | section_service_progress（本地成功 clone/fetch） |
| task_integration.rs | 新建 | tasks/task_registry_and_service.rs | section_registry_lifecycle / section_registry_concurrency |
| task_registry_edge.rs | 新建 | tasks/task_registry_and_service.rs | section_registry_edge |
| task_registry_extra.rs | 新建 | tasks/task_registry_and_service.rs | section_registry_concurrency / section_registry_cancel |
| task_registry_post_complete_cancel.rs | 新建 | tasks/task_registry_and_service.rs | section_registry_cancel（post-complete cancel） |
| git_impl_tests.rs | 拆分/裁剪 | tasks/task_registry_and_service.rs + 既有 git_* | 进度/取消 -> service sections；重复基础 clone/fetch 验证若已覆盖则删除重复段 |
| prop_strategy_http_override.rs | 合并 | quality/strategy_and_filter_props.rs (Primary) 或 quality/error_and_i18n.rs(Fallback) | section_strategy_props |
| prop_retry_override.rs | 合并 | 同上 | section_retry_props |
| prop_strategy_summary_codes.rs | 合并 | 同上 | section_strategy_props（applied codes consistency） |
| prop_partial_filter_capability.rs | 合并 | 同上 | section_partial_filter_props |
| prop_tls_override.rs | 已退役 | - | TLS override 策略入口废弃后删除；由 Real Host 校验场景取代 |
| prop_tls_override.proptest-regressions | 保留原样（归档） | 同上所在目录 | 仅作为历史种子存档，不再被测试框架自动加载 |

### A.6 迁移实施顺序建议
1. 引入新模块骨架 (`git_tag_and_remote.rs`, `tasks/task_registry_and_service.rs`) + 空 sections。
2. 优先迁移 Tag/Remote（无并发依赖）。
3. 迁移 TaskRegistry & GitService（提炼通用 wait helpers → `common/fixtures.rs`）。
4. 迁移 / 合并 Strategy override 系列到已存在聚合文件（阶段化：先事件/summary，再 invalid/empty/unknown，再 TLS mixed）。
5. 整合属性测试：若新增 quality props 文件被接受则创建；否则合并进 `error_and_i18n.rs` 末尾，并以 `// --- props begin ---` 标记折叠块。
6. 剪裁 `git_impl_tests.rs` 中重复（已在聚合文件覆盖）场景，保留 service 特有（Negotiating phase, fast-cancel edge）。
7. 回归测试与行数审计（新文件 <800 行；策略聚合文件如超标再考虑拆分子 module）。

### A.7 风险与缓解
| 风险 | 缓解 |
|------|------|
| 新增 section 使 `git_strategy_and_override.rs` 超过 800 行 | 分拆 `strategy_override_extended.rs`（备用方案，不在本阶段执行）|
| 属性测试合并导致质量文件过大 | 回退到 Fallback（不创建第三个模块）|
| TaskRegistry 辅助函数重复 | 抽象进 `common/fixtures.rs` / `common/registry.rs` 并复用 |
| 重复断言迁移时语义漂移 | 迁移前列出原断言消息与关键字段，迁移后运行 diff（grep 关键短语）|
| proptest seeds 丢失 | 保持 `*.proptest-regressions` 原路径并在新 props 文件注释引用 |

### A.8 验收标准（本附录落实后）
| 指标 | 目标 |
|------|------|
| root-level 业务测试文件数 | 0（仅允许 seeds 文件）|
| 新增聚合模块数 | ≤2（不含可选 props 合并方案）|
| 重复删除 | 删除或裁剪 ≥90% `git_impl_tests.rs` 与 git_* 聚合重复段 |
| 可读性 | 每新增模块 ≤750 行；sections 清晰、顶部来源映射注释完整 |
| 属性测试集中度 | 所有 proptest 用例集中至单一 quality 文件（primary 或 fallback）|

### A.9 后续可选优化
- 若 Strategy 聚合继续增长：将 TLS / HTTP / Retry overrides 分拆成次级模块并通过 `mod` + re-export 保持对外单文件入口。
- 引入统计脚本生成“断言来源矩阵”帮助回归验证迁移完整性。
- 为 TaskRegistry 提供统一 async 测试宏（减少状态轮询样板）。

（附录 A 结束）

## 附录 B：模块化整合实施路线图（四阶段）

> 版本：v1.13 新增（Roadmap Added）。本附录将附录 A 的文件级映射（A.5）与迁移顺序建议（A.6）具体化为四个可执行阶段，以便分批合并并在每一阶段保持主分支稳定与测试绿灯。若任一阶段出现超范围风险（行数/时间/冲突）可提前切分为微里程碑 Bx.y。

### B.0 范围回顾与原则
1. 不新增超过 2 个正式聚合模块（Tag&Remote / Tasks&Service）。
2. 属性测试整合遵循“Primary 若被拒则回退 Fallback”策略，不阻塞主干。
3. 每阶段结束：`cargo test -q` 全绿 + 目标 root-level 文件进入 0 或下降至下一阶段输入集。
4. 严格保持断言语义：迁移仅做结构/命名调整，不修改预期值（除非发现重复冲突→记录并统一）。

### 阶段概览
| 阶段 | 名称 | 输入文件集合（来自 A.5） | 主要输出 | 完成后 root-level 剩余 |
|------|------|---------------------------|----------|-------------------------|
| Phase 1 | 基础骨架与 Tag/Remote 迁移 | `git_tag_remote.rs` `git_tag_remote_extra.rs` `refname_validation.rs` | 新文件 `git/git_tag_and_remote.rs` (含所有空/基础 section + 迁移内容) | 去除 Tag/Remote 三文件 |
| Phase 2 | Tasks & Service 聚合 | `task_integration.rs` `task_registry_*` `git_tasks*.rs` `git_impl_tests.rs`(进度/取消部分) | 新文件 `tasks/task_registry_and_service.rs` + 抽象 helpers | 仅剩 strategy/prop 系列 + `git_impl_tests.rs` 剩余基础片段 |
| Phase 3 | Strategy/TLS/Override 合并 | 全部 `git_strategy_override_*` `strategy_override_*` `git_tls_*` + `git_impl_tests.rs` 重复段 | 扩展 `git/git_strategy_and_override.rs` 新 sections；裁剪 `git_impl_tests.rs` | 仅余 prop_* + `git_impl_tests.rs` 剩余 service 特有（如仍保留） |
| Phase 4 | 属性测试集中与最终剪裁 | `prop_*` 全部 + `git_impl_tests.rs` 残留 | 归并 props (Primary: 新 `quality/strategy_and_filter_props.rs` | Fallback: 合并入 `error_and_i18n.rs`)；删除或最小化 `git_impl_tests.rs` | root-level 仅保留 `prop_tls_override.proptest-regressions` |

### Phase 1 详细：Tag & Remote 整合
目标：引入第 1 个新聚合模块并迁移相关 3 个文件，建立 section 骨架以支持未来扩展（force/refname 规则）。
任务清单：
1. 创建 `git/git_tag_and_remote.rs`，添加文件头注释（来源、版本、关联 A.5 条目列表 subset）。
2. 定义空 sections：tag_lightweight / tag_annotated / remote_lifecycle / remote_validation / refname_rules。
3. 迁移对应测试：按语义分配至 sections，保持原测试函数名（可前加前缀如 `lw_`, `ann_`, `remote_`, `ref_` 以防冲突）。
4. 提炼重复断言（如存在）到 `git_helpers` 或本文件内部私有函数（不跨文件先，不破坏稳定）。
5. 删除原 3 个 root-level 文件。
验收指标：
 - root-level Tag & Remote 文件=0。
 - 新文件行数 < 500。
 - 所有原断言关键字 (`tag`, `remote`, `refname`) grep 仍可找到至少一处。
风险与缓解：
 - 名称冲突：使用 section 前缀重命名测试函数。
 - 重复逻辑 premature 抽象：阶段内仅本地私有函数，不上移 common。

### Phase 2 详细：Tasks & Service 聚合
目标：引入第 2 个新聚合模块；集中 TaskRegistry 生命周期、并发与 GitService 进度/取消；初步剪裁 `git_impl_tests.rs`。
任务清单：
1. 创建 `tasks/task_registry_and_service.rs` 文件 + 头部注释（列出来源 A.5 条目）。
2. 迁移 `task_integration.rs` / `task_registry_*` / `git_tasks*.rs`。
3. 从 `git_impl_tests.rs` 挑选“进度标签/Negotiating/Completed anchors/fast-cancel”测试迁入 `section_service_progress` & `section_service_cancel_fast`。原文件保留其余暂不动。
4. 提炼公共等待/轮询：`wait_for_state`, `spawn_and_wait`。若两者 + 相关辅助 > 120 行，才考虑放至 `common/registry.rs`（否则内联保持局部性）。
5. 删除迁移后的 root-level task* 与 git_tasks* 文件。
验收指标：
 - root-level 任务相关文件=0。
 - 新聚合文件行数 < 600。
 - `git_impl_tests.rs` 行数较初始剪裁 ≥30%。
风险与缓解：
 - 并发 test flakiness：增加最大等待时间 + 指数回退轮询；必要时使用 deterministic fixture（若已有 test_env 提供）。
 - 轮询 helper 不稳定：保留超时日志上下文（包含 task id & state snapshot）。

### Phase 3 详细：Strategy/TLS/Override 合并
目标：清空所有 strategy_override / tls / override scattered 文件；扩展已有 `git/git_strategy_and_override.rs`。
任务清单：
1. 依序迁移（A.6 建议）：
  a. summary / structured / push & fetch 组合 -> section_strategy_summary_multiop。
  b. invalid / empty / unknown -> 对应 sections。
  c. tls mixed / event variation / insecure only -> section_tls_mixed_scenarios。
  d. gating summary -> section_summary_gating。
2. 对新增 sections 添加头部注释块：列出来源文件名 & 原测试函数映射（如函数名改动需列表）。
3. 迁移后删除所有对应 root-level 文件。
4. 再次剪裁 `git_impl_tests.rs`：删除已被聚合覆盖的基础 clone/fetch/push/tls 场景。
5. 若文件行数 > 800：预留 TODO 标记（不在本阶段再拆）。
验收指标：
 - root-level strategy/tls/override 文件=0。
 - `git/git_strategy_and_override.rs` 新增 sections 全部含来源注释。
 - `git_impl_tests.rs` 剩余仅 service 特有（Negotiating 已迁出）或暂存 TODO。
风险与缓解：
 - 行数膨胀：内部按 section 注释折叠 + 后续 B.5 优化预留；暂不新建额外文件。
 - 断言漂移：迁移前后使用 grep 比对关键短语（例如 appliedCodes, override, tls, insecure）。

### Phase 4 详细：属性测试集中与最终剪裁
目标：统一 proptest 用例；最终裁剪 `git_impl_tests.rs`（若仍有可替代重复）。
任务清单：
1. 评估属性用例总行数（除 seeds）。
2. Primary 路径：创建 `quality/strategy_and_filter_props.rs`，分 section (strategy/retry/partial_filter/tls)。
3. 若 Primary 违反“新增模块≤2”或评审拒绝：Fallback → 将 sections 追加至 `quality/error_and_i18n.rs` 末尾，用 `// --- props begin ---` 与 `// --- props end ---` 包裹。
4. 保留 `prop_tls_override.proptest-regressions` 原位置（只改注释指向新文件）。
5. 删除所有 `prop_*.rs` root-level 测试文件。
6. 最终审视 `git_impl_tests.rs`：若仅剩冗余基础 clone/fetch（已完全覆盖）则删除；如还有暂不覆盖 service 深层语义测试，可最小化（头部补充 TODO & 来源说明）。
验收指标：
 - root-level 业务测试文件数=0。
 - 属性测试集中度=100%。
 - 所有 proptest 用例仍通过（无 flake 新增）。
风险与缓解：
 - proptest flakiness：限定 cases/timeout；保持回归 seeds；新增注释说明如何重放失败案例。
 - 文件过大：Fallback 合并模式减少新增文件。

### B.1 阶段里程碑度量
| 指标 | Phase 1 目标 | Phase 2 目标 | Phase 3 目标 | Phase 4 目标 |
|------|--------------|--------------|--------------|--------------|
| 新增/修改文件数 (测试) | +1 新增, -3 删除 | +1 新增, -6~-7 删除 | 0 新增, -10~-12 删除 | +0 或 +1 新增, -5 删除 |
| root-level 剩余文件数 | 其余保持 | 下降到 Strategy+Props+impl | 仅 Props+(impl 残留) | 仅 seeds |
| `git_impl_tests.rs` 剪裁累计 | 0% | ≥30% | ≥70% | ≥90% 或删除 |
| 新增聚合模块累计 | 1 | 2 | 2 | 2 (若 Primary 则 3*) |
| 属性测试集中度 | 不变 | 不变 | 不变 | 100% |

*若出现第 3 个文件（Primary 路径），需在提交信息注明“超出≤2限制已获批准”或回退 Fallback。

### B.2 风险矩阵（跨阶段）
| 风险 | 触发阶段 | 影响 | 缓解 |
|------|----------|------|------|
| 行数超阈值 (≥800) | 2/3 | 可读性下降 | 添加 TODO + 后续重构拆分计划，不阻塞当前阶段 merge |
| Flaky 并发测试 | 2 | CI 不稳定 | 指数回退等待 + 日志快照 + 可配置重试（仅本地）|
| 重复断言未清除 | 3/4 | 维护成本 | Phase 3、4 结束各执行一次 grep 关键短语计数对比 |
| 断言语义意外改变 | 全程 | 隐性回归 | 保留原函数名/注释；对关键标签使用 expect_subsequence DSL 保序 |
| proptest 时间过长 | 4 | CI 时间上升 | 限定 cases（通过 ENV 参数）并在 README/注释说明如何 full-run |
| 种子文件误删 | 4 | 难以重现 | git add 前 grep "proptest-regressions" 确认存在 |

### B.3 与附录 A 的交叉引用
- 文件映射：详见 A.5。
- 迁移顺序：详见 A.6（Phase 1-4 分别对应 A.6 步骤 1~7 的分段打包）。
- 验收指标：A.8 指标按阶段拆分为 B.1 度量行。

### B.4 执行建议
1. 每阶段独立提交（Conventional Commits）：`test(git): add tag_and_remote aggregate (phase1)` 等；提交信息中文说明迁移来源列表。
2. Phase 内如需多次提交，首个提交引入骨架，其后迁移；避免一次极大 diff。
3. Phase 3 大量合并前先在分支本地执行 grep 基线（记录 appliedCodes/tls/insecure/override 计数字符串）。
4. Phase 4 属性集中之前先统计 prop_* 行数；若 >900 直接走 Fallback。
5. 每阶段结束更新本文件“路线图完成状态标记”表（13 节）添加 Phase X Completed 备注。未来可将阶段状态并入CI概览徽章。

### B.5 后续增强（非本路线图范围）
- 把 strategy/tls 额外增长的 sections 拆至 `git/strategy_override_extended/` 子目录并用 `pub mod` re-export。
- 引入脚本：统计 test 函数数目、行数、断言关键字分布（appliedCodes/tls_override/partial_filter）。
- 提供统一 `assert_event_sequence!(...)` 宏替换 expect_subsequence + 手写数组。
- 为 registry 并发测试引入 deterministic 调度（如基于 tokio::time::pause + manual advance）。

（附录 B 结束）

