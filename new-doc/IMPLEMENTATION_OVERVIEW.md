# Fireworks Collaboration 实现总览（MP0 -> P8 & 测试重构）

> 目的：以统一视角梳理 MP0、MP1、P2、P3、P4、P5、P6 七个阶段以及测试重构后的现状，面向后续演进的研发、运维与质量成员，提供完整的实现细节、配置指引、事件契约、测试矩阵与回退策略。
>
> 版本：v1.8（2025-10-11） 维护者：Core Team

---

### 2025-10-11 增量更新摘要（v1.8）

本次增量聚焦 Fake SNI 场景下的 TLS 校验一致性，主要更新如下：

1. `RealHostCertVerifier` 现始终使用真实域名执行证书链校验，即使启用 Fake SNI 也不会回退到伪造域名，相关的 `tls.realHostVerifyEnabled` 配置开关已删除以避免误关能力。
2. 新增 `override_host_enforced_during_fake_sni` 集成测试（位于 `src-tauri/tests/events/events_structure_and_contract.rs`），通过记录式 verifier 证明实际传入的 ServerName 为真实域；同时回归 `cargo test --test events` 与完整 `cargo test` 均通过。
3. 团队模板与前端配置类型已同步移除遗留的 `tls.realHostVerifyEnabled` 字段，避免误以为仍可热切换 Real Host 校验。
4. 目前 SPKI pin 校验仅在通过 `create_client_config_with_expected_name` 构造的 Fake SNI 路径生效（即安装了 `RealHostCertVerifier` 的请求），真实 SNI 场景暂未挂载该校验器，后续需统一处理。

### 2025-10-09 增量更新摘要（v1.7）

本次版本围绕 IP 池与预热链路做了一轮集中迭代，重点如下：

1. 运行期默认启用 IP 池，并引入全新的 DNS 运行时配置：支持系统解析器开关、自定义 DoH/DoT/UDP 解析器、内置预设目录与启用列表；团队模板合并逻辑同步下发 `enabled` 与 `dns` 字段，确保分发环境与模板保持一致。
2. 新增 Tauri `ip_pool_*` 命令面向配置管理、快照查询、预热刷新与候选调试；前端提供 `src/api/ip-pool.ts`、全新 `IpPoolLab.vue` 实验室视图以及导航入口，可实时编辑运行期/文件配置、管理 DNS 解析器、禁用内置预热域、查看候选缓存。
3. 预热管线重写：缓存快照新增预热目标覆盖率统计，Check View 新增“IP 池预热”步骤并复用 `waitForIpPoolWarmup` 辅助检测后台预热进度，失败/跳过场景给出清晰提示；可通过 `ip_pool_start_preheater` 显式拉起一次预热并观测状态。
4. 测试与文档同步：Rust 侧更新 config/commands/git/adaptive_tls/transport 多处用例适配新默认值，并新增 `tests/commands/ip_pool_commands.rs` 覆盖完整命令路径；前端补充 `ip-pool.api.test.ts`、`utils/__tests__/check-preheat.test.ts` 验证 API/预热逻辑；文档新增 DNS 运行时、禁用内置预热域、命令列表与 UI 说明，现有回退/Smoke 清单全部检视过默认启用场景。

### 2025-10-07 增量更新摘要（v1.6）

本次版本未引入新的里程碑功能，聚焦以下维护性与稳定性改进：

1. 前端样式体系（Tailwind v4 全量迁移）
  - 移除所有组件级 `<style scoped>` 中的 `@apply`，统一改为模板内联 utility classes；
  - 新增全局样式入口 `src/style.css`（集中 `@import "tailwindcss"`、`@plugin "daisyui"/@tailwindcss/typography` 以及 dark variant 与通用 utilities），`App.vue` 中原先的 @plugin/@custom-variant/@utility 声明已删除；
  - 仅保留极少量必要的自定义 CSS（例如 `MetricChart.vue` 中 SVG 轴线），其余均使用原子类；
  - 解决 Tailwind v4 下“Unknown utility / variant” 报错（原因：组件内独立 `@apply` + 无集中入口导致扫描缺失）；
  - 约定：新增组件禁止再引入局部 `@apply` 聚合样式；若必须写自定义选择器，放入全局 `style.css` 并加注释；
  - 选择器兼容：保留原 BEM-like 类名（如 `git-panel__cards`）以兼容潜在测试/自动化定位；不再依赖其进行视觉呈现，可逐步内联精简。

2. 后端命令与初始化修复
  - `setup.rs`：改用全量显式 `crate::app::commands::<module>::<fn>` 注册，移除过往通过 re-export 聚合导致的 `__cmd__*` 宏展开不稳定风险；
  - 暂停 metrics export HTTP server 的启动（避免早期 Tokio runtime 未就绪触发 panic）。后续将引入“延迟至首个 snapshot 请求”或“插件 on_ready” 方案；
  - `core/http/client.rs`：IP 池选择逻辑改为在锁内克隆结果后立即释放，修复 `Future not Send` / MutexGuard 跨 await 的潜在问题；
  - `core/http/client.rs`：IP 池集成增强——为避免在 Tokio 运行时内创建/阻塞新的 runtime，引入了异步桥接调用路径：`core/ip_pool/global.rs` 提供 `pick_best_async(host, port)`（将请求发送到后台单线程 Tokio runtime），HTTP 客户端通过该 async API 获取选择结果而不阻塞当前 runtime。
    - 同时在异步桥中新增了 fire-and-forget 的上报通道 `report_outcome_async(selection, outcome)`，用于把请求完成后的 success/failure 回写到 `IpPool`（包含对 candidate-level 的 `report_candidate_outcome` 调用以驱动 circuit-breaker）。
    - `core/http/client.rs` 使用了一个小型的 RAII guard（OutcomeReporter）：当 HTTP 实际使用 IP 池候选（Cached）时创建 guard，默认在 Drop 上报 Failure；在正常成功返回路径上显式上报 Success，保证早退/错误路径不会漏写 outcome。
    - 为了可观测性，HTTP 路径现在会在选择与完成时发射结构化事件（`IpPoolSelection` 与 `AdaptiveTlsTiming`），使前端面板和 metrics pipeline 能够看到从 HTTP 路径产生的 IP 池 / TLS 时序数据。
  - `commands/http.rs`：`validate_url` 返回值 clone 语义修正；redirect 最终日志先缓存 `redirect_count` 再输出，避免临时借用冲突；
  - `commands/metrics.rs`：分位去重条件由 `(a - b)` 改为 `(*a - *b)`，修正编译/语义一致性；
  - `commands/oauth.rs`：移除未使用的 `Mutex` 引入；
  - `core/git/http_transport/fallback.rs`：测试辅助在启用 `tauri-app` 特性时正确回退到内部分类函数；
  - `commands/git.rs`：凭证结构字段访问从 getter 改为直接字段（结构体当前公开）；
  - `commands/workspace.rs` & `WorkspaceStorage`：接口改造（`validate/backup/restore_from_backup` 不再接受外部路径参数，构造时绑定路径），路径序列化统一使用 `to_string_lossy()`；
  - `commands/credential.rs`：`CredentialConfig` 不再持有 `master_password` 字段；`set_master_password` 现仅记录警告（占位实现），避免误导已成功启用加密；字段访问统一为结构体公开字段（`cred.host`）；
  - `core/credential/audit.rs`：新增 `OperationType::Unlock`，审计可区分“存储解锁”操作；
  - Cargo：新增 `default-run = "fireworks-collaboration"` 解决多 [[bin]] 目标二义性；
  - 其它：若干 `unused_mut` / 未使用参数将在后续清理（不影响功能）。

### 本次修复与验证摘要（针对 IP 池运行时 panic）

为便于审阅与回溯，本节记录最近针对“Cannot start a runtime from within a runtime” panic 的关键代码变更、验证步骤与结果：

主要变更概览：
- 在 `src-tauri/src/core/ip_pool/global.rs` 中新增异步桥（background bridge running a single-thread Tokio runtime），并公开 `pick_best_async(host, port)` 与 `report_outcome_async(selection, outcome)` 接口；
- 在 `src-tauri/src/core/http/client.rs` 中替换了原先的阻塞式选择调用为 `pick_best_async(...).await`，并引入 OutcomeReporter RAII guard，保证请求完成后 candidate outcome 会被上报；
- 在 `src-tauri/src/core/ip_pool/sampling.rs` 与 `core/metrics/event_bridge.rs` 中增加/增强事件发射与日志，用以使前端 observability 面板能看到 `ip_pool_refresh_total` / `ip_pool_selection_total` 等样本；
- 在文档 `new-doc/IMPLEMENTATION_OVERVIEW.md` 中补充了实现说明与验证步骤（即本文件的增补）。

验证与运行记录（工程内可复现步骤）：
- 阅读/审查文件：核心修改涉及 `core/ip_pool/global.rs`, `core/http/client.rs`, `core/ip_pool/sampling.rs`, `core/metrics/event_bridge.rs` 与若干测试与配置文件；
- 本地测试（src-tauri）：执行 `cargo test -q`（若需运行单测可指定 `--test <name>`）；在修复过程中曾触发一处测试失败（因临时更改 ip_pool 默认值），已恢复配置以保持测试稳定；
- 回归结果示例：`cargo test --test config` 返回 `32 passed, 0 failed`，其它分组测试在本次迭代中均通过（见提交记录与 CI）；
- 运行时验证：在 `pnpm tauri dev` / 本地手工触发 HTTP fake request 场景时，日志中可观察到：ip_pool 预热/刷新事件、选择事件（IpPoolSelection）与 TLS timing (AdaptiveTlsTiming) 的发射，且不再出现 nested runtime panic；

注意事项与后续建议：
- `ip_pool.enabled` 默认值现为 `true`（测试与模板已同步）；如需停用请在配置或命令中显式关闭后再观察相关指标；
- 若需在 CI 中断言 metrics 注册项（非 N/A），建议添加 deterministic helper 测试或 mock 探针，避免网络抖动造成的测试不稳定；
- 文档中变更点已写入本文件，后续若有更多小幅回溯/修复请在变更摘要中追加一行时间与简述，便于审计。

### P4 实现细化：代码指引与本地验证步骤

为便于后续开发者快速定位实现与进行本地验证，下列为精确的代码文件、关键函数与建议的本地验证命令（PowerShell）。遵循这些步骤可以高概率复现/验证修复效果与指标流动。

关键代码位置与要点：
- `src-tauri/src/core/ip_pool/global.rs`
  - Async bridge 类型与入口：`enum AsyncRequest { Pick { .. }, ReportOutcome { .. } }`，使用 `std::sync::mpsc` 为请求通道与 `tokio::sync::oneshot` 为 Pick 的响应通道。
  - 启动桥接线程：`spawn_async_bridge_if_needed()`（在首次调用 `pick_best_async` / `report_outcome_async` 时被创建，在线程内构建 single-thread `tokio` runtime 并在 loop 中接收请求）。
  - 异步选择接口：`pub async fn pick_best_async(host: &str, port: u16) -> IpSelection`（将请求通过 mpsc 发送到后台单线程 Tokio runtime，并通过 oneshot 返回结果；该函数避免在调用处创建/block_on 新的 runtime，因此可安全在任意 async context 中调用）。
  - 异步上报接口：`pub fn report_outcome_async(selection: IpSelection, outcome: IpOutcome)`（fire-and-forget，将 outcome 发送到桥接线程，桥接线程会在其 runtime 中调用 `IpPool::report_outcome` / `report_candidate_outcome` 来更新熔断与 outcome 统计；注意：该 API 不保证同步完成，如需同步确认可通过扩展桥接协议（添加 oneshot 确认通道）实现）。

- `src-tauri/src/core/http/client.rs`
  - 使用：`let sel = pick_best_async(&host, port).await;`
  - Outcome 上报守卫：局部类型 `OutcomeReporter`（在 Drop 上默认上报 Failure，成功路径显式调用 `report_success()`）
  - 观测事件：调用 `emit_ip_pool_selection(...)` 并通过 `publish_global(StructuredEvent::Strategy(AdaptiveTlsTiming{..}))` 发出 `AdaptiveTlsTiming` 事件以记录 TLS 时序与 ip 选择信息

  进一步说明：
  - `OutcomeReporter` 只在 `sel.strategy() == IpSelectionStrategy::Cached` 时创建。它持有 `Option<IpSelection>` 并在 `Drop` 时（若尚未显式标记成功）调用 `report_outcome_async(selection, IpOutcome::Failure)`，确保任何早退、panic 或错误路径都会把 Failure 发送回 IpPool。正常成功流程应调用 `report_success()`，该方法会消费 held selection 并异步上报 Success。此策略保证上报路径的健壮性，同时避免阻塞调用区。
  - HTTP 客户端在发射事件或调用上报前会先通过 `metrics_enabled()` gating，以减少非必要的事件噪声；如果需要在测试中断言上报被消费，推荐使用文档中的 tracing 日志断言模板或桥接层的测试辅助 API。

- `src-tauri/src/core/ip_pool/sampling.rs`
  - 按需采样路径会在不同结果分支发出 `emit_ip_pool_refresh(id, host, success, stats, reason)`，其中 reason 包括 `no_candidates`、`all_probes_failed`、`on_demand` 等

- `src-tauri/src/core/metrics/event_bridge.rs`
  - 事件消费逻辑将 `StrategyEvent::IpPoolSelection` 转换为 `ip_pool_selection_total{strategy,outcome}` 计数器，并在 `IpPoolRefresh` 分支写 `ip_pool_refresh_total{reason,success}`
  - 为便于本地核对，代码中会在处理 `IpPoolRefresh` 后以日志形式打印当前计数值（log 消息包含 "ip_pool_refresh_total incremented"）

本地验证步骤（PowerShell，可复制粘贴）：
1) 在 `src-tauri` 目录运行单测，确保 `pick_best_async` 回归：
```powershell
cd src-tauri
cargo test -q --test ip_pool_async
```
预期：测试通过（`ok` / `passed`），若失败请检查是否有残留全局 pool 状态或锁定文件，测试在 CI 中已被使用以回归 panic 修复。

2) 启动开发模式并触发 HTTP fake request：
```powershell
# 在项目根
pnpm tauri dev
```
然后在浏览器或前端的 HTTP Fake 调试 UI 发起一次 HTTPS 请求（确保 `ip_pool.enabled=true` 若你想观察预热/刷新行为）。

3) 检查后端日志（在运行的 terminal 中或日志文件）：
   - 搜索关键字：
     - "emitting IpPoolSelection event" —— 表示 HTTP 路径已发出选择事件；
     - "processing IpPoolSelection event" —— metrics bridge 正在将选择事件计数为 ip_pool_selection_total；
     - "ip_pool_refresh_total incremented" —— 预热/按需刷新计数器已被 registry 增量并以日志暴露当前值；
     - 不应出现的错误："Cannot start a runtime from within a runtime"（若出现则表示桥接未正确使用）

4) 快速查看 metrics registry（运行时日志中已经有 increment 输出），示例预期日志片段：

   - 当按需采样无候选时：
     "ip_pool_refresh_total incremented" reason=no_candidates success=false count=1
   - 当样本成功写回时：
     "ip_pool_refresh_total incremented" reason=on_demand success=true count=5
   - 在选择路径：
     "processing IpPoolSelection event" strategy=Cached source=builtin latency_ms=12

5) 若希望在测试中断言 metrics，可添加一个专用测试 hook：
   - 建议在 `src-tauri/src/core/metrics` 增加一个测试-only API（feature-gated）返回 registry 的 counter 值，或在测试使用日志断言来避免对内部类型的直接依赖。

注意事项与小技巧：
- `ip_pool.enabled` 默认在 config 中为 `true` —— 新实例会自动拉起预热线程；若想暂时回退到系统 DNS，可通过配置禁用或调用 `ip_pool_clear_auto_disabled`/`ip_pool_update_config` 明确关闭；
- 观测链路依赖 `metrics_enabled()` gating（HTTP 客户端使用相同 gating 以减少噪声），必要时可在 `core/git/transport/metrics` 中临时开关该 flag 以便调试；
- 若需要模拟候选失败/成功做 circuit-breaker 场景，请在 `preheat::measure_candidates` 或 probe 函数中注入可控的测试钩子（推荐通过 #[cfg(test)] helpers），以避免网络抖动造成的不可重复测试。

小节：快速调试配置片段与日志断言（可复制）

1) 在本地启用 IP 池与 metrics（示例 `config.json` 片段）：

```jsonc
{
  "ip_pool": {
    "enabled": true,
    "maxCacheEntries": 256
  },
  "observability": {
    "basicEnabled": true,
    "aggregateEnabled": true,
    "exportEnabled": false,
    "uiEnabled": true
  }
}
```

把此片段合并到项目根的 `config/config.json`（或通过你的本地配置覆盖机制），然后重启应用/重新加载配置。

2) 常用日志 grep 模式（PowerShell / Windows）：

```powershell
# 寻找选择事件发出
Select-String -Path .\src-tauri\target\debug\* -Pattern "emitting IpPoolSelection event" -SimpleMatch -CaseSensitive

# 运行时日志实时查看（在 dev 终端中）：
Get-Content -Path .\path\to\runtime.log -Wait | Select-String "IpPoolSelection|IpPoolRefresh|ip_pool_refresh_total incremented|Cannot start a runtime from within a runtime"
```

3) 最小 Rust 测试片段（日志断言思路，粘贴到 `src-tauri/tests/ip_pool_log_assert.rs` 并在 CI/本地运行）：

```rust
// 该示例演示如何在测试中捕获 tracing 日志并断言包含关键消息。需根据项目的 tracing 初始化调整。
use tracing_subscriber::fmt::Collector;
use tracing::info;

#[test]
fn ip_pool_emits_refresh_log() {
    // 初始化 tracing 以捕获到同一进程的日志（仅测试环境）
    let collector = Collector::builder().with_max_level(tracing::Level::INFO).finish();
    let _guard = tracing::subscriber::set_default(collector);

    // 触发按需采样（使用测试钩子或直接调用 sampling::sample_once 的包装）
    // e.g., crate::core::ip_pool::testing::trigger_on_demand("example.test", 443);

    // 断言：在日志输出中找到 "ip_pool_refresh_total incremented" 或 "emitting IpPoolSelection event"
    // 具体实现可用 tracing-test 或捕获日志输出到内存 buffer 并搜索关键字。
    assert!(true, "示例：请用 tracing-test 或日志 buffer 搜索关键字符串");
}
```

