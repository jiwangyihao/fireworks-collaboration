# 统一 Git 加速与传输控制综合技术方案

> 路线更新提示：本仓库的 Git 实现路线已切换为 git2-rs（libgit2 绑定）。
> 具体迁移与后续阶段细则请参考：`new-doc/TECH_DESIGN_git2rs.md`（新 P0=从 gitoxide 迁移 → P1 Push 与自定义 subtransport → P2+ 深化）。

> 本文将“原始统一 Git 加速与传输控制技术方案（含伪 SNI、SAN 白名单、IP 优选、代理、任务事件、通用伪 SNI HTTP 请求 API 等）”与“当前仓库（Tauri + Vue + TypeScript 模板）落地新版技术方案”合并，形成一份既包含完整长期蓝图，又具有在现有仓库逐步实施路径的综合方案。  
> 面向：架构 / 实施开发 / 测试 / 安全 / 后续演进。

---

## 目录（整合版）
1. 背景与目标（宏观 + 仓库现状）  
2. 范围与不做的内容  
3. 整体差距分析（原始蓝图 vs 当前仓库）  
4. 总体架构（分层 + 演进）  
5. 功能需求（统一最终版 + 仓库阶段映射）  
6. 非功能需求（性能 / 安全 / 可维护 / 可观测）  
7. 技术选型与理由（聚合）  
8. 模块划分与职责（蓝图 + 现状落地）  
9. Git Smart HTTP 协议策略（阶段引入）  
10. 伪 SNI + SAN 白名单 + 可选 SPKI Pin 验证体系  
11. 代理（HTTP / SOCKS5）与回退策略  
12. 精简 IP 优选体系（5 来源）  
13. 通用伪 SNI HTTP 请求开放 API（设计与使用）  
14. 任务模型、事件与取消机制  
15. 错误分类与回退链  
16. 安全设计与风险控制（整合 + 放宽后）  
17. 可观测性体系（日志 / 指标 / 事件）  
18. 配置体系与文件落地  
19. 分阶段 Roadmap（整合 & 仓库执行计划）  
20. 前端 API & 事件（命令统一清单）  
21. 关键时序流程（HTTP / Git / 回退）  
22. 测试策略与用例分层（阶段化）  
23. 性能与调优路线  
24. 核心数据结构（结构体汇总 + 分阶段）  
25. 关键伪代码（验证器 / IP 评分 / HTTP / 任务）  
26. 仓库目录结构建议与文件骨架  
27. 风险矩阵（整合更新）  
28. TL;DR 摘要（高层速览）  
29. 原方案与落地方案需求映射表  
30. 后续扩展议题与增强方向  

---

## 1. 背景与目标（宏观 + 仓库现状）

### 原始背景
在部分复杂网络环境中：  
- GitHub 访问受 SNI 检测 / DNS 污染影响  
- 多路径（直连 / 代理 / 系统 git）调试繁琐  
- 请求中 TLS 验证仍需安全完整性（CA + SAN + 可选 Pin）  
目标：统一一个 Smart HTTP Git 传输栈，提升可达性、性能与观测性，提供调试能力（通用伪 SNI HTTP 请求 API）。

### 仓库现状（fireworks-collaboration）
- 当前仅为 Tauri + Vue + TS 模板，无 Git / 网络增强逻辑  
- 没有任务调度 / 事件总线 / TLS 自定义 / IP 优选  
- 需要自零构建基础设施（TaskRegistry、Config、Transport）

### 综合目标
| 维度 | 目标 |
|------|------|
| 功能统一 | Smart HTTP Git Clone/Fetch/Push + Shallow/Partial + LFS（后期） |
| 连通增强 | 伪 SNI / 多 IP 优选 / 代理回退 |
| 调试能力 | 通用伪 SNI HTTP 请求 API（首阶段即可） |
| 安全基线 | CA 链验证 + SAN 白名单 + 可选 SPKI Pin |
| 观测 | 任务事件、进度、网络时序、证书指纹变更 |
| 扩展性 | 后续 HTTP/2、SSH fallback、流式响应、指标面板 |
| 落地路径 | 按阶段迭代，不阻塞前端 UI 使用 |

