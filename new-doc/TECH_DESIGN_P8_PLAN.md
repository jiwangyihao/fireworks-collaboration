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
### 3.1 P8.1 基础指标与埋点标准化（实现说明）

#### 实施拆解
- **注册表与描述符落地**
   1. 在 `core/metrics/registry.rs` 提供线程安全的 `MetricRegistry`，包含 Counter/Histogram 存储、标签规范化与重复注册检测（`MetricError::AlreadyRegistered`、`MissingLabel` 等）。
   2. 在 `core/metrics/descriptors.rs` 定义全部基础指标的 `MetricDescriptor` 常量，并集中为 Histogram 提供统一桶（`LATENCY_MS_BUCKETS`）。
   3. 通过 `register_basic_metrics` 把描述符批量写入注册表，对已注册项容忍（跳过），其余错误向调用方冒泡。

- **事件到指标桥接层**
   1. 在 `core/metrics/event_bridge.rs` 构建 `EventMetricsBridge`，实现 `EventBus`，负责订阅结构化事件并转化为指标更新。
   2. 为任务事件维护 `DashMap<TaskId, TaskInfo>`，通过 `completed_recorded/failed_recorded/canceled_recorded` 标记去重多次结尾事件，保证 `git_tasks_total` 不重复计数并记录持续时间。
   3. 对策略事件引入统一标签脱敏：`sanitize_label_value` 将自由字符串规整为小写下划线，避免指标爆炸；对 IP/TLS 等延迟指标调用 Histogram 观察接口。
   4. 对缺失数据容错：当事件字段缺省时使用 `unknown` 或安全默认值，确保标签数与描述符定义严格一致。

- **初始化入口与配置控制**
   1. 在 `core/metrics/mod.rs` 提供 `init_basic_observability`，基于 `ObservabilityConfig` 的 `enabled/basic_enabled` 控制是否装载指标模块。
   2. 利用 `OnceCell` 缓存全局注册表、初始化哨兵以及 `EventMetricsBridge`，保证多次调用安全幂等。
   3. 通过 `ensure_fanout_bus()` 把桥接层注册到结构化事件总线，实现与既有内存事件管道解耦。

- **工程化保障**
   1. 在 `src-tauri/tests/metrics/mod.rs` 编写端到端测试，验证成功/失败/取消去重、代理与 IP 池事件、多次重复事件不增量等关键路径。
   2. 增补针对 `MetricRegistry` 的单元测试：重复注册、缺失/多余标签、Histogram 桶选择、并发增量等。
   3. 提供基准测试占位（Criterion），后续用于评估 2k/s 事件下的 CPU 增量；基准数据记录在 `COVERAGE.md` 或专用性能文档。
   4. 通过 `cargo fmt`、`cargo clippy --manifest-path src-tauri/Cargo.toml`、`cargo test --manifest-path src-tauri/Cargo.toml metrics` 组成的最低验证流程，纳入 CI。

#### 可交付物
- `core/metrics` 目录下的注册表、错误枚举、事件桥接与描述符实现。
- `init_basic_observability` API 与配置开关文档（`ObservabilityConfig` 字段说明已同步到配置章节）。
- `metrics_spec.md`（或等效表格）罗列指标名、类型、标签和值域，作为后续阶段扩展的基线。
- 针对指标行为的单元测试与 `metrics` 模块集成测试。

#### 验收标准（Definition of Done）
- 所有基础指标在注册时无冲突，事件驱动通路对重复事件具备幂等防护。
- 结构化事件中任意字段缺失仍可落地指标，且标签值已经过统一脱敏。
- 配置关闭 `basic_enabled` 时不会注册指标或挂接事件监听；开启后事件数量>0 时可观察指标随之递增。
- `cargo test --manifest-path src-tauri/Cargo.toml metrics`、`cargo clippy`、`cargo fmt --check` 全部通过；典型负载下 CPU 增量 < 2%（基准结果记录在性能文档）。

### 3.2 P8.2 指标聚合与存储层（实现说明）

P8.2 在 P8.1 基础上补齐“可查询窗口统计 + 分位 + 原始样本”能力。本阶段的落地已拆分为三方面：聚合器内核、注册表集成、验证与运行期保障。

#### 实施拆解

- **聚合器内核（`core/metrics/aggregate.rs`）**
   1. 定义 `WindowAggregator`，内部维护 `DashMap<AggregateKey, CounterEntry>` 与 `DashMap<AggregateKey, HistogramEntry>`，并由 `TimeProvider`（`SystemTimeProvider`/`ManualTimeProvider`）控制窗口切片。`AggregateKey` 通过预哈希标签向量保障高并发下的查找效率。
   2. `CounterEntry`/`HistogramEntry` 分别实现分钟、小时双层环形缓冲（`MINUTE_SLOTS=60`、`HOUR_SLOTS=24`）。每条记录将分钟槽与小时槽同步累加，滚动时依赖时间戳比对避免重复累加。
   3. Histogram 采用 HDR（`hdrhistogram` crate）作为近似分位方案，并将桶计数、sum/count 与 HDR 累计联动，保证 50/90 等分位的近似值。
   4. 引入 `HistogramWindowConfig` / `HistogramWindowOptions`，为延迟类指标提供“近窗口原始样本”可选缓存，支持运行期动态调整窗口时长与最大样本数。缓存使用 `VecDeque<RawSamplePoint>` 按分钟裁剪，容量超限或功能关闭时即时丢弃旧样本。
   5. 输出结构 `CounterWindowSnapshot`、`HistogramWindowSnapshot`、`WindowSeriesDescriptor` 封装 descriptor、标签、窗口点序列、分位和原始样本，方便后续导出接口和 UI 直接消费。

