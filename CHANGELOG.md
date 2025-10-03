# Changelog

## Unreleased (P6)

### P6.0 (Completed) Credential Storage & Security Management - Baseline Architecture

**Added:**
- **凭证管理核心模块** (`core/credential`):
  - `Credential` 数据模型，包含 host, username, password_or_token, expires_at, created_at, last_used_at 字段
  - `CredentialStore` trait 定义统一的存储抽象接口 (get, add, remove, list, update_last_used)
  - `MemoryCredentialStore` 内存存储实现，支持并发访问和过期检测
  - `CredentialConfig` 配置结构，支持 system/file/memory 三种存储类型
  - `StorageType` 枚举，定义存储类型选项

- **安全特性**:
  - 凭证序列化时自动跳过 `password_or_token` 字段，防止泄露
  - `Display` 和 `Debug` trait 实现自动脱敏显示（如 `ghp_****cdef`）
  - `masked_password()` 方法用于日志和 UI 显示
  - 过期检测：`is_expired()` 方法和自动过滤

- **配置集成**:
  - 在 `AppConfig` 中添加 `credential: CredentialConfig` 字段
  - `config.example.json` 新增完整的凭证配置章节，包含 5 个场景示例
  - 支持配置验证 (`CredentialConfig::validate()`)

- **文档**:
  - `CREDENTIAL_SECURITY_ASSESSMENT.md`: 识别 15 个安全威胁及缓解措施
  - `CREDENTIAL_ENCRYPTION_DESIGN.md`: AES-256-GCM + Argon2id 加密方案详细设计
  - API 文档注释，包含使用示例

- **测试**:
  - 33 个单元测试（model: 10, config: 9, storage: 14）
  - 10 个集成测试，覆盖完整生命周期、并发操作、边界情况
  - 100% 单元测试通过率，无回归失败

- **依赖**:
  - `aes-gcm` 0.10 - AES-256-GCM 对称加密
  - `argon2` 0.5 - 密钥派生函数
  - `hmac` 0.12 - HMAC-SHA256 完整性校验
  - `sha2` 0.10 - SHA-256 哈希
  - `zeroize` 1.x - 内存清零

**Changed:**
- 无破坏性变更，所有新功能均为增量添加

**Backward Compatibility:**
- 凭证配置字段在 `AppConfig` 中使用 `#[serde(default)]`，旧配置文件自动使用默认值
- 默认配置使用系统钥匙串 (`storage: system`)，不影响现有功能
- 所有新增 API 仅在显式调用时生效

**Security:**
- 密码/令牌字段在序列化时自动跳过
- Display/Debug 输出自动脱敏
- 过期凭证自动被过滤，不会被使用
- 配置验证确保安全默认值

**Documentation:**
- 安全威胁评估文档（700+ 行）
- 加密方案设计文档（800+ 行）
- 配置示例与最佳实践
- API 文档注释与使用示例

**Performance:**
- 单次凭证操作平均耗时 < 1ms（内存存储）
- 支持并发访问，通过 10 线程并发测试
- 大规模测试：1000 个凭证管理无性能问题

**Testing:**
- Unit tests: 33 个测试，0.20s 完成
- Integration tests: 10 个测试，0.15s 完成
- 边界测试：过期时间边界、并发访问、大量凭证
- 安全测试：序列化安全、脱敏显示、配置验证

**Revert Path:**
- P6.0 仅建立基线架构，无运行时影响
- 后续阶段如需回退，移除 `credential` 模块即可
- 配置文件中的 `credential` 字段会被忽略（使用默认值）

**Next Steps (P6.1+):**
- P6.1: 实现系统钥匙串、加密文件存储
- P6.2: 实现加密/解密、内存清零
- P6.3: 前端 UI 集成
- P6.4: 凭证生命周期管理
- P6.5: 安全审计与准入

---

## Unreleased (P2)

