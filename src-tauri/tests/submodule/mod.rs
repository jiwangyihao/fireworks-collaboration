//! 子模块测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

mod model_tests;
mod operations_tests;

use common::test_env::init_test_env;

#[ctor::ctor]
fn __init_env() {
    init_test_env();
}
