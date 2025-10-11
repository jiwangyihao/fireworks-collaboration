# P3 实现与维护对接文档 (Implementation Guide)

> 适用读者：自适应 TLS 传输层维护者、可观测性与任务编排开发者、质量保障
> 配套文件：`new-doc/TECH_DESIGN_P3_PLAN.md`
> 当前状态：P3 目标全部交付（至 v1.19），进入“持续监测 + 渐进优化”阶段。

---
## 目录
1. 交付范围概述
2. 核心模块映射
3. 配置项与默认值
4. Rollout 策略与事件流
5. 可观测性与指标采集
6. Real-Host 验证与回退
7. SPKI Pin 强校验
8. 自动禁用与稳健性
9. Soak 稳定性验证与报告
10. 测试矩阵与关键用例
11. 运维说明与回退策略
12. 后续优化建议

---
## 1. 交付范围概述
| 主题 | 目标 | 状态 |
|------|------|------|
| 默认启用 + 渐进放量 | 对白名单域按百分比 rollout，自适应 Fake→Real→Default 链条 | ✅ 完成，默认 100% |
| 可观测性强化 | timing、fallback、cert 指纹事件与日志 | ✅ 完成，事件 DSL 已接入 |
| Real-Host 验证 | Fake 握手后按真实域名验证 + 单次 Real 回退 | ✅ 完成，现为强制启用 |
| SPKI Pin | 指纹强校验，支持多指纹并行，触发 mismatch 事件 | ✅ 完成，默认关闭 |
| 自动禁用策略 | 窗口失败率阈值 + 冷却恢复，事件可观测 | ✅ 完成 |
| Soak 稳定性 | Push→Fetch→Clone 泡脚 + 报告输出 | ✅ 完成，含基线对比 |

---
## 2. 核心模块映射
| 模块 | 文件 | 说明 |
|------|------|------|
| Rollout 采样 | `src-tauri/src/core/git/transport/rewrite.rs` | `RewriteDecision` 暴露 `sampled`/`eligible`，采样哈希稳定 |
| 回退决策 | `src-tauri/src/core/git/transport/fallback.rs` | 纯状态机，Fake→Real→Default transitions |
| 指标采集 | `src-tauri/src/core/git/transport/metrics.rs` | `TimingRecorder` + TL 快照，尊重 `metrics_enabled` |
| 指纹日志 | `src-tauri/src/core/git/transport/fingerprint.rs` | JSONL log + 24h LRU 缓存 + 结构化事件 |
| Real-Host 验证 | `src-tauri/src/core/tls/verifier.rs` | `RealHostCertVerifier` 以真实域名包装 `WebPkiVerifier` |
| SPKI Pin | 同上 | `validate_pins` 过滤非法/超限指纹，mismatch 发事件 |
| 自动禁用 | `src-tauri/src/core/git/transport/runtime.rs` | 失败率窗口 + 冷却，事件由任务层转发 |
| 任务集成 | `src-tauri/src/core/tasks/registry/git/*.rs` | Rollout 事件、TLS 可观测事件、策略总结 |
| Soak Runner | `src-tauri/src/soak/mod.rs` | 迭代运行 + 报告生成 + 基线对比 |

---
## 3. 配置项与默认值
| 路径 | 键 | 默认 | 说明 |
|------|----|------|------|
| `http.fakeSniEnabled` | bool | true | 全量开启 adaptive TLS |
| `http.fakeSniRolloutPercent` | u8 | 100 | 采样百分比，0 视为禁用 |
| `http.hostAllowListExtra` | Vec<String> | [] | Rollout 白名单附加域 |
| `http.autoDisableFakeThresholdPct` | u8 | 20 | 自动禁用失败率阈值 |
| `http.autoDisableFakeCooldownSec` | u64 | 300 | 自动禁用冷却秒数 |
| `tls.metricsEnabled` | bool | true | 控制 `TimingRecorder` 是否生效 |
| `tls.certFpLogEnabled` | bool | true | 指纹日志与事件开关 |
| `tls.certFpMaxBytes` | u64 | 5 MiB | 日志滚动阈值 |
| `tls.spkiPins` | Vec<String> | [] | Base64URL 指纹列表 (≤10) |

> ⚠️ 运行期变更依赖 `load_or_init()` 动态加载，无需重启。

---
## 4. Rollout 策略与事件流
- `RewriteDecision`：`eligible=true` 表示命中策略条件（协议/白名单），`sampled` 体现是否真正改写。
- 任务注册层 (`clone/fetch/push`) 无论命中与否都发 `AdaptiveTlsRollout` 事件：
  - `percent_applied`=当前配置，`sampled`=true/false。
  - 事件只在 `eligible` 场景触发，避免非白名单噪声。
