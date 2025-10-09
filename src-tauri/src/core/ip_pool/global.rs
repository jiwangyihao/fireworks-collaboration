use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::sync::oneshot;

use crate::core::config::{loader, model::AppConfig};

use super::{builder, EffectiveIpPoolConfig, IpOutcome, IpPool, IpSelection};

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

// ==== 异步选择桥接 ==== //
enum AsyncRequest {
    Pick {
        host: String,
        port: u16,
        respond: oneshot::Sender<IpSelection>,
    },
    /// Fire-and-forget outcome report
    ReportOutcome {
        selection: IpSelection,
        outcome: IpOutcome,
    },
}

fn async_bridge_sender() -> &'static Mutex<Option<Sender<AsyncRequest>>> {
    static S: OnceLock<Mutex<Option<Sender<AsyncRequest>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

fn spawn_async_bridge_if_needed() {
    let mut guard = async_bridge_sender().lock().expect("async bridge mutex");
    if guard.is_some() {
        return;
    }
    let (tx, rx): (Sender<AsyncRequest>, Receiver<AsyncRequest>) = mpsc::channel();
    *guard = Some(tx);

    // 后台线程：单线程 tokio runtime，避免在现有 runtime 内部构建多线程 runtime 的限制。
    thread::Builder::new()
        .name("ip-pool-async-bridge".into())
        .spawn(move || {
            let rt = RuntimeBuilder::new_current_thread()
                .enable_time()
                .enable_io()
                .build();
            if rt.is_err() {
                tracing::error!(target="ip_pool", "failed to build async bridge runtime");
                return;
            }
            let rt = rt.unwrap();
            rt.block_on(async move {
                while let Ok(req) = rx.recv() {
                    match req {
                        AsyncRequest::Pick { host, port, respond } => {
                            let pool_arc = obtain_global_pool();
                            let selection = if let Ok(pool_guard) = pool_arc.lock() {
                                // 在桥接线程内安全调用异步方法
                                pool_guard.pick_best(&host, port).await
                            } else {
                                IpSelection::system_default(host.clone(), port)
                            };
                            let _ = respond.send(selection);
                        }
                        AsyncRequest::ReportOutcome { selection, outcome } => {
                            // Fire-and-forget reporting back to ip pool
                            if let Ok(pool_guard) = obtain_global_pool().lock() {
                                // report_outcome will internally ignore system default selections
                                pool_guard.report_outcome(&selection, outcome);
                                // Also report candidate-level outcome to update circuit breaker
                                if let Some(stat) = selection.selected() {
                                    pool_guard.report_candidate_outcome(selection.host(), selection.port(), stat, outcome);
                                }
                            } else {
                                tracing::warn!(target = "ip_pool", "ip pool mutex poisoned while reporting outcome via async bridge");
                            }
                        }
                    }
                }
            });
        })
        .expect("spawn ip pool async bridge thread");
}

/// Fire-and-forget API for reporting an outcome from async contexts.
pub fn report_outcome_async(selection: IpSelection, outcome: IpOutcome) {
    spawn_async_bridge_if_needed();
    if let Ok(guard) = async_bridge_sender().lock() {
        if let Some(sender) = guard.clone() {
            let _ = sender.send(AsyncRequest::ReportOutcome { selection, outcome });
        }
    }
}

/// 异步获取最佳 IP 选择；在 Tokio 运行时内可安全调用。
pub async fn pick_best_async(host: &str, port: u16) -> IpSelection {
    spawn_async_bridge_if_needed();
    let sender_opt = async_bridge_sender().lock().ok().and_then(|g| g.clone());
    if let Some(sender) = sender_opt {
        let (tx, rx) = oneshot::channel();
        let req = AsyncRequest::Pick {
            host: host.to_string(),
            port,
            respond: tx,
        };
        if sender.send(req).is_err() {
            tracing::warn!(
                target = "ip_pool",
                host,
                port,
                "async bridge send failed; fallback system"
            );
            return IpSelection::system_default(host.to_string(), port);
        }
        match rx.await {
            Ok(sel) => sel,
            Err(_) => {
                tracing::warn!(
                    target = "ip_pool",
                    host,
                    port,
                    "async bridge recv failed; fallback system"
                );
                IpSelection::system_default(host.to_string(), port)
            }
        }
    } else {
        tracing::warn!(
            target = "ip_pool",
            host,
            port,
            "async bridge unavailable; fallback system"
        );
        IpSelection::system_default(host.to_string(), port)
    }
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
