# MP0 阶段细化路线图与开发计划（gitoxide → git2-rs 迁移）

> 本文将“新 MP0：从 gitoxide 全面迁移到 git2-rs（libgit2 绑定）”拆解为可执行的周迭代与任务清单，确保在保留前端 API/事件/任务模型不变的前提下，完成等价功能、可回滚与高质量交付。

---

## 0. 目标、范围与成功标准

- 目标
  - 在不改变前端命令/事件/状态模型的前提下，将后端 Git 实现从 gix 替换为 git2-rs。
  - clone/fetch 行为等价或更优，进度/取消/错误分类与现有前端契约兼容。
  - 完成依赖与代码迁移、测试替换、文档更新，可一键回滚。
- 范围
  - Git：Clone、Fetch 两路径（Smart HTTP）。
  - 事件：task://state、task://progress、task://error（可选）。
  - 取消：协作式取消不回退。
  - 错误分类：Network/Tls/Verify/Auth/Protocol/Cancel/Internal（按 MP1 预分类收敛）。
- 不做（MP0）
  - Push / 方式A subtransport / Retry v1 / 代理 / IP 优选 / LFS / Pin / TOFU / SSH。
- 成功标准（验收）
  - 单测全绿；在公开测试仓库上 clone/fetch 成功，进度显示与取消可用；日志脱敏；无明显性能回退。

---

## 1. MP0 分阶段与时间线（建议 2–3 周）

MP0 拆分为 4 个可验收的小阶段，确保每阶段可单独合入、可回滚且对前端零侵入：

### MP0.1 依赖与骨架（约 0.5 周）
- 范围：
  - 引入 `git2` 依赖与基础构建；新增 `git_impl` 特性位（`gix|git2`，默认 git2）。
  - 抽象 `core::git::service` 统一入口；定义 `ProgressPayload`、`ErrorCategory`。
  - 进度桥接/取消令牌接口对齐但仅放置占位（不接线）。
- 交付：能以 git2 编译通过；服务与事件桥接骨架编译通过；CI 绿色。
- 验收：
  - Build PASS（各平台 CI）；lint/type PASS。
  - 新增最小单测编译通过（无需真实 clone）。
- 回滚：版本回退；开发期可使用 `gix` 特性位进行对比（上线前移除）。

### MP0.2 Clone 基线（约 0.5–0.75 周）
- 范围：
  - 使用 git2-rs 实现 Clone；`RemoteCallbacks::transfer_progress` → 统一 `ProgressPayload`；
  - 协作式取消（回调检查 token）；错误分类初版（Network/Tls/Verify/Cancel/Internal）。
- 交付：公共小仓库 clone 成功；进度/取消/错误事件正常；日志脱敏。
- 验收：
  - 单测新增：成功/取消/网络错误/证书错误路径；
  - UI 端无需修改即可显示进度；
  - 性能不明显回退（以小样本为准）。
- 回滚：版本回退；开发期可切换 `gix` 路径做对照测试。

### MP0.3 Fetch 与一致性（约 0.5 周）
- 范围：
  - 基于现有仓库实现 Fetch；与 Clone 复用进度/取消/错误映射；
  - 完善错误分类与事件时序一致性（pending→running→completed/failed/canceled）。
- 交付：已有仓库 Fetch 成功；失败路径（超时/证书/取消）可复现并被分类。
- 验收：
  - 单测覆盖 Fetch 成功与失败；
  - 事件与前端契约保持兼容；
  - CI 绿色。
- 回滚：版本回退。

### MP0.4 切换、清理与基线（约 0.5–1 周）
- 范围：
  - 移除 `gix` 依赖与旧代码；关闭（并最终删除）`gix` 构建开关；确保 git2 唯一路径；
  - 测试与文档收尾：替换/补充用例、更新变更日志、在主方案标记 MP0 完成；
  - 性能与稳定性对比：与 gix 路径基线对比 3 个样本仓库（小/中），作为未来优化参考。
- 交付：主分支默认 git2；文档/变更日志更新；样本仓库对比数据记录。
- 验收：所有测试通过；手动冒烟通过；不再存在 `gix` 运行路径。
- 回滚：版本回退。

---

## 2. 技术方案拆解（MP0 视角）

### 2.1 依赖与构建
- Cargo.toml：`git2 = "0.19"`；如需，记录 Windows 上的 `vcpkg` 或预编译 libgit2 说明（仅文档）。
- Feature flag（可选）：`git_impl = ["gix"|"git2"]`；默认 git2。

### 2.2 模块与接口
- 目录：`src-tauri/src/core/git/{mod.rs, service.rs, git2_impl.rs, progress.rs, errors.rs}`
- 接口（保持不变）：
  - 命令：`git_clone(repo, dest, opts?)`、`git_fetch(repo, opts?)`、`task_cancel(id)`。
  - 事件：`task://state`（pending|running|completed|failed|canceled）、`task://progress`（objects/bytes/totalHint/percent/phase）。
- 进度桥接：`RemoteCallbacks::transfer_progress` → 统一 `ProgressPayload`。
- 取消：`CancellationToken` 注入到回调闭包，命中即提前返回中止。

