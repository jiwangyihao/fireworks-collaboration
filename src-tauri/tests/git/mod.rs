//! Git 模块集成测试

// 为所有子模块提供 common 访问
#[path = "../common/mod.rs"]
pub(crate) mod common;

mod git_add_and_commit;
mod git_basic_operations;
mod git_branch_and_checkout;
mod git_clone_shallow_and_depth;
mod git_credential_autofill;
mod git_fetch_core_and_shallow;
mod git_preconditions_and_cancel;
mod git_push_and_retry;
mod git_strategy_and_override;
mod git_tag_and_remote;
mod opts;
mod transport;
