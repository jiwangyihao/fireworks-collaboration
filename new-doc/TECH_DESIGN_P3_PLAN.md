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
9. 启用 SPKI Pin 强校验：当配置 `tls.spkiPins?: string[]`（Base64URL SPKI SHA256）非空时，握手后将证书 SPKI 指纹与列表匹配；若不匹配则按 Verify 失败直接终止（不触发 Fake→Real），记录 `cert_fp_pin_mismatch` 事件与 `pin_mismatch` 日志；支持并行轮换（多指纹）。

### 1.3 范围
- 后端（Rust）：传输层注册与默认启用逻辑、回退决策统一、指标/指纹采集、事件字段扩展、配置与 gating、长时运行稳定性验证脚手架。
- 前端（Vue/Pinia）：可选显示 timing 与 usedFakeSni；错误/回退信息友好化；不强制新增 UI 结构（渐进增强）。
- 文档：更新配置模型、事件与指标说明、回退链描述与安全策略说明。

### 1.4 不在本阶段
- IP 池与 IP 优选（P4）。
- 代理与自动降级（P5）。
-（已纳入本阶段：SPKI Pin 强校验）。
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
- 配置：保持 `http.fakeSniEnabled` 键；新增 `http.fakeSniRolloutPercent?`（0..100，可缺省）；在 `tls` 命名空间增量添加 `metricsEnabled`、`certFpLogEnabled`、`certFpMaxBytes`、`realHostVerifyEnabled`（默认 true）；新增 `spkiPins?`（Base64URL SPKI SHA256 列表，非空即启用强校验，建议 ≤10 个）。
- 事件：在既有 `task://progress|error` 通道新增可选结构 `{ timing?, usedFakeSni?, fallbackStage?, certFpChanged? }` 与信息型代码 `adaptive_tls_rollout`、`adaptive_tls_fallback`、`cert_fingerprint_changed`，并启用 `cert_fp_pin_mismatch`（Pin 不匹配时发送）。
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
| Pin 强校验误配 | 中 | 配置的 SPKI 列表未覆盖现网证书或轮换 | 渐进发布/灰度域试点 + 快速回退（清空 pins）+ mismatch 事件告警 + 列表上限控制 |

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
| P3.4 | SPKI Pin 强校验（启用） | Pin 列表解析 / 强校验 / pin_match & pin_mismatch 事件与日志 |
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
（本阶段暂不引入 Real-Host 验证与 SPKI Pin 校验，二者分别在 P3.3 / P3.4 实施；P3.4 为强校验。）
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

### P3.4 SPKI Pin 强校验
目标：当配置 `tls.spkiPins` 非空时，对服务端证书的 SPKI 指纹进行强制匹配校验；不匹配即按 Verify 失败直接终止连接（不触发 Fake→Real），以提升中间人与证书替换风险的防护能力。

范围：
- 解析并校验 `tls.spkiPins?: string[]`：Base64URL（无填充，`-`/`_` 字符集）编码的 SHA256 值，长度固定 43；数量上限 ≤10（超限拒绝配置）。
- 握手完成后计算 leaf 证书 SPKI SHA256（与 P3.2 指纹体系一致/可替换为精确 ASN.1 提取），与列表做包含匹配。
- 列表为空或缺失时不启用 Pin 检查；列表非空但全部非法则视为配置错误（记录 Protocol 日志，禁用本次 Pin 检查）。
- 不匹配：立即返回 Verify 类错误，发送事件 `cert_fp_pin_mismatch`，并输出 `pin_mismatch` 日志；匹配则输出 `pin_match` 日志（含匹配前缀）。
- 不触发 Fake→Real 回退；Pin 与 Fake/Real 链路独立。

交付物：
- 强校验实现（verifier 内钩子）：与 Real-Host 验证兼容，先做链与域名验证，再执行 Pin 比对；
- 日志格式：在握手日志打印 `pin_enforced=on`、`pin_count=<n>`、`pin_match`/`pin_mismatch`；
- 事件：`cert_fp_pin_mismatch { host, spkiSha256(cert), matched:false, pinCount }`；
- 测试：解析（合法/非法）、空列表、上限裁剪或拒绝、匹配/不匹配路径、分类为 Verify；与 Fake→Real 回退路径的独立性测试。

验收：
- 存在合法 `spkiPins` 且包含目标证书 SPKI 时，握手成功（不影响连接）。
- Pin 不匹配时握手失败并归类 Verify，发 `cert_fp_pin_mismatch` 事件；不发生 Fake→Real 重试。
- 非法指纹（长度/编码）被拒并记录 Protocol 日志，不影响基础功能（Pin 检查被跳过）。
- 日志可统计 pin 覆盖率与命中率（pin_count、pin_match/mismatch）。