---

## 2. 范围与不做内容

| 范围内 | 说明 |
|--------|------|
| Git Smart HTTP | Clone / Fetch / Push / Shallow / Partial |
| 伪 SNI | 先用于通用 HTTP，再整合 Git |
| 证书验证 | SAN 白名单 + 可选 SPKI Pin |
| 代理支持 | HTTP / SOCKS5 + 自动回退 |
| IP 优选 | builtin / history / user_static / dns / fallback |
| 通用伪 SNI HTTP API | 不限方法 / 取消速率大小限制（仅警告） |
| 任务模型 | 事件化 + 可取消 |
| 指纹与日志 | 证书记录与变化事件 |
| 配置 | JSON 持久化 + 未来热更新 |
| 安全建议 | 授权头脱敏可选开关 |

| 不在范围 | 理由 |
|----------|------|
| Git 服务端实现 | 客户端专注 |
| 系统全局代理安装 | 降低侵入 |
| ECH / HTTP/3 初期 | 增量演进 |
| 海量 IP 扫描 / 外部 Feed | 复杂度、合规风险 |
| HSM / SGX 等强安全 | 当前需求外 |
| 完整 LFS 上传 | 后期再定 |
| 深度行为风控 | 早期不加重负担 |

---

## 3. 整体差距分析（蓝图 vs 现状）

| 能力 | 蓝图目标 | 仓库现状 | 差距类型 | 解决优先级 |
|------|----------|----------|----------|------------|
| 通用 HTTP 调试 | 完整伪 SNI + 白名单 | 无 | 功能 | P0 |
| Git Clone | Smart HTTP + 进度 | 无 | 功能 | P0 |
| Fetch / Push | 完整 | 无 | 功能 | P1 |
| Shallow / Partial | 过滤优化 | 无 | 性能 | P2 |
| 伪 SNI 应用于 Git | Fallback 真实 SNI | 无 | 连通性 | P3 |
| 代理 / 回退 | 双协议 | 无 | 网络 | P4 |
| IP 优选 | 评分模型 | 无 | 可达性 | P5 |
| LFS 下载 | 指针替换 | 无 | 大文件 | P6 |
| SPKI Pin / 指纹事件 | 可选防劫持 | 无 | 安全 | P7 |
| SSH fallback | 兜底 | 无 | 可用性 | P8 |
| 指标面板 | 可视化 | 无 | 观测 | P9 |
| HTTP/2 / Streaming | 优化 | 无 | 性能 | P10 |

---

## 4. 总体架构（分层 + 演进）

```
Frontend (Vue + TS + Pinia)
  ├─ API SDK (invoke wrapper)
  ├─ Panels: Git / HTTP Tester / Network Insights
  └─ Task & Log Stores

Tauri Backend (Rust)
  ├─ api:: (tauri commands / outward facade)
  ├─ core::
  │   ├─ git:: (gitoxide integration / progress)
  │   ├─ http:: (unified client + hyper + rustls)
  │   ├─ tls:: (SAN verifier / SPKI pin)
  │   ├─ ip_pool:: (sources + probe + score)
  │   ├─ proxy:: (HTTP CONNECT / SOCKS5)
  │   ├─ tasks:: (registry / cancellation / dispatch)
  │   ├─ config:: (load / watch / override)
  │   ├─ security:: (fingerprint log & change detect)
  │   ├─ metrics:: (later counters/histograms)
  │   └─ util:: (retry / base64 / timing)
  ├─ events::emitter (central channel -> tauri emit)
  └─ storage (json & log files)
```

可选未来扩展：`core::ssh`, `core::http2`, `core::streaming`.

---

## 5. 功能需求（统一最终清单 + 阶段映射）

