# Changelog

## Unreleased (P2.3e)

新增：任务级策略覆盖护栏与冲突规范化
- Added: per-task strategy override ignored fields event `strategy_override_ignored_fields` （解析阶段收集未知顶层与分节字段并一次性提示，不阻断任务）。
- Added: conflict normalization + event `strategy_override_conflict`：
  - HTTP: followRedirects=false 且 maxRedirects>0 → 规范化 maxRedirects=0；
  - TLS: insecureSkipVerify=true 且 skipSanWhitelist=true → 规范化 skipSanWhitelist=false；
  - 规范化后若最终值与全局不同仍会伴随 `*_strategy_override_applied`；若相同仅发 conflict。
- 测试：新增 `git_strategy_override_guard_ignored.rs`、`git_strategy_override_conflict_{http,tls,combo,no_conflict}.rs` 及 Registry 单元测试更新；全量 `cargo test` + 前端 `pnpm test` 通过。
- 文档：`new-doc/TECH_DESIGN_P2_PLAN.md` 已补充冲突规范化与事件顺序、回退策略、测试矩阵。

回退：删除 conflict emit 分支可静默规范化；进一步删除规范化逻辑回到仅忽略字段阶段；移除 ignored emit 分支回退为仅日志。

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