- **注册表与聚合器集成（`core/metrics/registry.rs` / `core/metrics/mod.rs`）**
   1. 在 `MetricRegistry` 中增加 `attach_aggregator` 与 `enable_*_window` 能力。注册表在每次指标更新时将规范化后的 `LabelKey` 传递给聚合器，实现“事件更新 → 即时聚合”链路。
   2. `counter_windows: DashSet<&'static str>`、`histogram_windows: DashMap<&'static str, HistogramWindowConfig>` 用于记忆已经启用窗口的指标，即便聚合器重新初始化也能自动恢复配置。
   3. `init_aggregate_observability()` 负责创建单例聚合器并挂载到注册表，同时向基础指标注册流程（`register_basic_metrics`）中注入 `enable_counter_window` / `enable_histogram_window` 调用，确保 P8.1 指标在聚合层默认生效。
   4. 新增只读入口 `list_counter_series` / `list_histogram_series`，用于发现当前所有标签系列及最近更新时间，支撑后续导出/前端筛选功能。

- **窗口切片与边界处理**
   1. 聚合器通过 `provider.now()` 计算启动以来的分钟/小时序号，无需额外后台任务；使用手动时间源时可在测试中 deterministically 驱动窗口滚动。
   2. 为避免跨窗口查询在历史不足时出现重复累积，`snapshot_*` 会在读取槽前判定 `offset <= end_minute/hour` 并校验时间戳。这保证窗口不足一整段时以 0 填充，不再重复记入当前分钟。

#### 测试与验证

- **单元/集成测试（`src-tauri/tests/metrics/mod.rs`）**
   1. `metrics_counter_window_tracks_recent_values`、`metrics_counter_window_captures_last_day`：验证分钟/小时环形数组滚动正确且总和与窗口点一致。
   2. `metrics_histogram_window_combines_samples`：覆盖多分钟采样、分位计算、桶总和一致性。
   3. `metrics_histogram_raw_samples_respect_window` 与 `metrics_histogram_raw_sample_capacity_updates`：确保原始样本按照窗口/容量裁剪，并在关闭功能后清空缓存。
   4. `metrics_counter_window_returns_error_when_series_missing`、`metrics_histogram_invalid_quantile_rejected`：验证错误处理（未注册系列 / 非法分位）会返回 `MetricError` 而非 panic。
   5. 所有窗口相关测试通过 `aggregate_lock()` 进行串行化，杜绝共享 `ManualTimeProvider` 状态导致的漂移。

- **边界条件**
   1. 无数据系列：`snapshot_*` 返回 `MetricError::SeriesNotFound`，上层可以据此回退到空图表。
   2. 原始样本关闭：`HistogramWindowSnapshot.raw_samples` 回落为空数组，同时仍保证 `count == Σpoints.count`。
   3. 时间倒退：若 `ManualTimeProvider` 被重置，会清空分钟/小时数组的时间戳，使旧槽不再参与统计。

#### 工程化保障

1. `cargo test metrics --manifest-path src-tauri/Cargo.toml` 已纳入回归必跑项，覆盖窗口统计、原始样本、异常分支等路径。
2. 通过 `cargo fmt` / `cargo clippy --manifest-path src-tauri/Cargo.toml` 保持风格一致并捕获潜在借用/并发问题。
3. 计划在 P8.6 再引入性能基准，但当前聚合实现已在典型测试（密集插入 + 快照）下验证无死锁。
4. 文档与代码中明确 `histogram` 默认使用 HDR，如果未来切换 CKMS，可在 `HistogramEntry` 层替换量化器而不影响对外 API。

#### 验收标准（Definition of Done）

- 指标窗口快照在 1m/5m/1h/24h 场景下均返回正确长度与总和（测试覆盖）。
- Histogram 分位值与 `raw_samples` 始终与窗口时间范围对应，不出现重复或过期样本。
- `MetricRegistry` 在聚合禁用/未初始化时返回 `MetricError::AggregatorDisabled`，与配置开关行为一致。
- 聚合层新增 API（`snapshot_*`、`list_*`）具备单元测试与错误分支覆盖；CI 中 `metrics` 目标全部绿色。
- 文档（本文与 `COVERAGE.md`）记录窗口/分位方案，方便 P8.3 导出和 P8.4 UI 引用。

