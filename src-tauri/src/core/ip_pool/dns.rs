use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use tokio::{net::lookup_host, task::JoinSet, time::timeout};
use trust_dns_resolver::{
    config::{LookupIpStrategy, NameServerConfigGroup, ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};
use url::Url;

use super::config::{DnsResolverConfig, DnsResolverProtocol, DnsRuntimeConfig};

/// 单条 DNS 解析结果，包含 IP 与来源标签。
#[derive(Debug, Clone)]
pub struct DnsResolvedIp {
    pub ip: IpAddr,
    pub label: Option<String>,
}

impl DnsResolvedIp {
    fn new(ip: IpAddr, label: Option<String>) -> Self {
        Self { ip, label }
    }
}

const SYSTEM_LABEL: &str = "系统 DNS";

/// Resolver retry policy tuned to avoid long blocking waits.
const DNS_REQUEST_TIMEOUT_MS: u64 = 1_500;
const DNS_MAX_ATTEMPTS: usize = 2;
const DNS_TOTAL_TIMEOUT_MS: u64 = 4_000;
const DNS_MIN_CUSTOM_SUCCESSES: usize = 1;
const DNS_MAX_CONSECUTIVE_ERRORS: usize = 2;

/// 根据运行期配置解析域名，支持系统解析及自定义 DoH/DoT/UDP 解析器。
pub async fn resolve(host: &str, port: u16, cfg: &DnsRuntimeConfig) -> Result<Vec<DnsResolvedIp>> {
    let mut results: Vec<DnsResolvedIp> = Vec::new();

    if cfg.use_system {
        match resolve_system(host, port).await {
            Ok(ips) => {
                for ip in ips {
                    results.push(DnsResolvedIp::new(ip, Some(SYSTEM_LABEL.to_string())));
                }
            }
            Err(err) => {
                tracing::warn!(
                    target = "ip_pool",
                    host,
                    error = %err,
                    "system dns resolution failed"
                );
            }
        }
    }

    let mut effective_resolvers: Vec<DnsResolverConfig> = Vec::new();

    let mut visited_presets: HashSet<String> = HashSet::new();

    for key in &cfg.enabled_presets {
        if !visited_presets.insert(key.clone()) {
            continue;
        }
        if let Some(preset) = cfg.preset_catalog.get(key) {
            if preset.description.as_deref() == Some("不可用") {
                tracing::debug!(
                    target = "ip_pool",
                    preset = key.as_str(),
                    "skip dns preset marked as unavailable"
                );
                continue;
            }
            match preset.to_resolver_config(key) {
                Ok(resolver) => effective_resolvers.push(resolver),
                Err(err) => {
                    tracing::warn!(
                        target = "ip_pool",
                        preset = key.as_str(),
                        error = %err,
                        "failed to build dns resolver from preset"
                    );
                }
            }
        } else {
            tracing::warn!(
                target = "ip_pool",
                preset = key.as_str(),
                "dns preset not found in catalog"
            );
        }
    }

    effective_resolvers.extend(cfg.resolvers.clone());

    if !effective_resolvers.is_empty() {
        let host_owned = host.to_string();
        let mut join_set: JoinSet<
            Result<(String, Vec<IpAddr>), (String, DnsResolverProtocol, anyhow::Error)>,
        > = JoinSet::new();

        for entry in effective_resolvers {
            let host = host_owned.clone();
            join_set.spawn(async move {
                let tag = entry.display_tag();
                let label = entry.label.clone();
                let protocol = entry.protocol.clone();
                match resolve_with_custom(&host, &entry).await {
                    Ok(ips) => Ok((tag, ips)),
                    Err(err) => Err((label, protocol, err)),
                }
            });
        }

        let total_timeout = Duration::from_millis(DNS_TOTAL_TIMEOUT_MS);
        let start = std::time::Instant::now();
        let mut consecutive_errors = 0_usize;
        let mut custom_successes = 0_usize;

        while !join_set.is_empty() {
            let elapsed = start.elapsed();
            if elapsed >= total_timeout {
                tracing::warn!(
                    target = "ip_pool",
                    host,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "custom dns resolution timed out; aborting remaining resolvers"
                );
                join_set.abort_all();
                break;
            }

            let remaining = total_timeout - elapsed;
            match timeout(remaining, join_set.join_next()).await {
                Ok(Some(Ok(Ok((tag, ips))))) => {
                    consecutive_errors = 0;
                    if ips.is_empty() {
                        tracing::debug!(
                            target = "ip_pool",
                            host,
                            resolver = tag.as_str(),
                            "custom resolver returned no records"
                        );
                        continue;
                    }

                    for ip in ips {
                        results.push(DnsResolvedIp::new(ip, Some(tag.clone())));
                    }
                    custom_successes = custom_successes.saturating_add(1);
                    if custom_successes >= DNS_MIN_CUSTOM_SUCCESSES {
                        join_set.abort_all();
                        break;
                    }
                }
                Ok(Some(Ok(Err((label, protocol, err))))) => {
                    consecutive_errors = consecutive_errors.saturating_add(1);
                    tracing::warn!(
                        target = "ip_pool",
                        host,
                        resolver = label.as_str(),
                        protocol = protocol.display_name(),
                        error = %err,
                        "custom dns resolution failed"
                    );

                    if custom_successes > 0 && consecutive_errors >= DNS_MAX_CONSECUTIVE_ERRORS {
                        tracing::debug!(
                            target = "ip_pool",
                            host,
                            "stopping additional resolvers after repeated failures"
                        );
                        join_set.abort_all();
                        break;
                    }
                }
                Ok(Some(Err(join_err))) => {
                    tracing::debug!(
                        target = "ip_pool",
                        host,
                        error = %join_err,
                        "custom resolver task join error"
                    );
                }
                Ok(None) => break,
                Err(_) => {
                    tracing::warn!(
                        target = "ip_pool",
                        host,
                        timeout_ms = remaining.as_millis() as u64,
                        "custom resolver tasks exceeded remaining timeout"
                    );
                    join_set.abort_all();
                    break;
                }
            }
        }
    }

    results.sort_by(|a, b| a.ip.cmp(&b.ip).then_with(|| a.label.cmp(&b.label)));
    results.dedup_by(|a, b| a.ip == b.ip && a.label == b.label);
    Ok(results)
}

async fn resolve_system(host: &str, port: u16) -> Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = lookup_host((host, port))
        .await?
        .map(|addr| addr.ip())
        .collect();
    ips.sort();
    ips.dedup();
    Ok(ips)
}

