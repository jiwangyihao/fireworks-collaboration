# P4 实现与维护对接文档 (IP 池与握手优选)

> 适用读者：IP 池/网络调度维护者、传输层与可观测性开发者、质量保障
> 配套文件：`doc/TECH_DESIGN_P4_PLAN.md`
> 当前状态：P4 目标全部交付（合入 main 分支），处于“受控灰度 + 指标监控”阶段。

---
## 目录
1. 交付范围概述
2. 核心模块映射
3. 配置项与默认值
4. IP 池总体生命周期
5. 预热调度 (PreheatService)
6. 按需采样与缓存维护
7. 传输层集成与优选决策
8. 异常治理：熔断、黑白名单与自动禁用
9. 观测事件与指标
10. 历史数据与持久化
11. Soak 准入与阈值
12. 测试矩阵与关键用例
13. 运维说明与回退策略
14. 后续优化建议
15. 快速校验命令

---
## 1. 交付范围概述
| 主题 | 目标 | 状态 |
|------|------|------|
| IP 池核心模块 | 候选收集、缓存、历史持久化 | ✅ 完成 |
| 预热调度 | 启动批量采样 + TTL 刷新 + 失败退避 | ✅ 完成 |
| 按需采样 | 首访即时测速、TTL 清理、容量控制 | ✅ 完成 |
| 传输集成 | TLS 连接按延迟优选 + 回退系统 DNS | ✅ 完成 |
| 观测体系 | Selection/Refresh 事件、Adaptive TLS 扩展字段 | ✅ 完成 |
| 异常治理 | IP 级熔断、黑白名单、全局自动禁用 | ✅ 完成 |
| Soak 准入 | 指标阈值、基线对比、报告拓展 | ✅ 完成 |
| 回归保障 | Rust 单测/集测 + Soak + 前端兼容校验 | ✅ 完成 |

---
## 2. 核心模块映射
| 模块 | 文件/目录 | 说明 |
|------|-----------|------|
| IP 池入口 | `src-tauri/src/core/ip_pool/mod.rs` | `IpPool` 对外接口（pick/report/maintenance/config）|
| 缓存实现 | `src-tauri/src/core/ip_pool/cache.rs` | `IpScoreCache`、`IpStat`、容量与TTL辅助 |
| 历史存储 | `src-tauri/src/core/ip_pool/history.rs` | `IpHistoryStore` 读写、裁剪、降级 |
| 预热调度 | `src-tauri/src/core/ip_pool/preheat.rs` | `PreheatService`、`run_preheat_loop`、多来源收集 |
| 传输集成 | `src-tauri/src/core/git/transport/http/subtransport.rs` | 候选优选、线程本地观测字段 |
| 事件发布 | `src-tauri/src/core/ip_pool/events.rs` | Selection/Refresh/熔断/CIDR/Auto-Disable 辅助 |
| 熔断逻辑 | `src-tauri/src/core/ip_pool/circuit_breaker.rs` | IP 级失败窗口、冷却、事件发射 |
| 配置解析 | `src-tauri/src/core/ip_pool/config.rs` | 运行期 + 文件配置、默认值、热更新 |
| 全局接入 | `src-tauri/src/core/ip_pool/global.rs` | 全局 `OnceLock<Arc<Mutex<IpPool>>>` |
| Soak 扩展 | `src-tauri/src/soak/mod.rs` | IP 池指标统计、阈值判定、报告输出 |
| 测试套件 | `src-tauri/tests/tasks/`、`src-tauri/tests/events/` | 事件契约与集成回归 |

---
## 3. 配置项与默认值
| 文件 | 键 | 默认 | 说明 |
|------|----|------|------|
| `config.json` (`AppConfig`) | `ip_pool.enabled` | false | 主开关，禁用时直接使用系统 DNS |
|  | `ip_pool.cachePruneIntervalSecs` | 60 | TTL 清理周期，按需触发 |
|  | `ip_pool.maxCacheEntries` | 256 | 按需域名缓存容量（预热域不计入） |
|  | `ip_pool.singleflightTimeoutMs` | 10000 | 同域采样超时 |
|  | `ip_pool.failureThreshold` | 5 | 熔断窗口最小样本 |
|  | `ip_pool.failureRateThreshold` | 0.6 | 熔断失败率 |
|  | `ip_pool.failureWindowSeconds` | 120 | IP 级窗口长度 |
|  | `ip_pool.cooldownSeconds` | 300 | IP 熔断冷却 |
|  | `ip_pool.circuitBreakerEnabled` | true | 允许 IP 级熔断 |
| `ip-config.json` (`IpPoolFileConfig`) | `preheatDomains` | [] | 预热域名 + 端口 |
|  | `scoreTtlSeconds` | 300 | 评分有效期 |
|  | `maxParallelProbes` | 4 | 并发握手数 |
|  | `probeTimeoutMs` | 3000 | 单次探测速超时 |
|  | `userStatic` | [] | 额外静态 IP 列表 |
|  | `blacklist` / `whitelist` | [] | CIDR / 单 IP 过滤 |
|  | `historyPath` | `ip-history.json` | 历史存储位置 |

