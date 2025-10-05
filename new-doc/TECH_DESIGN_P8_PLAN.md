# P8 技术设计计划（可观测性面板与指标汇聚）

> 适用读者：后端开发、前端开发、运维/质量、性能与安全审计
> 关联阶段：P0–P7 已交付能力（Git 基线、Push、自适应 TLS、IP 池、代理、凭证、安全、LFS 等）
> 当前状态：草案（Draft）——本文档将细化 P8 阶段的实施路线

---
## 目录
1. 概述
2. 详细路线图
   - P8.1 基础指标与埋点标准化
   - P8.2 指标聚合与存储层
   - P8.3 指标导出与采集接口
   - P8.4 可观测性前端面板 UI
   - P8.5 阈值告警与 Soak 深度集成
   - P8.6 性能与安全硬化
   - P8.7 灰度与推广策略
3. 实现说明（按子阶段展开）

---
## 1. 概述

### 1.1 目标摘要
P8 阶段的核心目标是：将前序阶段（自适应 TLS、IP 池、Git 任务、代理、凭证、安全策略、LFS 等）产生的“事件型瞬时数据”系统化转化为结构化、可查询、低开销的度量指标（Metrics），并提供一套内建的本地可观测性面板，使开发与运维可以：
1. 快速洞察 Git 任务（clone/fetch/push）、网络策略（Fake/Real SNI 回退链）、IP 池刷新与选择、熔断与自动禁用、TLS 延迟、代理降级等关键路径表现。
2. 通过统一指标命名/标签规范，实现跨阶段（P3 TLS、P4 IP 池、P5 代理、P6 安全、P7 LFS）的可横向对比与趋势分析。
3. 支持 Prometheus 拉取或本地 JSON API 读取，便于集成外部监控体系，同时内置轻量前端面板（无需额外部署 Grafana）。
4. 在不显著增加 CPU/内存开销的前提下提供分钟级聚合统计 + 近期窗口原始采样，配合阈值/告警对 Soak 与灰度阶段做实时质量判定。

### 1.2 范围（In Scope）
- 指标分类与命名规范（Counter / Gauge / Histogram / Summary 拆分策略）
- 埋点标准化：Git 任务、传输层、IP 池、TLS、代理、熔断、自动禁用、重试、Soak 统计桥接
- 聚合与存储：内存多窗口（近 5m 原始、近 1h/24h 聚合）+ 可选持久化（关进程丢失可接受）
- 指标导出：Prometheus `/metrics`（可开关）+ 本地 JSON `/metrics/snapshot`（分页 / 分类过滤）
- 前端可观测性面板：延迟火焰/阶段瀑布、任务成功率、回退链占比、IP 池刷新成功率、熔断与 auto-disable 计数、TLS 时延分布、代理切换、重试次数分布
- 阈值与告警：本地规则引擎（静态阈值 + 比例 + 滑动窗口），触发后转化为事件并出现在面板 / Soak 报告扩展区
- 安全与合规：指标脱敏（不含 Token / 真实仓库私有名称）、采集速率限制、防止 DoS
- 灰度策略：P8 功能整体可通过配置分阶段启用（仅埋点→导出→面板公开→告警）

### 1.3 非目标（Out of Scope）
- 不引入分布式集中式时序数据库（如 Prometheus Server / VictoriaMetrics）——使用方可自行拉取
- 不做跨进程或集群聚合（单节点、本地工具场景）
- 不提供复杂查询语言（PromQL 之外）
- 不做全量日志搜索与 Trace（分布式追踪留待远期）
- 不实现历史长期持久化（>7 天）

### 1.4 价值与收益
| 维度 | 收益 | 说明 |
|------|------|------|
| 故障定位 | 提升 | 快速看到“失败集中在 Fake→Real 回退”或“IP 池刷新成功率骤降” |
| 性能优化 | 提升 | Histogram 展示 TLS 握手 P95/P99、Git 对象下载速率变化 |
| 风险收敛 | 提升 | 阈值告警在 Soak 前暴露回归（如自动禁用次数 > 0） |
| 观测统一 | 降复杂 | 事件→指标桥接统一入口，避免重复埋点 |
| 成本可控 | 稳定 | 单节点内存环形缓冲，不强依赖外部组件 |

### 1.5 成功判定标准
- 指标覆盖：设计清单 ≥ 95% 已列关键指标（见后续 P8.1）全部实现并经测试校验标签完整性
- 开销控制：在典型使用（并发 5 个 Git 任务 + IP 池预热）下 CPU 额外占用 < 3%，内存常驻 < 8 MB（不含面板渲染）
- 面板体验：交互延迟 < 300 ms（切换时间范围/分类），前端渲染首屏时间 < 1 s
- 数据一致性：Prometheus `/metrics` 输出与面板展示核心指标（成功率、延迟分位数）偏差 < 1%
- 告警有效性：通过故障注入脚本（模拟高失败率/高握手延迟）可在 1 分钟内触发告警事件
- Soak 集成：Soak 报告新增“实时指标对比”段落且与阈值结果一致

### 1.3 非目标（Out of Scope）
- 不引入分布式集中式时序数据库（如 Prometheus Server / VictoriaMetrics）——使用方可自行拉取

| 故障定位 | 提升 | 快速看到“失败集中在 Fake→Real 回退”或“IP 池刷新成功率骤降” |
| 性能优化 | 提升 | Histogram 展示 TLS 握手 P95/P99、Git 对象下载速率变化 |
| 风险收敛 | 提升 | 阈值告警在 Soak 前暴露回归（如自动禁用次数 > 0） |
| 观测统一 | 降复杂 | 事件→指标桥接统一入口，避免重复埋点 |
| 成本可控 | 稳定 | 单节点内存环形缓冲，不强依赖外部组件 |

