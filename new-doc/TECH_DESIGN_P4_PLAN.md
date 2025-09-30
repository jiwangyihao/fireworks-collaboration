# P4 阶段技术设计文档 —— IP 优选与握手延迟调度

## 待办事项（2025-10-01）
- [x] 制定提交拆分计划
- [x] 提交：IP 池异常治理与事件增强
- [x] 提交：HTTP 客户端集成 IP 池候选
- [x] 提交：P4.5 文档更新

## 1. 概述

本阶段在 MP0～P3 已完成的 git2-rs 基线、自适应 TLS 传输层（含默认启用、Real-Host 验证、SPKI Pin、自动禁用）与观测框架之上，引入“IP 池与优选”能力。目标是在不破坏现有任务契约的前提下，为白名单域构建可配置的预热域名列表、统一的 IP 收集与 TCP 握手测速管线，依据延迟评分在每次任务开始前选择最优 IP；同时保持评分 TTL 过期机制与回退链协同，确保当网络环境变化或 IP 失效时能快速刷新或退回系统 DNS。

### 1.1 背景
- 当前 Fake→Real→Default 传输链依赖系统 DNS 解析，面对网络抖动或劣质链路难以及时绕开高延迟节点；
- 已在文档中定义的 IP 池数据结构与评分规则尚未实现，需要统一采样、缓存与失效策略；
- 自适应 TLS 已输出 timing / fallback / cert fingerprint 观测，为评估 IP 优选收益提供基础；
- 需兼顾预热域名（GitHub 域族等）与按需域名，两者在评分刷新时序与 TTL 处理不同。

### 1.2 目标
1. 建立可配置的 IP 池服务：支持多来源（内置、DNS、历史、用户静态、兜底）聚合与去重；
2. 统一评分逻辑：仅使用目标端口 TCP 握手延迟（ms）作为优选依据，延迟越小优先级越高；
3. 支持预热域名列表：进程启动即对名单内域名（默认 443，同步兼容 80）完成批量采样与评分，并在 TTL 过期后后台刷新；
4. 支持按需域名：首次使用时即时采样并缓存，评分过期后清除，下次访问重新采样；
5. 与自适应 TLS 协同：向 Fake/Real 阶段提供最优 IP 候选，失败后按现有回退链处理，并记录使用的 IP 与来源；
6. 保障回退与安全：当 IP 池不可用或评分过程失败时自动回退到系统解析；不向前端暴露敏感 IP 列表，事件仅携带来源与延迟摘要。

### 1.3 范围
- 后端 Rust：实现 `ip_pool` 模块、延迟采样任务、评分缓存、TTL 管理、与 transport 的接入；
- 配置：扩展 `ip-config.json` / `config.json` 添加 `preheatDomains`、`scoreTtlSeconds`、来源开关、并提供热加载；
- 数据落地：标准化 `ip-history.json` 记录最近一次成功握手（IP、端口、来源、延迟、expires_at）；
- 前端：仅显示可选的“当前任务使用 IP 来源/延迟”信息（非必须）；
- 文档与运维：更新配置说明、观测指标、告警策略、回退手册。

### 1.4 不在本阶段
- 代理失效自动切换（P5）；
- IP 池信誉评分或权重学习（未来阶段）；
- LFS/大文件专项优化；
- 真实多路径并行尝试或 Happy Eyeballs 类机制；
- Frontend UI 的复杂 IP 诊断面板（延后）。

### 1.5 成功标准
| 指标 | 目标 | 说明 |
|------|------|------|
| 预热域名覆盖率 | 100% | 列表内域名在启动后成功完成首轮采样 |
| 默认任务延迟改善 | ≥15% | 与系统 DNS 相比，p50 connect+tls 总延迟降低 |
| 评分刷新及时性 | ≤ TTL | 预热域名在 TTL 到期后自动刷新，按需域名过期后被清除 |
| 回退成功率 | ≥99% | 当 IP 池不可用时自动回退系统解析，任务不失败 |
| 指标完整性 | 100% | 事件/日志包含 IP 来源、延迟摘要，便于观测 |
| 回退开关响应 | <5s | 禁用 IP 优选后新任务立即回退系统解析 |

### 1.6 验收条件
1. 预热列表在启动阶段完成首次采样，生成 `ip-history.json` 条目并带有 `expires_at`；
2. 按需域名在首次任务时同步采样，并在 TTL 到期后条目被清除，再次访问重新采样；
3. 延迟评分结果被 transport 使用，任务事件或 debug 日志可看到选中的 IP、来源与延迟；
4. 注入 IP 失效或握手失败时，任务能回退到系统 DNS，且统计的 fallback 事件记录原因；
5. 配置变更（启用/禁用 IP 池、调整 TTL）可热加载，影响新任务；
6. 所有新增单元/集成测试通过，现有回归测试无失败；
7. 文档、配置样例、运维手册更新完毕。

### 1.7 交付物
- 代码：`core/ip_pool` 模块、采样调度器、延迟测量器、transport 接入改造；
- 配置：更新 `config.json`、`ip-config.json`，添加预热域名、TTL、来源开关、最大并发等字段；
- 数据：规范化 `ip-history.json` 结构（IP、port、sources、latency_ms、measured_at、expires_at）；
- 观测：新增 `ip_selection` 相关事件/日志与指标；
- 测试：单元测试、故障注入、与 soak 脚本扩展；
- 文档：P4 设计文档、配置指南、故障排查手册。

### 1.8 回退策略
| 场景 | 操作 | 影响 |
|------|------|------|
| IP 池整体异常 | 设置 `ipPool.enabled=false` | 立即使用系统 DNS，不再读写 ip-history |
| 单域连通性差 | 移除该域出预热列表或手动禁用 | 对应域回退系统解析 |
| 评分逻辑异常 | 降级为随机 IP 或回退系统 DNS | 保持任务成功，失去优选收益 |
| 历史文件损坏 | 自动重新生成空缓存 | 失去历史记录，但任务继续 |
| 观测噪声过大 | 降低事件等级或关闭可选字段 | 不影响核心流程 |

### 1.9 关键依赖与假设
- 网络环境允许在后台执行 TCP 握手测试（不会被防火墙阻断）；
- 预热域名列表主要为 GitHub 域族，数量可控；
- 延迟测量对远端无副作用（采用 SYN+ACK 完成后立即关闭连接）；
- 现有自适应 TLS transport 可接受外部提供的 IP（连接接口支持自定义 socket address）；
- 配置热加载机制已在 P3 阶段建立，可复用；
- Soak 工具可扩展以模拟 IP 池行为空主线验收手段。

### 1.10 风险概览
| 风险 | 等级 | 描述 | 缓解 |
|------|------|------|------|
| TCP 预热被远端限流 | 中 | 高频握手导致被网络策略限制 | 控制并发 + 指数退避，必要时支持采样间隔扩展 |
| 评分过期不及时 | 中 | 预热刷新失败导致使用旧 IP | 后台刷新失败报警 + 回退系统解析 |
| 历史文件膨胀 | 低 | 大量按需域名缓存 | TTL 到期自动清理 + 最大容量限制 |
| 配置误操作 | 中 | 误删预热域名或禁用池 | 回退默认 + 运维审计 log |
| IP 池污染 | 低 | 收集到恶意 IP | 来源白名单 + 手动黑名单/回滚 |
| 观测信息敏感 | 低 | 事件包含 IP | 默认脱敏为前缀+来源，详尽信息仅写 debug 日志 |

### 1.11 兼容与迁移
| 旧版本行为 | P4 调整 | 保证措施 |
|--------------|-----------|-----------|
| 所有任务使用系统 DNS | 引入可选 IP 池优选 | 默认保留回退开关，配置缺省可关闭 |
| 无 `ip-config.json` 预热字段 | 新增 `preheatDomains`、`scoreTtlSeconds` | 缺省填空数组/默认 300 秒，向后兼容 |
| 无历史缓存 | 新增 `ip-history.json` 结构 | 首次运行自动生成；损坏时重建 |
| transport 不识别 IP 来源 | 扩展传输上下文携带 IP 来源 | 未启用 IP 池时字段为空 |
| Soak 不关注 IP 优选 | 扩展脚本输出 IP 指标 | 可配置开关禁用 |

## 2. 详细路线图

### 子阶段划分
| 阶段 | 主题 | 核心关键词 |
|------|------|------------|
| P4.0 | 基线架构与配置打通 | 模块化 ip_pool / 配置解析 / 缓存骨架 |
| P4.1 | 预热域名采样与调度 | 启动探测 / 并发控制 / 指标占位 |
| P4.2 | 按需域名评分与 TTL 管理 | 首次采样 / 缓存写入 / 过期清理 |
| P4.3 | 传输层集成与优选决策 | Transport 接口 / Retry 协调 / 回退策略 |
| P4.4 | 观测、日志与数据落地 | 事件扩展 / 指标 / `ip-history.json` 落盘 |
| P4.5 | 异常治理与回退控制 | 熔断 / 黑白名单 / 系统 DNS 回退 |
| P4.6 | 稳定性验证与准入 | Soak / 指标阈值 / 报告输出 |

### P4.0 基线架构与配置打通
- **目标**：建立独立的 `ip_pool` 基础模块，完成配置解析、缓存骨架和测试支撑，使后续子阶段在不影响现有传输链的情况下增量接入评分逻辑。
- **范围**：
	- 新建 `core/ip_pool/{mod.rs,cache.rs,config.rs}`，定义 `IpCandidate`、`IpStat`、`IpScoreCache` 等核心数据结构；
	- 加载 `ip-config.json` 与 `config.json` 中与 IP 池相关的新字段（`preheatDomains`、`scoreTtlSeconds`、来源开关、并发上限等），支持热加载；
	- 设计统一的来源枚举（Builtin/History/UserStatic/Dns/Fallback）与去重策略；
	- 预留与 transport 的接口（如 `IpPool::pick_best(domain, port)`、`IpPool::report_outcome`），当前返回占位结果并回退系统解析。
- **交付物**：
	- 模块骨架与单元测试（配置默认值、枚举序列化、缓存读写幂等）；
	- 新增配置示例与文档说明；
	- `ip-history.json` 结构草案（尚未写入实际数据）。
- **依赖**：复用 P3 阶段的配置加载/热更新机制；无外部服务依赖。
- **验收**：
	- 后端可在无评分实现的情况下顺利编译、运行；
	- 新配置项缺省值不破坏现有任务；
	- 单元测试覆盖配置解析与缓存操作。
