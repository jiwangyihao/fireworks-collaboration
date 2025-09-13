# 技术方案（git2-rs 版本）——从迁移到可控传输的完整路线

> 适用范围：以当前仓库状态为起点（已完成“旧版 P1.1 Git Fetch 基础”，后端采用 gitoxide/gix 路线），在不改变前端 API/事件/任务模型的前提下，完成“新 MP0：从 gitoxide 全面迁移到 git2-rs（libgit2 绑定）”，并规划自定义 smart subtransport（方式A：仅接管连接与 TLS/SNI）的落地路线与回滚策略。

---

## 0. TL;DR（给忙人看的）

- 为什么迁移：gitoxide 短期内没有 push 能力；git2-rs（libgit2）在 push/fetch/clone 上更成熟，生态验证充分。
- 新 MP0 目标：保持既有命令/事件/前端 UI 不变，后端 Git 实现从 gix 替换为 git2-rs；清理 gix 依赖与实现；全部单测通过。
- MP1 目标：在 git2-rs 基础上补齐 push，随后灰度引入“自定义 smart subtransport（SNI/证书校验进程内完成）”，淘汰 MITM 代理路径但保留回退开关。
- 不变部分：任务模型（task://state/progress）、配置命令（get_config/set_config）、HTTP 伪 SNI 调试 API（独立模块，继续可用）。

更新（2025-09-14）：已完成 MP0.4（切换、清理与基线）——后端统一使用 git2-rs；移除 gix 与相关特性开关；前后端全部测试通过，事件/进度/取消契约不变。

关联文档：
- 旧版 P0 交接稿（现状约定）：`doc/TECH_DESIGN_P0_HANDOFF.md`
- 旧版 P1 原路线（gitoxide 视角）：`doc/TECH_DESIGN_P1.md`
- 方式A详细方案与代码骨架：`new-doc/TECH_DESIGN_P1A_git2rs_custom_transport.md`
- 方式A迁移指南（从代理/MITM 切换）：`new-doc/transport-A-migration.md`

---

## 1. 范围与目标（新 MP0 → MP1 整体）

| 阶段 | 核心目标 | 对用户可见的变化 |
|------|----------|------------------|
| 新 MP0 | 用 git2-rs 替换 gix，实现 clone/fetch 的等价功能，清理 gix 依赖；保留命令名/事件结构/前端不变 | UI 与命令不变，稳定性与兼容性提升 |
| MP1.1 | Git Push（HTTPS 基础） | 新增 push 命令与 UI 表单（用户名+令牌或仅令牌），日志脱敏 |
| MP1.2 | 自定义 smart subtransport（方式A）灰度 | URL 重写至自定义 scheme（https+custom），对白名单主机启用内置 TLS/SNI 与证书 pin/TOFU，保留代理回退 |
| MP1.3 | Retry v1（HTTP/Git 早期错误） | 统一重试策略、指数退避；Push 仅在“进入上传前”允许重试 |
| MP1.4 | 事件增强与错误分类 | 丰富 progress 对象/字节/阶段；新增 task://error；前端可见最近错误与计数 |

不做（本阶段）：代理/IP 优选、系统级证书安装、凭证持久化、安全存储、浅克隆/部分克隆；详见 `doc/TECH_DESIGN_P1.md` 的“只做/不做”。

---

## 2. 新 MP0：从 gitoxide 迁移到 git2-rs（交付细则）

目标：
- 不改前端：命令签名、事件名与 store 结构保持兼容（`git_clone`、`task_cancel`、`task://state|progress`）。
- 后端等价：使用 git2-rs 实现 clone/fetch；确保取消、进度桥接与错误分类行为与现状一致或更优。
- 清理：移除 gix 依赖与源文件、测试替换为 git2-rs 版本；Cargo.lock/特性位更新。

### 2.1 代码变更点（Rust/Tauri）

- 依赖：
  - 移除：`gix`, `gix-transport` 等所有 gitoxide 相关 crates。
  - 新增/确认：`git2 = "0.19"`（与平台兼容的 libgit2 动态/静态链接配置按平台默认，必要时补充 build 注释）。

