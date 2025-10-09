use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fireworks_collaboration_lib::app::commands::ip_pool::{
    ip_pool_clear_auto_disabled,
    ip_pool_get_snapshot,
    ip_pool_pick_best,
    ip_pool_request_refresh,
    ip_pool_update_config,
};
use fireworks_collaboration_lib::app::types::{ConfigBaseDir, SharedConfig, SharedIpPool};
use fireworks_collaboration_lib::core::config::{
    loader::{self, testing as loader_testing},
    model::AppConfig,
};
use fireworks_collaboration_lib::core::ip_pool::{
    config::{self as ip_pool_cfg, IpPoolFileConfig, IpPoolRuntimeConfig, UserStaticIp},
    EffectiveIpPoolConfig, IpPool,
};
use serde_json::Value;
use tauri::State;
use tempfile::TempDir;

fn leak_state<T: 'static>(value: T) -> State<'static, T> {
    let leaked: &'static T = Box::leak(Box::new(value));
    State::from(leaked)
}

struct IpPoolCommandTestEnv {
    base_dir: TempDir,
    shared_config: SharedConfig,
    shared_pool: SharedIpPool,
}

impl IpPoolCommandTestEnv {
    fn new() -> Self {
        let base_dir = tempfile::tempdir().expect("create temp base dir");
        loader_testing::override_global_base_dir(base_dir.path());

        let runtime_cfg = IpPoolRuntimeConfig::default();
        let file_cfg = IpPoolFileConfig::default();
        let effective = EffectiveIpPoolConfig::from_parts(runtime_cfg.clone(), file_cfg.clone());
        let shared_pool: SharedIpPool = Arc::new(Mutex::new(IpPool::new(effective)));

        let mut app_cfg = AppConfig::default();
        app_cfg.ip_pool = runtime_cfg.clone();
        loader::save_at(&app_cfg, base_dir.path()).expect("write config.json");
        ip_pool_cfg::save_file_at(&file_cfg, base_dir.path()).expect("write ip-config.json");
        let shared_config: SharedConfig = Arc::new(Mutex::new(app_cfg.clone()));

        Self {
            base_dir,
            shared_config,
            shared_pool,
        }
    }

    fn pool_state(&self) -> State<'static, SharedIpPool> {
        leak_state(self.shared_pool.clone())
    }

    fn config_state(&self) -> State<'static, SharedConfig> {
        leak_state(self.shared_config.clone())
    }

    fn base_state(&self) -> State<'static, ConfigBaseDir> {
        leak_state(self.base_dir.path().to_path_buf())
    }

    fn config_path(&self) -> PathBuf {
        self.base_dir.path().join("config").join("config.json")
    }

    fn ip_config_path(&self) -> PathBuf {
        self.base_dir
            .path()
            .join("config")
            .join("ip-config.json")
    }

    fn current_runtime(&self) -> IpPoolRuntimeConfig {
        self.shared_config
            .lock()
            .expect("lock shared config")
            .ip_pool
            .clone()
    }
}

impl Drop for IpPoolCommandTestEnv {
    fn drop(&mut self) {
        loader_testing::clear_global_base_dir();
    }
}

#[tokio::test]
async fn get_snapshot_returns_default_state() {
    let env = IpPoolCommandTestEnv::new();

    let snapshot = ip_pool_get_snapshot(env.pool_state()).await.expect("snapshot");

    assert!(snapshot.enabled);
    assert!(!snapshot.preheater_active);
    assert_eq!(snapshot.cache_entries.len(), 0);

    let runtime_cfg = env.current_runtime();
    assert_eq!(snapshot.runtime.enabled, runtime_cfg.enabled);
    assert_eq!(snapshot.runtime.max_parallel_probes, runtime_cfg.max_parallel_probes);
}

#[tokio::test]
async fn update_config_persists_and_updates_pool() {
    let env = IpPoolCommandTestEnv::new();

    let mut runtime_update = env.current_runtime();
    runtime_update.enabled = true;
    runtime_update.max_parallel_probes = 16;
    runtime_update.history_path = Some("cache/history.json".to_string());
    runtime_update.failure_threshold = 6;
    runtime_update.failure_rate_threshold = 0.35;

    let mut file_update = IpPoolFileConfig::default();
    file_update.score_ttl_seconds = 900;
    file_update.user_static = vec![UserStaticIp {
        host: "example.com".into(),
        ip: "203.0.113.10".into(),
        ports: vec![443, 8443],
    }];
    file_update.blacklist = vec!["10.0.0.0/8".into()];
    file_update.whitelist = vec!["203.0.113.0/24".into()];

    let snapshot = ip_pool_update_config(
        runtime_update.clone(),
        file_update.clone(),
        env.config_state(),
        env.base_state(),
        env.pool_state(),
    )
    .await
    .expect("update config");

    assert!(snapshot.enabled);
    assert_eq!(snapshot.runtime.max_parallel_probes, 16);
    assert_eq!(snapshot.runtime.history_path, Some("cache/history.json".into()));
    assert_eq!(snapshot.file.score_ttl_seconds, 900);
    assert_eq!(snapshot.file.user_static.len(), 1);

    {
        let guard = env
            .shared_pool
            .lock()
            .expect("lock shared pool after update");
        assert!(guard.runtime_config().enabled);
        assert_eq!(guard.runtime_config().failure_threshold, 6);
        assert_eq!(guard.runtime_config().history_path, Some("cache/history.json".into()));
    }

    let cfg_data = std::fs::read_to_string(env.config_path()).expect("read config.json");
    let cfg_json: Value = serde_json::from_str(&cfg_data).expect("parse config");
    assert_eq!(cfg_json["ipPool"]["enabled"], Value::Bool(true));

    let ip_data = std::fs::read_to_string(env.ip_config_path()).expect("read ip-config.json");
    let ip_json: Value = serde_json::from_str(&ip_data).expect("parse ip-config");
    assert_eq!(ip_json["scoreTtlSeconds"], Value::from(900));
    assert_eq!(ip_json["userStatic"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn request_refresh_returns_false_without_preheater() {
    let env = IpPoolCommandTestEnv::new();

    let accepted = ip_pool_request_refresh(env.pool_state())
        .await
        .expect("request refresh");

    assert!(!accepted);
}

#[tokio::test]
async fn clear_auto_disabled_resets_flag() {
    let env = IpPoolCommandTestEnv::new();

    {
        let pool = env.shared_pool.clone();
        let guard = pool.lock().expect("lock pool for disable");
        guard.set_auto_disabled("test", 30_000);
    }

    let cleared = ip_pool_clear_auto_disabled(env.pool_state())
        .await
        .expect("clear auto disabled");

    assert!(cleared);

    let guard = env
        .shared_pool
        .lock()
        .expect("lock pool after clear");
    assert!(guard.auto_disabled_until().is_none());
}

#[tokio::test]
async fn pick_best_falls_back_when_disabled() {
    let env = IpPoolCommandTestEnv::new();

    let selection = ip_pool_pick_best("github.com".into(), 443, env.pool_state())
        .await
        .expect("pick best");

    assert_eq!(selection.strategy, "system");
    assert!(!selection.cache_hit);
    assert!(selection.selected.is_none());
}
