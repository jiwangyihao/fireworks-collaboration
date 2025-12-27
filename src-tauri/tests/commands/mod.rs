//! Commands 测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}

// 新增的纯函数测试模块
mod git_commands;
mod http_commands;
mod oauth_commands;
mod workspace_commands;

// 以下模块需要更新 API 调用后再启用
// mod credential_commands_integration;
// mod ip_pool_commands;
// mod submodule_tests;
