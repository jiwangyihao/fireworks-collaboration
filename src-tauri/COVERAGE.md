# Rust 覆盖率使用说明

本项目使用 `cargo-llvm-cov` + `cargo-nextest` 生成覆盖率报告。

## 快速开始

```bash
# 安装依赖工具（一次性）
cargo install cargo-llvm-cov --locked
cargo install cargo-nextest --version 0.9.114 --locked  # 需要 rustc 1.88+

# 使用 nextest 运行覆盖率测试（推荐，速度快 55%）
cargo llvm-cov nextest --lcov --output-path lcov.info

# 生成 HTML 报告并打开浏览器
cargo llvm-cov nextest --open
```

## 覆盖率目标

- 总体行覆盖 (line) >= 75%
- 关键策略/分类模块行覆盖 >= 90%

## Windows 测试环境

Windows 平台使用 `tauri-core` 特性避免 DLL 冲突（`0xc0000139`）。该特性已设为默认。

**推荐做法：**

```powershell
# 使用专用脚本（自动安装 nextest）
./scripts/test_windows.ps1

# 或直接使用 nextest
cargo nextest run

# 运行特定测试
cargo nextest run --test commands

# 覆盖率测试
cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
```

## nextest 配置

项目配置文件位于 `src-tauri/.config/nextest.toml`：

- `fail-fast = false`：失败时继续运行其他测试
- `retries = 1` (CI)：CI 环境自动重试失败测试

## 前端覆盖率

```bash
pnpm test:cov
# 结果在 coverage/ 目录
```

## CI 说明

CI 中使用 `taiki-e/install-action` 安装工具链，并以 `cargo llvm-cov nextest` 生成报告。
详见 `.github/workflows/coverage.yml`。

## 性能对比

| 方法                     | 耗时     |
| ------------------------ | -------- |
| `cargo llvm-cov` (标准)  | ~11 分钟 |
| `cargo llvm-cov nextest` | ~5 分钟  |

nextest 模式每个测试独立进程，隔离性更好，速度更快。