- **风险与缓解**：
	- 配置兼容风险 → 提供默认值并在日志中提示新字段启用状态；
	- 模块侵入度 → 通过 trait 接口与现有 transport 解耦，仅在 P4.3 进行实际接入。

### P4.1 预热域名采样与调度
- **目标**：对配置的 `preheatDomains` 在进程启动后立即完成多来源 IP 收集与 TCP 握手测速，生成初始评分并为后续任务提供缓存；同时构建周期性刷新与并发控制机制。
- **范围**：
	- 解析 `preheatDomains` 列表，与来源开关结合生成采样计划；
	- 实现多来源收集器（内置、DNS、历史、用户静态、兜底）及统一的去重与优先级；
	- 构建 TCP 握手测速器，支持 80/443 端口，记录 `latency_ms`、`measured_at`、`expires_at`；
	- 实现启动阶段的批量调度器（有界并发 + 失败退避 + 可观测日志）；
	- 设计后台刷新任务：在 TTL 到期前触发重新采样，失败时按指数退避重试，并在连续失败后回退系统 DNS。
- **交付物**：
	- 预热调度器与单元测试（并发度控制、失败重试、TTL 刷新）；
	- `ip-history.json` 首次持久化写入（含来源列表、延迟、expires_at）；
	- 预热过程的结构化日志与 debug 事件（如 `ip_pool_preheat_started/completed`）。
- **依赖**：依赖 P4.0 的缓存与配置骨架；DNS 解析复用现有 http 或系统解析模块。
- **验收**：
	- 启动后预热域名均写入缓存并包含延迟与过期时间；
	- 并发度可配置且超阈时阻塞新采样；
	- TTL 到期后自动刷新成功，失败时记录事件并退避；
	- 预热失败不会阻塞核心任务（自动回退系统 DNS）。
- **风险与缓解**：
	- 预热频率过高 → 为每个域名引入最小刷新间隔与失败退避；
	- TCP 预热被远端限流 → 限制每主机并发数并提供节流配置；
	- 历史文件损坏 → 遇到解析错误自动重建空结构并重新采样。
- **提交范围**：
    - 新增 `src-tauri/src/core/ip_pool/preheat.rs`，内置多来源候选收集器（Builtin/DNS/History/UserStatic/Fallback）、并发受控的 TCP 握手测速器以及基于 `tokio` 运行时的循环调度器；默认在首轮完成后按 `scoreTtlSeconds` 全量刷新。
    - 新增 `src-tauri/src/core/ip_pool/history.rs`，定义 `IpHistoryStore` 与 `IpHistoryRecord`，支持基于 `config/ip-history.json` 的懒加载、容错重建与内存降级。
    - 扩展 `IpPoolFileConfig`，引入 `userStatic` 配置用于静态 IP 声明；`IpStat` 追加 `sources` 字段以记录合并来源；`IpPool` 负责生命周期内预热任务的创建、热更新与历史存档访问。
    - 追加内置/兜底 IP 白名单、配置刷新触发接口 `PreheatService::request_refresh`，并在 `set_config` 热更新时自动替换预热线程。
- **差异记录**：
	- 预热调度器改为维护逐域计划，成功路径按 TTL 续约，失败路径采用最大 6×TTL 的指数退避，并支持热更新/手动刷新即时重置队列；后续若需要提前量可在 P4.2 拓展。
	- Fallback 来源现阶段仅暴露一组静态地址，占位以保留枚举通路；动态兜底逻辑待后续阶段补齐。
	- 历史候选在采集阶段会判定过期并自动剔除，避免重复使用陈旧 IP，同时在落盘失败时记录告警日志。
- **测试与验收**：
	- 新增单元测试覆盖 `IpHistoryStore` 读写、`collect_candidates` 去重合并及过期历史剔除、`DomainSchedule` 退避/刷新行为、`probe_latency` 超时以及缓存/历史写入幂等。
    - 已执行 `cargo fmt` 与 `cargo test -q`，全部用例通过；运行期间仅保留既有 Git fixture 的噪声日志，无新增 warning。
    - 人工验证 `config/ip-config.json` 自动生成 `userStatic` 默认字段，预热禁用场景未启动后台线程。

### P4.2 按需域名评分与 TTL 管理
- **目标**：为预热列表之外的域名提供首访即时采样与评分，统一缓存写入、TTL 生命周期与过期清理策略，确保按需域名在网络环境变化时能自动刷新。
- **范围**：
	- 扩展 `IpPool::pick_best`：当缓存缺失或过期时触发即时采样（收集来源、握手测速、写入缓存与历史文件）；
	- 引入写路径单飞机制，避免同一域名被并发重复采样；
	- 实现 TTL 过期清理器：周期性扫描缓存，将过期条目移除（按需域名不做后台刷新，直接删除以促使下次再采样）；
	- 在任务结束路径新增 `IpPool::report_outcome`，记录成功/失败供后续熔断统计；
	- 形成命中率、再采样次数等内部指标，为 P4.4 观测输出做准备。
- **交付物**：
	- 按需采样逻辑与单元测试（缓存命中、并发、过期）；
	- 缓存清理任务与配置项（扫描间隔、最大容量）；
	- 更新 `ip-history.json` 写入策略（按需条目过期即删除、不保留失败记录）。
- **依赖**：复用 P4.1 的测速器与来源聚合；需要 transport 层在任务完成时调用 `report_outcome`。
- **验收**：
	- 同一域名首访时写入缓存并返回延迟；
	- 第二次访问命中缓存，无需重复采样（测试验证）；
	- TTL 到期后条目被清除并在下一次访问重新采样；
	- 同域并发请求仅触发一次采样，其余等待结果。
- **风险与缓解**：
	- 缓存膨胀 → 设置最大条目数 + LRU 淘汰策略；
	- 单飞阻塞时间过长 → 设置采样超时并在失败时回退系统 DNS；
	- 清理逻辑误删预热条目 → 在缓存结构中区分预热与按需标记，清理器仅处理按需条目。
- **实现说明**
- **提交范围**：
    - `src-tauri/src/core/ip_pool/mod.rs` 重写为异步取用：`IpPool::pick_best` 通过单飞 (`tokio::sync::Mutex<HashMap<IpCacheKey, Arc<Notify>>>`) 串行化同域采样，新增 `ensure_sampled`、`sample_once`、`maybe_prune_cache`、`enforce_cache_capacity` 等步骤；同时引入 `OutcomeStats` 与 `report_outcome` 计数逻辑、`outcome_snapshot` 测试辅助方法。
    - `src-tauri/src/core/ip_pool/config.rs` 扩展运行期配置字段 `cachePruneIntervalSecs`、`maxCacheEntries`、`singleflightTimeoutMs` 并补充默认值与测试覆盖。
    - `src-tauri/src/core/ip_pool/history.rs` 新增 `remove` 接口，支持 TTL 过期或容量控制时同步清理历史记录。
    - `src-tauri/src/core/ip_pool/preheat.rs` 公开 `collect_candidates`/`measure_candidates`/`update_cache_and_history`/`probe_latency`，供按需路径复用；`AggregatedCandidate` 调整为 `pub(super)`。
    - `src-tauri/src/core/ip_pool/mod.rs` 单元测试新增 `on_demand_sampling_uses_user_static_candidate`、`ttl_expiry_triggers_resample`、`single_flight_prevents_duplicate_sampling`，并把既有缓存命中测试迁移到异步风格。
	- `src-tauri/src/core/ip_pool/cache.rs` 内联 `IpSource`、`IpStat::is_expired` 与缓存枚举，替代独立的 `source.rs`/`time.rs`，保持缓存读取、过期判断与来源追踪在同一处维护。
	- `src-tauri/src/core/ip_pool/manager.rs` 合并原 `selection.rs`、`outcome.rs` 内容，集中定义 `IpSelection`、`IpOutcome`、`OutcomeMetrics` 等结构；`mod.rs` 仅保留必要的 `pub use`，减少跨文件跳转成本并便于后续 P4.3 扩展。
- **关键代码路径与行为**：
	- `IpPool::pick_best` 先检查缓存有效性（`expires_at` 缺省视为永不过期），命中失败时触发 `ensure_sampled`；若采样仍失败则回退系统 DNS。
	- `ensure_sampled` 以 `Notify` 单飞等待其他协程结果，并在超时（默认 10s，可配置）时返回系统解析；`sample_once` 复用预热探测逻辑并写入 `IpScoreCache` + `IpHistoryStore`。
	- `maybe_prune_cache` 使用 `AtomicI64` 控制 TTL 清理节奏，按配置间隔移除非预热域名的过期条目，并调用 `enforce_cache_capacity` 基于 `measured_at` 进行最久淘汰。
	- `report_candidate_outcome` 与 `candidate_outcome_metrics` 组合提供 per-IP 统计视图：记录成功/失败计数、最近一次时间戳与来源集合，并在 `report_outcome` 的聚合层补全 `last_outcome_ms`，为后续熔断与观测打通数据链路。
	- `report_outcome` 在非系统路径下记录成功/失败计数和最近时间，为后续 P4.5 熔断铺路。
	- `IpSelectionStrategy::pick`、`IpSelection::from_cache_snapshot` 迁移至 `manager.rs` 后直接依赖缓存内联的 `IpStat::is_expired`；`apply_probe_result` 在同一文件内串联 `OutcomeStats` 更新与缓存写入，确保候选筛选、延迟排序与失败计数拥有一致的上下文。
- **设计差异与取舍**：
	- TTL 清理采用“按需触发”策略（在 `pick_best` 时判定 interval），未额外保留常驻后台线程；满足资源约束同时与原方案目标一致。
	- 当历史文件持久化失败时仅记录告警并保留内存态，避免影响链路；未来可结合 P4.4 指标进一步暴露。
	- 缓存容量限制仅统计非预热域名，预热域维持由调度器负责；淘汰顺序使用 `measured_at`，暂未实现 LRU 更精细策略。
	- `pick_best` 现已异步，后续 P4.3 集成传输层需配合调用点改造；Tauri 端仍通过外层 `Mutex` 序列化访问，未引入 API 破坏。
	- 为降低模块碎片化并支撑后续观测与熔断决策，本阶段合并了不足 50 行的 `selection.rs`、`outcome.rs`、`time.rs`，牺牲部分文件长度换取上下文集中，减少未来重构时的同步成本。
