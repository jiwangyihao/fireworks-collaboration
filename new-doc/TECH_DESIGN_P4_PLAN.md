# P4 阶段技术设计文档 —— IP 优选与握手延迟调度

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

### P4.4 观测、日志与数据落地 实现说明（占位）
- **提交范围待补充**：记录指标与事件的最终字段、`ip-history.json` 滚动策略与日志格式。
- **差异记录占位**：如观测粒度、采样率或脱敏策略调整，与原设计差异需在此注明。
- **测试与验收占位**：列出指标导出、日志滚动、历史文件恢复等测试结果。

### P4.5 异常治理与回退控制 实现说明（占位）
- **提交范围待补充**：整理熔断状态机、黑白名单与全局回退实现细节。
- **差异记录占位**：若阈值、冷却策略或事件代号调整，请同步原因与影响。
- **测试与验收占位**：补充失败注入、熔断恢复、手动黑名单生效等验证结果。

### P4.6 稳定性验证与准入 实现说明（占位）
- **提交范围待补充**：完成 soak 与 readiness review 后，记录脚本、CI 接入与报告生成实现。
- **差异记录占位**：准入阈值、灰度计划或协同流程若有变更需在此说明。
- **测试与验收占位**：补充长稳运行、故障注入、准入评审结果及未解决风险。