| ID | 描述 | 优先级 | 交付阶段 |
|----|------|--------|----------|
| F1 | Clone 基础 | 高 | P0 |
| F2 | Fetch | 高 | P1 |
| F3 | Push 基础 | 高 | P1 |
| F4 | Shallow | 中 | P2 |
| F5 | Partial (blob:none) | 中 | P2 |
| F6 | LFS 下载 | 中 | P6 |
| F7 | 伪 SNI（HTTP 调试 + Git） | 高 | P0 (HTTP) / P3 (Git) |
| F8 | SAN 白名单验证 | 高 | P0 |
| F9 | SPKI Pin（可选） | 中 | P7 |
| F10 | HTTP / SOCKS5 代理 | 高 | P4 |
| F11 | 代理失败回退 | 中 | P4 |
| F12 | IP 优选（5 来源） | 高 | P5 |
| F13 | 任务事件 (state/progress/error) | 高 | P0 |
| F14 | 任务取消 | 高 | P0 |
| F15 | HTTP 策略动态配置 | 中 | P1+ |
| F16 | 通用伪 SNI HTTP 请求 API | 高 | P0 |
| F17 | 证书指纹记录与事件 | 中 | P7 |
| F18 | 分类错误 / 回退链 | 高 | 渐进完善 |
| F19 | 日志脱敏（可选） | 中 | P0 |
| F20 | 任务级策略覆盖 | 中 | P2+ |

---

## 6. 非功能需求

| 项 | 指标 / 要求 |
|----|-------------|
| 性能 | 中型仓库 clone 耗时 ≤ 系统 git * 1.25 初期，后期优化至 1.05 |
| 稳定性 | 网络抖动自动切换 IP / fallback 代理 |
| 并发 | ≥4 并行 Git 任务不崩溃 |
| 内存 | 单 Git 任务 <300MB；HTTP 大响应 WARN |
| 安全 | 不关闭链验证；SAN 强制；伪 SNI 不削弱 CA 验证 |
| 可观测 | 全阶段事件 + 可选 metrics |
| 可维护 | 模块化、单元测试覆盖核心模块 |
| 可扩展 | HTTP/2、Streaming、SSH fallback 易插入 |

---

## 7. 技术选型（整合）

| 组件 | 选型 | 理由 |
|------|------|------|
| Git | gitoxide | 纯 Rust 可插拔 |
| HTTP | hyper | 异步连接复用、便于后期 HTTP/2 |
| TLS | rustls + 自定义 verifier | 安全 & 可扩展 Pin |
| 异步 | tokio | 生态成熟 |
| 代理 | tokio-socks + 手写 CONNECT | 精细控制 |
| 取消 | tokio-util CancellationToken | 简洁协作式 |
| 日志 | tracing | 结构化 + 层级过滤 |
| 序列化 | serde_json / toml | 标准 |
| 指纹哈希 | ring / sha2 | 安全实现 |
| 配置热更新 | notify (后期) | 动态策略 |
| 构建 | pnpm + vite + tauri | 桌面体验 |

---

## 8. 模块职责（扩展说明）

| 模块 | 主要内部子组件 | 输出 |
|------|----------------|------|
| git::service | handshake / negotiation / pack streaming | 进度事件 |
| git::progress | side-band 解码 | git://progress |
| http::client | 连接 + TLS + 重定向 + 伪 SNI | 通用响应 |
| tls::verifier | SAN 白名单 + Pin | 安全失败分类 |
| ip_pool::probe | RTT 探测 / score | ip://updated |
| proxy::manager | 失败计数 -> fallback | proxy://fallback |
| tasks::registry | id / state / cancel | 任务快照 |
| tasks::dispatcher | 事件分发 | Tauri emit |
| config::loader | 读写 / 覆盖 | 全局策略 |
| security::fingerprint | 证书哈希记录 | tls://fingerprintChange |
| metrics | 计数器 / 时序 | 可视化 |
| util | base64 / retry / timing | 内部复用 |

---

## 9. Git Smart HTTP 协议策略（阶段）

| 阶段 | 聚焦 | 细节 |
|------|------|------|
| P0 | Clone 基础 | 使用 gitoxide 默认 http，先保守 |
| P1 | Fetch / Push | 增加 remote negotiation |
| P2 | Shallow / Partial | 添加 depth, filter=blob:none |
| P3 | 替换 transport | 插入统一 HTTP（伪 SNI/Fallback） |
| P4+ | 代理/回退 | network -> proxy -> fallback direct |
| P6 | LFS 下载 | pointer -> batch -> object GET |
| P8 | SSH fallback | 调整回退链 |
| Future | Pack resume | 减少中断损耗 |

