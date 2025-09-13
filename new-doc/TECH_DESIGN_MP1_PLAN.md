# 技术方案（MP1 阶段路线图）——Push + 自定义 Subtransport(A) + Retry v1 + 事件增强

> 适用范围：以 MP0（git2-rs 迁移完成）为基线，保持前端 API/事件/任务模型不变，补齐 Push 能力，并灰度接入“方式A：自定义 smart subtransport（仅接管连接/TLS/SNI）”，同时引入统一重试（Retry v1）与事件增强；要求全部单测通过且可一键回退。

关联文档：
- 现状与总体设计（git2-rs 版本）：`new-doc/TECH_DESIGN_git2rs.md`
- 旧版 P0 交接稿：`doc/TECH_DESIGN_P0_HANDOFF.md`
- 旧版 P1 设计（历史语境）：`doc/TECH_DESIGN_P1.md`
- MP0 计划与交付：`new-doc/TECH_DESIGN_MP0_PLAN.md`

---

## 0. TL;DR

- 本阶段目标（MP1）：
  - MP1.1：打通 HTTPS Push（凭证回调、进度/取消/错误分类完善）；
  - MP1.2：灰度启用“方式A：自定义 smart subtransport”，仅接管连接与 TLS/SNI，保留自动回退；
  - MP1.3：引入统一 Retry v1（指数退避+类别化）；
  - MP1.4：事件增强（push 阶段化进度、标准化错误事件）。
- 不变：任务模型、命令/事件命名、前端现有 UI 和 Store 结构保持兼容；新增字段保持可选，前端容忍未知字段。
- 可回退：
  - Push 可通过功能开关关闭（后端停止暴露命令或返回“未启用”错误）；
  - 自定义 subtransport 默认关闭，可按仓/域白名单灰度；失败自动回退 libgit2 默认路径；
  - Retry v1 可通过配置禁用或调低阈值。

---

## 1. 范围与目标

| 子阶段 | 核心目标 | 对用户可见变化 |
|--------|----------|----------------|
| MP1.1 Push | 支持 HTTPS Push（PAT/用户名+令牌），进度、取消与错误分类完善 | 新增 push 按钮/表单（可后置），日志脱敏 |
| MP1.2 Subtransport(A) | 白名单域启用自定义 smart subtransport，仅接管连接/TLS/SNI，失败自动回退 | 默认为关闭，灰度开关后网络兼容性提升 |
| MP1.3 Retry v1 | 统一重试策略（指数退避+类别化），push 遵守“上传前可重试” | 失败时更稳健，错误消息包含重试计数 |
| MP1.4 事件增强 | push 阶段化进度与标准化错误事件（task://error） | UI 可显示更丰富进度/错误（保持兼容） |

不做（本阶段）：代理/IP 优选、浅克隆/部分克隆、LFS、SSH 兜底、指标面板等（详见 `new-doc/TECH_DESIGN_git2rs.md` 的后续阶段）。

---

## 2. 交付清单（Deliverables）

- 后端（Rust/Tauri，基于 git2-rs）：
  - 新增 git_push 命令；
  - Push 凭证回调（PAT / 用户名+令牌）；
  - Push 进度桥接（objects/bytesSent/percent/phase）；
  - 取消与错误分类（Auth/Network/Tls/Verify/Protocol/Cancel/Internal）；
  - 自定义 smart subtransport(A) 可插拔、白名单域灰度、失败自动回退；
  - 统一重试（Retry v1）：Clone/Fetch 遵守类别化重试，Push 仅在“进入上传前”允许重试；
  - 事件增强：`task://progress`（push 阶段化）与 `task://error`（可选）。
- 前端（保持兼容）：
  - 现有任务/事件模型不变；
  - 可按需增加 push UI（表单：仓库、分支、远端、凭证），非强制；
  - 支持展示新增的可选进度/错误字段（向后兼容）。
- 文档与测试：
  - 新增集成测试（push 至公共试仓库或本地模拟）；
  - 手册用例与回退指南；
  - 安全与脱敏检查。

---

## 3. 设计要点与决策

- Git Push（HTTPS）：
  - 凭证回调：
    - 仅令牌：用户名固定为 `x-access-token`（兼容 GitHub），密码为 token；
    - 用户名 + 令牌（或密码）。
  - 进度：`transfer_progress` +（若可用）`push_transfer_progress`，映射 bytesSent/objects/percent/phase。
  - 取消：在回调/循环中检查取消标记；进入上传后不再自动重试；取消立即中止。
  - 错误分类：401/403→Auth；连接/超时→Network；TLS/证书→Tls/Verify；用户取消→Cancel；其他→Protocol/Internal。
- 自定义 smart subtransport（方式A）：
  - 触发：对白名单域（如 github.com 域族）启用；
  - 行为：仅接管连接与 TLS/SNI；HTTP 语义仍由 libgit2 处理；
  - SNI 策略：默认 Real；可灰度 Fake→失败回退 Real；
  - 验证：保持链验证；可选 Real-Host 复核；
  - 回退：Fake→Real→libgit2 默认；错误归类与事件记录。
