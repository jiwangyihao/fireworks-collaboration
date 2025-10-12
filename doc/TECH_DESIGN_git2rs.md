# 技术方案（git2-rs 版本）——从迁移到可控传输的完整路线

> 适用范围：以当前仓库状态为起点（已完成“旧版 P1.1 Git Fetch 基础”，后端采用 gitoxide/gix 路线），在不改变前端 API/事件/任务模型的前提下，完成“新 MP0：从 gitoxide 全面迁移到 git2-rs（libgit2 绑定）”，并规划自适应 TLS 传输层（原方式A：仅接管连接与 TLS/SNI）的落地路线与回滚策略。

---

## 0. TL;DR（给忙人看的）

- 为什么迁移：gitoxide 短期内没有 push 能力；git2-rs（libgit2）在 push/fetch/clone 上更成熟，生态验证充分。
- 新 MP0 目标：保持既有命令/事件/前端 UI 不变，后端 Git 实现从 gix 替换为 git2-rs；清理 gix 依赖与实现；全部单测通过。
- MP1 目标：在 git2-rs 基础上补齐 push，并灰度引入“自适应 TLS 传输层（原方式A，仅接管连接/TLS/SNI）”，失败自动回退；后续与 IP 池集成，连接时优选基于 TCP 握手延迟的评分最高 IP，只有在确认是 IP 连通性问题时才更换 IP，更换后仍先使用 Fake SNI。
- P2 将引入本地 Git 操作（init/add/commit/branch/tag/remote/checkout 等）与 shallow/partial，以及任务级策略覆盖；代理支持按规划延后到 P5。
- 路线图统一为 9 阶段（MP0、MP1、P2…P8），长期目标与现有技术方案保持一致。

更新（2025-09-14）：已完成 MP0.4（切换、清理与基线）——后端统一使用 git2-rs；移除 gix 与相关特性开关；前后端全部测试通过，事件/进度/取消契约不变。

更新（2025-09-15）：完成 MP1.2（自适应 TLS 传输层灰度）的关键实现与前后端对齐：
- 配置：从模型中移除 `http.fakeSniHost`，改为使用 `http.fakeSniHosts: string[]` 候选，运行期维护 last-good SNI；
- 轮换：`403` 仅在 `GET /info/refs` 阶段按流单次轮换，排除当前 SNI，随机其余候选，否则回退 Real；
- TLS：移除 `tls.insecureSkipVerify`/`tls.skipSanWhitelist` 等跳过开关，统一在 Fake SNI 场景挂载 `RealHostCertVerifier` 以真实域名校验证书与 SPKI pin；
- 代理：检测到代理时禁用 Fake SNI 与 URL 改写；
- 配置热加载路径一致：subtransport 读取 app_config_dir 注入的全局 base dir，保存后即时生效；
- 可观测性：HTTP 嗅探与调试日志完善，默认脱敏；
- 前端：移除 fakeSniHost UI，新增“跳过 SAN 白名单校验”选项，保持 API 兼容。

更新（2025-09-15）：完成 MP1.3（Push 使用自定义 Subtransport(A)）与配套改进：
- Push 流程启用与 clone/fetch 一致的 `https+custom` 改写与回退链，保持灰度与代理互斥路径；
- 线程局部注入 Authorization，仅限 receive-pack 流程（`GET info/refs?service=git-receive-pack` 与 `POST /git-receive-pack`），clone/fetch 不受影响；
- 将 receive-pack 的 401 明确映射为 `Auth` 类错误，避免“bad packet length”误导；
- 传输层与默认 Git 实现拆分为模块目录（`core/git/transport/{mod.rs,register.rs,rewrite.rs,http/{mod.rs,auth.rs,util.rs,stream.rs}}` 与 `core/git/default_impl/*`），旧文件归档；
- 统一配置读取路径到应用数据目录；新增/完善公共 E2E（clone/fetch）默认启用，CI 环境可通过环境变量禁用；
- 构建无警告（清理 `private_interfaces`），所有 Rust/前端测试通过。

补充（模块结构与归档说明）：

- 对外导出点：`transport/mod.rs` 暴露 `ensure_registered`、`maybe_rewrite_https_to_custom`、`set_push_auth_header_value`；其中 `set_push_auth_header_value` 由 `transport/http/auth.rs` 提供并在 `mod.rs` 中 re-export。
- 目录化拆分：原 `transport/http.rs` 拆分为 `http/{mod.rs,auth.rs,util.rs,stream.rs}`；为避免历史文件与目录模块重名导致 `E0761`，通过 `#[path = "http/mod.rs"]` 将 `transport::http` 绑定到目录模块。
- 归档：历史 `transport/http.rs` 已移动到 `src-tauri/_archive/http.legacy_YYYYMMDD_HHMMSS.rs`，方便回溯而不影响编译。

