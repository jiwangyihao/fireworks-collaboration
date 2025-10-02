use fireworks_collaboration_lib::core::ip_pool::config::EffectiveIpPoolConfig;
use fireworks_collaboration_lib::core::ip_pool::manager::IpPool;
use fireworks_collaboration_lib::core::ip_pool::{
    IpCandidate, IpOutcome, IpSelection, IpSource, IpStat,
};
use std::net::{IpAddr, Ipv4Addr};

fn make_stat(addr: [u8; 4], port: u16) -> IpStat {
    let candidate = IpCandidate::new(
        IpAddr::from(Ipv4Addr::from(addr)),
        port,
        IpSource::UserStatic,
    );
    let mut stat = IpStat::with_latency(candidate, 12);
    stat.measured_at_epoch_ms = Some(1);
    stat.expires_at_epoch_ms = Some(i64::MAX - 1);
    stat.sources = vec![IpSource::UserStatic];
    stat
}

#[test]
fn report_candidate_outcome_tracks_per_ip() {
    let mut cfg = EffectiveIpPoolConfig::default();
    cfg.runtime.enabled = true;
    let pool = IpPool::new(cfg);
    let stat = make_stat([127, 0, 0, 2], 443);
    pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Failure);
    pool.report_candidate_outcome("example.com", 443, &stat, IpOutcome::Success);
    let metrics = pool
        .candidate_outcome_metrics("example.com", 443, stat.candidate.address)
        .expect("candidate metrics present");
    assert_eq!(metrics.success, 1);
    assert_eq!(metrics.failure, 1);
    assert_eq!(metrics.last_sources, vec![IpSource::UserStatic]);
}

#[test]
fn report_outcome_updates_aggregate_counts() {
    let mut cfg = EffectiveIpPoolConfig::default();
    cfg.runtime.enabled = true;
    let pool = IpPool::new(cfg);
    let stat = make_stat([127, 0, 0, 3], 443);
    let selection = IpSelection::from_cached("example.com", 443, stat.clone());
    pool.report_outcome(&selection, IpOutcome::Success);
    let aggregate = pool
        .outcome_metrics("example.com", 443)
        .expect("aggregate metrics present");
    assert_eq!(aggregate.success, 1);
    assert_eq!(aggregate.failure, 0);
    assert!(aggregate.last_outcome_ms > 0);
}
