# P8 实现与维护对接文档（可观测性体系）

> 适用读者：后端/前端开发、运维与 SRE、性能与安全审计、QA
> 关联设计：`TECH_DESIGN_P8_PLAN.md`
> 当前阶段：功能已交付并进入维护优化阶段

---
## 目录
1. 交付范围概述
2. 核心模块映射
3. 配置项与默认值
4. 运行时总体架构与数据流
5. 指标体系与命名规范
6. 事件桥接与数据采集流程
7. 聚合与窗口统计机制
8. 导出接口与访问控制
9. 前端面板结构与交互模型
10. 告警规则与 Soak 集成机制
11. 性能优化与资源控制策略
12. 灰度层级与回退策略
13. 测试矩阵与关键用例
14. 运维操作与故障诊断
15. 后续优化建议
16. 快速校验命令

---
## 1. 交付范围概述
| 主题 | 目标 | 当前状态 | 说明 |
|------|------|----------|------|
| 指标基础层 | 统一注册/标签/命名 | ✅ 运行中 | Registry + Descriptor 去重与标签规范化 |
| 事件桥接 | 事件→指标映射幂等 | ✅ 运行中 | 任务/IP 池/TLS/回退/告警/Soak 事件全部接入 |
| 多窗口聚合 | 1m/5m/1h/24h + 分位 | ✅ 运行中 | HDR 分位 + 可选原始样本缓存 |
| 导出能力 | Prometheus + JSON Snapshot | ✅ 运行中 | 认证/限流/序列上限/脱敏访问日志 |
| 前端面板 | 本地可观测性 UI | ✅ 运行中 | Vue + Pinia 缓存 + LTTB 降采样 |
| 阈值告警 | 静态/比值/窗口规则 | ✅ 运行中 | DSL 解析 + 去抖 + 事件化 + Soak 阻断判定 |
| 性能硬化 | 线程缓冲/采样/分片/脱敏 | ✅ 运行中 | 内存水位自动降级 raw samples |
| 灰度层级 | basic→optimize 状态机 | ✅ 运行中 | 自动降级 + Gauge + 事件审计 |

交付边界：
- 仅单进程（本地工具）内存态指标，不做跨节点汇聚与长期存储。
- Tracing（分布式调用链）、日志全文检索不在本阶段范围。
- 高级查询（PromQL 组合表达式）由外部 Prometheus+Grafana 实现。

## 2. 核心模块映射
| 能力域 | 路径 / 文件 | 关键结构 / API | 说明 |
|--------|-------------|----------------|------|
| 描述符注册 | `core/metrics/descriptors.rs` | `MetricDescriptor` 常量 | 统一指标名/类型/桶/标签定义 |
| 注册表 | `core/metrics/registry.rs` | `MetricRegistry` | 计数/直方图存储、窗口使能、分片配置 |
| 事件桥接 | `core/metrics/event_bridge.rs` | `EventMetricsBridge` | 订阅 Strategy/Task/Alert/Soak 事件更新指标 |
| 聚合窗口 | `core/metrics/aggregate.rs` | `WindowAggregator` | 多窗口环缓冲 + HDR 分位 + 原始样本缓存 |
| 导出服务 | `core/metrics/export.rs` | `start_http_server`, `encode_prometheus` | Prometheus 文本 & `/metrics/snapshot` JSON |
| 告警引擎 | `core/metrics/alerts.rs` | 规则 DSL / 状态机 | 周期评估、去抖、事件广播 |
| 性能运行时 | `core/metrics/runtime.rs` | 线程缓冲 / 采样 / 脱敏 | 批量 flush、TLS 采样、内存水位监控 |
| 灰度层级 | `core/metrics/layer.rs` | `LayerManager` | 层级解析、自动降级、Gauge 写入 |
| 配置模型 | `core/config/model.rs` | `ObservabilityConfig` | 各子功能开关与性能/告警/导出参数 |
| Soak 集成 | `src-tauri/src/soak/*` | `SoakReport` 扩展 | 告警阻断、指标摘要注入 |
| 前端 API | `src/api/metrics.ts` | `fetchMetricsSnapshot` | HTTP / Tauri 双通道获取 Snapshot |
| 前端 Store | `src/stores/metrics.ts` | `ensure`, `isStale` | TTL 缓存 + stale-while-refresh |
| 面板视图 | `src/views/ObservabilityView.vue` | Tabs/Range 切换 | 动态 Tab (告警开关) + 手动刷新 |
| 面板组件 | `src/components/observability/*` | `OverviewPanel` 等 | KPI 汇总、图表、降采样渲染 |

## 3. 配置项与默认值
| 键 | 默认 | 说明 | 影响范围 |
|----|------|------|----------|
| observability.enabled | true | 全局开关（false 时所有子功能关闭仅保留事件） | 全局 |
| observability.basicEnabled | true | 基础指标是否启用（随 enabled=false 强制关闭） | 注册/桥接 |
| observability.aggregateEnabled | true | 是否启用窗口与分位 | 聚合/告警/UI 分位 |
| observability.exportEnabled | true | 是否启动 HTTP 导出 | /metrics 接口 |
| observability.uiEnabled | true | 是否显示前端面板入口 | 前端导航 |
| observability.alertsEnabled | true | 是否启用告警评估线程 | 告警/Soak 阻断 |
| observability.performance.batchFlushIntervalMs | 500 | 线程缓冲 flush 周期（0=禁用缓冲） | 性能/CPU |
| observability.performance.tlsSampleRate | 5 | TLS 握手 1/N 采样 | 延迟 histogram 精度 vs 开销 |
| observability.performance.maxMemoryBytes | 8_000_000 | Raw samples 内存上线 | 自动降级触发 |
| observability.performance.enableSharding | true | Histogram 分片开关 | 高并发更新耗时 |
| observability.performance.redact.repoHashSalt | "" | 仓库名 Hash 盐（空=随机） | 脱敏稳定性 |
| observability.performance.redact.ipMode | mask | IP 脱敏：mask|classify|full | 标签敏感度 |
| observability.export.authToken | null | 访问令牌（为空不校验） | 安全控制 |
| observability.export.rateLimitQps | 5 | `/metrics` QPS 软限制 | 防滥用/资源 |
| observability.export.maxSeriesPerSnapshot | 1000 | Snapshot 最大序列 | 防止过大响应 |
| observability.alerts.rulesPath | config/observability/alert-rules.json | 告警规则文件 | 告警 DSL 加载 |
| observability.alerts.evalIntervalSecs | 30 | 告警评估间隔 | 告警延迟/开销 |
| observability.alerts.minRepeatIntervalSecs | 30 | 同一告警最小重复 firing 间隔 | 去抖 |
| observability.layer | optimize | 期望层级 basic→optimize | 功能放量 |
| observability.autoDowngrade | true | 资源异常自动降级 | 稳定性 |
| observability.internalConsistencyCheckIntervalSecs | 300 | 指标 vs 事件对账周期（0=禁用） | 质量自检 |