- Retry v1：
  - 可重试：超时、连接重置、暂时性网络、5xx（幂等阶段）；
  - 不可重试：证书验证失败、认证失败、用户取消、明确协议错误；
  - Push 特别规则：仅在“进入上传前”允许重试。
- 事件增强：
  - `task://progress` 新增可选字段：`bytesSent`, `phase`（`PreUpload|Upload|PostReceive`）；
  - `task://error` 标准化：`{ category, code?, message, retriedTimes? }`。

---

## 4. 子阶段拆解与验收

### 4.1 MP1.1 Git Push（HTTPS 基础）

- 目标：在 git2-rs 基础上实现 push，具备凭证回调、进度桥接、取消与错误分类；保持命令/事件兼容；日志脱敏。
- 实现要点：
  - 新命令：`git_push(repo: string, remote: string, branch: string, auth?: { token?: string; username?: string; password?: string })`；
  - `RemoteCallbacks::credentials` 回调按上述两种模式提供；
  - 进度：`transfer_progress` + `push_transfer_progress`（若可用），映射到 `task://progress`；
  - 取消：`AtomicBool`/CancellationToken，回调中尽早返回；
  - 错误分类：401/403→Auth；连接/超时→Network；TLS/证书→Tls/Verify；用户取消→Cancel；其他→Protocol/Internal；
  - 脱敏：默认不记录 Authorization/密码/令牌；调试模式下也要做脱敏。
- 接口影响：
  - 新增命令 `git_push`；其余命令/事件保持不变；
  - 事件新增的可选字段前端可无感；后续 UI 可以渐进展示。
- 测试与验收：
  - 单元：凭证回调覆盖两种模式；错误分类映射；取消路径单测；
  - 集成：对公共试仓库或本地裸仓库进行 push（可在 CI 使用本地仓库模拟）；
  - 验收：能成功 push；取消立即生效；错误分类符合预期；日志不泄漏敏感；
  - 回退：通过配置禁用 push 功能或让命令返回“未启用”。

### 4.2 MP1.2 自定义 smart subtransport（方式A）灰度

- 目标：对白名单主机启用仅接管连接/TLS/SNI 的自定义 subtransport；失败自动回退；可一键关闭。
- 实现要点：
  - URL 识别与白名单：命中域名时启用；
  - SNI 策略：默认 Real；灰度控制 Fake→Real 单次回退；
  - TLS 验证：保持链验证；可选 Real-Host 复核；
  - 代理模式下默认禁用 Fake（避免异常指纹叠加）；
  - 事件：在调试级别记录 usedFakeSni/realHost 验证结果（可选字段）；
  - 回退链：Fake→Real→libgit2 默认；确保最终可用或明确失败类别。
- 接口影响：
  - 无破坏性变更；通过配置开启/关闭；
  - 事件可附带可选调试字段，前端容忍未知键。
- 测试与验收：
  - 单元：白名单判定、策略分支、回退链覆盖；
  - 集成：对 github.com 域族仓库进行 clone/fetch/push（若启用），观测回退；
  - 验收：默认关闭时行为与 MP0 等价；开启灰度后失败可自动回退，成功率不降低；
  - 回退：运行时配置关闭后立即恢复 libgit2 默认路径。

### 4.3 MP1.3 Retry v1（统一重试策略）

- 目标：为 Clone/Fetch/Push 建立统一、可配置的重试策略；Push 仅在上传前允许重试。
- 实现要点：
  - 类别化：`{ retryable: Network/5xx | non-retryable: Verify/Auth/Cancel/Protocol }`；
  - 策略：指数退避 + 抖动（如 base=300ms, factor=1.5, max=6），可通过配置调整；
  - Push：若已开始上传 pack，不再自动重试；
  - 事件：在 progress 或 error 中附加 `retriedTimes`（可选）。
- 接口影响：
  - 无破坏性变更；新增配置项 `retry.*`；
  - 事件可增加可选字段。
- 测试与验收：
  - 单元：重试判定函数的分类与边界；退避计算；
  - 集成：模拟网络抖动/超时，观察重试次数与退出条件；
  - 验收：失败类别正确分类；最大重试后给出清晰错误；
  - 回退：配置中关闭或调低重试。

### 4.4 MP1.4 事件增强与错误分类

- 目标：丰富 push 进度信息并标准化错误事件，保持兼容。
- 实现要点：
  - `task://progress`：push 增加 `bytesSent`, `objects`, `percent`, `phase`（`PreUpload|Upload|PostReceive`）；
  - `task://error`：`{ category, code?, message, retriedTimes? }`，其中 `category ∈ { Network, Tls, Verify, Protocol, Proxy, Auth, Lfs, Cancel, Internal }`；
  - 保持现有字段不变，新字段为可选。
- 测试与验收：
  - 单元：事件构造与脱敏；
  - 集成：推送与失败路径均能产出预期事件；
  - 验收：前端不需变更亦能正常运行；可选增强展示正常工作。

---

## 5. 命令与事件（契约）