### 2.3 错误分类与日志
- 分类映射：
  - `could not resolve`/`connection reset`/`timeout` → Network
  - `certificate`/`x509` → Tls/Verify
  - `HTTP 401/403` → Auth
  - `user canceled` → Cancel
  - 其他 → Protocol/Internal
- 日志脱敏：Authorization/Token 默认隐藏；错误带 `category` 与可选 `code`。

### 2.4 兼容性与回滚
- 兼容：payload 字段向后兼容；新增字段标记为可选。
 - 回滚：开发阶段可通过 gix 构建开关临时切换验证；不提供“系统 git”兜底；合并前清理为 git2 唯一路径。

---

## 3. 任务分解（WBS）

1) 准备（[MP0.1]）
- [ ] 引入 git2 依赖，编译通过（不接入调用）。
- [ ] 定义 `ProgressPayload` 与 `ErrorCategory`（沿用枚举）。
- [ ] 在 service.rs 中抽象 `GitService` trait（或等效统一入口）。

2) 实现 Clone（[MP0.2]）
- [ ] 初始化仓库：`Repository::clone_with(...)` 或 `Remote::connect` 自定义回调。
- [ ] 注册回调：transfer_progress、credentials（暂保持匿名/无 push）。
- [ ] 取消：回调定期检查 token；
- [ ] 事件：开始/进度/结束/失败；
- [ ] 错误映射：到 `ErrorCategory`；
- [ ] 小样本验证：成功/取消/网络错误/证书错误。

3) 实现 Fetch（[MP0.3]）
- [ ] 打开已有 repo，取 remote（origin或传入），`fetch(refspecs, Some(mut callbacks), None)`；
- [ ] 同 Clone 路径桥接进度与取消；
- [ ] 错误映射一致；
- [ ] 验证：已有仓库 fetch 成功与各失败路径。

4) 接口稳定与替换（[MP0.4]）
- [ ] 命令层保持签名；
- [ ] 移除 gix 依赖、删除未使用代码；
 - [ ] 彻底关闭并删除 `gix` 构建开关（仅在开发期使用过的临时对比开关）。

5) 文档与测试（[MP0.4]）
- [ ] 替换对应单测（mock git2 行为或使用临时仓库）；
- [ ] 更新 `new-doc/TECH_DESIGN_git2rs.md` 的 P0 章节为“完成”；
- [ ] 记录迁移变更日志（CHANGELOG.md）。

---

## 4. 迁移策略与回滚预案（面向 MP0）

- 双实现短期共存（仅本地调试）：
  - 通过 feature flag/环境变量切换实现；默认 git2；
  - 当 git2 出现平台构建问题时临时切回 gix；
- 合并到主分支前：
  - 清理 gix 依赖与代码；保持 git2 唯一路径；
  - 不提供“系统 git”路径；如需诊断使用临时 gix 构建开关与详细日志（仅开发期）。
- 回滚策略（发布后）：
  - 若出现致命问题，保持版本回退；
  - 文档保留 gix 的“开发者调试指南”（系统 git 相关路径不纳入方案）。

---

## 5. 测试计划（最小可行）

### 5.1 单元/集成
- Clone 成功：公共小仓库；
- Fetch 成功：已有 repo 更新路径；
- 取消：在大量对象时取消，任务进入 canceled；
- 错误映射：网络超时、证书错误、非 200 响应、用户取消；
- 事件时序：state(pending→running→completed/canceled/failed)、progress 连贯。

### 5.2 端到端（E2E，脚本/手动）
- 前端 Git 面板启动 clone，观察进度与取消；
- 大小不同的仓库（小/中）对比性能；
- 日志中敏感头不出现原文；
- Windows/macOS（如可）双平台冒烟。

---

## 6. 质量门禁与交付清单

- 质量门禁
  - Build: PASS（各平台 CI）
  - Lint/Typecheck: PASS
  - Unit/Integration: PASS
  - E2E 冒烟（手册）：PASS
  - 回滚预案：已记录
- 交付清单
  - 代码：git2-rs 实现 + 事件/取消/错误映射
  - 删除：gix 依赖与旧实现
  - 文档：本计划 + 主技术方案 P0 章节更新 + 变更日志
  - 测试：新增/替换用例与说明

---

## 7. 风险清单与缓解

| 风险 | 表现 | 缓解 |
|------|------|------|
| libgit2 平台差异 | Windows 构建/运行异常 | 预编译/说明文档/CI 预热 |
| 进度桥接不一致 | UI 百分比跳变 | 做平滑与阶段标记 |
| 取消不及时 | 网络读阻塞 | 回调频率与超时保护 |
| 错误分类模糊 | 难以定位 | 错误前缀与 code 归并表 |
| 性能回退 | clone 变慢 | 对比基线优化参数 |
| 第三方依赖变更 | 构建失败 | 锁定版本与 Renovate |

---

## 8. 对齐 MP1 的前置铺垫（可选）

- 在 MP0 中预留：
  - `RemoteCallbacks::credentials` 接口位置（但 P0 不启用）；
  - `push_transfer_progress` 的桥接骨架（占位）；
  - 错误分类包含 Auth（空路径）。
- 这样 MP1 引入 Push 时只需开启与补充，不改动 MP0 事件与模型。

---

## 附：变更记录（本文件）
- v1: 初版（MP0 细化拆解）