说明：上面测试为示例模板；推荐使用 `tracing-test`（crate）或把 tracing 输出重定向到内存缓冲区再断言，避免对内部 registry 的直接依赖。

我将在文档中保留这些具体命令与日志示例，便于审阅者和运维按步骤复核。如果你同意，我可以继续：
- 在 `src-tauri/tests` 中添加一个确定性测试（feature-gated 或使用注入的测试钩子），直接断言在触发 on-demand sampling 后 `ip_pool_refresh_total` 有增量（建议采用日志断言实现以减少对内部 registry 的耦合）。

3. Observability / P8 运行差异记录
  - 当前构建中 metrics export server **未启动**（参见 `setup.rs` 中 NOTE），文档的导出/告警层级行为以设计为准；运行期仅 basic/aggregate 注册逻辑执行；
  - 前端指标面板代码已完成 Tailwind v4 适配（所有 `observability/*Panel.vue` & `Metric*` 组件）；
  - 观测链路增强：HTTP 客户端与 IP 池的事件（`IpPoolSelection`、`AdaptiveTlsTiming`）已被接入事件桥（EventMetricsBridge），在本地运行可使前端 snapshot 收到 `ip_pool_selection_total` 与 TLS timing 样本。但注意 metrics export server 在当前分支仍被延迟启动（参见 §2），如需对外导出需显式启用或改为按需 spawn。
  - 若需手动验证导出/告警，请暂时在本地分支恢复 `init_export_observability` 调用或实现延迟启动逻辑（建议：在首次 `metrics_snapshot` 命令时 `spawn`）；
  - 追加开发指引：当导出层暂不可用时，前端不应假定 `/metrics` 存在，须对 404 / 网络错误做优雅降级（当前 UI 已容错）。
  - 与 §4.10 “设计 vs 当前运行态” 表保持同步（表中列出每个子能力当前状态）。
  - 统一“占位/未实现”列表见“已知占位与待办”章节，避免散落信息遗漏。

4. 工作区 / 子模块
  - 序列化字段 `root_path` / 仓库 `path` 改为字符串（lossy UTF-8），避免 Windows 非 UTF-8 路径 panic；
  - 相关文档中关于 `WorkspaceStorage::validate(&path)` 的旧调用示例已过期，现应写作：`let storage = WorkspaceStorage::new(path.clone()); storage.validate()?;`；
  - 备份与恢复 API 同理：`storage.backup()?` / `storage.restore_from_backup(&backup_path)?`。

5. 凭证与安全
  - 主密码设置尚处“占位”阶段：调用 `set_master_password` 仅完成存储重建 + 警告日志，不持久保存密码，也未对后续加密流程生效；
  - 审计事件新增 `unlock` 操作类型后，前端在展示审计表格时需兼容该枚举（如未适配，显示原始字符串即可）。

6. 迁移风险 & 回滚
  - 样式迁移全部为静态模板改写，不改变脚本逻辑，风险集中在视觉回归；
  - 回退策略：若出现布局异常，可在本地 revert `src/style.css` 与受影响组件，恢复到 v1.5（2025-10-06）标签或该提交之前的快照；
  - Metrics export 缺失导致的观测空洞：临时需求可快速本地添加一行恢复调用（`init_export_observability`），*不建议*在未分析 Tokio 上下文的情况下提前合入主干。

7. 开发规范补充
  - 新增组件请：优先内联 utility，最多一行自定义 CSS（非 Tailwind 所能表达）保留在局部 `<style scoped>`；
  - 需要复用的“语义组合”使用可读的 class 别名（例如 `git-panel__meta`），同时仍保留核心原子类；
  - 不再使用 `@apply` / `@plugin` / `@custom-variant` 于单文件组件；统一放置 `src/style.css`；
  - 若前端测试依赖旧 `.xxx__y` 选择器，当前保留，不建议再新增无语义的多层 BEM 套嵌。

影响评估：
| 方面 | 结果 |
|------|------|
| 二进制行为 | 与 v1.5 对比，无命令签名破坏；初始化流程更安全（少启动 export） |
| 样式构建 | 构建无 “Unknown utility” 报错；构建时间轻微下降（无多文件 @apply 扫描） |
| 运行稳定性 | 解决 IP 池锁跨 await 潜在 Send 问题；减少宏注册歧义 |
| 安全/审计 | 新增 Unlock 操作；主密码逻辑暂未启用（标注清晰） |
| 回滚成本 | 仅需 revert 前端样式与 `setup.rs` 中 handler 列表即可恢复上一稳定版本 |

（后续计划：A) 延迟 metrics export 启动；B) 实现真正的加密主密码流；C) 清理 credential 命令中占位/unused 警告；D) 增补 Tailwind 组件层测试快照。）

---

## 1. 范围与阅读指引

- **涵盖阶段**：
  - **MP0**：gitoxide -> git2-rs 替换，任务/事件契约保持不变；
  - **MP1**：Push、方式A自定义 smart subtransport、Retry v1、事件增强；
  - **P2**：本地 Git 操作扩展、Shallow/Partial、任务级策略覆盖、策略信息事件与护栏；
  - **P3**：自适应 TLS 全量 rollout、可观测性强化、Real Host 校验、SPKI Pin、自动禁用、Soak；
  - **P4**：IP 池采样与优选、预热调度、熔断治理、Soak 阈值；
  - **P5**：代理支持（HTTP/SOCKS5/System）、自动降级与恢复、健康检查、前端集成；
  - **P6**：凭证存储与安全管理（三层存储、AES-256-GCM加密、Argon2id密钥派生、审计日志、访问控制、Git集成）；
  - **P7**：多仓工作区（Workspace）模型、Git 子模块支持、批量并发调度（clone/fetch/push）、团队配置模板导出/导入、跨仓库状态监控、前端一体化视图与性能/稳定性基准；
  - **测试重构**：`src-tauri/tests` 聚合结构、事件 DSL、属性测试与回归种子策略。
- **读者画像**：
  - 新接手的后端/前端开发；
  - 运维与 SRE（回退、监控、调参）；
  - 测试与质量保障（测试矩阵、DSL 约束）。
- **联动文档**：`new-doc/MP*_IMPLEMENTATION_HANDOFF.md`、`new-doc/P*_IMPLEMENTATION_HANDOFF.md` 系列交接稿、`new-doc/TECH_DESIGN_*.md` 设计稿、`doc/TESTS_REFACTOR_HANDOFF.md`。

---

## 2. 里程碑一览

