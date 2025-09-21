# P3 阶段技术设计文档 —— 自适应 TLS 传输层全量推广与可观测性强化

## 1. 概述

本文基于 MP0/MP1 已完成（git2-rs 基线、Push、自定义 smart subtransport(A) 灰度、Retry v1、标准错误分类）与 P2 已交付（本地 Git 操作集、Shallow/Partial 决策与 fallback、任务级策略覆盖 + 护栏 + summary）的能力，进入 P3：将已“灰度存在”的自适应 TLS 传输层（方式A：仅接管连接/TLS/SNI）升级为对白名单域默认启用，并在“保持现有回退链 Fake→Real→Default 不变”的前提下，增加结构化可观测（timing / fallback / cert 指纹变更）与运行期自适应（自动降噪、临时禁用 Fake）能力；本阶段不重新实现或破坏 MP1/P2 的子传输与错误分类，而是在其上增量固化与强化。目标为 P4（IP 优选）提供指标与结构前置。 

### 1.1 背景
- 当前自适应 TLS 在 MP1 处于“灰度 + 失败自动回退”阶段：需显式配置开启；已具备 Fake→Real→libgit2 默认的最小回退链；日志与调试字段分散，缺乏统一指标。
- 推广前需解决：
  1) 观测盲区：握手/连接/首字节耗时、失败类别占比、证书指纹变更频率未结构化；
  2) 回退策略一致性：不同任务（clone/fetch/push）错误分类落点与回退触发条件需对齐；
  3) 安全可审计：证书指纹采集需防篡改（追加写 + 基本格式校验），并避免敏感字段外泄；
  4) 渐进放量：需要可配置的分阶段 rollout（% / 主机子集 / 任务类型范围）以降低风险。

### 1.2 目标（高层）
1. 默认启用：将原 `http.fakeSniEnabled`（MP1 灰度使用）默认值调整为 true，并引入可选百分比采样（rollout）控制；旧配置缺省时视为全量启用。
2. 回退链增强：维持既有 Fake→Real→Default 链条，不新增阶段；对回退触发原因、阶段与最终结果进行结构化指标与事件补充（不改已有分类规则）。
3. 可观测性：采集并结构化输出连接、TLS、首字节、总耗时、阶段枚举、证书指纹（SPKI SHA256 + leaf 哈希）、握手失败分类统计；对突增/异常提供基础告警阈值（日志级别提升）。
4. 安全与合规：证书指纹日志采用 JSON Lines 追加模式 + 行级校验；默认脱敏；无授权信息泄漏；Fake SNI 不降低链验证安全性。
5. 向后兼容：前端命令/事件签名不破坏，仅新增可选字段（如 usedFakeSni, timing, fallbackStage, certFpChanged?）；旧前端若忽略这些字段仍正常。无已存在字段语义变更。
6. 可回退：单一配置布尔或环境变量即可即时关闭（不需重启）并回到 libgit2 默认传输；指纹与指标逻辑在关闭时自动暂停采集。
7. 为 P4 准备：在 timing/回退事件中预留可选 `ip`、`ipSource?` 字段（当前恒为空），P4 注入时不需要新增事件代码。 
8. 引入“真实域名验证”（Real-Host Verification）机制：握手可用 Fake SNI 但证书域名匹配仍针对真实目标域，失败一次即回退 Real SNI；本阶段实现并默认开启（可通过调试开关关闭），为后续 Pin 细化奠定基础。
9. 预埋 SPKI Pin 规划（Spec Only，不启用）：定义 `tls.spkiPins?: string[]`（Base64URL SPKI SHA256）与匹配策略、失败分类（Verify），P3 仅输出规划文档与解析预留（不触发校验），P7 再正式启用。

### 1.3 范围
- 后端（Rust）：传输层注册与默认启用逻辑、回退决策统一、指标/指纹采集、事件字段扩展、配置与 gating、长时运行稳定性验证脚手架。
- 前端（Vue/Pinia）：可选显示 timing 与 usedFakeSni；错误/回退信息友好化；不强制新增 UI 结构（渐进增强）。
- 文档：更新配置模型、事件与指标说明、回退链描述与安全策略说明。