关联文档：
- 旧版 P0 交接稿（现状约定）：`doc-archive/TECH_DESIGN_P0_HANDOFF.md`
- 旧版 P1 原路线（gitoxide 视角）：`doc-archive/TECH_DESIGN_P1.md`
 - MP0 实施交接稿（git2-rs 基线落地细则）：`doc/MP0_IMPLEMENTATION_HANDOFF.md`
 - MP1 实施交接稿（Push + 自适应 TLS 灰度 + Retry v1）：`doc/MP1_IMPLEMENTATION_HANDOFF.md`
 - MP0/MP1 规划补充（计划层面）：`doc/TECH_DESIGN_MP0_PLAN.md`、`doc/TECH_DESIGN_MP1_PLAN.md`

---

## 1. 范围与目标（新 MP0 → MP1 整体）

| 阶段 | 核心目标 | 对用户可见的变化 |
|------|----------|------------------|
| 新 MP0 | 用 git2-rs 替换 gix，实现 clone/fetch 的等价功能，清理 gix 依赖；保留命令名/事件结构/前端不变 | UI 与命令不变，稳定性与兼容性提升 |
| MP1.1 | Git Push（HTTPS 基础） | 新增 push 命令与 UI 表单（用户名+令牌或仅令牌），日志脱敏 |
| MP1.2 | 自适应 TLS 传输层（灰度） | URL 重写至自定义 scheme（https+custom），对白名单主机启用内置 TLS/SNI 与证书 pin/TOFU，保留代理回退 |
| MP1.4 | Retry v1（HTTP/Git 早期错误） | 统一重试策略、指数退避；Push 仅在“进入上传前”允许重试 |
| MP1.5 | 事件增强与错误分类 | 丰富 progress 对象/字节/阶段；新增 task://error；前端可见最近错误与计数 |

不做（本阶段）：代理/IP 优选、系统级证书安装、凭证持久化、安全存储、浅克隆/部分克隆；详见 `doc-archive/TECH_DESIGN_P1.md` 的“只做/不做”。

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

### 2.2 MP1：Push/自适应 TLS 灰度/Retry v1（已完成）

目标：在保持前端 API/事件不变的前提下，补齐 Push，并灰度接入“自适应 TLS 传输层（仅接管连接与 TLS/SNI）”，同时引入 Retry v1。

- Push（HTTPS 基础）：支持 basic/token（PAT）认证，进度/取消/错误分类完善。
- Subtransport（自适应 TLS 传输层）灰度：对白名单主机启用 `https+custom` scheme，走内置 TLS（含 Fake SNI / Real-Host 校验），可一键回退。
- Retry v1：指数退避 + 类别化；Push 仅在“开始上传前”可重试；Clone/Fetch 按安全类别重试。

验收：
- 能对公开测试仓库完成 Push；错误可读、日志脱敏；可开关自定义 subtransport；所有单测通过。

Push（HTTPS）设计（git2-rs）：
- 凭证回调：`RemoteCallbacks::credentials` 两种：用户名+令牌（或密码）；仅令牌（用户名 `x-access-token`，密码为 token）。
- 进度：`transfer_progress` 与 `push_transfer_progress`（若可用）上报 `bytesSent` 与 `phase=PreUpload|Upload|PostReceive`。
- 取消：进入上传后不再自动重试，取消立即中止。
- 错误分类：401/403→Auth；网络→Network；TLS/证书→Tls/Verify；权限/受保护分支→Auth/Protocol；上传中断→Network/Cancel。

---

## 3. 事件与进度契约（MP1 增强）

- `task://state`：`pending|running|completed|failed|canceled`。
- `task://progress`：
  - Clone/Fetch：`{ objects, totalHint, bytes, percent, phase }`
  - Push：`{ bytesSent, objects, percent, phase }`，phase 示例：`PreUpload|Upload|PostReceive`。
- `task://error`：`{ category, code?, message, retriedTimes? }`。
- 兼容性：
  - 新增字段保持可选；前端现有 UI 不破坏，后续可渐进增强展示。
  - 大小写兼容：`retriedTimes` | `retried_times`（错误事件）；`totalHint` | `total_hint`（进度事件）。

### 3.1 命令清单（对前端稳定）