async fn resolve_with_custom(host: &str, entry: &DnsResolverConfig) -> Result<Vec<IpAddr>> {
    let resolver = build_resolver(entry).await?;
    let response = resolver.lookup_ip(host).await?;
    let mut ips: Vec<IpAddr> = response.iter().collect();
    ips.sort();
    ips.dedup();
    Ok(ips)
}

async fn build_resolver(entry: &DnsResolverConfig) -> Result<TokioAsyncResolver> {
    let resolver_config = match entry.protocol {
        DnsResolverProtocol::Udp => build_udp_config(entry).await?,
        DnsResolverProtocol::Dot => build_dot_config(entry).await?,
        DnsResolverProtocol::Doh => build_doh_config(entry).await?,
    };

    let mut opts = ResolverOpts::default();
    opts.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
    opts.cache_size = entry.cache_size.unwrap_or(0);
    opts.attempts = DNS_MAX_ATTEMPTS;
    opts.timeout = Duration::from_millis(DNS_REQUEST_TIMEOUT_MS);
    opts.try_tcp_on_error = false;

    let resolver = TokioAsyncResolver::tokio(resolver_config, opts);
    Ok(resolver)
}

async fn build_udp_config(entry: &DnsResolverConfig) -> Result<ResolverConfig> {
    let port = entry.effective_port();
    let ips = collect_endpoint_ips(entry, port).await?;
    if ips.is_empty() {
        return Err(anyhow!("udp resolver {} missing upstream ip", entry.label));
    }

    let group = NameServerConfigGroup::from_ips_clear(&ips, port, true);
    Ok(ResolverConfig::from_parts(None, vec![], group))
}

