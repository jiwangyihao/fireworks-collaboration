//! 代理模块集成测试

#[path = "../common/mod.rs"]
pub(crate) mod common;

mod config;
mod detector;
mod events;
mod http_connector;
mod manager;
mod socks5_connector;
mod state;
mod unit_tests;