| 阶段 | 核心交付 | 事件新增/调整 | 配置扩展 | 回退策略 | 测试现状 |
|------|-----------|---------------|----------|----------|----------|
| MP0 | git2-rs 基线，Clone/Fetch 稳定，取消/错误分类统一 | 无新增，进度保持兼容 | 继承 HTTP Fake 调试配置 | 可回退到旧二进制（保留 gitoxide tag 归档） | `cargo test` / `pnpm test` 全绿 |
| MP1 | Push、方式A smart subtransport、Retry v1、进度阶段化 | `task://error`，Push `PreUpload/Upload/PostReceive`，错误分类输出 | HTTP/TLS/Fake SNI 配置热加载 | Push/方式A/Retry 可配置关闭或自动回退 | Rust/前端测试覆盖 push、事件 casing |
| P2 | 本地操作（commit/branch/checkout/tag/remote）、Shallow/Partial、策略覆盖、护栏、Summary | 结构化策略事件（Strategy::HttpApplied/Conflict/IgnoredFields/Summary + Policy::RetryApplied + Transport::PartialFilterFallback） | `strategyOverride` 入参，env gating（`FWC_PARTIAL_FILTER_SUPPORTED`、`FWC_PARTIAL_FILTER_CAPABLE`） | 逐项移除 TaskKind / 停用能力探测 | 新增矩阵测试、属性测试覆盖策略解析 |
| P3 | 自适应 TLS rollout + 可观测性、Real Host 校验、SPKI Pin、自动禁用、Soak | `AdaptiveTls*` 结构化事件、指纹变化事件 | `http.fakeSniRolloutPercent`、`tls.metricsEnabled`、`tls.certFpLogEnabled`、`tls.spkiPins` 等 | 配置层关闭 Fake/metrics/pin；自动禁用冷却（Real Host 校验常开） | Soak 测试 + 指标契约测试 |
| P4 | IP 池采样与握手优选、传输集成、异常治理、观测扩展、Soak 阈值 | `IpPoolSelection`、`IpPoolRefresh`、`IpPoolAutoDisable`、`IpPoolCidrFilter` 等 | `ip_pool.*` 运行期与文件配置（缓存、熔断、TTL、黑白名单）| 配置禁用 IP 池/熔断/预热；自动禁用冷却 | Rust 单测/集测、IP 池集成测试、Soak 报告 |
| P5 | 代理支持（HTTP/SOCKS5/System）、自动降级与恢复、前端集成 | `ProxyStateEvent`、`ProxyFallbackEvent`、`ProxyRecoveredEvent`、`ProxyHealthCheckEvent` 等 | `proxy.*` 配置（mode/url/auth/超时/降级/恢复/健康检查/调试日志）| 配置禁用代理/手动降级恢复/调整阈值 | 276个测试（243 Rust + 33 TypeScript），跨平台系统检测，状态机转换验证 |
| P6 | 凭证存储（三层：系统钥匙串/加密文件/内存）、加密安全（AES-256-GCM + Argon2id）、审计日志、访问控制、Git自动填充 | `CredentialEvent`（Add/Get/Update/Delete/List/Cleanup）、`AuditEvent`（操作审计 Unlock 含）、`AccessControlEvent`（失败锁定） | `credential.*` 配置（storage/auditMode/accessControl/keyCache/TTL 等，已移除 masterPassword 字段）| 配置逐层禁用存储/关闭审计/调整锁定阈值 | 1286个测试（991 Rust + 295 前端），99.9%通过率，88.5%覆盖率，批准生产环境上线 |
| 测试重构 | 主题聚合、事件 DSL、属性测试集中管理 | DSL 输出 Tag 子序列 | N/A | N/A | `src-tauri/tests` 结构稳定，CI 使用共享 helper |
| P7 | 工作区模型、子模块支持、批量并发 clone/fetch/push、团队配置模板、跨仓库状态监控、前端一体化视图 | 无新增事件类型（复用 task/state/progress/error），批量任务 progress phase 含聚合文本 | `workspace.*`、`submodule.*`、`teamTemplate`、`workspace.status*` | 配置禁用 workspace 或降并发；子模块/模板/状态可单项停用 | 新增 24 子模块测试 + 12 批量调度测试 + 状态缓存测试 + 前端 store 17 测试 + 性能基准 |
| P8 | 可观测性体系（统一指标注册/事件桥接/窗口聚合/导出/前端面板/告警+Soak/灰度层级/性能降级） | `MetricAlert`、`ObservabilityLayerChanged`、`MetricDrift` 新增；复用 TLS/IP/代理/Soak 事件 | `observability.*`（enabled/layer/*Enabled/performance/export/alerts/...） | 层级裁剪 + autoDowngrade；逐项关闭 export/ui/alerts | 指标/导出/告警/层级/降级/前端缓存测试（详见 P8 handoff §13） |

---

## 3. 总体架构快照

### 3.1 命令与任务接口

Tauri 暴露的稳定命令（保持 camelCase 输入，容忍 snake_case）：

```ts
// Git 操作
git_clone(repo: string, dest: string): Promise<string>
git_fetch(repo: string, dest: string, preset?: 'remote'|'branches'|'branches+tags'|'tags'): Promise<string>
git_push(opts: { dest: string; remote?: string; refspecs?: string[]; username?: string; password?: string }): Promise<string>

// 本地操作（P2）
git_commit(opts: CommitInput): Promise<string>
git_branch(opts: BranchInput): Promise<string>
git_checkout(opts: CheckoutInput): Promise<string>
git_tag(opts: TagInput): Promise<string>
git_remote_add|set|remove(opts: RemoteInput): Promise<string>

// 凭证管理（P6）
add_credential(opts: { host: string; username: string; password: string; expiresAt?: number }): Promise<void>
get_credential(host: string, username: string): Promise<CredentialInfo | null>
update_credential(opts: { host: string; username: string; password: string; expiresAt?: number }): Promise<void>
delete_credential(host: string, username: string): Promise<void>
list_credentials(): Promise<CredentialInfo[]>
cleanup_expired_credentials(): Promise<number>
set_master_password(password: string, config: CredentialConfig): Promise<void>
unlock_store(password: string, config: CredentialConfig): Promise<void>
export_audit_log(): Promise<string>
cleanup_audit_logs(retentionDays: number): Promise<number>
is_credential_locked(): Promise<boolean>
reset_credential_lock(): Promise<void>
remaining_auth_attempts(): Promise<number>

// 任务控制
task_cancel(id: string): Promise<boolean>
task_list(): Promise<TaskSnapshot[]>

// IP 池
ip_pool_get_snapshot(): Promise<IpPoolSnapshot>
ip_pool_update_config(runtime: IpPoolRuntimeConfig, file: IpPoolFileConfig): Promise<IpPoolSnapshot>
ip_pool_request_refresh(): Promise<boolean>
ip_pool_start_preheater(): Promise<IpPoolPreheatActivation>
ip_pool_clear_auto_disabled(): Promise<boolean>
ip_pool_pick_best(host: string, port: number): Promise<IpSelectionResult>

// 调试
git_task_debug?(internal)
http_fake_request(input: HttpRequestInput): Promise<HttpResponseOutput>
```

并非所有命令都会返回 `taskId`：

返回模式分类（含 P7 扩展）：
- 返回任务ID (string)：触发异步 Git / 批量 / 长时工作（如 `git_clone`、`workspace_batch_clone` 等），前端需订阅事件流。
- 直接结构化对象：同步查询或立即构造结果（例如 `create_workspace` / `load_workspace` 返回 `WorkspaceInfo`，`get_workspace_statuses` 返回状态结果，`import_team_config_template` 返回导入报告）。
- 直接 void / boolean：快速成功/失败或无附加数据（`save_workspace` -> void，`restore_workspace` -> void，`validate_workspace_file` -> boolean）。
- 字符串（非任务ID）：导出/备份生成的文件路径（如 `export_team_config_template`、`backup_workspace`、`export_audit_log`）。
- 列表：同步枚举结果（`list_submodules`、`init_all_submodules` 等）。
前端策略：仅当返回值形态为“看起来像任务ID”且调用约定属于异步任务类命令时进入事件订阅；其余直接更新本地 store。必要时可通过附加前缀/长度规则区分（当前 taskId 为 UUID 形式）。

P7 新增的工作区/子模块/批量与团队配置相关命令（命名保持 camelCase，可与上表并列理解）：

```ts
// 工作区管理
create_workspace(opts: { name: string; rootPath: string }): Promise<WorkspaceInfo>
load_workspace(path: string): Promise<WorkspaceInfo>
save_workspace(path: string): Promise<void>
add_repository(opts: { workspaceId: string; repo: RepositorySpec }): Promise<string>
remove_repository(opts: { workspaceId: string; repoId: string }): Promise<boolean>
update_repository_tags(opts: { workspaceId: string; repoId: string; tags: string[] }): Promise<boolean>
validate_workspace_file(path: string): Promise<boolean>   // 校验 workspace.json 结构
backup_workspace(path: string): Promise<string>           // 返回带时间戳的备份文件路径
restore_workspace(backupPath: string, workspacePath: string): Promise<void>

// 子模块操作
list_submodules(opts: { repoPath: string }): Promise<SubmoduleInfo[]>
has_submodules(opts: { repoPath: string }): Promise<boolean>
init_all_submodules(opts: { repoPath: string }): Promise<string[]>
update_all_submodules(opts: { repoPath: string }): Promise<string[]>
sync_all_submodules(opts: { repoPath: string }): Promise<string[]>

// 批量任务（返回父任务 taskId）
workspace_batch_clone(req: WorkspaceBatchCloneRequest): Promise<string>
workspace_batch_fetch(req: WorkspaceBatchFetchRequest): Promise<string>
workspace_batch_push(req: WorkspaceBatchPushRequest): Promise<string>

### 3.6 可观测性运行时（P8 速览）

拓扑：事件 → Bridge → Registry(缓冲+分片) → Aggregator(1m/5m/1h/24h) → 导出/告警/前端面板/一致性自检/Soak；层级 basic→optimize，资源异常自动降级；详述见 §4.10 与 `P8_IMPLEMENTATION_HANDOFF.md`。

// 团队配置模板
export_team_config_template(opts?: { path?: string; sections?: string[] }): Promise<string> // 返回生成文件路径
import_team_config_template(opts: { path?: string; strategy?: ImportStrategyConfig }): Promise<TemplateImportReport>

// 跨仓库状态
get_workspace_statuses(opts: StatusQuery): Promise<WorkspaceStatusResult>
clear_workspace_status_cache(): Promise<number>
invalidate_workspace_status_entry(opts: { repoId: string }): Promise<boolean>
```

### 3.2 事件总览

- `task://state`：`{ taskId, kind, state, createdAt }`，`state ∈ pending|running|completed|failed|canceled`；
- `task://progress`：
  - Clone/Fetch：`{ taskId, kind, phase, percent, objects?, bytes?, totalHint?, retriedTimes? }`；
  - Push：`phase ∈ PreUpload|Upload|PostReceive`；
- `task://error`：分类或信息事件，`{ taskId, kind, category, message, code?, retriedTimes? }`；
- 自适应 TLS 结构化事件（P3）：`AdaptiveTlsRollout`、`AdaptiveTlsTiming`、`AdaptiveTlsFallback`、`AdaptiveTlsAutoDisable`、`CertFingerprintChanged`、`CertFpPinMismatch`；
- 策略结构化事件（P2）：`Strategy::HttpApplied`、`Strategy::Conflict`、`Strategy::IgnoredFields`、`Strategy::Summary`、`Transport::PartialFilterFallback`、`Policy::RetryApplied`（Clone/Push）；Fetch 的 Retry 差异仅在 Summary `applied_codes` 中体现。
- IP 池与优选事件（P4）：`IpPoolSelection`、`IpPoolRefresh`、`IpPoolCidrFilter`、`IpPoolIpTripped`、`IpPoolIpRecovered`、`IpPoolAutoDisable`、`IpPoolAutoEnable`、`IpPoolConfigUpdate`；同时 `AdaptiveTlsTiming/Fallback` 增补 `ip_source`、`ip_latency_ms`、`ip_selection_stage` 可选字段。
- 代理事件（P5）：`ProxyStateEvent`（状态转换，含扩展字段）、`ProxyFallbackEvent`（自动/手动降级）、`ProxyRecoveredEvent`（自动/手动恢复）、`ProxyHealthCheckEvent`（健康检查结果）；代理启用时通过传输层注册逻辑强制禁用自定义传输层与 Fake SNI；配置热更新和系统代理检测不发射独立事件，由Tauri命令直接返回结果。
- 凭证与审计事件（P6）：`CredentialAdded`、`CredentialRetrieved`、`CredentialUpdated`、`CredentialDeleted`、`CredentialListed`、`ExpiredCredentialsCleanedUp`（凭证生命周期）；`AuditEvent`（操作审计，含用户/时间/操作类型/结果/SHA-256哈希）；`AccessControlLocked`、`AccessControlUnlocked`（失败锁定与恢复）；`StoreUnlocked`、`StoreLocked`（加密存储解锁状态）。

事件顺序约束在测试中锁定：策略结构化事件按 applied → conflict → ignored → partial fallback → summary 发出；TLS 事件在任务结束前统一刷出；凭证操作触发审计事件在命令执行后同步发射。Push 的 conflict 仍补充一条信息级 `task://error` 以兼容旧 UI。

P7 未新增独立事件类型：工作区、子模块、批量调度与状态查询均复用既有 `task://state|progress|error` 语义。批量任务父进度的 `phase` 字段采用聚合文本（如 `Cloning 2/5 completed (1 failed)`），子模块递归克隆阶段通过主任务进度区间（0-70-85-100%）映射，不引入单独子模块事件流。状态服务（WorkspaceStatusService）目前仅通过命令拉取结果，后续事件推送在后续迭代规划中。

P7 已知限制（未纳入本次交付）：
- 子模块并行初始化/更新参数 `parallel/maxParallel` 预留但未实现（串行足够 <10 子模块常见场景）。
- 子模块粒度实时进度事件（`SubmoduleProgressEvent`）尚未连接前端事件总线，仅通过主任务阶段映射。
- 批量任务进度权重均等，未按仓库体积/历史耗时加权；大体量差异下显示可能不线性。
- 工作区状态服务无事件推送（需轮询）；大量仓库高频刷新需手动调大 `statusCacheTtlSecs` 与关闭自动刷新。

TaskKind 扩展（P7 补充说明）：
- 递归克隆：沿用 `TaskKind::GitClone`，仅在克隆完成后根据配置附加子模块 init/update 两阶段（映射到 70–85%、85–100% 进度区间）。
- 子模块独立操作：未新增专属 TaskKind，命令直接进行同步/初始化逻辑，失败通过日志与返回值暴露。
- 批量调度：新增 `TaskKind::WorkspaceBatch { operation, total }` 作为父任务快照，子任务仍为原生 Git TaskKind（Clone/Fetch/Push），通过父子关联表跟踪；父任务进度 = 子任务完成百分比平均。

### 3.3 服务与分层

**P7 状态管理架构补充**:
Tauri 应用层使用 `Arc<Mutex<T>>` 模式管理三个独立的全局状态:
- `SharedWorkspaceManager = Arc<Mutex<Option<Workspace>>>`：当前加载的工作区实例，commands 通过 State 注入访问；
- `SharedWorkspaceStatusService = Arc<WorkspaceStatusService>`：跨仓库状态查询服务，内部维护 TTL 缓存与并发控制，与 WorkspaceManager 解耦；
- `SharedSubmoduleManager = Arc<Mutex<SubmoduleManager>>`：子模块管理器，拥有独立配置(`SubmoduleConfig`)，支持递归初始化/更新/同步操作。

`WorkspaceStorage` 不是全局单例，每次 `load_workspace`/`save_workspace` 调用时实例化并传入路径，确保多工作区场景下无状态冲突。批量任务通过快照(`workspace.clone()`)避免持锁跨 async 边界。

```
TaskRegistry (core/tasks/registry.rs)
 ├─ 状态机：注册/运行/取消/重试
 ├─ 事件汇聚：state/progress/error -> Tauri emitter
 ├─ Retry v1：指数退避、类别判定（Push 上传前）
 └─ 策略应用：策略覆盖、护栏、Summary（P2+）

GitService (core/git/default_impl/*)
 ├─ git2-rs clone/fetch/push 基线（MP0/MP1）
 ├─ Push 凭证回调、进度阶段（MP1）
 ├─ 自定义 smart subtransport 方式A（transport/*, MP1）
 ├─ Shallow/Partial 策略与 capability（P2）
 └─ Adaptive TLS、fallback、metrics（P3）

Transport Stack
 ├─ Rewrite + rollout 决策（P3）
 ├─ Fallback 状态机 Fake->Real->Default
 ├─ TLS 验证与 SPKI Pin
 ├─ IP 池候选消费与握手埋点（P4）
 └─ 自动禁用窗口

 IP Pool Service (core/ip_pool/*)
 ├─ `IpPool` 统一入口（pick/report/maintenance/config）
 ├─ `PreheatService` 调度多来源采样（builtin/history/userStatic/DNS/fallback）
 ├─ `dns` 模块管理系统解析器开关、自定义 DoH/DoT/UDP 解析器与预设启用列表
 ├─ `IpScoreCache` + `IpHistoryStore` 缓存与持久化（TTL、容量、降级）
 ├─ 异步桥接（新）：`global.rs` 提供 `pick_best_async(host,port)`，在后台单线程 Tokio runtime 中安全执行 `IpPool::pick_best`，避免在任意 Tokio 运行时内部构建/阻塞新的 runtime 导致 panic；该桥也支持 `report_outcome_async(selection,outcome)` 的 fire-and-forget 上报。
 ├─ 传输层集成：`custom_https_subtransport` 仍直接消费候选、尝试 candidate 连接并在本地线程路径上调用 `report_candidate_outcome`；HTTP 客户端（`core/http/client.rs`）也已集成候选消费路径并通过异步桥上报 outcome，从而保证所有传输路径都能为熔断与观测供样本。
 └─ 异常治理：`circuit_breaker`、黑白名单、全局自动禁用（P4）

Proxy Service (core/proxy/*)
 ├─ `ProxyManager` 统一管理（连接器、状态、配置、健康检查）
 ├─ HTTP/SOCKS5 连接器（CONNECT隧道、协议握手、Basic Auth）
 ├─ `ProxyFailureDetector` 滑动窗口失败检测与自动降级
 ├─ `ProxyHealthChecker` 后台探测与自动恢复
 ├─ `SystemProxyDetector` 跨平台系统代理检测（Windows/macOS/Linux）
 └─ 传输层集成：代理启用时强制禁用自定义传输层与 Fake SNI（P5）

Credential Service (core/credential/*)
 ├─ `CredentialStoreFactory` 三层存储智能回退（系统钥匙串 → 加密文件 → 内存）
 ├─ 系统钥匙串集成：Windows Credential Manager、macOS Keychain、Linux Secret Service（P6.1）
 ├─ `EncryptedFileStore` AES-256-GCM加密 + Argon2id密钥派生 + 密钥缓存（P6.1）
 ├─ `InMemoryStore` 进程级临时存储（P6.0）
 ├─ `AuditLogger` 双模式审计（标准/审计模式，SHA-256哈希，持久化）（P6.2/P6.5）
 ├─ `AccessControl` 失败锁定机制（5次失败 → 30分钟锁定 → 自动过期）（P6.5）
 ├─ Git集成：`git_credential_autofill` 智能降级（存储 → 未找到 → 出错）（P6.4）
 └─ 前端集成：13个Tauri命令、4个Vue组件、Pinia Store（P6.3/P6.5）
```

前端（Pinia + Vue）在 `src/api/tasks.ts` 统一订阅事件，将 snake/camel 输入归一，`src/stores/tasks.ts` 管理任务、进度、错误、策略事件。

### 3.4 核心依赖与版本策略
- 后端 Rust 依赖：
  - `git2 = "0.19"`（MP0 起启用，MP1 推送凭证仍依赖该版本提供的回调 API；若需升级，需先验证 Windows/MSVC 与 macOS 通路的二进制兼容性）。
  - `tauri = 1.x` + `tauri-build`：通过可选特性 `tauri-app` 控制，`cargo test` 默认禁用 Tauri UI，确保核心逻辑可在 CI 上独立构建。
  - 传输层模块自带 `reqwest`（方式A 仅在 Fake SNI 调试路径使用，MP3 adaptive TLS 不直接依赖）。
  - TLS 校验依赖 `rustls` + `webpki`（P3 引入 Real Host 校验与 SPKI Pin 时同步升级到最新 LTS）。
- 前端依赖：
  - `vite`, `vue 3`, `pinia`, `@tauri-apps/api`，与 MP0 前保持一致；P2 起新增策略编辑组件使用 `zod` 做轻量校验。
  - 测试栈 `vitest` + `@testing-library/vue`；事件 DSL 断言主要位于 `src/views/__tests__`。
- 配置加载：`src-tauri/src/config/loader.rs` 使用 `directories` crate 定位应用目录，支持热加载；更改配置文件后由任务注册器下一次读取时生效。
- 版本管理：所有 Git 子传输代码在 `src-tauri/src/core/git/transport` 下有 `COVERAGE.md` 与 `MUTATION_TESTING.md`，升级依赖须同步更新两份保障文档。

P7 追加的主要配置键（集中在 `config.json`）：
`workspace.enabled`（启用工作区，**实际默认 false**，保持向后兼容）、`workspace.maxConcurrentRepos`（批量并发上限，**实际默认 3**，保守配置）、`workspace.statusCacheTtlSecs`（**实际默认 15秒**）/ `workspace.statusMaxConcurrency`（**实际默认 4**）/ `workspace.statusAutoRefreshSecs`（默认 null，禁用自动刷新）；`submodule.*`（autoRecurse/maxDepth/autoInitOnClone/recursiveUpdate/parallel/maxParallel，其中并行当前未实现）；`teamTemplate`（导出/导入默认路径及策略开关）；其余保持向后兼容，未启用时不影响已有单仓库功能。

最小可用配置示例（启用工作区 + 子模块支持 + 保守刷新）：
```jsonc
{
  "workspace": {
    "enabled": true,              // 默认 false，需要显式启用
    "maxConcurrentRepos": 3,      // 默认值即为 3，保守并发
    "statusCacheTtlSecs": 15,     // 默认值 15 秒
    "statusMaxConcurrency": 4,    // 默认值 4
    "statusAutoRefreshSecs": 60   // 可选，默认 null（禁用）
  },
  "submodule": {
    "autoRecurse": true,
    "maxDepth": 5,
    "autoInitOnClone": true,
    "recursiveUpdate": true,
    "parallel": false,
    "maxParallel": 3
  },
  "teamTemplate": {
    "defaultExportPath": "config/team-config-template.json"
  }
}
```

P7 测试覆盖摘要：新增子模块模型与操作单/集成测试 24 项；批量调度（clone/fetch/push）集成测试 12 项，验证并发、失败摘要与取消传播；状态服务缓存/失效/性能测试若干（含 10/50/100 仓库 p95 基准）；团队模板导出/导入 7 项；前端 Pinia store 新增 17 项单测（批量任务、模板报告、状态缓存）；端到端性能基准测试纳入 Nightly（批量 clone、状态刷新）。

性能基线（本地自动化环境 p95 指标，用于回归门槛参考）：
- 批量 clone：10/50/100 仓库 p95 用时分别 ≈15.5ms / 11.2ms / 10.2ms（相对单仓 baseline 105.5ms 聚合后均 < 0.2×/仓）。
- 状态刷新：10/50/100 仓库总耗时 ≈10.9ms / 54.2ms / 80.8ms（100 仓库 <3s 目标内充足裕度）。
- 子模块递归初始化+更新阶段占主任务 30%（70→100% 区间），失败不阻塞主任务完成。
- 性能回归策略：Nightly 比对最近 7 次运行滚动窗口，如 p95 超出基线 2× 触发告警并要求人工复核差异日志（任务元数据与结构化事件）。

回退快速参考（按优先级从最小影响到功能全禁用）：
| 场景 | 操作 | 影响范围 | 备注 |
|------|------|----------|------|
| 批量任务负载过高 | 将 `workspace.maxConcurrentRepos` 降为 1 | 退化为顺序执行 | 不中断现有父任务，但新任务生效 |
| 子模块初始化频繁失败 | `submodule.autoInitOnClone=false` | 保留主仓库克隆 | 可手动调用 init_all_submodules |
| 状态刷新造成 IO 压力 | `workspace.statusAutoRefreshSecs=0` 提高 `statusCacheTtlSecs` | 停止自动轮询 | 需要手动刷新按钮或命令 |
| 模板导入疑似破坏配置 | 使用最近备份文件覆盖 `config.json` | 恢复所有配置节 | 备份命名含时间戳易定位 |
| 工作区整体不稳定 | `workspace.enabled=false` | 回退单仓模式 | 不需要重编译，重启后生效 |
| 仅想屏蔽批量 UI | 保持 enabled，前端配置隐藏入口（可选） | 后端能力仍可保留 | 方便灰度逐步恢复 |

工作区文件与并发风险提示：
- 当前未实现显式锁文件；多进程同时写入 `workspace.json` 理论上存在竞态，建议运维避免同一物理目录并行启动两个实例（或通过容器编排保证单实例）。
- 保存操作采用原子写（写临时文件后 rename），若写入中断可回退最近备份或使用 `.bak` 文件恢复。
- 频繁批量操作配合低 TTL 状态刷新可能导致磁盘 I/O 峰值，建议：高并发任务时临时调大 `statusCacheTtlSecs` 并暂停自动刷新。

团队模板安全与去敏化要点：
- 导出时自动清理：代理密码、凭证文件路径、IP 池历史文件路径、运行态敏感统计字段。
- 备份策略：每次导入前生成 `team-config-backup-YYYYMMDDHHMMSS.json`，回滚只需将备份覆盖现有 `config.json` 并重新加载。
- Merge 策略忽略本地默认值（保持最小差异），保留本地 IP 池 historyPath 以避免分发机器路径。
- Schema 主版本不匹配直接拒绝导入；报告中列出 `applied` 与 `skipped` 节及原因（如 strategyKeepLocal / sectionDisabled / noChanges）。

### 3.5 发布节奏与跨阶段集成
- **推广顺序**：遵循 MP0 -> MP1 -> P2 -> P3 的递进路径，每阶段功能上线前都需确认前置阶段的回退手段仍可用；详见 `new-doc/MP*_IMPLEMENTATION_HANDOFF.md`。
- **配置切换流程**：
  1. 在预生产环境调整 `AppConfig`/环境变量验证行为；
  2. 触发 `fwcctl reload-config` 或重启 Tauri 容器以生效；
  3. 通过 `task_list` 验证无历史任务处于异常状态，再逐步扩大发布半径。
- **灰度策略**：MP1 的方式A 与 Retry、P3 的 fake SNI rollout 都支持按域或百分比分级，推荐每级灰度至少运行一次 soak 或回归脚本；
  - Rollout 0% -> 25% -> 50% -> 100%，期间监控 `AdaptiveTlsRollout` 与 `AdaptiveTlsFallback` 事件；
  - Push/策略相关配置调整需同步更新前端提示与文档，避免用户误解重试次数或凭证要求。
  - P4 的 IP 池默认已启用：可先通过 `preheatDomains` 控制范围，再逐步扩容；如需灰度或回退，可在 `ip_pool.enabled`/`maxCacheEntries`/`singleflightTimeoutMs` 上做分环境调节，并搭配 Soak 报告监控 `selection_by_strategy` 与 `refresh_success_rate`。
- **跨阶段依赖**：
  - P2 的策略覆盖与 P3 的 adaptive TLS 共享 HTTP/TLS 配置，只在任务级做差异化；
  - P3 的指纹日志与自动禁用依赖 MP1 方式A 的传输框架，如需临时关闭 Fake SNI，应评估 P3 指标链路的可见性。
  - P4 的 IP 池与 P3 的 Adaptive TLS 深度联动：传输层在同一线程上下文填充 `ip_source`/`ip_latency_ms` 并继续触发 `AdaptiveTlsTiming/Fallback`；关闭 IP 池时需同步评估 P3 自动禁用与指标的观测空洞；黑白名单/熔断策略依赖 P2 的任务配置热加载能力。
  - P7 工作区/批量/子模块逻辑仅在 `workspace.enabled=true` 时激活；批量调度复用任务注册器与进度事件，不改变 MP0-P6 任务语义；子模块递归克隆建立在现有 `TaskKind::GitClone` 之后附加阶段（70-85-100%）；团队配置模板导入仅写入配置文件与内存运行态，不影响已存在 TLS/IP/代理/凭证模块的回退路径；跨仓库状态服务读取仓库索引，不修改 Git 操作代码。
  - 跨模块影响：
    - 代理启用（P5）时不影响工作区/批量逻辑；批量任务内部仍复用 Git 传输层现有代理互斥策略（自定义传输与 Fake SNI 已由前序逻辑屏蔽）。
    - 凭证存储（P6）自动为批量 clone/push 子任务统一回调，无需在批量请求中重复提供凭证；子模块操作沿用主仓库凭证。
    - IP 池（P4）与工作区解耦：批量任务底层仍按单仓 Git 任务路径调用；若 IP 池禁用不会影响 Workspace 元数据与任务调度。
    - 模板导入仅覆盖配置，不直接触发批量/子模块任务；导入后需要手动 reload 或下一次任务读取时生效。
- **回滚指引**：
  - 生产事故时优先通过配置禁用新增功能（Push、策略覆盖、Fake SNI、指标采集等）；
  - 若需降级二进制，参考 `src-tauri/_archive` 中的 legacy 实现及各阶段 handoff 文档的“回退矩阵”。
  - P7 回退：关闭 `workspace.enabled` 即可整体禁用工作区、批量与子模块 UI 入口；如仅批量操作异常，可下调 `workspace.maxConcurrentRepos=1` 退化为顺序；子模块异常时禁用递归（`submodule.autoInitOnClone=false`，手动操作可用）；模板导入风险时避免执行 `import_team_config_template` 并保留自动备份回滚；状态服务异常时将 `workspace.statusAutoRefreshSecs=0` 并清空缓存。
- **交接资料**：
  - 每次版本发布前更新 `CHANGELOG.md`、`new-doc/IMPLEMENTATION_OVERVIEW.md` 与对应的 handoff 文档；
  - 附上最新 soak 报告、配置快照、事件截图，供下游团队复用。

---

## 4. 阶段详情

### 4.1 MP0 - git2-rs 基线

- **目标**：替换 gitoxide 实现，保持前端行为；
- **关键实现**：
  - `GitService` 使用 git2-rs，桥接 `transfer_progress` 与 `checkout` 回调；
  - `TaskRegistry` 统一取消 token（`ErrorCode::User` -> Cancel）；
  - 错误分类：Network/Tls/Verify/Protocol/Auth/Cancel/Internal；
  - HTTP Fake 调试接口沿用，提供白名单/重定向/脱敏；
- **配置**：`AppConfig` 热加载 `logging.authHeaderMasked`；原先的 `tls.sanWhitelist` 白名单在 v1.8 起被移除，改由 `http.fakeSniTargetHosts` 控制改写范围。
- **测试**：`cargo test` 与 `pnpm test` 全部通过，git2-rs 在 Windows 确认可构建；
- **限制**：无 push、无浅/部分克隆、无策略覆盖。
- **后端细节**：
  - Clone/Fetch 分别在独立工作线程执行，`TaskRegistry` 通过 `std::thread::spawn` 搭配 `CancellationToken` 协调取消；
  - 进度换算：Checkout 阶段在 `default_impl/ops.rs::do_clone` 中将 git2 的 0-100 线性映射到全局 90-100（90 + percent * 0.1），保持前端进度条平滑；
  - 错误映射集中在 `default_impl/errors.rs`，方便后续阶段扩展错误分类而不影响调用方；
  - HTTP Fake API 通过 `reqwest` 自行发起请求，与 git2 传输栈隔离。
- **前端配合**：`src/api/tasks.ts` 中的事件归一化函数需处理 MP0 仍然包含的 snake_case 字段（`total_hint`），在 MP1+ 中继续沿用；
- **交接要点**：
  - 如需回滚到 gitoxide 版本，可使用 `src-tauri/_archive/default_impl.legacy_*` 中的旧实现，编译时需重新启用 `gix` 相关依赖（非推荐，仅供紧急回退）。
  - 修改 git2 版本前务必在 Windows + macOS 双平台运行 `cargo test -q` 验证动态库加载。
- **交接 checklist**：
  - 对照 `new-doc/MP0_IMPLEMENTATION_HANDOFF.md` §2 的代码结构，确认 `core/git/default_impl.rs`、`core/tasks/registry.rs` 的负责人，并在交接记录中注明。
  - 每次发版前手动执行 `pnpm test -s` 与 `cargo test -q`；同时用 `http_fake_request` 校验白名单与脱敏日志，确保调试工具保持可用。
  - 若需要临时回滚到 gitoxide 版本，提前验证 `_archive/default_impl.legacy_*` 分支仍能通过最小 smoke，避免紧急恢复时缺乏可用包。

### 4.2 MP1 - Push + Subtransport + Retry + 事件增强

- **新增能力**：
  - `git_push` 命令与 Push 任务，凭证回调支持用户名/密码（PAT）；
  - Push 进度阶段化 `PreUpload|Upload|PostReceive`，进度事件含对象/字节；
  - `task://error` 引入，分类 `Network|Tls|Verify|Protocol|Proxy|Auth|Cancel|Internal`；
  - Retry v1：指数退避 + 抖动，`Upload` 阶段后不自动重试；
  - 自定义 smart subtransport 方式A：接管连接/TLS/SNI，代理场景自动禁用，Fake->Real->libgit2 回退链；
  - Push 特化：TLS 层注入 Authorization，401 -> Auth 分类，403 info/refs 阶段触发一次性 SNI 轮换；
- **配置热加载**：
  - `http.fakeSniEnabled`、`http.fakeSniHosts`、`http.sniRotateOn403`；
  - `retry.max`/`baseMs`/`factor`/`jitter`；
  - `proxy.mode|url`；`logging.debugAuthLogging`（脱敏开关）；
- **前端改动**：
  - GitPanel 支持 Push 表单（凭证、TLS/SNI 策略编辑）；
  - 全局错误列显示分类 Badge + 重试次数；
- **回退策略**：参考附录C 步骤9（Push/Retry 回退）与步骤10（最终关闭可观测性）。阶段特有：方式A 自带 Fake→Real→libgit2 链，无需人工；仅需配置禁用 Push/方式A/Retry 即可回落 MP0 基线。
- **测试**：Push 集成测试、事件 casing 测试（snake/camel 兼容），Retry 指数退避测试。
- **后端细节**：
  - Push 使用 `RemoteCallbacks::credentials` 结合 Tauri 命令提供的用户名/密码，回调在每次授权失败时重试；
  - `push_transfer_progress` 回调仅在服务器支持时触发，若无该回调则通过 libgit2 上传阶段对象/字节推估进度；
  - Retry v1 位于 `TaskRegistry::run_with_retry`，根据 `ErrorCategory` 判定是否 `is_retryable`，重试间隔计算函数在 `retry/backoff.rs`，使用 `rand` 抖动避免雪崩；
  - 方式A 的重写逻辑 `transport/rewrite.rs` 仅改写 `https://` 前缀，保留查询参数与 fragment；`transport/runtime.rs` 按代理模式决定是否禁用 Fake；
  - Authorization 注入通过线程局部 `AUTH_HEADER_OVERRIDE` 在 TLS 层读取，确保不会泄漏到非 Push 请求。
- **前端/服务配合**：
  - `src/stores/tasks.ts` 中的 `setLastError` 兼容 `retried_times` 与 `retriedTimes`；
  - Push 表单存储的凭证仅保存在内存，取消或完成后主动清空，避免残留。
- **交接要点**：
  - 若需在后续阶段扩展 Push 认证方式（如 OAuth 设备码），需扩展 `CredentialProvider` 接口并更新 Tauri 命令签名；
  - 方式A 白名单需在 `http.fakeSniTargetHosts`（或 `hostAllowListExtra`）中维护；证书校验直接按真实域名进行，无需同步额外的 SAN 列表。
  - Retry 参数调整需同时更新前端提示文本，保持用户对重试次数与耗时的认知。
- **关键文件定位**：
  - 传输层：`src-tauri/src/core/git/transport/rewrite.rs`（改写决策）、`transport/runtime.rs`（Fake/Real 回退）、`transport/streams.rs`（方式A IO 桥接）、`transport/auth.rs`（Authorization 注入）。
  - 任务与重试：`src-tauri/src/core/tasks/registry.rs`（`run_with_retry`、事件发射）、`src-tauri/src/core/tasks/model.rs`（任务/事件载荷结构）。
  - 前端落点：`src/views/GitPanel.vue`（Push 表单、TLS/SNI 编辑）、`src/views/__tests__/git-panel.error.test.ts`（错误列校验）。
- **事件契约补充**：
  - `task://error` 的 `message` 以人类可读文本呈现，`code` 预留（Retry v1 暂未使用）；`retriedTimes` 仅在自动重试后出现，Push 上传阶段不再自增。
  - Push 进度事件固定 `phase` 顺序 `PreUpload -> Upload -> PostReceive`，当服务器不提供回调时 `objects`/`bytes` 可能缺失。
  - 方式A 失败回退会通过一次性 `task://error` 将原因标记为 `Proxy` 或 `Tls`，便于前端展示提示。
- **常见故障排查**：
  - Push 401/403：检查 PAT/组织 SSO；Inspect `task://error` `category=Auth`，必要时启用 `logging.debugAuthLogging`（仍脱敏）。
  - TLS/Verify 失败：确认目标域证书覆盖真实域名，并检查 `http.fakeSniTargetHosts` 是否包含该域；代理模式下默认禁用 Fake。
  - 进度停滞：若 Upload 阶段长时间无变化，提示手动取消并重试；事件中 `retriedTimes` 不再增长属于预期表现。
- **交接 checklist**：
  - 评估 Push/方式A rollout 前，在预生产逐项验证：正常 Push、401/403、代理透传、取消路径、Retry 关闭/开启效果。
  - 更新 `new-doc/MP1_IMPLEMENTATION_HANDOFF.md` 中的白名单与凭证说明，确保 SRE 拥有最新连接策略。
  - 前端确认 `GitPanel` 凭证输入不落盘，并在发布说明中告知新事件字段，便于文档同步。

### 4.3 P2 - 本地操作与策略扩展

- **目标**：
  - 扩充常用本地 Git 操作（commit/branch/checkout/tag/remote），保持与任务系统一致的生命周期与错误语义；
  - 为 Clone/Fetch/Push 提供 shallow/partial 能力与任务级 HTTP/Retry 覆盖；
  - 通过结构化事件与护栏反馈避免误配置引入的隐性风险。
- **关键实现**：
  - 新增命令 `git_commit`、`git_branch`、`git_checkout`、`git_tag`、`git_remote_add|set|remove`，在 `core/tasks/git_registry` 内映射为对应 TaskKind；
  - `core/git/default_impl/opts.rs` 统一解析 `depth` / `filter` / `strategyOverride`，输出 `StrategyOverrideParseResult`，记录未知字段；
  - `helpers::apply_http_override` / `helpers::apply_retry_override`（`core/tasks/git_registry/helpers.rs`）在任务内合并覆盖并返回差异标记；
  - StructuredEvent 总线发出 `Strategy::HttpApplied/Conflict/IgnoredFields/Summary`、`Transport::PartialFilterFallback`、`Policy::RetryApplied`（Clone/Push）等事件；其中 `Strategy::Conflict` 仅由 Clone 触发，Push 为兼容现有前端仍额外发送一条 `task://error` 信息事件描述 HTTP 冲突；
  - `TaskRegistry::emit_strategy_summary` 统一生成 `Strategy::Summary` 事件，包含最终 HTTP/Retry 参数与 `applied_codes`；Fetch 虽不发 `Policy::RetryApplied`，但会在 Summary 中保留 `retry_strategy_override_applied` 字符串。
- **配置与能力开关**：
  - `strategyOverride.http` 仅支持 `followRedirects` / `maxRedirects`，`retry` 支持 `max` / `baseMs` / `factor` / `jitter`；其他字段将被忽略并记入 `IgnoredFields`；
  - `FWC_PARTIAL_FILTER_SUPPORTED=1` 或 `FWC_PARTIAL_FILTER_CAPABLE=1` 声明环境支持 partial filter；未设置或设为 0 时直接走 fallback 并发出 `Transport::PartialFilterFallback`；
  - 数值区间在解析阶段校验：`maxRedirects ≤ 20`，`retry.max ∈ [1,20]`，`retry.baseMs ∈ [10,60000]`，`retry.factor ∈ [0.5,10]` 等。
- **测试矩阵**：
  - `git_clone_partial_filter.rs`、`git_fetch_partial_filter.rs` 验证 capability 缓存与 fallback 事件；
  - `git_strategy_and_override.rs` 锁定 HTTP/Retry 覆盖、冲突、ignored、结构化 Summary 序列；
  - `git_tag_and_remote.rs`、`git_branch_and_checkout.rs` 覆盖本地操作的成功/失败/取消路径；
  - `quality/error_and_i18n.rs` 属性测试枚举策略解析边界与错误分类。
- **后端细节**：
  - 解析层记录 `ignored_top_level` 与 `ignored_nested(section.key)`，任务层将其转化为 `Strategy::IgnoredFields`；
  - HTTP 覆盖冲突（`followRedirects=false && maxRedirects>0`）在 Clone 发布 `Strategy::Conflict`，在 Push 同时写入信息级 `TaskErrorEvent`；Fetch 仅规范化后继续执行；
  - Retry 覆盖调用 `load_retry_plan` + `compute_retry_diff`，差异字段在 `Policy::RetryApplied.changed` 中列出；
  - Partial filter fallback 事件记录 `shallow` 布尔值，调用方仍可通过返回状态决定是否提示用户。
- **数据模型与事件顺序**：
  - 结构化事件按 applied → conflict → ignored → partial fallback → summary 顺序发送，测试中使用内存事件总线断言；
  - `Strategy::Summary` 包含 `retry_*` 数值与 `applied_codes` 列表（如 `http_strategy_override_applied`），前端无需再从 `task://error` 聚合；
  - `TaskRegistry::decide_partial_fallback` 返回 `(message, shallow)`，用于统一的 fallback 事件。
- **护栏策略**：
  - HTTP 冲突自动回落到安全值（`maxRedirects=0`），并通过事件提示；
  - Retry 覆盖限制范围，越界即 Protocol 错误；
  - TLS 覆盖开关已自 P2 起移除，Real Host 与 SPKI 校验在 P3 实现且不可配置关闭。
- **常见故障排查**：
  - 若持续收到 `Strategy::Conflict`，检查 UI 是否同时设置 `followRedirects=false` 与正数 `maxRedirects`；
  - 频繁的 `Transport::PartialFilterFallback` 说明远端不支持 partial filter，可将相关环境变量置 0 以减少探测；
  - Push 任务若出现信息级冲突事件但无结构化冲突，属于兼容模式，日志仍可定位最终跟随策略。
- **交接 checklist**：
  - 与前端确认是否需要消费结构化事件（可通过 `events::structured::set_global_event_bus` 或 `MemoryEventBus` 订阅）；
  - 为新增策略字段补充解析、护栏、Summary、事件与测试断言；
  - 运维手册仅需记录 `FWC_PARTIAL_FILTER_SUPPORTED` / `FWC_PARTIAL_FILTER_CAPABLE` 的默认值与调整流程。
- **前端/服务配合**：
  - `GitPanel` 继续透传 `strategyOverride`，Summary 工具需从结构化事件或 `applied_codes` 中读取变更；
  - Task store 仍保存 `lastError` 以兼容 Push 冲突提示，其他策略提示建议改为订阅结构化事件；
  - 前端单测 `strategy-override.events.test.ts` 可升级为模拟结构化事件流以减少对 legacy 错误通道的依赖。
- **交接要点**：
  - 调整覆盖逻辑时务必同步更新 `core/tasks/git_registry/helpers.rs`、`git_strategy_and_override.rs` 及文档；
  - 引入新的 capability provider 需扩展 `runtime_config` 与 fallback 逻辑，并更新测试缓存键；
  - 回退策略：移除新增 TaskKind 或跳过策略解析，仍可保持 Clone/Fetch/Push 正常执行。

### 4.4 P3 - 自适应 TLS 与可观测性强化

- **目标**：
  - 将方式A 自适应 TLS 全量 rollout，并提供可观测性与自动护栏；
  - 强化指纹、时延与回退事件，便于运维审计；
  - 引入 Soak 报告确保长时运行稳定性。
- **关键实现**：
  - `transport/rewrite.rs` 执行 Fake SNI 改写与采样决策，记录 `AdaptiveTlsRollout` 事件；
  - `transport/runtime.rs` 维护 Fake->Real->Default fallback 状态机，并触发 `AdaptiveTlsFallback`；
  - `transport/metrics.rs` 的 `TimingRecorder` 与 `fingerprint.rs` 的日志逻辑在任务结束时统一 flush 事件；
  - 自动禁用窗口根据失败率触发 `AdaptiveTlsAutoDisable`，冷却后自动恢复；
  - Soak runner (`src-tauri/src/soak`) 以环境变量驱动迭代运行并生成报告。
  - `RealHostCertVerifier` 始终在 Fake SNI 场景下以真实域名调用内层 `ServerCertVerifier`（若 override 无法解析则自动回退到原始 SNI），避免证书链与 SAN 校验对伪域妥协；同时承担当前阶段的 SPKI pin 校验（真实 SNI 路径尚未安装该包装器）。
- **配置与指标**：
  - 关键项：`http.fakeSniEnabled`、`http.fakeSniRolloutPercent`、`http.autoDisableFakeThresholdPct`、`http.autoDisableFakeCooldownSec`、`tls.metricsEnabled`、`tls.certFpLogEnabled`、`tls.spkiPins`；
  - `tls.realHostVerifyEnabled` 已从配置模型、团队模板与前端类型中移除：Real host 校验不可再通过配置关闭，前后端保持一致。
  - 指纹日志写入 `cert-fp.log`，滚动阈值由 `tls.certFpMaxBytes` 控制；
  - 环境变量：`FWC_TEST_FORCE_METRICS` 强制指标采集，`FWC_ADAPTIVE_TLS_SOAK` 和 `FWC_SOAK_*` 控制 soak。
- **测试矩阵**：
  - `transport/rewrite.rs` 单测覆盖 0%/10%/100% 采样与 URL 处理；
  - `transport/runtime.rs` 单测验证 fallback 状态机与 auto disable；
  - `tls/verifier.rs` 单测覆盖 Fake SNI 路径下的 Real host 校验与 SPKI pin（真实 SNI 尚未启用 pin 校验），`events/events_structure_and_contract.rs` 新增 `override_host_enforced_during_fake_sni` 用例锁定 Fake SNI 覆盖真实域名校验；
  - Soak 模块单测确保报告生成、基线对比与阈值判定。
- **后端细节**：
  - `RewriteDecision` 使用 host + path 的稳定哈希决定 `sampled`，同一仓库在不同任务中行为一致；
  - `TimingRecorder` 捕获 connect_ms/tls_ms/first_byte_ms/total_ms（毫秒），并在任务完成时产生单一 `AdaptiveTlsTiming` 事件；
  - 指纹缓存 LRU 记录 512 个 host，24 小时内变化才会触发 `CertFingerprintChanged`；
  - 自动禁用窗口 `SAMPLE_CAP=20`，最少样本 `MIN_SAMPLES=5`，触发后立即清空窗口并记录 `enabled=false/true` 两个事件；
  - Real host 验证失败视为 Verify 类错误，同时触发 Fake->Real fallback 统计；该校验现为强制行为，无需也无法通过配置关闭（旧配置项被忽略）。
- **模块映射与代码指针**：
  - `transport/metrics.rs`（TimingRecorder）、`transport/fingerprint.rs`（指纹日志）、`transport/fallback.rs`（状态机）、`transport/runtime.rs`（自动禁用/状态协调）。
  - TLS 验证集中在 `src-tauri/src/core/tls/verifier.rs`：同时负责 Real host 与 SPKI pin；测试样例位于同路径 `tests` 模块。
  - Soak 入口 `src-tauri/src/soak/mod.rs`，报告结构与基线比对实现位于同目录；`soak/README.md` 列出运行参数。
- **事件 & 指标字段细化**：
  - `AdaptiveTlsRollout` 字段：`percent_applied`、`sampled`、`eligible`，用于确认采样策略是否命中；搭配监控可量化 rollout 覆盖面。
  - `AdaptiveTlsTiming` 仅在 `tls.metricsEnabled=true` 时发送；`cert_fp_changed=true` 表示 24 小时窗口内指纹发生更新。
  - `AdaptiveTlsAutoDisable` 在触发与恢复时分别发送一次，`enabled=false` 表示 Fake SNI 被暂停；结合日志可定位原因。
- **常见故障排查**：
  - Fake 回退频繁：查看 `AdaptiveTlsFallback` 中的 `reason`，若为 `FakeHandshakeError`，多为目标域证书或 CA 链问题；确认证书已覆盖真实域名后再恢复 rollout。
  - 指纹 mismatch：事件 `CertFpPinMismatch` 出现后应立即核对 `tls.spkiPins`；如误配导致大面积失败，临时清空 pin 后重新采集。
  - 自动禁用 oscillation：检查失败率阈值是否过低，或 Soak 报告中是否存在网络抖动；必要时提高 `autoDisableFakeThresholdPct`。
- **交接 checklist**：
  - 发布前确认 `cert-fp.log` 滚动机制与磁盘配额，必要时在运维标准中加入归档脚本。
  - 调整 rollout 百分比时，更新监控告警阈值并记录在 `new-doc/P3_IMPLEMENTATION_HANDOFF.md` 交接表。
  - Soak 报告归档到 `doc/` 目录并附在发布邮件，确保后续回溯有依据。
- **前端/服务配合**：
  - GitPanel 监听 `AdaptiveTls*` 信息事件并折叠展示关键字段，未开启 UI 时仍可在全局日志查看；
  - 指纹事件在 UI 中标记为敏感，仅显示哈希前缀；
  - Soak 报告默认输出至项目根目录，可在运维脚本中采集并上报。
- **交接要点**：
  - 调整 rollout 百分比需同步更新监控告警阈值，推荐 0 -> 25 -> 50 -> 100 渐进策略；
  - 新增 SPKI pin 前需先通过 `cert-fp.log` 获取现有指纹，避免误配置导致 Verify 失败；
  - Auto disable 阈值或冷却时间变化后请执行短程 soak，确认不会反复触发；
  - 指纹日志与 soak 报告包含敏感信息，导出前确保 `logging.authHeaderMasked` 与脱敏策略开启。
- **回退策略**：参考附录C 步骤7（Fake SNI / 指纹相关降级）与步骤10（完全关闭观测）。阶段特有：若需 legacy 传输实现可使用 `transport/_archive`，仅在指纹/验证逻辑持续异常时采用（临时）。

### 4.5 P4 - IP 池与握手优选

  - **目标**：
    - 为指定域名和按需域名收集多来源 IP 候选，通过 TCP 握手延迟排序选择最佳连接；
    - 保持缓存 TTL、容量与历史持久化，确保网络变化时能快速刷新或回退；
    - 与传输层、自适应 TLS、观测体系打通，并在异常场景下提供熔断和全局禁用能力；
    - 提供 Soak 阈值和报告扩展，为灰度和准入提供量化依据。
  - **核心模块**：
    - `IpPool` 统一封装 pick/report/maintenance/config 接口；
    - `IpScoreCache`（内存缓存）+ `IpHistoryStore`（磁盘 `ip-history.json`）负责 TTL、容量、降级处理；
    - `PreheatService` 独立 tokio runtime 调度多来源采样（Builtin/UserStatic/History/DNS/Fallback），指数退避与手动刷新并存；`dns` 子模块负责解析器池、DoH/DoT/UDP 预设与系统 DNS 协调；
    - `custom_https_subtransport` 以延迟优先顺序尝试候选，失败后回退系统 DNS，并记录 `IpPoolSelection` 事件及线程本地埋点；
    - `circuit_breaker` + 黑白名单 + 全局 `auto_disabled_until` 联合管理熔断与禁用，事件 `IpPoolIpTripped/Recovered`、`IpPoolAutoDisable/Enable` 反映状态；
    - `core/ip_pool/events.rs` 集中封装所有新事件、保证测试可注入总线。
  - **配置与默认值**：
  - 运行期（`config.json`）：`ip_pool.enabled=true`、`dns`（`useSystem=true`、`resolvers=[]`、`presetCatalog` 内置 Cloudflare/Google/Aliyun/Quad9 等 DoH/DoT/UDP 预设、`enabledPresets` 自动筛除 `desc="不可用"` 项）、`maxParallelProbes=4`、`probeTimeoutMs=1500`、`cachePruneIntervalSecs=60`、`maxCacheEntries=256`、`singleflightTimeoutMs=10000`、熔断窗口 (`failureThreshold=3`、`failureRateThreshold=0.5`、`failureWindowSeconds=60`、`minSamplesInWindow=5`、`cooldownSeconds=300`、`circuitBreakerEnabled=true`)；
    - 文件（`ip-config.json`）：`preheatDomains=[]`、`scoreTtlSeconds=300`、`maxParallelProbes=4`、`probeTimeoutMs=3000`、`userStatic=[]`、`blacklist/whitelist=[]`、`disabledBuiltinPreheat=[]`、`historyPath="ip-history.json"`；所有字段热更新后立即重建预热计划与熔断状态。
  - **运行生命周期**：
    1. 应用启动加载配置 -> 构建 `IpPool` -> `PreheatService::spawn` 若启用则拉起后台 runtime；
    2. 预热循环按域调度采样，写入缓存与历史，并发 `IpPoolRefresh`；
    3. 任务阶段 `pick_best` 优先命中缓存，否则 `ensure_sampled` 同域单飞采样；
  4. `report_outcome` 回写成功/失败，为熔断统计提供数据；
  5. 异步桥回写语义：`report_outcome_async(selection, outcome)` 为 fire-and-forget 调用，语义上保证将在后台单线程 runtime 中执行 `IpPool::report_outcome` 或 `report_candidate_outcome`（若 selection 包含 candidate 细粒度信息）。调用方不得假定同步完成；若需要同步确认（仅在测试或特殊检查场景），可在 bridge 层新增一个确认通道（oneshot）并在调用点等待回应（当前未启用以减少路径复杂度）。
    5. `maybe_prune_cache` 按 `cachePruneIntervalSecs` 清理过期与超额条目，同时调用 `history.prune_and_enforce`；
    6. 持续失败或运维干预触发 `set_auto_disabled`，冷却到期 `clear_auto_disabled` 自动恢复。
  - **预热调度细节**：
    - `DomainSchedule` 维护 `next_due`、`failure_streak` 与指数退避（封顶 6×TTL），热更新与 `request_refresh` 会立即重置；
    - 候选收集 `collect_candidates` 合并五类来源，白名单优先保留、黑名单直接剔除并发 `IpPoolCidrFilter`；
    - 内置预热域可通过 `disabledBuiltinPreheat` 精确禁用；DNS 预设与自定义解析器按 `dns.enabledPresets` 与 `dns.resolvers` 合并，支持 DoH/DoT/UDP；
    - `measure_candidates` 受信号量限制并发数，`probe_latency` 根据配置截断超时，成功/失败均写 `ip_pool` target 日志；
    - 当所有域达到失败阈值时执行 `set_auto_disabled("preheat consecutive failures", cooldown)` 并进入冷却；
    - 预热成功/失败均会发 `IpPoolRefresh` 事件（`reason=preheat/no_candidates/all_probes_failed`）。
  - **按需采样与缓存维护**：
    - `ensure_sampled` 使用 `Notify` 单飞避免同域重复采样，超时（默认 10s）后回落系统 DNS；
    - `sample_once` 复用预热逻辑，成功写回缓存与历史；
    - `maybe_prune_cache` 清理过期条目、执行 LRU 式容量淘汰，再调用 `history.prune_and_enforce(now, max(maxCacheEntries, 128))`；
    - `IpHistoryStore` 持久化失败时降级为内存模式，仅记录 `warn`，运行期不受阻塞；
    - `auto_disable_extends_without_duplicate_events` 回归测试确保冷却延长不重复发 disable 事件，`clear_auto_disabled` 仅在状态切换时广播 enable。
  - **传输层集成**：
  - `acquire_ip_or_block` 返回按延迟排序的候选 snapshot，逐一尝试并通过 `report_candidate_outcome` 记录；
  - 成功/失败均通过 `IpPoolSelection`、线程局部 `ip_source`/`ip_latency_ms` 反馈给 `AdaptiveTlsTiming/Fallback`；
  - 阻塞接口 `pick_best_blocking` 通过 `OnceLock` 懒初始化一个名为 `ip-pool-blocking` 的两线程 Tokio runtime，并在该 runtime 上 `block_on(self.pick_best(...))`；初始化失败时退回 `IpSelection::system_default`。该路径不再试图探测外部 runtime，只依赖内部共享 runtime，避免在调用方线程重复创建多线程 runtime。
    具体实现：代码先调用 `Handle::try_current()`，若成功则记录 debug 日志并返回 `IpSelection::system_default(...)`；否则尝试从内部 `blocking_runtime()`（OnceLock 保存的 multi-thread runtime）中取出 runtime 并使用 `rt.block_on(self.pick_best(...))`，若该 runtime 初始化失败也回退为 system default。
  - IP 池禁用或候选耗尽时事件中的 `strategy=SystemDefault`，前端可据此回退展示。
  - **异常治理**：
    - `CircuitBreaker::record_outcome` 基于滑动窗口判定并发 `IpPoolIpTripped/Recovered`；
    - 黑白名单从配置热更新后即时生效，过滤结果通过 `IpPoolCidrFilter` 记录；
    - 全局自动禁用采用 CAS/Swap 保证幂等，冷却中延长仅写 debug，恢复只发一次 `IpPoolAutoEnable`；
    - 事件辅助 `event_bus_thread_safety_and_replacement` 测试覆盖并发场景，确保不会丢失或重复。
  - **观测与数据**：
    - 事件：`IpPoolSelection`（strategy/source/latency/candidates）、`IpPoolRefresh`（success/min/max/原因）、`IpPoolConfigUpdate`、熔断/禁用/CIDR；
    - `AdaptiveTlsTiming/Fallback` 新增 ip 字段，与 P3 事件共享线程局部；
    - 快照新增 `preheatTargets/preheatedTargets` 统计，Check View 通过 `waitForIpPoolWarmup` 展示预热覆盖率；
    - `ip-history.json` 超过 1 MiB 记录警告；`prune_and_enforce` 在维护周期内统一清理；
    - Soak 报告新增 `ip_pool` 统计（selection_total/by_strategy、refresh_success/failure、success_rate）。
  - **Soak 与阈值**：
    - 环境变量 `FWC_ADAPTIVE_TLS_SOAK=1`、`FWC_SOAK_MIN_IP_POOL_REFRESH_RATE`、`FWC_SOAK_MAX_AUTO_DISABLE`、`FWC_SOAK_MIN_LATENCY_IMPROVEMENT` 等可调；
    - 报告 `thresholds` 判断 ready 状态，`comparison` 对比基线（成功率、回退率、IP 池刷新率、自动禁用次数、延迟改善）；
    - 无基线时自动标记 `not_applicable` 并写入原因。
  - **测试矩阵**：
    - 单元：`preheat.rs`、`history.rs`、`mod.rs`（缓存/单飞/TTL）、`circuit_breaker.rs`、`events.rs`；
    - 集成：`tests/commands/ip_pool_commands.rs`、`tests/tasks/ip_pool_manager.rs`、`ip_pool_preheat_events.rs`、`ip_pool_event_emit.rs`、`ip_pool_event_edge.rs`、`events_backward_compat.rs`；
    - 前端：`src/api/__tests__/ip-pool.api.test.ts`、`src/utils/__tests__/check-preheat.test.ts`、`src/views/__tests__/check-view.test.ts` 验证 API、预热助手与 UI 交互；
    - Soak：`src-tauri/src/soak/mod.rs` 对阈值、报告、基线比较和环境变量覆盖提供测试；
    - 全量回归：`cargo test -q --manifest-path src-tauri/Cargo.toml`、前端 `pnpm test -s`。
  - **命令与前端**：
    - 新增 Tauri 命令 `ip_pool_get_snapshot`/`ip_pool_update_config`/`ip_pool_request_refresh`/`ip_pool_start_preheater`/`ip_pool_clear_auto_disabled`/`ip_pool_pick_best`，`setup.rs` 默认注册；
    - 前端新增 `src/api/ip-pool.ts` 封装上述命令、`IpPoolLab.vue` 提供运行期/文件配置编辑、DNS 解析器管理与候选调试界面，导航栏增加“IP 池实验室”入口；
    - 配套类型定义补充至 `src/api/config.ts`（`DnsRuntimeConfig`/`DnsResolverConfig` 等）并在 `IpPoolLab` 及配置表单中使用；
    - `CheckView.vue` 在环境预热流程中调用 `startIpPoolPreheater`+`waitForIpPoolWarmup`，向用户展示预热进度与跳过原因；
  - **运维要点与故障排查**：
    - 快速禁用：`ip_pool.enabled=false` 或停用预热线程，新任务立即回退系统 DNS；
    - 手动预热：通过 `ip_pool_start_preheater` 或 `ip_pool_request_refresh` 触发即时采样，结合前端实验室/Check View 观察覆盖率；
    - 黑白名单：更新 `ip-config.json` 后调用 `request_refresh` 即时生效，事件中保留被过滤 IP 与 CIDR；
    - 自动禁用：观察 `IpPoolAutoDisable`/`IpPoolAutoEnable` 与日志，必要时手动调用 `clear_auto_disabled` 或调整 `cooldownSeconds`；
    - 历史异常：删除损坏的 `ip-history.json` 会自动重建，日志含 `failed to load ip history`；
    - 调试：`RUST_LOG=ip_pool=debug` 打开预热/候选/退避细节；Soak 报告 `ip_pool.refresh_success_rate` < 阈值时重点排查网络连通性。
  - **交接 checklist**：
    - 发布前确认 `ip-config.json`/`config.json` 的 IP 池字段（TTL、并发、黑白名单、熔断阈值）与预期一致；
    - 运行 `cargo test --test ip_pool_manager`、`--test ip_pool_preheat_events`、`--test events_backward_compat` 快速验证集成与事件向后兼容；
    - 所有灰度环境需保留最新 soak 报告与 `selection_by_strategy` 指标截图，供准入评审；
    - 告警体系需新增 IP 池类事件（刷新失败率、auto disable、熔断）监控，防止观测盲点；
    - 运维手册应补充黑白名单维护与历史文件巡检 SOP。

### 4.6 P5 - 代理支持与自动降级

  - **目标**：
    - 支持 HTTP/HTTPS、SOCKS5 和系统代理，提供统一配置与管理接口；
    - 实现代理失败的自动降级直连与健康检查恢复机制；
    - 与 Fake SNI、IP 优选等既有策略保持互斥，确保网络环境适配性；
    - 提供跨平台系统代理检测（Windows/macOS/Linux）与前端集成。
  - **核心模块**：
    - `ProxyManager` 统一封装模式、状态、连接器、健康检查、配置热更新；
    - `HttpProxyConnector`（CONNECT隧道、Basic Auth）与 `Socks5ProxyConnector`（协议握手、认证方法、地址类型）实现 `ProxyConnector` trait；
    - `ProxyFailureDetector` 滑动窗口统计失败率，触发自动降级并发 `ProxyFallbackEvent`；
    - `ProxyHealthChecker` 后台定期探测（默认60秒），连续成功达阈值后触发自动恢复并发 `ProxyRecoveredEvent`；
    - `SystemProxyDetector` 跨平台检测系统代理（Windows注册表/macOS scutil/Linux环境变量）；
    - 传输层集成：`register.rs` 检查代理配置，启用时跳过自定义传输层注册，强制使用 libgit2 默认 HTTP 传输。
  - **配置与默认值**：
    - 运行期（`config.json`）：`proxy.mode=off`（off/http/socks5/system）、`url=""`、`username/password=null`、`disableCustomTransport=false`（代理启用时强制true）、`timeoutSeconds=30`、`fallbackThreshold=0.2`、`fallbackWindowSeconds=300`、`recoveryCooldownSeconds=300`、`healthCheckIntervalSeconds=60`、`recoveryStrategy="consecutive"`、`probeUrl="www.github.com:443"`（host:port格式）、`probeTimeoutSeconds=10`、`recoveryConsecutiveThreshold=3`、`debugProxyLogging=false`；
    - 所有字段支持热更新，通过重新创建 `ProxyManager` 实例生效。
  - **运行生命周期**：
    1. 应用启动加载配置 -> 创建 `ProxyManager` -> 初始化失败检测器和健康检查器；
    2. 传输层注册时调用 `should_skip_custom_transport()`，代理启用则跳过 `https+custom` 注册；
    3. 任务阶段 `get_connector()` 返回对应连接器（HTTP/SOCKS5），建立隧道并报告结果；
    4. `report_failure()` 更新滑动窗口，失败率超阈值触发 `trigger_automatic_fallback()`；
    5. 后台健康检查定期探测，连续成功达阈值触发 `trigger_automatic_recovery()`；
    6. 冷却窗口结束后自动清除禁用状态，恢复代理模式；
    7. 状态机转换规则：Disabled↔Enabled（启用/禁用）、Enabled→Fallback（失败降级）、Fallback→Recovering（开始恢复）、Recovering→Enabled（恢复成功）或Recovering→Fallback（恢复失败），所有转换通过 `can_transition_to()` 验证。
  - **强制互斥策略**：
    - 代理启用时 `ProxyManager::should_disable_custom_transport()` 强制返回 `true`（检查 `is_enabled()` 且 `mode != Off`）；
    - 传输层注册阶段 `register.rs::should_skip_custom_transport()` 创建临时 `ProxyManager` 检查配置，若应禁用则直接返回 `Ok(())`，跳过 `https+custom` 注册；
    - 同时通过 `tl_set_proxy_usage()` 记录代理使用状态到线程局部metrics，供传输层和观测系统使用；
    - 结果：代理模式下不使用 Fake SNI、IP 优选，直接使用 libgit2 默认 HTTP 传输（真实SNI），避免复杂度叠加和潜在冲突。
  - **协议实现细节**：
    - HTTP CONNECT：构造 `CONNECT host:port HTTP/1.1`，解析 200/407（需认证）/502（网关错误）响应，支持 Basic Auth（Base64编码 `username:password`）；超时通过 `TcpStream::set_read_timeout()` 和 `set_write_timeout()` 控制；
    - SOCKS5：版本协商（0x05）-> 认证（No Auth 0x00 / Username/Password 0x02）-> CONNECT请求（CMD=0x01），支持 IPv4（ATYP=0x01）/IPv6（ATYP=0x04）/域名（ATYP=0x03）地址类型，映射 REP 错误码（0x01-0x08：通用失败/规则禁止/网络不可达/主机不可达/连接拒绝/TTL超时/命令不支持/地址类型不支持）；
    - 系统检测：Windows 读取注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings` 的 `ProxyEnable` 和 `ProxyServer` 字段，macOS 执行 `scutil --proxy` 并解析输出，Linux 检测 `HTTPS_PROXY`/`HTTP_PROXY` 环境变量（按优先级）；
    - 错误分类：`ProxyError` 包含5个变体（Network/Auth/Proxy/Timeout/Config），每个错误通过 `category()` 方法返回分类字符串供日志和诊断使用。
  - **自动降级与恢复**：
    - 失败检测器维护滑动窗口（默认300秒），样本数≥5且失败率≥20%触发降级；
    - 降级后状态切换为 `Fallback`（通过 `can_transition_to()` 验证 Enabled→Fallback 合法），`is_enabled()` 返回 `false`，后续任务走直连；状态机转换规则在 `state.rs` 的 `apply_transition()` 中强制验证；
    - 健康检查器定期探测代理可用性（探测目标为 `probeUrl` 配置的 host:port）；
    - 连续成功次数达阈值（默认3次）且冷却期满（默认300秒）触发恢复；
    - 恢复后状态切换为 `Enabled`，重置失败统计，发射 `ProxyRecoveredEvent`；
    - 支持三种恢复策略：`immediate`（单次成功）、`consecutive`（连续多次成功）、`exponential-backoff`（退避恢复）。
  - **前端集成**：
    - `ProxyConfig.vue`：代理配置UI（模式选择、URL/凭证输入、系统检测按钮、禁用自定义传输层开关、高级设置包含降级/恢复/探测配置、调试日志开关）；
    - `ProxyStatusPanel.vue`：状态面板（当前状态、降级原因、失败统计、URL显示含凭证脱敏）；
    - Tauri 命令：`detect_system_proxy()`（检测系统代理，返回 SystemProxyResult）、`force_proxy_fallback(reason?: Option<String>)`（手动降级，支持自定义原因）、`force_proxy_recovery()`（手动恢复）、`get_system_proxy()`（legacy 命令，返回基础信息）；
    - Pinia store：`useConfigStore` 管理代理配置（读写 config.json），前端组件通过 Tauri 命令直接调用后端功能；
    - 系统代理检测通过 `detect_system_proxy()` 命令返回结果，不发射事件；配置热更新无独立事件，由组件保存时触发。
  - **观测与事件**：
    - `ProxyStateEvent`：状态转换（previous/current state、reason、timestamp），包含扩展字段（proxy_mode、proxy_state、fallback_reason、failure_count、health_check_success_rate、next_health_check_at、system_proxy_url、custom_transport_disabled）；
    - `ProxyFallbackEvent`：降级事件（reason、failure_count、window_seconds、failure_rate、proxy_url、is_automatic）；
    - `ProxyRecoveredEvent`：恢复事件（successful_checks、proxy_url、is_automatic、strategy、timestamp）；
    - `ProxyHealthCheckEvent`：健康检查结果（success、response_time_ms、error、proxy_url、test_url、timestamp）；
    - 代理事件通过传输层线程局部变量与 P3 的 `AdaptiveTlsTiming/Fallback` 事件联动，但不会修改既有事件结构。
  - **Soak 与阈值**：
    - 环境变量：`FWC_PROXY_SOAK=1`、`FWC_SOAK_MIN_PROXY_SUCCESS_RATE=0.95`（代理成功率≥95%）、`FWC_SOAK_MAX_PROXY_FALLBACK_COUNT=1`（最多降级1次）、`FWC_SOAK_MIN_PROXY_RECOVERY_RATE=0.9`（恢复率≥90%，如有降级）；
    - 报告扩展：`proxy` 统计（selection_total、selection_by_mode、fallback_count、recovery_count、health_check_success_rate、avg_connection_latency_ms、system_proxy_detect_success）；
    - 阈值判定：`proxy_success_rate >= 0.95`、`fallback_count <= 1`、`recovery_rate >= 0.9`（如有降级）、`system_proxy_detect_success == true`（System模式必须检测成功）。
  - **测试矩阵**：
    - config.rs（36测试）：ProxyConfig结构、validation规则、默认值、is_enabled逻辑；
    - state.rs（17测试）：ProxyState状态机、转换验证、状态上下文；
    - detector.rs（28测试）：ProxyFailureDetector滑动窗口、失败率计算、阈值触发；
    - manager.rs（59测试）：ProxyManager统一API、配置热更新、状态管理、连接器切换；
    - http_connector.rs（29测试）：HTTP CONNECT隧道、Basic Auth、响应解析、超时处理；
    - socks5_connector.rs（59测试）：SOCKS5协议握手、认证方法、地址类型、REP错误码映射；
    - events.rs（15测试）：事件结构体序列化、时间戳生成、事件构造器；
    - ProxyConfig.vue（14测试）：配置UI交互、表单验证、系统检测、配置保存；
    - ProxyStatusPanel.vue（19测试）：状态显示、URL脱敏、模式Badge；
    - 总计：276个测试（243 Rust 单元/集成测试分布在 7 个文件：config.rs/state.rs/detector.rs/manager.rs/http_connector.rs/socks5_connector.rs/events.rs + 33 TypeScript 组件测试：ProxyConfig.test.ts 14个 + ProxyStatusPanel.test.ts 19个）。
  - **常见故障排查**：
    - 代理连接失败：检查 `proxy.url` 格式（必须包含协议前缀如 `http://`）、网络可达性（ping代理服务器）、凭证正确性（用户名/密码）；查看 `task://error` 中的 `category=Proxy/Auth`；启用 `debugProxyLogging=true` 查看详细连接日志（包含sanitized URL、认证状态、响应时间）；
    - 频繁降级：查看 `ProxyFallbackEvent` 中的 `failure_rate` 和 `failure_count`，可能需调高 `fallbackThreshold`（默认0.2即20%） 或检查代理稳定性；检查滑动窗口 `fallbackWindowSeconds` 是否过短；
    - 系统检测失败：Windows 检查注册表权限（需要读取 `HKCU`）和 IE代理设置是否配置、macOS 检查 `scutil` 命令是否可执行和网络偏好设置、Linux 检查环境变量（优先 `HTTPS_PROXY` 再 `HTTP_PROXY`）；提供手动配置回退（切换到http/socks5模式手动输入）；
    - 自定义传输层未禁用：确认代理 `is_enabled()` 返回 `true`（mode非off且URL非空或mode为system），检查 `should_disable_custom_transport()` 逻辑；查看日志中的 "Skipping custom transport registration" 消息；
    - 恢复不触发：检查冷却窗口是否到期（查看日志中的 recovery cooldown 提示）、`recoveryConsecutiveThreshold` 是否过高（默认3次，建议不超过10）、健康检查是否正常执行（查看 `ProxyHealthCheckEvent`）、探测URL是否可达（`probeUrl` 默认 `www.github.com:443`）。
  - **交接要点**：
    - 代理配置凭证当前明文存储在 `config.json`，P6 将引入安全存储（Windows Credential Manager/macOS Keychain/Linux Secret Service）；
    - 仅支持 Basic Auth，企业认证协议（NTLM/Kerberos）暂不支持，可使用 CNTLM 等本地转换工具；
    - PAC 文件解析、代理链、实时配置监听等功能延后到 P6 或后续版本；
    - 代理启用时强制禁用自定义传输层与 Fake SNI 是设计选择（通过 `should_disable_custom_transport()` 实现），即使 `disableCustomTransport=false` 也会被覆盖；
    - 热更新代理配置需要重新创建 `ProxyManager` 实例，通过传输层注册检查 `should_skip_custom_transport()` 生效；
    - 手动降级/恢复立即切换状态，下一个任务立即使用新配置；
    - 探测URL必须是 `host:port` 格式（如 `www.github.com:443`），不支持完整URL格式。
  - **回退策略**：参考附录C 步骤6（禁用策略覆盖）与步骤9（Push/Retry 回退）。阶段特有：可直接关闭 `strategyOverride` gating 变量，逐项移除 TaskKind 回退到仅 Clone/Fetch。
    - 配置层：设置 `proxy.mode=off` 立即禁用代理，下一个任务生效；
    - 手动控制：前端点击"手动降级"或调用 `force_proxy_fallback(reason?)` Tauri命令强制切换直连，发送 `ProxyFallbackEvent` (is_automatic=false)；
    - 调整阈值：修改 `fallbackThreshold`（0.0-1.0）/`recoveryConsecutiveThreshold`（1-10）/`recoveryCooldownSeconds`（≥10）并保存配置文件，应用重启或重新加载配置后生效；
    - 清理统计：重启应用或手动调用 `force_proxy_recovery()` 重置滑动窗口的失败统计并尝试恢复；
    - 运维介入：通过日志观察 `ProxyStateEvent` 和 `ProxyHealthCheckEvent` 获取诊断信息（当前状态、失败计数、健康检查结果），必要时临时设置 `healthCheckIntervalSeconds` 为更大值（如3600）延长探测间隔，或直接禁用代理。

### 4.7 P6 - 凭证存储与安全管理

  - **目标**:
    - 提供生产级凭证存储方案，支持三层存储智能回退（系统钥匙串 → 加密文件 → 内存）；
    - 实现企业级加密安全（AES-256-GCM + Argon2id密钥派生 + ZeroizeOnDrop内存保护）；
    - 提供完整审计日志与访问控制机制（失败锁定、自动过期、持久化）；
    - 与Git操作深度集成（自动填充凭证、智能降级、过期提醒）；
    - 前端用户体验优化（凭证管理表单、过期凭证管理、审计日志查看）。
  - **核心模块**：
    - `CredentialStoreFactory` 三层存储抽象与智能回退（根据平台能力、用户权限、配置自动选择最优存储）；
    - 系统钥匙串集成：Windows Credential Manager（`WindowsCredentialStore`）、macOS Keychain Services、Linux Secret Service（通过统一接口 `CredentialStore` trait实现）；
    - `EncryptedFileStore` 文件加密存储（AES-256-GCM加密、Argon2id密钥派生、密钥缓存优化200倍性能提升）；
    - `InMemoryStore` 进程级临时存储（回退兜底、测试隔离）；
    - `AuditLogger` 双模式审计（标准模式不记录哈希、审计模式记录SHA-256哈希）+ 持久化（JSON文件、自动加载、容错设计）；
    - `AccessControl`（内嵌于AuditLogger）失败锁定机制（默认5次失败 → 默认1800秒即30分钟锁定 → 自动过期或管理员重置）；
  - **补充要点**：
    - Git集成：`git_credential_autofill` 三级智能降级（存储凭证 → 未找到提示 → 错误继续）。
    - 审计新增 `OperationType::Unlock`：解锁加密文件存储（或尝试解锁）时记录 `unlock` 事件。
    - 主密码占位实现：`set_master_password` 当前仅记录警告，不持久化密码，也不触发加密；真实密钥派生在 `unlock_store` 路径中执行。
  - **配置与默认值**：
    - 运行期（`config.json`）：`credential.storage=system`（system/file/memory）、`default_ttl_seconds=7776000`（90天）、`debug_logging=false`、`audit_mode=false`、`require_confirmation=false`、`file_path=null`（加密文件路径，可选）、`key_cache_ttl_seconds=3600`（1小时）。已移除：`master_password` 字段（防止误认为可直配持久化）。
    - 访问控制（内部硬编码，不可配置）：`max_failures=5`、`lockout_duration_secs=1800`（30分钟）；
    - 环境变量：`FWC_CREDENTIAL_STORE`（覆盖storage）、`FWC_MASTER_PASSWORD`（测试/CI场景，加密文件模式使用）；
    - 所有字段支持热更新，修改后下一次操作生效。
  - **运行生命周期**：
    1. 应用启动 → `CredentialStoreFactory::create()` 根据配置尝试三层存储，失败则自动降级；
    2. 加密文件模式需用户调用 `unlock_store(masterPassword)` 解锁（触发审计 `unlock`）→ Argon2id密钥派生（1-2秒）→ 缓存密钥（TTL 300秒）；
    3. 凭证操作（add/get/update/delete/list）→ 路由到对应存储实现 → 自动记录审计日志；
    4. Git操作调用 `git_credential_autofill(host, username)` → 自动填充存储的凭证 → 未找到则返回None继续原有流程；
    5. 访问控制检测连续失败，达阈值触发 `AccessControlLocked` 事件并拒绝后续操作；
    6. 定期调用 `cleanup_expired_credentials()` 清理过期凭证，前端显示即将过期警告（7天）和已过期提示。
  - **三层存储智能回退**：
    - **Layer 1 - 系统钥匙串**：Windows Credential Manager（`CredReadW`/`CredWriteW`/`CredDeleteW`）、macOS Keychain（Security Framework）、Linux Secret Service（`libsecret` D-Bus）；失败原因包括权限不足、服务未运行、API错误；
    - **Layer 2 - 加密文件**：`credentials.enc` AES-256-GCM加密（随机nonce、AEAD认证标签）+ Argon2id密钥派生（m_cost=64MB, t_cost=3, p_cost=1）+ 密钥缓存（首次1-2秒，缓存后<10ms）；失败原因包括主密码错误、文件损坏、磁盘权限；
    - **Layer 3 - 内存存储**：进程内 `HashMap` 兜底，应用重启丢失；始终可用，确保功能不中断；
    - 回退决策：系统钥匙串失败 → 尝试加密文件（需主密码）→ 回退内存存储；每次回退记录日志并通过 `StoreBackendChanged` 事件通知前端。
  - **加密与安全**：
    - AES-256-GCM：对称加密算法，提供机密性和完整性保护（AEAD），每个凭证独立nonce确保安全；
    - Argon2id：密钥派生函数（KDF），抗GPU/ASIC破解，参数：内存64MB、时间3迭代、并行度1线程（符合OWASP推荐）；
    - HMAC验证：审计模式下对主机名/用户名生成SHA-256 HMAC，用于凭证追溯而不泄露明文；
    - ZeroizeOnDrop：`MasterPassword`、`EncryptionKey`、`Credential`中的密码字段使用 `zeroize` crate自动清零，防止内存残留；
    - 密钥缓存：首次派生1-2秒，缓存后<10ms，性能提升200倍；缓存密钥使用 `Arc<RwLock<Option<EncryptionKey>>>` 保护，TTL默认3600秒（1小时）；
    - Display/Debug trait：密码字段使用 `masked_password()` 脱敏（前2字符+***+后2字符），防止日志泄露。
  - **审计与访问控制**：
    - 审计日志包含：操作类型（Add/Get/Update/Delete/Unlock）、时间戳（Unix秒）、主机名、用户名、结果（Success/Failure/AccessDenied）、可选SHA-256哈希（审计模式）。
    - 持久化：`audit-log.json` JSON Lines格式，应用启动自动加载，损坏时优雅降级创建新文件；
    - 访问控制：连续5次失败 → 锁定30分钟 → 自动过期或管理员调用 `reset_credential_lock()` 重置；锁定期间返回 `remaining_attempts()` 供前端显示剩余尝试次数；
    - 容错设计：审计日志写入失败不影响凭证操作（降级为内存日志），文件损坏时自动重建。
  - **Git集成细节**：
    - `git_credential_autofill(host, username)` 在Git Push/Fetch前调用，返回 `Option<CredentialInfo>`；
    - 三级降级策略：存储中找到凭证 → 直接使用；未找到 → 返回None，Git操作继续交互式输入；获取失败（锁定/错误）→ 返回None并记录错误；
    - URL格式支持：HTTPS（`https://github.com/...`）、SSH（`ssh://git@github.com:...`）、Git简写（`git@github.com:...`）；
    - 过期处理：即将过期（7天内）显示黄色警告，已过期显示红色错误并提供一键清理按钮；
    - 3次迭代优化（P6.4.1-P6.4.3）：初始实现 → 添加URL解析与域名提取 → 优化错误处理与降级逻辑（共1,135行代码）。
  - **前端集成**：
    - **Tauri命令**（13个）：
      - 凭证操作（5个）：`add_credential`、`get_credential`、`update_credential`、`delete_credential`、`list_credentials`；
      - 生命周期管理（2个）：`cleanup_expired_credentials`、`set_master_password`（初始化）、`unlock_store`（解锁）；
      - 审计日志（2个）：`export_audit_log`、`cleanup_audit_logs`；
      - 访问控制（3个）：`is_credential_locked`、`reset_credential_lock`、`remaining_auth_attempts`；
    - **Vue组件**（4个）：
      - `CredentialForm.vue`（165-182行）：凭证添加/编辑表单，支持主机名、用户名、密码/令牌输入，过期时间选择（天数）；
      - `CredentialList.vue`（178行）：凭证列表展示，脱敏显示（仅前后2字符），过期状态Badge（即将过期/已过期），删除确认；
      - `ConfirmDialog.vue`（65行，P6.5新增）：通用确认对话框，3种变体（danger/warning/info），DaisyUI modal实现；
      - `AuditLogView.vue`（156行，P6.5新增）：审计日志查看，时间范围过滤、操作类型筛选、导出JSON功能；
    - **Pinia Store**（`credential.store.ts`）：9个actions（loadCredentials、addCredential、updateCredential、deleteCredential、unlockStore、lockStore、cleanupExpired、resetLock、exportAuditLogs）、5个getters（isLocked、expiringSoon、expired、sortedCredentials、auditSummary）。
  - **测试矩阵**：
    - 后端测试：521个（60单元测试 + 461集成测试），206个凭证模块专项测试（73存储 + 48管理 + 31审计 + 24生命周期 + 9 Git + 21 CredentialView组件）；
    - 前端测试：295个（全部通过），144个P6凭证相关测试（17 credential.store + 28 CredentialForm + 99 UI组件）；
    - 总计：1286个测试（991 Rust + 295 前端），99.9%通过率（仅1个proxy模块pre-existing issue），88.5%覆盖率；
    - 关键测试场景：三层回退、加密解密往返、密钥缓存TTL、访问控制锁定与恢复、审计日志持久化与容错、Git自动填充3种URL格式、过期凭证清理、并发操作安全。
  - **性能指标**：
    - 系统钥匙串：add/get/delete <5ms（Windows实测），list(100) ~15ms；
    - 加密文件：首次操作1000-2000ms（密钥派生），缓存后<10ms，性能提升200倍；
    - 内存存储：所有操作<1ms，list(1000) <200ms；
    - 审计日志：写入<0.5ms（异步），SHA-256哈希<0.5ms；
    - 并发性能：100线程并发读写无死锁、无数据竞争。
  - **代码规模**：
    - 总计：17,540行（核心4,684 + 测试8,406 + 文档4,450）；
    - 测试/核心比例：1.8:1（优秀）；
    - Clippy警告：0；unwrap()数量：0（全部使用expect或?）；unsafe代码：0。
  - **技术创新**（10项）：
    1. 三层存储智能回退（平衡安全性与可用性）；
    2. SerializableCredential模式（解决 `#[serde(skip)]` 序列化问题）；
    3. 密钥派生缓存优化（200倍性能提升）；
    4. Windows API凭证前缀过滤（`fireworks-collaboration:git:` 避免冲突）；
    5. CredentialInfo自动映射（密码永不传输到前端）；
    6. 审计日志双模式（标准/审计模式平衡隐私与追溯）；
    7. Git凭证智能降级（3级降级保证可用性）；
    8. 过期凭证双重提醒（即将过期/已过期）；
    9. 审计日志容错设计（损坏时优雅降级）；
    10. 访问控制自动过期（30分钟自动解锁）。
  - **安全审计结论**（2025年10月4日）：
    - 审计范围：~3,600行核心代码，8个维度（加密、内存、日志、错误、并发、平台、配置、密钥）；
    - 总体评分：⭐⭐⭐⭐⭐ (4.9/5)；
    - 风险识别：0高危、3中危（macOS/Linux未实机验证、密钥缓存内存风险、审计日志无限增长）、3低危；
    - 合规性：OWASP Top 10全部通过、NIST标准符合（AC/AU/IA/SC系列）、依赖安全无已知CVE；
    - 准入决策：✅ **批准生产环境上线**（附条件：CI/CD跨平台测试）。
  - **准入评审**（7项标准全部达标）：
    - 功能完整性：99%（仅 `last_used` 未实现，受Rust不可变模型限制）；
    - 测试通过率：99.9%（1286个测试，仅1个非相关失败）；
    - 测试覆盖率：88.5%（后端90%、前端87%）；
    - 安全审计：0高危风险；
    - 性能指标：<500ms达标（除首次密钥派生）；
    - 文档完整性：100%（所有公共API）；
    - 代码质量：0 Clippy警告。
  - **常见故障排查**：
    - 系统钥匙串失败：Windows检查Credential Manager服务是否运行、macOS检查Keychain Access权限、Linux检查Secret Service（`gnome-keyring`/`seahorse`）是否安装；查看日志中的具体错误码；
    - 主密码错误：加密文件模式下密钥派生失败返回 `InvalidMasterPassword`，重置需删除 `credentials.enc` 并重新设置；
    - 访问控制锁定：连续5次失败后锁定30分钟，查看 `AccessControlLocked` 事件中的 `locked_until` 时间戳，管理员可调用 `reset_credential_lock()` 立即解锁；
    - 审计日志损坏：删除 `audit-log.json` 会自动重建，日志含 "failed to load audit log" 警告；
    - Git自动填充不工作：检查URL格式是否支持（HTTPS/SSH/git@），确认凭证已存储且未过期，查看 `git_credential_autofill` 返回值；
  - 密钥缓存过期：默认TTL 3600秒（1小时），过期后下次操作重新派生（1-2秒），可通过 `key_cache_ttl_seconds` 调整。
  - 看到 `set_master_password` 日志但加密未生效：属预期，占位实现；需调用 `unlock_store`。
  - **交接要点**：
  - 凭证当前明文存储在系统钥匙串/加密文件，P7可考虑HSM集成或硬件密钥；`set_master_password` 正式实现（持久化+旋转）列入“已知占位”待办。
    - macOS/Linux系统钥匙串代码已实现但未实机验证，建议添加CI/CD跨平台测试；
    - 审计日志暂无自动滚动策略，需手动清理或在后续版本实现（短期优化）；
    - 性能基准测试框架已完成（295行，8个测试组），建议运行 `cargo bench --bench credential_benchmark` 获取实际数据；
    - 最后使用时间（`last_used`）字段因Rust不可变模型限制未实现，需重构为可变结构（技术债务）；
    - 凭证导出功能暂无额外加密保护，用户自行管理导出文件安全（延后增强）。
  - **回退策略**：参考附录C 步骤1~2（层级降级/关闭告警导出）。阶段特有：优先 set_layer 降级；导出/告警关闭后仍保留事件桥接以便最小诊断。
    - 配置层：逐层禁用存储（system → file → memory），或完全禁用凭证功能；
    - 审计日志：关闭审计模式（`auditMode=standard`）或禁用持久化；
    - 访问控制：调整阈值（`maxFailures`、`lockoutDurationMinutes`）或完全禁用（`enabled=false`）；
    - Git集成：移除 `git_credential_autofill` 调用，回退到交互式输入；
    - 主密码：重置需删除 `credentials.enc` 并重新解锁，已存储凭证丢失（提前备份）。
  - **上线策略**（推荐三阶段灰度）：
    1. **阶段1（灰度）**：10-20个用户测试（1周），重点验证系统钥匙串集成和主密码流程；
    2. **阶段2（扩大）**：100个用户测试（2周），监控审计日志存储和访问控制触发频率；
    3. **阶段3（全量）**：全量发布，持续监控性能指标和安全事件。
  - **后续优化建议**：
    - 短期（1-3个月）：macOS/Linux实机验证、审计日志滚动策略、性能基准测试执行、用户体验优化（搜索/过滤/批量操作）；
    - 长期（3-12个月）：生物识别解锁（Touch ID/Windows Hello）、OAuth 2.0自动刷新、凭证跨设备同步、审计日志远程上传、HSM集成。

### 4.8 P7 - 工作区与批量能力

- **目标**:
  - 建立多仓库工作区(Workspace)管理模型,支持仓库 CRUD、标签分类与序列化存储;
  - 实现 Git 子模块探测与批量操作(init/update/sync),复用现有 git2 能力;
  - 提供批量并发任务调度(clone/fetch/push),通过 Semaphore 控制并发度,避免资源竞争;
  - 支持团队配置模板导出/导入,便于跨团队标准化与安全化;
  - 引入跨仓库状态监控服务,带 TTL 缓存与无效化 API,减少重复查询开销;
  - 前端一体化视图,集成任务进度、错误聚合与 Pinia store 响应式状态;
  - 提供性能基准与稳定性测试,支撑灰度上线决策。

- **批量任务并发控制实现细节**:
  - `workspace_batch_*` 命令通过 `resolve_concurrency(requested, config)` 解析最终并发数:优先使用请求中的 `maxConcurrency`,回退到配置的 `workspace.maxConcurrentRepos`(实际默认值 3),强制校验 `value > 0` 避免死锁;
  - 内部使用 `tokio::sync::Semaphore` 持有 `max_concurrency` 个 permit,每个子任务执行前 `acquire()`,完成后自动释放,确保同时运行的子任务数不超过阈值;
  - 父任务(`TaskKind::WorkspaceBatch { operation, total }`)创建后立即返回父 `taskId`,子任务递归创建为 `TaskKind::GitClone`/`GitFetch`/`GitPush`,通过 `parent_id` 关联,进度事件聚合到父任务的 phase 文本中(`Cloning 3/10 repositories`);
  - 失败策略:默认 `continueOnError=true`,单个子任务失败不中断批量流程,最终父任务汇总所有子任务状态到 `task://state`(`completed` 表示全部成功,`failed` 表示至少一个失败),错误详情通过 `task://error` 子任务事件分发。

- **RepositoryEntry.hasSubmodules 字段作用**:
  - 在 `workspace_batch_clone` 命令中,若请求未明确指定 `recurseSubmodules` 参数,则回退到该字段值作为默认行为:
    ```rust
    recurse_submodules: request.recurse_submodules.unwrap_or(repo.has_submodules)
    ```
  - 允许为不同仓库单独配置子模块处理策略(例如前端仓库启用,后端服务禁用),提高灵活性;
  - 该字段默认值为 `false`,在 `workspace.json` 中显式声明后生效。

- **SubmoduleManager 独立状态**:
  - `SharedSubmoduleManager = Arc<Mutex<SubmoduleManager>>` 与 workspace/status service 并行,拥有独立 `SubmoduleConfig`(默认 autoRecurse=true, maxDepth=5, autoInitOnClone=true, recursiveUpdate=true);
  - 命令返回 `SubmoduleCommandResult { success: bool, message: String, data?: string[] }`:前端必须检查 `success` 字段判断成功/失败,而非直接依赖 Promise resolve/reject。`data` 字段包含受影响子模块名称列表。

 - **运维命令扩展**:
  - `validate_workspace_file(path)`: 校验 workspace.json 结构合法性,返回布尔值（内部通过 `WorkspaceStorage::new(path).validate()`）；
  - `backup_workspace(path)`: 创建带时间戳的备份文件(`workspace.json.backup.YYYYMMDD_HHMMSS`)，返回完整路径（`WorkspaceStorage::backup()`）；
  - `restore_workspace(backupPath, workspacePath)`: 从备份恢复,覆盖目标文件（`WorkspaceStorage::restore_from_backup`）。
  - 备份策略建议:每次批量操作前手动备份或配置自动备份钩子(未实现)。

- **测试覆盖**:
  - 子模块: 24 项(列表/检测/初始化/更新/同步,含递归场景);
  - 批量调度: 12 项(clone/fetch/push 各 4个,含并发/失败聚合/取消传播);
  - 状态服务: 缓存/TTL/失效集成测试 + 10/50/100 仓库性能基准;
  - 前端 store: 17 项 Pinia 测试(批量任务、模板报告、状态缓存);
  - 性能基准 Nightly 门槛: 批量 clone p95 < 0.2×/仓、状态刷新 100 仓 < 3s。

- **回退策略快速参考**:
  - 批量负载过高:降 `workspace.maxConcurrentRepos=1` 退化为顺序;
  - 子模块初始化失败:禁用 `submodule.autoInitOnClone=false`,手动调用;
  - 状态刷新 IO 压力:停止自动轮询(`statusAutoRefreshSecs=0`)提高 TTL;
  - 模板导入风险:使用自动备份覆盖 config.json 回滚;
  - 整体禁用:`workspace.enabled=false` 回退单仓模式(不需重编译)。

- **模块映射与代码指针**:
  - `src-tauri/src/core/workspace/`: model.rs(核心结构), config.rs(配置管理), storage.rs(序列化/验证/备份), status.rs(状态服务);
  - `src-tauri/src/core/submodule/`: model.rs, manager.rs(init/update/sync 操作), config.rs(子模块配置);
  - `src-tauri/src/core/tasks/workspace_batch.rs`: Semaphore 调度、父子任务关联、进度聚合;
  - `src-tauri/src/core/config/team_template.rs`: 模板导出/导入、安全化清理、备份机制；当模板与本地配置不一致时会同步写入 `ip_pool.enabled` 与 `ip_pool.dns` 字段，避免分发后出现默认值回退;
  - `src-tauri/src/app/commands/workspace.rs`: 18 个 Tauri 命令(CRUD/批量/状态/备份);
  - `src-tauri/src/app/commands/submodule.rs`: 9 个 Tauri 命令(list/has/init/update/sync + 配置);
  - `src/views/WorkspaceView.vue`: 工作区视图(仓库列表、批量操作、状态监控);
  - `src/stores/workspace.ts`: Pinia store(CRUD actions/getters, 与后端命令桥接);
  - `src/stores/tasks.ts`: 批量任务父子关系跟踪、进度聚合。

- **跨阶段集成补充**:
  - P7 工作区/批量逻辑仅在 `workspace.enabled=true` 时激活,不改变 MP0-P6 任务语义;
  - 子模块递归克隆附加在 `TaskKind::GitClone` 之后(70-85-100% 进度区间映射);
  - 团队配置模板导入仅写入 config.json,不影响 TLS/IP/代理/凭证回退路径;
  - 代理(P5)启用时不影响批量逻辑,内部仍复用 Git 传输层现有互斥策略;
  - 凭证存储(P6)自动为批量 clone/push 子任务统一回调,无需重复提供凭证;
  - IP 池(P4)与工作区解耦,批量任务底层按单仓 Git 路径调用。

- **已知限制**:
  - 子模块并行参数(`parallel`/`maxParallel`)预留但未实现(串行足够 <10 子模块场景);
  - 子模块粒度进度事件尚未连接前端总线,仅通过主任务阶段映射;
  - 批量进度权重均等,未按仓库体积/历史耗时加权(大体量差异下不线性);
  - 工作区状态服务无事件推送(需轮询),大量仓库高频刷新需调大 TTL 与关闭自动刷新;
  - 工作区文件并发风险:未实现显式锁,多进程同时写 workspace.json 存在竞态(建议容器编排保证单实例)。

### 4.9 测试重构 - 统一验证体系

- **目录布局**：`src-tauri/tests` 现按主题聚合——`common/`（共享 DSL 与 fixtures）、`git/`（Git 语义）、`events/`、`quality/`、`tasks/`、`e2e/`；每个聚合文件控制在 800 行内，新增用例优先追加至现有 section。
- **公共模块**：`common/test_env.rs`（全局初始化）、`fixtures.rs` 与 `repo_factory.rs`（仓库构造）、`git_scenarios.rs`（复合操作）、`shallow_matrix.rs`/`partial_filter_matrix.rs`/`retry_matrix.rs`（参数矩阵），确保相同语义只实现一次。
- **事件 DSL**：`common/event_assert.rs` 提供 `expect_subsequence`、`expect_tags_subsequence`、策略/TLS 专用断言；测试通过 Tag 序列或结构化辅助降低脆弱度，所有策略/TLS 事件均已接入。
- **属性测试与回归种子**：集中在 `quality/error_and_i18n.rs`，按 `strategy_props`、`retry_props`、`partial_filter_props`、`tls_props` 分 section；`prop_tls_override.proptest-regressions` 保存最小化案例，遵循附录 B SOP 定期清理。
- **指标与质量监测**：重构后维护若干基线指标（单文件行数、关键词出现次数、属性测试执行时间），通过 PowerShell 脚本或 `wc -l` 快速检查，防止回归到碎片化结构。
- **新增用例流程**：
  1. 选择合适聚合文件并引用 `test_env::init_test_env()`；
  2. 若覆盖新参数维度，先在当前文件内定义 case 枚举；只有 ≥2 文件复用时才上移 `common/`；
  3. 针对策略/TLS/事件，使用 DSL 子序列断言而非硬编码完整列表；
  4. 需要属性测试时，在 `quality/error_and_i18n.rs` 新建 section，并在生成器中实现 `Display` 方便调试；失败案例写入 seed 文件尾部。
- **交接 checklist**：
  - 新功能合入前检查对应聚合文件行数与 DSL 覆盖，必要时拆分 section 或抽象 helper；
  - 在 PR 模板中勾选“更新测试 DSL/矩阵”项，避免忘记同步；
  - 发布前运行 `cargo test -q` 与 `pnpm test -s`，若属性测试时间 >5s 需调查生成器是否退化。

  ### 4.10 P8 - 可观测性体系摘要

  - **范围**：指标注册、事件桥接、窗口聚合、导出、前端面板、告警引擎、Soak 集成、灰度层级、性能与资源降级、自检与运维。
  - **模块映射**：`core/metrics/{descriptors,registry,aggregate,event_bridge,export,alerts,runtime,layer}.rs`；前端 `api/metrics.ts`、`stores/metrics.ts`、`components/observability/*`、`views/ObservabilityView.vue`。
  - **配置**：`observability.enabled|basicEnabled|aggregateEnabled|exportEnabled|uiEnabled|alertsEnabled`、`observability.layer`、`autoDowngrade`、层级驻留/冷却(`minLayerResidencySecs`/`downgradeCooldownSecs`)、性能(`performance.batchFlushIntervalMs|tlsSampleRate|maxMemoryBytes|enableSharding|debugMode|redact.repoHashSalt|redact.ipMode`)、导出(`export.authToken|rateLimitQps|maxSeriesPerSnapshot|bindAddress`)、告警(`alerts.rulesPath|evalIntervalSecs|minRepeatIntervalSecs`)。注：此前文档的 `internalConsistencyCheckIntervalSecs` 实现中不存在，已移除（若未来加入请同步附录D）。
  - **层级**：0 basic → 1 aggregate → 2 export → 3 ui → 4 alerts → 5 optimize；低层不依赖高层；自动降级按资源反压逐级回退，手动 `set_layer` 可回升（冷却约束）。
  - **数据流**：事件→Bridge→Runtime 缓冲→Registry→(窗口)Aggregator→导出/告警/面板/自检→Soak；失败或资源事件→LayerManager 调整层级→裁剪下游链路。
  - **指标命名**：`snake_case`，counter `_total`，延迟 `_ms`，Histogram 桶统一；内部导出指标：`metrics_export_requests_total{status}`、`metrics_export_rate_limited_total` 等。
  - **告警 DSL**：分位 `metric[p95]>800`，比值 `a/b>0.05`，标签 `{label=value}`，窗口 `window:5m`；状态机 firing→active→resolved + 去抖；Soak 阻断未恢复 critical。
  - **性能**：线程本地批量、分片、可调采样、raw 样本可选、标签脱敏、内存水位禁用 raw + 降级；保持 Git 主流程热点最小额外锁争用。
  - **运维**：健康检查清单（导出/核心指标/层级/内存压力/告警抖动）；日志关键字（metrics_export/metric_alert/metric_memory_pressure/observability_layer/metric_drift）；常用操作（调采样/停告警/重建导出/层级切换）。
  - **回退**：全局 `enabled=false`；层级 basic 保留最小计数集；逐项关闭 export/ui/alerts；删除规则文件快速静默。
  - **测试**：注册/桥接去重/窗口/导出/限流/告警状态机/热更新/内存压力/层级/前端缓存 & 降采样；性能 smoke；详见 P8 handoff §13。
  - **后续增强**：自适应采样、CKMS 分位、HTTPS 导出、规则分组+抑制、Trace、诊断 CLI、指标预算。
  
  #### 设计 vs 当前运行态（v1.6）
  | 子能力 | 设计目标 | 当前状态 | 说明 / 后续动作 |
  |--------|----------|----------|----------------|
  | basic 注册 | 采集核心计数/时延 | 已启用 | 正常运行 |
  | aggregate 窗口 | 1m/5m/1h/24h 聚合 | 已启用 | 正常运行 |
  | export 导出 | HTTP `/metrics` + snapshot | 未启动 | 计划改为惰性首次访问启动 |
  | ui 面板 | 范围&下采样可视化 | 前端存在,后端接口缺失时降级 | 对 404 容错已实现 |
  | alerts 告警 | 规则评估/去抖 | 未启动 | 依赖 export 底座 |
  | optimize 层 | 分片/批量/标签脱敏 | 部分生效 | 高层未启用不影响基础统计 |
  | 自动降级 | 资源/内存触发层级回退 | 逻辑已接入 | export 未启用减少触发面 |
  | 自检/漂移 | 指标一致性校验 | 未实现 | 后续版本排期 |
  | 规则热更新 | 文件变更即生效 | 未启动 | 等 alerts 启用后接入 |
  | Snapshot 鉴权 | Token/限流 | 未启动 | 与 export 一并启用 |

## 5. 交接与发布 checklist 概览

- **文档同步**：核对最新阶段 handoff（MP*/P*）与本文版本号、设计稿、`CHANGELOG.md`；P8 专项：`P8_IMPLEMENTATION_HANDOFF.md` 是否与实现一致（指标/配置/层级表/告警规则样例）。
- **Smoke 速查**：见附录A；上线前按顺序执行 1~6 步确认核心链路与降级/告警可控。
- **配置审计**：阶段配置核对：MP1（Fake SNI/Retry）、P2（`FWC_PARTIAL_FILTER_SUPPORTED` / `FWC_PARTIAL_FILTER_CAPABLE`）、P3（rollout/auto disable/SPKI pin/tls.metricsEnabled）、P4（`ip_pool.*` 缓存/熔断/并发/黑白名单）、P5（`proxy.*` 阈值/探测URL/禁用自定义传输）、P6（`credential.*` 存储/审计/缓存）、P7（`workspace.*`、`submodule.*`、`teamTemplate.*`）、P8（`observability.*` 见附录D：层级/驻留与冷却/性能(batchFlushIntervalMs,tlsSampleRate,maxMemoryBytes,enableSharding,debugMode,redact.repoHashSalt,redact.ipMode)/导出(authToken,rateLimitQps,maxSeriesPerSnapshot,bindAddress)/告警(rulesPath,evalIntervalSecs,minRepeatIntervalSecs)/降级(autoDowngrade)）。
- **灰度计划**：记录阶段/层级推进与回退：P6（三步钥匙串→扩大→全量）；P8 层级 basic→aggregate→export→ui→alerts→optimize（每层 ≥24h 观察：导出延迟 <50ms、memory_pressure=0 或低频、告警噪声可控）。
- **测试执行**：合并前运行 `cargo test -q`、`pnpm test -s`；附必要 soak 报告。P6：1286 测试 + 安全审计；P8：聚合窗口/导出/限流/告警状态机/规则热更新/内存压力降级/层级状态机/前端缓存降采样；保存 `/metrics` 与 `/metrics/snapshot` 样例输出（含内部指标与分位）。
- **运维交接**：交付包包含：`cert-fp.log` 示例、策略 Summary 截图、最新配置快照、P8 健康清单（/metrics OK、核心指标非空、layer 预期、无连续 memory_pressure、告警稳定）、告警规则文件样例、导出内部指标说明及含义、层级降级/回升操作说明。
- **回退路径**：列出配置级开关与顺序：Push/Fake SNI/策略覆盖/IP 池/代理/凭证/工作区/可观测性（层级降级→disabled）。确认回退不会破坏事件总线及核心 Git 任务链路。
- **监控接入**：Prometheus 采集 `/metrics`；针对内部指标添加告警（rate_limited_total 激增、export_requests_total{status="error"}、observability_layer 非期望值、alerts_fired_total 抖动）；创建面板：Git 成功率、TLS p95、IP 刷新成功率、代理降级次数、内存压力次数。
- **安全校验**：确认导出未暴露敏感标签（repo/IP 明文），Token 访问策略测试（401/429/200）、审计日志不含密码、凭证存储审计模式符合策略；可观测性 UI 在导出关闭时 fallback 正常。
- **发布前 Smoke**：执行：1) `/metrics` 拉取 2) Snapshot 指定窗口+分位 3) 修改阈值触发告警 firing→resolved 4) 人工 set_layer 降级 & 回升 5) 调低 memory limit 触发 raw 样本禁用 6) 前端面板范围切换与缓存命中日志。
- **速查附录**：指标→代码映射见附录B；统一回退执行序列见附录C。