> 层级与单独开关冲突时以层级裁剪为准（例如 layer=basic 则自动禁用聚合/导出/UI/告警）。

## 4. 运行时总体架构与数据流
### 总览
事件/任务 → (桥接规范化) → 指标注册表 (线程缓冲 + 分片) → 聚合窗口 (1m/5m/1h/24h) →
	A) 导出（Prometheus / Snapshot）
	B) 告警引擎（周期拉取窗口快照）
	C) 前端面板（Snapshot API / Fallback）
	D) 一致性自检（事件 vs 指标） → 产生自检事件再回流

### 组件职责链
| 阶段 | 组件 | 输入 | 处理 | 输出 |
|------|------|------|------|------|
| 事件捕获 | 各业务模块 / Strategy / Soak | 原始事件结构 | 枚举/字段 | Rust struct |
| 桥接规范化 | EventMetricsBridge | 事件 | 去重/标签/脱敏 | registry 调用 |
| 缓冲写入 | Runtime TLS Buffer | (metric_id,value,tags) | Append / Flush | batched writes |
| 注册表更新 | MetricRegistry | batched writes | Counter +=N / HDR.observe | Series state |
| 聚合窗口 | WindowAggregator | Series state + now() | 写入环槽 / 样本裁剪 | 窗口视图 |
| 导出编码 | Exporter | Registry + Aggregator | 过滤/合并/编码 | 文本/JSON 响应 |
| 告警评估 | AlertsEngine | 窗口快照 | 规则计算/去抖 | MetricAlert 事件 |
| 自检任务 | ConsistencyChecker | 事件粗计数 + 指标 | 计算偏差 | MetricDrift 事件 |
| 层级管理 | LayerManager | 配置/资源事件 | Clamp / 降级 | 功能开关变化 |

### 数据流细节
1. 事件生成：Git 任务结束 / IP 刷新 / TLS 采样 / 回退链 / Soak 汇总 / 告警状态变迁。
2. 桥接：同步调用桥接更新（轻量操作）写入线程局部缓冲；无锁 fast path。
3. Flush：定时或阈值触发，将批量条目投递到注册表（DashMap 或分片结构）。
4. 聚合：写入同时更新对应时间槽；Histogram 更新 HDR（分位数据结构）并缓存 raw 样本（可选）。
5. 导出 / Snapshot：读取时合并分片；对 Histogram 进行 lazy merge（只在请求发生）。
6. 告警：在 evalInterval 内从 Aggregator 拉指定窗口值（含分位），执行表达式树，产出事件。
7. 自检：周期对比事件计数缓冲与窗口累积差异；超阈值生成 MetricDrift → 回到步骤 2。
8. 层级：资源事件（memory pressure/export 5xx）→ LayerManager 调整层级 → 裁剪功能（关闭导出/告警等），减少后续步骤负载。

### 线程与并发模型
| 线程类别 | 主要职责 | 并发控制 |
|---------|---------|---------|
| 主业务线程 | 产生事件 | 仅写 TLS 缓冲（无锁 push）；必要时 flush |
| Flush 定时器 | 扫描缓冲 | 每周期合并写入；短临界区 |
| HTTP 导出线程 | 处理 /metrics & snapshot | 读锁 / 分片遍历；速率限制桶原子操作 |
| 告警评估线程 | 周期评估规则 | 只读快照；输出事件再走桥接 |
| 自检线程 | 一致性漂移检测 | 只读窗口+事件计数；低频 |
| 层级监控线程 | 监听资源信号 | CAS 更新层级状态 |

### 内存占用构成（估算）
| 项 | 构成 | 控制手段 |
|----|------|----------|
| 描述符/注册表 | Hash / Series 元数据 | 指标常量集中定义，防止动态膨胀 |
| Counter 窗口 | 槽数 * 系列数 * (u64) | 限制系列基数（标签枚举） |
| Histogram HDR | 桶结构 + 指针 | 固定桶预设；分片合并只读 |
| Raw 样本 | Vec<value,timestamp> | 容量/窗口上界 + 内存压力禁用 |
| 缓冲区 | 线程局部 Vec | Flush 周期/阈值限制 |

### 热点路径优化点
- 避免导出线程在高频写锁：写入仅更新分片局部；读取合并时短期遍历。
- HDR observe 使用无锁原子更新（或细粒度锁）保证低延迟。
- 标签组装预留 capacity=5 减少 reallocation。

### 故障传播与阻断
| 故障 | 影响路径 | 阻断策略 |
|------|----------|----------|
| 导出线程阻塞 | A/B/C | 层级自动降至 aggregate 关闭导出，前端 fallback |
| 告警评估延迟 | B (告警) | 不影响 A/C（读取独立）；日志提示评估堆积 |
| 内存压力 | D/A/B/C | Raw 样本禁用 + 降级，保持基础指标完整 |
| 大量新系列 | A→内存 | 标签枚举限制 + 审计日志 |

### 时序示例（典型一次事件→告警）
TaskCompleted → Bridge 计数 + Duration Observe → Registry/Histogram 更新 → Aggregator 写 1m 槽 → 告警周期快照读取 fail rate > 阈值 → 生成 MetricAlert 事件 → Bridge → alerts_fired_total + 前端下次刷新展示。

## 5. 指标体系与命名规范
### 命名原则
- 使用 snake_case；计数器统一 `_total`；时间单位 `_ms`；Gauge 无后缀或语义化名称。
- 指标含义一律使用英文简短描述（Prometheus 要求），中文说明置于文档。

### 标签规范
| 约束 | 说明 |
|------|------|
| 标签数量 ≤5 | 超出需评审避免 cardinality 膨胀 |
| 固定枚举 | kind/state/stage/reason 等均为有限集合 |
| 缺失字段回退 | 使用 `unknown` 避免生成新标签键 |
| 字符规范化 | to_lowercase + 非 `[a-z0-9_]` 替换为 `_` |
| 脱敏 | repo 名 hash 前缀；IP 掩码/分类（详见性能章节） |

