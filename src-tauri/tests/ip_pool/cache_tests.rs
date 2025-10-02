use fireworks_collaboration_lib::core::ip_pool::cache::{IpCacheKey, IpCacheSlot, IpScoreCache};
use fireworks_collaboration_lib::core::ip_pool::{IpCandidate, IpSource, IpStat};
use std::net::{IpAddr, Ipv4Addr};

#[test]
fn cache_insert_and_get_best() {
    let cache = IpScoreCache::new();
    let key = IpCacheKey::new("github.com", 443);
    let stat = IpStat::with_latency(
        IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            443,
            IpSource::Builtin,
        ),
        42,
    );
    cache.insert(key.clone(), IpCacheSlot::with_best(stat.clone()));
    let fetched = cache.get("github.com", 443).unwrap();
    assert_eq!(fetched.best.as_ref().unwrap().latency_ms, Some(42));
    assert_eq!(
        fetched.best.unwrap().candidate.address,
        stat.candidate.address
    );
    assert_eq!(stat.sources, vec![IpSource::Builtin]);
    // 确保 snapshot 复制，而不是共享引用
    let snapshot = cache.snapshot();
    assert!(snapshot.contains_key(&key));
}

#[test]
fn cache_remove_and_clear() {
    let cache = IpScoreCache::new();
    cache.insert(
        IpCacheKey::new("github.com", 443),
        IpCacheSlot::with_best(IpStat::with_latency(
            IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                443,
                IpSource::Builtin,
            ),
            10,
        )),
    );
    cache.remove("github.com", 443);
    assert!(cache.get("github.com", 443).is_none());
    cache.insert(
        IpCacheKey::new("github.com", 80),
        IpCacheSlot::with_best(IpStat::with_latency(
            IpCandidate::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80, IpSource::Dns),
            5,
        )),
    );
    cache.clear();
    assert!(cache.get("github.com", 80).is_none());
}
