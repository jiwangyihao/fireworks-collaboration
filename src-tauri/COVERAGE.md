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