- `git_clone(repo: string, dest: string): Promise<string /* taskId */>`
- `git_fetch(repo: string, dest: string, preset?: 'remote'|'branches'|'branches+tags'|'tags'): Promise<string>`
- `git_push({ dest: string; remote?: string; refspecs?: string[]; username?: string; password?: string }): Promise<string>`
- `task_cancel(id: string): Promise<boolean>`
- `task_list(): Promise<TaskSnapshot[]>`

说明：P2 起在不破坏上述签名的前提下，新增“对象参数重载”，以支持 shallow/partial 与任务级策略覆盖，详见 §18。

## 4. 自适应 TLS 传输层（原方式A，面向 MP1）

设计要点：
- 触发：只对白名单主机开启（如 github.com 域族）；URL 重写为 `https+custom://...`。
- 实现：注册自定义“smart subtransport”，仅接管连接建立（TCP/TLS），HTTP 语义仍由 libgit2 处理；
  - SNI 可为 Fake 或 Real；
  - TLS 使用 rustls，自定义验证器支持 SAN 白名单、可选 SPKI Pin、Real-Host 验证；
  - 代理模式下禁用 Fake SNI（减少可识别特征）。
  - 与 IP 池协作（启用时）：连接前由 IP 池基于最新的 TCP 握手延迟（按域名+端口缓存，列表域名在启动时预热，其余按需采样）提供评分最高的候选 IP；仅在确认是 IP 连通性问题时才触发更换，更换后仍优先使用 Fake SNI，再按回退链进行。
- 回退：连接或验证失败 → 自动切回 Real SNI；仍失败 → 切换 IP（后期）/代理（若可用）；最终回退到 libgit2 默认。
- 安全基线：不关闭链验证；Fake SNI 仅改变握手 SNI，不降低验证强度。

集成步骤（灰度）：
1) 默认关闭，通过配置对白名单可单仓开启；
2) 记录 usedFakeSni/realHost 校验结果与证书指纹（后期指标/日志）。

实现挂接点（简要）：
- URL 改写：`transport::maybe_rewrite_https_to_custom(url)`
- 传输注册：`transport::ensure_registered()`（启动时一次）
- Push 授权注入：`transport::set_push_auth_header_value(value)`（仅 receive-pack）
- IP 池对接：在建立 TCP 前调用 `ip_pool.pick_best(host)`，失败后根据连通性标记 `ip_pool.report_fail(ip)` 并择优重选
- 代理互斥：检测代理配置后强制 Real SNI，跳过 Fake SNI 分支

---

## 5. Retry v1（统一重试策略）

- 重试类别：
  - 可重试：超时、连接重置、暂时性网络错误、5xx（幂等请求），TLS 握手失败可进行一次“Fake→Real”切换；
  - 不可重试：证书验证失败（SAN/SPKI）、认证失败、明确的协议错误、用户取消；
  - Push 限制：仅在进入上传前允许重试，若已发送 pack 则不再自动重试（避免重复写入）。
- 策略：指数退避 + 抖动（如 base=300ms, factor=1.5, max=6），阶段化上报到事件。
- 观察：记录最后一次错误及重试次数，便于前端提示。

---


----

## 6. 关键时序流程（HTTP / Git / 回退）

HTTP (MP0/MP1)：
1. 前端通过 `tauriFetch`（封装 `http_fake_request`，默认补全 `User-Agent: fireworks-collaboration/tauri-fetch` 并保留 Authorization 等头部）发起请求 → 解析 URL & 校验白名单 → 判定 useFakeSni
2. 若启用 IP 池（后期）→ 选 IP；TCP → TLS（SNI=伪或真实；代理模式下强制真实 SNI）
3. 发送请求与计时（connect/tls/firstByte/total）→ 返回 Base64 Body；若 `status=403` 且启用 403 轮换，在信息发现阶段（GET /info/refs）随机切换至不同候选后重试一次

Git Clone (MP0)：
1. 创建任务记录 → state(pending→running)
2. git2-rs 默认 HTTP → info/refs → Negotiation → Pack streaming
3. side-band → progress 事件 → 完成 → state=completed

Git 自适应 TLS（P3 全量推广）：
统一 transport：Fake SNI 握手失败 → Real SNI → IP fallback（P4）→ 代理 fallback（P5）。
Real-Host 验证按真实域匹配（详见 §8），失败时回退真实 SNI 再试一次。

---

## 7. 配置模型（MP1 更新）