- **测试与验收**：
	- 新增单测覆盖单飞互斥、TTL 过期再采样、缓存命中与默认回退行为，均使用本地 `TcpListener` 验证真实握手。
	- `IpHistoryStore::remove`、配置默认值、序列化反序列化均有独立测试，覆盖新字段。
	- 本阶段运行 `cargo fmt` 与 `cargo test -q --manifest-path src-tauri/Cargo.toml` 全量通过，测试日志仅保留既有 git fixture 的噪声（预期）。
	- `src-tauri/tests/tasks/ip_pool_manager.rs` 集成测试扩展预热命中、按需采样以及失败回退用例，验证合并后公开 API 与外部可观测数据保持一致。
- **运维与配置落地**：
    - 新增运行期字段写入 `config.json` 后可以热生效，默认 `cachePruneIntervalSecs=60`、`maxCacheEntries=256`、`singleflightTimeoutMs=10000`，满足常规场景；文档需提醒按需调大或关闭（设为 0）容量限制。
    - `IpPool` 热更新时清空单飞地图并重建预热线程，确保配置切换后不遗留旧任务。
    - 默认仍使用磁盘 `ip-history.json`，但若路径异常会降级为内存模式并告警，不阻断任务。
- **残留风险**：
    - TTL 清理依赖任务触发，极低流量场景可能延迟释放过期条目；后续可结合调度器心跳优化。
    - 历史文件写入失败仍只记录日志，未向前端 surface；需在 P4.4/运维手册中补充巡检策略。
    - `singleflightTimeoutMs` 过小可能导致频繁回退系统 DNS（当前默认 10s 较保守），配置误设需结合运维监控。
    - 现阶段 `OutcomeStats` 仅计数未做策略使用，若长时间运行可能累积较大 HashMap；P4.5 引入熔断时需增量治理。

### P4.3 传输层集成与优选决策
- **目标**：在不破坏自适应 TLS 既有回退链的前提下，将 IP 池评分结果注入 Fake/Real 阶段连接建立流程，形成可回退的优选决策；同时记录失败结果为后续熔断提供数据。
- **范围**：
	- 修改 `CustomHttpsSubtransport::connect_tcp`/`connect_tls_with_fallback`，在创建连接前调用 `IpPool::pick_best(host, port)` 获取评分最高的 IP；
	- 支持多 IP 候选顺序尝试：优先评分最低者，失败后可尝试下一候选，最终回退系统 DNS；
	- 将选中 IP、来源、延迟通过 thread-local 保存，并扩展 timing 事件携带 `ipSource`、`ipLatencyMs`（可选字段）；
	- 在连接失败时调用 `IpPool::report_outcome` 反馈失败信息，供 P4.5 的熔断或黑名单策略使用；
	- 与 Retry 机制对齐：确保 Fake 阶段网络错误触发 Real 重试前，可切换至下一 IP 或系统 DNS；保持错误分类稳定。
- **交付物**：
	- 传输层改造代码、回退链单元/集成测试（成功、单 IP 失败、多 IP 失败回退）；
	- 事件/日志扩展：`used_ip_source`、`used_ip_latency_ms` 等字段；
	- 配置开关 `ipPool.enabled`（布尔），支持即时禁用 IP 优选。
- **依赖**：依赖 P4.0～P4.2 的缓存与评分；需要与 P3 的 timing/fallback thread-local 结构协同。
- **验收**：
	- 启用 IP 池时任务日志显示所选 IP 与来源；
	- 禁用 IP 池后恢复系统 DNS（事件中 `used_ip_source` 为空）；
	- IP 连接失败时自动尝试下一候选或系统 DNS，最终任务成功率不下降；
	- Retry 触发次数与 P3 基线一致，无额外重复尝试。
- **风险与缓解**：
	- 候选切换导致连接时间上升 → 限制候选尝试次数（默认 2），失败立即回退；
	- 事件暴露敏感信息 → 默认只输出来源枚举与延迟，详细 IP 仅写 debug 日志；
	- 错误分类漂移 → 保持现有分类映射，并新增回归测试覆盖 IP 池路径。

### P4.4 观测、日志与数据落地
- **目标**：完善 IP 池运行期的可观测性与数据持久化，确保延迟评分、来源分布、回退原因等信息在事件、日志与指标中可追踪；同时规范 `ip-history.json` 的写入与滚动策略。
- **范围**：
	- 扩展 Strategy 事件（例如 `AdaptiveTlsTiming`、`AdaptiveTlsFallback`）附加 `ipSource`、`ipLatencyMs`、`ipSelectionStage` 等可选字段；
	- 新增信息事件 `ip_pool_selection`、`ip_pool_refresh` 记录选中 IP、来源、延迟范围（脱敏）；
	- 定义指标：`ip_pool_selection_total{source}`、`ip_pool_fallback_total{reason}`、`ip_pool_latency_ms_bucket`；
	- 规范 `ip-history.json` 结构与滚动策略：超过阈值（容量/文件大小）时合并或裁剪旧记录；
	- 提供调试日志（等级 debug）输出详细 IP 列表，仅在本地或特定 flag 下开启。
- **交付物**：
	- 事件/指标实现与测试（确保 metricsEnabled=false 时事件仍可输出）；
	- `ip-history` 管理器（读写、滚动、错误恢复）的单元测试；
	- 更新运维文档与 Soak 报告结构（增加 IP 优选指标）。
- **依赖**：依赖 P4.3 thread-local 扩展；指标体系复用 P3 的 metrics collector；文件写入需复用 P3.2 的日志滚动工具类。
- **验收**：
	- 任务事件中可看到 IP 来源与延迟字段（旧客户端忽略不报错）；
	- 指标在测试环境可导出并统计；
	- `ip-history.json` 达到大小阈值后正确滚动；
	- metricsEnabled=false 时指标停用但信息事件仍可用。
- **风险与缓解**：
	- 观测噪声过大 → 默认仅在采样/刷新/回退时发事件，提供采样率配置；
	- 历史文件频繁写入 → 批量落盘或写入缓冲；
	- 脱敏不足 → 仅输出来源与延迟区间，完整 IP 只在 debug 模式输出。

### P4.5 异常治理与回退控制
- **目标**：当 IP 池出现持续失败、错误评分或异常延迟时，能够快速熔断、拉黑问题 IP，并回退到系统 DNS，保障任务稳定性；同时为手动运维提供调试接口。
- **范围**：
	- 引入 IP 级失败统计：结合 `report_outcome` 记录连续失败次数与最近失败时间；
	- 实现自动熔断策略：同一 IP 在窗口内失败率超过阈值时临时拉黑（加入冷却列表），并触发 `ip_pool_auto_disable` 事件；
	- 支持手动黑名单/白名单配置，热加载后即刻生效；
	- 当整个平台 IP 池状态异常（预热连续失败、评分超时）时，触发全局回退（等价于 `ipPool.enabled=false`）并在冷却后尝试恢复；
	- 调整 transport 决策：当检测到熔断状态时直接跳过 IP 池，避免重复失败；
	- 与自适应 TLS fallback/auto-disable 事件对齐，形成统一的故障观察面板。
- **交付物**：
	- 熔断状态机与测试（触发、冷却恢复、黑名单优先级）；
	- 新增配置 `ipPool.failureThreshold`, `ipPool.cooldownSeconds`, `ipPool.blacklist`, `ipPool.whitelist`；
	- 事件与日志：`ip_pool_auto_disable`, `ip_pool_blacklist_hit` 等。
- **依赖**：依赖 P4.2 的 `report_outcome` 数据与 P4.3 的传输接入；需要与 P3.5 的 adaptive TLS auto disable 区分并协同性能。
- **验收**：
	- 注入重复失败时触发熔断，后续任务回退系统 DNS；
	- 冷却到期后恢复使用 IP 池，事件记录恢复信息；
	- 手动黑名单生效后直接跳过被拉黑 IP；
	- 全局回退时任务成功率保持与关闭 IP 池时一致。
- **风险与缓解**：
	- 熔断阈值误设导致频繁回退 → 默认阈值保守并提供运维监控；
	- 黑名单误配置 → 提供诊断事件和快速撤销接口；
	- 熔断与自适应 TLS 同时触发 → 日志和事件明确来源，避免混淆。

### P4.6 稳定性验证与准入
- **目标**：通过 soak、故障注入与准入评审验证 IP 优选链路在长时间运行下的稳定性与收益，为生产灰度提供可执行的准入结论。
- **范围**：
	- 扩展 `soak/` 脚本与 GitHub Actions 任务，模拟预热域名与按需域名的混合任务，收集延迟、回退与熔断指标；
	- 设计覆盖异常场景的故障注入脚本（模拟 IP 失效、握手超时、配置切换），确保回退链路按预期触发；
	- 定义准入阈值（延迟改善、回退比率、熔断触发次数）并编写自动化报告，标记达标与否；
	- 与运维协作制定灰度计划、监控看板与手动回滚手册，明确启用/禁用流程；
	- 汇总测试数据，形成最终 P4 阶段 readiness review 结论，输出到技术设计与运维文档。
- **交付物**：
	- 更新后的 soak 脚本与 CI 配置、带有 IP 优选指标的测试报告模板；
	- 故障注入与准入 checklist 文档，包含触发步骤与预期结果；
	- readiness review 会议纪要与上线建议（包含灰度范围、监控项、回滚条件）。
- **依赖**：依赖 P4.1～P4.5 功能完整并在测试环境可用；需要 CI 环境与 soak 集群具备外网访问与日志采集能力；准入评审需协调运维与安全团队时间窗口。
- **验收**：
	- 连续 soak >=24 小时期间，IP 优选路径任务成功率与延迟指标符合阈值；
	- 故障注入场景均触发回退/熔断并在日志、指标中可追踪；
	- 准入报告明确给出上线/灰度建议与需要关注的风险项，获得相关团队签字确认；
	- 灰度开关演练通过（启用、禁用、回滚全程 <5 分钟，日志完整）。
- **风险与缓解**：
	- Soak 环境噪声掩盖收益 → 引入对照组（系统 DNS）并拉长观测窗口；
	- 准入阈值过严导致迟迟不能上线 → 分阶段设定基线/目标值，并在评审中讨论调整；
	- 协同团队时间冲突 → 提前预约评审窗口，准备异步报告供审阅。

## 3. 实现说明

以下章节预留给后续交付后的实现复盘，结构对齐 P3 文档。每个子阶段完成后请在对应小节补充：
- 关键代码路径与文件列表；
- 实际交付与设计差异；
- 验收/测试情况与残留风险；
- 运维手册或配置样例的落地状态。