> 所有配置均支持热更新；`IpPool::set_config` 会原子替换预热线程、熔断状态与缓存参数。

---
## 4. IP 池总体生命周期
1. **启动阶段**：`app::run` 设置配置基目录 → 加载 `AppConfig` 与 `ip-config.json` → 构建 `IpPool` → 如启用则启动 `PreheatService`。
2. **预热阶段**：`PreheatService` 根据 `preheatDomains` 收集候选、测速、写入缓存与历史。
3. **任务阶段**：传输层调用 `IpPool::pick_best` / `pick_best_blocking` 获取最佳候选；任务完成后调用 `report_outcome` 回写成功/失败。
4. **维护阶段**：每次 `pick_best` 检查是否触发 `maybe_prune_cache`，清理过期与超额条目，同时裁剪历史文件。
5. **异常阶段**：当 IP 熔断或预热持续失败时触发自动禁用；冷却结束自动恢复。
6. **热更新**：`set_config` 重新构建运行期配置、刷新预热计划、清理单飞锁；事件 `IpPoolConfigUpdate` 记录差异。

---
## 5. 预热调度 (PreheatService)
- 入口：`PreheatService::spawn(pool, config)` 使用独立 tokio runtime 2 worker。
- `run_preheat_loop`：
  - `DomainSchedule` 维护每域 `next_due`、`failure_streak`、`backoff`，成功后按 TTL + 提前量续约，失败时指数退避（≤6×TTL）。
  - 候选收集 `collect_candidates`：Builtin / UserStatic / History / DNS / Fallback 多来源合并，白名单优先放行，黑名单立即剔除并发 `IpPoolCidrFilter`。
  - 探测速 `measure_candidates`：信号量限制并发，`probe_latency` 根据 `probeTimeoutMs` 截断，成功返回握手耗时；失败记录日志。
  - 刷新事件：无论成功失败均调用 `emit_ip_pool_refresh`（reason 分别为 `preheat` / `no_candidates` / `all_probes_failed`）。
- 热更新：`request_refresh` 将所有 schedule `force_refresh`，下次循环立即执行；禁用时停止 runtime。
- 自动禁用：当 `failure_streak` 全域 ≥ 阈值触发 `set_auto_disabled("preheat consecutive failures", cooldown)` 并进入冷却等待。

---
## 6. 按需采样与缓存维护
- `IpPool::pick_best`：
  1. 乐观读取缓存有效条目（`IpStat::is_expired`）。
  2. 未命中或过期 → `ensure_sampled`：同域使用 `Notify` 单飞；超时回退系统 DNS。
  3. `sample_once` 复用预热收集/测速逻辑，成功写入缓存 + 历史。
- `report_outcome(IpOutcome)`：记录成功/失败计数、最后一次时间戳、来源；供熔断/观测使用。
- `maybe_prune_cache`：按 `cachePruneIntervalSecs` 间隔触发 `prune_cache`：
  - 删除非预热过期条目；
  - `enforce_cache_capacity` 按 `measured_at` 淘汰最旧条目；
  - `history.prune_and_enforce` 清理过期 + 超容量历史（容量下限 128）。
- 历史写入失败仅日志警告，不阻断主流程；过期记录在下次读取时惰性删除。

---
## 7. 传输层集成与优选决策
- 接入点：`CustomHttpsSubtransport::connect_tls_with_fallback`。
  - 调用 `acquire_ip_or_block` 获取候选序列（缓存快照 + 新采样）。
  - 按延迟升序尝试连接；成功后 `report_candidate_outcome(success)`，失败尝试下一候选，全部失败后回退系统 DNS。
  - 每次选择（包含系统回退）调用 `emit_ip_pool_selection` 记录策略、来源、延迟。
- 线程局部：扩展 `metrics::tl_set_ip_selection` 存储 `ip_source`、`ip_latency_ms`、`ip_selection_stage`，`AdaptiveTlsTiming`/`AdaptiveTlsFallback` 事件读取。
- `pick_best_blocking`：用于同步调用（非 async 上下文）；内部重用 tokio runtime `OnceLock`。
- 熔断与禁用：若 `IpPool::is_enabled` 返回 false（全局禁用或冷却），传输层直接走系统 DNS 并在事件中体现 `strategy=SystemDefault`。

