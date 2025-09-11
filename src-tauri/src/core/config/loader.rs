use std::{fs, io::Write, path::{Path, PathBuf}};
use anyhow::{Context, Result};

use super::model::AppConfig;

fn join_default_path(base: &Path) -> PathBuf {
    let mut p = base.to_path_buf();
    p.push("config");
    p.push("config.json");
    p
}

fn config_path() -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    join_default_path(&base)
}

pub fn load_or_init() -> Result<AppConfig> {
    load_or_init_at_path(&config_path())
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    save_at_path(cfg, &config_path())
}

pub fn load_or_init_at(base_dir: &Path) -> Result<AppConfig> {
    let path = join_default_path(base_dir);
    load_or_init_at_path(&path)
}

pub fn save_at(cfg: &AppConfig, base_dir: &Path) -> Result<()> {
    let path = join_default_path(base_dir);
    save_at_path(cfg, &path)
}

fn load_or_init_at_path(path: &Path) -> Result<AppConfig> {
    if path.exists() {
        let data = fs::read(path).with_context(|| format!("read config: {}", path.display()))?;
        let cfg: AppConfig = serde_json::from_slice(&data).context("parse config json")?;
        Ok(cfg)
    } else {
        let cfg = AppConfig::default();
        save_at_path(&cfg, path)?;
        Ok(cfg)
    }
}

fn save_at_path(cfg: &AppConfig, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() { fs::create_dir_all(dir).ok(); }
    let json = serde_json::to_string_pretty(cfg).context("serialize config")?;
    let mut f = fs::File::create(path).with_context(|| format!("create config: {}", path.display()))?;
    f.write_all(json.as_bytes()).context("write config")?;
    tracing::info!(target = "config", path = %path.display(), "config saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> &'static Mutex<()> {
        static G: OnceLock<Mutex<()> > = OnceLock::new();
        G.get_or_init(|| Mutex::new(()))
    }

    fn with_temp_cwd<T>(name: &str, f: impl FnOnce() -> T) -> T {
        let _lock = test_guard().lock().unwrap();
        let old = std::env::current_dir().unwrap();
        let base = std::env::temp_dir().join(format!("fwc-p01-{}-{}", name, uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        std::env::set_current_dir(&base).unwrap();
        let res = f();
        std::env::set_current_dir(&old).unwrap();
        let _ = fs::remove_dir_all(&base);
        res
    }

    #[test]
    fn test_load_or_init_creates_default() {
        with_temp_cwd("create-default", || {
            assert!(!std::path::Path::new("config/config.json").exists());
            let cfg = load_or_init().expect("should create default config");
            assert!(std::path::Path::new("config/config.json").exists());
            // 校验部分默认值
            assert!(cfg.http.fake_sni_enabled);
            assert_eq!(cfg.http.fake_sni_host, "baidu.com");
            assert!(cfg.tls.san_whitelist.iter().any(|d| d.contains("github.com")));
            assert_eq!(cfg.logging.log_level, "info");
        });
    }

    #[test]
    fn test_save_and_reload_roundtrip() {
        with_temp_cwd("save-reload", || {
            let mut cfg = AppConfig::default();
            cfg.http.fake_sni_enabled = false;
            cfg.http.max_redirects = 3;
            save(&cfg).expect("save should succeed");
            // 再次读取
            let loaded = load_or_init().expect("load should succeed");
            assert_eq!(loaded.http.fake_sni_enabled, false);
            assert_eq!(loaded.http.max_redirects, 3);
        });
    }
}