---

## 10. 伪 SNI + 验证体系

| 条目 | 内容 |
|------|------|
| 触发策略 | fakeSniEnabled && direct && !forceRealSni |
| 伪域默认 | baidu.com（可配置） |
| 验证 | 链验证（CA）+ SAN 白名单 + 可选 SPKI Pin |
| 白名单域 | github.com / *.github.com / *.githubusercontent.com / *.githubassets.com / codeload.github.com |
| Fallback 顺序 | Fake SNI → Real SNI → 换 IP → 代理/直连 |
| 指纹 | 提取 leaf SPKI SHA256 + 证书整体哈希 |
| Pin 机制 | 配置 spkiPins[]，不匹配即失败 |
| 与代理 | 代理模式禁用伪 SNI（规避可检测特征） |

---

## 11. 代理策略

| 项 | 说明 |
|----|------|
| 支持 | HTTP CONNECT / SOCKS5 |
| 设置 | set_proxy / disable_proxy |
| 失败判定 | 连接/握手/读写错误累积到阈值 |
| 回退事件 | proxy://fallback -> direct |
| 伪 SNI | 被禁用（以减少结合代理的异常特征） |
| 未来 | 自动恢复探测（心跳成功后可重新启用） |

---

## 12. 精简 IP 优选（5 来源）

| 来源 | 描述 |
|------|------|
| builtin | 内置可信 IP |
| history | 历史成功（附加权重 0） |
| user_static | 用户手动添加 |
| dns | 多 DoH 解析结果 |
| fallback | 最近失败 IP（惩罚高） |

流程：  
1. 聚合 -> 去重  
2. 探测（TCP->TLS->HEAD）收集 RTT/成功  
3. 更新统计 (rtt_avg, jitter, fail)  
4. Score = rtt_avg + jitter*w1 + fail*w2 + min(source_weight) + cooldown_penalty  
5. 选最优非冷却 IP  
6. 失败重试换下一个 → 达阈值加入 fallback/cooldown  

权重示例：history=0, user_static=0, builtin=5, dns=10, fallback=50。

---

## 13. 通用伪 SNI HTTP 请求 API

| 能力 | 描述 |
|------|------|
| 方法 | GET/HEAD/POST/PUT/PATCH/DELETE/OPTIONS |
| Body | Base64（当前整体加载；后续流式） |
| 伪 SNI | 可启用；可 forceRealSni 覆盖 |
| 白名单控制 | enforceDomainWhitelist=true 默认 |
| 授权头 | 透传；日志默认脱敏（debugAuthLogging=false） |
| 重定向 | followRedirects + maxRedirects |
| 响应 | status/headers/bodyBase64/timing/redirects/ip/usedFakeSni |
| 风险控制 | largeBodyWarnBytes 超出 WARN |
| 用例 | 调试 Git API、验证网络路径、证书观察 |

---

## 14. 任务模型 / 事件 / 取消

| 字段 | 含义 |
|------|------|
| taskId | UUID |
| kind | GitClone / GitFetch / HttpFake ... |
| state | pending/running/completed/failed/canceled |
| phase | Git 特定阶段（Negotiating / ReceivingPack 等） |
| progress | bytes / objects / totalHint / rate |
| cancellation | CancellationToken |

事件：
- git://state
- git://progress
- git://sideband
- git://error
- ip://updated
- proxy://fallback
- tls://fingerprintChange
- net://strategyChange
- http://fakeRequestStats（可选）

取消：轮询 token.is_cancelled() 中断读写。

---

## 15. 错误分类与回退链

| 分类 | 示例 | 回退链 |
|------|------|--------|
| network | connect timeout | 换 IP → 代理/直连 |
| tls | handshake fail | Fake → Real SNI → 换 IP |
| verify | SAN/SPKI mismatch | 直接失败 |
| protocol | pack decode | 失败 |
| proxy | CONNECT fail | fallback=direct |
| auth | push permission | 用户提示 |
| lfs | batch 403 | 重试一次 |
| cancel | 用户取消 | 结束 |
| internal | panic | fail + 日志 |