### 1.5 成功判定标准
- 指标覆盖：设计清单 ≥ 95% 已列关键指标（见后续 P8.1）全部实现并经测试校验标签完整性
- 开销控制：在典型使用（并发 5 个 Git 任务 + IP 池预热）下 CPU 额外消耗 < 3%，内存常驻 < 8 MB（不含面板渲染）
- 面板体验：交互延迟 < 300 ms（切换时间范围/分类），前端渲染首屏时间 < 1 s
- 数据一致性：Prometheus `/metrics` 输出与面板展示核心指标（成功率、延迟分位数）偏差 < 1%
- 告警有效性：通过故障注入脚本（模拟高失败率/高握手延迟）可在 1 分钟内触发告警事件
- Soak 集成：Soak 报告新增“实时指标对比”段落且与阈值结果一致

### 1.6 里程碑摘要
| 子阶段 | 核心交付 | 判定信号 |
|--------|----------|----------|
| P8.1 | 指标规范+埋点改造 | 单测校验指标注册表、命名不冲突 |
| P8.2 | 聚合内存层 | 可查询窗口数据；内存占用指标自检 |
| P8.3 | 导出接口 | curl /metrics 返回无格式错误；抓取稳定 |
| P8.4 | UI 面板 | 面板路由可访问，多视图联动 |
| P8.5 | 阈值与告警 + Soak 扩展 | 人为注入触发告警显示在面板/报告 |
| P8.6 | 优化与安全 | 压测数据满足开销约束；速率限制生效 |
| P8.7 | 灰度与推广 | 默认启用基础埋点，面板按配置放量 |

### 1.7 与前序阶段关系
- 复用 P4 IP 池事件（Selection/Refresh/AutoDisable）、P3 TLS Timing/Fallback、任务事件（task://state|progress|error）、Soak 报告数据结构
- 不改变现有事件 schema；通过“事件→指标转换适配层”实现向后兼容
- 计划在 `core/metrics` 新增轻量注册与采集模块，不侵入现有业务逻辑（通过订阅事件总线）

### 1.8 高层风险与缓解
| 风险 | 描述 | 影响 | 缓解 |
|------|------|------|------|
| 指标泛滥 | 埋点粒度过细 | 内存膨胀/抓取慢 | 白名单注册 + 审核清单 |
| 锁竞争 | 高并发更新同一 Histogram | 请求延迟上升 | 分片桶/无锁环形写缓冲 |
| UI 卡顿 | 大量数据点直接渲染 | 前端掉帧 | 后端降采样+前端虚拟化 |
| 泄漏敏感 | 标签含用户 repo/private host | 合规风险 | 统一脱敏函数（hash 截断）|
| 指标与事件偏差 | 双通路统计不一致 | 误判 | 周期对账校验任务 |
| 违规抓取滥用 | 未授权访问 `/metrics` | 信息暴露 | 访问控制+速率限制 |

### 1.9 术语
- 原始样本（Sample）：事件到达后即时转换的一条记录
- 聚合桶（Bucket）：时间窗口（如 1m）对某类指标累积统计
- 分位估计（Quantile Estimation）：基于 HDR/CKMS 算法近似 P95/P99 的过程
- 视图（Panel View）：前端某一功能图表组合（任务稳定性 / 传输性能 / IP 池 / TLS / 代理 / 告警）

### 1.10 退出准则 / 回退策略
- 任何子阶段可通过配置 `observability.enabled=false` 一键关闭（保留事件）
- 出现性能回退（CPU > 5%）或内存泄漏时：降级为仅 P8.1 指标（禁用聚合/面板）
- 如果 Prometheus 导出被外部安全审计拒绝，可退回仅本地 JSON Snapshot

### 1.11 配置键速览（摘要）
| 键 | 类型/默认 | 说明 |
|----|-----------|------|
| observability.enabled | bool=true | 全局总开关（false 时全部禁用，仅事件保留） |
| observability.basicEnabled | bool=true | 仅基础指标（P8.1）开关；随 enabled=false 强制关闭 |
| observability.aggregateEnabled | bool=true | 聚合窗口/分位（P8.2）开关 |
| observability.exportEnabled | bool=true | 导出接口（P8.3）开关 |
| observability.uiEnabled | bool=true | 前端面板（P8.4）入口开关 |
| observability.alertsEnabled | bool=true | 告警与阈值引擎（P8.5）开关 |
| observability.performance.batchFlushIntervalMs | u32=500 | 批量 flush 周期（P8.6） |
| observability.performance.tlsSampleRate | u32=5 | TLS 采样率（1/N） |
| observability.performance.maxMemoryBytes | u64=8_000_000 | 内存水位阈值（超限降级） |
| observability.performance.enableSharding | bool=true | Histogram 分片开关 |
| observability.performance.redact.repoHashSalt | string="" | 仓库名哈希盐（空=随机生成不持久） |
| observability.performance.redact.ipMode | enum=mask | ip 脱敏模式：mask|classify|full |
| observability.export.authToken | string? | 导出接口可选访问令牌（为空不验证） |
| observability.export.rateLimitQps | u32=5 | `/metrics` QPS 软限制 |
| observability.export.maxSeriesPerSnapshot | u32=1000 | JSON snapshot 最大序列数 |
| observability.alerts.rulesPath | string="config/observability/alert-rules.json" | 规则文件路径 |
| observability.alerts.evalIntervalSecs | u32=30 | 规则评估间隔 |
| observability.alerts.minRepeatIntervalSecs | u32=30 | 相同告警重复触发最小间隔（去抖） |
| observability.layer | enum=basic | 灰度层级（P8.7） |
| observability.autoDowngrade | bool=true | 是否根据资源自动降级 |
| observability.internalConsistencyCheckIntervalSecs | u32=300 | 指标 vs 事件对账周期 |

> 注：若存在层级冲突（如 layer=basic 但 exportEnabled=true），初始化时优先层级推导并记录警告，自动将高层开关重写为 false。


---
## 2. 详细路线图（子阶段占位）
### P8.1 基础指标与埋点标准化
聚焦“指标清单 + 命名规范 + 事件→指标转换层”；不引入聚合存储，仅保证注册/更新无冲突。产出：规范文档、注册表、单元测试。

