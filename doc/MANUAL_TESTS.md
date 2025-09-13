# 手动测试脚本（P0.8 验收）

本文件用于帮助在本地对 P0 能力做一次完整的人工验收，覆盖 Fake SNI on/off、Git Clone 取消、日志脱敏与简单性能基线记录。

> 环境前提
> - Windows（PowerShell）
> - 已安装 pnpm 与 Rust 工具链（用于 dev 启动）
> - 网络可访问 GitHub（或至少不完全封锁 HTTPS 出站）

---

## 1. 启动与准备

- 安装依赖与启动 Dev（可在 VS Code 调试或命令行）：
  - 前端：pnpm install；
  - 运行应用：pnpm tauri dev（或通过 VS Code 任务/调试）。
- 打开应用后，确保首页可见导航，能进入“HTTP 测试器”和“Git 克隆”面板。

---

## 2. HTTP Tester：Fake SNI on/off 对比

目的：验证通用伪 SNI HTTP 请求 API 的工作情况，以及 SAN 白名单与脱敏日志。

步骤：
1) 打开“HTTP 测试器”面板。
2) 输入：
   - URL: https://github.com/
   - Method: GET
   - Headers: 可留空或添加 { "User-Agent": "P0Test" }
   - Body: 空
3) 在右侧策略开关中：
   - 关闭“不安全跳过证书验证”（应为默认关闭）。
   - 开启“Fake SNI”。
4) 点击发送：
   - 预期返回 status=200；
   - 结果区应显示 usedFakeSni=true；
   - Timing 字段包含 connectMs/tlsMs/firstByteMs/totalMs。
5) 关闭“Fake SNI”，再发送一次：
   - 预期返回 status=200；
   - usedFakeSni=false。
6) 将 Headers 中添加 Authorization: Bearer dummy-token，发送一次：
   - 观察应用控制台或文件日志（若开启），Authorization 应被记录为 REDACTED，不应出现明文 token。

白名单验证：
- 将 URL 改为 https://example.com/ 并发送：
  - 预期被拒绝（Verify: SAN whitelist mismatch ...）。

---

## 3. Git 克隆：启动与取消

目的：验证基础克隆成功与任务取消可用，并观测任务事件。

步骤：
1) 打开“Git 克隆”面板。
2) 填写：
   - 仓库 URL：例如 https://github.com/rust-lang/log
   - 目标目录：例如 C:/tmp/log（确保目录可写且不存在）。
3) 点击“开始克隆”，应看到：
   - 任务出现在列表中，状态从 Pending -> Running；
   - 进度条显示 phase/percent 动态变化。
4) 在克隆中途点击“取消”：
   - 任务状态应变为 Canceled；
   - 目标目录可能不完整，属预期。
5) 再次以一个新的空目录重试完整克隆：
   - 任务最终状态 Completed，目录含完整工作区。

---

## 4. 性能基线记录（参考）

目的：粗略记录系统 git 与应用克隆的耗时，作为后续优化的对照参考（无需严格）。

- 使用 PowerShell 测量系统 git：
  ```powershell
  Remove-Item -Recurse -Force C:/tmp/log  -ErrorAction SilentlyContinue
  $t = Measure-Command { git clone https://github.com/rust-lang/log C:/tmp/log }
  $t.TotalSeconds
  ```
- 使用应用进行一次相同目标路径的克隆，手工记录从点击“开始克隆”到状态变为 Completed 的时间（秒）。
- 目标：初期允许应用耗时 ≤ 系统 git 的 1.25 倍；后续优化再收紧。

注意：不同网络与磁盘状况会影响绝对数值，记录趋势即可。

---

## 5. 日志与安全校验

- 检查日志中是否存在 Authorization 明文（应为 REDACTED）。
- 白名单：对非 GitHub 域的请求应被拒绝。
- 关键路径无 panic/崩溃；若遇到，应附带最简复现步骤提交 Issue。

---

## 6. 常见问题（Troubleshooting）

- TLS 证书错误或握手失败：
  - 确认系统时间正确；
  - 确认未启用企业中间人代理；
  - 如需原型联调，可在“HTTP 测试器”中临时开启“不安全跳过证书验证”，但务必在完成验证后关闭。
- 克隆目标目录已存在：
  - 当前策略为直接失败，请选择一个全新空目录。
- Fake SNI 导致请求失败：
  - 这是可能的，P0 阶段不做自动回退；可手工关闭 Fake SNI 再试。

---

## 7. 验收勾选

- [ ] HTTP：github.com 请求 200，usedFakeSni on/off 均能成功。
- [ ] HTTP：白名单外域被拒绝。
- [ ] HTTP：Authorization 日志已脱敏。
- [ ] Git：一次取消任务后状态=Canceled。
- [ ] Git：一次完整克隆状态=Completed。
- [ ] 性能：记录了系统 git 与应用的参考时长。

完成以上手动项，并保证 `cargo test` 与 `pnpm test` 全绿，即可视为通过 P0.8 的“测试与校验”。