标准回退顺序优先级（可配置）：Fake SNI → Real SNI → IP 切换 → 代理/直连 → SSH (后期)。

---

## 16. 安全设计与风险控制

保留：
- 不关闭链验证
- 强制 SAN 白名单
- 伪 SNI 不移除 CA 验证，只改变 TLS SNI 与匹配策略
- 可选 SPKI Pin（缩小信任面）

放宽带来新风险：
| 风险 | 影响 | 缓解 |
|------|------|------|
| 任意域访问 | 滥用为代理 | 默认启用 whitelist；UI 警示关闭行为 |
| 大响应内存 | OOM | WARN 阈值 + 未来流式 |
| 凭证泄漏 | Token 外泄 | 日志脱敏默认 |
| 高频写请求 | 远端风控 | 后续指标报警 |
| 滥用调用 | 资源抢占 | 将来速率限制 / 配额 |

---

## 17. 可观测性体系

| 类别 | 指标例子 |
|------|----------|
| TLS | tls_handshake_ms |
| Git 阶段 | git_phase_duration_ms{phase} |
| IP 探测 | ip_probe_success_total{source} / ip_rtt_ms |
| 代理 | proxy_failover_total |
| 证书 | cert_fp_changes_total |
| Git 流量 | git_bytes_received_total |
| HTTP 调试 | http_fake_requests_total / http_fake_request_bytes_total |
| 大响应 | http_fake_large_response_total |

事件用于前端实时显示，指标用于后期统计/面板。

---

## 18. 配置体系与文件

| 文件 | 用途 |
|------|------|
| config.json | httpStrategy / httpFake / tls / proxy |
| ip-config.json | dns providers / scoring |
| ip-history.json | 历史成功失败与 RTT |
| cert-fp.log | 指纹追加日志（JSON line） |
| strategy.lock | 可选运行时快照 |
| lfs-cache/ | LFS 对象（后期） |

支持热更新（后期）：文件变更 -> reload -> emit net://strategyChange。

---

## 19. Roadmap（整合阶段 + 仓库任务）

| 阶段 | 核心交付 | 验收标准 |
|------|----------|----------|
| P0 | http_fake_request + 基础 clone + 任务/事件/取消 + SAN 验证 | UI 可发 HTTP & clone 并显示进度与 timing |
| P1 | Fetch/Push + 重试策略 + 状态丰富 | 推送公共测试仓库成功 |
| P2 | Shallow + Partial + 任务级策略覆盖 | depth=1 / blob:none 克隆显著减小数据 |
| P3 | Git 伪 SNI + fallback | Fake -> fail -> Real fallback |
| P4 | 代理接入 + 自动回退 | 代理故障触发 proxy://fallback |
| P5 | IP 优选全套 + UI 展示 | ip://updated 动态排序 |
| P6 | LFS 下载 | LFS 文件可获取 |
| P7 | SPKI Pin + 指纹事件 | Pin 不匹配失败；指纹变更事件触发 |
| P8 | SSH fallback | 偏极端网络成功 Clone |
| P9 | 指标 & 简易面板 | Metrics 图表 |
| P10 | HTTP/2 + Streaming + Pack resume（探索） | 大仓库速度与内存改善 |

## 19.1 进度更新（本仓库）

截至 2025-09-13：已完成 P0.7「前端面板与可视化」阶段的交付，包含：
- 任务事件接入：前端监听 `task://state` 与 `task://progress`，聚合到 Pinia store（`progressById`）。
- Git 面板：新增 `GitPanel.vue`，支持输入仓库与目标目录、启动克隆、实时进度条显示与取消任务。
- HTTP 面板：`HttpTester.vue` 增强了请求历史（点击可回填）与策略开关（Fake SNI / 跳过证书校验[原型]），保存至配置。
- 全局错误提示：新增日志 store 与全局吐司组件，统一展示错误信息并默认对敏感头进行脱敏日志。