#### 目标
1. 定义首批核心指标（≥ 95% 关键路径覆盖）。
2. 建立统一命名与标签约束，防止指标爆炸。
3. 实现事件到指标的适配层（订阅 MemoryEventBus）。
4. 提供编译期/启动期冲突检测（同名不同类型报错）。
5. 保证新增指标对现有运行路径开销可忽略（单次更新 < 200ns 目标）。

#### 范围
- 指标类型：Counter / Gauge / Histogram（Summary 暂不启用）。
- 覆盖模块：Git任务、传输层、IP 池、TLS、代理、熔断、自动禁用、重试、Soak 桥接摘要。
- 标签控制：严格白名单（最多 5 个标签/指标）。

#### 非目标
- 不做时间窗口聚合（由 P8.2 实现）。
- 不做分位（无 HDR/CKMS）。
- 不暴露导出接口（P8.3）。

#### 指标清单（首版草案）
| 名称 | 类型 | 标签 | 描述 |
|------|------|------|------|
| git_tasks_total | counter | kind (clone/fetch/push/http_fake), state (completed/failed/canceled) | 任务结束计数 |
| git_task_duration_ms | histogram | kind | 任务总时长分布 |
| git_retry_total | counter | kind, category | 重试次数 |
| tls_handshake_ms | histogram | sni_strategy (fake/real), outcome (ok/fail) | TLS 握手耗时 |
| ip_pool_selection_total | counter | strategy(Cached/SystemDefault), outcome(success/fail) | IP 选择尝试 |
| ip_pool_refresh_total | counter | reason(preheat|on_demand|no_candidates|all_probes_failed), success(bool) | 刷新事件 |
| ip_pool_latency_ms | histogram | source (builtin/history/user_static/dns/fallback) | 采样 RTT 分布 |
| ip_pool_auto_disable_total | counter | reason | 自动禁用次数 |
| circuit_breaker_trip_total | counter | reason(failure_rate) | 熔断触发次数 |
| circuit_breaker_recover_total | counter | - | 熔断恢复次数 |
| proxy_fallback_total | counter | mode(http|socks5) | 代理降级次数（P5 预埋） |
| http_strategy_fallback_total | counter | stage(connect|tls|http), from(fake|real) | 回退链次数 |
| soak_threshold_violation_total | counter | name | Soak 阈值未达成计数 |
| alerts_fired_total | counter | severity(info|warn|critical) | 告警触发（预留） |

#### 命名与标签规范
- 统一使用 snake_case；单位后缀：_ms / _bytes / _total。
- Histogram 使用固定桶策略（ms：1,5,10,25,50,75,100,150,200,300,500,750,1000,1500,2000,3000,5000）。
- 标签值需枚举常量，禁止自由字符串（使用小写、无空格）。
- 每条指标总标签 cardinality 预估 < 30 * 组合（内控 < 500 总系列）。

#### 数据模型接口（Rust 草案）
```rust
pub enum MetricKind { Counter, Gauge, Histogram }
pub struct MetricDesc { name: &'static str, kind: MetricKind, help: &'static str, labels: &'static [&'static str] }
pub struct Registry { /* lock-free slabs + atomic ptr */ }
pub trait Recorder { fn incr(&self, name:&'static str, labels:&[(&'static str,&str)], v:u64); fn observe(&self, name:&'static str, labels:&[(&'static str,&str)], v:f64); }
```

#### 事件到指标转换（适配层）
- 监听 `StrategyEvent::*`、任务事件 `task://state|progress|error`、IP 池事件、Soak 报告中间态。
- 使用匹配表：EventType → 更新动作（闭包）。
- 防止重复：任务完成仅计一次（completed|failed|canceled）。

#### 落地步骤
1. 定稿指标清单（评审） → 生成 `metrics_spec.md`（附机读 JSON）。
2. 实现最小注册表 + API（无导出）。
3. 编写事件适配层，接入总线。
4. 为关键事件添加单测（模拟事件发射，断言内部累积值）。
5. 压测：每秒 2k 事件更新下 CPU 增量 < 2%。
6. 配置开关：`observability.basicEnabled`（默认 true，可禁用）。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| 注册重复 | 启动报错/拒绝第二次注册 |
| 任务三态 | clone 正常 / 失败 / 取消计数各自增加 |
| 回退链 | Fake→Real 触发 http_strategy_fallback_total +=1 |
| IP 刷新失败 | no_candidates/all_probes_failed 标签写入 |
| 熔断往返 | trip + recover 指标各 +1 |
| 高并发 | 2k/s 事件下无死锁，计数单调 |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| 指标覆盖不足 | 审批清单，预留扩展命名空间 *_ext |
| 原子更新开销偏高 | 批量合并（线程本地缓冲 flush）推迟到 P8.6 |
| 标签拼写错误 | 编译期常量 + 统一构造宏 |

#### 回退策略
- 关闭 `observability.basicEnabled` → 停止注册和事件订阅，其他阶段不受影响。

#### 事件→指标映射（补充）
| 事件源 | 条件筛选 | 指标更新 | 标签构造 | 去重策略 |
|--------|----------|----------|----------|----------|
| task://state | state in (completed,failed,canceled) | git_tasks_total +1 | kind=task.kind, state=state | 任务 ID 已处理集合 |
| task://error | category=Retryable | git_retry_total +1 | kind=task.kind, category=error.category | 同一次重试循环每次计数 |
| StrategyEvent::IpPoolSelection | always | ip_pool_selection_total +1 | strategy, outcome=success/fail | 无去重 |
| StrategyEvent::IpPoolRefresh | always | ip_pool_refresh_total +1 | reason, success | 无去重 |
| StrategyEvent::IpPoolAutoDisable | disable 触发 | ip_pool_auto_disable_total +1 | reason | 同一原因重复扩展不计（比较 until_ms）|
| StrategyEvent::IpPoolIpTripped | always | circuit_breaker_trip_total +1 | reason | IP+window 去重 1 次/窗口 |
| StrategyEvent::IpPoolIpRecovered | always | circuit_breaker_recover_total +1 | - | 无 |
| StrategyEvent::IpPoolRefresh (latency) | success & has latency list | ip_pool_latency_ms observe | source=primary 来源（优先 builtin→dns→history→user_static→fallback）| 每 IP 一次 |
| AdaptiveTlsTiming | success or fail | tls_handshake_ms observe | sni_strategy=fake/real,outcome=ok/fail | 无 |
| AdaptiveTlsFallback | from=fake → real | http_strategy_fallback_total +1 | stage, from=fake | 每连接一次 |
| StrategyEvent::MetricAlert | firing | alerts_fired_total +1 | severity | firing→active 不重复计 |
| SoakReport (汇总) | threshold violation | soak_threshold_violation_total +1 | name | 每报告一次 |