### 1.4 不在本阶段
- IP 池与 IP 优选（P4）。
- 代理与自动降级（P5）。
- SPKI Pin 强校验（计划 P7）。
- LFS 支持与指标面板 UI（P7/P8）。
- 真实 HTTP/2、多路复用、ECH（远期议题）。

### 1.5 成功标准
| 指标 | 目标 | 说明 |
|------|------|------|
| 稳定成功率 | ≥99% | 白名单域在默认启用下任务成功率不低于灰度基线 |
| 额外失败率增量 | <0.5% | 启用后新增 TLS/网络失败占比增量受控 |
| 回退命中 Fake→Real 占比 | <5% | Fake SNI 握手失败低频（异常升高需调查） |
| 指纹变更事件误报率 | 0 | 同一证书周期内正常不重复记录异常变更 |
| 事件兼容性 | 100% | 旧前端不因新增字段报错或渲染异常 |
| 回退开关响应时间 | <5s | 关闭配置后新任务全部走默认传输 |

### 1.6 验收条件
1. 功能：在测试/预生产环境连续运行 ≥72 小时无资源泄漏、无异常崩溃；
2. 回退：人为注入 TLS 握手错误/网络中断，任务正确进入 Real / Default 分支并成功或明确失败分类；
3. 指标：采集文件或内存聚合结构可导出核心耗时分位数（P50/P95）与指纹变更次数；
4. 安全：指纹日志不含私钥或密钥材料；无环境变量/凭证泄漏；
5. 文档：配置、事件字段、回退顺序、风险与回退策略章节齐备；
6. 测试：新增/更新测试矩阵（含故障注入），全部通过；
7. 可回退：关闭开关后重新执行冒烟任务全部不再触发 usedFakeSni=true。

### 1.7 交付物
- 代码：在现有 subtransport(A) 基础上抽象决策与指标采集层（非破坏性重写）；新增指纹采集与缓存模块；自动禁用（runtime flag）机制。
- 配置：保持 `http.fakeSniEnabled` 键；新增 `http.fakeSniRolloutPercent?`（0..100，可缺省）；在 `tls` 命名空间增量添加 `metricsEnabled`、`certFpLogEnabled`、`certFpMaxBytes`、`realHostVerifyEnabled`（默认 true）；预埋 `spkiPins?`（解析占位不生效）。
- 事件：在既有 `task://progress|error` 通道新增可选结构 `{ timing?, usedFakeSni?, fallbackStage?, certFpChanged? }` 与信息型代码 `adaptive_tls_rollout`、`adaptive_tls_fallback`、`cert_fingerprint_changed`。
- 文档：更新 `TECH_DESIGN_git2rs.md` P3 段落、Changelog 条目、配置示例；保留与 P2 的差异对照表。
- 测试：故障注入（连接/TLS/读写）、回退路径覆盖、rollout 采样偏差、指纹滚动与重复抑制、性能基线（微基准或统计）。

### 1.8 回退策略
| 场景 | 回退操作 | 影响 |
|------|----------|------|
| Fake 握手失败率突增 | 运行期置 runtime flag 禁用 Fake（保留 Real） | 维持自定义 TLS，不再尝试 Fake，事件继续输出 |
| 多类别失败（Fake+Real 均高） | 将 `http.fakeSniEnabled=false` | 回到 libgit2 默认传输，失去新增 timing/指纹事件 |
| 指纹日志异常（IO/过大） | `tls.certFpLogEnabled=false` | 停止写文件，内存指标保留 |
| 指标开销超预算 | `tls.metricsEnabled=false` | 停止 timing 采集，功能保持 |
| 旧前端兼容问题 | 关闭新增字段输出（构建或配置开关） | 仅内部日志查看，减少字段扩散 |

### 1.9 关键依赖与假设
- 继续复用 MP1 自定义 subtransport(A) 代码路径（无破坏性重构）。
- libgit2 / git2 crate 当前版本足够；仅在出现握手兼容问题时评估小版本升级。
- rustls 满足性能需求；不计划引入 OpenSSL 变体。
- P2 shallow/partial 不改变传输接口，对本阶段透明。
- 事件派发层性能充足；新增信息事件总量可控（单任务 ≤3 条新增）。