#### 测试覆盖补充（2025-09-25）
- `section_tls_fingerprint_and_logging::fingerprint_logs_include_spki_source_exact_and_fallback`：验证精确解析与退化路径均按 `spkiSource` 字段记录日志。
- `section_tls_pin_enforcement::pin_mismatch_emits_event_and_counts_verify`：断言 Pin 失败事件与 Verify 分类计数。
- `section_tls_pin_enforcement::pin_match_allows_connection_without_mismatch_event`：新增，确认合法 Pin 匹配时握手成功且不产生 `CertFpPinMismatch` 事件。
- `core::tls::verifier::tests::test_validate_pins_rules`：补充重复指纹去重的输入校验用例。

回退：删除或清空 `spkiPins` 即可；也可在运维层下发空数组临时停用。

风险&缓解：
- 误配导致连接失败 → 分阶段为关键域先行试点；提供快速回退（清空 pins）；记录详细 mismatch 事件便于定位。
- 日志膨胀 → 限制 Pin 列表上限（≤10），仅在结果（match/mismatch）时输出一次日志与事件。

#### 要点表
| 项 | 内容 |
|----|------|
| 字段 | `tls.spkiPins?: string[]`（Base64URL SPKI SHA256 列表，43 字符，无填充） |
| 解析 | 非法值拒绝并记录；合法值去重；上限 ≤10 |
| 校验策略 | 列表非空则强校验：证书 SPKI 必须 ∈ 列表，否则 Verify 失败 |
| 轮换策略 | 支持多指纹并行（旧+新）；可后续扩展 `pinMetadata`（可选） |
| 事件 | `cert_fp_pin_mismatch`（本阶段启用） |
| 回退 | 与 Fake/Real 链独立；不匹配不触发 Fake→Real（直接失败） |
| 日志 | `pin_enforced=on`，`pin_count`，`pin_match` / `pin_mismatch` |

设计理由：通过在 P3 即启用强校验，减少后续阶段（P7）引入破坏性变更的风险；配合 Real-Host 验证确保在 Fake SNI 场景也针对真实域执行 Pin；提供明确的回退路径与观测信号以降低运维风险。

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

## 3. 实现说明

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

#### 1. 目标
默认启用自适应 TLS（白名单域），提供稳定一致的按 host 百分比采样 + 信息事件；不破坏既有回退链 / 策略覆盖事件；形成后续 timing / 指纹 / fallback 指标挂点且可一键回退。

#### 2. 配置
| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| http.fakeSniEnabled | bool | true | 关闭后完全停用 Fake 改写与 rollout 事件 |
| http.fakeSniRolloutPercent | u8(0..=100) | 100 | 采样阈值；0=全部 MISS（仍保留回退逻辑框架） |
| http.hostAllowListExtra | string[] | [] | 附加允许进入 Fake 判定的域（不影响证书 SAN 校验逻辑） |

缺省缺字段时采用默认值（向后兼容）。`fakeSniEnabled=false` 优先级最高，直接短路。

#### 3. 采样 & 判定流程
1. Host allow 判定：主白名单命中或在 extra 列表中 → allow，否则直接 Default（不统计 MISS）。
2. 若 allow：`bucket = (SHA1(host)[0..2] => u16) % 100`；命中条件 `bucket < percent`。
3. 命中（HIT）→ 进入 Fake→Real→Default 回退链；未命中（MISS）→ 直接 Default。
4. 任务层可重复调用判定（纯函数），保证拥有 taskId 时再发 rollout 事件；无副作用。

特性：同 host 稳定；percent=100 恒 HIT；percent=0 恒 MISS（但与 fakeSniEnabled=false 区分：仍走判定框架）。

#### 4. 事件
新增 `adaptive_tls_rollout`（信息型）：
- 通道：`task://error`，`category=Protocol`。
- 触发：任务 summary 发出后确认此次连接为采样 HIT 且执行 Fake 改写。
- 单任务最多 1 条。未命中不发。
示例（message 内 JSON）：`{"taskId","kind","code":"adaptive_tls_rollout","percentApplied":37,"sampled":true}`。

保留：`strategy_override_summary` + `*_strategy_override_applied`（已改为精确事件匹配）。

#### 5. 内部计数器
`ROLLOUT_HIT` / `ROLLOUT_MISS` （Relaxed 原子）。allow 且：HIT 计 HIT；bucket>=percent 计 MISS。非 allow host 不计入。当前仅内存占位，后续指标导出。

#### 6. 测试策略（关键点）
| 场景 | 断言 |
|------|------|
| 0% | 无 `adaptive_tls_rollout` 事件 |
| 100% | 有且最多 1 条事件 |
| hostAllowListExtra | 非主白名单域可触发事件 |
| 一致性 | 同 host 多次要么全 HIT 要么全 MISS |
| insecure push | 精确 1 条 `tls_strategy_override_applied` |
| no override | summary 存在，appliedCodes=[] |
| 并发串行化 | 不丢失 summary / 不重复 rollout |
| helper 事件 | 无网络仍能稳定发 summary + rollout |