一致性校验任务（internalConsistencyCheckIntervalSecs）：
1. 周期读取最近窗口任务事件计数与指标 git_tasks_total（按标签聚合）对比，允许偏差 <1%。
2. 若偏差超限，发出 `StrategyEvent::MetricDrift { metric, expected, actual }` 并记录日志。
3. 支持通过配置禁用（设 interval=0）。
4. 同时比对：ip_pool_refresh_total 与实际 refresh 事件数、tls_handshake_ms 样本计数 vs AdaptiveTlsTiming 事件数；允许计数差异容忍度分别为 2% / 5%。
5. 生成自检指标：`metric_consistency_last_run_ms`、`metric_consistency_drift_total{metric}`。
6. 连续 3 次 drift 同一 metric 触发一次告警（severity=warn, rule_id=auto_metric_drift）。


### P8.2 指标聚合与存储层
实现内存多窗口聚合（1m/5m/1h/24h）与可选近 5 分钟原始样本缓存；支持分位近似（HDR/CKMS）。

#### 目标
1. 为 Histogram/Counter 提供多时间窗口快速读取能力。
2. 为延迟类指标提供 P50/P90/P95/P99 近似分位查询。
3. 控制内存：默认总占用 < 5 MB（窗口 + 原始样本）。
4. 数据一致性：窗口滚动时无双重计数或丢失（单调累积）。

#### 范围
- 聚合窗口：1m（最近 60 个桶）/5m（最近 60 个聚合）/1h（60 个 1m merge）/24h（24 个 1h merge）。
- 分位估计算法：首选 CKMS（适合在线更新，误差 ε=0.01），若实现复杂暂退 HDR（固定桶）。
- 原始样本缓存：仅对明确标注 `sampled=true` 的延迟指标开启（如 tls_handshake_ms, ip_pool_latency_ms, git_task_duration_ms）。

#### 非目标
- 不做跨进程一致性。
- 不持久化（崩溃丢失可接受）。
- 不支持任意自定义窗口（固定预设）。

#### 数据结构草案
```rust
struct WindowedCounter { current: AtomicU64, slots: ArrayVec<[u64; 60]>, last_rotate: Instant }
struct QuantileStream { ckms: CKMS<f64>, last_flush: Instant }
struct HistogramWindow { buckets: [AtomicU64; N_BUCKETS] }
struct MultiWindow<T> { one_min: T, five_min: T, one_hour: T, one_day: T }
```

滚动策略：
- 每 1s 检查 `last_rotate`，跨分钟边界刷新 minute slot；
- 5m/1h/1d 通过合并低级窗口快速生成（lazy 计算并缓存有效期）。

#### 接口（读取）
```
GET (internal) metrics/window?name=<metric>&range=1m|5m|1h|24h&stat=counter|histogram&quantiles=p50,p95
返回：{ name, range, type, points:[{t, value|buckets|q:{p50,..}}] }
```

#### 落地步骤
1. 设计内存布局与大小评估（每指标上限估算）。
2. 实现 WindowedCounter + HistogramWindow + CKMS 封装。
3. 集成 P8.1 注册表：新增 `enable_window(name, cfg)`。
4. 定时器/后台任务：tick 负责窗口滚动 & CKMS 压缩。
5. 单测：人为注入序列验证窗口值与滚动边界。
6. 压测：10k/s 更新持续 2 分钟，观察 CPU 增量 < 5%。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| 分钟滚动 | 新旧 slot 切换后新数据不污染旧分钟 |
| 空窗口读取 | 返回空 points 数组，不 panic |
| 分位精度 | 生成已知分布（正态/指数），误差 < 5% |
| 高频更新 | 无死锁，CKMS 大小受控 |
| 原始样本开关 | 关闭 sampled 时不存储 raw buffer |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| 分位实现复杂 | 先落 HDR，再迭代 CKMS |
| 旋转抖动 | 使用单线程调度器 + 时间对齐 | 
| 内存超出预估 | 运行期统计 metrics_mem_bytes，超限自动丢弃最久窗口 |

#### 回退策略
- 配置 `observability.aggregateEnabled=false` 时禁用滚动与 CKMS，仅保留 P8.1 原始累积。

#### 分位算法选择理由（CKMS vs HDR）
| 维度 | CKMS | HDR Histogram |
|------|------|--------------|
| 精度控制 | 相对误差 ε 可配置 | 依赖桶布局（固定） |
| 内存占用 | 与分布相关，通常低 | 与范围与分辨率线性相关 |
| 动态范围 | 不需预设 max | 需要预设最大可追踪值 |
| 更新复杂度 | O(log n) 近似 | O(1) bucket++ |
| 实现复杂度 | 较高（压缩逻辑） | 较低 |
| 长尾适应 | 自动插值 | 需更细桶导致膨胀 |

策略：默认 CKMS；当 ckms 节点数 >10k 且连续两轮超限或触发内存预警 → 降级 HDR（线性 0-5s + 指数 5-30s）。降级与回升各产生一次事件 `MetricQuantileDowngrade` / `MetricQuantileUpgrade`，回升需 10 分钟稳定窗口。


### P8.3 指标导出与采集接口
提供 `/metrics`（Prometheus 文本格式）与 `/metrics/snapshot`（JSON 分类/过滤/分页）；访问控制与速率限制雏形。