### P3.2 (In-progress) Adaptive TLS Observability
Added:
- Config `tls.metricsEnabled` (default true) to enable adaptive TLS timing capture (connect/tls/firstByte/total) per connection.
- Config `tls.certFpLogEnabled` (default true) and `tls.certFpMaxBytes` (default 5MB) with rolling `cert-fp.log` (JSONL) storing `{ ts, host, spkiSha256, certSha256, changed }`.
- Fingerprint module: leaf certificate SPKI SHA256 (Base64URL) + full cert SHA256 hashing (ring) with 24h change suppression window and LRU (512 hosts) cache.
- Structured events:
  - `StrategyEvent::AdaptiveTlsTiming { used_fake_sni, fallback_stage, connect_ms, tls_ms, first_byte_ms, total_ms, cert_fp_changed }` (emitted on task terminal state when metrics enabled and timing present).
  - `StrategyEvent::CertFingerprintChanged { host, spki_sha256, cert_sha256 }` (now actively emitted on initial and subsequent change events; complements boolean `cert_fp_changed`).
  - Thread-local timing recorder integrated into Fake→Real fallback chain; first byte capture now precise via HTTP response decoding hook (SniffingStream) marking the first body bytes arrival.

Changed:
- `Fallback` chain now records final stage & whether Fake was used; tasks emit timing independent of success/failure.
- Transport metrics scaffolding extended with global collector & thread-local snapshot API.

Backward Compatibility:
- All new fields/events are additive; existing consumers ignoring unknown StrategyEvent variants remain functional.
- Disabling metrics (`tls.metricsEnabled=false`) fully suppresses timing event emission without altering transport behavior.
- 运行时环境覆盖 `FWC_TEST_FORCE_METRICS`（test instrumentation），用于在测试中显式强制 metrics 开/关验证事件行为。

Revert Path:
- Set `tls.metricsEnabled=false` to stop timing; set `tls.certFpLogEnabled=false` to stop fingerprint logging.
- Remove `fingerprint.rs` & timing emission patches to return to P3.1 baseline (no code path dependency elsewhere).
- `AdaptiveTlsFallback` / `AdaptiveTlsAutoDisable` 事件在 `tls.metricsEnabled=false` 时也会发射；Timing 事件仍受该开关控制。

Security / Privacy:
- Fingerprint log excludes SAN list or any credential data; only hashes and host.

- `section_adaptive_tls_fallback` 新增 metrics 关闭场景断言：Fallback 事件继续输出且 Timing 事件受抑制。
Refinement (post-initial P3.2 patch): implemented precise firstByte capture hook & activated dedicated CertFingerprintChanged structured event.

### Test Refactor Second Pass
- 新增事件等待 helper: `wait_for_event` / `wait_for_applied_code(_default)`，支持基于结构化事件出现的精准等待。
- 批量使用 `tests_support::repo::build_repo` 替换重复仓库初始化（策略冲突、HTTP/TLS/Retry 组合、TLS mixed、Retry override、HTTP override idempotent 等）。
- 统一等待：移除多处手写 for 循环轮询，改用 `wait_task_terminal`（返回 Result，集中化超时逻辑）。
- 覆盖率硬化预备：新增脚本 `scripts/coverage_check.ps1`，支持 `FWC_COVERAGE_MIN_LINE` / `FWC_COVERAGE_ENFORCE` 环境变量，当前仍为软门控。
- 代码清理：移除未使用导入，确保等待结果显式消费，降低未来启用 `-D warnings` 风险。

### Added
- `strategy_override_summary` 聚合事件（Clone / Fetch / Push）提供 http/retry/tls 最终值、`appliedCodes`、`filterRequested`。
- 任务级策略覆盖扩展：TLS (`insecureSkipVerify` / `skipSanWhitelist`) 与 Retry (`max/baseMs/factor/jitter`) 字段。
- 环境变量：
  - `FWC_PARTIAL_FILTER_SUPPORTED`（=1 视为支持 partial filter，不触发回退事件；兼容 `FWC_PARTIAL_FILTER_CAPABLE`）。
  - （已废弃）`FWC_LEGACY_STRATEGY_EVENTS`：T6 中移除，对行为无影响（legacy 策略类 TaskErrorEvent 已删除）。