事件匹配规范：仅使用精确 `"code":"<event_code>"`；不要用子串（避免与 summary.appliedCodes 冲突）。

#### 7. 回归修复
| 问题 | 根因 | 修复 |
|------|------|------|
| 0% 仍发事件 | OnceLock 配置基目录第二次未生效 | 改为统一路径修改后保存 |
| push 计数=2 | 子串匹配误计 summary.appliedCodes | 精确 code 匹配 |
| summary 偶发缺失 | 并发 drain 竞争 | 事件捕获互斥 |

#### 8. 回退 / 兼容
快速暂停：`fakeSniRolloutPercent=0`；完全关闭：`fakeSniEnabled=false`。无需重启。旧前端忽略新事件不受影响。事件顺序：summary → rollout（若有）。

#### 9. 性能 & 风险
单次 SHA1 + 原子增量；未观察显著耗时或锁竞争。风险：采样倾斜（通过 SHA1 均匀性 + 测试护栏缓解）、计数误判（精确匹配策略）。

#### 10. 后续挂点
| 未来 | 复用 | 增量 |
|------|------|------|
| P3.2 timing/usedFakeSni | rollout 判定结果 | 在 summary/timing 事件扩展字段 |
| P3.2 指纹 | Fake/Real 成功分支 | 指纹缓存 + change 事件 |
| P3.5 fallback 事件 | 状态机历史 | 生成 `adaptive_tls_fallback` |
| P3.5 auto-disable | 计数 + 错误率 | runtime flag 切换 |

#### 11. 验收结论
所有计划测试矩阵通过；0% / 100% 行为正确；无重复事件；前端存储兼容；Changelog 条目已准备；形成下一阶段指标与可观测扩展挂点。

#### 12. Changelog 建议
Added: Adaptive TLS percentage rollout (host-stable sampling) + event `adaptive_tls_rollout` (backward compatible). Changed: `http.fakeSniEnabled` now defaults true. Internal: in-memory rollout hit/miss counters.

#### 13. 回退验证
1. 设置 percent=0 → 重新执行 clone/fetch 无 rollout 事件。\n2. 设置 enabled=false → 无改写/无事件。\n3. 原 override / partial / shallow 用例全部保持通过。

#### 14. 一句话总结
以最小侵入方式完成默认启用与确定性采样，事件与指标挂点就绪且可一键回退，为后续 timing / 指纹 / fallback 与自适应禁用提供稳定基线。

### P3.2 可观测性强化（基础） 实现说明

本阶段已完成如下实现，形成后续 Real-Host 验证与自动禁用（P3.3/P3.5）的数据基础：

1. 配置与默认值
  - 新增 `tls.metricsEnabled=true`：关闭后不再采集或输出 timing 事件；不影响传输功能。
  - 新增 `tls.certFpLogEnabled=true`：关闭后不写入 `cert-fp.log` 且不触发指纹变更标志（`cert_fp_changed=false`）。
  - 新增 `tls.certFpMaxBytes=5MB`：超过阈值时对 `cert-fp.log` 进行单文件滚动（rename -> `cert-fp.log.1`，新建空文件继续）。

2. Timing 采集
  - 在自定义子传输握手路径建立 `TimingRecorder`（connect_start/end + tls_start/end），完成后计算 total；
  - 通过 thread-local (`TL_TIMING`) 暂存一次连接的 `TimingCapture{connect_ms,tls_ms,first_byte_ms(total 起始),total_ms}`；
  - firstByte 捕获：最初占位为 total 前缀；后续 refinement 已在 HTTP 响应解码流 (`SniffingStream`) 首次读出正文字节时调用 `tl_mark_first_byte()` / 精确更新，避免预估误差。
  - 任务完成（成功或失败）时读取 snapshot -> 结构化事件 `StrategyEvent::AdaptiveTlsTiming`；未开启 metrics 或无 capture 不发。

3. Fake / Fallback 关联
  - 记录最终阶段枚举（Fake|Real|Default），以及本次是否使用 Fake SNI（成功分支的 used_fake_sni flag）。
  - 通过 thread-local 保存 `usedFakeSni` 与 `fallback_stage`，统一注入 timing 事件；为 P3.5 的 fallback 事件解耦准备。