`config.json` 关键片段：
- `http`：`{ fakeSniEnabled: boolean, fakeSniHosts?: string[], sniRotateOn403?: boolean, followRedirects: boolean, maxRedirects: number, largeBodyWarnBytes: number }`
- `tls`：`{ spkiPins?: string[], metricsEnabled?: boolean, certFpLogEnabled?: boolean, certFpMaxBytes?: number }`
- `retry`：`{ max: number, baseMs: number, factor: number, jitter: boolean }`（规划项，MP1.4）
- `proxy`: `{ mode: 'off'|'http'|'socks5', url?: string }`
- `logging`: `{ debugAuthLogging: boolean }`（默认脱敏）

任务级覆盖（P2+）：命令入参可选择性覆盖上述策略子集。

兼容说明：配置键内部统一使用 camelCase；输入端容忍 snake_case（例如 `base_ms` → `baseMs`）。`retry` 默认值建议 `{ max: 6, baseMs: 300, factor: 1.5, jitter: true }`。

P2 起的任务级覆盖对象（strategyOverride）结构补充：
```
strategyOverride?: {
  http?: { followRedirects?: boolean; maxRedirects?: number },
  tls?: { spkiPins?: string[]; metricsEnabled?: boolean; certFpLogEnabled?: boolean; certFpMaxBytes?: number },
  retry?: { max?: number; baseMs?: number; factor?: number; jitter?: boolean }
}
```
合并语义：对全局配置做浅合并（shallow merge），未提供字段沿用全局；越权字段忽略并记录告警。

示例配置（片段）：

```
{
  "http": {
    "fakeSniEnabled": false,
    "fakeSniHosts": ["a.githubapp.com", "b.githubapp.com"],
    "sniRotateOn403": true,
    "followRedirects": true,
    "maxRedirects": 5,
    "largeBodyWarnBytes": 10485760
  },
  "tls": {
    "spkiPins": [],
    "metricsEnabled": false,
    "certFpLogEnabled": false,
    "certFpMaxBytes": 4096
  },
  "retry": { "max": 6, "baseMs": 300, "factor": 1.5, "jitter": true },
  "proxy": { "mode": "off" },
  "logging": { "debugAuthLogging": false }
}
```

默认值建议（摘要）：
- http.followRedirects=true，maxRedirects=5，largeBodyWarnBytes=10MB
- retry={max:6, baseMs:300, factor:1.5, jitter:true}
- proxy.mode=off
- logging.debugAuthLogging=false（默认脱敏）

### 7.1 配置文件与数据落地（由原 §11 合并）

| 文件 | 用途 |
|------|------|
| config.json | httpStrategy / httpFake / tls / proxy |
| ip-config.json | dns providers / 预热域名列表 / 评分 TTL |
| ip-history.json | 最近一次 TCP 握手采样（延迟 / 来源 / 过期时间） |

预热与评分策略：
- `ip-config.json` 内新增 `preheatDomains` 与 `scoreTtlSeconds`。预热列表中的域名在进程启动后立即从全部可用来源（内置、DNS、历史、用户静态、兜底）收集 IP，并分别对目标端口（Github 场景默认 443，同时兼容 80）执行一次 TCP 握手测速，记录最优延迟。
- 非预热域名在第一次选址时才触发同样的 IP 收集与测速；完成后写入 `ip-history.json` 并带上 `expires_at`。
- 评分仅等于最近一次 TCP 握手延迟（毫秒），延迟越小优先级越高；不存在复杂权重或失败惩罚。
- 评分在 `scoreTtlSeconds` 后过期（默认 300 秒）。预热域名到期后后台自动刷新；非预热域名到期后条目被移除，下一次访问重新采样。
| cert-fp.log | 证书指纹追加日志（JSON line） |
| strategy.lock | 可选运行时快照 |
| lfs-cache/ | LFS 对象缓存（后期） |

热更新（后期）：文件变更 -> reload -> emit net://strategyChange。

---

## 8. TLS 与 Fake SNI/Real-Host 验证

- SAN 白名单：仅允许 Github 域族：`github.com/*.github.com/*.githubusercontent.com/*.githubassets.com/codeload.github.com`。
- SPKI Pin（可选）：开启后必须匹配，否则直接失败；用于高安全环境；
- Real-Host 验证：握手使用 Fake SNI 时，按“真实域名”进行证书匹配（通过 override_host）；若证书不含真实域名 → 立刻回退一次使用 Real SNI 重握手；
- 证书指纹：记录 leaf SPKI SHA256 与证书整体哈希（后期用于指标与告警）。