---
## 8. 异常治理：熔断、黑白名单与自动禁用
- **IP 级熔断 (`CircuitBreaker`)**：
  - `record_outcome(ip, result)` 更新滑动窗口；失败率 ≥ 阈值且样本数满足时进入 `Cooldown`，发 `IpPoolIpTripped { reason="failure_rate" }`。
  - 冷却时间到达后自动恢复并发 `IpPoolIpRecovered`。
  - 配置项：`failureThreshold`、`failureRateThreshold`、`failureWindowSeconds`、`cooldownSeconds`、`circuitBreakerEnabled`。
- **黑白名单**：
  - 预热与按需采样前执行：白名单命中直接保留并发 `list_type="whitelist"` 事件；黑名单命中立即丢弃并发 `list_type="blacklist"`。
- **全局自动禁用**：
  - `set_auto_disabled(reason, cooldown_ms)` 比较 `auto_disabled_until`，仅在从未禁用或更短冷却时更新并发 `IpPoolAutoDisable`；冷却中延长仅打印 debug。
  - `clear_auto_disabled()` 返回是否状态切换；成功时发 `IpPoolAutoEnable`，多次调用不会重复发事件。
  - 统一通过 CAS/Swap 保证事件幂等：当禁用尚在生效时 `set_auto_disabled` 只会延长截止时间并写调试日志，避免重复 disable 事件；恢复路径同样只在状态真正切换时广播 enable 事件。
  - 触发源：预热连续失败 / 运维手动调用。

---
## 9. 观测事件与指标
| 事件 | 触发点 | 关键字段 |
|------|--------|----------|
| `StrategyEvent::IpPoolSelection` | 每次选 IP | `strategy` (`Cached`/`SystemDefault`)、`source`、`latency_ms`、`candidates_count` |
| `StrategyEvent::IpPoolRefresh` | 预热/按需刷新 | `success`、`candidates_count`、`min_latency_ms`、`max_latency_ms`、`reason` |
| `StrategyEvent::IpPoolCidrFilter` | 黑白名单匹配 | `ip`、`list_type`、`cidr` |
| `StrategyEvent::IpPoolIpTripped` / `Recovered` | 熔断状态变化 | `ip`、`reason`、`cooldown_until` |
| `StrategyEvent::IpPoolAutoDisable` / `Enable` | 全局禁用切换 | `reason`、`until_ms` |
| `StrategyEvent::IpPoolConfigUpdate` | 热更新 | `old`/`new` JSON 快照 |
| `AdaptiveTlsTiming` | 传输层完成 | `ip_source`、`ip_latency_ms`、`ip_selection_stage` (新增字段可选) |
| `AdaptiveTlsFallback` | 回退事件 | `ip_source`、`ip_selection_stage` (可选) |
| `AdaptiveTlsAutoDisable` | Fake SNI 自动禁用 | 与 P3 保持一致 |

- 事件走 `MemoryEventBus` / 全局总线，可在 tests 中注入。
- 事件发射辅助统一封装在 `ip_pool::events` 模块，测试通过 `install_test_event_bus()` 注入线程本地总线；在并发发布与替换场景下（`event_bus_thread_safety_and_replacement`）已验证不会出现重复或遗漏。
- 指标：在 soak 报告中统计 `selection_total`、`selection_by_strategy`、`refresh_success_rate`、`auto_disable_count`；Prometheus 暂未直接导出。

---
## 10. 历史数据与持久化
- 文件路径：`config/ip-history.json`，结构为 `[ {"domain","port","ip","latency_ms","measured_at_epoch_ms","expires_at_epoch_ms","sources"} ]`。
- `IpHistoryStore` 行为：
  - `load_or_default` 失败时重建空文件并记 `warn`。
  - `upsert` 在写入失败时降级为内存缓存（运行期仍可工作）。
  - `prune_and_enforce(now, capacity)`：移除过期条目，然后按 `measured_at` 裁剪到容量上限，过程中忽略写入错误。
- 文件超过 1 MiB 打印警告，提示调整容量或 TTL。