4. 证书指纹 Fingerprint 模块
  - 计算：leaf 证书整体 SHA256 +（简化）SPKI 区段 SHA256（当前未做 ASN.1 精确剪裁，后续 Pin 阶段可替换为 x509 解析）。
  - Base64URL（无 padding）编码：`spkiSha256` / `certSha256`。
  - 内存缓存：LRU (最大 512 host) + 24h 时间窗；同一 host 在窗口内指纹一致不再标记 changed；首次或改变时 `changed=true`。
  - 日志格式（JSON Lines）：`{"ts", "host", "spkiSha256", "certSha256", "changed"}`；超限滚动；失败静默（不影响主流程）。
  - 结构化事件：除布尔 `cert_fp_changed` 外，现已在指纹首次记录与后续真实变更时主动发射 `StrategyEvent::CertFingerprintChanged { host, spki_sha256, cert_sha256 }`，便于前端或外部系统即时响应证书轮换。

5. 结构化事件扩展
  - 新增 `StrategyEvent::AdaptiveTlsTiming`：一次任务仅 0~1 条；字段可选不破坏旧前端。
  - 启用 `StrategyEvent::CertFingerprintChanged` variant：P3.2 refinement 已激活触发逻辑。

6. 回退与开关
  - 即时关闭 timing：`tls.metricsEnabled=false` → 任务级不再产生 AdaptiveTlsTiming；thread-local 仍被安全清理。
  - 仅关闭指纹：`tls.certFpLogEnabled=false` → 日志停止，`cert_fp_changed` 恒为 false。
  - 完全回退到 P3.1：同时关闭上述两个开关；无代码路径需移除。

7. 测试与验证
  - 单元：配置默认值、timing recorder 幂等 finish、LRU 插入与裁剪（通过容量上限模拟）、指纹变更标志首变更/重复不变。
  - 集成：clone/fetch/push 终态存在 timing 事件；关闭 metrics 不出现；指纹日志文件创建并随多次连接增长；达缩小阈值（测试注入）后滚动。
  - 所有既有测试（76+）保持通过（clone/fetch/push/策略覆盖/回退矩阵）。

8. 性能影响评估（快速基线）
  - 额外 SHA256 两次 + 少量内存 HashMap 操作；在本地 100 次握手循环下无显著 wall time 增量（<1ms 波动范围）。
  - 可通过后续基准（P3.5 soak）细化。

9. 风险与缓解
  | 风险 | 描述 | 缓解 |
  |------|------|------|
  | 日志膨胀 | 高频任务导致 cert-fp.log 频繁滚动 | 限制大小 + changed 去重 + 可关闭 |
  | 解析不精准 | 简化 SPKI 提取可能导致与真实 SPKI 轻微差异 | 后续引入 x509 解析库替换 | 
  | 事件噪声 | 失败任务亦发 timing | 前端可按 state=Failed/`total_ms` 做过滤 |

10. 后续挂点（P3.3/P3.5）
  - Real-Host 验证：在现有握手成功分支注入域名匹配前后 timing 点；若触发回退更新 thread-local stage 再记录。
  - Fallback 事件：利用已存在的 `fallback_stage` 与状态机 history 生成 `adaptive_tls_fallback`（新增 Strategy/Transport variant）。
  - 自动禁用：统计 Fake->Real 失败率（组合计数器 + 时间窗）后在决策 ctx 注入 `runtime_fake_disabled` flag。

11. 验收结论（P3.2 范围）
  - 新增配置默认值正确；关闭开关行为符合设计。
  - Timing / 指纹日志在典型 clone/fetch/push 工作流正常出现。
  - 无破坏性 API 变更；旧前端忽略新事件仍可完成任务展示。

12. 回退策略验证
  - 设置 `tls.metricsEnabled=false` 后重新执行 clone → 未出现 `AdaptiveTlsTiming`。
  - 设置 `tls.certFpLogEnabled=false` 后日志文件不再增长且 `cert_fp_changed=false`。
  - 两者同时关闭：功能路径退化到 P3.1 行为（仅 rollout 事件）。

13. 一句话总结
  > 已以最小侵入方式交付 timing（含精确首字节）+ 指纹与结构化事件（含主动 CertFingerprintChanged），为后续 Real-Host 验证与自动回退提供可观测基线，可通过 2 个布尔开关即时回退。

### P3.2 最终实现补充说明（Refinement 完成态）

本补充章节记录在最初 P3.2 可观测性交付后追加的精确化与测试强化内容，形成可回退且高置信度的最终实现基线。

#### 1. 架构概览
| 组件 | 职责 | 关键点 |
|------|------|--------|
| metrics.rs (TimingRecorder + TL) | 记录 connect / tls / firstByte / total | 通过 thread-local 快照在任务终态统一发事件；测试 override 支持强制 gating |
| http/stream.rs (SniffingStream) | 首字节精确标记 | 第一次解码出正文数据时调用 tl_mark_first_byte（精确 firstByte） |
| fingerprint.rs | 证书 leaf SPKI & 整体哈希、LRU + 24h 抑制、日志滚动、变更事件 | changed=true 时追加日志并发 `CertFingerprintChanged` |
| tasks/registry.rs helper | 测试辅助发 timing 事件 | helper 遵守 metrics_enabled gating，避免测试误判 |
| structured events | `AdaptiveTlsTiming` / `CertFingerprintChanged` | Additive，不破坏旧消费者 |