### P4.0 基线架构与配置打通 实现说明
- **提交范围**：
    - 新增 `src-tauri/src/core/ip_pool/{mod,config,cache}.rs`，定义 `IpPool`、`EffectiveIpPoolConfig`、`IpPoolRuntimeConfig`、`IpPoolFileConfig`、`IpCandidate` 及 `IpScoreCache`，完成运行期配置与磁盘 `ip-config.json` 读写骨架。
    - `src-tauri/src/core/config/model.rs` 引入 `IpPoolRuntimeConfig` 字段，CLI/Tauri 两端共享默认值。
    - `src-tauri/src/app.rs` 将 `IpPool` 注册为全局 `State`，`set_config` 命令热更新运行期配置并刷新磁盘配置。
	- `src-tauri/src/core/config/loader.rs` 新增 `set_global_base_dir`，在 Tauri `setup` 阶段注入配置目录，保证 CLI/桌面端共享的 `config/` 与 `ip-config.json` 均落在同一根目录。
- **关键代码路径**：
    - `IpPool::pick_best`：在运行期禁用时直接回退系统 DNS，启用后优先返回缓存 `IpScoreCache` 中的 `best` 候选；缓存 miss 路径继续回退，确保 P4.0 不引入真实探测逻辑。
    - `load_effective_config_*`：以 `AppConfig.ip_pool` 为运行期来源、`ip-config.json` 为文件来源，组合成 `EffectiveIpPoolConfig`；支持 `base_dir` 覆盖方便测试。
    - `config::load_or_init_file_at`：首次运行落地默认配置，写入成功后通过 `tracing::info` 打印路径，便于运维定位生成文件。
	- `app::run` `setup` 钩子：统一推导配置基目录 → 调用 `cfg_loader::set_global_base_dir` → 初始化 `AppConfig` 与 `IpPool`，并在读取文件失败时降级为默认配置但仍记录错误，保证 UI 可继续启动。
- **数据流概览**：
	- 桌面端启动时通过 `cfg_loader::load_or_init_at` 与 `ip_pool::load_effective_config_at` 同步拉起两个配置文件，确保 UI 与 IP 池共享同一份运行期快照。
	- `set_config` 命令写入 `config.json` 后立即重建 `EffectiveIpPoolConfig`，再通过共享 `Arc<Mutex<IpPool>>` 更新全局状态，后续阶段可在相同入口追加缓存刷新或事件广播。
	- CLI/测试场景若未设置全局基目录，将回退到 `dirs::config_dir()` 或当前目录，文档需提醒运维保持目录一致以复用评分缓存。
- **设计差异与取舍**：
    - `IpPool::report_outcome` 暂仅写 debug 日志，未落地统计；与最初草案一致，将统计与评分推迟至 P4.2/P4.3，避免 P4.0 引入未完成的评分逻辑。
    - 缓存实现采用 `RwLock<HashMap<...>>`，未引入跨进程持久化；满足基线阶段“仅暴露结构”的目标。
- **测试与验证**：
    - `ip_pool::mod` 单测覆盖禁用回退、启用时命中缓存、运行期配置热更新，以及 `load_effective_config_at` 在定制基目录下的组合结果；同时校验损坏的 `ip-config.json` 会返回错误。
    - `config.rs` 单测验证运行期/文件默认值、`load_or_init_file` 的首次生成、`save_file` 的持久化，以及 JSON 反序列化默认填充。
	- `cache.rs` 单测覆盖 `insert/get/remove/clear/snapshot` 读写路径，确保缺省 `IpCandidate` 可安全构造；`config::model` 额外断言 `ipPool.enabled` 默认关闭，避免旧配置升级后误启用。
    - 运行 `cargo test -q` 全量通过，warning 仅来自现有 git fixture（未新增告警）。