async fn build_dot_config(entry: &DnsResolverConfig) -> Result<ResolverConfig> {
    let port = entry.effective_port();
    let (host, port) = parse_host_port(&entry.endpoint, port)?;
    let ips = collect_endpoint_ips(entry, port).await?;
    if ips.is_empty() {
        return Err(anyhow!("dot resolver {} missing bootstrap ip", entry.label));
    }

    let tls_name = match entry.sni.as_deref() {
        Some(sni) if !sni.eq_ignore_ascii_case(&host) => {
            tracing::debug!(
                target = "ip_pool",
                resolver = entry.label.as_str(),
                requested_sni = sni,
                actual_host = host.as_str(),
                "ignoring custom SNI for DoT resolver"
            );
            host.clone()
        }
        Some(sni) => sni.to_string(),
        None => host.clone(),
    };
    let group = NameServerConfigGroup::from_ips_tls(&ips, port, tls_name, true);
    Ok(ResolverConfig::from_parts(None, vec![], group))
}

async fn build_doh_config(entry: &DnsResolverConfig) -> Result<ResolverConfig> {
    let mut url = ensure_https_url(&entry.endpoint)?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/dns-query");
    }

    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("doh resolver {} missing host", entry.label))?
        .to_string();
    let port = url.port().unwrap_or(entry.effective_port());
    let ips = collect_endpoint_ips(entry, port).await?;
    if ips.is_empty() {
        return Err(anyhow!("doh resolver {} missing bootstrap ip", entry.label));
    }

    if entry.sni.is_some() && entry.sni.as_deref() != Some(host.as_str()) {
        tracing::debug!(
            target = "ip_pool",
            resolver = entry.label.as_str(),
            requested_sni = entry.sni.as_deref(),
            actual_host = host.as_str(),
            "ignoring custom SNI for DoH resolver"
        );
    }
    let group = NameServerConfigGroup::from_ips_https(&ips, port, host, true);
    Ok(ResolverConfig::from_parts(None, vec![], group))
}

async fn collect_endpoint_ips(entry: &DnsResolverConfig, port: u16) -> Result<Vec<IpAddr>> {
    let mut ips: Vec<IpAddr> = Vec::new();

    for raw in &entry.bootstrap_ips {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let ip: IpAddr = trimmed
            .parse()
            .with_context(|| format!("parse bootstrap ip '{}'", trimmed))?;
        ips.push(ip);
    }

    if ips.is_empty() {
        if let Ok(addr) = entry.endpoint.parse::<SocketAddr>() {
            ips.push(addr.ip());
        } else if let Ok(ip) = entry.endpoint.parse::<IpAddr>() {
            ips.push(ip);
        } else {
            let (host, _) = parse_host_port(&entry.endpoint, port)?;
            let resolved: Vec<IpAddr> = lookup_host((host.as_str(), port))
                .await?
                .map(|addr| addr.ip())
                .collect();
            ips.extend(resolved);
        }
    }

    ips.sort();
    ips.dedup();
    Ok(ips)
}

fn parse_host_port(endpoint: &str, default_port: u16) -> Result<(String, u16)> {
    if let Ok(socket) = endpoint.parse::<SocketAddr>() {
        return Ok((socket.ip().to_string(), socket.port()));
    }

    if let Ok(ip) = endpoint.parse::<IpAddr>() {
        return Ok((ip.to_string(), default_port));
    }

    let candidate = if endpoint.contains("://") {
        Url::parse(endpoint)
    } else {
        Url::parse(&format!("https://{}", endpoint))
    }
    .with_context(|| format!("parse dns endpoint '{}'", endpoint))?;

    let host = candidate
        .host_str()
        .ok_or_else(|| anyhow!("dns endpoint '{}' missing host", endpoint))?
        .to_string();
    let port = candidate.port().unwrap_or(default_port);
    Ok((host, port))
}

fn ensure_https_url(endpoint: &str) -> Result<Url> {
    let url = if endpoint.contains("://") {
        Url::parse(endpoint)
    } else {
        Url::parse(&format!("https://{}", endpoint))
    }
    .with_context(|| format!("parse doh endpoint '{}'", endpoint))?;

    if url.scheme() != "https" {
        return Err(anyhow!("doh endpoint '{}' must use https scheme", endpoint));
    }

    Ok(url)
}