### 关键指标列表
| 名称 | 类型 | 标签 | 说明 |
|------|------|------|------|
| git_tasks_total | counter | kind, state | Git 任务三态结束计数 |
| git_task_duration_ms | histogram | kind | 任务总时长分布 |
| git_retry_total | counter | kind, category | 重试次数分类统计 |
| tls_handshake_ms | histogram | sni_strategy, outcome | TLS 握手延迟分布 |
| http_strategy_fallback_total | counter | stage, from | Fake→Real 回退链次数 |
| ip_pool_refresh_total | counter | reason, success | IP 池刷新事件结果 |
| ip_pool_latency_ms | histogram | source | IP RTT 样本 |
| ip_pool_selection_total | counter | strategy, outcome | 选取 IP 尝试统计 |
| ip_pool_auto_disable_total | counter | reason | 自动禁用触发 |
| circuit_breaker_trip_total | counter | reason | 熔断触发次数 |
| circuit_breaker_recover_total | counter | (none) | 熔断恢复次数 |
| proxy_fallback_total | counter | reason | 代理降级 |
| alerts_fired_total | counter | severity | 告警触发（按严重级别） |
| soak_threshold_violation_total | counter | name | Soak 阈值未达标计数 |
| observability_layer | gauge | (none) | 0~5 当前层级（basic→optimize） |
| metric_memory_pressure_total | counter | (none) | 内存压力降级次数 |

### Histogram 桶策略（毫秒）
`[1,5,10,25,50,75,100,150,200,300,500,750,1000,1500,2000,3000,5000,+Inf]`
覆盖短延迟（<100ms）精细、中位梯度与长尾；统一桶便于前端与告警复用。

### 一致性校验机制
- 定时对比事件计数 vs 指标（任务、刷新、握手样本）；偏差阈值：任务1% / 刷新2% / 握手5%。
- 发现漂移 → 产生 `MetricDrift` 事件 + 自检指标 (`metric_consistency_drift_total{metric}`)。

### 指标扩展流程
1. 在 `descriptors.rs` 新增描述符，并更新本章节表格。
2. 添加事件桥接逻辑（若有新事件）。
3. 更新前端面板或告警规则（若需要）。
4. 补充/调整测试：注册、桥接、导出文本断言。

### 反模式（禁止）
- 在标签中放入高基数字符串（如完整 repo URL）。
- 临时调试指标未纳入描述符常量集合。
- 以动态拼接字段直接作为指标名（破坏统一命名）。

## 6. 事件桥接与数据采集流程
### 目标
将各业务/策略/运行事件统一转换为规范化指标更新，保证幂等、低开销与标签一致性。

### 事件来源
| 来源 | 事件示例 | 指标影响 |
|------|----------|----------|
| Git 任务 | Task state/progress/error | git_tasks_total / git_retry_total / git_task_duration_ms |
| IP 池 | Selection / Refresh / AutoDisable / IpTripped | ip_pool_selection_total / ip_pool_refresh_total / ip_pool_auto_disable_total / circuit_breaker_* |
| TLS 自适应 | AdaptiveTlsTiming / AdaptiveTlsFallback | tls_handshake_ms / http_strategy_fallback_total |
| 代理 | Proxy Fallback | proxy_fallback_total |
| 告警引擎 | MetricAlert (firing/active/resolved) | alerts_fired_total |
| Soak 汇总 | Threshold violation | soak_threshold_violation_total |
| 内部一致性 | MetricDrift | （可触发告警，不直接建新指标） |

### 桥接结构
- `EventMetricsBridge`：实现事件订阅接口，内部包含：
	- 任务去重 Map（task_id → 状态）避免重复计数。
	- 标签构造助手：枚举值校验+缺失回退+脱敏（调用 runtime redactor）。
	- 统一更新函数：`record_counter_metric` / `observe_histogram_metric`。

### 去重策略
| 场景 | 机制 |
|------|------|
| 任务结束 (completed/failed/canceled) | 任务状态原子标记，重复事件忽略 |
| 回退链 | 每连接事件一次；无连接 ID 则直接累加（允许多连接） |
| AutoDisable | 同一原因 & 冷却窗口内不重复计数（比较 until_ms） |
| 告警 firing→active | active 不再增加 alerts_fired_total |

### 标签规范化流程
事件到达 → 原始字段提取 → 映射枚举（非法=unknown）→ 脱敏（repo/IP）→ 长度裁剪（防止超长）→ 发送注册表更新。

### 容错
- 缺少字段：回退 `unknown`；不会拒绝事件。
- 未识别事件类型：记录 debug 日志，不报错。
- 序列未注册：注册表会先注册（期望设计中所有 descriptor 已预注册；异常视为编程错误）。

### 性能要点
- 任务高频进度事件不进入桥接（只在结束/错误时更新）。
- 线程缓冲（性能章节）在桥接调用后合并写减少原子争用。
- 标签组装使用预分配小向量（最多 5）。

### 指标一致性自检
- 定时任务读取窗口最新计数与事件历史粗计数对比；若偏差超阈值产出 MetricDrift 事件供告警使用。

### 常见问题排查
| 现象 | 处理 |
|------|------|
| 任务计数缺失 | 检查任务事件是否真正发送；日志搜索 task://state |
| 回退计数过高 | 确认是否存在重试层叠；查看 stage 标签分布 |
| TLS 样本稀疏 | 查看 tlsSampleRate 是否过高 / 事件频率低 |
| refresh 成功率为 0 | 校验 IP 池是否启用 & refresh 事件是否成功字段写入 |

## 7. 聚合与窗口统计机制
### 能力概述
提供 1m / 5m / 1h / 24h 窗口、HDR 分位、原始样本缓存（可选），支撑 UI 趋势、告警窗口表达式与一致性校验。

### 核心结构
| 结构 | 职责 |
|------|------|
| WindowAggregator | 统一管理 counter/histogram 窗口与系列发现 |
| CounterEntry | 分钟/小时环形槽累积计数 |
| HistogramEntry | HDR + 分钟/小时聚合 + raw samples 缓存 |
| HistogramWindowConfig | 控制 raw 样本窗口与容量 |
| WindowRange / Resolution | 标准化窗口请求与槽数量计算 |

