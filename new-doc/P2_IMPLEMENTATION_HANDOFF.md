# P2 实现说明（交接稿）— 本地操作扩展 + Shallow/Partial + 策略覆盖 + 护栏与汇总

版本：v1.0（2025-09-21） 维护者：Core Team

---

## 1. 范围与目标

- 范围：在 MP0/MP1 已完成 clone/fetch/push + Retry v1 + 方式A 子传输 + 事件分类基线之上，新增本地常用 Git 操作（commit/branch/checkout/tag/remote），交付 shallow/partial 克隆（depth/filter + 决策/回退/能力检测）、任务级策略覆盖（HTTP/TLS/Retry）、护栏（ignored/conflict 规范化）、策略汇总 summary 与事件 gating。
- 不改变：任务生命周期、事件通道（`task://state|progress|error`）、既有 push/cancel/retry 语义、前端 Store 结构（新增字段可选）。
- 可回退：所有新增功能均提供粒度化禁用路径（移除 TaskKind、去除 apply_*、关闭 summary/gating、移除 capability provider）。

## 2. 与 MP0 / MP1 差异概览

| 类别 | MP0 | MP1 | P2 新增/变化 |
|------|-----|-----|--------------|
| 本地操作 | clone/fetch | + push | + commit / branch / checkout / tag / remote 管理 |
| 克隆模式 | 完整 | 完整 | + depth / filter 解析、fallback、capability gating、回退事件 |
| 策略覆盖 | 无 | Retry v1（全局） | + per-task http/retry/tls 覆盖（应用 + 事件 + 汇总） |
| 事件扩展 | 基础 state/progress/error | push phased, retry 分类 | + http/retry/tls applied, conflict, ignored, partial_filter_fallback, strategy_override_summary |
| 护栏 | 基础分类 | N/A | unknown 字段 ignored、互斥规范化 conflict |
| 配置 gating | 无 | 部分（方式A白名单） | + FWC_PARTIAL_FILTER_SUPPORTED, FWC_STRATEGY_APPLIED_EVENTS |
| 回退策略 | 替换实现 | push 方式A灰度 | 细粒度组件级移除（见 §11） |

## 3. 命令与任务模型扩展

新增 TaskKind：`GitCommit` `GitBranch` `GitCheckout` `GitTag` `GitRemoteAdd` `GitRemoteSet` `GitRemoteRemove`。

策略覆盖适用任务：`GitClone|GitFetch|GitPush` 入参新增可选 `strategyOverride`：
```jsonc
{
  "strategyOverride": {
    "http": { "followRedirects": false, "maxRedirects": 3 },
    "retry": { "max": 6, "baseMs": 400, "factor": 1.5, "jitter": true },
    "tls": { "insecureSkipVerify": false, "skipSanWhitelist": false }
  },
  "depth": 1,
  "filter": "blob:none"
}
```
字段可缺省；大小写兼容 snake/camel；未知字段不失败（后续触发 ignored 事件）。

## 4. 事件契约增量

全部复用 `task://error` 通道（信息型与失败型统一）：
- `http_strategy_override_applied`
- `retry_strategy_override_applied`
- `tls_strategy_override_applied`
- `strategy_override_conflict`
- `strategy_override_ignored_fields`
- `partial_filter_fallback`
- `strategy_override_summary`

顺序（单任务）：applied*(0..3) → conflict*(0..2) → ignored?(0..1) → partial_filter_fallback?(≤1) → summary?(≤1)。
`strategy_override_summary` 聚合最终策略值与 `appliedCodes`；独立 applied 事件可被 gating 关闭（`FWC_STRATEGY_APPLIED_EVENTS=0`）。

## 5. 数据模型与解析

`strategy_override.rs`：解析 `strategyOverride`，返回结构：
```
ParsedOverride { http: Option<HttpOvr>, retry: Option<RetryOvr>, tls: Option<TlsOvr>, ignored_top: Vec<String>, ignored_nested: Vec<String> }
```
数值/范围校验在解析阶段完成；出现错误直接 Protocol 失败，不进入应用阶段。空对象 `{}` 与缺省等价（不触发事件）。

