use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::core::config::loader;
use crate::core::config::model::{AppConfig, TlsCfg};
use crate::core::credential::config::CredentialConfig;
use crate::core::ip_pool::{config::load_or_init_file_at, IpPoolFileConfig, IpPoolRuntimeConfig};
use crate::core::proxy::config::ProxyConfig;

const TEMPLATE_SCHEMA_MAJOR: u32 = 1;
pub const TEMPLATE_SCHEMA_VERSION: &str = "1.0.0";

fn default_true() -> bool {
    true
}

fn default_overwrite() -> SectionStrategy {
    SectionStrategy::Overwrite
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamConfigTemplate {
    pub schema_version: String,
    #[serde(default)]
    pub metadata: TemplateMetadata,
    #[serde(default)]
    pub sections: TemplateSections,
}

impl TeamConfigTemplate {
    pub fn new() -> Self {
        Self {
            schema_version: TEMPLATE_SCHEMA_VERSION.to_string(),
            metadata: TemplateMetadata::default(),
            sections: TemplateSections::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSections {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_pool: Option<IpPoolTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsCfg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<CredentialConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpPoolTemplate {
    pub runtime: IpPoolRuntimeConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<IpPoolFileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateExportOptions {
    #[serde(default = "default_true")]
    pub include_ip_pool: bool,
    #[serde(default = "default_true")]
    pub include_ip_pool_file: bool,
    #[serde(default = "default_true")]
    pub include_proxy: bool,
    #[serde(default = "default_true")]
    pub include_tls: bool,
    #[serde(default = "default_true")]
    pub include_credential: bool,
    #[serde(default)]
    pub metadata: Option<TemplateMetadata>,
}

impl Default for TemplateExportOptions {
    fn default() -> Self {
        Self {
            include_ip_pool: true,
            include_ip_pool_file: true,
            include_proxy: true,
            include_tls: true,
            include_credential: true,
            metadata: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateImportOptions {
    #[serde(default = "default_true")]
    pub include_ip_pool: bool,
    #[serde(default = "default_true")]
    pub include_ip_pool_file: bool,
    #[serde(default = "default_true")]
    pub include_proxy: bool,
    #[serde(default = "default_true")]
    pub include_tls: bool,
    #[serde(default = "default_true")]
    pub include_credential: bool,
    #[serde(default)]
    pub strategies: ImportStrategyConfig,
}

impl Default for TemplateImportOptions {
    fn default() -> Self {
        Self {
            include_ip_pool: true,
            include_ip_pool_file: true,
            include_proxy: true,
            include_tls: true,
            include_credential: true,
            strategies: ImportStrategyConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStrategyConfig {
    #[serde(default = "default_overwrite")]
    pub ip_pool: SectionStrategy,
    #[serde(default = "default_overwrite")]
    pub ip_pool_file: SectionStrategy,
    #[serde(default = "default_overwrite")]
    pub proxy: SectionStrategy,
    #[serde(default = "default_overwrite")]
    pub tls: SectionStrategy,
    #[serde(default = "default_overwrite")]
    pub credential: SectionStrategy,
}

impl Default for ImportStrategyConfig {
    fn default() -> Self {
        Self {
            ip_pool: SectionStrategy::Overwrite,
            ip_pool_file: SectionStrategy::Overwrite,
            proxy: SectionStrategy::Overwrite,
            tls: SectionStrategy::Overwrite,
            credential: SectionStrategy::Overwrite,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SectionStrategy {
    Overwrite,
    KeepLocal,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateImportReport {
    pub schema_version: String,
    #[serde(default)]
    pub applied: Vec<AppliedSection>,
    #[serde(default)]
    pub skipped: Vec<SkippedSection>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
}

impl TemplateImportReport {
    fn new(schema_version: String) -> Self {
        Self {
            schema_version,
            applied: Vec::new(),
            skipped: Vec::new(),
            warnings: Vec::new(),
            backup_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedSection {
    pub section: TemplateSectionKind,
    pub strategy: SectionStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSection {
    pub section: TemplateSectionKind,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum TemplateSectionKind {
    IpPoolRuntime,
    IpPoolFile,
    Proxy,
    Tls,
    Credential,
}

pub struct TemplateImportOutcome {
    pub report: TemplateImportReport,
    pub updated_ip_pool_file: Option<IpPoolFileConfig>,
}

pub fn export_template(
    cfg: &AppConfig,
    base_dir: &Path,
    options: &TemplateExportOptions,
) -> Result<TeamConfigTemplate> {
    let mut template = TeamConfigTemplate::new();
    template.metadata = options
        .metadata
        .clone()
        .unwrap_or_else(|| TemplateMetadata {
            generated_at: Some(Utc::now().to_rfc3339()),
            generated_by: Some("fireworks-collaboration".to_string()),
            ..TemplateMetadata::default()
        });

    if options.include_ip_pool {
        let mut ip_template = IpPoolTemplate {
            runtime: sanitized_ip_pool_runtime(cfg.ip_pool.clone()),
            file: None,
        };

        if options.include_ip_pool_file {
            match load_or_init_file_at(base_dir) {
                Ok(file_cfg) => {
                    ip_template.file = Some(file_cfg);
                }
                Err(err) => {
                    return Err(err.context("failed to load ip pool file config"));
                }
            }
        }

        template.sections.ip_pool = Some(ip_template);
    }

    if options.include_proxy {
        template.sections.proxy = Some(sanitized_proxy(cfg.proxy.clone()));
    }

    if options.include_tls {
        template.sections.tls = Some(cfg.tls.clone());
    }

    if options.include_credential {
        template.sections.credential = Some(sanitized_credential(cfg.credential.clone()));
    }

    Ok(template)
}

use crate::core::config::fs::{FileSystem, RealFileSystem};
use std::sync::Arc;

pub struct TeamTemplateManager {
    fs: Arc<dyn FileSystem>,
}

impl Default for TeamTemplateManager {
    fn default() -> Self {
        Self::new(Arc::new(RealFileSystem))
    }
}

impl TeamTemplateManager {
    pub fn new(fs: Arc<dyn FileSystem>) -> Self {
        Self { fs }
    }

    pub fn write_template_to_path(&self, template: &TeamConfigTemplate, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            self.fs
                .create_dir_all(parent)
                .with_context(|| format!("create template parent dir: {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(template).context("serialize template")?;
        self.fs
            .write(dest, json.as_bytes())
            .with_context(|| format!("write template: {}", dest.display()))?;
        Ok(())
    }

    pub fn load_template_from_path(&self, path: &Path) -> Result<TeamConfigTemplate> {
        let data = self
            .fs
            .read(path)
            .with_context(|| format!("read template: {}", path.display()))?;
        let template: TeamConfigTemplate =
            serde_json::from_slice(&data).context("parse team config template")?;
        Ok(template)
    }

    pub fn backup_config_file(&self, base_dir: &Path) -> Result<Option<PathBuf>> {
        let source = loader::config_path_at(base_dir);
        if !self.fs.exists(&source) {
            return Ok(None);
        }

        let timestamp = Utc::now().format("%Y%m%d%H%M%S");
        let backup_name = format!("team-config-backup-{}.json", timestamp);
        let mut dest = source
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| base_dir.to_path_buf());
        dest.push(backup_name);

        if let Some(parent) = dest.parent() {
            self.fs
                .create_dir_all(parent)
                .with_context(|| format!("create backup directory: {}", parent.display()))?;
        }
        self.fs.copy(&source, &dest).with_context(|| {
            format!(
                "backup config from {} to {}",
                source.display(),
                dest.display()
            )
        })?;

        Ok(Some(dest))
    }
}

pub fn write_template_to_path(template: &TeamConfigTemplate, dest: &Path) -> Result<()> {
    TeamTemplateManager::default().write_template_to_path(template, dest)
}

pub fn load_template_from_path(path: &Path) -> Result<TeamConfigTemplate> {
    TeamTemplateManager::default().load_template_from_path(path)
}

pub fn backup_config_file(base_dir: &Path) -> Result<Option<PathBuf>> {
    TeamTemplateManager::default().backup_config_file(base_dir)
}

pub fn apply_template_to_config(
    cfg: &mut AppConfig,
    current_ip_file: Option<IpPoolFileConfig>,
    template: &TeamConfigTemplate,
    options: &TemplateImportOptions,
) -> Result<TemplateImportOutcome> {
    validate_schema(&template.schema_version)?;

    let mut report = TemplateImportReport::new(template.schema_version.clone());
    let mut ip_file_work = current_ip_file;

    if let Some(ip_section) = template.sections.ip_pool.as_ref() {
        if options.include_ip_pool {
            match options.strategies.ip_pool {
                SectionStrategy::KeepLocal => report.skipped.push(SkippedSection {
                    section: TemplateSectionKind::IpPoolRuntime,
                    reason: "strategyKeepLocal".to_string(),
                }),
                SectionStrategy::Overwrite => {
                    cfg.ip_pool = sanitized_ip_pool_runtime(ip_section.runtime.clone());
                    report.applied.push(AppliedSection {
                        section: TemplateSectionKind::IpPoolRuntime,
                        strategy: SectionStrategy::Overwrite,
                    });
                }
                SectionStrategy::Merge => {
                    let mut runtime = cfg.ip_pool.clone();
                    let template_runtime = sanitized_ip_pool_runtime(ip_section.runtime.clone());
                    let changed = merge_ip_pool_runtime(&mut runtime, &template_runtime);
                    cfg.ip_pool = runtime;
                    if changed {
                        report.applied.push(AppliedSection {
                            section: TemplateSectionKind::IpPoolRuntime,
                            strategy: SectionStrategy::Merge,
                        });
                    } else {
                        report.skipped.push(SkippedSection {
                            section: TemplateSectionKind::IpPoolRuntime,
                            reason: "noChanges".to_string(),
                        });
                    }
                }
            }
        } else {
            report.skipped.push(SkippedSection {
                section: TemplateSectionKind::IpPoolRuntime,
                reason: "sectionDisabled".to_string(),
            });
        }

        if let Some(file_cfg) = ip_section.file.as_ref() {
            if options.include_ip_pool_file {
                match options.strategies.ip_pool_file {
                    SectionStrategy::KeepLocal => report.skipped.push(SkippedSection {
                        section: TemplateSectionKind::IpPoolFile,
                        reason: "strategyKeepLocal".to_string(),
                    }),
                    SectionStrategy::Overwrite => {
                        ip_file_work = Some(file_cfg.clone());
                        report.applied.push(AppliedSection {
                            section: TemplateSectionKind::IpPoolFile,
                            strategy: SectionStrategy::Overwrite,
                        });
                    }
                    SectionStrategy::Merge => {
                        let mut base = ip_file_work.unwrap_or_default();
                        let changed = merge_ip_pool_file(&mut base, file_cfg);
                        if changed {
                            ip_file_work = Some(base);
                            report.applied.push(AppliedSection {
                                section: TemplateSectionKind::IpPoolFile,
                                strategy: SectionStrategy::Merge,
                            });
                        } else {
                            ip_file_work = Some(base);
                            report.skipped.push(SkippedSection {
                                section: TemplateSectionKind::IpPoolFile,
                                reason: "noChanges".to_string(),
                            });
                        }
                    }
                }
            } else {
                report.skipped.push(SkippedSection {
                    section: TemplateSectionKind::IpPoolFile,
                    reason: "sectionDisabled".to_string(),
                });
            }
        }
    } else if options.include_ip_pool {
        report.skipped.push(SkippedSection {
            section: TemplateSectionKind::IpPoolRuntime,
            reason: "templateMissing".to_string(),
        });
    }

    if let Some(proxy_section) = template.sections.proxy.as_ref() {
        if options.include_proxy {
            match options.strategies.proxy {
                SectionStrategy::KeepLocal => report.skipped.push(SkippedSection {
                    section: TemplateSectionKind::Proxy,
                    reason: "strategyKeepLocal".to_string(),
                }),
                SectionStrategy::Overwrite => {
                    cfg.proxy = sanitized_proxy(proxy_section.clone());
                    report.applied.push(AppliedSection {
                        section: TemplateSectionKind::Proxy,
                        strategy: SectionStrategy::Overwrite,
                    });
                }
                SectionStrategy::Merge => {
                    let mut merged = cfg.proxy.clone();
                    let changed = merge_proxy_config(&mut merged, proxy_section);
                    cfg.proxy = merged;
                    if changed {
                        report.applied.push(AppliedSection {
                            section: TemplateSectionKind::Proxy,
                            strategy: SectionStrategy::Merge,
                        });
                    } else {
                        report.skipped.push(SkippedSection {
                            section: TemplateSectionKind::Proxy,
                            reason: "noChanges".to_string(),
                        });
                    }
                }
            }
        } else {
            report.skipped.push(SkippedSection {
                section: TemplateSectionKind::Proxy,
                reason: "sectionDisabled".to_string(),
            });
        }
    } else if options.include_proxy {
        report.skipped.push(SkippedSection {
            section: TemplateSectionKind::Proxy,
            reason: "templateMissing".to_string(),
        });
    }

    if let Some(tls_section) = template.sections.tls.as_ref() {
        if options.include_tls {
            match options.strategies.tls {
                SectionStrategy::KeepLocal => report.skipped.push(SkippedSection {
                    section: TemplateSectionKind::Tls,
                    reason: "strategyKeepLocal".to_string(),
                }),
                SectionStrategy::Overwrite => {
                    cfg.tls = tls_section.clone();
                    report.applied.push(AppliedSection {
                        section: TemplateSectionKind::Tls,
                        strategy: SectionStrategy::Overwrite,
                    });
                }
                SectionStrategy::Merge => {
                    let mut merged = cfg.tls.clone();
                    let changed = merge_tls_config(&mut merged, tls_section);
                    cfg.tls = merged;
                    if changed {
                        report.applied.push(AppliedSection {
                            section: TemplateSectionKind::Tls,
                            strategy: SectionStrategy::Merge,
                        });
                    } else {
                        report.skipped.push(SkippedSection {
                            section: TemplateSectionKind::Tls,
                            reason: "noChanges".to_string(),
                        });
                    }
                }
            }
        } else {
            report.skipped.push(SkippedSection {
                section: TemplateSectionKind::Tls,
                reason: "sectionDisabled".to_string(),
            });
        }
    } else if options.include_tls {
        report.skipped.push(SkippedSection {
            section: TemplateSectionKind::Tls,
            reason: "templateMissing".to_string(),
        });
    }

    if let Some(credential_section) = template.sections.credential.as_ref() {
        if options.include_credential {
            match options.strategies.credential {
                SectionStrategy::KeepLocal => report.skipped.push(SkippedSection {
                    section: TemplateSectionKind::Credential,
                    reason: "strategyKeepLocal".to_string(),
                }),
                SectionStrategy::Overwrite => {
                    cfg.credential = sanitized_credential(credential_section.clone());
                    report.applied.push(AppliedSection {
                        section: TemplateSectionKind::Credential,
                        strategy: SectionStrategy::Overwrite,
                    });
                }
                SectionStrategy::Merge => {
                    let mut merged = cfg.credential.clone();
                    let changed = merge_credential_config(&mut merged, credential_section);
                    cfg.credential = merged;
                    if changed {
                        report.applied.push(AppliedSection {
                            section: TemplateSectionKind::Credential,
                            strategy: SectionStrategy::Merge,
                        });
                    } else {
                        report.skipped.push(SkippedSection {
                            section: TemplateSectionKind::Credential,
                            reason: "noChanges".to_string(),
                        });
                    }
                }
            }
        } else {
            report.skipped.push(SkippedSection {
                section: TemplateSectionKind::Credential,
                reason: "sectionDisabled".to_string(),
            });
        }
    } else if options.include_credential {
        report.skipped.push(SkippedSection {
            section: TemplateSectionKind::Credential,
            reason: "templateMissing".to_string(),
        });
    }

    Ok(TemplateImportOutcome {
        report,
        updated_ip_pool_file: ip_file_work,
    })
}

fn validate_schema(schema_version: &str) -> Result<()> {
    let major = parse_schema_major(schema_version)?;
    if major != TEMPLATE_SCHEMA_MAJOR {
        return Err(anyhow!(
            "template schema major version mismatch: expected {}, got {}",
            TEMPLATE_SCHEMA_MAJOR,
            major
        ));
    }
    Ok(())
}

fn parse_schema_major(schema: &str) -> Result<u32> {
    let major_part = schema
        .split('.')
        .next()
        .ok_or_else(|| anyhow!("invalid schema version: {}", schema))?;
    major_part
        .parse::<u32>()
        .with_context(|| format!("invalid schema major version: {}", schema))
}

fn sanitized_proxy(mut proxy: ProxyConfig) -> ProxyConfig {
    proxy.password = None;
    proxy
}

fn sanitized_credential(mut credential: CredentialConfig) -> CredentialConfig {
    credential.file_path = None;
    credential
}

fn sanitized_ip_pool_runtime(mut runtime: IpPoolRuntimeConfig) -> IpPoolRuntimeConfig {
    runtime.history_path = None;
    runtime
}

fn merge_ip_pool_runtime(dest: &mut IpPoolRuntimeConfig, src: &IpPoolRuntimeConfig) -> bool {
    let defaults = IpPoolRuntimeConfig::default();
    let mut changed = false;

    if dest.enabled != src.enabled {
        dest.enabled = src.enabled;
        changed = true;
    }
    if src.sources != defaults.sources {
        dest.sources = src.sources.clone();
        changed = true;
    }
    if src.dns != defaults.dns {
        dest.dns = src.dns.clone();
        changed = true;
    }
    if src.max_parallel_probes != defaults.max_parallel_probes {
        dest.max_parallel_probes = src.max_parallel_probes;
        changed = true;
    }
    if src.probe_timeout_ms != defaults.probe_timeout_ms {
        dest.probe_timeout_ms = src.probe_timeout_ms;
        changed = true;
    }
    if src.history_path.is_some() {
        dest.history_path = src.history_path.clone();
        changed = true;
    }
    if src.cache_prune_interval_secs != defaults.cache_prune_interval_secs {
        dest.cache_prune_interval_secs = src.cache_prune_interval_secs;
        changed = true;
    }
    if src.max_cache_entries != defaults.max_cache_entries {
        dest.max_cache_entries = src.max_cache_entries;
        changed = true;
    }
    if src.singleflight_timeout_ms != defaults.singleflight_timeout_ms {
        dest.singleflight_timeout_ms = src.singleflight_timeout_ms;
        changed = true;
    }
    if src.failure_threshold != defaults.failure_threshold {
        dest.failure_threshold = src.failure_threshold;
        changed = true;
    }
    if (src.failure_rate_threshold - defaults.failure_rate_threshold).abs() > f64::EPSILON {
        dest.failure_rate_threshold = src.failure_rate_threshold;
        changed = true;
    }
    if src.failure_window_seconds != defaults.failure_window_seconds {
        dest.failure_window_seconds = src.failure_window_seconds;
        changed = true;
    }
    if src.min_samples_in_window != defaults.min_samples_in_window {
        dest.min_samples_in_window = src.min_samples_in_window;
        changed = true;
    }
    if src.cooldown_seconds != defaults.cooldown_seconds {
        dest.cooldown_seconds = src.cooldown_seconds;
        changed = true;
    }
    if src.circuit_breaker_enabled != defaults.circuit_breaker_enabled {
        dest.circuit_breaker_enabled = src.circuit_breaker_enabled;
        changed = true;
    }

    changed
}

fn merge_ip_pool_file(dest: &mut IpPoolFileConfig, src: &IpPoolFileConfig) -> bool {
    let defaults = IpPoolFileConfig::default();
    let mut changed = false;

    if src.score_ttl_seconds != defaults.score_ttl_seconds {
        dest.score_ttl_seconds = src.score_ttl_seconds;
        changed = true;
    }

    if merge_unique(&mut dest.preheat_domains, &src.preheat_domains) {
        changed = true;
    }
    if merge_unique(&mut dest.user_static, &src.user_static) {
        changed = true;
    }
    if merge_unique(&mut dest.blacklist, &src.blacklist) {
        changed = true;
    }
    if merge_unique(&mut dest.whitelist, &src.whitelist) {
        changed = true;
    }

    changed
}

fn merge_proxy_config(dest: &mut ProxyConfig, src: &ProxyConfig) -> bool {
    let defaults = ProxyConfig::default();
    let mut changed = false;

    if src.mode != defaults.mode {
        dest.mode = src.mode;
        changed = true;
    }
    if !src.url.trim().is_empty() {
        if dest.url != src.url {
            dest.url = src.url.clone();
            changed = true;
        }
    }
    if let Some(username) = src.username.as_ref() {
        if !username.is_empty() {
            dest.username = Some(username.clone());
            changed = true;
        }
    }
    if src.disable_custom_transport != defaults.disable_custom_transport {
        dest.disable_custom_transport = src.disable_custom_transport;
        changed = true;
    }
    if src.timeout_seconds != defaults.timeout_seconds {
        dest.timeout_seconds = src.timeout_seconds;
        changed = true;
    }
    if (src.fallback_threshold - defaults.fallback_threshold).abs() > f64::EPSILON {
        dest.fallback_threshold = src.fallback_threshold;
        changed = true;
    }
    if src.fallback_window_seconds != defaults.fallback_window_seconds {
        dest.fallback_window_seconds = src.fallback_window_seconds;
        changed = true;
    }
    if src.recovery_cooldown_seconds != defaults.recovery_cooldown_seconds {
        dest.recovery_cooldown_seconds = src.recovery_cooldown_seconds;
        changed = true;
    }
    if src.health_check_interval_seconds != defaults.health_check_interval_seconds {
        dest.health_check_interval_seconds = src.health_check_interval_seconds;
        changed = true;
    }
    if src.recovery_strategy != defaults.recovery_strategy {
        dest.recovery_strategy = src.recovery_strategy.clone();
        changed = true;
    }
    if !src.probe_url.trim().is_empty() && src.probe_url != defaults.probe_url {
        dest.probe_url = src.probe_url.clone();
        changed = true;
    }
    if src.probe_timeout_seconds != defaults.probe_timeout_seconds {
        dest.probe_timeout_seconds = src.probe_timeout_seconds;
        changed = true;
    }
    if src.recovery_consecutive_threshold != defaults.recovery_consecutive_threshold {
        dest.recovery_consecutive_threshold = src.recovery_consecutive_threshold;
        changed = true;
    }
    if src.debug_proxy_logging != defaults.debug_proxy_logging {
        dest.debug_proxy_logging = src.debug_proxy_logging;
        changed = true;
    }

    changed
}

fn merge_tls_config(dest: &mut TlsCfg, src: &TlsCfg) -> bool {
    let defaults = AppConfig::default().tls;
    let mut changed = false;

    if merge_unique(&mut dest.spki_pins, &src.spki_pins) {
        changed = true;
    }
    if src.metrics_enabled != defaults.metrics_enabled {
        dest.metrics_enabled = src.metrics_enabled;
        changed = true;
    }
    if src.cert_fp_log_enabled != defaults.cert_fp_log_enabled {
        dest.cert_fp_log_enabled = src.cert_fp_log_enabled;
        changed = true;
    }
    if src.cert_fp_max_bytes != defaults.cert_fp_max_bytes {
        dest.cert_fp_max_bytes = src.cert_fp_max_bytes;
        changed = true;
    }

    changed
}

fn merge_credential_config(dest: &mut CredentialConfig, src: &CredentialConfig) -> bool {
    let defaults = CredentialConfig::default();
    let mut changed = false;

    if src.storage != defaults.storage {
        dest.storage = src.storage;
        changed = true;
    }
    if src.default_ttl_seconds != defaults.default_ttl_seconds {
        dest.default_ttl_seconds = src.default_ttl_seconds;
        changed = true;
    }
    if src.debug_logging != defaults.debug_logging {
        dest.debug_logging = src.debug_logging;
        changed = true;
    }
    if src.audit_mode != defaults.audit_mode {
        dest.audit_mode = src.audit_mode;
        changed = true;
    }
    if src.require_confirmation != defaults.require_confirmation {
        dest.require_confirmation = src.require_confirmation;
        changed = true;
    }
    if src.key_cache_ttl_seconds != defaults.key_cache_ttl_seconds {
        dest.key_cache_ttl_seconds = src.key_cache_ttl_seconds;
        changed = true;
    }

    changed
}

fn merge_unique<T>(dest: &mut Vec<T>, src: &[T]) -> bool
where
    T: Clone + PartialEq,
{
    let mut changed = false;
    for item in src {
        if !dest.contains(item) {
            dest.push(item.clone());
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;
    use std::io::{self, Error, ErrorKind};

    mock! {
        pub FileSystem {}
        impl FileSystem for FileSystem {
            fn create_dir_all(&self, path: &Path) -> io::Result<()>;
            fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
            fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
            fn copy(&self, from: &Path, to: &Path) -> io::Result<u64>;
            fn exists(&self, path: &Path) -> bool;
        }
    }

    #[test]
    fn test_write_template_success() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs
            .expect_create_dir_all()
            .with(always())
            .returning(|_| Ok(()));
        mock_fs
            .expect_write()
            .with(always(), always())
            .returning(|_, _| Ok(()));

        let manager = TeamTemplateManager::new(Arc::new(mock_fs));
        let template = TeamConfigTemplate::new();
        let path = Path::new("/tmp/test.json");

        assert!(manager.write_template_to_path(&template, path).is_ok());
    }

    #[test]
    fn test_write_template_write_failure() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.expect_create_dir_all().returning(|_| Ok(()));
        mock_fs
            .expect_write()
            .returning(|_, _| Err(Error::new(ErrorKind::PermissionDenied, "denied")));

        let manager = TeamTemplateManager::new(Arc::new(mock_fs));
        let template = TeamConfigTemplate::new();
        let path = Path::new("/tmp/test.json");

        let res = manager.write_template_to_path(&template, path);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("write template"));
    }

    #[test]
    fn test_load_template_read_failure() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs
            .expect_read()
            .returning(|_| Err(Error::new(ErrorKind::NotFound, "not found")));

        let manager = TeamTemplateManager::new(Arc::new(mock_fs));
        let path = Path::new("/tmp/test.json");

        let res = manager.load_template_from_path(path);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("read template"));
    }

    #[test]
    fn test_backup_config_file_not_exists() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.expect_exists().returning(|_| false);

        let manager = TeamTemplateManager::new(Arc::new(mock_fs));
        let base = Path::new("/app/config");

        let res = manager.backup_config_file(base);
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn test_backup_config_file_copy_failure() {
        let mut mock_fs = MockFileSystem::new();
        mock_fs.expect_exists().returning(|_| true);
        mock_fs.expect_create_dir_all().returning(|_| Ok(()));
        mock_fs
            .expect_copy()
            .returning(|_, _| Err(Error::new(ErrorKind::Other, "disk full")));

        let manager = TeamTemplateManager::new(Arc::new(mock_fs));
        let base = Path::new("/app/config");

        let res = manager.backup_config_file(base);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("backup config"));
    }
}
