# Fireworks Collaboration (P0 交付)

Tauri + Vue 3 + TypeScript 的桌面应用，用于“统一 Git 加速与传输控制”的分阶段落地验证。当前版本完成 P0：

## 🧩 P0 能力清单

- 通用伪 SNI HTTP 请求 API（http_fake_request）
	- 支持 Fake SNI 开关、重定向、完整 timing、body base64 返回
	- 前端通过 `src/api/tauri-fetch.ts` 提供的 Fetch 兼容封装调用 Tauri 命令：若未显式设置 `User-Agent` 将自动补全为 `fireworks-collaboration/tauri-fetch`，同时原样保留 Authorization 头以满足 GitHub API 认证
	- SAN 白名单强制校验；日志对 Authorization 自动脱敏
- 基础 Git Clone（基于 gitoxide）
	- 任务模型（创建/状态/进度/取消）；事件推送至前端
- 前端面板
	- HTTP Tester：便捷发起请求、Fake SNI/不安全验证开关（原型）、请求历史回填
	- Git 面板：输入仓库与目标目录、启动克隆、进度条、取消
	- 全局错误提示（脱敏）

详细技术方案见 `doc/TECH_DESIGN.md`、`doc/TECH_DESIGN_P0.md`，以及 P1 阶段细化文档 `doc/TECH_DESIGN_P1.md`（涵盖 Fetch/Push 与重试策略 v1 计划）。

## ⚙️ 构建与运行（Windows / PowerShell）

前置：安装 pnpm、Rust 工具链。

```powershell
# 安装依赖
pnpm install

# 运行前端（仅 Web，调试样式/页面用）
pnpm dev

# 启动桌面应用（Tauri）
pnpm tauri dev
```

## ✅ 测试

```powershell
# 前端单测
pnpm test

# 后端（Rust）单测
powershell -NoProfile -ExecutionPolicy Bypass -Command "cd '$PWD/src-tauri'; cargo test --quiet"
```

所有现有用例应通过；人工验收脚本见 `doc/MANUAL_TESTS.md`。

## 🔐 安全基线

- TLS 链验证与主机名校验强制开启：Fake SNI 场景也会按真实域名调用 `RealHostCertVerifier`，不可通过配置关闭。
- Fake SNI 目标域使用与 `ip_pool::preheat::BUILTIN_IPS` 同步的内置名单，禁止通过配置追加，避免误改写非预期域。
- 伪 SNI：仅改变握手的 SNI，不削弱 CA 验证；如失败可切换关闭后再试。
- 日志脱敏：Authorization 头默认记录为 `REDACTED`。

## 🧭 快速导航

- Git 面板：`/git`
- HTTP 测试器：主页导航进入
- 手动验收脚本：`doc/MANUAL_TESTS.md`
- 设计文档：`doc/TECH_DESIGN.md`、`doc/TECH_DESIGN_P0.md`
- P1 细化：`doc/TECH_DESIGN_P1.md`

## 🛠️ 开发者环境建议

- VS Code + Volar + Tauri + rust-analyzer