- 事件：
  - `strategy_override_conflict`（HTTP & TLS 冲突归一化提示）。
  - `strategy_override_ignored_fields`（汇总未知顶层与分节字段）。
  - `partial_filter_fallback`（不支持 partial 时的 shallow/full 回退提示）。
  - `*_strategy_override_applied`（http/tls/retry 变更；可被 gating 抑制）。
- i18n：网络错误分类增加中文关键字（连接被拒绝/解析失败/超时 等）。

### Changed
- 独立 `*_strategy_override_applied` 与 `strategy_override_summary` 结构化事件始终发布；其 legacy TaskErrorEvent 版本（含 summary）在 T6 中被彻底移除。
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
- （更新）T6 后需消费结构化事件（Strategy/Policy/Transport）；旧 *_strategy_override_applied / conflict / summary / adaptive / partial_filter_fallback / ignored_fields TaskErrorEvent 不再发射。
- 冲突归一化：
  - HTTP：`followRedirects=false` 且 `maxRedirects>0` → 规范化 `maxRedirects=0` 并发 conflict。
  - TLS：`insecureSkipVerify=true` 且 `skipSanWhitelist=true` → 规范化 `skipSanWhitelist=false` 并发 conflict。
  - 规范化导致值变化仍会出现对应 *_applied（若 gating 开）。

### Revert / 回退指引
- 恢复 legacy 事件已不支持（代码删除）；如需调试旧前端请基于历史 tag 回滚。
- 若需降噪：仅消费 `StrategyEvent::Summary` 的 `applied_codes` 与 Conflict/Retry/Transport 结构化事件。

### Front-end
- strategyOverride 透传深度与 filter 与后端保持兼容；`startGitFetch` 兼容旧 preset 字符串与对象参数新写法。

### P3.5 (Complete) Adaptive TLS Resilience
Added:
- Config defaults `http.autoDisableFakeThresholdPct` (20) 与 `http.autoDisableFakeCooldownSec` (300s) 进入 `AppConfig`，序列化/反序列化自动填充。
- Runtime auto-disable state machine：滑动窗口（120s / 20 样本 / 至少 5 条）跟踪 Fake SNI 成功率；当失败率 ≥ 阈值触发冷却并在恢复后重置。
- Structured events：
  - `StrategyEvent::AdaptiveTlsFallback { from, to, reason }`（记录 Fake→Real→Default 转移及原因）。
  - `StrategyEvent::AdaptiveTlsAutoDisable { enabled, threshold_pct, cooldown_secs }`（记录熔断开/关时刻与配置）。
  - 任务注册表（Clone/Fetch/Push）统一 Drain thread-local fallback 事件并发布。
- Test-only helpers：HTTP 传输提供 TLS 失败注入队列；runtime 导出 `test_auto_disable_guard`；事件合约测试扩展至新枚举。

Changed:
- `connect_tls_with_fallback` 在 Fake 阶段失败时调用 auto-disable 统计，并在进入 Default 前回放最后一次错误，确保注入测试稳定。
- Thread-local `FallbackEventRecord` 统一保存 Transition/AutoDisable 记录，便于任务端获取。
- 新增同步互斥锁防止 auto-disable 全局状态在多测试间互相干扰。

Tests:
- Rust：`core::git::transport::runtime` 覆盖触发、冷却、禁用开关；HTTP 回退测试确认事件与熔断行为；`tests/events/events_structure_and_contract.rs` 校验结构化事件 schema。
- 前端：`pnpm test`（Vitest）全量通过，错误日志记录保持原有断言。

Backward Compatibility:
- 默认阈值/冷却为非零；将 `autoDisableFakeThresholdPct` 设为 0 即可禁用熔断并回退至旧行为。
- 事件消费者若尚未适配新增枚举，可忽略 `StrategyEvent::AdaptiveTlsFallback` / `AdaptiveTlsAutoDisable`。

Revert Path:
- 移除 runtime auto-disable 逻辑并将线程本地事件 Drain 回退至仅记录 Transition，即恢复至 P3.2 状态；配置字段保留但可标记 deprecated。



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