---

## 9. 代理策略与回退（P5，延后阶段）

- 支持 HTTP CONNECT 与 SOCKS5；
- 失败判定：连接/握手/读写错误累计达到阈值触发降级直连，并发 `proxy://fallback`；
- 与 Fake SNI：代理模式禁用 Fake SNI（避免结合代理产生异常特征）；
- 恢复：后期通过心跳探测自动恢复代理（P5+）。

---

## 10. 错误分类与回退链

分类：`Network | Tls | Verify | Protocol | Auth | Proxy | Cancel | Internal`。

回退顺序（可配置）：
1) Fake SNI → 2) Real SNI → 3) 换 IP（P4）→ 4) 代理/直连切换（P5）。

约束：
- Verify（SAN/SPKI）失败直接失败，不再尝试 Fake/Real 切换；
- Auth 失败直返，提示用户；
- Push 上传阶段出错仅回退一次到 Real SNI，不做自动重试；
- 代理失败达到阈值自动降级直连（并发出 `proxy://fallback` 事件）。

---

## 11. 核心数据结构（参考）

```rust
enum IpSource { Builtin, History, UserStatic, Dns, Fallback }

struct IpStat {
  ip: String,
  port: u16,
  sources: Vec<IpSource>,
  latency_ms: u32,
  measured_at: std::time::Instant,
  expires_at: std::time::Instant,
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

## 12. 核心伪代码（摘录）

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
fn ensure_latency_score(domain: &str, port: u16, now: Instant) -> Option<IpStat> {
  if let Some(entry) = cache.get(domain, port).filter(|s| s.expires_at > now) {
    return Some(entry);
  }

  let ttl = cfg.score_ttl_seconds;
  let ips = collect_all_sources(domain);
  let mut best: Option<IpStat> = None;
  for (ip, sources) in ips {
    if let Ok(latency_ms) = tcp_handshake_latency(&ip, port) {
      let measured = IpStat {
        ip,
        port,
        sources,
        latency_ms,
        measured_at: now,
        expires_at: now + Duration::from_secs(ttl),
      };
      cache.upsert(domain, port, measured.clone());
      if best.as_ref().map(|b| latency_ms < b.latency_ms).unwrap_or(true) {
        best = Some(measured);
      }
    }
  }
  best
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

## 13. 仓库目录结构建议

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

## 14. 风险矩阵（补充）

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
| IP 池污染 | 中 | 引入坏 IP | 过期后自动重新测速 | 来源白名单 + 延迟采样 |
| 指纹文件篡改 | 低 | 误导 Pin | 只追加 + 校验行格式 | Hash 链/签名 |
| 安全策略误配置 | 中 | 关闭 whitelist | UI 警告 | 安全模式一键恢复 |

---

## 15. 需求映射表（对齐自查）

| 原方案功能点 | 状态 | 落地阶段 | 仓库实现方式 |
|--------------|------|----------|--------------|
| 通用伪 SNI HTTP API | 调整为 MP0 | MP0 | http_fake_request |
| Clone 基础 | 保持 | MP0 | git2-rs + 任务管理 |
| Fetch / Push | 保持 | MP1 | git service 扩展 |
| Shallow / Partial | 保持 | P2 | depth/filter 参数 |
| 伪 SNI（Git） | 后移到 P3 | P3 | 统一 transport 替换 |
| SAN 白名单 | 强制 | MP0 | 自定义 verifier |
| SPKI Pin | 可选 | P7 | tls::verifier + config |
| 代理与回退 | 保留 | P5 | proxy::manager |
| IP 优选 5 来源 | 精简 | P4 | ip_pool with latency scoring |
| 任务事件/取消 | 保留 | MP0 | TaskRegistry + emit |
| 错误分类 | 保留 | 渐进 | ErrorCategory 枚举 |
| 指纹事件 | 保留 | P7 | security::fingerprint |
| 日志脱敏 | 建议 | MP0 | debugAuthLogging 开关 |
| 任务策略覆盖 | 保留 | P2+ | 覆盖 httpStrategy |
| LFS 下载 | 保留 | P7 | lfs::module |
| 可观测性面板 | 后期 | P8 | metrics + UI |
| HTTP 流式 | 展望 | 远期 | streaming body |
| Pack resume | 展望 | 远期 | pack 分段校验 |

---

## 16. 后续扩展议题（优先级待定）

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

## 17. 完整 Roadmap（MP0–P8 + 远期）

注：本路线图遵循“先稳定基线、再灰度增强、最后全量推广”的策略，保持对前端 API/事件的向后兼容，并为每项能力提供可回退开关。

HTTP 策略摘要（由原 §11 合并）：
- MP1：沿用 git2-rs 默认 HTTP，进度/取消/错误分类完善；
- P2：支持浅克隆（depth）、部分克隆（`filter=blob:none`）并对进度与速率做展示；
- P3：自适应 TLS 传输层全量推广（默认启用，仍可关闭），含 Fake/Real 回退与验证策略；
- P4：启用 IP 池/优选作为可选增强，仅以 TCP 握手延迟为评分依据，自适应 TLS 在连接前选取延迟最低的 IP。
重定向策略：默认跟随有限次（如 5 次），跨主域重定向需命中白名单。

### MP0 — git2-rs 基线（已完成）
- 目标：用 git2-rs 等价替换 gix；实现 clone/fetch；进度桥接、取消与错误分类；HTTP 伪 SNI 调试 API 保留。
- 接口：命令不变；事件 `task://state|progress`。
- 验收：前后端测试全绿；公共仓库克隆冒烟通过。
- 回退：版本回退；不依赖系统 git。