### 写入路径
事件 → Registry 更新 → （若启用窗口）Aggregator 根据当前分钟序号写入对应槽；Histogram 同步更新 HDR（分位）与 sum/count。

### 窗口滚动策略
- 无后台 tick；通过 `now()` 计算当前 minute index。
- 提取快照时依据请求窗口回溯槽位并跳过未写入的空槽。

### 分位
- 当前使用 HDR（3 significant figures），上层可配置输出 p50/p90/p95/p99。
- 切换实现（CKMS）仅需替换 HistogramEntry 内部封装，不影响外部 API。

### 原始样本缓存
- 针对延迟型指标可启用（任务时长、握手、IP RTT）。
- 窗口或容量超限：逐条从队首丢弃，保证上界内存。
- 内存压力：性能模块可触发 `disable_raw_samples()` 全量关闭。

### 快照接口要点
| 操作 | 行为 |
|------|------|
| snapshot_counter | 返回窗口点数组 + 总量（total） |
| snapshot_histogram | 返回窗口点、桶、sum、count、分位、raw样本（可选） |
| list_*_series | 枚举激活系列的标签组合与最后更新时间 |

### 错误与降级
- 系列不存在 → None（上层渲染空数据）。
- 非法分位参数预解析报错；避免运行时失败。
- 时间回退（测试模拟）会使旧槽弃用，确保单调性。

### 运维排查指引
| 现象 | 排查步骤 | 可能原因 |
|------|----------|----------|
| 分位长时间为 0 | 确认该指标是否 histogram + 有事件写入 | 未启用或事件未产生 |
| 24h 窗口点稀疏 | 查看进程是否重启导致环形重置 | 进程重启/窗口尚未填满 |
| 原始样本缺失 | 检查 memory pressure 事件与配置 | 已被自动禁用 |

### 性能特性
- O(1) 写入（除 HDR 桶更新），读取按窗口线性聚合；多系列并行 DashMap 分片降低锁冲突。

## 8. 导出接口与访问控制
### 提供的端点
| 路径 | 方法 | 描述 | 返回格式 |
|------|------|------|----------|
| /metrics | GET | Prometheus 文本编码 | text/plain (HELP/TYPE + samples) |
| /metrics/snapshot | GET | 精确名称过滤 + 窗口/分位 JSON | application/json |

### 导出相关内部指标
| 指标 | 标签 | 含义 |
|------|------|------|
| metrics_export_requests_total | status | 导出请求结果分类（ok/unauthorized/rate_limited/error/…） |
| metrics_export_series_total | endpoint | 单次导出返回的序列数量累加（endpoint=metrics|snapshot） |
| metrics_export_rate_limited_total | (none) | 速率限制触发次数 |

### Snapshot 查询参数
| 参数 | 必填 | 示例 | 说明 |
|------|------|------|------|
| names | 否 | names=git_tasks_total,tls_handshake_ms | 逗号分隔；为空=全部（受 maxSeriesPerSnapshot 限制） |
| range | 否 | range=1h | 取值：5m/1h/24h（默认5m） |
| quantiles | 否 | quantiles=p95,p99 | 仅 histogram 生效；未指定则输出默认集合（p50,p95） |

### 访问控制
- 可选 Bearer Token：Header `Authorization: Bearer <token>`；错误返回 401。
- 令牌桶速率限制：容量 = 2 * rateLimitQps；溢出请求返回 429，并增加 `metrics_export_rate_limited_total` 与 `metrics_export_requests_total{status="rate_limited"}`。
- 未启用导出（exportEnabled=false）时：端口不监听；UI 自动 fallback 到 Tauri 内部命令。

### Prometheus 输出规范
- 每个指标包含 `# HELP` 与 `# TYPE` 行。
- Histogram 输出 `_bucket`、`_sum`、`_count`，桶顺序与定义一致，尾随 `+Inf`。
- 分位数不在文本中额外暴露（避免伪造 `_quantile` 语义），仅通过 JSON Snapshot 返回。

### 性能与资源约束
| 指标 | 目标 |
|------|------|
| 300 序列编码耗时 | < 50ms |
| 单次 Snapshot 响应体 | < 512KB（默认场景） |
| 速率限制误差 | <10%（1 分钟统计） |

### 错误处理
| 情形 | 状态码 | 行为 |
|------|--------|------|
| 未授权 | 401 | 不暴露指标存在性；计数 status=unauthorized |
| 方法非 GET | 405 | 记录 status=method_not_allowed |
| 速率超限 | 429 | 计数 rate_limited_total |
| 参数非法 | 400 | 返回错误描述（不含内部细节） |
| 内部错误 | 500 | 记录日志 + status=error |

### 运维排查指引
| 现象 | 排查步骤 | 可能原因 |
|------|----------|----------|
| 大量 401 | 校验调用方 header / 更新 token | 客户端过期 token |
| 大量 429 | 调整 rateLimitQps 或客户端采集间隔 | 抓取频率过高 |
| Snapshot 截断 | 查看 maxSeriesPerSnapshot 配置 | 序列超过上限 |
| Prometheus 缺失分位 | 预期设计（仅 JSON 提供） | 使用 /metrics/snapshot |
| 导出端口无响应 | 确认 exportEnabled 与层级 >= export | 配置层级裁剪 |

### 安全注意事项
- 访问日志对远端地址做 hash 截断，不存储原始 IP。
- Token 轮换：更新配置后旧 token 保留短暂过渡窗口（若实现），期间监控 401 比例。
- 建议在外部反向代理再添加一层 IP 白名单与 TLS。

## 9. 前端面板结构与交互模型
### 目标
无需外部 Grafana 即可本地查看关键 KPI 与趋势，支持 5m / 1h / 24h 范围快速切换与手动刷新。

### 组成
| 组件 | 文件 | 作用 |
|------|------|------|
| 主视图 | `views/ObservabilityView.vue` | Tab 容器 / 时间范围 / 手动刷新 / 禁用提示 |
| 时间范围选择 | `components/observability/TimeRangeSelector.vue` | 统一范围选择，支持双向绑定 |
| KPI 面板 | `components/observability/OverviewPanel.vue` | 成功率、P95、刷新率、告警计数等卡片 |
| 其他子面板 | `GitPanel.vue`, `NetworkPanel.vue`, `IpPoolPanel.vue`, `TlsPanel.vue`, `ProxyPanel.vue`, `AlertsPanel.vue` | 域专属图表与数据透视 |
| 通用图表 | `MetricChart.vue` / Histogram 子组件 | 折线/面积/直方图渲染 + LTTB 降采样 |
| Store | `stores/metrics.ts` | 请求缓存、stale-while-refresh、错误冷却 |