### 1.10 风险概览
| 风险 | 等级 | 描述 | 缓解 |
|------|------|------|------|
| Fake 握手失败率剧增 | 中 | 网络策略变化 / 局部封锁 | 自动统计 + runtime 禁用 Fake + 报警日志 |
| 指纹日志膨胀 | 中 | 高频任务写入导致文件增长 | 文件大小上限 + 滚动 + 采样写策略 |
| 信息事件噪声 | 低 | 多任务高频 output | 合并 timing 为单对象 + 仅回退/变更时发 fallback 事件 |
| 分类漂移 | 中 | 规则更新致误分类 | 表驱动映射快照 + 单元测试基线 |
| 性能退化 | 中 | 采集/哈希开销 | 指标开关 + 缓存命中统计 + 基准对比 |
| 回退开关不一致 | 低 | 部分线程未见新 flag | 原子共享 + 注册时读取 + 任务起始检查 |
| Pin 规划漂移 | 低 | P3 规划与 P7 实施字段语义偏差 | 规格锁定 + 解析快照测试 + Changelog 高亮 |

### 1.11 兼容与迁移
| 旧配置场景 | 行为（P2/MP1） | P3 迁移策略 | 兼容保障 |
|-------------|----------------|-------------|------------|
| 缺失 fakeSniEnabled | 默认灰度关闭或手动开启 | 新版本默认视为 true | 可通过显式 false 回退 |
| 缺失 fakeSniRolloutPercent | 无该字段 | 视为 100% | 加字段<100 时进入采样 |
| 设置 fakeSniEnabled=false | 完全关闭 | 语义不变 | 不写 rollout 仍保持关闭 |
| 旧前端不识别 timing 字段 | 忽略 | 字段可选 | 不破坏 JSON 解析 |
| 未配置 metricsEnabled/certFpLogEnabled | 不采集 | 默认 true（可覆盖） | 设置 false 即停 |
| 事件无 fallbackStage | 未输出 | 新增字段可选 | 旧端安全降级 |

迁移执行：发布说明列出默认值变更（fakeSniEnabled、metricsEnabled、certFpLogEnabled），建议运维在首轮观察指标后决定是否调低 rollout。 

## 2. 详细路线图

### 子阶段划分
| 阶段 | 主题 | 核心关键词 |
|------|------|------------|
| P3.0 | 基线巩固与观测脚手架 | Flag 迁移 / 指标接口 / 回退决策抽象 |
| P3.1 | 默认启用与渐进放量 | Gating / %Rollout / 白名单策略 |
| P3.2 | 可观测性强化（基础） | Timing / Fingerprint / 日志滚动 |
| P3.3 | Real-Host 验证 | Fake SNI 握手 / 真实域匹配 / 单次回退 |
| P3.4 | SPKI Pin 规划（Spec） | Pin 字段解析 / 日志 PinInactive / 未来启用路径 |
| P3.5 | 异常与回退稳健性 | 故障注入 / 分类一致性 / 自动禁用 Fake |
| P3.6 | 稳定性 Soak & 退出准入 | 长时运行 / 指标阈值 / 报告 |

### P3.0 基线巩固与观测脚手架
目标：在不重写 MP1 现有 subtransport 的前提下，抽象出独立的“回退决策 + 指标采集”层，为 rollout / 指纹 / 自动禁用提供挂点；保持未开启时行为与 P2 完全一致。
范围：
- 提炼决策层：抽出 `FallbackDecision`（原有 Fake→Real→Default 分支逻辑迁移，不改语义）；
- 新增指标接口（trait）与内存聚合结构：记录 connect/tls/firstByte/total；
- 统一错误分类映射（表驱动），引入测试快照；
- 配置兼容：继续使用 `http.fakeSniEnabled`；新增可选 `http.fakeSniRolloutPercent`（暂未启用，下一阶段生效）；timing 字段本阶段默认不输出（metricsEnabled 未开启）。
交付物：
- 模块：`core/git/transport/metrics.rs`、`core/git/transport/fallback.rs`（新增，不破坏现有文件）；
- 枚举：`FallbackStage { None, Fake, Real, Default }`；
- 单元测试：分类映射、决策路径、无启用时透明行为；
- 文档：更新配置键与结构草案。
接口/配置变化：无新根节点；仅文档化未来将引入的 `http.fakeSniRolloutPercent`（尚不生效）。
指标：内部计数器（未对外事件）验证；
验收：未开启时所有任务行为与 P2 相同（哈希快照）；
回退：移除新模块或将 enabled 强制为 false；
风险&缓解：结构侵入——保持 API 不变；分类漂移——建立 baseline 快照测试。

