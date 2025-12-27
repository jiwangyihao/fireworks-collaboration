//! 任务模块集成测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

mod credential_file_store;
mod git_conflict;
mod git_operations;
mod metrics_aggregate;
mod proxy_config;
mod submodule_ops;
mod workspace_batch;
mod workspace_model;

mod helpers; // Task helpers tests (from tasks_helpers.rs)
mod ip_pool_fault_injection;
mod ip_pool_manager;
mod ip_pool_mock;
mod task_registry_and_service;
mod unit_tests;
