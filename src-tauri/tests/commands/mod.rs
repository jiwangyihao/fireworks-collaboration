//! Commands 测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// 命令测试模块
mod credential_commands_integration;
mod git_commands;
mod http_commands;
mod ip_pool_commands;
mod oauth_commands;
mod submodule_tests;
mod workspace_commands;