### MP1 — Push + 自适应 TLS（灰度）+ Retry v1 + 事件增强（已完成）
- 目标：HTTPS push（凭证回调）、引入“自适应 TLS 传输层（原方式A，仅接管连接/TLS/SNI）”并灰度；统一 Retry v1；新增 task://error 与 push 阶段化进度。
- 接口：新增 `git_push`；事件可带 `phase=PreUpload|Upload|PostReceive`、`retriedTimes?`。
- 验收：本地/公开仓库 push 冒烟；失败场景自动回退；日志脱敏。
- 回退：一键关闭自适应 TLS；失败链 Fake→Real→libgit2 默认。

### P2 — 本地 Git 操作 + Shallow/Partial + 任务级策略覆盖
- 目标：
  - 本地 Git：`init/add/commit/branch/checkout/tag/remote(set-url/add/remove)` 等常用操作；
  - clone/fetch 支持 `depth` 与 `filter`（如 `blob:none`）；
  - 任务级覆盖 `http?`/`tls?`/`retry?` 子集（浅合并全局，仅当前任务生效）。
- 接口：为本地操作新增命令；在 `git_clone`/`git_fetch` 入参新增 `depth?`、`filter?` 与任务级策略子对象。
- 验收：本地操作单测覆盖；depth/filter 在本地与中等体量仓库场景验证有效；互斥校验（代理×Fake SNI）有告警。
- 回退：depth/filter 不支持时退回全量；无效覆盖项忽略并告警。

### P3 — 自适应 TLS 传输层全量推广与可观测性强化
- 目标：对白名单域默认启用（仍可关闭）；完善观测（TLS/阶段耗时、证书指纹变更统计）；保持回退链。
- 接口：无破坏性变更；可选调试字段如 `usedFakeSni?`。
- 验收：长时稳定；失败回退路径覆盖；无敏感泄漏。
- 回退：总开关关闭自适应 TLS；回到 libgit2 默认传输。

### P4 — IP 优选与 IP 池（独立阶段）
- 目标：引入 IP 池与评分；评分仅基于最近一次 TCP 握手延迟，连接前自动选择延迟最低的 IP 并按照 TTL 定期刷新。
- IP 来源：`builtin`/`history`/`user_static`/`dns`/`fallback`。
- 预热域名列表（配置）在启动时对 80/443 端口预采样；列表外域名按需采样，记录 5 分钟（可配置）TTL，过期即重测或清除。
- 切换策略：仅在“确认是 IP 连通性问题”时更换 IP；更换后仍先用 Fake SNI，再按回退链进行。
- 验收：在不稳定网络下连通性/时延中位数提升；故障注入下能快速切换与冷却。
- 回退：关闭 IP 池功能，回到系统解析。

### P5 — 代理支持与自动降级（延后阶段）
- 目标：支持 HTTP/HTTPS 代理与 SOCKS5；失败达到阈值时自动降级直连并可恢复。
- 互斥：代理模式下强制 Real SNI（与 Fake SNI 互斥）。
- 接口/配置：`proxy.mode/url` 与阈值、冷却期；可选 `proxy://fallback` 事件。
- 验收：故障注入验证降级/恢复；事件一致。
- 回退：禁用自动降级；强制代理或直连配置。

