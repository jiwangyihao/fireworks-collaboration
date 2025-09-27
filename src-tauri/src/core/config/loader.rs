use anyhow::{Context, Result};
use dirs_next as dirs;
use std::sync::{Mutex, OnceLock};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use super::model::AppConfig;

fn join_default_path(base: &Path) -> PathBuf {
    let mut p = base.to_path_buf();
    p.push("config");
    p.push("config.json");
    p
}

// 全局配置基目录（用于 Tauri 应用在 setup 阶段注入 app_config_dir）
static GLOBAL_BASE_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn global_base_dir() -> Option<PathBuf> {
    if let Some(lock) = GLOBAL_BASE_DIR.get() {
        if let Ok(guard) = lock.lock() {
            return guard.clone();
        }
    }
    None
}

/// 由应用在启动时设置配置基目录，一旦设置将作为默认配置路径的来源。
/// 重复设置将被忽略（保持第一次设置的值）。
pub fn set_global_base_dir<P: AsRef<Path>>(base: P) {
    let cell = GLOBAL_BASE_DIR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap();
    if guard.is_none() {
        *guard = Some(base.as_ref().to_path_buf());
    }
}

fn config_path() -> PathBuf {
    // 优先使用应用在启动时注入的基目录；若尚未注入，则回退到系统应用配置目录
    // Windows: %APPDATA%\<identifier>
    // macOS: ~/Library/Application Support/<identifier>
    // Linux: ~/.config/<identifier>
    let base = global_base_dir().unwrap_or_else(|| {
        // 与 tauri.conf.json 中的 identifier 保持一致
        let identifier = "top.jwyihao.fireworks-collaboration";
        if let Some(mut dir) = dirs::config_dir() {
            dir.push(identifier);
            dir
        } else {
            // 极端环境下获取失败，才回退到当前目录（尽量避免落盘到执行目录）
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }
    });
    join_default_path(&base)
}

/// 返回配置基目录（包含 config 子目录的上一级）。仅用于派生其它观测文件（如 cert-fp.log）。
pub fn base_dir() -> PathBuf {
    let p = config_path();
    p.parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf()
}

#[cfg(test)]
pub fn test_override_global_base_dir<P: AsRef<Path>>(base: P) {
    let cell = GLOBAL_BASE_DIR.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().unwrap();
    *guard = Some(base.as_ref().to_path_buf());
}

#[cfg(test)]
pub fn test_clear_global_base_dir() {
    if let Some(cell) = GLOBAL_BASE_DIR.get() {
        let mut guard = cell.lock().unwrap();
        *guard = None;
    }
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
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    let json = serde_json::to_string_pretty(cfg).context("serialize config")?;
    let mut f =
        fs::File::create(path).with_context(|| format!("create config: {}", path.display()))?;
    f.write_all(json.as_bytes()).context("write config")?;
    tracing::info!(target = "config", path = %path.display(), "config saved");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> &'static Mutex<()> {
        static G: OnceLock<Mutex<()>> = OnceLock::new();
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
}
