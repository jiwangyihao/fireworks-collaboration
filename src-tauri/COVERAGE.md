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

Windows 平台上直接运行 `cargo test` 可能因 DLL 冲突（`0xc0000139`）导致 `git` 等测试套件崩溃。

### 原因

`git2-rs` 依赖的 `libgit2`/`zlib` 等库可能加载到与 Git for Windows 等工具冲突的系统 DLL。

### 解决方案

**方法 A: 使用专用脚本（推荐）**

```powershell
# 运行所有测试
./scripts/test_windows.ps1

# 运行特定测试套件
./scripts/test_windows.ps1 -TestName commands
./scripts/test_windows.ps1 -TestName credential

# 过滤到特定测试函数
./scripts/test_windows.ps1 -TestName git -Filter my_test_function
```

该脚本会自动隔离 PATH 环境，确保测试二进制文件在干净环境中运行。

**方法 B: 手动隔离运行**

```powershell
# 1. 构建但不运行
cargo test --test git --no-run

# 2. 找到最新的测试 exe
$exe = Get-ChildItem target/debug/deps/git-*.exe | Sort-Object LastWriteTime -Descending | Select-Object -First 1

# 3. 设置干净 PATH 并运行
$env:PATH = "C:\Windows\System32;C:\Windows"
& $exe.FullName --nocapture
```

### 已知限制

- 需要预先安装 `cmake`（用于静态链接 `zlib-ng`）。
- 某些测试依赖系统上的 `git` CLI，在纯净 PATH 下会失败（这是预期行为，可在 CI 补充）。