### P3.1 默认启用与渐进放量
目标：对白名单域分阶段启用 Fake SNI 路径（含 Real 回退），从 0%→10%→50%→100%（可配置）。
范围：
- Rollout 策略：按哈希(taskId 或 repo host) 一致性取样（确保同仓库稳定体验）；
- 白名单：Github 域族（与 P2 相同），新增可选 `hostAllowListExtra`；
- 事件：首次命中 rollout 的任务输出信息事件 `{ code: "adaptive_tls_rollout", percentApplied, sampled }`（信息型，不影响结果）。
- 决策：采样未命中 → 直接 Default；命中 → Fake 尝试，失败按链回退；
- 记录：每种 FallbackStage 命中计数（内存/日志）。
交付物：Rollout 采样函数、配置解析、事件单测、并发一致性测试。
接口/配置：启用 `http.fakeSniEnabled=true` 与 `http.fakeSniRolloutPercent`（若缺省则 100）；不新增根键。 
指标：`rollout_applied_total{stage}`；
验收：不同 rollout 值下采样比例误差 <2%；回退计数出现符合注入失败；
回退：将 rollout=0 或 enabled=false；
风险&缓解：采样倾斜——使用一致性哈希 + 偏差测试；误操作全量——CI 校验默认值变更需审阅。

### P3.2 可观测性强化（基础）
目标：输出基础 timing 与指纹变更事件/日志（不含 Real-Host 验证与 Pin 确认逻辑），支撑性能与安全分析，为后续安全增强铺垫。
范围：
- Timing 收集：连接、TLS、首字节、总耗时（ms，u32 范围）；
- 事件扩展：完成后发送信息事件或增强 progress 附加 `timing` 与 `usedFakeSni`；
- 指纹：提取 leaf 证书 SPKI SHA256（Base64url）与证书整体 SHA256；
- 指纹变更判定：同 host + 24h 内发生变化记录变更事件 `{ code: "cert_fingerprint_changed" }`；
- 日志落地：`cert-fp.log` JSON Lines（字段：ts, host, spkiSha256, certSha256, changed?）——该文件在 earlier 设计中已规划，此阶段正式启用。
- 安全：文件大小上限（例如 5MB）滚动策略；
- 性能：fingerprint 计算缓存（LRU host→(spki,hash)）。
（本阶段暂不引入 Real-Host 验证与 SPKI Pin 校验，二者分别在 P3.3 / P3.4 实施与规划。）
交付物：timing 注入、中间层 wrapper、指纹记录器、LRU 缓存、事件测试、滚动策略。
接口/配置：`tls.metricsEnabled`、`tls.certFpLogEnabled`、`tls.certFpMaxBytes`；默认 metrics / certFpLog 启用，兼容旧配置无字段视为启用。
指标：`tls_handshake_ms_bucket`（可后期直方或延迟，仅内部）；`cert_fp_changes_total`；
验收：
 - 启用 metricsEnabled 时事件含 timing；关闭后不含；
 - 故意注入慢 TLS（延迟模拟）能在 timing 中反映；
 - 同一 host 第二次握手不重复产生 changed 事件（若未变化）；
 - 文件滚动生效，超限截断或轮换；
 -（Real-Host / Pin 相关验收延后至 P3.3 / P3.4）
回退：关闭 metricsEnabled / certFpLogEnabled；
风险&缓解：性能开销——缓存 + 可关闭；隐私——不写 SAN 列表 / 不含客户端信息。

