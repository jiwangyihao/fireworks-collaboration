use fireworks_collaboration_lib::core::config::model::AppConfig;
use fireworks_collaboration_lib::core::config::team_template::*;
use fireworks_collaboration_lib::core::proxy::config::ProxyConfig;
use std::fs;

#[test]
fn test_apply_template_strategies() {
    let mut cfg = AppConfig::default();
    cfg.proxy.mode = fireworks_collaboration_lib::core::proxy::config::ProxyMode::Off;

    let mut template = TeamConfigTemplate::new();
    let mut proxy_template = ProxyConfig::default();
    proxy_template.mode = fireworks_collaboration_lib::core::proxy::config::ProxyMode::System;
    template.sections.proxy = Some(proxy_template);

    let mut options = TemplateImportOptions::default();
    options.strategies.proxy = SectionStrategy::Overwrite;

    let outcome = apply_template_to_config(&mut cfg, None, &template, &options).unwrap();
    assert!(matches!(
        cfg.proxy.mode,
        fireworks_collaboration_lib::core::proxy::config::ProxyMode::System
    ));
    assert_eq!(outcome.report.applied.len(), 1);
}

#[test]
fn test_apply_template_keep_local() {
    let mut cfg = AppConfig::default();
    cfg.proxy.mode = fireworks_collaboration_lib::core::proxy::config::ProxyMode::Off;

    let mut template = TeamConfigTemplate::new();
    let mut proxy_template = ProxyConfig::default();
    proxy_template.mode = fireworks_collaboration_lib::core::proxy::config::ProxyMode::System;
    template.sections.proxy = Some(proxy_template);

    let mut options = TemplateImportOptions::default();
    options.strategies.proxy = SectionStrategy::KeepLocal;
    options.include_ip_pool = false;
    options.include_ip_pool_file = false;
    options.include_tls = false;
    options.include_credential = false;

    let outcome = apply_template_to_config(&mut cfg, None, &template, &options).unwrap();
    assert!(matches!(
        cfg.proxy.mode,
        fireworks_collaboration_lib::core::proxy::config::ProxyMode::Off
    ));
    assert_eq!(outcome.report.skipped.len(), 1);
    assert_eq!(outcome.report.skipped[0].reason, "strategyKeepLocal");
}

#[test]
fn test_backup_config_file_no_source() {
    let temp = tempfile::tempdir().unwrap();
    let result = backup_config_file(temp.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_backup_config_file_success() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    fs::create_dir(&config_dir).unwrap();
    let config_file = config_dir.join("config.json");
    fs::write(&config_file, "{}").unwrap();

    let result = backup_config_file(temp.path());
    assert!(result.is_ok());
    let backup_path = result.unwrap().unwrap();
    assert!(backup_path.exists());
    assert!(backup_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("team-config-backup-"));
}

#[test]
fn test_write_template_io_error() {
    let temp = tempfile::tempdir().unwrap();
    let block = temp.path().join("block");
    fs::write(&block, "").unwrap();
    let bad_path = block.join("template.json");

    let template = TeamConfigTemplate::new();
    let result = write_template_to_path(&template, &bad_path);
    assert!(result.is_err());
}

#[test]
fn test_apply_template_merge_proxy_partial() {
    let mut cfg = AppConfig::default();
    cfg.proxy.url = "http://original:8080".to_string();
    cfg.proxy.username = Some("user1".to_string());

    let mut template = TeamConfigTemplate::new();
    let mut proxy_template = ProxyConfig::default();
    proxy_template.url = "http://new:8080".to_string();
    proxy_template.mode = fireworks_collaboration_lib::core::proxy::config::ProxyMode::System;

    template.sections.proxy = Some(proxy_template);

    let mut options = TemplateImportOptions::default();
    options.strategies.proxy = SectionStrategy::Merge;

    let outcome = apply_template_to_config(&mut cfg, None, &template, &options).unwrap();

    assert!(matches!(
        cfg.proxy.mode,
        fireworks_collaboration_lib::core::proxy::config::ProxyMode::System
    ));
    assert_eq!(cfg.proxy.url, "http://new:8080");
    assert_eq!(cfg.proxy.username, Some("user1".to_string()));

    let applied = &outcome.report.applied[0];
    assert!(matches!(applied.strategy, SectionStrategy::Merge));
}

#[test]
fn test_apply_template_merge_no_changes() {
    let mut cfg = AppConfig::default();
    cfg.proxy.url = "http://same:8080".to_string();

    let mut template = TeamConfigTemplate::new();
    let mut proxy_template = ProxyConfig::default();
    proxy_template.url = "http://same:8080".to_string();

    template.sections.proxy = Some(proxy_template);

    let mut options = TemplateImportOptions::default();
    options.strategies.proxy = SectionStrategy::Merge;
    options.include_ip_pool = false;
    options.include_ip_pool_file = false;
    options.include_tls = false;
    options.include_credential = false;

    let outcome = apply_template_to_config(&mut cfg, None, &template, &options).unwrap();

    assert_eq!(outcome.report.applied.len(), 0);
    assert_eq!(outcome.report.skipped.len(), 1);
    assert_eq!(outcome.report.skipped[0].reason, "noChanges");
}

#[test]
fn test_export_template() {
    let mut cfg = AppConfig::default();
    cfg.proxy.url = "http://export:8080".to_string();
    let temp = tempfile::tempdir().unwrap();
    let options = TemplateExportOptions::default();

    let template = export_template(&cfg, temp.path(), &options).unwrap();
    assert_eq!(template.sections.proxy.unwrap().url, "http://export:8080");
}

#[test]
fn test_load_template_io_error() {
    let temp = tempfile::tempdir().unwrap();
    let bad = temp.path().join("missing.json");
    let res = load_template_from_path(&bad);
    assert!(res.is_err());
}

#[test]
fn test_apply_template_merge_tls() {
    let mut cfg = AppConfig::default();
    cfg.tls.metrics_enabled = true; // Default
    cfg.tls.spki_pins.push("pin1".to_string());

    let mut template = TeamConfigTemplate::new();
    let mut tls_template = AppConfig::default().tls;
    tls_template.metrics_enabled = false; // Changed
    tls_template.spki_pins.push("pin2".to_string());

    template.sections.tls = Some(tls_template);

    let mut options = TemplateImportOptions::default();
    options.strategies.tls = SectionStrategy::Merge;

    let outcome = apply_template_to_config(&mut cfg, None, &template, &options).unwrap();

    assert!(!cfg.tls.metrics_enabled); // Should be false
    assert!(cfg.tls.spki_pins.contains(&"pin1".to_string()));
    assert!(cfg.tls.spki_pins.contains(&"pin2".to_string()));

    // Check report
    let applied = outcome
        .report
        .applied
        .iter()
        .find(|a| matches!(a.section, TemplateSectionKind::Tls))
        .unwrap();
    assert!(matches!(applied.strategy, SectionStrategy::Merge));
}