- 模块替换：
  - `src-tauri/src/core/git/*`：将 `clone`、`fetch` 实现由 gix 换为 git2-rs；保留统一入口 `service.rs` 风格（使用中性命名 `default_impl` 作为默认实现）。
  - 进度桥接：使用 `RemoteCallbacks::transfer_progress` 映射 objects/received_bytes/total_objects 至 `{ percent, phase, objects, bytes, totalHint }`。
  - 取消：基于 `AtomicBool` + 回调早退；在 `transfer_progress` 与网络阶段检查中断信号。
  - 错误分类：
    - 网络类（connect/reset/timeout）→ `Network`
    - TLS/证书错误 → `Tls`/`Verify`（按错误消息前缀归类）
    - 认证（401/403/Basic）→ `Auth`（为 P1 push 预留）
    - 用户取消 → `Cancel`
    - 其他 → `Internal/Protocol`

- 命令注册：`src-tauri/src/app.rs` 保持命令名与签名不变；内部改为调用 git2 实现。

- 事件：沿用 `task://state`/`task://progress`，payload 字段保持兼容；允许附加可选字段（前端已容忍未知字段）。

---

## 3. MP1 路线与分解（Push / Subtransport / Retry）

目标：在保持前端 API/事件不变的前提下，补齐 Push，并逐步接入“方式A：自定义 smart subtransport（仅接管连接与 TLS/SNI）”，同时引入 Retry v1。

- MP1.1 Push（HTTPS 基础）：支持 basic/token（PAT）认证，进度/取消/错误分类完善。
- MP1.2 Subtransport（方式A）灰度：对白名单主机启用 `https+custom` scheme，走内置 TLS（含 Fake SNI / Real-Host 校验），可一键回退。
- MP1.3 Retry v1：指数退避 + 类别化；Push 仅在“开始上传前”可重试；Clone/Fetch 按安全类别重试。

验收：
- 能对公开测试仓库完成 Push；错误可读、日志脱敏；可开关自定义 subtransport；所有单测通过。

---

## 4. Git Push（HTTPS）设计（git2-rs，面向 MP1）

- 凭证回调：`RemoteCallbacks::credentials` 支持两种：
  - 用户名 + 令牌（或密码）；
  - 仅令牌：用户名置为 `x-access-token`（兼容 GitHub），密码为 token。
- 进度：
  - `transfer_progress` 提供对象/字节统计；
  - Push 阶段用 `push_transfer_progress`（若可用）上报 `bytesSent`、`phase`（PreUpload/Upload/PostReceive）。
- 取消：
  - 在各回调与大循环检查取消标记，进入上传后不再自动重试，取消立即中止。
- 错误分类：
  - 401/403 归为 `Auth`；网络类 → `Network`；TLS/证书 → `Tls/Verify`；
  - 服务器拒绝（如权限不足、受保护分支）→ `Auth`/`Protocol`；
  - 上传中断 → `Network` 或 `Cancel`。

---

## 5. 自定义 smart subtransport（方式A，面向 MP1）

设计要点：
- 触发：只对白名单主机开启（如 github.com 域族）；URL 重写为 `https+custom://...`。
- 实现：注册自定义“smart subtransport”，仅接管连接建立（TCP/TLS），HTTP 语义仍由 libgit2 处理；
  - SNI 可为 Fake 或 Real；
  - TLS 使用 rustls，自定义验证器支持 SAN 白名单、可选 SPKI Pin、Real-Host 验证；
  - 代理模式下禁用 Fake SNI（减少可识别特征）。
- 回退：连接或验证失败 → 自动切回 Real SNI；仍失败 → 切换 IP（后期）/代理（若可用）；最终回退到 libgit2 默认。
- 安全基线：不关闭链验证；Fake SNI 仅改变握手 SNI，不降低验证强度。

集成步骤（灰度）：
1) 默认关闭，通过配置对白名单可单仓开启；
2) 记录 usedFakeSni/realHost 校验结果与证书指纹（后期指标/日志）。

---

## 6. Retry v1（统一重试策略）

- 重试类别：
  - 可重试：超时、连接重置、暂时性网络错误、5xx（幂等请求），TLS 握手失败可进行一次“Fake→Real”切换；
  - 不可重试：证书验证失败（SAN/SPKI）、认证失败、明确的协议错误、用户取消；
  - Push 限制：仅在进入上传前允许重试，若已发送 pack 则不再自动重试（避免重复写入）。
- 策略：指数退避 + 抖动（如 base=300ms, factor=1.5, max=6），阶段化上报到事件。
- 观察：记录最后一次错误及重试次数，便于前端提示。

---

## 7. 事件与进度契约（MP1 增强）

