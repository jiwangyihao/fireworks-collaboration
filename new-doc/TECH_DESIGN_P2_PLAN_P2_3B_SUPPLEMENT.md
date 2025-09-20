# TECH_DESIGN_P2_PLAN 补充：P2.3b（二阶段）HTTP 策略覆盖事件与 changed flag

> 本补充文件与主文档 `TECH_DESIGN_P2_PLAN.md` 中的 “P2.3b 实现说明（已完成）” 章节配套，聚焦第二阶段新增的事件、幂等与测试强化。若后续将 TLS / Retry 覆盖一并应用，可在此文件继续增补相似章节，主文档保持概要。

## 1. 目标概述
- 在 clone / fetch / push 任务解析 `strategyOverride` 后应用 HTTP 子集（followRedirects, maxRedirects）。
- 新增 changed 判定：仅当有效值与全局不同才算“应用”。
- 通过结构化 TaskErrorEvent（code=`http_strategy_override_applied`）发射一次非致命提示，提升前端可观测性而不引入新事件主题。
- 保持零破坏：未改变底层 HTTP 行为（后续阶段再实际接入网络层）。

## 2. 合并与事件逻辑
```rust
// registry.rs
let (f, m, changed) = apply_http_override("GitClone", &id, &global_cfg, opts.strategy_override.as_ref().and_then(|s| s.http.as_ref()));
if changed {
    if let Some(app_ref)=&app {
        let evt = TaskErrorEvent { task_id:id, kind:"GitClone".into(), category:"Protocol".into(), code:Some("http_strategy_override_applied".into()), message: format!("http override applied: follow={} max={}", f, m), retried_times:None };
        this.emit_error(app_ref,&evt);
    }
}
```

规则回顾：
| 步骤 | 说明 |
|------|------|
| 基线 | 复制 `AppConfig::default().http` 值（后续换成运行时配置） |
| 覆盖 | 若提供 followRedirects / maxRedirects 且不同则替换并标记 changed=true；maxRedirects clamp ≤ 20 |
| 发射 | changed=true 时，仅一次事件（spawn 时） |
| 日志 | tracing target="strategy" 同步 info 行（含 follow/max） |

## 3. 事件结构
```json
{
  "taskId": "<uuid>",
  "kind": "GitClone|GitFetch|GitPush",
  "category": "Protocol",
  "code": "http_strategy_override_applied",
  "message": "http override applied: follow=<bool> max=<u8>",
  "retriedTimes": null
}
```
选择原因：
- 复用错误通道（前端已统一消费）；
- 与 partial_filter_fallback 一致，形成“协议提示”类别；
- 避免新增主题造成前端监听扩散。

## 4. 测试矩阵（新增）
| 文件 | 断言要点 |
|------|----------|
| `git_http_override_event.rs` | 覆盖值改变 → 存在事件（包含 code 与 taskId） |
| `git_http_override_no_event.rs` | 覆盖值与默认相同 → 不存在事件 |
| `git_http_override_idempotent.rs` | 单任务事件次数恰为 1 |
| registry 内部单测 | clamp / changed 判定逻辑 |

实现细节：
- 测试需传入 `Some(AppHandle)`（非 tauri feature 下为空占位 struct）才能捕获事件。
- 使用 `peek_captured_events()` 收集全部事件再过滤 code。

## 5. 幂等与回退
| 场景 | 行为 |
|------|------|
| 重复调用 apply（当前不会发生） | 若未来出现，需在上层调用点防抖；现阶段单次调用保证幂等 |
| 回退需求 | 删除 `if changed { emit ... }` 分支和 3 个测试文件即可；合并逻辑保留 |

## 6. 风险评估
- 只读 → 局部变量；无共享状态写入；
- 事件数量受限（最多 1/任务），前端无性能压力；
- 若未来引入真实 redirect 行为，需确认 follow=false 与 max>0 的组合语义（可能追加提示）。

## 7. 后续扩展建议
1. 注入真实运行时配置（替换默认值）并支持热加载。
2. 扩展 TLS / Retry 应用：沿用 `*_strategy_override_applied` code 规范。
3. 前端 UI 增加“提示”标签区分致命 vs 信息事件。
4. 统一策略事件聚合：在任务详情面板聚合展示一次性策略差异摘要。

## 8. Changelog 建议条目
```
Added: per-task HTTP strategy override application + informative event `http_strategy_override_applied` (emitted once when override changes followRedirects/maxRedirects).
```

## 9. 现状结论
- 覆盖逻辑与事件已落地并通过 3 个集成 + 1 组单元测试；
- 后端、前端全部测试通过；
- 回退与后续拓展路径清晰。

---