### 数据获取策略
1. 首次进入：拉取默认 Tab (overview) 所需指标集合。
2. 切换范围：构造 (names, range, quantiles) key，命中缓存则回显旧数据并后台刷新。
3. 导出关闭：自动 fallback → `tauri.invoke('metrics_snapshot')`。
4. 错误：展示最近一次成功时间戳 + 重试按钮；连续失败 >=3 次进入 60s 冷却。

### 缓存与过期
| 范围 | TTL | 说明 |
|------|-----|------|
| 5m | 10s | 高频刷新快照 |
| 1h | 30s | 平衡精度和请求数 |
| 24h | 120s | 低频大窗口 |

### 降采样 (LTTB)
- 超过 600 点触发；保留端点与峰值。
- 目标：首屏总 JS 执行 + 绘制 < 800ms。

### 状态与标识
| 状态 | 判定 | UI 表现 |
|------|------|---------|
| loading | 初次或强制刷新未完成 | 骨架/旋转指示 |
| stale | 当前时间 - fetched_at > TTL | 轻微灰标签“Stale” |
| error | 最近刷新失败 | 错误条 + 重试按钮 |
| disabled | config 关闭 uiEnabled | 灰底提示 + 配置指引 |

### 交互要点
- 手动刷新按钮强制 bypass 缓存。
- 告警 Tab 动态显示/隐藏（alertsEnabled=false 时剔除并自动跳转第一个 Tab）。
- 若导出层被关闭，标题区域展示提示“使用本地快照模式”。

### 扩展接口建议（未来）
- Drill-down：点击峰值弹出最近相关事件（现已保留占位逻辑，可对接 `metrics_recent_events`）。
- CSV 导出：通用图表加入“下载数据”按钮。

### 常见问题排查
| 问题 | 排查 | 处理 |
|------|------|------|
| 图表闪烁 | 检查是否重复销毁重建组件 | 使用 keyed diff / 避免全量 remount |
| 请求风暴 | 观察 Network 是否同一 key 多并发 | 确认去抖逻辑 + 缓存 TTL 是否被误改 |
| 告警 Tab 不显示 | 检查配置 alertsEnabled / 层级 >= alerts | 调整 layer 或启用告警 |
| 所有数据 stale | 导出与 fallback 同时失败 | 检查后端端口与 Tauri invoke 权限 |

## 10. 告警规则与 Soak 集成机制
### 设计目标
本地化轻量规则引擎 + Soak 报告阻断判定：发现质量回归（失败率、刷新成功率、延迟劣化）并最小化噪声。

### 规则 DSL 概述
| 语法片段 | 含义 | 示例 |
|----------|------|------|
| metric[p95] | 指定分位 | tls_handshake_ms[p95] > 800 |
| a/b > x | 比值阈值 | git_tasks_total{state=failed}/git_tasks_total > 0.05 |
| {label=value} | 标签过滤 | ip_pool_refresh_total{success=true} |
| window:5m | 使用 5 分钟窗口 | （规则字段） |

### 生命周期状态
| 状态 | 触发条件 | 说明 |
|------|----------|------|
| Firing | 首次越过阈值 | 发送事件 + 增加 alerts_fired_total |
| Active | 去抖窗口内持续超阈值 | 无新增计数，避免噪声 |
| Resolved | 连续评估回到阈值内 | 发送恢复事件 |

### 去抖与重复控制
- 最小重复 firing 间隔：`minRepeatIntervalSecs`。
- 同一 rule 在 Active 状态内不再生成 firing 事件。
- 恢复需连续 N（当前 1）次未超阈值；可调以降低抖动。

### Soak 报告集成
- Soak 结束时读取最近窗口告警集合：若存在未 Resolved 的 critical 则 `ready=false`。
- 报告附加：alerts 数组（id、severity、value、threshold、since_ms），以及 blocking 标记。

### 关键内部结构
| 结构 | 职责 |
|------|------|
| Rule | 解析后的表达式（Left/Right 操作数 + 比较符） |
| RuleState | 当前状态及上次变迁时间、去抖计时 |
| AlertsEngine | 定时评估调度器；聚合批量读取窗口快照 |

### 指标与事件
| 指标 | 说明 |
|------|------|
| alerts_fired_total{severity} | firing 计数（Active 不加） |
| （依赖基础指标） | 规则表达使用的计数/直方图 |
| metric_consistency_drift_total{metric} | 可被规则利用（监控漂移） |

事件：`StrategyEvent::MetricAlert { rule_id, severity, state, value, threshold, comparator }`。

### 运维操作
| 操作 | 步骤 | 验证 |
|------|------|------|
| 修改阈值 | 编辑 rulesPath 文件 → 等待下轮 eval | 观察日志“rules reloaded” |
| 禁用规则 | 设 `enabled=false` | 规则不再出现在评估日志 |
| 临时停告警 | observability.alertsEnabled=false | 相关线程停止，告警 Tab 仍显示历史 |
| 手动评估 | 提供内部命令/测试钩子 (evaluate_alerts_now) | 返回当前告警列表 |

### 常见问题排查
| 现象 | 排查 | 可能原因 |
|------|------|----------|
| 规则不生效 | 查看加载日志/语法错误 | JSON 格式/表达式解析失败 |
| 告警频繁抖动 | 检查阈值贴近噪声波动 | 增大窗口 / 调高阈值 / 延长去抖 |
| 比值分母为 0 | 引擎跳过并记录 debug | 低流量阶段等待或调整规则 |
| Soak 被阻断 | 查看 critical Active 告警 | 指标真实退化或阈值过严 |

### 示例规则片段
```json
[
	{"id":"git_fail_rate","expr":"git_tasks_total{state=failed}/git_tasks_total > 0.05","severity":"warn","window":"5m"},
	{"id":"tls_p95_high","expr":"tls_handshake_ms[p95] > 800","severity":"warn","window":"5m"},
	{"id":"ip_refresh_success_low","expr":"ip_pool_refresh_total{success=true}/ip_pool_refresh_total < 0.85","severity":"critical","window":"5m"}
]
```