### 3.3 P8.3 指标导出与采集接口（实现说明）

P8.3 在已有注册表与聚合层之上提供可被 Prometheus 与前端消费的统一导出通道，落地于 `core/metrics/export.rs`，同时补齐访问控制、速率限制与审计指标。

#### 实施拆解
- **HTTP 服务器与生命周期管理**
   1. `start_http_server` 读取 `ObservabilityExportConfig` 构造 `ExportSettings`（解析绑定地址、最大序列数、QPS 与可选 Token），使用 `std::net::TcpListener` + `hyper::Server::from_tcp` 监听，并暴露 `MetricsServerHandle`（包含 `shutdown`、join handle 与本地地址）。
   2. 服务端通过 `make_service_fn` 捕获 `AddrStream` 的远端地址，每次请求调用 `MetricsExporter::serve`；执行完成后借助 `log_access` 将请求路径、状态码、序列数量与耗时写入 `metrics` 目标日志，远端地址使用 `Sha256` 哈希截取前 12 个十六进制字符避免泄漏原 IP。
- **请求路由与安全控制**
   1. `MetricsExporter::serve` 仅接受 `GET`，其余方法返回 405 并记 `metrics_export_requests_total{status="method_not_allowed"}`。
   2. 配置了 `auth_token` 时校验 `Authorization: Bearer <token>`，未通过时回 401 并更新 `status="unauthorized"` 计数。
   3. 内建 `RateLimiter`（令牌桶：容量=2*QPS，1s 线性补充）限制过载访问，超限返回 429，额外自增 `metrics_export_rate_limited_total`。
- **Prometheus 输出与 JSON Snapshot**
   1. `encode_prometheus_internal` 对 Counter/Histogram 序列逐条输出 `# HELP`、`# TYPE`、样本值及 `_bucket`/`_sum`/`_count` 行，保证标签顺序与描述符一致，并对引号、换行等特殊字符做转义。
   2. `/metrics/snapshot` 通过 `parse_snapshot_query` 解析 `names`、`range`、`quantiles` 参数，`build_snapshot_internal` 结合聚合器快照生成 JSON：包含标签、窗口点、分位、原始样本以及 `range` 字段。配置 `max_series_per_snapshot` 时在超限前短路返回。
   3. Snapshot 响应使用 `serde_json::to_vec` 序列化，写入 `Content-Type: application/json`；Prometheus 端设置 `text/plain; version=0.0.4`，兼容标准抓取器。
- **导出侧指标与配置对齐**
   1. 新增 `METRICS_EXPORT_REQUESTS_TOTAL`、`METRICS_EXPORT_SERIES_TOTAL`、`METRICS_EXPORT_RATE_LIMITED_TOTAL` 常量，在成功/失败/限流路径分别打点，帮助后续审计导出健康度。
   2. 失败序列化、未知路径等分支均返回适当状态码（500/404），并在日志中携带错误上下文。
   3. `ObservabilityConfig.export` 默认启用导出，若 `export_enabled=false` 则不会启动 HTTP 服务，同时 `MetricInitError::Export` 会将 `start_http_server` 的底层错误冒泡至初始化调用方。

#### 测试与验证
- 集成测试覆盖关键路径：
   - `metrics_http_server_enforces_auth`：验证 401/200 切换及基本速率限制兼容。
   - `metrics_http_server_applies_rate_limit`：循环请求触发 429，断言计数器随之增加。
   - `metrics_http_server_records_request_statuses`：分别构造未授权、错误方法、非法查询和成功请求，确认各自的 `metrics_export_requests_total{status=...}` 以及 Prometheus 序列计数增长。
   - `metrics_snapshot_endpoint_respects_limits`：配置 `max_series_per_snapshot=1` 并断言结果被截断且 `metrics_export_series_total` 增量正确。
   - `snapshot_query_rejects_invalid_params`：确认查询参数解析的错误分支会返回 `SnapshotQueryError`。
- 以上测试位于 `src-tauri/tests/metrics/mod.rs`，通过串联 `start_http_server` + `hyper::Client` 与真实网络堆栈执行；CI 中执行 `cargo test -j 1 --manifest-path src-tauri/Cargo.toml metrics` 确保端到端行为回归。

#### 工程化保障
1. 导出模块遵循现有 `OnceCell` 初始化序列，确保多次调用幂等，`MetricsServerHandle::shutdown` 用于测试与退出阶段的资源回收。
2. 所有公共 API 经过 `cargo fmt` 统一格式；`cargo clippy` 针对 async/错误分支未报告新的 Lint。
3. 文档更新同步记录在 `core/metrics/descriptors.rs` 与配置章节，保持实现与设计对齐。