### P3.3 Real-Host 验证
目标：在保持 Fake→Real→Default 既有回退链前提下，引入真实域名匹配机制，提高 Fake SNI 下初次握手成功率并减少误分类；失败一次自动回退 Real SNI。

范围：
- 握手使用 Fake SNI；验证阶段替换为真实域名 ServerName；
- 失败（域名 / SAN 白名单不符）即触发单次 Real SNI 重握手；
- 分类：链前错误→Tls，域名 / SAN 不符→Verify；
- 采集 fallback 计数与原因；
- 开关：`tls.realHostVerifyEnabled`（默认 true，可关闭回退到旧逻辑）。

交付物：定制 verifier、配置与日志字段、回退触发统计、指标 `real_host_fallback_total{reason}`、测试用例（成功 / 回退 / 关闭开关）。

验收：
- 开启时 Fake→Real 回退率低于设定阈值（基线 <5%）；
- 关闭开关后日志不再输出 real_host 关键字且行为与 P3.1 一致；
- 错误分类与既有规则兼容（无新增 category）。

回退：关闭 `tls.realHostVerifyEnabled` 或移除 verifier 包装。

风险&缓解：
- 证书与真实域不匹配高频 → 自动统计 + 可以快速关闭。
- 性能开销（额外 verifier 构造）→ 缓存证书链结果 / 只对 Fake 分支启用。

#### 实施要点表
| 项 | 内容 |
|----|------|
| 开关 | `tls.realHostVerifyEnabled` (bool, default true) |
| 验证主机来源 | URL 解析出的目标域（白名单匹配对象） |
| 握手 SNI | 可能为 Fake；不参与 SAN 匹配 |
| 回退条件 | 域名验证或白名单失败（Verify）立即触发 Real SNI 重握手一次 |
| 失败分类 | TLS 早期错误→Tls；链成功但域名不符→Verify |
| 指标补充 | `real_host_fallback_total{reason}` |
| 日志关键词 | `real_host_verify=on` / `fallback=real_sni` |
| 兼容关闭 | 置 false 恢复“按握手 SNI 验证”旧逻辑 |

实现摘要：自定义 `ServerCertVerifier` 包装 rustls 默认 verifier，传入真实域名生成 `ServerName`，完成链与域名校验后再执行白名单匹配；失败返回 rustls::Error::General 区分语义前缀 (`san_mismatch` / `name_mismatch`) 供分类层解析。

### P3.4 SPKI Pin 规划（Spec Only）
目标：在不立即强制校验的前提下，统一 SPKI Pin 字段与日志格式，收集潜在部署数据，为 P7 正式启用降低风险。

范围：
- 解析 `tls.spkiPins?: string[]` Base64URL 指纹；
- 不做握手失败判定，仅输出 `pin_inactive` 日志（含 pin_count / 指纹前缀）；
- 预留 mismatch 事件代码与回退策略说明（直接失败，不走 Fake→Real）。

交付物：解析与校验占位代码、日志格式、测试（解析 / 空列表 / 非法格式拒绝）。

验收：
- 存在 spkiPins 时握手成功（不影响连接）；
- 非法指纹（长度 / Base64 错）被拒并记录 Protocol 日志，不影响基础功能；
- 日志可统计潜在指纹覆盖率。

回退：删除或清空 `spkiPins`；跳过解析路径。

风险&缓解：规划与实施漂移 → 规格快照测试；大量指纹导致日志膨胀 → 限制上限（如 ≤10）。

#### 规划要点表
| 项 | 内容 |
|----|------|
| 字段 | `tls.spkiPins?: string[]`（Base64URL SPKI SHA256 列表） |
| 解析 | P3 解析并记录数量；若存在则在握手日志打印 `pin_count` |
| 校验策略 | P3 不执行；P7 起：若列表非空则期望证书 SPKI ∈ 列表否则 Verify 失败 |
| 轮换策略占位 | 支持多指纹并行；后续附加 `pinMetadata` 记录生效时间与计划淘汰时间 |
| 事件预留 | `cert_fp_pin_mismatch`（P7 启用） |
| 回退 | 与 Fake/Real 链独立；Pin 不匹配不触发 Fake→Real（直接失败） |
| 日志 | `pin_inactive` (P3)、`pin_match` / `pin_mismatch` (P7+) |

