use fireworks_collaboration_lib::core::config::loader::{load_or_init_at, save_at};
use fireworks_collaboration_lib::core::config::model::AppConfig;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> &'static Mutex<()> {
    static G: OnceLock<Mutex<()>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(()))
}

fn with_temp_cwd<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let _lock = test_guard().lock().unwrap();
    let old = std::env::current_dir().unwrap();
    let base =
        std::env::temp_dir().join(format!("fwc-p01-{}-{}", name, uuid::Uuid::new_v4()));
    fs::create_dir_all(&base).unwrap();
    std::env::set_current_dir(&base).unwrap();
    let res = f();
    std::env::set_current_dir(&old).unwrap();
    let _ = fs::remove_dir_all(&base);
    res
}

#[test]
fn test_load_or_init_creates_default_at_base() {
    with_temp_cwd("create-default", || {
        assert!(!std::path::Path::new("config/config.json").exists());
        let cfg =
            load_or_init_at(Path::new(".")).expect("should create default config at base");
        assert!(std::path::Path::new("config/config.json").exists());
        // 校验部分默认值
        assert!(cfg.http.fake_sni_enabled);
        assert!(cfg
            .tls
            .san_whitelist
            .iter()
            .any(|d| d.contains("github.com")));
        assert_eq!(cfg.logging.log_level, "info");
    });
}

#[test]
fn test_save_and_reload_roundtrip_at_base() {
    with_temp_cwd("save-reload", || {
        let mut cfg = AppConfig::default();
        cfg.http.fake_sni_enabled = false;
        cfg.http.max_redirects = 3;
        save_at(&cfg, Path::new(".")).expect("save should succeed");
        // 再次读取
        let loaded = load_or_init_at(Path::new(".")).expect("load should succeed");
        assert_eq!(loaded.http.fake_sni_enabled, false);
        assert_eq!(loaded.http.max_redirects, 3);
    });
}