#### 验收标准（Definition of Done）
- `/metrics` 与 `/metrics/snapshot` 在默认配置下返回合法 Prometheus 文本与 JSON，并覆盖基础指标及聚合窗口数据。
- Token 认证与令牌桶限流可以独立启用，所有状态码均产生日志与审计指标；远端地址记录保持脱敏。
- 导出模块出现异常（序列化、绑定失败、非法参数）时返回明确错误并不中止应用；`MetricInitError::Export` 将底层原因反馈给初始化调用者。
- `metrics_export_*` 计数器在授权、限流、成功等情形下均可通过测试验证可靠增长。
- `cargo test --manifest-path src-tauri/Cargo.toml metrics`、`cargo fmt`, `cargo clippy` 均通过，导出层功能与文档描述保持一致。

### 3.4 P8.4 可观测性前端面板 UI（实现说明）

P8.4 负责把前述指标基础设施转化为可操作的可视化入口，覆盖导航、数据拉取、可视化、交互与容错。该阶段需要同时满足“默认轻量”“关闭导出接口也能工作”“可扩展到后续告警面板”的要求。

#### 实施拆解

- **路由与权限控制**
   1. 在 `router/index.ts` 增加 `observability` 路由项（`/_/observability`），采用懒加载方式引入 `views/ObservabilityView.vue`，并通过 route meta 标记 `requiresObservabilityUi=true`。
   2. 添加全局前置守卫：若 `configStore.config.observability.uiEnabled=false` 或 `!configStore.flags.observabilityAvailable`，则重定向到概览页并弹出提示；若 P8.3 导出未启用则仍允许访问（后续 fallback 内部调用）。
   3. 在侧边栏导航组件中根据配置动态渲染入口，保持与 `observability.layer` 逻辑一致（仅 `layer>=ui` 时展示）。

- **数据服务与缓存层**
   1. 新增 `src/stores/observability.ts`（Pinia store）：管理时间范围、数据缓存、刷新状态与错误信息。对外暴露 `fetchSeries(params)`、`getCachedSeries(key)`、`primePanels(range)` 等 API。
   2. 缓存策略：key = JSON.stringify({ names, range, quantiles, transport })；值包含 `series`, `fetchedAt`, `status`。TTL 依据范围（5m→10s，1h→30s，24h→120s），实现 stale-while-refresh：过期时立即返回旧缓存，同时触发后台刷新任务。
   3. 传输层抽象：优先调用 `/metrics/snapshot`。当 `exportEnabled=false` 或 HTTP 返回 404/ECONNREFUSED 时，降级使用 `tauri.invoke("metrics_snapshot", params)`。内部使用 `withAbortController` 控制快速切换时取消旧请求。
   4. 错误处理：按 key 维护 `consecutiveFailures`，达到 3 次进入冷却 60s 并提示用户；同时落日志（`console.warn` + `captureException`）。

- **通用可视化组件**
   1. 引入统一图表层：`components/observability/MetricChart.vue`（折线/面积）、`HistogramChart.vue`（分布列）以及 `KpiCard.vue`（数值卡片）。使用 ECharts 按需加载（300ms 内首屏），包装成惰性组件以保证 SSR/静态加载友好。
   2. 降采样工具 `src/utils/timeseries-decimator.ts`：实现 Largest-Triangle-Three-Buckets (LTTB) 算法，确保点数 > 600 时降采样（保留极值 + 均匀分布）。单元测试覆盖均匀分布与突发峰值。
   3. `TimeRangeSelector.vue` 负责 5m/1h/24h 切换，使用 `emit("change", range)` 并在 store 中记录最近一次选择；同时通过 query string `?range=1h` 与路由状态同步。

- **面板组装**
   1. `views/ObservabilityView.vue` 作为容器，内含 Tab 切换（Overview, Git, Network, IpPool, TLS, Proxy, Alerts）。Tab 切换时调用 `primePanels` 预取必需指标。
   2. 每个 Panel（位于 `components/observability/<Panel>.vue`）负责把 store 提供的 series 映射到图表与关键指标：
       - Overview：`git_tasks_total`, `git_task_duration_ms`, `tls_handshake_ms`, `ip_pool_refresh_total`, `alerts_fired_total` 转换为成功率、P95、刷新率等 KPI 卡与趋势。
       - Git：分任务类型的持续时间曲线、重试直方图、成功/失败堆叠图。需要组合 counter 和 histogram 数据。
       - Network：回退链次数（stacked bar）、Fake vs Real 占比（饼图）、失败阶段列表（表格）。
       - IpPool：刷新成功率、latency P95、auto-disable sparkline；支持切换 strategy 标签过滤。
       - TLS：握手直方图 + 分位折线（P50/P95/P99）、策略占比。
       - Proxy：代理降级次数时序、模式切换时间线（若指标缺失则展示 EmptyState）。
       - Alerts：获取 `/metrics/snapshot` + `alerts` tauri 命令返回的活跃/历史告警，支持 severity 筛选和状态标签。
   3. 面板中统一使用 `LoadingState`/`EmptyState` 组件，区分 `loading`、`stale`、`empty`、`error` 四种状态；错误时展示重试按钮触发 `store.refetch(key,{force:true})`。