- **运维与配置落地**：
    - 默认生成的 `config/ip-config.json` 包含空的 `preheatDomains` 与 300s TTL；后续阶段可直接编辑生效。
	- `cfg_loader::set_global_base_dir` 会把配置写入应用数据目录（如 Windows `%APPDATA%\top.jwyihao.fireworks-collaboration\config\`），命令行测试可通过环境覆盖使用相同目录，以便后续阶段共享评分历史。
	- `AppConfig.ip_pool.historyPath` 暂保留接口未实现文件加载，文档继续提示待后续阶段接入。
- **残留风险**：
    - `set_config` 默认忽略 `IpPool` 锁竞争失败，仅写日志；若未来 UI 需要反馈，需要补充错误提示。
	- 文件读写失败目前仅在日志中呈现，没有向前端透出；待 P4.2 以后结合事件中心补齐。
	- `load_effective_config_at` 在读取失败时落回默认配置可能掩盖配置丢失，需要运维结合日志 `load ip pool config failed` 定期巡检。

### P4.1 预热域名采样与调度 实现说明
- **提交范围**：
	- `PreheatService::spawn` 通过独立 2 worker 的 tokio runtime 驱动 `run_preheat_loop`，维护 `stop_flag`/`Notify` 以支持热更新中断与 `request_refresh` 手动刷新。
	- `run_preheat_loop` 基于 `EffectiveIpPoolConfig.file.preheat_domains` 初始化 `DomainSchedule`，按 TTL 与 backoff 顺序调度 `preheat_domain`，并在停机或刷新请求时重置队列。
	- `collect_candidates` 使用 `AggregatedCandidate` 对 builtin/userStatic/history/dns/fallback 五类来源去重合并，历史来源经 `IpHistoryStore::get_fresh` 自动剔除过期项；`measure_candidates` 结合信号量控制并发、裁剪探测超时并按延迟排序。
	- `update_cache_and_history` 将最低延迟的 `IpStat` 写入 `IpScoreCache`，同步落盘 `IpHistoryStore`，保留 `sources`、`measured_at_epoch_ms` 与 `expires_at_epoch_ms`，确保缓存与历史一致。
	- 在 `IpPool::set_config` 热更新路径上替换 `PreheatService` 实例，变更配置后立即触发全量刷新；`userStatic` 配置项首次生成时自动持久化。
- **关键代码路径**：
	- `DomainSchedule` 将 TTL 作为 `min_backoff`，失败时按 2 倍指数退避并封顶为 `FAILURE_BACKOFF_MULT_MAX`（6×TTL），`next_due_schedule` 按 `next_due` 选取最早待执行域，避免饥饿；`force_refresh` 用于热更新或手动触发，立即重置调度。
	- `measure_candidates` 通过 `Semaphore(max_parallel_probes)` 限流单域探测，`probe_timeout_ms` 在 100ms 与 `MAX_PROBE_TIMEOUT_MS`（10s）之间裁剪，探测成功与失败均输出 `ip_pool` target 日志便于观测。
	- 历史写读依赖 `IpHistoryStore::upsert` 与 `get_fresh`：成功探测写入新记录，过期记录在下一次读取时删除并尝试持久化，防止陈旧候选继续参与排名。
	- 候选来源在 `collect_candidates` 中做集合并集，`AggregatedCandidate::to_stat` 将来源写回 `IpStat.sources`，保证后续观测可以区分采样链路。
- **差异记录**：
	- 当前实现依旧采用“整表 TTL 到期后统一刷新”，未实现设计原稿中的按域提前刷新；通过 `request_refresh` 或配置热更新可提前启动一次全量刷新。
	- Fallback 来源继续是静态占位 IP，动态兜底与退避事件指标计划在 P4.4/P4.5 引入；本阶段仅保证通路完整。
	- 为避免极小 TTL 导致循环自旋，引入 `MIN_TTL_SECS=30` 的硬下限，并以 `FAILURE_BACKOFF_MULT_MAX=6` 限制最大退避时长，这是对原方案的补充约束。
- **测试与验收**：
	- 新增 `next_due_schedule_selects_earliest_entry`、`domain_schedule_backoff_caps_after_retries`、`domain_schedule_force_refresh_resets_state` 验证调度排序、退避封顶与刷新语义；`collect_candidates_merges_sources_from_history`、`collect_candidates_skips_expired_history_entries` 覆盖来源合并与过期剔除。
	- `IpHistoryStore` 增补 `get_fresh_returns_valid_record`，与原有 `get_fresh_evicts_expired_records` 一起验证历史读取同时保留新鲜记录、删除过期记录的对称性。
	- `update_cache_and_history_writes_best_entry`、`probe_latency_times_out_reasonably` 等用例确认探测速率、缓存写入与失败超时路径；CI 执行 `cargo fmt`、`cargo test -q` 全量通过，仅保留既有 git fixture 噪声。
	- 人工验证 `ipPool.enabled=false` 或 `preheatDomains` 为空时预热线程退出，`set_config` 热更新会触发立即刷新，`config/ip-config.json` 默认生成空 `userStatic` 字段。
- **运维与配置落地**：
	- `max_parallel_probes`、`probe_timeout_ms` 可运行期调整，预热线程固定 2 worker；建议在带宽有限环境调低并发以避免对目标域造成突发握手压力。
	- 静态 IP 可通过 `userStatic` 配置补充，修改后可调用 `request_refresh` 立即采样；历史文件存于 `config/ip-history.json`，若解析失败会自动重建并打印 `ip_pool` 告警。
	- 预热日志集中在 `ip_pool` target，失败日志包含 `failure_streak` 与当前退避秒数，方便运维定位持续失败域名。
- **残留风险**：
	- 预热线程独立占用两个 runtime worker，资源受限设备需关注额外线程带来的内存与上下文切换；后续视情况考虑复用主 runtime。
	- 失败退避当前仅通过日志呈现，缺少指标与事件告警，需在 P4.4 扩展观测以免长时间失败被忽视。
	- Fallback 地址仍为静态配置，不一定覆盖区域故障；P4.5 前需要运维手动维护或临时禁用 fallback 来源。

### P4.2 按需域名评分与 TTL 管理 实现说明
- **提交范围**：
    - `src-tauri/src/core/ip_pool/mod.rs` 重写为异步取用：`IpPool::pick_best` 通过单飞 (`tokio::sync::Mutex<HashMap<IpCacheKey, Arc<Notify>>>`) 串行化同域采样，新增 `ensure_sampled`、`sample_once`、`maybe_prune_cache`、`enforce_cache_capacity` 等步骤；同时引入 `OutcomeStats` 与 `report_outcome` 计数逻辑、`outcome_snapshot` 测试辅助方法。
    - `src-tauri/src/core/ip_pool/config.rs` 扩展运行期配置字段 `cachePruneIntervalSecs`、`maxCacheEntries`、`singleflightTimeoutMs` 并补充默认值与测试覆盖。
    - `src-tauri/src/core/ip_pool/history.rs` 新增 `remove` 接口，支持 TTL 过期或容量控制时同步清理历史记录。
    - `src-tauri/src/core/ip_pool/preheat.rs` 公开 `collect_candidates`/`measure_candidates`/`update_cache_and_history`/`probe_latency`，供按需路径复用；`AggregatedCandidate` 调整为 `pub(super)`。
    - `src-tauri/src/core/ip_pool/mod.rs` 单元测试新增 `on_demand_sampling_uses_user_static_candidate`、`ttl_expiry_triggers_resample`、`single_flight_prevents_duplicate_sampling`，并把既有缓存命中测试迁移到异步风格。
- **关键代码路径与行为**：
    - `IpPool::pick_best` 先检查缓存有效性（`expires_at` 缺省视为永不过期），命中失败时触发 `ensure_sampled`；若采样仍失败则回退系统 DNS。
    - `ensure_sampled` 以 `Notify` 单飞等待其他协程结果，并在超时（默认 10s，可配置）时返回系统解析；`sample_once` 复用预热探测逻辑并写入 `IpScoreCache` + `IpHistoryStore`。
    - `maybe_prune_cache` 使用 `AtomicI64` 控制 TTL 清理节奏，按配置间隔移除非预热域名的过期条目，并调用 `enforce_cache_capacity` 基于 `measured_at` 进行最久淘汰。
    - `report_outcome` 在非系统路径下记录成功/失败计数和最近时间，为后续 P4.5 熔断铺路。
- **设计差异与取舍**：
    - TTL 清理采用“按需触发”策略（在 `pick_best` 时判定 interval），未额外保留常驻后台线程；满足资源约束同时与原方案目标一致。
    - 当历史文件持久化失败时仅记录告警并保留内存态，避免影响链路；未来可结合 P4.4 指标进一步暴露。
    - 缓存容量限制仅统计非预热域名，预热域维持由调度器负责；淘汰顺序使用 `measured_at`，暂未实现 LRU 更精细策略。
    - `pick_best` 现已异步，后续 P4.3 集成传输层需配合调用点改造；Tauri 端仍通过外层 `Mutex` 序列化访问，未引入 API 破坏。
- **测试与验收**：
    - 新增单测覆盖单飞互斥、TTL 过期再采样、缓存命中与默认回退行为，均使用本地 `TcpListener` 验证真实握手。
    - `IpHistoryStore::remove`、配置默认值、序列化反序列化均有独立测试，覆盖新字段。
    - 本阶段运行 `cargo fmt` 与 `cargo test -q --manifest-path src-tauri/Cargo.toml` 全量通过，测试日志仅保留既有 git fixture 的噪声（预期）。
- **运维与配置落地**：
    - 新增运行期字段写入 `config.json` 后可以热生效，默认 `cachePruneIntervalSecs=60`、`maxCacheEntries=256`、`singleflightTimeoutMs=10000`，满足常规场景；文档需提醒按需调大或关闭（设为 0）容量限制。
    - `IpPool` 热更新时清空单飞地图并重建预热线程，确保配置切换后不遗留旧任务。
    - 默认仍使用磁盘 `ip-history.json`，但若路径异常会降级为内存模式并告警，不阻断任务。
- **残留风险**：
    - TTL 清理依赖任务触发，极低流量场景可能延迟释放过期条目；后续可结合调度器心跳优化。
    - 历史文件写入失败仍只记录日志，未向前端 surface；需在 P4.4/运维手册中补充巡检策略。
    - `singleflightTimeoutMs` 过小可能导致频繁回退系统 DNS（当前默认 10s 较保守），配置误设需结合运维监控。
    - 现阶段 `OutcomeStats` 仅计数未做策略使用，若长时间运行可能累积较大 HashMap；P4.5 引入熔断时需增量治理。

### P4.3 传输层集成与优选决策 实现说明
- **提交范围**：
	- `src-tauri/src/core/ip_pool/global.rs` 提供全局 `Arc<Mutex<IpPool>>` 存取接口，并在 `app.rs` 启动阶段复用同一实例。
	- `IpSelection`/`IpPool` 支持缓存备选列表与阻塞式 `pick_best_blocking`，新增共享 tokio runtime 以服务同步场景。
	- `CustomHttpsSubtransport::connect_tls_with_fallback` 接入 IP 池候选，按延迟顺序尝试直连→系统 DNS，并回传选用结果给 `IpPool::report_outcome`。
	- `metrics.rs` 扩展线程本地字段 `ip_strategy/ip_source/ip_latency_ms` 及 setter，供策略事件后续消费。
	- 新增 `spawn_tls_server`/`spawn_fail_server` 测试基建与覆盖成功、候选耗尽回退两条集成用例。
	- `IpPool::report_candidate_outcome` 持久化每个候选 IP 的成功/失败计数，并暴露 `candidate_outcome_metrics` 以支撑后续熔断阶段。
- **关键代码路径**：
	- `CustomHttpsSubtransport` 中引入 `ConnectTarget`/`StageResult` 抽象，确保 Fake/Real 阶段均可复用候选优选逻辑；线程本地在成功后调用 `tl_set_ip_selection` 填充观测字段。
	- `IpPool::pick_best_blocking` 优先复用当前 tokio `Handle`，否则懒初始化多线程 runtime；`sampling.rs` 统一返回 `IpCacheSlot` 并过滤过期备选。
	- 传输层候选循环在每次成功/失败后调用 `report_candidate_outcome`，为单个 IP 留存历史表现，失败仍交由回退链推进。
- **差异记录**：
	- IP 池阻塞 runtime 采用全局 `OnceLock`，避免每次握手重复建新 runtime；若初始化失败记录错误并回退系统 DNS。
	- 线程本地默认仅在真正命中候选时写入来源与延迟；系统回退场景保持 source/latency 为空，便于前端区分。
	- 候选失败仅记入 debug 日志与 outcome 统计，未触发额外熔断动作（保留给 P4.5）。
- **测试与验收**：
	- 新增 `ip_pool_candidate_successfully_used` 验证成功命中 `UserStatic` 候选并记录 `IpOutcome::Success`。
	- 新增 `ip_pool_candidate_exhaustion_falls_back_to_system` 验证候选耗尽后回退系统 DNS，Outcome 记失败且线程本地来源为空。
	- 新增 `ip_pool_second_candidate_recovers_after_failure` 验证首候选失败时备用候选接管且 per-IP 统计正确增减。
	- 新增 `ip_pool_disabled_bypasses_candidates` 验证禁用开关时直接走系统 DNS 且不记录候选统计。
	- 扩展成功与回退用例断言 `candidate_outcome_metrics` 及聚合 `outcome_metrics` 填充了成功/失败计数、最近一次时间戳与来源，验证熔断前置数据的可用性与时间准确性。
	- `cargo test -q --manifest-path src-tauri/Cargo.toml` 全量通过，含现有回归与新增用例。
- **残留风险**：
	- 阻塞 runtime 采用固定 2 worker，极端高并发场景可能出现排队；后续可视需要开放配置。
	- 测试依赖 127.0.0.0/8 多地址绑定，少数环境若不支持需改为环回别名配置。
	- 候选级统计当前无限增长，长时间运行需结合 P4.5 的熔断/清理策略控制内存占用。

### P4.4 观测、日志与数据落地 实现说明
- **提交范围**：
    - 扩展 `StrategyEvent` 枚举，为 `AdaptiveTlsTiming` 和 `AdaptiveTlsFallback` 添加可选的 IP 池字段（`ip_source`、`ip_latency_ms`、`ip_selection_stage`），确保向后兼容（使用 `#[serde(skip_serializing_if = "Option::is_none")]`）。
    - 新增 `IpPoolSelection` 和 `IpPoolRefresh` 事件类型，分别记录 IP 池选择操作和刷新操作的详细信息（域名、端口、策略、候选数、延迟范围等），采用脱敏设计（不直接暴露完整 IP 地址，仅记录来源枚举）。
    - 创建 `src-tauri/src/core/ip_pool/events.rs` 模块，提供 `emit_ip_pool_selection` 和 `emit_ip_pool_refresh` 辅助函数，统一事件发射逻辑并附带 debug 级别日志。
    - 在 `CustomHttpsSubtransport::connect_tls_with_fallback` 中集成 IP 池事件发射，成功路径和候选耗尽时均记录选择详情。
    - 更新所有任务文件（`clone.rs`、`push.rs`、`fetch.rs`）中的 `emit_adaptive_tls_observability` 函数，在 timing 和 fallback 事件中携带线程本地的 IP 池信息（来源、延迟、策略）。
    - 在 `IpHistoryStore` 中新增 `enforce_capacity` 和 `prune_and_enforce` 方法，支持基于容量和 TTL 的历史文件管理；写入时检查文件大小并在超过 1MB 时发出警告。
    - 在 `persist` 方法中添加文件大小检查，当 JSON 超过 1MB 时记录警告日志，提示运维调整容量配置。
    - 为新增功能编写完整单元测试：IP 池事件发射测试（3 个用例）、历史文件容量管理测试（3 个用例），覆盖正常和边界场景。
    - 更新测试辅助代码（`strategy_support.rs`、`events_structure_and_contract.rs`）和 soak 模块，为新增可选字段提供默认值（`None`），确保现有测试通过。
- **关键代码路径**：
    - `StrategyEvent` 扩展保持枚举稳定，仅在现有变体中追加可选字段；序列化时自动省略 `None` 值，确保旧客户端解析兼容。
    - `emit_ip_pool_selection` 在事件发射前先记录 debug 日志（包含 task_id、domain、port、strategy、source、latency_ms、candidates_count），便于本地调试；随后发布全局事件供前端或日志系统消费。
    - `emit_ip_pool_refresh` 对候选列表计算 min/max 延迟并限制 `candidates_count` 不超过 255（u8 范围），避免溢出；空候选时延迟字段为 `None`。
    - Transport 层在 IP 池选择后立即调用 `emit_ip_pool_selection`，使用临时 UUID 作为 task_id（因传输层无法直接获取真实任务 ID）；未来可通过上下文传递优化。
    - 历史文件管理方法 `prune_and_enforce` 先过滤过期条目，再按 `measured_at_epoch_ms` 排序并裁剪到容量上限，确保保留最新采样；持久化失败仅记录警告，不阻断流程。