- 新增验证：`rewrite.rs` 覆盖查询/fragment 保留、`.git` 去重与百分比上限；`git/test_support.rs` 断言 rollout 事件在 0%/100% 配置下的 `sampled` 标记。
- 指标：`ROLLOUT_HIT/MISS` 通过 `AtomicU64` 记录，可在调试时读取。
- 结构化事件载荷：`StrategyEvent::AdaptiveTlsRollout { id, kind, percent_applied, sampled }`。`id`=任务 UUID，`kind` 固定为 `GitClone` / `GitFetch` / `GitPush`。`sampled=false` 表示哈希偏移未命中但仍记录决策基线；`collect_rollout` 帮助用例在 `rollout_event_reflects_sampled_*` 中覆盖 0% 与 100% 两端，验证约定。
- Rollout 事件仅在 `eligible=true` 的白名单域触发，避免外部域噪声进入结构化事件或计数器；非白名单场景直接回落至 libgit2 默认传输，并在日志中打印 `host not allowed by SAN whitelist` 便于溯源。

---
## 5. 可观测性与指标采集
- `TimingRecorder` 记录 connect/tls/firstByte/total ms；`finish_and_store` 在 `metrics_enabled`=true 时写入 TL。
- 新增单元测试 `finish_respects_metrics_enabled_flag`、`metrics_enabled_env_override_takes_precedence`，分别覆盖配置开关与环境变量优先级。
- `tl_snapshot` + `tl_take_fallback_events` 在任务完成时转换为结构化事件：
  - `AdaptiveTlsTiming`
  - `AdaptiveTlsFallback`
  - `AdaptiveTlsAutoDisable`
- 指纹变化 `CertFingerprintChanged` 事件 + `cert-fp.log`（JSON Lines, 5 MiB 滚动）。
- `AdaptiveTlsTiming { id, kind, used_fake_sni, fallback_stage, connect_ms, tls_ms, first_byte_ms, total_ms, cert_fp_changed }` 仅在 `tls.metricsEnabled=true` 时发射；`FWC_TEST_FORCE_METRICS=0/1` 可在运行时强制关闭/开启（测试中配合 `test_override_metrics_enabled`）。线程局部 `tl_*` 状态字段确保多阶段握手耗时与伪 SNI 决策在任务汇报前一次性消费。
- `AdaptiveTlsFallback` 取自 `FallbackEventRecord::Transition`，其中 `from/to` 对应 `FallbackStage::{Fake,Real,Default}`，`reason` 来源于 `FallbackReason::{EnterFake,FakeHandshakeError,SkipFakePolicy,RealFailed}`。`classify_and_count_fallback` 据此把错误分桶到 `Tls` / `Verify` 并维护原子计数，`events_structure_and_contract.rs` 覆盖回退噪声在结构化事件中的契约。
- 自动禁用结构化事件 `AdaptiveTlsAutoDisable { enabled, threshold_pct, cooldown_secs }` 不受 metrics 开关影响；无论触发还是冷却恢复都会写入 TL，任务结束时必定发射，保证关闭 timing 时仍有可观测信号。
- 指纹日志写入配置基目录下的 `cert-fp.log`，超出 `certFpMaxBytes`（默认 5 MiB）即滚动为 `cert-fp.log.1`。内存缓存最多记录 512 主机、窗口 24 小时；仅当指纹有变更才追加 `CertFingerprintChanged { host, spki_sha256, cert_sha256 }` 事件。集成测试 `fingerprint_logs_include_spki_source_exact_and_fallback` 与 `events_structure_and_contract.rs` 的契约断言保证字段稳定。
- `cert_fp_changed=true` 仅在 `record_certificate` 发现 leaf SPKI / 全证书哈希与缓存不同（24 小时窗口内）时设置；若证书未发生变化则 timing 事件保持默认 `false`，避免干扰故障排查。

---
## 6. Real-Host 验证与回退
- Fake SNI 握手成功后，`RealHostCertVerifier` 使用真实域名执行证书链 + SAN 白名单校验。
- 失败归类 `Verify`，触发 Fake→Real 回退统计；该校验为强制行为，不再提供配置开关。
- 若真实域名无法被解析成 `DnsName`（例如含非法字符），验证器会自动退回到 SNI，对运维透明；白名单匹配仍优先基于 `override_host`，保证 Fake SNI 下也按真实域筛选。
- 验证失败的错误文本包含 `SAN whitelist mismatch` / `name mismatch` 等关键字，并由 `classify_and_count_fallback` 计入 Verify 分桶；`AdaptiveTlsFallback` 在该场景下给出 `from="Fake"`、`to="Real"`、`reason="FakeHandshakeError"`，配合内部原子计数器即可量化真实域核验带来的回退量。

---
## 7. SPKI Pin 强校验
- 配置 `tls.spkiPins`（Base64URL，长度 43，≤10）非空即启用。
- 匹配失败：
  - 日志 `pin_mismatch`
  - 事件 `cert_fp_pin_mismatch`
  - 返回 Verify 类错误（不再尝试 Real SNI）。