#### 目标
1. 支持 Prometheus 拉取标准文本；名称与标签遵循 P8.1 规范。
2. 提供本地 JSON API，方便前端直接消费（避免自己解析文本）。
3. 加入最小安全措施：可选 token 校验 + 速率限制（令牌桶）。
4. 性能：导出时序在 50ms 内完成（典型 300 条时间序列）。

#### 范围
- `/metrics` GET：聚合 Counter/Gauge/Histogram（当前累积 + 1m/5m 分位附加注释）。
- `/metrics/snapshot`：参数 `?names=git_tasks_total,tls_handshake_ms&range=1h&quantiles=p95,p99`。
- 授权：配置 `observability.export.authToken`（缺省不需要）。
- 速率限制：默认 5 QPS（滑动窗口），超过返回 429 并打点。

#### 非目标
- 不提供写接口。
- 不做 Prometheus remote_write。
- 不做多租户隔离。

#### API 设计
`GET /metrics` → text/plain; version 注释：`# fireworkscollab_metrics 1`。
`GET /metrics/snapshot` → application/json 示例：
```json
{
   "generated_at_ms": 1730700000000,
   "series": [
      {"name":"git_tasks_total","type":"counter","value":1234},
      {"name":"tls_handshake_ms","type":"histogram","buckets":[{"le":5,"c":10},{"le":10,"c":25}],"sum":12345,"count":300,
         "quantiles":{"p50":12,"p95":78,"p99":120}}
   ]
}
```

#### 配置
```
observability: {
   exportEnabled: true,
   export: {
      authToken: "<optional>",
      rateLimitQps: 5,
      maxSeriesPerSnapshot: 1000
   }
}
```

#### 落地步骤
1. 序列化层：实现 Prometheus 文本编码（含 HELP/TYPE 行）。
2. 量化选择：Histogram 输出 buckets + sum + count；分位通过 `_approx` 注释或在 JSON 中。
3. Snapshot 控制：按名称过滤 + 分类（前缀/模糊不支持，只支持精确或枚举）。
4. 安全：Token 验证中间件 + 速率限制（简单原子计数+时间桶或 `leaky bucket`）。
5. 集成测试：模拟并发 20 个并行请求，验证速率限制。
6. 兼容：exportDisabled 时访问接口返回 404。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| 基本导出 | 文本包含 HELP/TYPE 且序列完整 |
| 空指标 | 返回空/最小格式，不 panic |
| JSON 过滤 | names 仅返回指定集合 |
| Token 错误 | 返回 401 |
| 速率限制 | 超阈值后返回 429 且计数指标增加 |
| 性能 | 300 序列编码 < 50ms |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| 编码性能不足 | 预分配字符串缓冲；复用 `String` 容器 |
| 速率限制误伤 | 暂时放宽 QPS，记录日志分析 |
| Token 泄漏 | 支持热更 token；日志脱敏（hash 前 6 位） |

#### 回退策略
- `observability.exportEnabled=false`：关闭所有导出接口；面板转为轮询本地缓存（仅使用内部读取 API）。

#### 安全与审计补充
- 访问日志：记录 timestamp, remote_addr(hash), path, status, duration_ms, series_count。
- 审计指标：`metrics_export_requests_total{status}`、`metrics_export_series_total`、`metrics_export_rate_limited_total`。
- 授权失败策略：返回 401，不泄漏是否存在特定指标名称。
- 速率限制实现：令牌桶（容量 = rateLimitQps*2，填充间隔=1s）。
- 热更：修改 authToken 后旧 token 2 分钟内仍可访问（迁移窗口），期间发出 `ExportAuthTokenRolled` 事件。

#### 接受标准补充
| 项 | 目标 |
|----|------|
| 导出 CPU 额外占用 | <= 1% 在 300 序列场景 |
| 内存暂峰 | < 512KB（编码缓存） |
| 未授权访问 | 100% 返回 401/429/404 合理状态码 |
| 大名单过滤（names 1000 条） | 处理 < 80ms |
| 速率限制准确度 | 误差 < 10%（统计 1 分钟） |


### P8.4 可观测性前端面板 UI
新增“Observability” 视图，分 Tab：Overview / Git / Network / IP Pool / TLS / Proxy / Alerts；图表组件与数据拉取协议。

#### 目标
1. 提供统一导航入口与七类视图，支持快速切换时间范围（5m / 1h / 24h）。
2. 图表组件复用（折线/柱状/分布/饼/表格）并支持空数据占位。
3. 数据获取抽象：统一 metrics service；缓存 10 秒内重复请求。
4. 交互：点击回退链失败点 → 展示对应最近 10 条任务/事件（侧边抽屉）。

#### 范围
- 前端：Vue 组件集（`ObservabilityView.vue` + 子组件目录）。
- 拉取协议：优先使用 JSON Snapshot API；export 关闭时 fallback 内部命令（tauri invoke）。
- 缓存层：内存 Map + 过期时间戳；失效后后台刷新，UI 先展示旧值（乐观）。
- 可视化：
   - Overview：任务成功率、平均任务时长、TLS P95、IP 刷新成功率、告警计数
   - Git：clone/fetch/push 时长分布、重试次数、任务数堆叠
   - Network：回退链次数、Fake vs Real 占比、HTTP 错误分类
   - IP Pool：刷新成功率趋势、延迟分位趋势、auto-disable 事件 sparkline
   - TLS：握手耗时分布（直方图+分位）、SNI 策略占比
   - Proxy：代理降级次数、模式切换时间线（占位 P5）
   - Alerts：当前触发/历史清单（可按 severity 过滤）

#### 非目标
- 不实现自由查询编辑（无自定义 PromQL）。
- 不内置长时间序列（>24h）。
- 不做国际化（保持中文/英文简洁标签即可）。