#### 2. 时序流程（成功握手路径）
1. 建立 TCP：`mark_connect_start/end` -> connect_ms
2. TLS 握手：`mark_tls_start/end` -> tls_ms
3. 握手成功：记录 fallback 阶段、used_fake_sni、调用 `record_certificate` -> 可能触发指纹事件 & set cert_fp_changed
4. 首次读取 HTTP 正文：`tl_mark_first_byte` -> first_byte_ms
5. 任务结束（成功 / 失败）：`finish()` 计算 total_ms；`tl_snapshot` 生成 `AdaptiveTlsTiming` 事件（若 metrics_enabled）

#### 3. 线程局部设计
Thread-local 保存 (timing, used_fake, fallback_stage, cert_fp_changed)。无共享锁争用；每任务链路上线性使用一次，结束后事件读取即完成生命周期，不需显式清理（下一任务覆盖）。

#### 4. 指纹流程细节
1. 取 leaf cert DER，计算 cert SHA256 & 简化 SPKI SHA256（后续 Pin 阶段可替换精确解析）。
2. LRU (512 hosts) + 24h window：首次或内容变化 => changed=true；相同内容且仍在窗口 => changed=false。
3. JSONL 追加：`{ts,host,spkiSha256,certSha256,changed}`；超大小（`cert_fp_max_bytes`） rename → `.1`；新文件继续。
4. changed=true 时发 `CertFingerprintChanged`（含 Base64URL 指纹）。
5. 测试提供 `test_reset_fp_state` 清理缓存隔离用例。

#### 5. 事件与 gating
| 事件 | 触发条件 | 抑制条件 |
|------|----------|----------|
| AdaptiveTlsTiming | metrics_enabled && 有 timing.capture | metrics_enabled=false 或未建立 timing |
| CertFingerprintChanged | 指纹首次或真实变化且 cert_fp_log_enabled | cert_fp_log_enabled=false |

#### 6. 测试矩阵（新增部分）
- metrics override：force false -> 无 timing；force true -> 有 timing；clear -> 继续有 timing。
- certFpLogEnabled=false：record_certificate 返回 None，无结构化事件。
- LRU 淘汰：>512 host 后重新写首 host 视为 changed，再次发事件。
- Base64 长度：SHA256 Base64URL 无填充长度=43。
- 精确 firstByte：事件 first_byte_ms 存在且不回退为 total 占位。
- 日志滚动：极小 maxBytes 触发 `.1`。

#### 7. 回退路径
| 目标 | 操作 | 副作用 |
|------|------|---------|
| 停止 timing | metricsEnabled=false | 不发 AdaptiveTlsTiming；其余功能不变 |
| 停止指纹 | certFpLogEnabled=false | 不写日志 / 不发指纹事件；timing 不受影响 |
| 恢复 P3.1 基线 | 同时关闭两者 | 仅保留原 rollout/fallback 逻辑 |

#### 8. 性能与安全考量
- 开销：两次 SHA256 + LRU HashMap O(1)/常量，未观察测试时间显著增长。
- 安全：日志仅含哈希，不包含 SAN 列表或私有数据；可快速关闭。

#### 9. 风险与缓解
| 风险 | 缓解 |
|------|------|
| 日志膨胀 | changed 抑制 + 滚动 + 可关 |
| 误判 firstByte | 精确流读钩子；测试断言存在 |
| Gating 不可测 | 覆盖 force on/off/restore 测试 |
| LRU 状态污染测试 | 提供 test_reset_fp_state |

#### 10. 后续扩展前置
- Real-Host 验证：可在握手成功后插入域名匹配再更新 fallback_stage，不影响现有 TL 结构。
- SPKI Pin：可替换简化 SPKI 计算为 ASN.1 解析并在指纹模块加入 pin 列表比对，不改事件 schema。
- 自动禁用 Fake：可在 registry 或 transport 统计失败率后通过 DecisionCtx 注入 runtime flag。

#### 11. 成熟度结论
当前实现具备：精确 timing、可控指纹事件、可回退 gating、完备测试矩阵与文档说明，可作为 P3.3/P3.5 的稳定观测基线。

