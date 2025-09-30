use once_cell::sync::OnceCell;
use std::sync::Weak;

use crate::core::ip_pool::manager::IpPool;

/// 全局 IP 池弱引用，用于 preheat 回退机制
pub static IP_POOL_GLOBAL: OnceCell<Weak<IpPool>> = OnceCell::new();
pub mod config;
pub mod git;
pub mod http;
pub mod ip_pool;
pub mod tasks;
pub mod tls;
