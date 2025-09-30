use std::sync::{Arc, Mutex, OnceLock};

use crate::core::config::{loader, model::AppConfig};

use super::{builder, EffectiveIpPoolConfig, IpPool};

fn storage() -> &'static Mutex<Option<Arc<Mutex<IpPool>>>> {
    static STORAGE: OnceLock<Mutex<Option<Arc<Mutex<IpPool>>>>> = OnceLock::new();
    STORAGE.get_or_init(|| Mutex::new(None))
}

fn init_default_pool() -> Arc<Mutex<IpPool>> {
    let app_cfg = loader::load_or_init().unwrap_or_else(|err| {
        tracing::error!(target = "ip_pool", error = %err, "load config for ip pool failed; using defaults");
        AppConfig::default()
    });
    let effective = builder::load_effective_config(&app_cfg).unwrap_or_else(|err| {
        tracing::error!(target = "ip_pool", error = %err, "load ip pool config failed; using defaults");
        EffectiveIpPoolConfig::default()
    });
    Arc::new(Mutex::new(IpPool::new(effective)))
}

/// 获取全局共享的 IP 池实例；若未初始化则按当前配置创建默认实例。
pub fn obtain_global_pool() -> Arc<Mutex<IpPool>> {
    let mut guard = storage().lock().expect("global ip pool mutex poisoned");
    if let Some(existing) = guard.as_ref() {
        return Arc::clone(existing);
    }
    let pool = init_default_pool();
    *guard = Some(Arc::clone(&pool));
    pool
}

/// 设置全局共享的 IP 池实例，供外部（如测试或应用启动）覆盖。
pub fn set_global_pool(pool: Arc<Mutex<IpPool>>) {
    let mut guard = storage().lock().expect("global ip pool mutex poisoned");
    *guard = Some(pool);
}

#[cfg(any(test, not(feature = "tauri-app")))]
pub(crate) fn reset_global_pool_internal() {
    if let Ok(mut guard) = storage().lock() {
        *guard = None;
    }
}

#[cfg(test)]
pub(crate) fn test_reset_global_pool() {
    reset_global_pool_internal();
}

#[cfg(not(feature = "tauri-app"))]
pub mod testing {
    //! Integration-test helper APIs to adjust global IP pool state.
    pub fn reset_global_pool() {
        super::reset_global_pool_internal();
    }
}