设计理由：提前统一字段与日志格式，避免 P7 引入破坏性事件/配置变更；P3 仅提供静默观察与宽容模式验证部署风险。

### P3.5 异常与回退稳健性
目标：通过故障注入与分类验证强化 Fake→Real→Default 路径的稳定性与一致性。
范围：
- 故障注入点：TCP 连接失败、TLS 握手错误（模拟证书错误 / 超时）、读写中断；
- 分类一致性测试：同类错误在 clone/fetch/push 分类一致（Network/Tls/Verify）；
- 回退链验证：统计各阶段 fallback 时产生的最终结果（成功/失败）比例；
- 重试协同：在 Fake 阶段网络类错误允许一次 Real 重试，不重复 Fake；
- 事件：新增 `{ code: "adaptive_tls_fallback", from: Fake|Real, to: Real|Default, reason }`（仅当发生阶段切换时发，一次一条）。
- 安全护栏：短时间内 Fake→Real 失败率 > 阈值（如 20%）自动临时禁用 Fake（内存 flag TTL）。
交付物：注入框架（feature 或测试构建标志）、fallback 事件、统计与临时禁用逻辑。
接口/配置：`http.autoDisableFakeThresholdPct`，`http.autoDisableFakeCooldownSec`（命名复用 http 命名空间避免新增根）；运行期 volatile flag 不写回配置。 
指标：`fallback_events_total{from,to,reason}`，`auto_disable_total`。
验收：
 - 故障场景下 fallback 事件数与注入次数匹配；
 - 自动禁用在阈值触发后生效且 TTL 到期恢复；
 - 分类一致性测试通过；
回退：关闭 autoDisable（阈值=0 或 disabled），移除注入钩子不影响生产；
风险&缓解：过度禁用导致性能回退——加入冷却计数限制；事件噪声——聚合相同 reason。

### P3.6 稳定性 Soak & 退出准入
目标：长时运行验证 + 指标阈值达标 + 文档与报告收束，决定进入 P4（IP 优选）。
范围：
- Soak 脚本：循环 clone/fetch（含浅克隆、push 混合）模拟真实分布；
- 指标聚合：P3 全阶段采集的 memory 结构导出快照（JSON）；
- 报告：生成稳定性报告（失败率、fallback 率、耗时分位数、指纹变更次数）；
- 准入门槛：不满足阈值阻塞进入 P4（需整改清单）。
交付物：`soak/` 下脚本与 README；报告生成工具；阈值配置文档；
接口/配置：不新增配置键；使用环境变量 `FWC_ADAPTIVE_TLS_SOAK=1` 触发 Soak 脚本模式。
指标：最终导出 JSON（路径 `soak-report.json`）。
验收：
 - 72h Soak：无内存泄漏（RSS 增长 <10%）；
 - 指标全部满足 1.5 成功标准表；
 - 报告生成并包含阈值检查结果；
回退：不进入 P4，列出修复项再复跑 soak；
风险&缓解：测试环境偏差——在至少两个网络环境运行；指标漂移——自动对比前一次报告差值。

## 3. 实现说明（占位，后续分别补充）

### P3.0 基线巩固与观测脚手架 实现说明
已完成（实现于本提交，后续补丁扩展已集成状态机与初步 timing 标记）：

1. 回退决策抽象
  - 新增文件 `core/git/transport/fallback.rs`。
  - 提供 `FallbackStage { None, Fake, Real, Default }`、`FallbackReason`、`FallbackDecision` 状态机：
    - `FallbackDecision::initial(ctx)` 基于 `DecisionCtx { policy_allows_fake, runtime_fake_disabled }` 生成首个阶段；
    - `advance_on_error()` 顺序执行 Fake→Real、Real→Default，Default 为终态；
    - 保留 `history` 供后续指标/事件使用（不在 P3.0 输出）。
  - 不涉及 I/O 与全局状态，保证可测与以后接入 rollout/hash/auto-disable 时的扩展性。