- 重复/非法值会导致本次连接禁用 Pin（写警告日志）。
- `validate_pins` 会在运行期逐条校验 Base64URL 格式（长度=43、无填充），超出 10 条或存在非法值即整体禁用 Pin（返回 `None`）；合法值自动去重。单元测试 `core::tls::verifier::tests::test_validate_pins_rules` 锁定解析规则。
- Pin 命中时写 `pin_match` 调试日志且不额外发事件；未命中则发 `StrategyEvent::CertFpPinMismatch { host, spki_sha256, pin_count }`，并立即以 Verify 错误终止握手（不触发 Fake→Real），确保运营可以通过事件与日志快速定位配置缺失。集成测试 `pin_mismatch_emits_event_and_counts_verify` 覆盖该流程。

---
## 8. 自动禁用与稳健性
- `AutoDisableConfig` 从 http 配置派生。
- `record_fake_attempt` 在 Fake 成功/失败时记录窗口样本。
- 触发条件：样本数 ≥5 且失败率 ≥ 阈值。
- 事件链：任务层监听 TL 事件，发布 `AdaptiveTlsAutoDisable`（enabled=true/false）。
- 算法细节：窗口最多保留 20 条样本（`SAMPLE_CAP=20`），统计周期 120 秒，少于 5 条样本 (`MIN_SAMPLES`) 不判定；达到阈值后将 `disabled_until` 设为 `now + cooldown` 并清空窗口，记录触发事件。
- 指标：触发与恢复分别递增 `adaptive_tls_auto_disable_triggered_total`、`adaptive_tls_auto_disable_recovered_total`；单元测试 `auto_disable_triggers_when_ratio_exceeds_threshold`、`auto_disable_recovers_after_cooldown` 校验计数器与状态机协同。
- 即便 `tls.metricsEnabled=false`，线程局部仍会推送 `AdaptiveTlsAutoDisable`，确保关闭 timing 时运维仍能收到启停通知；配套日志 `adaptive_tls_fake auto-disable triggered/recovered` 可与结构化事件对照。

---
## 9. Soak 稳定性验证与报告
- 入口：`FWC_ADAPTIVE_TLS_SOAK=1`，可选参数：
  - `FWC_SOAK_ITERATIONS`、`FWC_SOAK_KEEP_CLONES`、`FWC_SOAK_REPORT_PATH`
  - `FWC_SOAK_BASELINE_REPORT` 支持基线对比
- 报告字段：成功率、fallback 比率、自动禁用触发次数、指纹事件计数、P50/P95 timing.
- 测试：
  - `soak_runs_minimal_iterations`
  - `soak_attaches_comparison_when_baseline_available`
  - `comparison_summary_detects_regressions`

---
## 10. 测试矩阵与关键用例
| 类别 | 位置 | 说明 |
|------|------|------|
| Rollout | `rewrite.rs` 单测 | 采样 0%/10%/100%、查询/fragment 保留、`.git` 去重、额外白名单、代理禁用 |
| 决策状态机 | `fallback.rs` 单测 | Fake→Real→Default 链路、Skip Policy |
| 指标开关 | `metrics.rs` 单测 | 新增 gating 覆盖 |
| TLS 验证 | `tls/verifier.rs` 单测 | Real host、Pin 匹配/失败、非法指纹 |
| Auto disable | `http/mod.rs` 单测 | 注入失败驱动触发与恢复 |
| 指纹事件 | `tests/events/events_structure_and_contract.rs` | 结构化事件契约 |
| Soak | `soak/mod.rs` 单测 | 报告生成、基线对比、阈值判定 |
| 集成 | `cargo test -q` (src-tauri) | Rust 单元/集成全量通过 |

---
## 11. 运维说明与回退策略
| 场景 | 操作 | 影响 |
|------|------|------|
| 暂时禁用 Fake SNI | `http.fakeSniEnabled=false` 或 rollout=0 | 回退至 libgit2 默认传输 |
| 暂停指标采集 | `tls.metricsEnabled=false` | 不再生成 timing 事件/日志 |
| 停止指纹日志 | `tls.certFpLogEnabled=false` | 停止写 `cert-fp.log` 与变更事件 |
| 停止 Real-Host 校验 | 不支持配置关闭；需禁用 Fake SNI 或回滚版本 | 当前版本强制启用 |
| 清空 Pin | `tls.spkiPins=[]` | 立即停用强校验 |
| 自动禁用触发 | 等待冷却或提高阈值 | 期间 Fake SNI 不再尝试 |

---
## 12. 后续优化建议
1. Rollout 指标上报至外部监控（当前仅原子计数）。
2. 指纹日志可选压缩/分桶，降低高频任务磁盘压力。
3. Fallback 原因分类可扩展为枚举 + 序列化，减少字符串匹配成本。
4. 自动禁用与 Soak 报告联动：提供最近一次触发时间与阈值快照。
5. 指纹事件增加 host 分段限流，防止异常域 spam。

---
## 快速校验命令
```powershell
# Rust 模块测试（src-tauri）
cd src-tauri
cargo test -q

# 前端/Pinia 测试
cd ..
pnpm install
pnpm test -s

# Soak 样例（5 轮，保留报告）
set FWC_ADAPTIVE_TLS_SOAK=1
set FWC_SOAK_ITERATIONS=5
set FWC_SOAK_REPORT_PATH=%CD%\soak-report.json
cd src-tauri
cargo run --bin fireworks-collaboration --features soak
```