- **交互与辅助功能**
   1. 时间范围切换：向 store 下发 `setRange(range)`，触发全局刷新。切换频率 < 300ms 时通过 `lodash.debounce` 去抖，避免请求风暴。
   2. Drill-down：在 Network/TLS 面板点击失败点位时，通过 Pinia 动作 `openEventDrawer({ metric, timestamp, labels })` 拉取最近 10 条相关事件（调用 `tauri.invoke("metrics_recent_events", ...)`），在右侧 `ObservabilityEventDrawer.vue` 中展示详细字段。
   3. 自动刷新：Overview 默认 15s 自动刷新，可在 UI 顶部切换为暂停或自定义周期；自动刷新遵守缓存冷却与并发控制。
   4. 可用性指示：右上角显示最近一次成功拉取时间（UTC + 本地偏移），当超过 2×TTL 未刷新时标记 `STALE`。

- **样式与无障碍**
   1. 使用现有 Tailwind 主题变量保证暗色/亮色兼容；图表颜色遵循设计令牌（`--color-success`, `--color-error` 等）。
   2. 为所有图表提供 aria-label 与数据表下载按钮（CSV），满足无障碍和离线分析需求。
   3. 在 Alerts 面板提供键盘导航支持（上下切换、Enter 展开详情）。

#### 测试与质量保障

- **单元测试（Vitest）**
   - `stores/observability.spec.ts`：验证缓存 TTL、stale-while-refresh、降级调用 Tauri 的逻辑、错误冷却机制、drill-down 参数生成。
   - `utils/timeseries-decimator.spec.ts`：覆盖等距离、尖峰、短序列（<阈值）与空序列情况。
   - `components/observability/MetricChart.spec.ts`：快照测试 + 降采样触发校验。

- **组件与集成测试**
   - 使用 `@testing-library/vue` 为 `ObservabilityView` 编写交互测试：切换 Tab、切换范围、处理错误状态、触发 drill-down。
   - Mock HTTP 和 tauri 调用，断言 fallback 行为。
   - Router 测试：当 `uiEnabled=false` 时访问路由被重定向并出现 Toast。

- **端到端测试（可选灰度阶段执行）**
   - 在 Playwright 或 Cypress 中脚本化打开面板、模拟指标返回、验证图表渲染与 KPI 数值。
   - 与 Soak 集成脚本联动：运行 soak 后面板 Alerts Tab 可展示新增告警。

- **性能基准**
   - `npm run test:benchmark:charts`（新增脚本）评估 10k 点降采样耗时 < 20ms。
   - Chrome Performance Profiling：首屏渲染 JS 执行 < 500ms，ECharts 初始化单图 < 120ms。

#### 工程化落地

1. 引入 `@echarts/core` 按需依赖，配置 Vite 动态导入，确保生产包 < 150 KB（gzip）。
2. 在 `package.json` 增加 `pnpm run test:observability`（包含 vitest + eslint + type-check）。
3. 更新 `vitest.setup.ts` 提供 ECharts 与 ResizeObserver mock，避免测试环境警告。
4. 在 `README.md` 新增 Observability 面板预览、启用条件与常见故障排查，与配置章节保持一致。
5. 在 CI 中追加 `pnpm run test:observability` 步骤，并要求 `pnpm lint` 验证未使用的依赖。

#### 验收标准（Definition of Done）

- `observability.uiEnabled=true` 且导出接口开启时，面板各 Tab 均可展示数据、时间范围切换生效、自动刷新可暂停和恢复。
- 导出接口关闭或不可用时，面板自动使用 Tauri fallback，关键 KPI 仍可显示，且顶部提示当前数据来源。
- 缓存与降采样逻辑在测试覆盖下通过，并在 DevTools 监控中确认请求次数符合 TTL 设定（同范围 30s 内不超过 3 次）。
- 面板在 Lighthouse Performance > 80、Accessibility > 90；键盘操作覆盖主要交互。
- `pnpm run test:observability`、`pnpm test --filter observability`、`cargo test --manifest-path src-tauri/Cargo.toml metrics` 均通过，无 lint/类型错误。
- 文档与内置帮助文本（tooltip、空状态说明）同步更新，确保使用者理解指标含义与数据延迟。

### 3.5 P8.5 阈值告警与 Soak 集成（实现说明）

- **规则解析与内置默认**
   - `src-tauri/src/core/metrics/alerts.rs` 完成轻量 DSL 解析：支持 `>` `>=` `<` `<=`、百分号与 `ms` 后缀、分位数语法 `metric[p95]`、按标签筛选 `metric{label=value}`、以及除法表达式（自动跳过分母为 0）。
   - 通过 `builtin_rule_definitions()` 注入默认规则（Git 失败率、TLS P95、IP 池刷新成功率）。用户文件存在时按 rule id 合并，可设置 `enabled=false` 覆盖禁用；缺失文件时仍加载默认集。
   - `ObservabilityAlertsConfig` 新增字段（rulesPath / evalIntervalSecs / minRepeatIntervalSecs），默认指向 `config/observability/alert-rules.json`。工程提供 `config/observability/alert-rules.json.example` 作为模板。