### P6 — 凭证存储与安全
- 目标：安全管理 Token/密码（系统安全存储或加密文件）；默认脱敏日志。
- 接口：命令层尽量不变；可新增凭证管理命令。
- 验收：安全扫描通过；过期/撤销流程明确。
- 回退：关闭存储功能不影响 Push 基线。

### P7 — LFS 基础支持
- 目标：识别 LFS，下载基础路径与缓存；限流降低峰值。
- 接口：可在 clone/fetch 中自动处理或提供单独命令。
- 验收：含 LFS 仓库流程可用；缓存命中有效。
- 回退：失败回退普通流并提示。

### P8 — 可观测性面板与指标汇聚
- 目标：将 TLS 时延、阶段耗时、代理/IP 失败率、证书指纹变更等指标汇聚到轻量面板（默认关闭）。
- 接口：无；基于事件与可选指标导出。
- 验收：指标可视化与开销可控；隐私合规（脱敏/本地）。
- 回退：关闭面板与指标收集。

附加说明（指标与事件摘要，由原 §13 合并）：
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

事件仍用于前端实时显示，指标用于后期统计/面板（P8）。

---

## 18. P2 本地 Git 操作（契约草案）

目标：提供常用本地 Git 操作能力，统一事件与错误分类，满足 UI 常用工作流。

命令（初版建议）：
- `git_init({ dest: string }): Promise<string /* taskId */>`
- `git_add({ dest: string; paths: string[] }): Promise<string>`
- `git_commit({ dest: string; message: string; author?: { name: string; email: string }; allowEmpty?: boolean }): Promise<string>`
- `git_branch({ dest: string; name: string; checkout?: boolean; force?: boolean }): Promise<string>`
- `git_checkout({ dest: string; ref: string; create?: boolean }): Promise<string>`
- `git_tag({ dest: string; name: string; message?: string; annotated?: boolean; force?: boolean }): Promise<string>`
- `git_remote_set({ dest: string; name: string; url: string }): Promise<string>`
- `git_remote_add({ dest: string; name: string; url: string }): Promise<string>`
- `git_remote_remove({ dest: string; name: string }): Promise<string>`

事件：
- 多数本地操作仅发 `task://state`（pending→running→completed/failed/canceled）。
- 如需耗时统计，可发送一次 `task://progress`（phase=Running, percent=100）。

错误分类：
- 路径/权限 → `Internal` 或 `Protocol`（按错误源分级）；
- 冲突/不可快进 → `Protocol`；
- 用户取消 → `Cancel`。

边界与校验：
- `git_add` 空路径或不存在路径报错；
- `git_commit` 默认拒绝空提交（除非 `allowEmpty=true`）；
- `git_branch` 已存在分支且 `force=false` 报错；
- `git_checkout` 不存在 ref 且 `create=false` 报错；
- `git_remote_*` 名称与 URL 校验，保持幂等（set 覆盖、add 要求不存在、remove 要求存在）。

验收：
- 覆盖 happy path + 2 个失败用例/命令；
- 事件有序，状态与错误分类正确；
- 与后续 `git_push`/`git_fetch` 流程无缝衔接（remote 正确生效）。

---

### 18.1 Shallow/Partial（depth/filter）合同

适用命令：`git_clone` 与 `git_fetch`

输入扩展（对象重载，不破坏原签名；前端保持向后兼容）：
- `git_clone({ repo: string; dest: string; depth?: number; filter?: string; strategyOverride?: Partial<StrategyCfg> }): Promise<string>`
- `git_fetch({ repo: string; dest: string; preset?: 'remote'|'branches'|'branches+tags'|'tags'; depth?: number; filter?: string; strategyOverride?: Partial<StrategyCfg> }): Promise<string>`

参数约束：
- `depth`: 可选，正整数；`1` 表示只要最新一层历史；`0` 或负值视为无效（报错）。
- `filter`: 可选，字符串；首版仅允许：`'blob:none' | 'tree:0'`；非法值报错；与 `depth` 可同时存在。
- 同时指定时的协同：二者叠加约束内容，即“部分+浅”；实现层使用 git2-rs 对应选项组合。
- `strategyOverride`: 可选，任务级策略覆盖子对象，仅允许覆盖 `http/tls/retry` 的安全子集；非法键忽略并在日志告警。

默认行为：两参数缺省等价“全量/完整历史”；当后端不支持 `filter`（环境或远端不兼容）时，回退到不带 `filter` 的浅克隆或全量，按“最接近”原则进行，并在进度完成后追加一次 `task://error`（category=Protocol，message="partial filter not supported, fell back to ..."）。