---

## 已知占位与待办（v1.6）

| 类别 | 项目 | 当前状态 | 计划/动作 | 影响与临时处理 |
|------|------|----------|-----------|----------------|
| 可观测性 | metrics export server | 未启动 | 改为首次 snapshot 请求时惰性启动 | 前端对 404 优雅降级；手动验证需临时启用调用 |
| 可观测性 | alerts 告警引擎 | 未启动 | 待 export 启用后接入规则加载/状态机 | 无告警；不影响 basic/aggregate 指标采集 |
| 可观测性 | 规则热更新 | 未实现 | 随 alerts 一起实现文件监听或轮询 | 修改规则需重启（未来优化） |
| 可观测性 | 自检/漂移检测 | 未实现 | 规划加入内部一致性校验周期 | 可能延迟发现指标缺失，需要人工 Smoke |
| 凭证 | set_master_password 真正持久化 | 占位（忽略密码） | 实现密钥写入/旋转 & 验证流程 | 用户需用 unlock_store 解锁实际加密存储 |
| 凭证 | last_used 字段 | 未实现 | 需要调整不可变模型/写路径 | 前端暂用 created/expired 近似提醒 |
| TLS | 真实 SNI 路径未执行 SPKI pin 校验 | 未解决 | 在默认 `ClientConfig` 上挂载 `RealHostCertVerifier` 或等效逻辑 | 当前仅 Fake SNI 路径使用 SPKI pin |
| 凭证 | 审计日志滚动策略 | 未实现 | 加入大小/日期轮换 + 保留策略 | 日志过大需人工清理 |
| 工作区 | 子模块并行参数 parallel/maxParallel | 预留未实现 | 视规模需求引入并发执行 | 大量子模块时耗时偏长（可手动拆分） |
| 工作区 | 状态事件推送 | 未实现 | 后续通过事件总线广播增量 | 目前需轮询；高频降 TTL/自动刷新开销 |
| 观测/性能 | Snapshot 鉴权/限流 | 未启动 | 与 export 同批实现 Token + 令牌桶 | 暂无；当前 404 不触发安全风险 |
| 安全 | HSM/生物识别解锁 | 未实现 | 规划长期路线 | 现有加密仍满足基础安全要求 |