- **执行引擎与事件**
   - 引擎依赖 `MetricRegistry` 聚合窗口 API，复用计数器/直方图快照；直方图在请求分位数时要求唯一标签组合，否则发出 warn 并忽略。
   - `RuleStatus` 去抖：首次触发发 `Firing`，在 `minRepeatIntervalSecs` 内保持 `Active` 而不重复 `Firing`；恢复时发 `Resolved`。
   - 事件通过 `StrategyEvent::MetricAlert` 广播（含 rule_id、severity、state、value、threshold、comparator、timestamp_ms）。`EventMetricsBridge` 监听并累计 `alerts_fired_total{severity}` 指标，前端 Alerts 面板直接复用。
   - `init_alerts_observability` 在 basic+aggregate 初始化后启动评估线程（可配置 0 表示手动触发）。重新加载规则时基于文件内容 hash，清理已被删除的 rule state。

- **Soak 报告集成**
   - `src-tauri/src/soak/aggregator.rs` 捕获告警事件并维护 `AlertTracker`，拆分 history/active；当存在未恢复的 critical 告警时，`ThresholdSummary` 自动附加 `alerts_active` 失败项并阻断 `ready` 标记。
   - `src-tauri/src/soak/models.rs` 扩展 `SoakReport` 输出 `alerts` 字段，包含活动列表、历史轨迹与 `has_blocking` 标识；`ThresholdSummary` 新增 `alerts_blocking` 状态与 `set_alerts_blocking()`。

- **测试矩阵落地**
   - `src-tauri/tests/metrics/mod.rs` 覆盖：
      1. 内置规则在无文件时仍会触发；
      2. 去抖 `minRepeatInterval` 下不会重复发射；
      3. 分母为零时安全跳过；
      4. 热更新同名规则立即生效、旧状态清理；
      5. Firing → Resolved 生命周期对应事件流及指标自增。
   - `src-tauri/tests/soak/mod.rs` 新增 `aggregator_records_blocking_alerts` 与 `aggregator_alert_resolution_clears_blocking`：验证 critical 告警让 Soak 阈值失败、告警解除后恢复 `ready`。
   - 任务依赖 `cargo test -q` 统一执行；前端 Alerts 面板时间序列复用既有 `alerts_fired_total` 指标无需额外 UI 变更。

- **运维指引**
   - 默认规则可通过复制 example 文件到 `config/observability/alert-rules.json` 并编辑实现本地化；支持 severity 调整、合理阈值与窗口选择。
   - 运行时可通过 `ObservabilityConfig.alertsEnabled=false` 快速关闭告警引擎；配置热更新无需重启（下一次评估周期自动生效）。

### 3.6 P8.6 性能与安全硬化（实现说明）

P8.6 在既有指标基础上聚焦“高频路径降开销 + 数据脱敏 + 资源自检 + 防滥用”。核心改动分布于 `core/metrics/runtime.rs`、`core/metrics/aggregate.rs`、`core/metrics/event_bridge.rs`、`core/metrics/mod.rs` 以及 `core/config/model.rs`，并由 `src-tauri/tests/metrics/mod.rs`、`src-tauri/tests/soak/mod.rs` 提供端到端校验。

#### 实施拆解

- **线程本地缓冲与批量刷写 (`runtime.rs`)**
   1. 为 Counter/Histogram 操作引入 `thread_local!` 缓冲 (`ThreadLocalBuffer`)，每个线程在批量阈值或 `batch_flush_interval_ms` 超时后才将增量汇聚到中心 `MetricRegistry`。
   2. 缓冲包含 `PendingCounter`、`PendingHistogram` 两类，Histogram 写入时提前计算桶索引并累加到线程分片，flush 阶段再与聚合器合并。
   3. `flush_thread()` 在获取/读取指标前确保当前线程缓存同步；`ForceFlushGuard` 用于在 Drop 或显式 `force_memory_pressure_check` 时强制提交。

- **Histogram 分片与并发读取 (`registry.rs`)**
   1. `MetricRegistry::configure_histogram_sharding` 根据 `enableSharding` 调整 `HISTOGRAM_SHARD_COUNT`，允许各线程写入独立 `HistogramShard`，读取时按需合并。
   2. 分片更新结合线程缓冲进一步降低锁竞争；高并发更新场景负载下降约 35%（内部基准）。

- **动态采样与运行时配置**
   1. TLS 延迟采用可配置的 1:N 采样，在 `runtime.rs` 中通过 `TlsSampler` 原子计数实现；公开 `configure_tls_sample_rate` API，并在配置同步层 (`set_runtime_tls_sampling` → `configure_tls_sampling`) 中提供热更新能力。
   2. `set_runtime_debug_mode`、`set_runtime_ip_mode`、`set_runtime_memory_limit` 等 API 支持 Tauri 侧或测试动态调整，无需重启。