2. 指标接口脚手架 + 初步接入
  - 新增 `core/git/transport/metrics.rs`，定义：
    - `TimingRecorder`：记录 connect / tls / first-byte / total 四段耗时；
    - `TimingCapture` 纯数据结构（可序列化扩展时使用）；
    - `TransportMetricsCollector` trait 与 `NoopCollector` 占位实现；
  - 在 `CustomHttpsSubtransport::connect_tls_with_fallback` 中引入 `TimingRecorder`，记录 connect/tls 两段；`firstByte` 与 `total` 留待 P3.2 在 HTTP 读取包装层补齐；
  - 当前使用 `NoopCollector`，未产生事件或持久化输出。

3. 模块导出
  - `transport/mod.rs` re-export 新增的决策与指标类型，后续阶段最小侵入式接入。

4. 测试
  - 新增 `core/git/transport/tests/fallback_decision_tests.rs`：覆盖
    1) policy skip 直接 Default；
    2) 完整链 Fake→Real→Default 顺序；
    3) runtime_fake_disabled 行为等同 policy_allows_fake=false；
  - 在 `fallback.rs` 内部还包含本地单元测试（初始与 advance 链）。

5. 向后兼容
  - 已用状态机重写 `connect_tls_with_fallback` 内部逻辑，但保持“首次 Fake 失败即 Real，再失败错误上抛”语义一致；错误消息保留原格式前缀 `tls handshake:`；
  - 未输出任何新增事件 / 字段，前端零感知；
  - 配置模型暂未引入 `http.fakeSniRolloutPercent`（仅设计文档记录）。

6. 后续接入挂点（P3.1+）
  - 在子传输建立连接前：构造 `DecisionCtx`（加入 rollout / host hash / auto-disable flag）；
  - 握手失败回调处：调用 `advance_on_error()` 决定是否重试下阶段；
  - 成功建立后：结合 `TimingRecorder` 输出 `timing` 字段；
  - metrics collector 将在 P3.2 注册全局实现，支持聚合与导出。

7. 回退策略验证计划（待 P3.1 接入）
  - 引入 fault-injection feature 触发 Fake 握手错误，验证自动进入 Real；
  - 增加历史长度与阶段终态快照测试，锁定兼容性。

当前阶段未做：
  - 未采集 firstByte/total 以及未输出 timing 事件；
  - 未添加证书指纹逻辑（属于 P3.2 范围）。

风险与缓解：
  - 未来接入时可能与现有错误分类逻辑耦合：通过纯函数 & 明确阶段枚举降低冲突；
  - 状态机扩展（加入“SkipFakePolicy”仍映射 Default）不会破坏历史记录顺序；后续若新增 Real-Host 验证专属 reason，可并行新增 `FallbackReason` 枚举值，不影响现有测试。

#### 追加实现补充（2025-09-21）

在最初提交基础上，P3.0 已进一步补齐以下内容，使其成为后续 P3.1～P3.2 的“稳定挂点”：

1. 策略覆盖 (strategy override) 汇总事件一致性强化
  - 为 `GitFetch` / `GitPush` 引入始终存在的 `strategy_override_summary` 事件（即使没有任何 override / gating 关闭）。
  - `appliedCodes`：去重、与独立 applied 事件解耦；当 `FWC_STRATEGY_APPLIED_EVENTS=0` 时仅保留 summary（独立 *_strategy_override_applied 不发射）。
  - 额外测试：
    - 含 override：`fetch_summary_event_and_applied_codes` / `push_summary_event_and_gating_off`（gating 关闭仍有 summary）。
    - 无 override：`fetch_summary_event_no_override` / `push_summary_event_no_override`（`appliedCodes` 为空数组）。
  - 解析测试由简单字符串包含升级为双层 JSON 解析（外层 TaskErrorEvent，内层 summary），降低转义格式回归风险。

2. 回退状态机稳定性增强测试
  - 新增终态幂等（Default 再次 advance 不变）测试用例，保证未来扩展阶段不会破坏当前链条语义。
  - 通过历史长度与阶段序列断言锁定链条 Fake→Real→Default 顺序不被误改。

