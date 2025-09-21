# Changelog

## Unreleased (P2)

### Added
- `strategy_override_summary` 聚合事件（Clone / Fetch / Push）提供 http/retry/tls 最终值、`appliedCodes`、`filterRequested`。
- 任务级策略覆盖扩展：TLS (`insecureSkipVerify` / `skipSanWhitelist`) 与 Retry (`max/baseMs/factor/jitter`) 字段。
- 环境变量：
  - `FWC_STRATEGY_APPLIED_EVENTS`（=0 关闭独立 *_strategy_override_applied 事件，仅用 summary）。
  - `FWC_PARTIAL_FILTER_SUPPORTED`（=1 视为支持 partial filter，不触发回退事件）。
- 事件：
  - `strategy_override_conflict`（HTTP & TLS 冲突归一化提示）。
  - `strategy_override_ignored_fields`（汇总未知顶层与分节字段）。
  - `partial_filter_fallback`（不支持 partial 时的 shallow/full 回退提示）。
  - `*_strategy_override_applied`（http/tls/retry 变更；可被 gating 抑制）。
- i18n：网络错误分类增加中文关键字（连接被拒绝/解析失败/超时 等）。

### Changed
- 独立 `*_strategy_override_applied` 事件受 `FWC_STRATEGY_APPLIED_EVENTS` 控制；关闭时仍在 summary.appliedCodes 中呈现差异。
- Push 任务策略覆盖逻辑与 Clone/Fetch 对齐（变更总被计入 `appliedCodes`，独立事件按 gating）。
- Informational 覆盖事件不再清空 `retriedTimes`（保持先前重试上下文）。

### Tests
- 新增：Clone / Fetch / Push summary & gating 正负用例；TLS summary & conflict；ignored fields；partial capability（capable / fallback）；retry/http 事件精确匹配；gating off 行为；冲突组合 (http/tls/combo)。
- 新增/改造文件示例：`strategy_override_summary.rs`、`git_strategy_override_summary_fetch_push.rs`、`git_strategy_override_tls_summary.rs`、`git_strategy_override_conflict_{http,tls,combo,no_conflict}.rs`、`git_strategy_override_guard_ignored.rs`、partial capability 相关测试等。
- 对公网依赖 shallow/partial 测试加入软跳过（失败输出标记不失败）。

### Docs
- README：新增环境变量、summary 事件结构与使用建议。
- `new-doc/TECH_DESIGN_P2_PLAN.md`：补充 P2.3c~P2.3g 综合章节（gating / partial / i18n / 回退矩阵 / summary schema）。

### Backward Compatibility
- 前端无需修改即可继续消费原有事件；可以逐步迁移为只解析 `strategy_override_summary` 以降噪。
- 冲突归一化：
  - HTTP：`followRedirects=false` 且 `maxRedirects>0` → 规范化 `maxRedirects=0` 并发 conflict。
  - TLS：`insecureSkipVerify=true` 且 `skipSanWhitelist=true` → 规范化 `skipSanWhitelist=false` 并发 conflict。
  - 规范化导致值变化仍会出现对应 *_applied（若 gating 开）。

### Revert / 回退指引
- 关闭 summary：移除 `emit_strategy_summary` 调用（功能退化为独立事件模式）。
- 关闭 gating：删除 `strategy_applied_events_enabled` 分支逻辑（总是发独立 *_applied）。
- 移除 TLS/Retry 覆盖：删对应 apply 分支与相关事件发射。
- 取消 partial fallback 逻辑：移除 `decide_partial_fallback` 调用及事件。
- 静默冲突：删除 conflict emit 分支（仍规范化）；再删除规范化逻辑可回到“忽略”模式。
- 忽略未知字段提示：删除 ignored emit 分支（仅日志或静默）。
- i18n 回退：移除新增中文关键字匹配。

### Front-end
- strategyOverride 透传深度与 filter 与后端保持兼容；`startGitFetch` 兼容旧 preset 字符串与对象参数新写法。



## v0.2.0-P2.2b (2025-09-19)

P2.2b: Shallow Clone (`depth` for `git_clone`) 实现：
- 新增：`git_clone` 支持可选 `depth`（浅克隆），通过参数解析后在执行层设置 `FetchOptions.depth`；
- 本地路径克隆不支持浅克隆，自动忽略 depth（静默回退，无事件扰动）；
- 解析上限由 `u32::MAX` 调整为 `i32::MAX` 以匹配 git2 接口，超出返回 `Protocol(depth too large)`；
- Trait 变更：`GitService::clone_blocking` 新增 `depth: Option<u32>`；所有调用点已更新传 `None`；
- 过滤器 / 策略（`filter` / `strategyOverride`）仍为占位解析，不改变行为；
- 新增测试：`tests/git_shallow_clone.rs`（公网深度=1 验证 `.git/shallow` 存在；全量克隆无 shallow 文件）；
- 新增测试：`tests/git_shallow_local_ignore.rs` 验证本地路径克隆即使传入 depth=1 仍获得完整历史且无 `.git/shallow`（静默回退保障）；
- 新增测试：`tests/git_shallow_invalid_depth.rs` 验证 depth=0、负值、超出 i32::MAX 均被解析阶段拒绝（任务 Failed，错误分类 Protocol）；
- 组合参数测试保持通过（本地路径上 depth 被忽略不失败）；
- 前端无需改动（TaskKind 已包含可选字段，事件未变）。

回退：在 `DefaultGitService` 强制忽略 depth 即可软回退；移除 trait 参数与 `fo.depth()` 调用可硬回退。

已知限制：尚未实现 fetch depth / partial filter / 节省指标；不支持为本地路径发回退事件（后续 partial 路径统一）。

## v0.1.1-MP0.4 (2025-09-14)

完成 MP0.4：从 gitoxide/gix 完整迁移到 git2-rs，并清理旧实现
- 后端 Git 实现：统一使用 git2-rs（libgit2 绑定）完成 clone/fetch；
- 任务/事件：保持命令签名与 `task://state|progress` 事件兼容；
- 取消与错误：协作式取消生效；错误分类 Network/Tls/Verify/Auth/Protocol/Cancel/Internal；
- 清理：移除 gix 与 gix-transport 依赖，删除旧的 clone/fetch 与进度桥接模块；移除构建特性开关；
- 测试：Rust 与前端 75 项测试全部通过。

## v0.1.0-P0 (2025-09-13)

P0 初始交付：
- 通用伪 SNI HTTP 请求 API（http_fake_request）
  - Fake SNI 开关、重定向、timing、body base64、Authorization 脱敏
  - SAN 白名单强制校验
- 基础 Git Clone（gitoxide）
  - 任务模型（创建/状态/进度/取消）与事件
- 前端面板
  - HTTP Tester（历史回填、策略开关）、Git 面板（进度/取消）、全局错误提示
- 文档与测试
  - 技术设计（整合版 + P0 细化）、手动验收脚本（MANUAL_TESTS）
  - Rust/Vitest 全部测试通过

已知限制与后续计划：
- 未接入代理与 IP 优选（Roadmap P4-P5）
- Git 伪 SNI 与自动回退（Roadmap P3）
- SPKI Pin & 指纹事件（Roadmap P7）
- 指标面板（Roadmap P9），流式响应/HTTP2（Roadmap P10）
