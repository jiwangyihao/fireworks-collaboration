#![allow(dead_code, unused_imports, unused_variables)]
//! 公共测试模块聚合（阶段 12.1 初版）
//!
//! 后续阶段会继续扩展：
//! - repo_factory: 复杂仓库拓扑构造
//! - event_assert: 事件匹配 DSL
//! - matrices: shallow / partial / retry 等
//!
//! 目前仅暴露基础 fixtures 与环境初始化。

pub mod fixtures;
pub mod test_env;
pub mod repo_factory;
pub mod git_helpers;
pub mod git_scenarios;
pub mod event_assert;
pub mod shallow_matrix; // 12.5: 浅克隆 / 深度矩阵
pub mod partial_filter_matrix; // 12.6: partial clone filter 矩阵
pub mod partial_filter_support; // 新模块
pub mod retry_matrix; // 12.9: push & retry 矩阵
pub mod http_override_stub; // 12.10: http override cases & stub
