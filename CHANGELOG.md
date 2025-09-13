# Changelog

## v0.1.1-MP0.4 (2025-09-14)

完成 MP0.4：从 gitoxide/gix 完整迁移到 git2-rs，并清理旧实现
- 后端 Git 实现：统一使用 git2-rs（libgit2 绑定）完成 clone/fetch；
- 任务/事件：保持命令签名与 `task://state|progress` 事件兼容；
- 取消与错误：协作式取消生效；错误分类 Network/Tls/Verify/Auth/Protocol/Cancel/Internal；
- 清理：移除 gix 与 gix-transport 依赖，删除旧的 clone/fetch 与进度桥接模块；移除构建特性开关；
- 测试：Rust 与前端 75 项测试全部通过。

## v0.1.0-P0 (2025-09-13)

P0 初始交付：
- 通用伪 SNI HTTP 请求 API（http_fake_request）
  - Fake SNI 开关、重定向、timing、body base64、Authorization 脱敏
  - SAN 白名单强制校验
- 基础 Git Clone（gitoxide）
  - 任务模型（创建/状态/进度/取消）与事件
- 前端面板
  - HTTP Tester（历史回填、策略开关）、Git 面板（进度/取消）、全局错误提示
- 文档与测试
  - 技术设计（整合版 + P0 细化）、手动验收脚本（MANUAL_TESTS）
  - Rust/Vitest 全部测试通过

已知限制与后续计划：
- 未接入代理与 IP 优选（Roadmap P4-P5）
- Git 伪 SNI 与自动回退（Roadmap P3）
- SPKI Pin & 指纹事件（Roadmap P7）
- 指标面板（Roadmap P9），流式响应/HTTP2（Roadmap P10）
