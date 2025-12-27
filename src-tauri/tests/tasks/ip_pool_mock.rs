use anyhow::{anyhow, Result};
use fireworks_collaboration_lib::core::ip_pool::dns::DnsResolvedIp;
use fireworks_collaboration_lib::core::ip_pool::preheat::{PreheatService, ProberFn, ResolverFn};
use fireworks_collaboration_lib::core::ip_pool::{
    EffectiveIpPoolConfig, IpPoolFileConfig, IpPoolRuntimeConfig,
};
use futures::future::BoxFuture;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

fn mock_resolve_impl(
    host: String,
    port: u16,
    canned: Arc<Mutex<Option<Result<Vec<IpAddr>>>>>,
) -> BoxFuture<'static, Result<Vec<DnsResolvedIp>>> {
    Box::pin(async move {
        // Use arguments to avoid unused warning if omitted
        let _ = (host, port);
        let guard = canned.lock().await;
        match guard.as_ref() {
            Some(Ok(ips)) => {
                let resolved = ips
                    .iter()
                    .map(|&ip| DnsResolvedIp { ip, label: None })
                    .collect();
                Ok(resolved)
            }
            Some(Err(e)) => Err(anyhow::anyhow!(e.to_string())),
            None => Ok(vec![]),
        }
    })
}

fn mock_dns_resolver(
    canned_response: Option<Result<Vec<IpAddr>>>,
) -> (ResolverFn, Arc<AtomicUsize>) {
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();
    let canned = Arc::new(Mutex::new(canned_response));

    let resolver: ResolverFn = Arc::new(
        move |host: &str, port: u16, _cfg| -> BoxFuture<'static, Result<Vec<DnsResolvedIp>>> {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            mock_resolve_impl(host.to_string(), port, canned.clone())
        },
    );
    (resolver, call_count)
}

fn mock_probe_impl(ip: IpAddr, fixed_latency_ms: Option<u32>) -> BoxFuture<'static, Result<u32>> {
    Box::pin(async move {
        match fixed_latency_ms {
            Some(ms) => Ok(ms),
            None => Err(anyhow::anyhow!("mock probe failure for {}", ip)),
        }
    })
}

fn mock_latency_prober(fixed_latency_ms: Option<u32>) -> (ProberFn, Arc<AtomicUsize>) {
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    let prober: ProberFn = Arc::new(
        move |ip,
              _port,
              _host,
              _sni,
              _path: &str,
              _timeout,
              _method|
              -> BoxFuture<'static, Result<u32>> {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            mock_probe_impl(ip, fixed_latency_ms)
        },
    );
    (prober, call_count)
}

#[tokio::test]
async fn test_preheat_dns_failure_backoff_mock() {
    // 1. Setup config manually (avoiding AppConfig structure mismatch)
    let mut runtime_config = IpPoolRuntimeConfig::default();
    runtime_config.sources.dns = true;
    // Disable other sources to ensure we isolate DNS testing
    runtime_config.sources.builtin = false;
    runtime_config.sources.history = false;
    runtime_config.sources.user_static = false;
    runtime_config.sources.fallback = false;

    let mut file_config = IpPoolFileConfig::default();
    file_config.preheat_domains = vec![
        fireworks_collaboration_lib::core::ip_pool::config::PreheatDomain::new("test.example.com"),
    ];

    let config = Arc::new(EffectiveIpPoolConfig::from_parts(
        runtime_config,
        file_config,
    ));

    let cache = Arc::new(fireworks_collaboration_lib::core::ip_pool::cache::IpScoreCache::new());
    let history =
        Arc::new(fireworks_collaboration_lib::core::ip_pool::history::IpHistoryStore::in_memory());

    // 2. Mock DNS Resolver to fail consistently
    let (resolver, call_count) = mock_dns_resolver(Some(Err(anyhow!("simulated dns error"))));
    // Prober shouldn't be called if DNS fails (and no other sources)
    let (prober, prober_count) = mock_latency_prober(Some(50));

    // 3. Spawn PreheatService manually
    let preheater = PreheatService::spawn(
        config.clone(),
        cache.clone(),
        history.clone(),
        Some(resolver),
        Some(prober),
    )
    .expect("spawn preheater");

    // 4. Trigger refresh and wait
    preheater.request_refresh();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // 5. Verification
    // DNS should be called at least once
    assert!(
        call_count.load(Ordering::SeqCst) >= 1,
        "DNS resolver should be called"
    );
    // Cache should be empty for this host
    assert!(cache.get("test.example.com", 443).is_none());
    // Prober should NOT be called because candidate collection failed
    assert_eq!(
        prober_count.load(Ordering::SeqCst),
        0,
        "Prober should not be called if DNS fails"
    );
}

#[tokio::test]
async fn test_preheat_probe_success_updates_cache_mock() {
    let mut runtime_config = IpPoolRuntimeConfig::default();
    runtime_config.sources.dns = true;
    runtime_config.sources.builtin = false;
    runtime_config.sources.history = false;
    runtime_config.sources.user_static = false;
    runtime_config.sources.fallback = false;

    let mut file_config = IpPoolFileConfig::default();
    file_config.preheat_domains = vec![
        fireworks_collaboration_lib::core::ip_pool::config::PreheatDomain::new(
            "success.example.com",
        ),
    ];

    let config = Arc::new(EffectiveIpPoolConfig::from_parts(
        runtime_config,
        file_config,
    ));

    let cache = Arc::new(fireworks_collaboration_lib::core::ip_pool::cache::IpScoreCache::new());
    let history =
        Arc::new(fireworks_collaboration_lib::core::ip_pool::history::IpHistoryStore::in_memory());

    // Mock DNS returning 1.1.1.1
    let ip: IpAddr = "1.1.1.1".parse().unwrap();
    let (resolver, _) = mock_dns_resolver(Some(Ok(vec![ip])));
    // Mock Prober returning 20ms latency
    let (prober, prober_count) = mock_latency_prober(Some(20));

    let preheater = PreheatService::spawn(
        config.clone(),
        cache.clone(),
        history.clone(),
        Some(resolver),
        Some(prober),
    )
    .expect("spawn preheater");

    preheater.request_refresh();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Verify cache update
    let snapshot = cache.get("success.example.com", 443);
    assert!(snapshot.is_some(), "Cache should contain entry");
    let slot = snapshot.unwrap();
    assert!(slot.best.is_some());
    assert_eq!(slot.best.as_ref().unwrap().candidate.address, ip);
    assert_eq!(slot.best.as_ref().unwrap().latency_ms.unwrap(), 20);

    assert!(prober_count.load(Ordering::SeqCst) >= 1);
}