#### 组件结构
```
components/observability/
   OverviewPanel.vue
   GitPanel.vue
   NetworkPanel.vue
   IpPoolPanel.vue
   TlsPanel.vue
   ProxyPanel.vue
   AlertsPanel.vue
   MetricChart.vue (通用)
   TimeRangeSelector.vue
   LoadingState.vue
   EmptyState.vue
```

#### 接口抽象（前端）
```ts
interface MetricSeries { name: string; points: Array<{ t: number; v: number }>; }
interface HistogramSeries { name: string; buckets: Array<{ le: number; c: number }>; sum: number; count: number; quantiles?: Record<string, number>; }
function fetchMetrics(params:{ names:string[]; range:string; quantiles?:string[] }): Promise<{ series: (MetricSeries|HistogramSeries)[] }>
```

#### 落地步骤
1. 设计 UI 草图 & 信息架构评审。
2. 实现通用 MetricChart（ECharts 或轻量 Canvas）+ TimeRangeSelector。
3. 接入后端 Snapshot API（轮询或用户触发刷新，初版 15s）。
4. 增量实现各 Panel：数据映射 → 组件组装 → 验证空与异常状态。
5. 添加前端单测（Pinia store：缓存、过期、错误处理）。
6. 性能预估：首屏并发请求 <= 3；其余懒加载（切换时请求）。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| exportEnabled=false | 回退内部 invoke，图表仍显示数据 |
| 空数据 | 展示 EmptyState，无异常 | 
| 时间范围切换 | 请求参数正确，缓存按范围隔离 |
| 高频切换 | 不产生请求风暴（去抖 300ms）|
| 数据缺失字段 | UI 容忍并标记 N/A |
| 错误注入 (500) | 展示重试按钮 + 最近一次成功快照 |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| 图表库体积大 | 优先 ECharts 按需导入或轻量封装 | 
| 多图渲染卡顿 | 虚拟滚动 + 延迟渲染 + requestAnimationFrame 分帧 |
| 数据闪烁 | 使用 diff 更新而非全量重建 |

#### 回退策略
- 关闭 `observability.uiEnabled`：隐藏入口菜单；保留导出接口供外部工具使用。

#### 性能 KPI 与缓存策略补充
- 首屏渲染（Overview + 2 子图）JS 执行 + 绘制 < 800ms（开发模式可放宽 30%）。
- 单视图最大折线点数：降采样后 ≤ 600；超出时后台自动抽样（保留极值 + 均匀间隔）。
- 内存：单图表实例峰值 < 3MB；超限触发一次 `UiChartDownsample` 日志。
- 缓存层：
   - key=(names,range,quantiles) → value=series + fetched_at。
   - TTL: range=5m→10s, 1h→30s, 24h→120s。
   - 过期后：先返回旧数据（stale-while-refresh），后台并发刷新；刷新失败保留旧缓存并展示轻提示。
- 失败快速重试抑制：同 key 连续失败 3 次后进入冷却 60s。
- 可用性指示：在面板右上角显示最近一次数据时间戳与新旧标记（Fresh/Stale）。


### P8.5 阈值告警与 Soak 深度集成
本地阈值规则（静态 + 比例 + 滑动窗口），告警事件化；Soak 报告新增 metrics 章节对比阈值/基线。

#### 目标
1. 支持静态数值阈值（例如：tls_handshake_ms_p95 > 800ms）。
2. 支持比例阈值（git_tasks_total{state="failed"}/git_tasks_total > 0.05）。
3. 支持滑动窗口（最近 5m IP 刷新成功率 < 0.85）。
4. 告警生命周期：firing → active（持续）→ resolved；去抖（最小抖动 30s）。
5. 与 Soak：将关键指标判定结果嵌入 Soak Report（ip_pool / tls / git / retry 部分）。

#### 范围
- 规则来源：配置文件 `observability/alert-rules.json` + 内置默认规则（可禁用）。
- 规则表达式 DSL（最小化）：
   - MetricRef：`metric.name[p95]?{label=val}?`
   - 运算：目前仅支持二元比较（>,>=,<,<=）与比值表达：`fail/total > 0.05`。
   - 窗口：`window:5m` 标记使用聚合窗口数据。
- 告警事件：`StrategyEvent::MetricAlert { name, severity, state, rule_id, value, threshold }`。

#### 非目标
- 不实现完整 PromQL 解析。
- 不支持复合布尔逻辑（AND/OR）（可通过拆分规则实现）。
- 不做静默 / 抑制树（后续视需求扩展）。

#### 规则文件示例
```json
[
   {"id":"git_fail_rate","expr":"git_tasks_total{state=failed}/git_tasks_total > 0.05","severity":"warn","window":"5m"},
   {"id":"tls_p95_high","expr":"tls_handshake_ms[p95] > 800","severity":"warn","window":"5m"},
   {"id":"ip_refresh_success_low","expr":"ip_pool_refresh_total{success=true}/ip_pool_refresh_total < 0.85","severity":"critical","window":"5m"}
]
```

#### 数据获取流程
1. 定时调度器每 30s 执行一次评估。
2. 解析表达式 → 拉取窗口聚合（P8.2 接口）→ 计算数值。
3. 与阈值比较；状态机更新（新触发/持续/恢复）。
4. 触发/恢复事件写入 Alerts 面板 & 指标 `alerts_fired_total`。

#### Soak 集成
- Soak 结束时读取最近窗口告警列表，将违反项录入 `report.metrics.alerts`。
- 若存在 critical 告警且 severity=critical 未恢复，标记 Soak not ready。

#### 落地步骤
1. DSL 解析器（简单 tokenizer + AST）。
2. 规则加载与热更新（文件 Watch，失败保留旧规则）。
3. 评估引擎 + 状态机（HashMap<rule_id, AlertState>）。
4. 告警事件发射与转换为指标更新。
5. Soak 报告扩展（新增字段）。
6. 单元与集成测试（规则评估 + 热更新 + 去抖）。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| 静态阈值越界 | firing 事件 + state=active |
| 比值计算 | 分母为 0 → 视为跳过（记录告警日志），不触发 |
| 去抖 | 在抖动周期内多次触发仅一次事件 |
| 恢复 | 值恢复到阈值内 → resolved 事件 |
| 热更新 | 新增规则即时生效，删除后不再评估 |
| Soak 集成 | 报告包含 alerts 数组与 ready 判定一致 |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| 规则表达式歧义 | 严格 JSON schema + 预编译验证 |
| 评估高开销 | 限制规则数（默认≤50），批量抓取指标缓存 |
| 过度告警 | 去抖 + severity 分级 + 默认保守阈值 |