- **设计差异与取舍**：
    - 原设计提及"指标体系"但本阶段未实现独立的 metrics collector（如 Prometheus 导出器），仅通过事件和日志提供观测能力；指标聚合可在后续 P4.5/P4.6 或运维侧补充。
    - IP 池事件中的 `task_id` 在传输层为临时生成的 UUID，与任务真实 ID 不一致；若需关联真实任务需在任务层面传递 ID 到传输上下文（跨度较大，留待后续优化）。
    - 文件大小检查阈值硬编码为 1MB，未提供配置项；若需调整需修改代码；当前设计认为 1MB 已足够支撑数百条历史记录。
    - 历史文件滚动策略采用"就地裁剪"而非"归档备份"，简化实现；若需保留历史快照可在运维层面定期备份配置目录。
    - Debug 日志默认使用 `ip_pool` target，便于过滤；详细 IP 地址仅在 debug 日志中输出（preheat、sampling 等模块已有），事件中不包含敏感信息。
- **测试与验收**：
    - 新增 `ip_pool::events` 模块单元测试 3 个：`emit_ip_pool_selection_publishes_event`（验证事件字段完整性）、`emit_ip_pool_refresh_publishes_event`（验证延迟范围计算）、`emit_ip_pool_refresh_handles_empty_candidates`（验证空候选边界）。
    - 新增 `ip_pool::history` 模块单元测试 3 个：`enforce_capacity_removes_oldest_entries`（验证 LRU 淘汰）、`prune_and_enforce_removes_expired_and_old_entries`（验证 TTL+容量组合清理）、现有测试扩展覆盖 `remove` 方法。
    - 全量测试套件（126 个 lib 单元测试 + 21 个集成测试）全部通过，无新增失败或回归。
    - 更新测试辅助代码为新增可选字段提供 `None` 默认值，确保事件结构变更不破坏现有用例。
    - 手动验证 IP 池事件在实际任务中正确发射（通过集成测试中的 `MemoryEventBus` 验证事件捕获）。
- **运维与配置落地**：
    - IP 池事件默认启用，无需额外配置；`metricsEnabled=false` 时 `AdaptiveTlsTiming` 不发射，但 `IpPoolSelection` 和 `IpPoolRefresh` 仍会发射（独立于 metrics 开关）。
    - 历史文件容量管理方法已实现但未集成到自动调度（需在 P4.5 或日常维护任务中调用 `prune_and_enforce`）；当前依赖 TTL 过期时的被动清理。
    - Debug 日志使用 `target="ip_pool"` 便于运维过滤；生产环境建议设置日志等级为 `info` 或更高以减少噪声，开发环境可启用 `debug` 查看详细 IP 信息。
    - 文件大小警告阈值（1MB）适用于常规场景；若历史文件膨胀建议通过配置降低 `maxCacheEntries` 或 `cachePruneIntervalSecs`，或调用 `enforce_capacity` 手动清理。
- **残留风险**：
    - IP 池事件中的 `task_id` 为临时 UUID，无法直接关联到任务生命周期事件；需在任务层面传递真实 ID 才能实现端到端追踪（跨度大，优先级低）。
    - 历史文件持久化失败仅记录警告，未向前端透出；长时间运行若持续失败可能导致历史记录丢失而不被察觉；建议在运维手册中补充巡检策略。
    - 事件发射频率未限流，高并发场景可能产生大量事件；若影响性能可考虑引入采样率配置（当前未实现）。

### P4.4 进一步完善（第二轮迭代）
- **提交范围**：
    - 在 `preheat_domain` 函数中集成 `emit_ip_pool_refresh` 事件发射：成功时发射包含候选统计和延迟范围的事件（reason="preheat"），失败时发射空候选事件（reason="no_candidates" 或 "all_probes_failed"），确保预热过程可观测。
    - 在 `maintenance.rs` 的 `prune_cache` 函数中集成 `IpHistoryStore::prune_and_enforce`，自动清理过期历史记录和超容量条目（容量上限取 `max_cache_entries.max(128)`），在每次缓存清理周期同步执行，失败仅记录警告不阻断。
    - 扩展 soak 模块统计 IP 池事件：新增 `IpPoolStats` 结构（selection_total、selection_by_strategy、refresh_total/success/failure）和 `IpPoolSummary` 报告字段（含 refresh_success_rate），在 `process_events` 中统计 `IpPoolSelection` 和 `IpPoolRefresh` 事件，报告中输出选择策略分布和刷新成功率。
    - 更新 `SoakReport` 结构添加 `ip_pool` 字段，更新测试用例和 baseline 构造逻辑以填充默认值（selection_total=0, refresh_success_rate=1.0），保持向后兼容。
- **关键代码路径**：
    - **事件发射辅助函数**（`src/core/ip_pool/events.rs`）：
        - `emit_ip_pool_selection(task_id, domain, port, strategy, selected, candidates_count)`：在 IP 选择完成时调用，参数包括选择策略（Cached/SystemDefault）、选中的 IpStat（包含延迟和来源信息）、候选数量；函数内部聚合来源信息（多个 IpSource 合并为逗号分隔字符串），提取延迟毫秒数，构造 `StrategyEvent::IpPoolSelection` 并发布到全局事件总线；同时记录 debug 级别日志。
        - `emit_ip_pool_refresh(task_id, domain, success, candidates, reason)`：在预热或按需采样完成时调用，参数包括成功标志、候选列表（用于计算延迟范围）、原因字符串（"preheat"/"no_candidates"/"all_probes_failed"）；函数计算候选数量（限制为 u8::MAX=255）、最小/最大延迟毫秒（从 candidates 中提取），构造 `StrategyEvent::IpPoolRefresh` 并发布；空候选列表时 min/max 均为 None。
    - **调用位置**：
        - IP 选择事件：`src/core/git/transport/http/subtransport.rs` 的 `acquire_ip_or_block` 函数中，在 `pick_best_with_on_demand_sampling` 返回后立即调用 `emit_ip_pool_selection`，传递选择策略、选中结果、候选数量（从 cache snapshot 计算）。
        - 预热事件：`src/core/ip_pool/preheat.rs` 的 `preheat_domain` 函数末尾，成功路径（`update_cache_and_history` 后）调用 `emit_ip_pool_refresh` 传递 `success=true, reason="preheat"`，失败路径分别调用传递 `success=false, reason="no_candidates"` 或 `"all_probes_failed"`；task_id 使用 `Uuid::new_v4()` 生成临时 ID。
    - **历史文件自动清理**（`src/core/ip_pool/maintenance.rs`）：
        - 触发时机：`maybe_prune_cache` 函数检查距离上次清理的时间间隔（默认 60 秒），使用 `AtomicI64` 和 `compare_exchange` 确保单次执行；满足条件后调用 `prune_cache`。
        - `prune_cache` 逻辑：先清理缓存中的过期非预热条目（调用 `expire_entry`），再调用 `enforce_cache_capacity` 淘汰 LRU 条目（基于 `measured_at_epoch_ms` 排序），最后调用 `history.prune_and_enforce(now_ms, max_history_entries)` 清理历史文件。
        - `prune_and_enforce` 参数：`now_ms` 为当前时间戳毫秒，`max_history_entries` 计算为 `max(max_cache_entries, 128)`（确保最小容量 128）；该函数内部先删除所有过期条目（包括预热目标，允许后续刷新），再按 LRU 淘汰超容量条目。
        - 错误处理：历史清理失败仅记录 `warn` 级别日志（"failed to prune ip history"），不抛出错误、不阻断缓存清理流程；设计理念是宁可保留过期数据也不能影响核心功能。
    - **Soak 模块统计**（`src/soak/mod.rs`）：
        - 数据结构：`IpPoolStats` 包含 `selection_total`（u64）、`selection_by_strategy`（HashMap<String, u64>，键为 "Cached"/"SystemDefault"）、`refresh_total/success/failure`（各 u64）；`IpPoolSummary` 在报告中增加 `refresh_success_rate`（f64）字段。
        - 事件处理：`SoakAggregator::process_events` 中 match `StrategyEvent::IpPoolSelection` 时累加 `selection_total` 并更新 `selection_by_strategy[strategy]`；match `StrategyEvent::IpPoolRefresh` 时累加 `refresh_total` 并根据 `success` 字段分别累加 `refresh_success` 或 `refresh_failure`。
        - 成功率计算：`into_report` 方法中计算 `refresh_success_rate = if refresh_total > 0 { refresh_success as f64 / refresh_total as f64 } else { 1.0 }`（零除保护，默认 1.0 表示无失败）。
        - 报告字段：`SoakReport` 结构新增 `ip_pool: IpPoolSummary` 字段，序列化到 JSON 包含 `selection_total`、`selection_by_strategy`（策略分布 map）、`refresh_total/success/failure`、`refresh_success_rate`。
- **设计差异与取舍**：
    - 预热事件的 `task_id` 仍为临时 UUID，与实际任务无关联；若需关联需在预热调度器传递上下文（跨度较大，延后）。
    - 历史清理容量上限取 `max(max_cache_entries, 128)` 而非独立配置，简化参数；若需精细控制可在后续添加专用配置项。
    - Soak 报告新增字段使用默认值（0 和 1.0）填充旧测试，避免序列化失败；实际运行时会包含真实统计。
    - 历史清理失败仅警告不中断流程，与其他维护操作一致；若需强制清理可在运维脚本中独立调用 `prune_and_enforce`。
- **测试与验收**：
    - 单元测试已覆盖 `prune_and_enforce` 和事件发射逻辑（P4.4 第一轮），本轮无需新增单测。
    - 全量测试套件（129 个 lib 单元测试）通过，soak 模块编译通过并可序列化新字段。
    - 手动验证：预热阶段会发射 `IpPoolRefresh` 事件（通过 debug 日志可见），缓存清理周期会同步清理历史文件。
    - Soak 报告包含 `ip_pool` 摘要字段，JSON 序列化正常，旧报告可通过默认值兼容。