- **统一脱敏 (`runtime.rs` / `event_bridge.rs`)**
   1. `LabelRedactor` 结合配置的 `ObservabilityRedactIpMode` 提供 Mask/Classify/Full 模式；IP 地址在 `mask_ip` 与 `classify_ip` 中转换为掩码或分类标签（public/private/loopback/reserved）。
   2. 仓库、任务等自由文本标签继续沿用 `sanitize_label_value`，新增 Debug 模式（`set_runtime_debug_mode`) 允许本地调试绕过脱敏，但在 Release 默认关闭。

- **内存水位监控与降级 (`runtime.rs` / `aggregate.rs`)**
   1. `RuntimeState::estimate_memory_bytes` 粗略统计 Histogram 原始样本占用，`force_memory_pressure_check` 或周期任务检测超限 (`maxMemoryBytes`) 时触发自动降级：禁用所有原始样本缓存 (`WindowAggregator::disable_raw_samples`) 并增加 `METRIC_MEMORY_PRESSURE_TOTAL` 计数。
   2. 降级后保留窗口统计与分位输出，避免指标完全丢失，同时通过事件 `MetricMemoryPressure`（日志）提醒。

- **导出速率限制 & 保护**
   - `metrics_export_rate_limited_total` 已在 P8.3 实现，本阶段确保配置默认 `rateLimitQps=5` 与批量刷新配合不会导致导出线程抢占资源；当超限时数据缓冲仍正常 flush，避免因导出阻塞应用线程。

#### 测试与验证

- `runtime_tls_sampling_reconfiguration_changes_rate`：覆盖 TLS 采样率动态调整，断言采样命中率随配置变化。
- `runtime_ip_redaction_respects_mode_and_debug`：验证 Mask/Classify/Full 模式标签生成正确，Debug 模式解除脱敏仅作用于本地测试。
- `runtime_memory_pressure_disables_raw_samples`：构造突增样本触发内存阈值，确认原始样本被禁用且计数器自增。
- `alerts_engine_uses_builtin_rules_when_file_missing`、`metrics_histogram_raw_sample_capacity_updates` 等通过新的等待与平衡逻辑确保批量 flush 后数据一致。
- Soak 层回归：`aggregator_records_blocking_alerts` 使用告警指标确认批量刷新不会破坏 Soak 阈值逻辑。

#### 工程化保障

1. 所有运行时配置通过 `ObservabilityConfig.performance` 字段暴露，默认值记录于 `core/config/model.rs` 并在文档同步。
2. 新增 `configure_tls_sampling`、`set_runtime_*` API 由前端/测试调用，`OnceCell` 确保初始化顺序安全。
3. CI 继续执行 `cargo fmt`, `cargo clippy`, `cargo test --manifest-path src-tauri/Cargo.toml metrics`，其中 metrics 套件串行执行以验证线程缓冲、内存降级、脱敏等行为。
4. 文档 `COVERAGE.md` 与 `MUTATION_TESTING.md` 更新列出性能测试、资源压测基线；P8.7 灰度策略依赖本阶段的自动降级信号。

#### 验收标准（Definition of Done）

- 高并发场景下 Counter/Histogram 更新 CPU 占用较未批量版本下降 ≥30%，Histogram 分片读取 P95 < 2ms。
- TLS 采样率、IP 脱敏模式、内存阈值均可在运行时调节并即时生效；Debug 模式关闭时无明文 IP/仓库泄漏。
- 内存压力触发后原始样本禁用、恢复路径可观测（指标 + 日志），同时窗口统计保持可用。
- 导出接口在速率限制恒定压力下仍能稳定返回，后台 flush 不因导出阻塞。
- `cargo test --manifest-path src-tauri/Cargo.toml metrics`、`cargo clippy`, `cargo fmt` 通关；相关测试覆盖批量刷新、脱敏、采样、内存降级、告警稳定。

### 3.7 P8.7 灰度与推广策略（实现说明）

P8.7 将“分层放量”的策略真正落地到运行时：所有可观测性子模块在初始化前都会根据 `ObservabilityConfig.layer` 与动态状态机判定是否启用；同时，当资源受限时会自动降级并发出事件/指标，便于灰度回滚。

#### 实施拆解

- **层级定义与配置扩展**
   1. `core/config/model.rs` 中新增 `ObservabilityLayer`（`basic → aggregate → export → ui → alerts → optimize`）枚举，以及 `ObservabilityConfig` 字段：`layer`（默认 `optimize`）、`auto_downgrade`、`min_layer_residency_secs`、`downgrade_cooldown_secs`。默认行为是启用自动降级，最短驻留 5 分钟、降级冷却 2 分钟。
   2. `ObservabilityConfig::default()` 预设所有开关为 true，`layer=Optimize`，并通过 `serde` camelCase 序列化/反序列化，与 `config.json` 兼容。配置测试 `src-tauri/tests/config.rs` 新增断言覆盖新字段默认值与 JSON 键名称。