### 测试关注点（整合）
- firing → active 去抖；resolved 过渡。
- 分母=0 安全跳过。
- 热更新：新增/移除/禁用立即生效。
- 与 Soak：critical 未恢复阻断 ready。

## 11. 性能优化与资源控制策略
### 目标
在高事件速率（≥5k/s 峰值）下将 CPU 与内存额外开销最小化，并提供自动降级与脱敏保证。

### 关键手段
| 策略 | 位置 | 效果 |
|------|------|------|
| 线程本地缓冲 (TLS buffer) | runtime.rs | 合批减少原子更新与锁竞争 |
| Histogram 分片 | registry.rs | 多核并行写入，读时合并 |
| 采样率 (TLS) | runtime.rs | 降低高频握手延迟写入成本 |
| 标签脱敏 | runtime.rs / event_bridge.rs | 避免敏感信息泄漏，降低唯一值风险 |
| 内存水位监控 | runtime.rs / aggregate.rs | 超限禁用 raw samples 防止 OOM |
| 自动降级层级 | layer.rs | 资源异常时逐级回退功能 |

### 线程缓冲
- 写入流程：事件 → 缓冲追加 → 达阈值或 flush 周期触发批量提交。
- Flush 触发：定时（batchFlushIntervalMs）或手动（强制检查 / 退出）。
- 容量控制：缓冲溢出立即刷写，防止长尾延迟。

### 分片 (Sharding)
- 根据 CPU 核数初始化 N 份 histogram shard；写操作仅命中当前线程 shard。
- 读取（导出/快照）时合并所有 shard 计数。

### 采样率
- `tlsSampleRate = 5` ⇒ 仅 20% 握手被记录；频率降低后可动态调低采样以恢复精度。
- 调整：更新配置 or 运行时 API 触发即时生效。

### 脱敏策略
| 对象 | 模式 | 结果示例 |
|------|------|----------|
| 仓库名 | hash 前 8 位 | `a1b2c3d4` |
| IP (mask) | a.b.*.* | 203.98.*.* |
| IP (classify) | public/private/loopback | public |
| IP (full) | 全隐藏 | `***` |

Debug 模式（本地开发）可临时展示原值；生产默认关闭。

### 内存水位 & 降级
- 估算总占用（结构 * 固定大小 + raw buffer 长度）；> `maxMemoryBytes`：
	1. 发 memory pressure 事件 + 自增 `metric_memory_pressure_total`；
	2. `disable_raw_samples()`；
	3. 触发层级自动降级（若开启 autoDowngrade）。

### 性能指标建议采集
| 指标 | 含义 | 运维动作 |
|------|------|----------|
| metric_memory_pressure_total | 发生次数 | 频繁增加 → 提升阈值或减少 raw 样本 |
| metrics_export_rate_limited_total | 导出被限流次数 | 调整 rateLimitQps / 抓取间隔 |
| alerts_fired_total{severity} | 告警频率 | 频繁告警评估阈值合理性 |
| observability_layer | 当前层级 | 长期 < 期望层说明资源不足 |

### 故障征兆 & 响应
| 征兆 | 可能原因 | 处理 |
|------|----------|------|
| UI 数据大面积 stale | Flush 阻塞 / 导出超时 | 检查线程池、降低采样率 |
| 分位波动异常大 | 采样率过高或事件稀疏 | 调整采样或合并窗口观察 |
| 内存飙升后突然回落 | Raw samples 被禁用 | 评估是否需要调大阈值 |

### 调优顺序建议
1. 确认热点指标（事件量排行）。
2. 调整采样率（仅延迟类）。
3. 缩减 raw samples（降低窗口或容量）。
4. 启用/扩大分片数（如允许）。
5. 上调内存阈值（硬件允许情况下）。

## 12. 灰度层级与回退策略
### 层级定义
| 数值 | 名称 | 启用能力 | 典型用途 |
|------|------|----------|----------|
| 0 | basic | 基础指标/桥接 | 初始接入，验证无回归 |
| 1 | aggregate | + 窗口聚合/分位 | 需要趋势/分位分析 |
| 2 | export | + HTTP 导出 | 对接外部 Prometheus |
| 3 | ui | + 前端面板 | 本地可视化需求 |
| 4 | alerts | + 告警引擎 | 质量门控 / Soak 准入 |
| 5 | optimize | + 性能硬化 | 高频 / 压力场景 |

### 推导规则
配置 `observability.layer` 代表期望最大层；单独开关关闭会向下裁剪。例如：layer=optimize 但 exportEnabled=false ⇒ 实际层级 ≤ aggregate。

### 状态机
| 触发 | 行为 |
|------|------|
| set_layer(target) | Clamp 到允许层并写 observability_layer Gauge |
| auto_downgrade(reason) | 若满足最小驻留 + 冷却窗口，层级 -1 并发事件 |
| 资源恢复（人工） | 手动 set_layer 逐级升回 |

### 自动降级触发（示例）
| 条件 | 说明 | 动作 |
|------|------|------|
| 内存 > maxMemoryBytes | 原始样本占用超限 | 禁用 raw + 降级一层 |
| 导出 5xx >5% | 导出接口不稳定 | 降级至 aggregate 观察 |
| CPU 增量 >5% | 性能回退 | 降级一层并记录原因 |

### 事件
`StrategyEvent::ObservabilityLayerChanged { from, to, initiator, reason }`
用于审计层级演进；initiator=auto-downgrade / manual。

### 运维操作
| 目标 | 操作 | 验证 |
|------|------|------|
| 手动紧急回退 | set_layer(basic) | Gauge=0，/metrics 不再可用（若原层≥export） |
| 升级开启 UI | set_layer(ui) 且 uiEnabled=true | `DeveloperToolsView` 中出现 Observability 卡片 |
| 重新启用告警 | 修复资源→set_layer(alerts) | 告警评估日志恢复 |
| 解除降级锁 | 超过冷却后 set_layer(original) | 事件记录恢复原因 |

### 排查场景
| 现象 | 排查 | 处理 |
|------|------|------|
| 层级无法升回 optimize | 查看最近 layer change 事件 reason | 资源仍未恢复 / 冷却期内 |
| 自动频繁 oscillation | 检查驻留/冷却参数是否过低 | 提高 minResidency / cooldown |
| Gauge 与实际功能不符 | 是否手动禁用了单独开关 | 修正配置或重新初始化 |