#### P3.2 需求覆盖与回退验证总结（实现后补充）
| 需求 | 实现 | 开关/回退 |
|------|------|-----------|
| timing(connect/tls/firstByte/total) | `TimingRecorder` + thread_local + `AdaptiveTlsTiming` | `tls.metricsEnabled=false` |
| usedFakeSni 标记 | handshake 成功分支 `tl_set_used_fake` | 同上（事件抑制） |
| fallbackStage 输出 | 决策阶段字符串保存到 thread-local | 同上 |
| 指纹采集 + 变更检测 | `fingerprint.rs` + LRU + 24h window | `tls.certFpLogEnabled=false` |
| cert-fp.log 滚动 | 文件大小检查 & rename `.1` | `certFpLogEnabled=false` |
| 配置新增 | `metricsEnabled`/`certFpLogEnabled`/`certFpMaxBytes` | 直接修改配置热加载 |
| 事件兼容 | 新增 StrategyEvent 变体（Additive） | 不需回退（前端忽略未知） |
| 回退链未破坏 | Fallback state machine 逻辑未更改 | 关闭 metrics/fingerprint 不影响链路 |

新增测试覆盖（Refinement 扩展）：
- metrics override 正/反向：强制 false 抑制事件；强制 true 产出；清除 override 后回到配置默认。
- certFpLogEnabled=false：record_certificate 直接返回 None 且无事件。
- 指纹 LRU 淘汰：>512 host 触发最早 host 淘汰，再次记录视为 changed（事件再次发射）。
- Base64 长度校验：`spki_sha256` / `cert_sha256` 均长度 43（SHA256 Base64URL 无填充）。
- 精确 firstByte：首包钩子验证 `first_byte_ms` 存在且与设定值一致。
- 日志滚动：极小阈值下触发 `cert-fp.log.1` 生成。

这些测试确保：
1. Gating/override 行为可预测且可回退；
2. 指纹缓存与淘汰语义正确，不影响后续变更检测；
3. 事件数据格式稳定（长度验证为未来 Pin/分析提供护栏）；
4. 回退（关闭 metrics / log）不会泄漏残留事件。

验证：通过 cargo test 全量（所有既有 + 新增）无失败；关闭 metrics / fingerprint 后不产生 timing 或日志增长；Changelog & Design 文档已更新。

### P3.3 Real-Host 验证 实现说明

已完成（实现于本阶段提交）：

1) 配置与默认值
- 新增 `tls.realHostVerifyEnabled: true`（默认开启，关闭后回退到旧逻辑）。

2) 验证器实现
- 在 `core/tls/verifier.rs` 引入 `WhitelistCertVerifier { real_host_verify_enabled }`。
- 当开启且存在 `override_host`（来自握手使用 Fake SNI 场景下的真实域）时，将 `override_host` 构造成 `ServerName` 传入内置 `WebPkiVerifier` 执行链路与主机名验证；否则使用 SNI 对应的 `server_name`。
- SAN 白名单匹配始终优先使用 `override_host`（若存在），保证 Fake 握手下仍按真实域做白名单判定。

3) 工厂与调用方
- `create_client_config_with_expected_name()` 通过 `build_cert_verifier(tls, Some(expected_host))` 传递真实域；`build_cert_verifier` 将 `real_host_verify_enabled` 贯穿至 `WhitelistCertVerifier`。
- 子传输握手日志增加 `real_host_verify=<on|off>` 标识，便于观测。

4) 测试
- 单元测试增加捕获型 `CaptureVerifier`，验证开启时使用 `override_host`，关闭时使用传入 SNI；并更新 `TlsCfg` 新字段的默认与序列化断言。
- 全量 `cargo test` 通过（Windows 环境下 0 失败）。

5) 行为与分类
- 该实现遵循设计：链前错误仍归类为 Tls；链成功但域名/SAN 不符归类为 Verify（分类基线未改动）。第一次验证失败会进入既有 Fake→Real 回退链的 Real 分支重握手（由 `fallback` 状态机与握手路径共同驱动）。

回退：设置 `tls.realHostVerifyEnabled=false` 即可停用该逻辑，恢复按 SNI 验证的旧路径。


#### 补充实现细节：计数 / 分类 / 日志锚点 / 测试 / 运维

1) 回退计数与原因分类（内存计数器）
- 计数器：在 Fake→Real 触发时按原因维度进行累加，目前区分两类：
  - Verify：证书链通过但域名或 SAN 白名单不匹配（典型关键词：SAN、whitelist、name mismatch、verify）。
  - Tls：握手前/握手期错误或网络类错误（典型关键词：tls handshake、tcp connect、unexpected eof、timeout 等）。
- 分类规则：依据错误消息关键字进行表驱动映射，保证与现有错误分类基线一致；后续若扩充 Pin 相关原因，新增分类映射即可。
- 计数读取：当前为进程内原子计数器，供测试与运行期观测；正式导出指标时以 `{reason="Tls|Verify"}` 维度聚合。

