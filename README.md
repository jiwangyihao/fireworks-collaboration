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

- TLS 链验证不关闭（默认）
- SAN 白名单强制：仅允许 github.com 相关域（可在配置中调整）
- 伪 SNI：仅改变握手的 SNI，不削弱 CA 验证；如失败可切换关闭后再试
- 日志脱敏：Authorization 头默认记录为 `REDACTED`
- TLS 校验开关可组合：
	- `tls.insecureSkipVerify`（默认 false）：跳过默认证书链与主机名校验；
	- `tls.skipSanWhitelist`（默认 false）：跳过自定义 SAN 白名单校验；
	- 组合语义：
		- 默认（两者均 false）：链验证 + 主机名 + 白名单（推荐）。
		- 仅开启 insecureSkipVerify：保留“仅白名单”校验（Whitelist-only）。
		- 同时开启两者：完全不做校验（极不安全，仅临时联调）。

## 🧭 快速导航

- Git 面板：`/git`
- HTTP 测试器：主页导航进入
- 手动验收脚本：`doc/MANUAL_TESTS.md`
- 设计文档：`doc/TECH_DESIGN.md`、`doc/TECH_DESIGN_P0.md`
 - P1 细化：`doc/TECH_DESIGN_P1.md`

## 🛠️ 开发者环境建议

- VS Code + Volar + Tauri + rust-analyzer

> 若需了解 Vue SFC `<script setup>` 的类型推导与 Volar Take Over 模式，可参考原模板文档：
> https://github.com/johnsoncodehk/volar/discussions/471

## 🚀 P2.3 任务级策略覆盖 (strategyOverride)

自 P2.3 起，`git_clone` / `git_fetch` / `git_push` 支持可选 `strategyOverride`，在“单个任务”范围内覆盖全局 HTTP / TLS / Retry 安全子集参数，不修改全局配置，也不影响其他并发任务：

支持字段：
- `http.followRedirects?: boolean`
- `http.maxRedirects?: number (<=20)`
- `tls.insecureSkipVerify?: boolean`
- `tls.skipSanWhitelist?: boolean`
- `retry.max?: number` / `retry.baseMs?: number` / `retry.factor?: number` / `retry.jitter?: boolean`

调用示例（前端）：

```ts
import { startGitClone } from './api/tasks';

await startGitClone('https://github.com/org/repo.git', 'D:/work/repo', {
	depth: 1,
	filter: 'blob:none',
	strategyOverride: {
		http: { followRedirects: false, maxRedirects: 0 },
		tls: { insecureSkipVerify: false, skipSanWhitelist: false },
		retry: { max: 3, baseMs: 400, factor: 2, jitter: true },
	},
});
```

信息事件（复用 `task://error` 通道, `category=Protocol`）在值发生实际变化时最多各出现一次：

| code | 场景 |
|------|------|
| `http_strategy_override_applied` | HTTP 覆盖生效 |
| `tls_strategy_override_applied` | TLS 覆盖生效 |
| `retry_strategy_override_applied` | Retry 覆盖生效 |
| `strategy_override_conflict` | 发现互斥组合并已规范化（如 follow=false & max>0 → max=0） |
| `strategy_override_ignored_fields` | 含未知字段被忽略 |

这些提示事件不会导致任务失败，可用于 UI 中“提示”标签展示；真正的失败仍是 `state=failed`。

前端实现要点：
- 事件监听已将 `code` 写入 `tasks` store 的 `lastErrorById[taskId].code`，供上层 UI 过滤。
- `startGitFetch` 兼容旧写法 `startGitFetch(repo,dest,"branches")`；推荐改用对象 `{ preset: "branches" }` 以便同时传递 `depth/filter/strategyOverride`。
- 多个覆盖相关事件会覆盖 code，但若后续 informational 事件不带 `retriedTimes`，会保留之前的重试次数值，避免丢失重试上下文。

回退策略：删除事件分支（仅日志）或移除对应 `apply_*_override` 调用即可恢复旧行为。

### 🔧 环境变量 (P2 新增)

| 变量 | 值 | 作用 | 默认 |
|------|----|------|------|
| `FWC_PARTIAL_FILTER_SUPPORTED` | `1`/其它 | 声明运行环境支持 Git partial clone filter；为 `1` 时不触发回退提示事件 | 未设置=不支持 |
| `FWC_STRATEGY_APPLIED_EVENTS` | `0` / 其它 | 是否发送独立 `*_strategy_override_applied` 信息事件；为 `0` 时仅保留 summary 汇总 | 未设置=发送 |

### 🧾 汇总事件：`strategy_override_summary`

为减少前端多事件聚合的复杂度，Clone/Fetch/Push 在解析与应用策略覆盖后会发送一次聚合事件（仍走 `task://error` 通道，`category=Protocol` 信息级）：

`code = strategy_override_summary`，`message` 字段是一个 JSON 字符串，示例：

```jsonc
{
	"taskId": "<uuid>",
	"kind": "GitClone",
	"code": "strategy_override_summary",
	"category": "Protocol",
	"message": "{\n  \"taskId\":\"<uuid>\",\n  \"kind\":\"GitClone\",\n  \"http\":{\"follow\":true,\"maxRedirects\":3},\n  \"retry\":{\"max\":5,\"baseMs\":200,\"factor\":1.5,\"jitter\":0.1},\n  \"tls\":{\"insecureSkipVerify\":false,\"skipSanWhitelist\":false},\n  \"appliedCodes\":[\"http_strategy_override_applied\",\"retry_strategy_override_applied\"],\n  \"filterRequested\": false\n}"
}
```

前端可：
1. 监听一次 summary 即得所有最终生效值；
2. 若 `FWC_STRATEGY_APPLIED_EVENTS=0`，独立 applied 事件不会出现，但 `appliedCodes` 仍列出；
3. 可用 `appliedCodes` 列表判断 UI 上是否需要高亮“有改写”。


