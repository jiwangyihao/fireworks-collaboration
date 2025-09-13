# Fireworks Collaboration (P0 交付)

Tauri + Vue 3 + TypeScript 的桌面应用，用于“统一 Git 加速与传输控制”的分阶段落地验证。当前版本完成 P0：

## 🧩 P0 能力清单

- 通用伪 SNI HTTP 请求 API（http_fake_request）
	- 支持 Fake SNI 开关、重定向、完整 timing、body base64 返回
	- SAN 白名单强制校验；日志对 Authorization 自动脱敏
- 基础 Git Clone（基于 gitoxide）
	- 任务模型（创建/状态/进度/取消）；事件推送至前端
- 前端面板
	- HTTP Tester：便捷发起请求、Fake SNI/不安全验证开关（原型）、请求历史回填
	- Git 面板：输入仓库与目标目录、启动克隆、进度条、取消
	- 全局错误提示（脱敏）

详细技术方案见 `doc/TECH_DESIGN.md` 与 `doc/TECH_DESIGN_P0.md`。

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

- TLS 链验证不关闭（默认）
- SAN 白名单强制：仅允许 github.com 相关域（可在配置中调整）
- 伪 SNI：仅改变握手的 SNI，不削弱 CA 验证；如失败可切换关闭后再试
- 日志脱敏：Authorization 头默认记录为 `REDACTED`
- 原型开关：`tls.insecureSkipVerify=true` 仅用于联调，请勿在常规场景启用

## 🧭 快速导航

- Git 面板：`/git`
- HTTP 测试器：主页导航进入
- 手动验收脚本：`doc/MANUAL_TESTS.md`
- 设计文档：`doc/TECH_DESIGN.md`、`doc/TECH_DESIGN_P0.md`

## 🛠️ 开发者环境建议

- VS Code + Volar + Tauri + rust-analyzer

> 若需了解 Vue SFC `<script setup>` 的类型推导与 Volar Take Over 模式，可参考原模板文档：
> https://github.com/johnsoncodehk/volar/discussions/471
