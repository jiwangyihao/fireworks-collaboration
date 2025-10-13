# Fireworks Collaboration

Fireworks Collaboration 是一款基于 Tauri + Vue 3 + TypeScript 的桌面应用，用于在复杂网络环境下统一管理 Git 传输、网络加速、凭证安全和团队工作区协作。当前版本已经合并全部计划中的核心能力，聚焦以下目标：

- **可靠的 Git 任务执行**：Clone/Fetch/Push、本地操作与批量调度共用统一的任务模型、事件契约与重试策略。
- **智能传输栈**：自适应 TLS、Fake SNI 改写、握手时序采样与 IP 池优选确保安全与性能兼得。
- **网络治理工具箱**：内置 IP 池、代理自动降级、HTTP 调试面板，帮助快速诊断与回退。
- **企业级安全基线**：凭证三层存储、AES-256-GCM 加密、审计日志、访问控制与日志脱敏。
- **团队协作**：工作区模型、子模块管理、批量任务视图、模板化配置分发。
- **可观测性与测试**：指标/事件桥接、快照式导出、可视化面板、Soak 测试与统一测试套件。 

## 主要能力

- **Git 任务与本地操作**：Clone、Fetch、Push、Commit、Branch、Checkout、Tag、Remote 管理；提供策略覆盖（HTTP/TLS、Retry）与任务级进度、错误分类。
- **传输与网络优化**：自适应 TLS、Fake/Real SNI 切换、SPKI Pin 校验、握手时序分析、IP 池采样与缓存、自动禁用/熔断、代理模式（HTTP/SOCKS5/System）与自动降级。
- **凭证与安全**：系统钥匙串→加密文件→内存的三层存储回退，AES-256-GCM + Argon2id 密钥派生，审计日志、访问控制（失败锁定）、凭证过期提醒与 Git 自动填充。
- **工作区与批量协作**：工作区模型、仓库标签、子模块递归操作、批量 Clone/Fetch/Push 并发调度、跨仓状态缓存、团队配置模板导入导出。
- **可观测性**：事件桥接、窗口聚合、可配置导出、分层降级、指标面板（前端 UI）、Soak 报告与阈值评估。
- **开发与测试支撑**：统一 DSL 测试框架、属性测试、覆盖率工具、Soak 与基准测试脚本、可复制的手工验收脚本。 

详细设计与交接文档位于 `doc/` 目录，按能力域提供技术方案、验收报告与操作指南。

## 构建与运行（Windows / PowerShell）

前置：安装 pnpm 与 Rust 工具链（Rust 版本请参见 `src-tauri/rust-toolchain.toml`）。

```powershell
# 安装依赖
pnpm install

# 仅启动前端（调试 UI 用）
pnpm dev

# 启动完整桌面应用（Tauri）
pnpm tauri dev
```

默认配置位于 `config/`，可复制 `.example` 文件后按需修改。运行期间的配置热更新规则请参考 `doc/IMPLEMENTATION_OVERVIEW.md` 与对应模块交接文档。

## 测试

```powershell
# 前端单元 / 组件测试
pnpm test

# 后端（Rust）测试
powershell -NoProfile -ExecutionPolicy Bypass -Command "cd '$PWD/src-tauri'; cargo test --quiet"

# 可选：运行 Rust 基准测试
powershell -NoProfile -ExecutionPolicy Bypass -Command "cd '$PWD/src-tauri'; cargo bench"
```

所有现有用例应保持通过。更多测试矩阵、Soak 流程与手工验收脚本请参考 `doc/TESTS_REFACTOR_HANDOFF.md` 与 `doc-archive/MANUAL_TESTS.md`。

## 开发调试工具

- 顶栏仅保留一个“开发调试”按钮，点击后进入 `DeveloperToolsView` 聚合页。
- 聚合页按卡片列出凭据管理、工作区、Git 面板、HTTP 测试、IP 池实验室、GitHub Actions 调试等调试入口。
- 可观测性面板仅在 `observability.enabled && observability.uiEnabled` 时显示对应卡片，避免在未启用监控的环境暴露入口。

## 安全与合规基线

- TLS 链验证与真实主机名校验始终开启；Fake SNI 场景仍使用真实域名执行证书校验。
- Fake SNI 改写严格依赖内置域名白名单（来源 `ip_pool::preheat::BUILTIN_IPS`），配置仅能调整候选顺序，无法新增域名。
- 日志默认脱敏 Authorization、凭证字段；审计日志与导出指标遵循最小披露原则。
- 凭证存储默认优先系统钥匙串，其次加密文件，最后内存存储；所有层级均提供过期管理与访问控制。 

## 文档导航

- `doc/IMPLEMENTATION_OVERVIEW.md`：全量实现概览、架构与运维指引。
- `doc/TECH_DESIGN_*.md`：能力域设计文档。
- `doc/*_IMPLEMENTATION_HANDOFF.md`：交接说明与回退策略。
- `doc/P*_SECURITY_AUDIT_REPORT.md` / `doc/*_ACCEPTANCE_REPORT.md`：安全与准入评审。
- `doc/TESTS_REFACTOR_HANDOFF.md`：测试组织、DSL 与矩阵说明。
- `doc-archive/`：历史设计与手工验收脚本。

## 开发环境建议

- 编辑器：VS Code + Volar + rust-analyzer + Tauri 扩展。
- 代码风格：Rust 使用 `cargo fmt`，前端遵循项目内 ESLint/Prettier 配置。
- 提交规范：遵循 Conventional Commits（中文描述），例如 `feat: 添加工作区批量调度`。

如需了解具体模块的实现或调试步骤，可继续阅读 `doc/IMPLEMENTATION_OVERVIEW.md` 及对应的 handoff 文档。