#### 回退策略
- `observability.alertsEnabled=false`：跳过评估与事件发射；面板显示“已禁用”。

#### 告警去抖与合并策略（补充）
- 去抖窗口：同一 rule_id 在 `minRepeatIntervalSecs` 内重复触发保持 state=active，不再发 firing 事件。
- 合并：多条规则指向同一根因（例如 git_fail_rate 与 git_retry_spike）可在配置中声明 `groupKey`；UI Alerts 面板按 group 折叠显示最新严重度。
- 抑制：当存在 `critical` 级别告警 active 时，同组 `warn` 级别 firing 事件仅记录日志不发事件（避免噪声）。
- 自动恢复判定：连续 `coolDownWindows`（默认 2 个评估周期）均未触发阈值才发 resolved，避免单次偶然回落。


### P8.6 性能与安全硬化
降采样、批量刷新、无锁结构或分片、指标脱敏、抓取速率限制、内存水位自检与回退策略。

#### 目标
1. 将高频指标更新 CPU 占用再降低 30%（相对 P8.2 基线）。
2. 防止指标标签中出现敏感值（repo 私有名字、IP 地址全量形式）。
3. 提供内存水位自检（> 配置阈值自动降级禁用原始样本）。
4. 防御滥用：导出接口被高频轮询不拖垮主流程。

#### 范围
- 性能：线程本地缓冲（TLS struct）+ 批量合并（flush 周期 500ms）。
- 分片：Histogram 桶数组按 CPU 核数分片（reduce 时合并）。
- 脱敏：
   - 仓库名：hash(repo) 取前 8 字符（可配置 salt）。
   - IP：保留前两段（a.b.*.*），或直接分类标签（private|public|loopback）。
- 降采样：对高频 TLS 握手耗时采样率 1:N（默认 N=5，可配置），保持统计稳定性。
- 内存水位：统计结构体 size + raw buffer 占用，超过阈值触发事件 `MetricMemoryPressure` 并停用 raw buffer。

#### 非目标
- 不实现自动回补（疏漏数据不追溯）。
- 不做跨进程合并优化。

#### 策略参数（建议）
```
observability.performance: {
   batchFlushIntervalMs: 500,
   tlsSampleRate: 5,
   maxMemoryBytes: 8_000_000,
   enableSharding: true,
   redact: { repoHashSalt: "<salt>", ipMode: "mask" }
}
```

#### 落地步骤
1. 实现线程本地缓冲（thread_local! Vec<PendingOp>）。
2. 后台 flush 任务：聚合后写主原子计数/桶。
3. Sharding：初始化时按 `num_cpus` 分配分片数组；读取时合并。
4. 脱敏模块：统一 `redact_repo(name)` / `redact_ip(addr)` API；在事件适配层应用。
5. 采样率：在更新入口处 `if COUNTER.fetch_add(1) % sample_rate ==0` 决定是否记录。
6. 水位监控：每 10s 估算内存（粗略：结构数量 * size 常量 + raw buffer len），超限发事件并禁用 raw。
7. 压测：模拟 20k/s TLS 事件评估 CPU 改善。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| 批量合并 | flush 前后最终数值一致 |
| 采样率 | 记录数接近 1/N 误差 < 10% |
| 脱敏 | repo/IP 不出现原始字符串 |
| 水位超限 | 触发 memory pressure 事件 + raw 停用 |
| Sharding | 并发更新无数据丢失 |

#### 接受标准补充
| 指标 | 目标 |
|------|------|
| 批量 flush CPU 降幅 | ≥30% 相对直接原子更新基线 |
| Histogram 读取合并开销 | P95 < 2ms（300 序列） |
| 采样分位偏差（P95） | |observed - true| / true < 5% |
| 脱敏覆盖率 | 100%（无明文 repo/IP 通过随机抽样 1k 标签） |
| 水位自动降级触发延迟 | <10s |
| Sharding 线性扩展 | 4 核 vs 8 核吞吐提升 >= 1.7x |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| flush 延迟导致丢数据 | 任务退出前强制 flush（Drop 实现） |
| 脱敏误伤（调试困难） | 提供 debugMode 开关（仅本地开发显示原文） |
| 采样导致分位偏差 | 动态调降 sample_rate 当事件频率低 |

#### 回退策略
- 关闭 `observability.performance.enableSharding` 或 `batchFlushIntervalMs=0` 回到直接原子更新。


### P8.7 灰度与推广策略
分层开关：`basic`（仅埋点）→`aggregate`→`export`→`ui`→`alerts` 全量；灰度日志与回退方案。

#### 目标
1. 以最小风险逐步启用各子能力，出现性能或稳定问题快速回退。
2. 记录每次层级切换事件（含旧→新、时间戳、原因）。
3. 提供自动评估脚本：在试运行 24h 后输出资源与告警稳定性报告。

#### 阶段划分
| 层级 | 条件 | 包含能力 |
|------|------|----------|
| basic | 默认启用 | P8.1 指标注册 + 更新 |
| aggregate | 手动开 | + P8.2 窗口/分位 |
| export | 资源稳定 | + P8.3 导出接口 |
| ui | 使用方需要 | + P8.4 面板 |
| alerts | 指标稳定 | + P8.5 告警引擎 |
| optimize | 高并发场景 | + P8.6 性能硬化 |

#### 决策准入（示例阈值）
- 升级至 aggregate：CPU 增量 < 2%，无 OOM；
- 升级至 export：上一层 6h 内无 memory pressure；
- 升级至 ui：用户侧需要图形化 & 指标 ≥ 50 条；
- 升级至 alerts：过去 24h 告警规则评估耗时 < 5% tick 时间；
- 升级至 optimize：事件速率峰值 > 5k/s。