说明：上表集中维护所有“占位/未启动”状态，相关章节（P6、P7、P8、增量摘要）只做指向引用，更新时请同时修改本表与对应段落。

---

## 附录A. 核心功能 & 可观测性 Smoke 命令速查

> 目标：最短路径验证核心 Git/策略/网络/可观测性/回退链路健康。建议在预生产 & 灰度每层级提升前执行。

1. Git 基本任务：
  - Clone: `git_clone` 任意公开仓库（确认产生 `task://state running→completed` 与 `git_tasks_total{kind="clone",state="completed"}` 增量）
  - Push: `git_push`（使用测试分支，小修改）观察 `git_retry_total` 是否为0 或可控
2. 策略覆盖：Clone 指定 shallow+partial+retry 覆盖，确认结构化事件序列：`Strategy::HttpApplied` / `Policy::RetryApplied`（若有） → `Strategy::Summary`
3. 自适应 TLS：设置 `http.fakeSniRolloutPercent=25` 触发一次 clone；检查 `/metrics` 中 `tls_handshake_ms_bucket` 与事件 `AdaptiveTlsRollout`
4. IP 池：确认 `ip_pool.enabled=true`（默认状态）并至少配置一个 `preheatDomains`；等待 1 个预热周期后确认 `ip_pool_refresh_total` 有 `reason=preheat`
5. 代理降级/恢复（可选）：配置无效代理，触发失败直至 `ProxyFallbackEvent`，改为有效代理验证 `ProxyRecoveredEvent`
6. 可观测性导出（当前 v1.6 未启动可能返回 404 属正常）：访问 `/metrics` 与 `/metrics/snapshot?window=5m&quantiles=p50,p95`，目标（启用后）：
  - HTTP 200；`metrics_export_requests_total{status="ok"}` 递增
  - 多次快速访问时在开启限流下 `metrics_export_rate_limited_total` 递增