- `task://state`：`pending|running|completed|failed|canceled`。
- `task://progress`：
  - Clone/Fetch：`{ objects, totalHint, bytes, percent, phase }`
  - Push：`{ bytesSent, objects, percent, phase }`，phase 示例：`PreUpload|Upload|PostReceive`。
- `task://error`：`{ category, code?, message, retriedTimes? }`。
- 兼容性：新增字段保持可选；前端现有 UI 不破坏，后续可渐进增强展示。

---

## 8. 错误分类与回退链

分类：`Network | Tls | Verify | Protocol | Auth | Proxy | Cancel | Internal`。

回退顺序（可配置）：
1) Fake SNI → 2) Real SNI → 3) 换 IP（P5）→ 4) 代理/直连切换（P4）→ 5) SSH（P8）。

约束：
- Verify（SAN/SPKI）失败直接失败，不再尝试 Fake/Real 切换；
- Auth 失败直返，提示用户；
- Push 上传阶段出错仅回退一次到 Real SNI，不做自动重试；
- 代理失败达到阈值自动降级直连（并发出 `proxy://fallback` 事件）。

---

## 9. 配置模型（MP1 初版）

`config.json` 关键片段：
- `httpStrategy`: `{ fakeSniEnabled: boolean, fakeHost: string, enforceDomainWhitelist: boolean }`
- `retry`: `{ max: number, baseMs: number, factor: number, jitter: boolean }`
- `tls`: `{ spkiPins?: string[], realHostVerify: boolean }`
- `proxy`: `{ mode: 'off'|'http'|'socks5', url?: string }`
- `logging`: `{ debugAuthLogging: boolean }`（默认脱敏）

任务级覆盖（P2+）：命令入参可选择性覆盖上述策略子集。

---

## 10. TLS 与 Fake SNI/Real-Host 验证

- SAN 白名单：仅允许 Github 域族：`github.com/*.github.com/*.githubusercontent.com/*.githubassets.com/codeload.github.com`。
- SPKI Pin（可选）：开启后必须匹配，否则直接失败；用于高安全环境；
- Real-Host 验证：握手使用 Fake SNI 时，按“真实域名”进行证书匹配；若证书不含真实域名 → 立刻回退一次使用 Real SNI 重握手；
- 证书指纹：记录 leaf SPKI SHA256 与证书整体哈希（后期用于指标与告警）。

---

## 11. Git Smart HTTP 策略（阶段化）

- MP1：沿用 git2-rs 默认 HTTP，进度/取消/错误分类完善；
- P2：支持浅克隆（depth）、部分克隆（`filter=blob:none`）并对进度与速率做展示；
- P3：替换 transport 为方式A的自定义 subtransport（含 Fake/Real 回退与验证策略）。

重定向策略：默认跟随有限次（如 5 次），跨主域重定向需命中白名单。

---

## 12. 代理策略与回退

- 支持 HTTP CONNECT 与 SOCKS5；
- 失败判定：连接/握手/读写错误累计达到阈值触发降级直连，并发 `proxy://fallback`；
- 与 Fake SNI：代理模式禁用 Fake SNI（避免结合代理产生异常特征）；
- 恢复：后期通过心跳探测自动恢复代理（P4+）。

---

## 13. 可观测性体系（指标与事件）

为便于定位问题与优化性能，在现有事件基础上补充可选指标采集：

| 类别 | 指标示例 |
|------|----------|
| TLS | tls_handshake_ms |
| Git 阶段 | git_phase_duration_ms{phase} |
| IP 探测 | ip_probe_success_total{source} / ip_rtt_ms |
| 代理 | proxy_failover_total |
| 证书 | cert_fp_changes_total |
| Git 流量 | git_bytes_received_total |
| HTTP 调试 | http_fake_requests_total / http_fake_request_bytes_total |
| 大响应 | http_fake_large_response_total |

事件仍用于前端实时显示，指标用于后期统计/面板（P9）。

---

## 14. 配置文件与数据落地

| 文件 | 用途 |
|------|------|
| config.json | httpStrategy / httpFake / tls / proxy |
| ip-config.json | dns providers / scoring |
| ip-history.json | 历史成功失败与 RTT |
| cert-fp.log | 证书指纹追加日志（JSON line） |
| strategy.lock | 可选运行时快照 |
| lfs-cache/ | LFS 对象缓存（后期） |

热更新（后期）：文件变更 -> reload -> emit net://strategyChange。