---
## 11. Soak 准入与阈值
- 环境变量：
  - `FWC_ADAPTIVE_TLS_SOAK=1`（启用）
  - `FWC_SOAK_ITERATIONS` (默认 20)
  - `FWC_SOAK_MIN_SUCCESS_RATE` (默认 0.98)
  - `FWC_SOAK_MAX_FAKE_FALLBACK_RATE` (默认 0.05)
  - `FWC_SOAK_MIN_IP_POOL_REFRESH_RATE` (默认 0.8)
  - `FWC_SOAK_MAX_AUTO_DISABLE` (默认 0)
  - `FWC_SOAK_MIN_LATENCY_IMPROVEMENT` (默认 0.1，支持空字符串跳过)
  - `FWC_SOAK_BASELINE_REPORT` (可选基线 JSON)
- 报告 (`SoakReport`)：`git`、`ip_pool`、`thresholds`、`comparison` 四部分。
  - `ip_pool` 包含 `selection_total`、`selection_by_strategy`、`refresh_success_rate`。
  - `thresholds` 标记 Ready 状态与未通过项。
  - `comparison` 提供与基线差异、回退率、延迟改善、自动禁用次数。
- 无基线时延迟改善标记为 `not_applicable` 并记录原因。

---
## 12. 测试矩阵与关键用例
| 类别 | 路径 | 重点 |
|------|------|------|
| 预热调度 | `preheat.rs` 单元测试 | `DomainSchedule` 退避、刷新、候选合并 |
| 历史存储 | `history.rs` 单元测试 | `get_fresh`、`remove`、`prune_and_enforce` |
| 缓存控制 | `mod.rs` 单元测试 | 单飞互斥、TTL 过期再采样、容量淘汰 |
| 传输集成 | `tests/tasks/ip_pool_manager.rs` | 候选优选、回退、事件发射、`auto_disable_extends_without_duplicate_events` 回归 |
| 预热事件 | `tests/tasks/ip_pool_preheat_events.rs` | 成功、无候选、全部失败场景 |
| 事件向后兼容 | `tests/events/events_backward_compat.rs` | 新增字段与旧版本 JSON 兼容 |
| 熔断/异常 | `tests/tasks/ip_pool_event_emit.rs`、`ip_pool_event_edge.rs` | 熔断触发、黑白名单、热更新并发、事件幂等 |
| Soak | `src-tauri/src/soak/mod.rs` 测试 | 阈值覆盖、报告生成、基线对比 |
| 全量回归 | `cargo test -q --manifest-path src-tauri/Cargo.toml` | Rust 单测/集测 |
| 前端回归 | `pnpm test -s` | 保障事件解析向后兼容 |

---
## 13. 运维说明与回退策略
| 场景 | 操作 | 影响 |
|------|------|------|
| 快速禁用 IP 池 | `ip_pool.enabled=false` 或预热命令停用 | 全部任务走系统 DNS，事件 `strategy=SystemDefault` |
| 手动黑名单 | 更新 `ip-config.json` → `request_refresh` | 下次采样即时生效，被过滤 IP 有事件记录 |
| 清理缓存/历史 | 删除 `ip-history.json` 或调低 TTL | 重新采样，历史丢失不会阻塞 |
| 解除自动禁用 | 等待冷却或手动调用 `clear_auto_disabled` | 事件 `IpPoolAutoEnable` 提示恢复 |
| 调整熔断阈值 | 修改 `config.json` 对应字段并热加载 | 新任务立即采用，旧状态在下一次窗口重新评估 |
| 调试事件 | `RUST_LOG=ip_pool=debug` | 输出详细候选/延迟/退避日志 |

---
## 14. 后续优化建议
1. 引入 Prometheus 指标导出，补齐 `selection_total`、`refresh_success_rate` 等监控。
2. 为预热调度增加提前刷新策略（TTL 前窗口采样），降低冷启动延迟。
3. 历史写入失败可升级为事件/告警，便于运维巡检。
4. 按需域名缓存采用 LRU + 命中统计，进一步提升容量利用率。
5. 传输层与任务层贯通真实 `task_id`，使 Selection/Refresh 事件可直接关联 clone/push。
6. Auto-disable 阈值可根据历史表现自适应（按域/按 IP 分段）。

---
## 15. 快速校验命令
```powershell
# Rust 单元 + 集成测试
cd src-tauri
cargo test -q

# 指定 IP 池相关测试
cargo test --test ip_pool_manager -- --nocapture
cargo test --test ip_pool_preheat_events -- --nocapture
cargo test --test events_backward_compat

# Soak 准入示例（10 轮，生成报告）
set FWC_ADAPTIVE_TLS_SOAK=1
set FWC_SOAK_ITERATIONS=10
set FWC_SOAK_REPORT_PATH=%CD%\soak-report.json
cargo run --bin fireworks-collaboration --features soak

# 前端契约回归
cd ..
pnpm install
pnpm test -s
```