错误映射：
- 非法 `depth`/`filter` → `Protocol`（code=`InvalidArgument`）。
- 远端不支持 partial → `Protocol`（code=`PartialNotSupported`，可伴随回退成功或失败）。
- 服务器拒绝浅操作（策略限制）→ `Auth|Protocol`，带原始消息摘要。

事件示例（clone，部分 + 浅）：
- 进度：`task://progress { phase: "Negotiation", objects, totalHint }` → `task://progress { phase: "Pack", bytes, percent }`。
- 完成：`task://state completed`，若发生回退：追加 `task://error { category: "Protocol", message: "partial unsupported; fallback=shallow(depth=1)" }`。

接受标准：
- 任一参数单独启用、或二者叠加时，完成率、对象数与体积显著小于全量；
- 失败或不支持时能清晰告知并按最近原则回退；
- 与 Retry v1 协同，重试不改变 depth/filter 组合，仅在安全类别下进行。

---

### 18.2 任务级策略覆盖（strategyOverride）

结构示例：
```
strategyOverride: {
  http?: { followRedirects?: boolean; maxRedirects?: number },
  tls?: { spkiPins?: string[]; metricsEnabled?: boolean; certFpLogEnabled?: boolean; certFpMaxBytes?: number },
  retry?: { max?: number; baseMs?: number; factor?: number; jitter?: boolean }
}
```

约束：
- 仅允许覆盖声明字段；越权字段忽略并记录告警。
- 与全局配置合并策略：浅合并（shallow merge），未提供字段沿用全局。
- 安全护栏：若启用代理（P5），则强制 `http.fakeSniEnabled=false`，并在任务开始时追加一次告警事件：`task://error { category: "Proxy", message: "proxy mode forces real SNI" }`（不阻断任务）。

---

### 18.3 最小测试矩阵（P2）

A. 本地 Git 命令（每条覆盖三例）
- Happy path：成功路径
- 参数非法：触发 `Protocol` 错误
- 用户取消：`Cancel`

B. Shallow/Partial
- clone depth=1（无 filter）→ 成功；对象/体积小于全量
- clone filter=blob:none（无 depth）→ 成功；体积显著下降
- clone depth=1 + filter=blob:none → 成功；体积与对象均下降
- fetch depth=1（已有仓库）→ 成功；增量正常
- filter 不支持的远端 → 回退 + `Protocol` 通知
- 非法参数（depth=0, filter=unknown）→ 立即 `Protocol` 错误

C. 互斥与护栏
- 代理开启（模拟，P5 占位）+ fakeSNI=true → 任务启动即发出互斥告警；实际使用 Real SNI
- Retry v1 与 shallow/partial 组合 → 重试次数与类别符合策略，不改变参数组合

产出：为每条用例保留任务事件快照，校验顺序、字段与分类。

## 19. 附录（术语与交叉引用）

- 术语：
  - 自适应 TLS 传输层（原方式A）：仅接管连接/TLS/SNI 的自定义 smart subtransport。
  - Fake SNI / Real SNI：握手时使用的 SNI 名称（伪装/真实）。
  - IP 来源（用于 IP 池/优选）：builtin（内置）、history（历史）、user_static（用户静态）、dns（DNS 解析）、fallback（兜底）。
- 参考文档：
  - MP0 实现交接稿：`doc/MP0_IMPLEMENTATION_HANDOFF.md`
  - MP1 实现交接稿：`doc/MP1_IMPLEMENTATION_HANDOFF.md`

### 19.1 关键文件速览（后端/前端）
- 后端（Rust/Tauri）
  - `src-tauri/src/core/tasks/registry.rs`：任务注册、生命周期、事件、Retry
  - `src-tauri/src/core/tasks/model.rs`：任务/事件模型
  - `src-tauri/src/core/git/default_impl/*`：git2-rs 实现（clone/fetch/push）、错误分类、进度桥接
  - `src-tauri/src/core/git/transport/*`：自适应 TLS 传输层（注册/改写/授权注入/流）
  - `src-tauri/src/app.rs`：Tauri 命令出口
- 前端（Vite/Vue + Pinia）
  - `src/api/tasks.ts`：事件订阅与归一化
  - `src/stores/tasks.ts`：任务进度与错误聚合
  - `src/views/GitPanel.vue`：Push 表单、TLS/SNI 策略、最近错误列

### 19.2 API 速览（事件与命令）
- 事件：`task://state | task://progress | task://error`
- 命令：`git_clone`、`git_fetch`、`git_push`、`task_cancel`、`task_list`
