# Mutation 测试基线指引

> 占位：提供使用 cargo-mutants 的基础说明，未在仓库中执行外部命令。

## 目标
借助 mutation testing 评估策略与回退相关纯函数测试充分度，首轮 kill rate 目标 ≥60%。

## 范围候选
- `TaskRegistry::apply_http_override`
- `TaskRegistry::apply_retry_override`
- `TaskRegistry::apply_tls_override`
- `TaskRegistry::decide_partial_fallback`

## 使用步骤（本地）
1. 安装工具：`cargo install cargo-mutants`
2. 运行：`cargo mutants --no-shuffle --in-place --timeout 15 --features ""`
3. 初次运行后查看 `mutants.out` 中的 survived 列表，针对存活变异添加属性测试或断言。

## 约定
- 不在 CI 默认流水线上运行（耗时）。
- 后续若需集成，可添加独立 workflow 并允许手动触发。

## 后续改进
- 添加对 `compute_retry_diff` 等 diff 逻辑的变异采样。
- 针对 event summary 生成逻辑添加 mutation。