#### 监控与回退
- 每层提供自检指标：`observability_layer` Gauge（数值枚举）。
- 触发条件（任一满足） → 自动降级一层并发事件 `LayerAutoDowngrade`：
   - CPU 增量 > 5%
   - 内存 > maxMemoryBytes * 1.2
   - 导出 5xx 比例 > 0.05
   - 告警评估超时 > 20% tick

#### 配置结构
```json
{
   "observability": {
      "layer": "basic", // 可选 basic|aggregate|export|ui|alerts|optimize
      "autoDowngrade": true
   }
}
```

#### 落地步骤
1. 定义层级枚举与状态机（提供 promote/downgrade API）。
2. 编写层级切换事件与指标。
3. 在各子模块初始化时检查当前层级（不满足则跳过）。
4. 自动评估任务（每 5m）根据资源指标决定是否降级。
5. 脚本：收集 24h 指标生成 `observability-eval.json`（资源趋势 / 阈值是否触发 / 建议下一层）。

#### 测试矩阵
| 场景 | 断言 |
|------|------|
| 升级 | 调用 promote 后 layer 指标更新且事件发射 |
| 不满足条件升级 | 返回错误并记录日志 |
| 自动降级 | 伪造高 CPU -> 触发 LayerAutoDowngrade |
| 回退 | 设置 layer=basic 时高层组件不初始化 |

#### 风险与回退
| 风险 | 缓解 |
|------|------|
| 频繁抖动 | 增加最小驻留时间（如 30m） |
| 自动降级误判 | 记录诊断指标（降级原因细分） |
| 状态机错乱 | 单元测试覆盖所有迁移边 | 

#### 回退策略
- 手动设置 `layer=basic`：即刻停止聚合/导出/UI/告警。

#### 回退与验证 Checklist（补充）
| 步骤 | 操作 | 期望结果 |
|------|------|----------|
| 1 | 记录当前层级 & 关键指标快照 | 生成 pre-downgrade.json |
| 2 | 设置 layer=basic | 仅基础计数仍增长，无窗口/告警线程日志 |
| 3 | 访问 /metrics | 若 export 层已降级应 404 或仅基础指标 |
| 4 | 打开 UI 面板 | Observability 菜单隐藏或提示已禁用 |
| 5 | 恢复 layer=aggregate | 聚合线程启动，1m 窗口填充 |
| 6 | 升级至 alerts | 告警评估日志出现 /alertsEnabled=true |
| 7 | 人工触发告警条件 | firing 事件产生，并在面板出现 |
| 8 | 清除条件并等待 coolDown | resolved 事件产生 |
| 9 | 自动降级注入（伪造高 CPU） | LayerAutoDowngrade 事件，层级下降 |
| 10 | 重新评估资源正常后手动升级 | 升级成功，无重复降级抖动 |


---
## 3. 实现说明（占位）
### 3.1 P8.1 基础指标与埋点标准化（实现说明占位）
TODO:
- [ ] 建立 `core/metrics/registry.rs`（注册/查找）
- [ ] 宏 `metric_counter!` / `metric_histogram!` 生成静态描述
- [ ] 事件适配器 `core/metrics/event_bridge.rs`
- [ ] 单元测试：重复注册、标签排序、线程并发更新
- [ ] 基线基准测试（criterion）输出 CPU 额外占用

### 3.2 P8.2 指标聚合与存储层（实现说明占位）
TODO:
- [ ] `window.rs` 定义 WindowedCounter/HistogramWindow
- [ ] `quantile/ckms.rs` 实现 epsilon 配置
- [ ] 旋转调度 `rotation_task`（tokio interval）
- [ ] Lazy 合并缓存（1h/24h）过期字段
- [ ] 内存估算与自检指标 `metrics_mem_bytes`

### 3.3 P8.3 指标导出与采集接口（实现说明占位）
TODO:
- [ ] 路由 `GET /metrics` `GET /metrics/snapshot`
- [ ] Prom 编码器（HELP/TYPE + 序列缓存）
- [ ] Snapshot 过滤与分页
- [ ] 令牌桶限流（atomic refill）
- [ ] Token 热更新监听 & 事件
- [ ] 集成测试（授权/限流/性能）

### 3.4 P8.4 可观测性前端面板 UI（实现说明占位）
TODO:
- [ ] `stores/metrics.ts` 缓存与 stale-while-refresh
- [ ] 通用图表组件 + 降采样算法 util
- [ ] 时间范围选择器与 URL state 同步
- [ ] Panels: Overview→Git→Network→IpPool→TLS→Proxy→Alerts 逐步实现
- [ ] 单测：缓存命中/失败重试/降采样正确性

### 3.5 P8.5 阈值告警与 Soak 集成（实现说明占位）
TODO:
- [ ] DSL parser （lexer + 简单 Pratt 或前缀）
- [ ] AST 执行器 & 指标读取抽象层
- [ ] 状态机与去抖实现
- [ ] 规则热更新 watch + 校验
- [ ] Soak 报告扩展字段注入
- [ ] 测试：表达式、去抖、合并、恢复

### 3.6 P8.6 性能与安全硬化（实现说明占位）
TODO:
- [ ] TLS thread_local 缓冲结构设计（可复用数组池）
- [ ] 分片 Histogram 结构 + 读取归并
- [ ] 脱敏库 `redact.rs`（repo/ip）
- [ ] 内存水位监控 + 事件发射
- [ ] 采样率调节策略（低频降采样关闭）

### 3.7 P8.7 灰度与推广策略（实现说明占位）
TODO:
- [ ] 层级枚举 + promote/downgrade API
- [ ] 资源评估任务（CPU/内存采样）
- [ ] 自动降级判定 & 事件
- [ ] 评估报告生成脚本（导出 JSON）
- [ ] 集成测试：多层切换、异常注入