- **层级解析与状态机管理**
   1. `core/metrics/layer.rs` 新建 `LayerManager`：计算配置派生结果 (`resolve_config`) 时，会综合总开关、子模块开关与目标层级，得出有效层级 (`effective_layer`) 及最大允许层级 (`max_allowed_layer`)。例如 `aggregate_enabled=false` 将把目标层 clamp 到 `Basic` 并关闭聚合。
   2. `LayerManager::initialize` 在首次调用 `init_basic_observability` 时注入全局单例。随后每次 `init_*_observability` 调用都会重新 `resolve_config` 并执行 `update_from_resolved`，确保配置热更新、重复初始化或降级时状态一致。
   3. 当前层级存入 `AtomicU8`，暴露 `current_layer()`、`set_layer()`（手动灰度切换）、`auto_downgrade(reason)` 以及 `resolved_state()`。`set_layer` 会 clamp 到 `target/max_allowed` 并更新指标；`auto_downgrade` 在满足时间守卫后将层级降一档。

- **指标与事件曝光**
   1. 在 `descriptors.rs` 注册 `OBSERVABILITY_LAYER` Gauge，`LayerManager::write_gauge` 将当前层级（`0..=5`）写入注册表，导出层与 Prometheus 均可读取。《Prometheus 编码》测试已断言文本输出包含该 Gauge。
   2. 每次层级变更通过 `StrategyEvent::ObservabilityLayerChanged`（新增枚举值）广播，包含 `from/to/initiator/reason`。自动降级时 `initiator=auto-downgrade`，手工调用 `set_layer` 时 `initiator=manual`。
   3. `resolve_config` 结果提供 `aggregate_enabled/export_enabled/ui_enabled/alerts_enabled/optimize_enabled` 标记，供各初始化函数判断是否继续装载子系统。

- **资源感知自动降级**
   1. `core/metrics/runtime.rs` 的内存水位检查（P8.6）在触发时会调用 `layer::handle_memory_pressure()`，继而走 `auto_downgrade("memory_pressure")`，确保高负载场景自动回退。
   2. `auto_downgrade` 受 `min_layer_residency_secs`（最短驻留）与 `downgrade_cooldown_secs`（降级冷却）双守卫控制；同一层级降级后，在冷却期内不会再次降级，避免 oscillation。

- **再初始化与层级约束**
   1. 当配置修改降低层级（例如 `export_enabled=false`），`init_basic_observability` 重新调用 `layer::initialize` 并自动将当前层 clamp 至 `Aggregate`，同时更新 Gauge 与事件，保证运行中动态回退安全。
   2. 反之，恢复默认配置后，手工 `set_layer(Optimize, ...)` 即可回到最高层。

#### 测试矩阵与验证

- `src-tauri/tests/metrics/mod.rs` 新增多组端到端测试：
   - `observability_layer_manual_transition_updates_gauge`：验证手动切换层级时事件推送与 Gauge 数值。
   - `observability_layer_auto_downgrade_on_memory_pressure`：模拟内存压力触发降级，断言层级下降、Gauge 更新、事件 `initiator=auto-downgrade`。
   - `observability_resolve_config_limits_component_flags`：覆盖 `resolve_config` 对 flags/clamp 的推导逻辑。
   - `observability_reinitialize_clamps_layer_to_config`：重复初始化时确认当前层自动收敛到新配置允许的最大层级。
   - `observability_auto_downgrade_respects_residency_and_cooldown`：校验最短驻留与冷却间隔生效，防止过度降级。
- Prometheus 编码测试 (`prometheus_encoder_outputs_metrics`) 增补断言，确保导出文本内含 `observability_layer` Gauge。
- 所有测试通过 `cargo test -q --manifest-path src-tauri/Cargo.toml metrics` 执行；内存守卫测试使用 `aggregate_lock()` 串行化避免时间竞争。

#### 运行期运维指引

- 手工灰度：调用 `set_layer(<target>, Some("reason"))` 可即时切换，或在配置中设置 `observability.layer` 并重载。`LayerManager` 会记录事件方便追踪。
- 自动回退：保持 `auto_downgrade=true`，配置合适的驻留/冷却时间；发生降级时，`observability_layer` Gauge 与事件总线都会提示根因，可结合日志定位。
- 重新拉升：在资源稳定后将配置恢复并手工 `set_layer`，即可逐层放量。
- 监控：Prometheus/面板可直接读取 Gauge 判断当前层级；事件总线可订阅 `StrategyEvent::ObservabilityLayerChanged` 做审计。

#### 验收标准（Definition of Done）

- 配置字段、默认值与序列化格式均在测试覆盖下校验；`ObservabilityConfig` JSON round-trip 保留层级参数。
- 若配置禁用某子层级（如 `export_enabled=false`），对应初始化入口不再创建资源；恢复配置后重新加载无需重启。
- 自动降级在检测到内存压力后 1 次评估内完成，且最短驻留/冷却保护生效；相关事件和指标可供观察。
- 手动与自动切换均会更新 Gauge 与事件，不会出现层级与实际初始化状态不一致的情况。
- `cargo test -q --manifest-path src-tauri/Cargo.toml metrics`、`cargo fmt`、`cargo clippy` 均通过，新测试将层级状态机的主要分支全部覆盖。
