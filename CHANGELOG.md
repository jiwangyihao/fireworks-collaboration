# Changelog

## v1.0.0 (2025-10-12)

The first consolidated release that brings every planned capability together.

- Unified Git task pipeline covering Clone/Fetch/Push、本地操作、批量调度与子模块处理，基于 git2-rs 与统一任务事件协议。
- Adaptive transport stack：Fake/Real SNI 自动切换、TLS 时序采样、SPKI 指纹监控、自动禁用窗口与 Soak 验证。
- IP 池运行时：多来源采样、熔断治理、候选缓存、按需刷新、Tauri 命令与前端实验室视图。
- 代理中心：HTTP/SOCKS5/System 模式、自动降级和恢复、健康检查、调试日志、前端配置面板。
- 凭证管理：系统钥匙串→加密文件→内存三层存储、AES-256-GCM 加密、Argon2id 密钥派生、审计日志、访问控制、UI 集成与 Git 自动填充。
- 工作区与团队协作：工作区模型、批量任务并发、子模块递归操作、状态缓存服务、团队模板导入导出。
- Observability：事件桥接、窗口聚合、层级降级、可视化面板、指标导出入口（默认惰性启动）、Soak 报告与阈值评估。
- 测试与质量：统一测试目录、事件 DSL、属性测试、覆盖率检查脚本、凭证基准测试套件。

### Notable Fixes & Enhancements

- 所有传输路径强制真实主机名证书校验，Fake SNI 也不例外。
- `http.fakeSniHosts` 仅用于候选排序，改写白名单改由内置列表（源自 IP 池内置域）统一维护并去重。
- HTTP 客户端与自定义 Git 传输均通过 RAII guard 上报 IP 池使用结果，避免漏写熔断统计。
- `setup.rs` 重新梳理命令注册，解决宏展开不稳定导致的稀有 panic。
- Tailwind v4 迁移完成，全局样式统一在 `src/style.css`，组件禁用 `@apply`。
- 观测层导出与告警暂时惰性/关闭，但 UI 已对 404 做降级处理。

### Testing & Quality Gates

- Rust: `cargo test -q` 覆盖 git/transport/ip_pool/proxy/credential/workspace/observability 全部模块。
- Front-end: `pnpm test` 覆盖组件、Pinia store、API 桥接与 utils。
- Soak：`src-tauri/src/soak` 目录提供自适应 TLS、IP 池、代理的阈值检查；CI/灰度可直接运行。
- Benchmarks：`cargo bench`（凭证、事件吞吐）可选执行。

### Operational Notes

- 默认配置启用 IP 池、Fake SNI、凭证安全与基础观测，代理/工作区/导出需按需开启。
- 回退路径按“观测→协作→网络→安全”顺序逐层关闭即可回归最小 clone/fetch 基线，详见 `doc/IMPLEMENTATION_OVERVIEW.md` 的回退章节。

## v0.9.0 (2025-10-07)

- Tailwind v4 升级、全局样式入口与组件样式统一化。
- 修复 Tokio runtime 嵌套导致的 IP 池 panic，引入异步桥接与 outcome RAII。
- 命令注册改为显式列表，解决 `__cmd__*` 宏展开冲突。
- 暂停 metrics export 立即启动，准备延迟到首个 snapshot 请求。

## v0.8.0 (2025-10-04)

- 凭证存储完成安全审计、准入评审与性能基准测试；AES-256-GCM + Argon2id 正式启用。
- 前端凭证表单、列表、主密码对话框、审计日志视图全部落地。
- `cargo bench` 结构化配置与警告修复。

## v0.7.0 (2025-10-01)

- 代理管理中心：HTTP/SOCKS5/System 模式、自动降级与健康检查、前端配置面板、事件体系。
- `ProxyFailureDetector`、`ProxyHealthChecker`、系统代理检测与调试日志。

## v0.6.0 (2025-09-29)

- IP 池运行时重写：预热调度、单飞采样、熔断治理、事件与 Tauri 命令。
- 前端新增 IP 池实验室、检查视图预热步骤、API 测试。

## v0.5.0 (2025-09-25)

- 自适应 TLS rollout、Fake/Real 回退、SPKI 指纹日志、自动禁用窗口、结构化事件。
- Soak runner 与指标提取。

## v0.4.0 (2025-09-22)

- Git 策略覆盖扩展：HTTP/Retry 任务级参数、结构化事件、Partial filter fallback。
- 本地 Git 操作命令（commit/branch/checkout/tag/remote）。

## v0.3.0 (2025-09-19)

- Push 支持、自定义 smart subtransport、Retry v1、错误分类与 UI 集成。

## v0.2.0 (2025-09-14)

- 完成 gitoxide → git2-rs 迁移，统一任务与事件模型。

## v0.1.0 (2025-09-13)

- 初始交付：HTTP Fake 请求调试、Clone 任务、前端基本面板、手工验收脚本。

---

> 详细的设计、阶段性报告与历史记录已迁移至 `doc-archive/` 与 `doc/` 各类 handoff 文档，以便追溯具体实现细节。