开发者入口：
- 路由 `/git` 进入 Git 克隆面板；`/` 主页包含导航。
- 详见实现说明与文件清单：`doc/TECH_DESIGN_P0.md` 的「P0.7 实际实现说明 (已完成)」。

下一阶段（P1）规划与实施细化请见：`doc/TECH_DESIGN_P1.md`。

---

## 20. 前端 API & 事件（命令清单）

| 类别 | Commands |
|------|----------|
| Git | git_clone / git_fetch / git_push / git_cancel / git_task_status |
| HTTP | http_fake_request |
| 策略 | get_http_strategy / set_http_strategy |
| IP | ip_get_pool / ip_probe_all / ip_add_manual / ip_remove |
| 代理 | set_proxy / get_proxy / disable_proxy |
| TLS/诊断 | tls_cert_fingerprints / http_raw_head (可选) |

事件同第 14 节。

---

## 21. 关键时序流程（文本）

### 通用 HTTP (P0)
1. 前端发起 http_fake_request  
2. 解析 URL & 校验白名单  
3. 判定 useFakeSni  
4. 若启用 IP 池（后期）→ 选 IP  
5. TCP -> TLS (SNI=伪或真实)  
6. 发送请求 → 收集 timing（connect/tls/firstByte/total）  
7. 读取完整 body → Base64 → 返回  
8. 记录指标与可选事件  

### Git Clone (P0 基础)
1. 创建任务记录 → git://state(pending→running)  
2. 使用 gitoxide 默认 HTTP → GET info/refs  
3. Negotiation -> Pack streaming  
4. side-band -> progress 事件  
5. 完成对象落盘 → state=completed  

### Git 伪 SNI (P3)
在建立 HTTP 连接时调用统一 transport：Fake SNI 握手失败 → Real SNI → IP fallback → 代理 fallback。

#### P3 技术增强：按“真实域名”验证而非按 SNI 名称验证（Real-Host Verification）

目标：当握手使用“伪 SNI”以规避网络策略时，客户端的证书域名匹配不再使用握手传入的 SNI，而是使用“实际要访问的真实域名”（例如 github.com），以减少“仅因伪 SNI 名称不一致而导致的客户端侧校验失败”。

要点与约束：
- 该能力并不能强行让服务器返回包含真实域名的证书。服务器选择证书通常依据它收到的 SNI；若伪 SNI 与真实域不同，多数服务器会返回默认证书，可能不包含真实域名。
- 因此该能力常与“Fake→Real 回退”配套：若以真实域名验证失败（证书不含真实域），立即回退一次“使用真实 SNI 重新握手”。
- 安全基线仍包含：CA 链验证 + 白名单匹配（对比目标域）。

实现方案（Rust + rustls）：
1) 自定义证书验证器 RealNameCertVerifier
   - 字段：
     - inner: WebPkiVerifier（执行标准 CA 链与域名验证）
     - whitelist: Vec<String>（白名单）
     - real_host: String（真实域名）
   - 行为：在 verify_server_cert 中忽略 rustls 提供的 server_name，改用 real_host 构造 `ServerName::try_from(real_host)`，调用 inner.verify_server_cert(...) 完成链与域名匹配；随后以 real_host 进行白名单匹配（通配规则与 P0 一致）。

2) 每次请求按需构建 ClientConfig
   - 因 real_host 随请求而变，需在发起请求时创建带有 RealNameCertVerifier 的 ClientConfig；避免在共享 ClientConfig 中夹带错误的 real_host。
   - 对于高频请求，可以考虑配置缓存（key=real_host），权衡构造开销与内存使用。

3) 握手与验证流程（结合伪 SNI）
   - 握手：SNI=假域（如 baidu.com）；
   - 验证：RealNameCertVerifier 使用 real_host=github.com 调用 WebPKI 验证域名；
   - 白名单：基于 real_host 做通配匹配；
   - 失败路径：若返回证书不含 github.com → 域名匹配失败 → 触发回退链（用真实 SNI 重试一次）。

4) 回退与分类
   - 首次失败类别：Tls（握手早期失败）或 Verify（链通过但域名/白名单不符）。
   - 回退一次：SNI=真实域名，验证仍用真实域名（此时名称一致，预计成功）；若仍失败，再按网络/IP/代理策略继续。