2) 日志锚点
- 在 Fake 阶段握手失败切换至 Real 阶段时输出结构化日志锚点，便于日志检索与统计：
  - 关键字：`adaptive_tls_fallback: fake->real`，附带 `reason=Tls|Verify`。
  - 握手起始日志包含 `real_host_verify=on|off` 标识，用于确认当次连接的 Real-Host 验证开关状态。

3) 测试可测性（test-only 入口）
- 为提高单测确定性并避免依赖真实握手/网络，导出仅在测试构建可见的辅助函数：
  - `test_reset_fallback_counters()`：将内存计数器清零；
  - `test_snapshot_fallback_counters() -> (tls_total, verify_total)`：读取当前快照；
  - `test_classify_and_count_fallback(err_msg: &str) -> &'static str`：对给定错误字符串执行分类并累加计数，返回归类结果（"Tls" | "Verify"）。
- 这些接口在 `core/git/transport/http` 模块中实现，并通过 `transport::mod` 在测试中复用。

4) 测试矩阵（新增）
- Verifier 路径：
  - 开启 `realHostVerifyEnabled=true` 时，捕获型 verifier（测试桩）观察到用于链与域名匹配的 `server_name` 为真实域（override host）；
  - 关闭开关后，回退为使用握手 SNI 的旧行为。
- 分类与计数器：
  - 使用 `test_classify_and_count_fallback()` 输入代表性错误消息：
    - Verify：`"tls: General(SAN whitelist mismatch)"`、`"certificate name mismatch"`；
    - Tls：`"tls handshake: unexpected eof"`、`"tcp connect: timed out"`；
  - 断言返回的分类字符串与计数快照（Verify=2、Tls=2）。
- 细节修复：
  - rustls SCT 迭代器在单测中的类型约束调整为 `std::iter::empty::<&[u8]>()`，避免空切片迭代器的生命周期推断问题。
- 集成观测（可选）：
  - 在故障注入场景下检索 `adaptive_tls_fallback: fake->real` 日志锚点，确认 reason 填充正确。

5) 运维与观察建议
- 观察指标与日志：
  - 通过内存计数器（后续也可导出为指标）关注 Fake→Real 的回退率，重点关注 `reason=Verify` 的上升（通常意味着证书域名或白名单规则变化）。
  - 配合握手日志中的 `real_host_verify=on|off` 校验当次任务是否启用 Real-Host 验证；
  - 结合 P3.2 的 timing 事件与证书指纹变更事件，定位是否因证书轮换引发短期 Verify 升高。
- 应急回退：
  - 若 `Verify` 原因占比显著升高（如 >20% 且持续）且证书侧短期无法修复，可临时将 `tls.realHostVerifyEnabled=false`，回退至旧逻辑，后续再择机恢复；
  - 若 `Tls` 原因整体升高（网络/策略变更），可考虑临时下调或关闭 Fake SNI（P3.1 开关），观察恢复情况。
- 前端兼容：
  - 已在类型中同步 `realHostVerifyEnabled?` 可选字段；前端忽略该字段不影响现有渲染。

6) 边界与风险
- IDNA/国际化域名：当前按解析后的 ASCII/Punycode 结果进行 `ServerName` 构造；特殊大小写/同形异构域名需依赖上游 URL 解析约束。
- 直连 IP：证书域名匹配对纯 IP 目标通常不成立，允许策略上直接走 Default 或关闭 Fake；
- 通配名限制：`*.example.com` 不匹配多级（如 `a.b.example.com`），符合常见 CA 规则；
- 代理/MITM：若存在企业代理进行 TLS 拦截，Verify 类错误可能升高；可与 `tls.skipSanWhitelist`（若有）或策略白名单协同评估；
- ECH/HTTP/2：不在当前阶段范围内；不影响 Real-Host 验证逻辑。
- 性能：Real-Host 验证在 Fake 分支上仅影响 `ServerName` 选择与一次校验，实测对握手耗时无显著影响。

7) 兼容与回退摘要
- 单一布尔开关：`tls.realHostVerifyEnabled=false` 即可回退至旧校验路径；
- 与 P3.1 开关配合：需要时可将 `http.fakeSniEnabled=false`，完全绕过 Fake→Real 路径；
- 事件与类型：新增字段/日志均为加法，不破坏旧消费者。

### P3.4 SPKI Pin 强校验 实现说明

1) 配置与默认
- `tls.spkiPins?: string[]`（Base64URL，无填充，长度 43）；缺省或空数组：不启用 Pin；非空：启用强校验。
- 上限 ≤10：超过时视为配置错误，记录 Protocol 日志并忽略 Pin（为安全起见可选择“严格模式”下直接失败，默认忽略）。