---

## 15. 关键时序流程（HTTP / Git / 回退）

HTTP (MP0)：
1. 前端发起 http_fake_request → 解析 URL & 校验白名单 → 判定 useFakeSni
2. 若启用 IP 池（后期）→ 选 IP；TCP → TLS（SNI=伪或真实）
3. 发送请求与计时（connect/tls/firstByte/total）→ 返回 Base64 Body

Git Clone (MP0)：
1. 创建任务记录 → state(pending→running)
2. git2-rs 默认 HTTP → info/refs → Negotiation → Pack streaming
3. side-band → progress 事件 → 完成 → state=completed

Git 伪 SNI (P3)：
统一 transport：Fake SNI 握手失败 → Real SNI → IP fallback → 代理 fallback。
Real-Host 验证按真实域匹配（详见 §10），失败时回退真实 SNI 再试一次。

---

## 16. 核心数据结构（参考）

```rust
enum IpSource { Builtin, History, UserStatic, Dns, Fallback }

struct IpStat {
  ip: String,
  sources: Vec<IpSource>,
  rtt_avg: f32,
  rtt_jitter: f32,
  success: u32,
  failure: u32,
  last_success: Option<std::time::Instant>,
  last_failure: Option<std::time::Instant>,
  cooldown_until: Option<std::time::Instant>,
  score: f32
}

struct HttpStrategy {
  fake_sni_enabled: bool,
  retry: RetryCfg,
  timeout: TimeoutCfg
}

struct RetryCfg { max: u8, backoff_ms: u64, factor: f32 }
struct TimeoutCfg { connect: u64, tls: u64, first_byte: u64, overall: u64 }

struct HttpFakeReq {
  url: String,
  method: String,
  headers: std::collections::HashMap<String,String>,
  body_base64: Option<String>,
  timeout_ms: u64,
  force_real_sni: bool,
  no_ip_pool: bool,
  follow_redirects: bool,
  max_redirects: u8
}

struct HttpFakeResp {
  ok: bool,
  status: u16,
  headers: std::collections::HashMap<String,String>,
  body_base64: String,
  used_fake_sni: bool,
  ip: Option<String>,
  timing: TimingInfo,
  redirects: Vec<RedirectInfo>,
  body_size: usize
}

struct TimingInfo { connect_ms: u32, tls_ms: u32, first_byte_ms: u32, total_ms: u32 }
struct RedirectInfo { status: u16, location: String, count: u8 }

enum ErrorCategory { Network, Tls, Verify, Protocol, Proxy, Auth, Lfs, Cancel, Internal }

enum TaskState { Pending, Running, Completed, Failed, Canceled }
enum TaskKind {
  GitClone{ repo: String, dest: String },
  GitFetch{ repo: String },
  GitPush{ repo: String },
  HttpFake{ url: String, method: String }
}

struct TaskMeta {
  id: uuid::Uuid,
  kind: TaskKind,
  state: TaskState,
  started_at: std::time::Instant,
  canceled: tokio_util::sync::CancellationToken
}
```

---

## 17. 核心伪代码（摘录）

TLS 验证器（简化）：
```rust
impl ServerCertVerifier for GithubSanVerifier {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer,
    intermediates: &[CertificateDer],
    server_name: &ServerName,
    _ocsp: &[u8],
    now: SystemTime
  ) -> Result<ServerCertVerified, rustls::Error> {
    self.default.verify_server_cert(end_entity, intermediates, server_name, &[], now)?;
    if !self.san_allowed(end_entity)? { return Err(rustls_err("SAN mismatch")); }
    if self.pin_enabled && !self.spki_pinned(end_entity)? { return Err(rustls_err("SPKI pin mismatch")); }
    Ok(ServerCertVerified::assertion())
  }
}
```

IP 评分：
```rust
fn compute_score(stat: &IpStat, cfg: &ScoreCfg, now: Instant) -> f32 {
  let jitter_pen = stat.rtt_jitter * cfg.jitter_weight;
  let fail_recent = recent_fail_count(stat, cfg.recent_failure_decay_sec, now);
  let fail_pen = fail_recent as f32 * cfg.failure_weight;
  let src_pen = stat.sources.iter().map(|s| cfg.source_weight[s]).min().unwrap_or(0.0);
  let cooldown_pen = stat.cooldown_until.filter(|t| *t > now).map(|_| 10_000.0).unwrap_or(0.0);
  stat.rtt_avg + jitter_pen + fail_pen + src_pen + cooldown_pen
}
```

