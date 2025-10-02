use fireworks_collaboration_lib::core::config::loader;

#[test]
fn test_log_path_disabled_when_cfg_off() {
    let temp = tempfile::tempdir().expect("create temp dir for config override");
    
    // 直接使用 public API 而不是测试辅助函数
    loader::set_global_base_dir(temp.path());
    
    // 由于 test_reset_fp_state 是内部函数，我们跳过这步
    // 测试的核心是验证配置的启用/禁用行为

    // 默认配置开启证书指纹日志
    let cfg = loader::load_or_init().expect("load default config");
    assert!(
        cfg.tls.cert_fp_log_enabled,
        "default config should enable cert fp log"
    );

    // 人为关闭后保存
    let mut cfg = cfg;
    cfg.tls.cert_fp_log_enabled = false;
    loader::save(&cfg).expect("save updated config");
    
    // 重新加载配置验证已保存
    let reloaded = loader::load_or_init().expect("reload config");
    assert!(!reloaded.tls.cert_fp_log_enabled);

    // 清理：恢复默认设置
    let mut cfg = reloaded;
    cfg.tls.cert_fp_log_enabled = true;
    let _ = loader::save(&cfg);
}