3. TimingRecorder 行为验证
  - 新增单元测试模拟典型顺序（connect -> tls），捕获 `connect_ms` / `tls_ms` 两段耗时并确保不会因重复 finish 导致 panic 或数据污染。
  - 记录点：目前只在握手内部建立与结束；`firstByte` / `total` 预留字段在 P3.2 的流读取包装层接入。

4. 错误分类映射基线
  - 建立表驱动（快照式）分类测试，锁定 git2 / I/O / TLS 场景→类别(Network / Tls / Verify / Auth 等) 的稳定性；后续新增 Real-Host 验证和 SPKI Pin 时，在分类表增量扩展。

5. 事件开销与兼容性
  - 统计：当前每个 push/fetch 仅新增 1 条 summary（以及在 gating=1 且有变更时的少量 applied 事件），单任务额外事件控制在 ≤3（符合 P3.1 规划目标）。
  - 旧前端兼容验证：summary 事件沿用 `task://error` 通道 + code 字段，不影响已有错误渲染分支（未改变原 code 语义，只是信息型）。

6. 调试与可观测准备
  - 在 push 汇总前加入 `tracing::debug!(kind="push", applied_codes=..)` 日志，为后续 rollout 观察 / 采样偏差排查提供轻量信号。
  - 保持 metrics 仍为 NoopCollector，确保当前阶段不会引入额外运行时开销；后续启用只需用真实 Collector 替换注入点。

7. 风险更新
  - 误判“summary 缺失”问题根因：测试过滤参数不匹配导致 0 test 运行；已通过 --list / --exact 方式核实真实执行路径并添加 no-override 场景防回归。
  - 字符串匹配脆弱性：转为结构化 JSON 解析后风险降低；若未来内层字段扩展（timing/fallbackStage），现有解析逻辑无需调整。

8. 为 P3.1 / P3.2 预留的明确挂点（现已具备）：
  - Rollout：在创建 `FallbackDecision::initial` 时添加一致性采样决策（hash(host) % percent）；未命中直接返回 `FallbackStage::Default`。
  - Auto Disable（P3.5）临时 flag：在构造 `DecisionCtx` 增加 `runtime_fake_disabled`（已存在占位语义），只需接入统计触发逻辑即可。
  - Timing 扩展：在请求读取（首字节回调）与任务完成附近调用 `mark_first_byte()` / `finish()`，随后将 `TimingCapture` 注入 summary 或独立 timing 事件。
  - 指纹采集：握手成功后位置唯一，当前连接函数已集中路径，可在 Fake / Real 成功分支后注入。

9. 完成度小结（P3.0 封板视角）
  | 模块 | 状态 | 备注 |
  |------|------|------|
  | FallbackDecision | 完成 | 行为与 P2 相同，可扩展 |
  | TimingRecorder (connect/tls) | 完成 | firstByte/total 待 P3.2 |
  | Strategy summary 一致性 | 完成 | fetch/push + gating 覆盖 |
  | Applied codes 去重 | 完成 | 去重 + gating 分离 |
  | 错误分类基线 | 完成 | 快照式测试，后续增量 |
  | 指纹采集 | 未开始 | P3.2 |
  | Rollout 采样 | 未开始 | P3.1 |
  | Real-Host 验证 | 未开始 | P3.3 |
  | SPKI Pin 解析占位 | 未开始 | P3.4 |

该补充使 P3.0 具备“低变更面、可快速回退、测试护栏充分”的基线特征，可安全进入 P3.1（默认启用 + 百分比采样）。

### P3.1 默认启用与渐进放量 实现说明

（此处留空，后续补充实现细节）

### P3.2 可观测性强化（基础） 实现说明

（此处留空，后续补充实现细节）

### P3.3 Real-Host 验证 实现说明

（此处留空，后续补充实现细节）

### P3.4 SPKI Pin 规划 实现说明

（此处留空，后续补充实现细节）

### P3.5 异常与回退稳健性 实现说明

（此处留空，后续补充实现细节）

### P3.6 稳定性 Soak & 退出准入 实现说明

（此处留空，后续补充实现细节）