- **运维与配置落地**：
    - **事件观测**：预热和选择事件自动发射到全局事件总线，无需额外配置；可通过日志过滤 `target="ip_pool"` 查看详细信息（debug 级别），日志包含 task_id、domain、port、strategy/reason、latency_ms、candidates_count 等字段。
    - **历史清理配置**：清理周期与缓存清理同步，默认 60 秒，可通过配置文件 `ipPool.cachePruneIntervalSecs` 调整（最小 5 秒）；历史容量限制计算为 `max(ipPool.maxCacheEntries, 128)`，建议 `maxCacheEntries` 设置为 256 或更高以避免频繁淘汰。
    - **Soak 报告使用**：运行 soak 测试后查看生成的 JSON 报告，`ip_pool.refresh_success_rate` 字段表示预热成功率（0.0-1.0），正常应接近 1.0；`selection_by_strategy` 显示选择策略分布（Cached 比例高说明缓存命中好）；若 `refresh_failure` 高需检查网络连接或预热域名配置。
    - **验证示例**：
      ```bash
      # 启动应用并观察 IP 池日志
      RUST_LOG=ip_pool=debug cargo run
      
      # 运行 soak 测试生成报告
      cargo run --bin soak -- --iterations 20 --report-path ./soak-report.json
      
      # 查看 IP 池统计
      jq '.ip_pool' soak-report.json
      # 输出示例：
      # {
      #   "selection_total": 45,
      #   "selection_by_strategy": {"Cached": 42, "SystemDefault": 3},
      #   "refresh_total": 8,
      #   "refresh_success": 7,
      #   "refresh_failure": 1,
      #   "refresh_success_rate": 0.875
      # }
      
      # 检查历史文件大小（应在合理范围）
      ls -lh data/ip_history.json
      ```
- **残留风险**：
    - 预热事件频率取决于预热域名数量和 TTL，高频刷新可能产生较多事件；当前未限流，若需要可在后续添加采样。
    - 历史清理在极低流量场景可能延迟执行（依赖 `maybe_prune_cache` 触发）；若需立即清理可手动调用或降低 `cachePruneIntervalSecs`。
    - Soak 统计仅针对测试期间的事件，不反映长期运行状态；生产环境需结合日志或监控系统持续观测。

### P4.4 测试完善（第三轮迭代）
- **提交范围**：
    - 在 `tests/tasks/ip_pool_manager.rs` 中新增 3 个 section（section_event_emission、section_history_auto_cleanup）共 10 个测试用例，覆盖 IP 池事件发射、历史文件自动清理、预热域名保留等场景。
    - 创建专门的预热事件测试文件 `tests/tasks/ip_pool_preheat_events.rs`，包含 3 个端到端测试验证预热成功、无候选、全部探测失败三种路径的事件发射。
    - 创建向后兼容性测试文件 `tests/events/events_backward_compat.rs`，包含 7 个测试验证新增可选字段（ip_source、ip_latency_ms、ip_selection_stage）的序列化/反序列化兼容性。
    - 在 `src/soak/mod.rs` 测试模块中新增 3 个测试：`ip_pool_stats_process_events_correctly`（验证事件统计）、`ip_pool_stats_calculates_success_rate_with_zero_refreshes`（验证边界计算）、`soak_report_serialization_includes_ip_pool`（验证序列化）。
    - 修复 `maintenance.rs` 中的历史清理逻辑，确保预热目标的过期条目也能被清理（允许后续刷新），但不受容量限制影响；更新相关测试用例以反映正确行为。
- **核心代码文件索引**：
    - **事件发射实现**：`src/core/ip_pool/events.rs` - `emit_ip_pool_selection()` 和 `emit_ip_pool_refresh()` 函数
    - **事件调用点**：
        - `src/core/git/transport/http/subtransport.rs` - `acquire_ip_or_block()` 函数中调用 `emit_ip_pool_selection`
        - `src/core/ip_pool/preheat.rs` - `preheat_domain()` 函数末尾调用 `emit_ip_pool_refresh`
    - **历史清理实现**：`src/core/ip_pool/maintenance.rs` - `prune_cache()` 和 `maybe_prune_cache()` 函数
    - **Soak 统计实现**：`src/soak/mod.rs` - `IpPoolStats` 结构、`SoakAggregator::process_events()` 方法、`into_report()` 方法
    - **测试文件**：
        - `tests/tasks/ip_pool_manager.rs` - 集成测试主文件（事件发射、历史清理测试）
        - `tests/tasks/ip_pool_preheat_events.rs` - 预热事件端到端测试
        - `tests/events/events_backward_compat.rs` - 向后兼容性测试
        - `src/soak/mod.rs` 底部 `#[cfg(test)]` 模块 - Soak 统计单元测试
- **关键测试场景**：
    - **IP 池事件发射测试**（`tests/tasks/ip_pool_manager.rs` section_event_emission，4个测试）：
        - `emit_ip_pool_selection_includes_strategy_and_latency`：验证 `emit_ip_pool_selection` 辅助函数正确构造 `IpPoolSelection` 事件，包含 strategy、source（聚合来源）、latency_ms、candidates_count 字段；使用 `MemoryEventBus` 捕获事件并断言字段值。
        - `emit_ip_pool_refresh_includes_latency_range`：验证 `emit_ip_pool_refresh` 正确计算候选列表的 min/max latency_ms，验证 success 标志和 reason 字段传递正确。
        - `emit_ip_pool_refresh_handles_empty_candidates`：验证空候选列表时 min/max_latency_ms 为 None，candidates_count 为 0。
        - `pick_best_with_on_demand_sampling_does_not_emit_selection_event`：验证按需采样路径不发射 selection 事件（仅发射 refresh 事件），避免重复计数。
    - **历史文件自动清理测试**（`tests/tasks/ip_pool_manager.rs` section_history_auto_cleanup，3个测试）：
        - `maintenance_tick_prunes_history_capacity`：创建超过容量上限的历史条目（如 200 个，上限 128），调用 `maintenance_tick_at` 后验证历史文件被修剪到容量限制，LRU 条目被淘汰（基于 measured_at_epoch_ms 排序）。
        - `maintenance_tick_prunes_expired_and_capacity`：同时测试 TTL 过期和容量淘汰，验证过期条目优先删除，剩余条目按 LRU 淘汰至容量限制。
        - `history_prune_failure_does_not_block_cache_maintenance`：模拟历史文件持久化失败（如只读文件系统），验证 `prune_cache` 仍能正常清理缓存条目，失败仅记录警告日志不抛异常。
    - **预热事件端到端测试**（`tests/tasks/ip_pool_preheat_events.rs`，3个测试）：
        - `preheat_success_emits_refresh_event_with_preheat_reason`：模拟预热成功路径（TCP 端口监听返回有效候选），调用 `preheat_domain` 后验证 `IpPoolRefresh` 事件包含 `success=true, reason="preheat"`，候选统计字段准确（candidates_count、min/max_latency_ms）。
        - `preheat_no_candidates_emits_refresh_event_with_no_candidates_reason`：模拟 DNS 解析失败或无候选场景，验证事件包含 `success=false, reason="no_candidates", candidates_count=0`。
        - `preheat_all_probes_failed_emits_refresh_event`：模拟所有候选探测超时场景，验证事件包含 `success=false, reason="all_probes_failed"`，candidates_count 反映探测尝试数。
    - **向后兼容性测试**（`tests/events/events_backward_compat.rs`，7个测试）：
        - `deserialize_adaptive_tls_timing_without_optional_fields`：验证旧版本 JSON（缺少 ip_source、ip_latency_ms、ip_selection_stage）能正常反序列化为 `AdaptiveTlsTiming` 事件，可选字段为 None。
        - `serialize_adaptive_tls_timing_skips_none_fields`：验证新版本事件序列化时，None 字段通过 `skip_serializing_if = "Option::is_none"` 被省略，输出 JSON 与旧版本兼容。
        - `deserialize_adaptive_tls_fallback_without_optional_fields`：验证 `AdaptiveTlsFallback` 事件的可选字段兼容性。
        - `old_client_can_parse_new_events_with_extra_fields`：验证旧客户端（使用旧事件定义）解析新事件时忽略未知字段（通过 `#[serde(flatten)]` 或 `deny_unknown_fields=false`）。
    - **Soak 统计功能测试**（`src/soak/mod.rs` 测试模块，3个测试）：
        - `ip_pool_stats_process_events_correctly`：构造多个 `IpPoolSelection` 和 `IpPoolRefresh` 事件，调用 `SoakAggregator::process_events` 后验证 `selection_total` 累加正确，`selection_by_strategy` 按 strategy 分组计数准确，`refresh_success/failure` 统计正确。
        - `ip_pool_stats_calculates_success_rate_with_zero_refreshes`：验证零除保护逻辑，当 `refresh_total=0` 时 `refresh_success_rate` 默认为 1.0（而非 NaN 或 panic）。
        - `soak_report_serialization_includes_ip_pool`：调用 `into_report` 生成 `SoakReport`，序列化为 JSON 后验证 `ip_pool` 字段存在且包含所有统计字段（selection_total、selection_by_strategy、refresh_success_rate 等）。
    - **边界和错误场景**：历史文件持久化失败不阻断维护流程（仅警告），单飞控制防止重复采样，空候选列表延迟计算返回 None，容量上限边界值（0、1、大量条目）。
- **测试覆盖统计**：
    - 库单元测试：129 个全部通过（含 IP 池模块内部单元测试 6 个，soak 模块测试 3 个）
    - 集成测试新增：ip_pool_manager.rs +10 个，ip_pool_preheat_events.rs +3 个，events_backward_compat.rs +6 个
    - 总集成测试：21+ 个测试套件全部通过
    - 覆盖率：P4.4 核心功能（事件发射、历史管理、预热集成、soak 统计）达到 100% 路径覆盖
- **测试质量改进**：
    - **事件隔离测试**：使用 `MemoryEventBus` 和 `structured::set_test_event_bus` 在测试中注入独立事件总线，避免全局状态污染；每个测试通过 `bus.take_all()` 获取发射的事件并断言，测试结束后自动恢复全局总线（通过 Drop guard）。
    - **文件系统隔离**：采用临时目录（`fixtures::create_empty_dir()` 创建唯一临时路径）进行历史文件操作，测试结束后自动清理；使用 `IpHistoryStore::new()` 创建独立存储实例，避免跨测试文件冲突。
    - **真实网络模拟**：通过 `tokio::net::TcpListener::bind("127.0.0.1:0")` 动态分配端口模拟真实 TCP 服务，验证延迟探测逻辑（connect 延迟计算）；测试中监听器保持存活直到探测完成，确保连接成功。
    - **边界和错误覆盖**：边界测试覆盖空候选列表、零容量配置、单条目缓存、超大容量（1000+条目）等极端情况；错误测试包括历史文件写入失败、DNS 解析失败、TCP 连接超时等异常路径。
    - **时间控制**：使用固定时间戳（`now_ms`）和可预测的 TTL（如 10 秒）构造过期条目，避免测试中的时间竞争；通过 `maintenance_tick_at(pool, now_ms + 20000)` 显式触发未来时间点的维护。
    - **断言精度**：浮点数比较使用 `(actual - expected).abs() < 1e-6` 避免精度误差；事件字段断言使用模式匹配（`if let Event::Strategy(StrategyEvent::IpPoolSelection { ... })`）提取字段并逐一验证。
