# Rust 覆盖率使用说明

本项目计划在 T4 集成覆盖率门槛。当前手动步骤（需要已安装 `llvm-tools-preview` 组件）：

```bash
# 安装依赖工具（一次性）
cargo install cargo-llvm-cov --locked

# 运行全部测试并生成报告
cargo llvm-cov --manifest-path src-tauri/Cargo.toml --workspace --lcov --output-path lcov.info

# 生成 HTML 浏览
cargo llvm-cov --manifest-path src-tauri/Cargo.toml --workspace --open
```

后续将在 CI 中：

- 设定行覆盖 (line) >= 75%
- 关键策略/分类模块行覆盖 >= 90%

前端：

```bash
pnpm test:cov
# 结果在 coverage/ 目录
```

集成门槛前的本地 Smoke：

```bash
cargo llvm-cov --manifest-path src-tauri/Cargo.toml --no-report > NUL 2>&1 || echo "(optional)"
```

---

## Windows 测试环境

Windows 平台上直接运行 `cargo test` 可能因 DLL 冲突（`0xc0000139`）导致 `git` 等测试流程崩溃。该冲突主要由 Tauri 的 WebView2 组件与 Git 依赖库在加载系统 DLL 时产生冲突引起。

### 解决方案：`tauri-core` 特性

本项目引入了 `tauri-core` 特性，专门用于无 UI 环境下的核心逻辑测试。该特性通过禁用 Tauri 的默认 UI 依赖（如 `wry`）来彻底消除 DLL 冲突。

**注意：** 我们已将 `tauri-core` 设为 `Cargo.toml` 的默认特性，因此可以直接运行测试。

**推荐做法：使用专用脚本**

```powershell
# 该脚本会自动处理 PATH 环境以增强稳定性
./scripts/test_windows.ps1
```

**手动运行测试：**

```powershell
# 无需额外参数，默认即为 tauri-core 模式
cargo test --test <test_name>
```

**生成覆盖率报告：**

```powershell
# 仍需 --ignore-run-fail 如果有部分测试不稳定
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

> **运行完整应用 (UI)**: 如果需要运行带界面的应用，请使用 `pnpm tauri dev` 或 `cargo run --features tauri-app`。

### 已知限制

- **无 UI 交互**: 在 `tauri-core` 模式下，所有涉及 `tauri::Window` 或 WebView2 的操作将被跳过或 Mock。
- **环境依赖**: 需要预先安装 `cmake`（用于静态链接底层库）。
- **PATH 隔离**: 脚本 `test_windows.ps1` 会尝试隔离系统 PATH 以进一步提高稳定性，但仍需确保 Git 等工具在 PATH 中可用（脚本已做自动处理）。