### 最佳实践
1. 初次上线：basic → (24h) → aggregate → export。
2. 告警稳定后再开启 alerts；高频事件场景再升 optimize。
3. 频繁 auto_downgrade 时优先确认采样率与 raw 样本配置。

## 13. 测试矩阵与关键用例
### 覆盖分层
| 维度 | 关键用例 | 目的 |
|------|----------|------|
| 指标注册 | 重复注册/缺失标签 | 保证幂等与错误报告 |
| 事件桥接 | 任务三态去重/回退链/IP 刷新/AutoDisable/告警 firing→active | 映射完整与去重正确 |
| 聚合窗口 | 1m/5m/1h/24h 点数 & 分位精度 | 时间滚动正确、分位可用 |
| 原始样本 | 窗口裁剪/容量限制/禁用 | 内存受控与开关行为 |
| 导出 | /metrics HELP/TYPE 正确；Snapshot 过滤/限流/Token | 协议兼容与安全控制 |
| 告警 | firing→active→resolved、比值分母=0、热更新 | 状态机与容错 |
| Soak 集成 | critical 阻断 ready | 质量门控有效 |
| 性能 | 线程缓冲 flush 一致性 / 分片合并 / 采样生效 | 高并发稳定与降开销 |
| 内存压力 | 超阈值禁用 raw samples | 自动降级触发 |
| 层级状态机 | 升级/自动降级/冷却 | 灰度安全 |
| 前端缓存 | TTL + stale-while-refresh / Fallback Tauri | 降低请求与容错 |
| 前端交互 | Tab 切换/范围切换/手动刷新/告警 Tab 隐藏 | UI 行为正确 |

### 代表性测试点示例
| 测试描述 | 断言 |
|----------|------|
| 任务完成后重复发送完成事件 | git_tasks_total 仅 +1 |
| 手动推进时间 2 小时聚合快照 | 1h 窗口点数=60，无重复累计 |
| Histogram 非法 quantile=1.5 | 返回参数错误，不 panic |
| Snapshot names 过滤 | 仅包含指定指标，序列数≤请求列表 |
| 速率限制触发 | 返回 429 且 rate_limited_total +1 |
| 告警比值分母=0 | 不进入 firing，日志含 skip 说明 |
| 告警规则移除后评估 | 不再出现该 rule_id 事件 |
| 内存阈值压测 | raw_samples 被清空 + metric_memory_pressure_total +1 |
| set_layer(alerts) 后关闭 exportEnabled | 层级 clamp 到 aggregate；Gauge 更新 |
| UI 范围切换 5m→1h | 新缓存 key 生效，旧数据即时显示后刷新 |

### 测试自动化建议
- Rust：保持 `cargo test -q -- --test-threads=1 metrics` 串行运行聚合相关测试，避免时间提供器干扰。
- 前端：Vitest + Testing Library；为面板交互与 store 缓存提供模拟 HTTP/tauri 双通道。
- 长期：可添加 Playwright 端到端脚本（启动后等待生成若干指标，再验证前端图表）。

### 基准/性能（可选）
| 基准项 | 指标 | 预期 |
|--------|------|------|
| 2k/s 事件（无窗口） | CPU 增量 | <2% |
| 5k/s 事件（窗口+分位） | CPU 增量 | <5% |
| 300 序列导出 | /metrics 延迟 | <50ms |
| 10k 点降采样 | LTTB 耗时 | <20ms |

## 14. 运维操作与故障诊断
### 常用操作
| 目标 | 操作 | 影响 |
|------|------|------|
| 暂停全部可观测性 | 设置 enabled=false | 停止注册/导出/告警/UI，仅事件继续产出 |
| 仅关闭面板 | uiEnabled=false | 不影响导出、窗口、告警 |
| 调整 TLS 采样 | 修改 tlsSampleRate / 运行时 API | 立即影响后续握手写入 |
| 临时禁用告警 | alertsEnabled=false | 保留历史；不再评估新规则 |
| 修改告警阈值 | 编辑 rulesPath 文件 | 下轮评估生效（默认 30s 内） |
| 降低内存占用 | 减小 raw 样本窗口/容量；或手动禁用 | 分位仍可用，失去精细样本 |
| 重建导出服务 | exportEnabled=false → true | 重启 HTTP 监听；Prometheus 需等待下一轮抓取 |
| 强制层级 | set_layer(ui/alerts/...) | 功能即时启停；记录层级事件 |

### 关键日志分类（建议 grep 关键字）
| 关键字 | 含义 |
|--------|------|
| metrics_export | 导出请求、状态码、序列数 |
| metric_alert | 告警状态变迁 (firing/active/resolved) |
| metric_memory_pressure | 内存压力触发 raw 样本降级 |
| observability_layer | 层级变更记录 |
| metric_drift | 指标与事件偏差自检 |

### 快速健康检查清单
1. /metrics 可访问且包含 HELP/TYPE。
2. Snapshot 返回的核心指标（git_tasks_total, tls_handshake_ms）非空。
3. observability_layer Gauge 等于期望层级。
4. 最近 10 分钟无连续 metric_memory_pressure 日志。
5. 告警面板无“抖动”频繁 firing/resolved 翻转。

### 故障分类与诊断
| 症状 | 诊断步骤 | 可能根因 | 处理建议 |
|------|----------|----------|----------|
| Prometheus 抓取超时 | curl /metrics; 检查 CPU 占用 | 序列过多/编码阻塞 | 过滤 names 或提升资源 |
| 告警全部失效 | 查看 alertsEnabled 与层级 | 层级 < alerts 或配置关闭 | set_layer(alerts) 启用 |
| KPI 与外部监控偏差 | 对比事件计数 & 指标 | 采样率/丢事件/时间漂移 | 调整采样或修复事件源 |
| UI 长期 stale | 查看导出端口与 fallback 日志 | 导出停用或网络失败 | 恢复导出或修复调用链 |
| 分位曲线锯齿 | 事件量低 + 采样率高 | 统计样本不足 | 降低采样率或扩大窗口 |