通用 HTTP 请求：
```rust
async fn http_fake_request(req: HttpFakeReq) -> Result<HttpFakeResp> {
  enforce_whitelist(&req)?;
  let use_fake = strategy.fake_sni_enabled && !req.force_real_sni && direct_mode();
  let ip = if req.no_ip_pool { None } else { ip_pool.pick()? };
  let (connect_ms, tcp) = timed_connect(ip.as_deref(), &req.url, proxy_cfg).await?;
  let sni = if use_fake { fake_host() } else { real_host(&req.url) };
  let (tls_ms, tls_stream) = tls_handshake(tcp, sni).await?;
  let r = send_and_collect(tls_stream, &req).await?;
  if r.body.len() as u64 > config.http_fake.large_body_warn_bytes {
    tracing::warn!("LargeBody size={}", r.body.len());
  }
  Ok(to_resp(r, use_fake, ip, connect_ms, tls_ms))
}
```

---

## 18. 仓库目录结构建议

```text
src/
  api/
    git.ts
    http.ts
    ip.ts
    strategy.ts
    tauri.ts
  stores/
    tasks.ts
    logs.ts
  views/
    GitPanel.vue
    HttpTester.vue
    NetworkInsights.vue
  components/
    ProgressBar.vue
    LogViewer.vue
src-tauri/
  src/
    main.rs
    api/
      git_api.rs
      http_fake_api.rs
      strategy_api.rs
      ip_api.rs
      tls_api.rs
    core/
      git/{mod.rs,service.rs,progress.rs}
      http/{mod.rs,client.rs}
      tls/{mod.rs,verifier.rs}
      ip_pool/{mod.rs,probe.rs}
      proxy/{mod.rs}
      tasks/{mod.rs,registry.rs,model.rs}
      config/{mod.rs,loader.rs}
      security/{fingerprint.rs}
      util/{base64.rs,retry.rs,time.rs}
    events/emitter.rs
config.json
ip-config.json
ip-history.json
cert-fp.log
```

---

## 19. 风险矩阵（补充）

| 风险 | 等级 | 描述 | 当前策略 | 后续强化 |
|------|------|------|----------|----------|
| HTTP API 被滥用 | 高 | 绕过安全代理访问任意域 | 默认 whitelist | 权限/速率限制 |
| 大响应内存压力 | 中 | 全量加载 | warn 阈值 | 流式读取 |
| 凭证日志泄漏 | 高 | Authorization 输出 | 默认脱敏 | 审计模式 |
| 伪 SNI 封锁 | 中 | 网络策略升级 | 自动回退 | 动态禁用策略 |
| DNS 污染 | 中 | 错误 IP | 多 DoH + fallback | ECH/ECS 研究 |
| 证书伪造 | 低 | 恶意证书 | SAN + 可选 Pin | Pin 轮换策略 |
| 代理失败未降级 | 低 | 停留不可达 | failure 阈值回退 | 自动恢复探测 |
| 并发资源争用 | 中 | 大量任务 | UI 限制提示 | 任务调度队列 |
| Pack 中断 | 中 | 重下载 | 整体重试 | Pack resume |
| IP 池污染 | 中 | 引入坏 IP | 失败惩罚 / cooldown | 信誉评分 |
| 指纹文件篡改 | 低 | 误导 Pin | 只追加 + 校验行格式 | Hash 链/签名 |
| 安全策略误配置 | 中 | 关闭 whitelist | UI 警告 | 安全模式一键恢复 |

---

## 20. 需求映射表（对齐自查）