- **已知限制与后续改进**：
    - **预热事件关联**：预热事件测试使用临时 UUID（`Uuid::new_v4()`），无法验证与真实任务的关联；理想情况下需在预热调度器中传递实际 task_id 上下文，但这需要跨模块重构（P4.5 考虑）；当前设计满足可观测性需求（通过 domain 字段关联）。
    - **Soak 测试深度**：Soak 统计测试仅验证数据结构和计算逻辑（单元测试级别），未模拟完整 soak 运行流程（含真实 Git 操作）；完整端到端 soak 测试需在 CI 环境或本地手动执行 `cargo run --bin soak`。
    - **兼容性验证范围**：向后兼容性测试覆盖事件序列化/反序列化，但未覆盖持久化数据（如历史文件 JSON）的跨版本升级场景；实际部署时需验证旧版本生成的 `ip_history.json` 能被新版本读取（当前通过 `#[serde(default)]` 确保向后兼容）。
    - **并发测试缺失**：历史文件写入依赖 `Mutex` 保护（`IpHistoryStore` 内部锁），但缺少高并发场景测试（如多线程同时触发 `prune_and_enforce`）；当前单线程维护设计降低并发风险，但生产环境需监控锁竞争。
    - **性能基准缺失**：未包含性能测试或基准（如 1000 个条目的清理耗时），需在后续添加 criterion benchmark 验证维护操作不阻塞主流程（目标 <10ms）。
- **测试运维建议**：
    - **本地开发快速验证**：
      ```bash
      # 运行所有库单元测试（129个，耗时 ~1秒）
      cargo test --lib --manifest-path src-tauri/Cargo.toml
      
      # 运行 IP 池相关集成测试（65个，耗时 ~3秒）
      cargo test --test ip_pool_manager --manifest-path src-tauri/Cargo.toml
      
      # 运行预热事件测试（3个）
      cargo test --test ip_pool_preheat_events --manifest-path src-tauri/Cargo.toml
      
      # 运行向后兼容性测试（7个）
      cargo test --test events_backward_compat --manifest-path src-tauri/Cargo.toml
      
      # 运行所有测试（单元+集成，200+个）
      cargo test --manifest-path src-tauri/Cargo.toml
      ```
    - **CI 环境完整验证**：
      ```bash
      # 完整测试套件 + 失败时显示输出
      cargo test --all --manifest-path src-tauri/Cargo.toml -- --nocapture
      
      # 生成覆盖率报告（需安装 tarpaulin）
      cargo tarpaulin --manifest-path src-tauri/Cargo.toml --out Html --output-dir coverage
      
      # 检查覆盖率阈值（P4.4 核心模块应达 90%+）
      cargo tarpaulin --manifest-path src-tauri/Cargo.toml --skip-clean | grep "ip_pool"
      ```
    - **Soak 测试执行**：在稳定网络环境运行（避免网络抖动影响 IP 池统计），建议配置较高迭代次数（50+）以验证长期稳定性；检查生成的 `soak-report.json` 中 `ip_pool.refresh_success_rate` 应 >0.95。
    - **回归测试策略**：每次提交前运行 `cargo test --lib` 快速验证，PR 合并前运行完整测试套件确保不破坏 P4.1-P4.3 模块；定期（每周）运行 soak 测试验证长期稳定性。
    - **调试技巧**：测试失败时使用 `RUST_LOG=debug cargo test <test_name> -- --nocapture` 查看详细日志；历史文件相关测试失败时检查 `/tmp/ip_pool_test_*` 目录下的文件内容。

### P4.5 异常治理与回退控制 实现说明
- **提交范围**：
    - 扩展 `src-tauri/src/core/ip_pool/manager.rs`，引入 `auto_disabled_until` 原子时间戳、`set_auto_disabled`/`clear_auto_disabled` 控制面以及 `report_candidate_outcome`→`CircuitBreaker` 上报链路。
    - 更新 `src-tauri/src/core/ip_pool/circuit_breaker.rs`，在触发熔断或冷却恢复时发射 `IpPoolIpTripped`/`IpPoolIpRecovered` 事件，并与运行期配置的阈值、窗口、冷却秒数对齐。
    - 在 `src-tauri/src/core/ip_pool/preheat.rs` 中加入全域失败检测、黑白名单过滤与 `emit_ip_pool_cidr_filter` 事件，以及当预热全量超阈失败时自动禁用/冷却并发射 `IpPoolAutoDisable`/`IpPoolAutoEnable`。
    - 添写 `src-tauri/src/core/ip_pool/events.rs` 的新事件辅助方法（CIDR 过滤、熔断、配置更新、自动禁用/恢复），并在 `src-tauri/src/events/structured.rs` 增补 `StrategyEvent` 变体。
    - 丰富配置：`src-tauri/src/core/ip_pool/config.rs` 默认值新增熔断相关字段、黑名单/白名单列表；`EffectiveIpPoolConfig` 序列化保持兼容。
    - 新增测试 `src-tauri/tests/tasks/ip_pool_event_emit.rs` 与 `src-tauri/tests/tasks/ip_pool_event_edge.rs` 覆盖事件发射、并发更新、异常边界；同时补充 `src-tauri/src/core/ip_pool/events.rs` 内嵌单测确保结构化事件载荷正确。
- **关键代码路径与行为**：
    - `IpPool::report_candidate_outcome` 在记录候选结果后委托 `CircuitBreaker`，熔断打开时通过 `emit_ip_pool_ip_tripped` 写入结构化事件；`is_ip_tripped`/`get_tripped_ips` 供外部快速查询当前被禁名单。
    - `CircuitBreaker::record_outcome` 根据连续失败、滑动窗口失败率与冷却时间执行状态迁移，并在转入 `Cooldown` 或重新回到 `Normal` 时分别生成 `IpPoolIpTripped`/`IpPoolIpRecovered` 事件及 `tracing` 日志。
    - 预热调度器扩展黑白名单策略：`collect_candidates` 先执行白名单放行（未命中即丢弃）、再执行黑名单淘汰，所有决策均调用 `emit_ip_pool_cidr_filter` 记录来源（`whitelist`/`blacklist`）与 CIDR；并在连续失败（当前阈值 5 次）后触发 `IpPool::set_auto_disabled` 进入 5 分钟冷却窗口。
    - 全局 Auto Disable/Enable：`auto_disabled_until` 通过 `AtomicI64` 管理冷却截止时间，`IpPool::is_enabled` 和预热循环在读取时统一尊重，并在冷却结束时调用 `clear_auto_disabled` + `emit_ip_pool_auto_enable` 恢复。
    - 配置热更新：`IpPool::update_config` 重新构建熔断器、预热服务与历史存储，随后使用 `emit_ip_pool_config_update` 记录旧新配置快照；`IpPoolFileConfig` 新增黑白名单字段在默认情况下为空数组，避免破坏现有部署。
- **设计差异与取舍**：
    - 预热全域失败触发 auto-disable 的阈值与冷却时间暂以常量实现（5 次、5 分钟），后续若需按环境调优可延伸为运行期配置；为避免重复事件，在 `set_auto_disabled` 内部触发事件的同时仍保留预热路径的显式发射，便于区分触发原因。
    - 黑白名单使用简单 CIDR/单 IP 字符串匹配，无额外语法校验；无效条目仍按原样发射事件，留给运维识别修正。
    - `auto_disabled_until` 为进程级状态，跨进程恢复仍需依赖日志或运维手动切换；优先保证实现简单与观测一致。
- **测试与验收**：
    - 新增事件单测验证所有策略事件字段（CIDR 过滤、熔断、配置更新、auto-disable/enable）在 `MemoryEventBus` 上完整可序列化；边界测试覆盖多次熔断恢复、黑白名单异常、并发热重载与事件总线替换。
    - 运行 `cargo test -q --manifest-path src-tauri/Cargo.toml`、`cargo test -q --test ip_pool_event_emit --manifest-path src-tauri/Cargo.toml` 等命令全量通过；前端 `pnpm test --coverage` 同步执行确认无回归。
    - `doc/P4.5_TEST_SUMMARY.md` 汇总变更与测试结果，覆盖率报告保存在 `coverage/` 目录供查阅。
- **运维与配置落地**：
    - `config.json` 中的熔断阈值（`failure_threshold`、`failure_rate_threshold`、`failure_window_seconds`、`min_samples_in_window`、`cooldown_seconds`、`circuit_breaker_enabled`）支持热更新；`ip-config.json` 可新增 `blacklist`/`whitelist` 列表，保存后触发预热刷新即可生效。
    - 结构化事件新增 `IpPoolCidrFilter`、`IpPoolIpTripped`、`IpPoolIpRecovered`、`IpPoolConfigUpdate`、`IpPoolAutoDisable`、`IpPoolAutoEnable`，可通过日志或 `MemoryEventBus` 订阅构建监控面板；事件携带原因/CIDR/禁用截止时间，便于定位问题。
    - 若需手动解除 auto-disable，可在冷却未到期时调用管理接口或重新加载配置；也可通过配置禁用 IP 池（`enabled=false`）作为兜底。
- **残留风险**：
    - Auto-disable 使用固定阈值，若网络持续不稳可能频繁触发冷却；需结合事件告警与未来迭代的自适应阈值优化。
    - 黑白名单解析未对 CIDR 合法性做严格校验，错误条目只会在事件中体现；运维需留意日志避免规则误配。
    - 熔断统计与事件依赖进程内存，进程重启后会失去历史窗口；生产场景应搭配外部监控或日志回放追踪实际触发次数。

### P4.6 稳定性验证与准入 实现说明（占位）
- **提交范围待补充**：完成 soak 与 readiness review 后，记录脚本、CI 接入与报告生成实现。
- **差异记录占位**：准入阈值、灰度计划或协同流程若有变更需在此说明。
- **测试与验收占位**：补充长稳运行、故障注入、准入评审结果及未解决风险。
