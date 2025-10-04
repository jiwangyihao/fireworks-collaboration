//! 任务模块集成测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

mod helpers; // Task helpers tests (from tasks_helpers.rs)
mod ip_pool_fault_injection;
mod ip_pool_manager;
mod task_registry_and_service;
mod unit_tests;