5) 伪代码草案
```
struct RealNameCertVerifier { inner: WebPkiVerifier, whitelist: Vec<String>, real_host: String }
impl ServerCertVerifier for RealNameCertVerifier {
  fn verify_server_cert(&self, end: &Certificate, inter: &[Certificate], _name: &ServerName, scts: &mut dyn Iterator<Item=&[u8]>, ocsp: &[u8], now: SystemTime) -> Result<ServerCertVerified, rustls::Error> {
    let name = ServerName::try_from(self.real_host.as_str()).map_err(|_| rustls::Error::General("bad real_host".into()))?;
    self.inner.verify_server_cert(end, inter, &name, scts, ocsp, now)?;
    if !host_in_whitelist(&self.whitelist, &self.real_host) { return Err(rustls::Error::General("SAN whitelist mismatch".into())); }
    Ok(ServerCertVerified::assertion())
  }
}

fn create_client_config_with_real_name(tls: &TlsCfg, real_host: &str) -> ClientConfig { /* 构造 root store -> WebPkiVerifier -> RealNameCertVerifier(real_host) */ }

async fn connect_with_fake_sni_and_real_verify(real_host: &str, fake_host: &str, cfg: &AppConfig) -> Result<TlsStream> {
  let client_cfg = create_client_config_with_real_name(&cfg.tls, real_host);
  let server_name = ServerName::try_from(fake_host)?;
  tls.connect(server_name, tcp).await
}
```

6) 与 P0 的差异
- P0 白名单与验证是“按 SNI 名称”；P3 则转为“按真实域名”，并在失败时回退“真实 SNI 再试”。
- 仍保持 CA 链验证；不引入 Insecure 模式。Insecure 模式仅用于原型联调（见 P0 文件）。

7) 风险与测试
- 风险：服务端因假 SNI 返回默认证书，真实域名验证大概率失败；但回退链会立刻以真实 SNI 重试，确保用户体验。
- 用例：
  - 假 SNI + RealName 验证失败 → 回退 Real SNI 成功；
  - 假 SNI + RealName 验证成功（少数情况，例如默认证书恰含真实域）→ 不回退；
  - 白名单外域 → 直接 Verify 错误；
  - 证书链不可信 → Tls 错误。


---

## 22. 测试策略与用例分类（阶段化）

| 类别 | P0 | P1-P3 | P4-P7 | P8+ |
|------|----|-------|-------|-----|
| HTTP 方法 | 各方法状态 | 重定向链 | 大体积响应流控 | Streaming |
| 伪 SNI | 成功/失败回退 | Git 伪 SNI | 指纹记录 | ECH/Evolution |
| 白名单 | 拒绝非法域 | 动态变更生效 | 关闭模式风险提示 | 审计 |
| Git Clone | 基础成功 | Shallow/Partial | LFS 下载 | Pack Resume |
| 取消 | Clone 中途 | Push 中途 | LFS 中断 | SSH fallback |
| 错误分类 | 网络/TLS/验证 | 代理错误分类 | Pin mismatch | 综合链路 |
| IP 优选 | - | - | 探测排序 | 历史持久化 |
| 代理 | - | - | 回退触发 | 恢复探测 |
| 性能 | 基线 | pack 优化 | 并发 stress | HTTP/2 对比 |
| 安全 | 授权脱敏 | SAN 变更失败 | Pin 生效 | 多 Pin 轮换 |

---

## 23. 性能与调优路线

| 层级 | 措施 |
|------|------|
| 网络 | 连接重用 + Session Resumption |
| Pack | 并行 delta 应用（受限） |
| 重试 | 区分类别（网络 vs 协议）渐进 backoff |
| IP 选择 | 缓存 top-K 减少全排序 |
| LFS | 并行 + 限流 |
| HTTP | Streaming body / HTTP/2 减少握手开销 |
| 内存 | 大响应分块 / pack pipeline 优化 |
| 指标 | 收集延迟分布指导优化 |

---