Depth/Filter：
- 决策枚举 `DepthFilterDecision`：`Full|DepthOnly|FilterOnly|DepthAndFilter|FallbackShallow|FallbackFull`。
- 能力 provider（含 env gating 与缓存）决定是否允许进入 FilterOnly / DepthAndFilter。

## 6. 后端实现要点

- 统一入口：`core/tasks/registry.rs` 扩展 spawn_* 函数；按固定顺序在 clone/fetch/push 任务中执行：解析 → HTTP → TLS → Retry → 护栏冲突/ignored → partial fallback → summary。
- 覆盖函数：`apply_http_override` / `apply_retry_override` / `apply_tls_override` 返回 (值, changed[, conflict])；changed 为 true 时条件发事件并向 `appliedCodes` 汇总。
- 护栏：冲突规范化在覆盖后立即执行，可能改变 changed 判定结果（规范化回到默认值时仅发 conflict）。
- Partial fallback：根据决策与 capability provider 结果产生单一信息事件，保证一次性。
- Summary：在全部差异与 fallback 决策完成后发射聚合事件；即使 applied 事件被关闭，summary 仍列出差异。

## 7. 本地 Git 操作实现摘要

| 操作 | 关键点 | 错误分类要点 | 回退 |
|------|--------|--------------|------|
| commit | 空提交判定 + allowEmpty | 空消息/空提交/作者缺失→Protocol | 移除 TaskKind |
| branch | 名称双阶段校验 / force 覆盖 | 已存在未 force / 无提交 create→Protocol | 移除 TaskKind |
| checkout | create+checkout 原子 | 不存在未 create→Protocol | 移除 TaskKind |
| tag | 轻量+附注 / 消息规范化 / force OID 不变 | 缺消息/重复非 force→Protocol | 移除 TaskKind |
| remote add/set/remove | URL 原始空白校验 / 幂等 set | add 重复 / set/remove 不存在→Protocol | 移除 TaskKind |

取消：所有写引用/创建对象临界点前检查 token 保证无半成品。

## 8. 前端适配

- API：透传可选 `depth` `filter` `strategyOverride`；旧 fetch 字符串签名保持兼容。
- Store：扩展错误事件记录 code，不降低已记录 `retriedTimes`（信息事件忽略缺失字段）。
- UI：可显示策略信息型事件与 fallback；事件排序基于到达时间 + 规范序列。

## 9. 配置与环境变量

| 变量 | 取值 | 作用 | 回退 |
|------|------|------|------|
| FWC_PARTIAL_FILTER_SUPPORTED | 0/1 | 是否允许尝试 filter 能力 | 设 0 强制 fallback 路径 |
| FWC_STRATEGY_APPLIED_EVENTS | 0/1 | 是否发送独立 applied 事件 | 设 0 仅 summary |

全局默认配置（AppConfig）未在 P2 被持久化修改；策略覆盖仅作用单任务内存副本。

## 10. 测试策略

- 单元：解析/覆盖/冲突规范化/决策矩阵/能力缓存。
- 集成：commit/branch/checkout/tag/remote happy+error+取消；depth 多次 deepen，filter fallback，能力 gating，策略覆盖组合 (http-only / retry-only / tls-only / mixed)，事件顺序与幂等。
- 组合：并行任务差异化策略与 fallback 确认互不污染；冲突+ignored 混合出现计数准确。
- 前端：事件代码存储、顺序、gating=0 模式、retriedTimes 保留、旧 fetch 签名兼容、参数排列（override+depth+filter+credentials）。

## 11. 回退矩阵

