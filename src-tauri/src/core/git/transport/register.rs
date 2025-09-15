use std::sync::OnceLock;

use git2::{transport, Error, Remote};

use crate::core::config::loader::load_or_init;
use crate::core::config::model::AppConfig;

use super::http::CustomHttpsSubtransport;

/// 仅注册一次自定义传输前缀 "https+custom"。注册后，所有该 scheme 的 URL 都会通过本实现建立连接。
static REGISTER_ONCE: OnceLock<()> = OnceLock::new();

pub fn ensure_registered(_cfg: &AppConfig) -> Result<(), Error> {
    let mut err: Option<Error> = None;
    REGISTER_ONCE.get_or_init(|| {
        // 安全：register 需外部同步；我们用 OnceLock 保证只注册一次。
        let r = unsafe {
            transport::register("https+custom", move |remote: &Remote| {
                // HTTP(s) 是无状态 smart 协议：需要启用 stateless-rpc 模式
                let rpc = true;
                // 每次创建传输时加载“最新配置”，避免保存后需重启
                let cfg_now = load_or_init().unwrap_or_else(|_| AppConfig::default());
                let sub = CustomHttpsSubtransport::new(cfg_now);
                transport::Transport::smart(remote, rpc, sub)
            })
        };
        if let Err(e) = r {
            err = Some(e);
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_once_ok() {
        let cfg = AppConfig::default();
        // 多次调用不应 panic
        let _ = ensure_registered(&cfg);
        let _ = ensure_registered(&cfg);
    }
}