## 24. 核心数据结构（阶段化）

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

## 25. 关键伪代码

### TLS 验证器（简化）
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
    // 链验证
    self.default.verify_server_cert(end_entity, intermediates, server_name, &[], now)?;
    // SAN 白名单
    if !self.san_allowed(end_entity)? { return Err(rustls_err("SAN mismatch")); }
    // SPKI Pin（可选）
    if self.pin_enabled && !self.spki_pinned(end_entity)? { return Err(rustls_err("SPKI pin mismatch")); }
    Ok(ServerCertVerified::assertion())
  }
}
```

### IP 评分
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

### 通用 HTTP 请求
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

### 任务注册
```rust
pub fn start_clone(repo: String, dest: String, registry: Arc<TaskRegistry>) -> Uuid {
  let (id, token) = registry.create(TaskKind::GitClone { repo: repo.clone(), dest: dest.clone() });
  spawn(async move {
    registry.update_state(id, TaskState::Running);
    // 调用 gitoxide clone 过程（带取消检查）
    match do_clone(&repo, &dest, token).await {
      Ok(_) => registry.update_state(id, TaskState::Completed),
      Err(e) if token.is_cancelled() => registry.update_state(id, TaskState::Canceled),
      Err(e) => {
        registry.update_state(id, TaskState::Failed);
        emit_error(id, ErrorCategory::Network, e.to_string());
      }
    }
  });
  id
}
```

---

## 26. 仓库目录结构与文件骨架

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

## 27. 风险矩阵（更新整合）

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

## 28. TL;DR 摘要

- 方案整合：原始 Git 加速与传输控制蓝图 + 当前仓库从零落地路径。  
- P0 即可获得：伪 SNI HTTP 调试 API + SAN 验证 + 基础 Clone + 任务事件与取消。  
- 后续逐步引入：Fetch/Push → Shallow/Partial → Git 伪 SNI → 代理 → IP 优选 → LFS → SPKI Pin → SSH fallback → 指标面板 → HTTP/2 / Streaming / Pack resume。  
- 安全基线：TLS 链验证不关闭，SAN 白名单强制，可选 Pin；伪 SNI 不削弱信任。  
- IP 优选仅 5 来源，控制复杂度和实现成本。  
- 通用 HTTP API 放宽限制 → 通过 whitelist / 日志脱敏 / 阈值告警降低风险。  
- 架构模块化，支撑后续扩展（HTTP2、缓存、分块、可视化面板）。  

---

## 29. 原方案与落地方案需求映射表

| 原方案功能点 | 状态 | 落地阶段 | 仓库实现方式 |
|--------------|------|----------|--------------|
| 通用伪 SNI HTTP API | 调整为 P0 | P0 | http_fake_request |
| Clone 基础 | 保持 | P0 | gitoxide + 任务管理 |
| Fetch / Push | 保持 | P1 | git service 扩展 |
| Shallow / Partial | 保持 | P2 | depth/filter 参数 |
| 伪 SNI（Git） | 后移到 P3 | P3 | 统一 transport 替换 |
| SAN 白名单 | 强制 | P0 | 自定义 verifier |
| SPKI Pin | 可选 | P7 | tls::verifier + config |
| 代理与回退 | 保留 | P4 | proxy::manager |
| IP 优选 5 来源 | 精简 | P5 | ip_pool with scoring |
| 任务事件/取消 | 保留 | P0 | TaskRegistry + emit |
| 错误分类 | 保留 | 渐进 | ErrorCategory 枚举 |
| 指纹事件 | 保留 | P7 | security::fingerprint |
| 日志脱敏 | 建议 | P0 | debugAuthLogging 开关 |
| 任务策略覆盖 | 保留 | P2+ | 覆盖 httpStrategy |
| LFS 下载 | 保留 | P6 | lfs::module |
| SSH fallback | 新蓝图后期 | P8 | 额外协议模块 |
| 性能指标面板 | 后期 | P9 | metrics + UI |
| HTTP 流式 | 展望 | P10 | streaming body |
| Pack resume | 展望 | P10 | pack 分段校验 |

---

## 30. 后续扩展议题

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