| 原方案功能点 | 状态 | 落地阶段 | 仓库实现方式 |
|--------------|------|----------|--------------|
| 通用伪 SNI HTTP API | 调整为 MP0 | MP0 | http_fake_request |
| Clone 基础 | 保持 | MP0 | git2-rs + 任务管理 |
| Fetch / Push | 保持 | MP1 | git service 扩展 |
| Shallow / Partial | 保持 | P2 | depth/filter 参数 |
| 伪 SNI（Git） | 后移到 P3 | P3 | 统一 transport 替换 |
| SAN 白名单 | 强制 | MP0 | 自定义 verifier |
| SPKI Pin | 可选 | P7 | tls::verifier + config |
| 代理与回退 | 保留 | P4 | proxy::manager |
| IP 优选 5 来源 | 精简 | P5 | ip_pool with scoring |
| 任务事件/取消 | 保留 | MP0 | TaskRegistry + emit |
| 错误分类 | 保留 | 渐进 | ErrorCategory 枚举 |
| 指纹事件 | 保留 | P7 | security::fingerprint |
| 日志脱敏 | 建议 | MP0 | debugAuthLogging 开关 |
| 任务策略覆盖 | 保留 | P2+ | 覆盖 httpStrategy |
| LFS 下载 | 保留 | P6 | lfs::module |
| SSH fallback | 新蓝图后期 | P8 | 额外协议模块 |
| 性能指标面板 | 后期 | P9 | metrics + UI |
| HTTP 流式 | 展望 | P10 | streaming body |
| Pack resume | 展望 | P10 | pack 分段校验 |

---

## 21. 后续扩展议题（优先级待定）

| 议题 | 价值 |
|------|------|
| 安全模式快速开关 | 一键回退到安全限制配置 |
| Streaming Body | 降内存峰值 |
| Pack Resume | 减少重下载 |
| HTTP/2 / Multiplex | 降低握手成本 |
| ECH / SNI 混淆 | 更好穿透 |
| Geo/ASN 标签 | 区域感知 IP 评分 |
| 指标面板 | 运维与调优可视化 |
| LFS 分块续传 | 大文件性能 |
| Pin 轮换策略 | 安全可持续 |
| 访问行为审计 | 企业级安全治理 |

---

## 22. 完整 Roadmap（MP0–P10）

注：本路线路径以“先迁移、后增强”为原则，尽量保持前端 API/事件稳定，通过配置灰度与可回退策略降低风险。

### MP0 迁移到 git2-rs（已规划/进行中）
- 目标：替换 gix 为 git2-rs，保持 Clone/Fetch 等价能力与事件/取消一致。
- 交付：git2-rs 实现的 clone/fetch、进度桥接、错误分类、取消；移除 gix 依赖；单测全绿。
- 接口影响：无（命令/事件名不变）。
- 配置：无强制新增；日志脱敏开关建议默认开启。
- 验收：现有测试全部通过；手动克隆公共仓库成功并有进度。
- 风险与回退：编译/链接差异 → CI 预构建；如遇平台兼容问题，采用“版本回退”策略；开发阶段可临时使用 gix 构建开关做对比与定位，上线前清理；不提供“系统 git”兜底路径。

### MP1 Push + Subtransport(A)灰度 + Retry v1 + 事件增强
- 目标：打通 HTTPS Push；引入方式A自定义 subtransport（仅接管连接/TLS/SNI）并灰度；建立统一重试规则；完善事件。
- 交付：
  - Push：凭证回调（用户名+token/仅token），进度/取消/错误分类；
  - Subtransport(A)：对白名单主机启用 `https+custom`，Fake→Real 回退；
  - Retry v1：指数退避 + 类别化（Push 仅上传前重试）；
  - 事件：task://error + push 阶段化进度（bytesSent/phase）。
- 接口影响：新增 git_push 命令；进度 payload 可新增可选字段。
- 配置：`httpStrategy.fakeSniEnabled`、`retry.*`、`tls.realHostVerify`。
- 验收：能向公开测试仓库 push；方式A可灰度开关与一键回退。
- 风险与回退：Push 上传中断重复写入 → 仅上传前重试；方式A失败自动回退到 libgit2 默认。

### P2 Shallow/Partial + 任务级策略覆盖
- 目标：在大型仓库下减少数据量并提升速度；命令参数支持 depth 与 filter=blob:none；允许任务级临时覆盖 httpStrategy/retry 子集。
- 交付：git clone/fetch 的 depth 与 filter 参数；前端表单选项；按任务覆盖配置（白名单字段）。
- 接口影响：git_clone/git_fetch 命令新增可选参数；事件不变。
- 配置：保持全局默认 + 任务覆盖。
- 验收：对同一仓库，浅克隆耗时/字节显著下降；覆盖参数仅影响当前任务。
- 风险与回退：部分仓库对 partial 支持不佳 → 自动降级；参数校验与 UI 提示。