### 观察指标组合建议
| 目的 | 指标组合 | 说明 |
|------|----------|------|
| Git 稳定性 | git_tasks_total{state} + git_retry_total | 失败率、重试放大 |
| 网络回退 | http_strategy_fallback_total{stage} | 哪个阶段导致降级 |
| IP 池健康 | ip_pool_refresh_total{success} + ip_pool_latency_ms{source} | 刷新成功率 & RTT 变化 |
| TLS 性能 | tls_handshake_ms 分位 + 采样率 | 长尾是否扩张 |
| 告警噪声 | alerts_fired_total{severity} | 频繁告警需调阈值 |
| 资源压力 | metric_memory_pressure_total + observability_layer | 是否频繁降级 |

### 变更前后对比流程（推荐）
1. 记录基础指标快照（Snapshot API）。
2. 应用配置 / 代码变更。
3. 10 分钟后再次抓取快照并计算差异（成功率 / P95 / 刷新率 / 回退率）。
4. 若差异 > 预期阈值（例如 ±10%）且无业务解释，回滚变更并开缺陷。

### 安全审计要点
- 确认无明文 IP/私有仓库字符串（抽样标签值）。
- Token 变更后验证旧 Token 失效窗口是否符合策略（若启用）。
- 检查导出访问日志来源是否符合预期采集器 IP 列表。

## 15. 后续优化建议
### 指标与聚合
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| CKMS 分位可选 | 替换/并存 HDR 与 CKMS | 更低内存适应极低基数场景 | 中 |
| 稀疏直方图 | 仅存非空桶 | 降低长尾场景内存 | 低 |
| 动态窗口 | 自适应滑动窗口粒度 | 提升低流量分位稳定性 | 中 |

### 导出与安全
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| HTTPS 支持 | 本地自签或可配置证书 | 防窃听/中间人 | 中 |
| Token 滚动 | 双 Token 过渡窗口 | 无缝更新 & 降低泄露风险 | 中 |
| IP 白名单 | 限制抓取来源 | 减少暴力枚举 | 低 |

### 告警系统
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| Rule 分组 | 批量共享窗口读 | 降低评估成本 | 中 |
| 抑制链 (suppress) | 高级告警抑制低级 | 降噪 | 中 |
| 复合表达式 | AND/OR 组合 | 复杂场景表达力 | 低 |
| 静默窗口 | 维护期静默策略 | 避免无意义告警 | 中 |

### 前端体验
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| 事件 Drill-down | 图表点展开相关最近事件 | 根因定位加速 | 中 |
| 导出 CSV/JSON | 一键下载当前窗口数据 | 数据分享与分析 | 低 |
| 轻量缓存预热 | 进入前后台 prefetch 关键指标 | 降低首屏延迟 | 低 |

### 性能与资源
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| 自适应采样 | 基于事件速率动态调整 | 保持精度与成本平衡 | 高 |
| Shard 自动数 | 根据并发实时调节分片 | 减少空闲 shard 开销 | 中 |
| 分位近似缓存 | 热指标分位结果缓存多读复用 | 减少重复合并 | 中 |

### 生态与扩展
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| Trace 集成 | 与调用链（后续阶段）对接 | 统一可观测闭环 | 高 |
| 事件统一总线 | 标准化 StrategyEvent -> Async Stream | 模块解耦/测试易用 | 中 |
| 外部插件点 | 指标导出插件 (In-Memory Adapter) | 第三方扩展 | 低 |

### 运维工具化
| 方向 | 描述 | 价值 | 优先级 |
|------|------|------|--------|
| 诊断 CLI | 一键输出快照+层级+最近事件 | 快速现场诊断 | 高 |
| 自动回归检测 | 基于历史基线计算异常 | 早期预警 | 中 |
| 指标预算仪表 | 计算系列数/内存占用趋势 | 防止隐性膨胀 | 中 |

> 规划原则：优先保障精度-成本动态平衡（自适应采样、诊断 CLI）再提升表达力（Rule 分组、Trace）。

### 优化落地建议（首批）
1. 实现自适应采样：设定目标 P95 统计误差阈值，依据最近窗口事件量调节采样率。
2. 提供诊断 CLI：封装 snapshot + layer + recent events 输出成结构化 JSON。
3. Token 双生效窗口：新增 `nextAuthToken` 与 `tokenRotateDeadline`，到期后旧 token 失效。
4. Rule 分组读取：同窗口指标批量一次性 snapshot，减少 N*网络/合并开销。
5. 指标预算：周期输出 `metrics_memory_bytes` 估值 + 系列数量，便于容量规划。

## 16. 快速校验命令
### 后端单元 / 集成测试
Rust 测试（串行保证时间控制一致）
```
cargo test -q -- --test-threads=1 metrics
```

### 前端指标面板测试
```
pnpm -s test --filter web --run metrics
```

### 手动拉取 Prometheus 文本
```
curl -s http://127.0.0.1:PORT/metrics | head -40
```
检查包含 `# HELP git_tasks_total` 与 `# TYPE tls_handshake_ms histogram`。

### 拉取 Snapshot（指定指标+窗口+分位）
```
curl -s "http://127.0.0.1:PORT/metrics/snapshot?names=git_tasks_total,tls_handshake_ms&range=1h&quantiles=p95,p99"
```

### 验证 Token 访问控制
```
curl -s -H "Authorization: Bearer WRONG" -o /dev/null -w "%{http_code}\n" http://127.0.0.1:PORT/metrics
```
期望返回 401。再用正确 Token 验证 200。

### 触发内存压力（示例思路）
（构造大量 histogram 写入或调低 maxMemoryBytes，观察 `metric_memory_pressure_total` 增加。）

### 模拟告警 firing
将规则阈值临时下调（如 fail rate > 0.0000001）等待评估周期：
```
tail -f logs/app.log | findstr metric_alert
```

### 层级降级/升级验证
```
# 降级
<内部命令> set_layer basic
# 升级
<内部命令> set_layer optimize
```
观察 Gauge 与日志事件 `observability_layer`。

### 快速一致性自检
```
curl -s http://127.0.0.1:PORT/metrics/snapshot?names=git_tasks_total | jq '.series[0].total'
```
对比本地任务完成事件粗计数（可用调试日志或内部计数器）。

### 导出速率限制测试
```
for /L %i in (1,1,10) do curl -s -o NUL http://127.0.0.1:PORT/metrics
```
当超过 QPS 阈值后出现 429；同时 `metrics_export_rate_limited_total` 增长。

### 快速回滚验证（变更前后）
1. 运行一次快照保存 baseline。
2. 应用配置变更。
3. 10 分钟后再抓取对比，异常立即回退。

> 将上述命令整理为脚本可形成 smoke 套件集成到 CI。