7. 告警引擎（当前未启用，跳过）：导出启用后可临时创建规则（如 `git_task_duration_ms[p95] > 1`）验证 `alerts_fired_total` 增量 → 移除规则后 resolved
8. 层级降级：手动 `observability.layer` 从 optimize 改为 basic，观察 `observability_layer` Gauge 数值下降且导出样本数减少
9. 内存压力模拟（可选）：调低 `observability.performance.maxMemoryBytes`，构造大量短时任务，观察 `metric_memory_pressure_total` 增量并层级回退
10. 前端面板：切换时间范围（1m → 1h），若导出未启用会降级（占位/空状态），启用后再验证 LTTB 下采样 & 缓存命中（第二次同窗访问不再发请求）

快速判定：全部成功且无异常错误事件 => 放行下一阶段；若某一步异常，优先参考附录C 回退序列。

## 附录B. 指标描述符速查 (导出名 → 常量 → 文件)

| 指标 | 常量 | 文件 | 标签 |
|------|------|------|------|
| git_tasks_total | GIT_TASKS_TOTAL | core/metrics/descriptors.rs | kind,state |
| git_task_duration_ms | GIT_TASK_DURATION_MS | core/metrics/descriptors.rs | kind |
| git_retry_total | GIT_RETRY_TOTAL | core/metrics/descriptors.rs | kind,category |
| tls_handshake_ms | TLS_HANDSHAKE_MS | core/metrics/descriptors.rs | sni_strategy,outcome |
| ip_pool_selection_total | IP_POOL_SELECTION_TOTAL | core/metrics/descriptors.rs | strategy,outcome |
| ip_pool_refresh_total | IP_POOL_REFRESH_TOTAL | core/metrics/descriptors.rs | reason,success |
| ip_pool_latency_ms | IP_POOL_LATENCY_MS | core/metrics/descriptors.rs | source |
| ip_pool_auto_disable_total | IP_POOL_AUTO_DISABLE_TOTAL | core/metrics/descriptors.rs | reason |
| circuit_breaker_trip_total | CIRCUIT_BREAKER_TRIP_TOTAL | core/metrics/descriptors.rs | reason |
| circuit_breaker_recover_total | CIRCUIT_BREAKER_RECOVER_TOTAL | core/metrics/descriptors.rs | (none) |
| proxy_fallback_total | PROXY_FALLBACK_TOTAL | core/metrics/descriptors.rs | reason |
| http_strategy_fallback_total | HTTP_STRATEGY_FALLBACK_TOTAL | core/metrics/descriptors.rs | stage,from |
| soak_threshold_violation_total | SOAK_THRESHOLD_VIOLATION_TOTAL | core/metrics/descriptors.rs | name |
| alerts_fired_total | ALERTS_FIRED_TOTAL | core/metrics/descriptors.rs | severity |
| metrics_export_requests_total | METRICS_EXPORT_REQUESTS_TOTAL | core/metrics/descriptors.rs | status |
| metrics_export_series_total | METRICS_EXPORT_SERIES_TOTAL | core/metrics/descriptors.rs | endpoint |
| metrics_export_rate_limited_total | METRICS_EXPORT_RATE_LIMITED_TOTAL | core/metrics/descriptors.rs | (none) |
| metric_memory_pressure_total | METRIC_MEMORY_PRESSURE_TOTAL | core/metrics/descriptors.rs | (none) |
| observability_layer | OBSERVABILITY_LAYER | core/metrics/descriptors.rs | (none) |

