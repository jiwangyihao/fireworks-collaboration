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
- Fake SNI 目标域需在 `http.fakeSniTargetHosts` 中显式配置，避免误改写非预期域。
- 伪 SNI：仅改变握手的 SNI，不削弱 CA 验证；如失败可切换关闭后再试。
- 日志脱敏：Authorization 头默认记录为 `REDACTED`。

## 🔐 P6.0 凭证存储与安全管理

P6.0 阶段提供凭证存储的基线架构，支持安全的凭证管理：

### 核心特性

- **凭证数据模型**：支持 host、username、password/token、过期时间等字段
- **存储抽象**：MemoryCredentialStore（内存存储，用于测试和临时会话）
- **自动安全**：
  - 日志自动脱敏（Display/Debug traits）
  - 序列化跳过密码字段
  - 过期凭证自动过滤
- **配置支持**：与主配置系统集成，支持从 config.json 加载
- **测试完整**：43 个测试（33 单元 + 10 集成），100% 通过

### 文档

- 📖 [快速入门（5分钟）](new-doc/CREDENTIAL_QUICKSTART.md) - 最小化配置和常见操作
- 📖 [使用示例](new-doc/CREDENTIAL_USAGE_EXAMPLES.md) - 完整代码示例（含 Tauri 集成）
- 📖 [错误处理指南](new-doc/CREDENTIAL_ERROR_HANDLING.md) - 每种错误的解决方案
- 📖 [故障排查](new-doc/CREDENTIAL_TROUBLESHOOTING.md) - 常见问题诊断
- 📖 [迁移指南](new-doc/CREDENTIAL_MIGRATION.md) - 版本迁移和外部系统集成
- 📖 [性能优化](new-doc/CREDENTIAL_PERFORMANCE.md) - 大规模场景优化
- 📖 [安全评估](new-doc/CREDENTIAL_SECURITY_ASSESSMENT.md) - 15 个威胁识别
- 📖 [加密设计](new-doc/CREDENTIAL_ENCRYPTION_DESIGN.md) - AES-256-GCM 方案
- 📖 [P6.0 完成报告](new-doc/P6.0_COMPLETION_REPORT.md) - 交付总结

### 下一步

- **P6.1**: 系统钥匙串集成（Windows Credential Manager、macOS Keychain）
- **P6.2**: 加密文件存储（AES-256-GCM + Argon2id）
- **P6.3**: 前端 UI 集成
- **P6.4**: 生命周期管理（自动清理、批量操作）
- **P6.5**: 安全审计与准入

---

## 🧭 快速导航

- Git 面板：`/git`
- HTTP 测试器：主页导航进入
- 手动验收脚本：`doc/MANUAL_TESTS.md`
- 设计文档：`doc/TECH_DESIGN.md`、`doc/TECH_DESIGN_P0.md`
 - P1 细化：`doc/TECH_DESIGN_P1.md`
- **凭证管理文档**：`new-doc/CREDENTIAL_QUICKSTART.md`（及上述文档列表）

## 🛠️ 开发者环境建议

- VS Code + Volar + Tauri + rust-analyzer

> 若需了解 Vue SFC `<script setup>` 的类型推导与 Volar Take Over 模式，可参考原模板文档：
> https://github.com/johnsoncodehk/volar/discussions/471

## 🚀 P2.3 任务级策略覆盖 (strategyOverride)

自 P2.3 起，`git_clone` / `git_fetch` / `git_push` 支持可选 `strategyOverride`，在“单个任务”范围内覆盖全局 HTTP / Retry 安全子集参数，不修改全局配置，也不影响其他并发任务：

支持字段：
- `http.followRedirects?: boolean`
- `http.maxRedirects?: number (<=20)`
- `retry.max?: number` / `retry.baseMs?: number` / `retry.factor?: number` / `retry.jitter?: boolean`

调用示例（前端）：

```ts
import { startGitClone } from './api/tasks';

await startGitClone('https://github.com/org/repo.git', 'D:/work/repo', {
	depth: 1,
	filter: 'blob:none',
	strategyOverride: {
		http: { followRedirects: false, maxRedirects: 0 },
		retry: { max: 3, baseMs: 400, factor: 2, jitter: true },
	},
});
```

策略覆盖相关提示通过 **结构化事件总线**（`StructuredEvent::Strategy` / `StructuredEvent::Policy`）发出，核心事件如下：

| 事件变体 | 含义 | 额外说明 |
|-----------|------|----------|
| `StrategyEvent::HttpApplied { follow, max_redirects }` | HTTP 覆盖生效 | 仅当实际改变跟随/跳转上限时发出 |
| `PolicyEvent::RetryApplied { code, changed }` | Retry 覆盖生效 | `code` 字段仍使用 `retry_strategy_override_applied`，同时返回变更字段列表 |
| `StrategyEvent::Conflict { message }` | 检测到互斥组合并已规范化 | 仅 `GitClone` 通过结构化事件广播；`GitPush` 保留信息级 `task://error` 提示；`GitFetch` 当前仅规范化并记录日志 |
| `StrategyEvent::IgnoredFields { top_level, nested }` | 忽略未知字段 | `GitClone`/`GitFetch`/`GitPush` 均会在集合非空时发射一次 |
| `StrategyEvent::Summary { applied_codes, http_*, retry_* }` | 汇总最终策略与差异 | `applied_codes` 中会列出 `http_strategy_override_applied` / `retry_strategy_override_applied` 字符串 |

若需要在前端/UI 中消费这些信号，可通过 `events::structured::set_test_event_bus`/`MemoryEventBus` 观察，或在应用启动时注册自定义事件总线实现。

回退策略：删除对应 `publish_global(StructuredEvent::...)` 分支即可静默这些提示；逻辑仍会按覆盖后的值执行。

### 🔧 环境变量 (P2 实装)

| 变量 | 值 | 作用 | 默认 |
|------|----|------|------|
| `FWC_PARTIAL_FILTER_SUPPORTED` | `1`/其它 | 声明运行环境支持 Git partial clone filter；为 `1` 时不触发回退提示事件 | 未设置=不支持 |
| `FWC_PARTIAL_FILTER_CAPABLE` | `1`/其它 | 与 `FWC_PARTIAL_FILTER_SUPPORTED` 行为相同的兼容别名，便于旧脚本沿用 | 未设置=不支持 |

### 🧾 汇总事件：`StrategyEvent::Summary`

汇总事件示例（结构化事件 JSON 片段）：

```jsonc
{
	"type": "strategy",
	"data": {
		"Summary": {
			"id": "<task-id>",
			"kind": "GitClone",
			"http_follow": true,
			"http_max": 3,
			"retry_max": 5,
			"retry_base_ms": 200,
			"retry_factor": 1.5,
			"retry_jitter": true,
			"applied_codes": [
				"http_strategy_override_applied",
				"retry_strategy_override_applied"
			],
			"filter_requested": false
		}
	}
}
```

`applied_codes` 即为原信息事件中的 code 字符串，便于 UI/日志继续高亮“覆盖生效”状态。


