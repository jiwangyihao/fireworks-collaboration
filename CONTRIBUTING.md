# Contributing Guide

感谢关注本项目！以下是快速贡献说明（P0 阶段简版）。

## 开发环境
- Windows / PowerShell
- pnpm（包管理）
- Rust 工具链（Tauri 后端）
- VS Code 插件：Volar、Tauri、rust-analyzer（推荐）

## 运行与测试
```powershell
# 安装依赖
pnpm install

# 启动 Web（仅前端）
pnpm dev

# 启动桌面应用（Tauri）
pnpm tauri dev

# 运行测试
pnpm test
powershell -NoProfile -ExecutionPolicy Bypass -Command "cd '$PWD/src-tauri'; cargo test --quiet"
```

## 代码风格
- 前端：TypeScript + Vue 3，遵循现有 ESLint/Prettier 设定（已内置 prettier）。
- 后端：Rust，保持 `clippy` 友好与模块内自测。
- 命名与结构：与 `doc/TECH_DESIGN_P0.md` 模块划分一致（tasks/http/tls/git/config 等）。

## 提交规范
- 语义化简要前缀：`feat|fix|docs|test|chore|refactor(scope): message`
- 一次提交聚焦单一主题；避免混合格式化与逻辑改动。
- 如改动公共接口（Tauri command/事件负载），请在 PR 中说明并同步前端。

## PR 指引
- 勾选/说明影响的模块与测试范围。
- 新增公共行为时，请附最小单元测试与/或更新手动验收脚本。
- 链接相关设计章节（`doc/TECH_DESIGN*.md`）。

## 安全注意事项
- 不要将真实 Token 写入样例/日志；`Authorization` 会自动脱敏为 `REDACTED`。
- 默认开启 SAN 白名单校验；TLS 校验开关已拆分：
	- `insecureSkipVerify` 跳过默认证书链与主机名校验；
	- `skipSanWhitelist` 跳过白名单校验；
	- 两者均不开启为推荐模式；若仅开启 `insecureSkipVerify`，仍保留“仅白名单”保护；两者同时开启则完全不校验（仅临时联调）。

谢谢你的贡献！