### P3 Git 伪 SNI 集成（方式A统一）
- 目标：将方式A用于 Git Smart HTTP；Fake SNI 与 Real-Host 验证配套，一次 Real SNI 回退；保留代理/直连回退链。
- 交付：统一 transport 插桩；Fake→Real→换 IP（占位）→代理/直连回退。
- 接口影响：无；事件中增加 usedFakeSni 等可选字段（调试）。
- 配置：白名单域、fakeHost、realHostVerify。
- 验收：在开启方式A时，常见网络策略下可连通；失败路径按回退链稳定收敛。
- 风险与回退：假 SNI 常拿不到含真实域名的证书 → 迅速回退 Real SNI；禁用方式A可立即恢复默认路径。

### P4 代理支持 + 自动回退
- 目标：支持 HTTP CONNECT 和 SOCKS5；代理失败阈值触发直连回退；后续自动恢复探测。
- 交付：代理配置与持久化；失败计数与回退事件 proxy://fallback。
- 接口影响：新增 set_proxy/get_proxy/disable_proxy。
- 配置：`proxy.mode/url`。
- 验收：代理可用时走代理；代理持续失败时降级直连并发事件。
- 风险与回退：与 Fake SNI 叠加导致指纹异常 → 代理模式禁用 Fake SNI。

### P5 IP 优选（5 来源）
- 目标：从 builtin/history/user_static/dns/fallback 聚合候选 IP，探测评分选优；失败惩罚与冷却；持久化历史。
- 交付：IP 池、探测、评分、历史文件；UI 展示（可选）；选择最优非冷却 IP。
- 接口影响：可选 ip_* 诊断命令。
- 配置：ip-config.json（权重/阈值）。
- 验收：在不稳定网络下连通性/时延中位数改善；失败后快速切换。
- 风险与回退：误选差 IP → 快速失败与冷却；保持 DNS 直连兜底。

### P6 LFS 下载
- 目标：支持 LFS 对象下载（读取指针、batch 协议、对象 GET），缓存目录与限流。
- 交付：lfs 模块、缓存目录、下载进度。
- 接口影响：可能新增 lfs_get 命令（或在 clone/fetch 流程内自动处理）。
- 配置：lfs-cache 路径与大小阈值。
- 验收：含 LFS 的仓库能顺利 clone 并下载对象；缓存命中有效。
- 风险与回退：大对象内存峰值 → 限流/分块；失败则回退普通流。

### P7 TLS 强化：SPKI Pin + TOFU + 指纹事件
- 目标：高安全环境下可选 SPKI pin；TOFU（首次信任）可选；记录证书指纹变化事件。
- 交付：pin 校验、指纹日志与事件；TOFU 数据文件（可选）。
- 接口影响：新增 tls_cert_fingerprints 查询。
- 配置：`tls.spkiPins[]`、`tls.enableTofu`。
- 验收：pin 不匹配时严格失败；指纹变更事件可观测。
- 风险与回退：证书轮换导致误报 → pin 轮换流程与 UI 引导。

### P8 SSH fallback（兜底）
- 目标：在极端网络下提供 SSH 路径作为最后兜底；与已有策略形成回退链的末端。
- 交付：ssh 模块（libssh2/ssh2-rs）；凭证/known_hosts 管理（最简）。
- 接口影响：可选 git_clone_ssh 命令或自动回退策略。
- 配置：ssh 开关与主机密钥策略。
- 验收：受限环境下能够完成 clone；安全基线不降低。
- 风险与回退：密钥管理复杂度 ↑ → 初期仅“已知主机”最小化实现。

### P9 可观测性：指标与轻量面板
- 目标：在事件基础上补充计数器/直方图，前端提供简单趋势面板。
- 交付：metrics 汇总（可选 exporter）；UI 面板。
- 接口影响：无；仅调试开关。
- 配置：metrics 启用、采样率。
- 验收：关键指标可见（握手时延、失败率、回退次数等）。
- 风险与回退：开销与隐私 → 采样/脱敏与本地存储。

### P10 性能与鲁棒增强：HTTP/2 / Streaming / Pack Resume（探索）
- 目标：降低握手开销、降低内存峰值、降低中断损耗。
- 交付：
  - HTTP/2 多路复用（兼容性评估）;
  - Streaming body（HTTP 调试）与 Git pack 流管线优化;
  - Pack resume（断点续传探索）。
- 接口影响：无；内部优化为主。
- 配置：性能开关与阈值。
- 验收：在大仓库/差网络场景，端到端耗时与内存占用可观改善（基线对比）。
- 风险与回退：服务器/代理兼容性 → 按域灰度与快速禁用；严格 A/B 对比与回滚开关。