2) 校验流程（与 Real-Host 兼容）
- 在 rustls 默认链与域名验证成功后，提取 leaf 证书 SPKI（与 P3.2 指纹相同算法；后续可替换为 ASN.1 精确解析）。
- 计算 SHA256，Base64URL 编码，与 `spkiPins` 做包含判断。
- 命中：记录 `pin_match` 日志；未命中：返回 Verify 类错误（`cert_fp_pin_mismatch`），不触发 Fake→Real，直接终止。
- 与 Real-Host 验证的顺序：链/域名 → Pin；Pin 失败不改变域名匹配结果的分类维度（仍归类 Verify）。

3) 日志与事件
- 握手起始日志：`pin_enforced=on`（当列表非空），`pin_count=<n>`；
- 结果日志：`pin_match` 或 `pin_mismatch`（附证书 SPKI 前缀与 count）；
- 事件：`cert_fp_pin_mismatch { host, spkiSha256(cert), pinCount }`（任务维度信息事件，category=Verify）。

4) 分类与回退
- 分类：Pin 不匹配 → Verify；
- 回退：不进入 Fake→Real；Pin 与回退链独立；
- 即时回退手段：清空/删除 `spkiPins`。

5) 测试矩阵
- 解析：合法 43 长度 Base64URL（含 `-`/`_`）、去重、大小写不敏感性（Base64URL 大小写敏感，要求严格保持大小写）；
- 非法：长度≠43、含非 URL 安全字符、超过上限、空字符串；
- 匹配路径：配置含现证书 SPKI → 成功；
- 不匹配路径：配置不含现证书 SPKI → 失败，事件与日志齐全；
- 与 Real-Host：二者同时开启时，域名通过但 Pin 不匹配 → Verify；域名不通过 → Verify（优先级不变）。

6) 运维建议
- Pin 轮换：同时下发新旧 SPKI；稳定后移除旧；
- 首次启用：建议对单域灰度验证，观察 `cert_fp_pin_mismatch` 事件是否升高；
- 监控：统计 pin_count、match/mismatch 比例与 host 维度分布。

#### 补充实现细节（2025-09-25）
- 配置与解析：`TlsCfg.spki_pins` 在 `core/config/model.rs` 中默认空数组，并随 `AppConfig` 序列化；每次握手调用 `validate_pins`（`core/tls/verifier.rs`）做去重、长度（43）与 Base64URL 合法性校验，超过 10 个或含非法值立即禁用本次 Pin 检查并记录 `pin_disabled_this_conn` 日志。
- 指纹提取：新增 `core/tls/spki.rs` 使用 `x509-parser` 精确解析 SPKI DER，失败时退化为整张证书哈希；返回值标记 `SpkiSource`，供日志区分 `exact` 与 `fallback` 路径，与 P3.2 指纹日志格式保持一致。
- 校验顺序：`WhitelistCertVerifier::verify_server_cert` 先执行链路/域名/白名单校验（Real-Host 开关生效），随后在 Fake→Real 回退之前比对 SPKI；命中时输出 `pin_match` debug 日志，未命中时立即抛出 `cert_fp_pin_mismatch`（归类 Verify），并通过 `StrategyEvent::CertFpPinMismatch` 将 host、SPKI、pin_count 发往全局事件总线。
- 观测锚点：握手起始日志包含 `pin_enforced` 与 `pin_count` 字段；禁用场景会带 `reason="invalid_pins"`，方便排查配置问题；事件与日志均以 override host（真实域）为准，便于与 Real-Host 验证对齐。
- 回退与统计：Pin 失败不会触发 Fake→Real，fallback 状态机停留在 Fake 阶段并直接返回错误；Verify 计数与 P3.3 分类表共享，运维可结合 `cert_fp_pin_mismatch` 事件频次与回退计数评估风险。

#### 测试与观测
- 单元测试：`core::tls::verifier::tests::test_validate_pins_rules` 覆盖长度、字符集、上限与去重；`test_pin_mismatch_returns_verify_error` / `test_pin_match_allows_connection` 验证强校验行为；`core::tls::spki` 模块确保指纹长度与 fallback 路径正确。
- 集成测试：`tests/git/git_strategy_and_override.rs` 新增三例——`pin_mismatch_emits_event_and_counts_verify` 断言 mismatch 场景触发结构化事件且返回 Verify，`pin_match_allows_connection_without_mismatch_event` 确认合法 Pin 不生成误报，`invalid_pins_disable_enforcement` 验证非法配置仅记录禁用日志不阻断连接。
- 回归保障：全量 `cargo test` 与 `pnpm test` 均覆盖新逻辑，MemoryEventBus 快照用于确保事件只在预期路径上出现。

### P3.5 异常与回退稳健性 实现说明

（此处留空，后续补充实现细节）

### P3.6 稳定性 Soak & 退出准入 实现说明

（此处留空，后续补充实现细节）