使用指引：
1. 新增指标：在 `descriptors.rs` 定义常量并加入 `BASIC_METRICS`；必要时更新 P8 handoff 与本附录表。
2. 标签变更：严格禁止破坏性重命名；新增标签需支持缺省值或版本兼容；提交前执行 `/metrics` 手动快照对比。
3. 排查：通过常量名称 `ripgrep` 定位引用，确认注册/聚合/导出链路是否完整。

## 附录C. 统一快速回退执行序列

> 原则：最小化影响、保持核心 Clone/Fetch 基线可用，逐层裁剪新增能力；适用于突发性能/稳定性/安全事件。

优先级自上而下执行（遇到指标恢复即可停止）：

| 步骤 | 动作 | 目标 | 预期恢复点 | 相关指标/事件 |
|------|------|------|------------|---------------|
| 1 | 下调 observability.layer (optimize→basic) | 降低监控/聚合开销 | `observability_layer`=0 | metric_memory_pressure_total, export_requests_total |
| 2 | 关闭导出/告警 (`export.enabled=false` / `alerts.enabled=false`) | 减少 I/O + 规则评估 | export 404 / 告警停止 | metrics_export_requests_total{status="error"}=0 |
| (当前 v1.6) | （export/alerts 默认未启动） | —— | 视同步骤2已生效 | —— |
| 3 | 禁用工作区 (`workspace.enabled=false`) | 移除批量/子模块调度压力 | 单仓任务成功率回升 | git_tasks_total{kind="workspace_batch"} 停止增长 |
| 4 | 禁用代理 (`proxy.mode=off`) 或强制直连 | 排除代理链路不稳定 | 连接错误下降 | proxy_fallback_total 不再增长 |
| 5 | 禁用 IP 池 (`ip_pool.enabled=false`) | 移除预热/熔断干扰 | TLS/连接成功率恢复 | ip_pool_refresh_total 停止增长 |
| 6 | 停用策略覆盖 (设置 gating 变量关闭) | 统一传输策略 | override/summary 事件消失 | http_strategy_fallback_total 稳定 |
| 7 | Fake SNI rollout=0 且 autoDisableFakeThresholdPct=1 | 排除 SNI 采样影响 | `AdaptiveTlsFallback` 降低 | tls_handshake_ms p95 稳定 |
| 8 | 禁用凭证高级特性（审计/访问控制） | 降低加密/IO | add/get 时延恢复 | 审计事件减少 |
| 9 | 回退 Push/Retry（关闭配置） | 保留克隆/拉取基线 | Clone/Fetch 成功率正常 | git_retry_total 降低 |
| 10 | 最终：`observability.enabled=false` | 仅保留最小事件流 | 基线维持 | git_task_duration_ms 仍可用（若已导出关闭则仅内部统计） |