| 功能 | 操作 | 残留影响 |
|------|------|----------|
| Commit/Branch/Tag/Remote | 移除 TaskKind/命令 | 其它任务不受影响 |
| depth/filter | 移除解析与决策调用 | 仍可完整克隆 |
| partial capability | 移除 provider 调用 | 统一 fallback 逻辑（无探测） |
| partial fallback 事件 | 移除 emit 分支 | 决策仍有效（静默） |
| http/retry/tls applied 事件 | gating=0 或删 emit | summary 仍列差异 |
| conflict/ignored | 删 emit 分支 | 仍执行或停用规范化（视是否删除规则） |
| 规范化规则 | 删除规则 | 可能传播矛盾组合 |
| summary | 移除 emit_strategy_summary | 依赖独立 applied 事件 |
| gating 环境变量 | 不设置 | 采用默认 (applied on / capability off if var=0) |

## 12. 安全与隐私

- 事件与日志不包含敏感凭证（URL/令牌脱敏策略沿用 MP1）。
- TLS 覆盖不允许修改 san_whitelist 列表，降低误配置风险。
- 冲突与 ignored 事件仅暴露字段名，避免回显整段策略 JSON。

## 13. 性能与并发

- 新增本地操作均为 O(files) 或 O(引用) 短操作，单进度事件足够。
- 覆盖与护栏解析开销常数级；并发任务各自维护覆盖副本无锁共享（只读全局配置）。
- capability 探测结果按远端 URL 缓存，避免重复 I/O；缓存命中 O(1)。

## 14. 已知限制

- 未实现标签删除、分支删除、upstream 追踪、任意提交检出。
- depth+filter 尚未真正裁剪对象内容（filter 阶段为结构化占用与决策演练）。
- TLS/HTTP 策略覆盖尚未下沉到自定义传输层实际行为改变（未来接入后复用同一事件语义）。
- retry 覆盖在不可重试 Internal 场景不会触发 attempt 序列（需 i18n 分类增强）。

## 15. 风险与缓解

| 风险 | 说明 | 缓解 |
|------|------|------|
| 事件序列竞态 | 并行策略事件与 summary 顺序扰动 | 发送顺序固定 + 测试锁定 |
| 规范化遗漏 | 新增策略字段未加入冲突规则 | 中央规则表 + 单测覆盖新增字段前置 |
| 能力探测误判 | 模拟钩子与真实远端差异 | 可替换 provider 实现 + 回退到静态路径 |
| i18n 分类不足 | 中文/本地化错误分类为 Internal | 关键字扩展（P3+） |
| 过多信息事件 | 覆盖字段增多放大事件量 | summary 聚合 + gating 抑制 applied |

## 16. 验收标准汇总

- 新 TaskKind 全部成功/错误/取消测试通过。
- depth/filter 决策与 fallback 事件矩阵测试通过（含 gating 关闭与并行）。
- http/retry/tls 覆盖组合与 skipped paths 幂等（事件最多一次）。
- conflict/ignored 规则与计数准确。
- summary 在 applied 关闭时仍列差异集合。
- 前端兼容旧 API，新增参数全部可选无破坏。

## 17. 示例（典型 Clone 事件序列）

```
state pending
state running
progress phase=Starting percent=0
error code=http_strategy_override_applied category=Protocol message="http override applied: follow=false max=3"
error code=strategy_override_conflict category=Protocol message="http conflict: follow=false => force maxRedirects=0 (was 3)"
error code=strategy_override_ignored_fields category=Protocol message="strategy override ignored unknown fields: top=[x] sections=[http.zz]"
error code=partial_filter_fallback category=Protocol message="partial filter fallback: requestedDepth=1 requestedFilter=blob:none decision=FallbackShallow"
error code=strategy_override_summary category=Protocol message="{...appliedCodes:[http_strategy_override_applied]}"
progress phase=Receiving percent=78 objects=120 bytes=52340
progress phase=Checkout percent=95
progress phase=Completed percent=100
state completed
```

## 18. 后续演进（P3+ 预留）

- 真实部分克隆对象裁剪与增量拉取。
- 策略 summary 拓展 metrics 导出（覆盖频次、冲突率、fallback 率）。
- TLS / HTTP 策略下沉自定义传输层（followRedirects, maxRedirects, insecureSkipVerify）。
- 策略差异缓存与 UI 聚合优化（抑制重复任务同配置信息事件）。

---

（完）