- 新命令：`git_push`
  - 入参（示例）：
    - `repo: string`（本地仓库路径）
    - `remote: string`（默认 `origin`）
    - `branch: string`（要推送的本地分支，或 `refs/heads/<name>`）
    - `auth?: { token?: string; username?: string; password?: string }`
    - 可选：`force?: boolean`（P2+ 再评估）
  - 事件：
    - `task://state`：`pending|running|completed|failed|canceled`
    - `task://progress`：push 增强字段（可选）
    - `task://error`：标准化错误对象（可选）

---

## 6. 配置模型（MP1 初版）

- `retry`: `{ max: number, baseMs: number, factor: number, jitter: boolean }`
- `httpStrategy`: `{ fakeSniEnabled: boolean, enforceDomainWhitelist: boolean }`
- `tls`: `{ realHostVerify: boolean }`
- `logging`: `{ debugAuthLogging: boolean }`（默认脱敏）

注：策略可在后续阶段（P2+）支持任务级覆盖，MP1 暂不强制。

---

## 7. 安全与隐私

- 不记录 Authorization/密码/令牌等敏感信息；
- 调试日志也需要脱敏（仅显示掩码/长度）；
- 证书验证不降低安全基线：自定义 subtransport 仅改变 SNI，链验证照常；
- 代理模式下禁用 Fake SNI（减少可识别异常特征）。

---

## 8. 可观测性与诊断

- 事件已满足实时显示需求；
- 可在 debug 日志中附加：usedFakeSni、realHost 验证结果、retriedTimes（均为可选）；
- 指标/面板在后续阶段推进（参考 `new-doc/TECH_DESIGN_git2rs.md` §13/§19）。

---

## 9. 测试计划

- 单元测试：
  - 凭证回调路径（仅 token / 用户名+令牌）；
  - 取消早退；
  - 错误分类映射；
  - Retry 判定与退避；
  - Subtransport 白名单与回退链。
- 集成测试：
  - Push 至本地裸仓库（CI 友好）；
  - 网络错误模拟（连接复位/超时）验证重试；
  - 开/关 subtransport(A) 的行为对比；
  - 事件流完整性（state/progress/error）。
- 手动测试（补充）：
  - 对公共试仓库的 Clone/Fetch/Push；
  - 断网/代理等边界场景；
  - 脱敏校验与日志审阅。

---

## 10. 回退策略

- Push：配置开关禁用；命令立即返回“未启用”或在 UI 层隐藏入口；
- Subtransport(A)：默认关闭；灰度开启后如失败自动回退 Real / 默认 libgit2 路径；
- Retry v1：配置关闭或降低重试次数；
- 事件增强：保留可选字段，回退不影响既有消费端。

---

## 11. 里程碑与节奏（相对）

- W1-W2：MP1.1 Push 实现与单测；本地集成测试跑通；
- W3：MP1.2 Subtransport(A) 原型与灰度开关；回退链与单/集成测试；
- W4：MP1.3 Retry v1 接入与分类打通；
- W5：MP1.4 事件增强与收尾；文档与回退演练；
- 持续：修复反馈、补齐边界；确保现有测试全绿。

（说明：具体节奏视 CI 与多平台联调进展微调，保持可回退。）

---

## 12. 风险与缓解

| 风险 | 等级 | 描述 | 缓解/回退 |
|------|------|------|-----------|
| Push 上传中断重复写入 | 中 | 自动重试导致重复写入 | 仅在上传前允许重试；清晰错误提示 |
| 自定义 subtransport 兼容性 | 中 | 某些网络策略/代理不兼容 | 默认关闭；Fake→Real→默认回退链；代理禁用 Fake |
| 错误分类不准 | 低 | 错误提示误导 | 分类表驱动+测试覆盖；保留原始 message |
| 凭证日志泄漏 | 高 | 敏感信息外泄 | 默认脱敏；审阅日志面板与测试 |
| 重试导致等待过长 | 低 | 用户体验下降 | 配置可调；最高重试次数限制；即时取消 |

---

## 13. 完成定义（DoD）

- 功能：Push + Subtransport(A) 灰度 + Retry v1 + 事件增强均可用；
- 兼容：前端无需改动即可运行；新增仅为可选字段；
- 质量：所有现有与新增测试全绿；无敏感日志；
- 可回退：任一子功能可单独关闭并恢复 MP0 行为；
- 文档：开发/测试/回退指南完整，关键配置有说明。

---

## 14. 实施 Checklist（开发者视角）

- [ ] 新增 `git_push` 命令与参数校验；
- [ ] 凭证回调（仅 token / 用户名+令牌）与脱敏；
- [ ] Push 进度桥接与取消；
- [ ] 错误分类与 `task://error` 事件；
- [ ] Subtransport(A) 白名单识别、SNI 策略与回退链；
- [ ] Retry v1：类别化判定与退避器；
- [ ] 配置开关：`retry.*`/`httpStrategy.fakeSniEnabled`/`tls.realHostVerify`；
- [ ] 单元/集成测试用例齐备（含本地裸仓库 push）；
- [ ] 文档更新与手册用例；
- [ ] 回退演练（开/关各子功能）。

---

（完）