执行 SOP：
1. 每步修改配置后 reload 或重启；
2. 观察 2 个窗口（1m/5m）指标变化与关键事件；
3. 记录采取的最小回退步数供事后复盘；
4. 恢复顺序与回退逆序（自底向上），每步 ≥30min 观察。

注意：步骤 1/2 之间可根据风险直接跳 10（彻底关闭观测）用于隔离性能雪崩；完成后务必捕获事故前后配置 diff。

---

## 附录D. Observability 配置字段速查（含默认值与语义）

| 路径 | 默认值 | 语义 / 约束 | 备注 |
|------|--------|-------------|------|
| observability.enabled | true | 顶层开关；false 时仅保留最小 basic 占位（不初始化 runtime） | 与 basicEnabled 组合决定实际激活 |
| observability.basicEnabled | true | 允许 basic 层；若 false 强制退化为 Basic 占位 | 需与 enabled 同时 true 才有效 |
| observability.aggregateEnabled | true | 允许窗口聚合初始化 | 依赖 basicEnabled |
| observability.exportEnabled | true | 允许启动 HTTP 导出服务器 | 依赖 aggregateEnabled |
| observability.uiEnabled | true | 允许前端面板使用快照/范围接口 | 依赖 exportEnabled |
| observability.alertsEnabled | true | 允许告警引擎加载/评估规则 | 依赖 uiEnabled |
| observability.layer | Optimize | 目标层级；受 flag 链与 max_allowed 限制 | 详见 §4.10 层级描述 |
| observability.autoDowngrade | true | 允许内存/资源事件触发自动降级 | 与 minLayerResidency/cooldown 共同限制频率 |
| observability.minLayerResidencySecs | 300 | 同一层级最短驻留时间 | 防止频繁震荡 |
| observability.downgradeCooldownSecs | 120 | 连续自动降级冷却窗口 | 冷却内忽略再次触发 |
| observability.export.authToken | null | Bearer Token；null 表示无鉴权 | 生产建议开启 |
| observability.export.rateLimitQps | 5 | 导出端点 QPS 令牌桶补充速率 | 0 表示不启用限流 |
| observability.export.maxSeriesPerSnapshot | 1000 | 单次 snapshot series 上限 | 超出被截断并计数 |
| observability.export.bindAddress | 127.0.0.1:9688 | 导出监听地址 | 只支持单地址（当前实现） |
| observability.alerts.rulesPath | config/observability/alert-rules.json | 告警规则文件路径 | 相对应用工作目录 |
| observability.alerts.evalIntervalSecs | 30 | 告警评估周期 | 过小增加 CPU/抖动风险 |
| observability.alerts.minRepeatIntervalSecs | 30 | Firing->Active 重复通知最小间隔 | 去抖/降噪 |
| observability.performance.batchFlushIntervalMs | 500 | 线程缓冲批量刷入间隔 | 降低锁争用；过大增加数据延迟 |
| observability.performance.tlsSampleRate | 5 | TLS Histogram 采样率（1/N） | 1=全采；增大降低精度 |
| observability.performance.maxMemoryBytes | 8000000 | 触发内存压力阈值；超过尝试降级 | 约 8MB 指标内存预算 |
| observability.performance.enableSharding | true | Histogram 分片以降低热点竞争 | 单核低并发可关闭 |
| observability.performance.debugMode | false | 额外内部调试日志（metrics target） | 仅临时排障启用 |
| observability.performance.redact.repoHashSalt | "" | 仓库名哈希盐；为空使用随机盐（进程级） | 变更会导致哈希重生成 |
| observability.performance.redact.ipMode | Mask | IP 脱敏模式（Mask/Hash/None） | Hash 利于聚合；None 谨慎使用 |
| (移除) internalConsistencyCheckIntervalSecs | (无) | 代码未实现；原文档残留 | 未来若实现需补充 |

使用守则：
1. 修改高频字段（rateLimitQps / batchFlushIntervalMs）前先在预生产压测；
2. 调低 maxMemoryBytes 仅用于压测或故障演练；
3. 生产导出必须配置 authToken（配合反向代理 TLS）；
4. 变更 repoHashSalt 会使前端缓存命中率短暂下降；
5. 观察自动降级：若 24h 内发生 >3 次，评估是否调高 minLayerResidencySecs 或优化指标写入；
6. 新增字段前：同时更新本附录、§4.10、附录B（若新增指标）。

