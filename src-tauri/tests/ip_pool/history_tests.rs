use fireworks_collaboration_lib::core::ip_pool::history::{IpHistoryRecord, IpHistoryStore};
use fireworks_collaboration_lib::core::ip_pool::{IpCandidate, IpSource};
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use uuid::Uuid;

#[test]
fn load_or_init_creates_file() {
    let dir = std::env::temp_dir().join(format!("ip-history-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let store = IpHistoryStore::load_or_init_at(&dir).expect("load history");
    let path = IpHistoryStore::join_history_path(&dir);
    assert!(path.exists());
    assert!(store.snapshot().unwrap().is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn upsert_and_get_roundtrip() {
    let dir = std::env::temp_dir().join(format!("ip-history-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let store = IpHistoryStore::load_or_init_at(&dir).expect("load history");
    let record = IpHistoryRecord {
        host: "github.com".into(),
        port: 443,
        candidate: IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            443,
            IpSource::Builtin,
        ),
        sources: vec![IpSource::Builtin, IpSource::Dns],
        latency_ms: 32,
        measured_at_epoch_ms: 1,
        expires_at_epoch_ms: 2,
    };
    store.upsert(record.clone()).expect("write history");
    let fetched = store.get("github.com", 443).expect("history entry");
    assert_eq!(fetched.latency_ms, 32);
    assert_eq!(fetched.sources, vec![IpSource::Builtin, IpSource::Dns]);
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.len(), 1);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_or_init_from_file_creates_parent_dirs() {
    let base = std::env::temp_dir().join(format!("ip-history-file-{}", Uuid::new_v4()));
    let path = base.join("nested").join("custom-history.json");
    let store = IpHistoryStore::load_or_init_from_file(&path).expect("load history file");
    assert!(path.exists());
    assert!(store.snapshot().unwrap().is_empty());
    fs::remove_dir_all(&base).ok();
}

#[test]
fn get_fresh_evicts_expired_records() {
    let store = IpHistoryStore::in_memory();
    let record = IpHistoryRecord {
        host: "github.com".into(),
        port: 443,
        candidate: IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            443,
            IpSource::Builtin,
        ),
        sources: vec![IpSource::Builtin],
        latency_ms: 10,
        measured_at_epoch_ms: 1,
        expires_at_epoch_ms: 5,
    };
    store.upsert(record).expect("write history");
    let missing = store.get_fresh("github.com", 443, 10);
    assert!(missing.is_none());
    assert!(store.snapshot().unwrap().is_empty());
}

#[test]
fn get_fresh_returns_valid_record() {
    let store = IpHistoryStore::in_memory();
    let record = IpHistoryRecord {
        host: "github.com".into(),
        port: 443,
        candidate: IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            443,
            IpSource::Builtin,
        ),
        sources: vec![IpSource::Builtin, IpSource::History],
        latency_ms: 8,
        measured_at_epoch_ms: 1,
        expires_at_epoch_ms: 10_000,
    };
    store.upsert(record.clone()).expect("write history");
    let fetched = store
        .get_fresh("github.com", 443, 5_000)
        .expect("fresh record");
    assert_eq!(fetched.latency_ms, record.latency_ms);
    assert_eq!(fetched.sources, record.sources);
    assert_eq!(store.snapshot().unwrap().len(), 1);
}

#[test]
fn remove_clears_matching_entry() {
    let store = IpHistoryStore::in_memory();
    let record = IpHistoryRecord {
        host: "github.com".into(),
        port: 443,
        candidate: IpCandidate::new(
            IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            443,
            IpSource::Builtin,
        ),
        sources: vec![IpSource::Builtin],
        latency_ms: 10,
        measured_at_epoch_ms: 1,
        expires_at_epoch_ms: 10_000,
    };
    store.upsert(record).expect("write history");
    assert!(store.remove("github.com", 443).expect("remove entry"));
    assert!(store.get("github.com", 443).is_none());
    assert!(!store.remove("github.com", 443).expect("idempotent remove"));
}

#[test]
fn enforce_capacity_removes_oldest_entries() {
    let store = IpHistoryStore::in_memory();
    for i in 0..5 {
        let record = IpHistoryRecord {
            host: format!("host{}.com", i),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, i as u8 + 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10 + i as u32,
            measured_at_epoch_ms: (i + 1) as i64 * 1000,
            expires_at_epoch_ms: 100_000,
        };
        store.upsert(record).expect("write history");
    }
    let removed = store.enforce_capacity(3).expect("enforce capacity");
    assert_eq!(removed, 2);
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.len(), 3);
    // Should keep the 3 newest (host2, host3, host4)
    assert!(snapshot.iter().all(|e| e.host.starts_with("host")
        && (e.host == "host2.com" || e.host == "host3.com" || e.host == "host4.com")));
}

#[test]
fn prune_and_enforce_removes_expired_and_old_entries() {
    let store = IpHistoryStore::in_memory();
    // Add 2 expired entries
    for i in 0..2 {
        let record = IpHistoryRecord {
            host: format!("expired{}.com", i),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(1, 1, 1, i as u8 + 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 10,
            measured_at_epoch_ms: 1000,
            expires_at_epoch_ms: 5000,
        };
        store.upsert(record).expect("write history");
    }
    // Add 4 valid entries
    for i in 0..4 {
        let record = IpHistoryRecord {
            host: format!("valid{}.com", i),
            port: 443,
            candidate: IpCandidate::new(
                IpAddr::V4(Ipv4Addr::new(2, 2, 2, i as u8 + 1)),
                443,
                IpSource::Builtin,
            ),
            sources: vec![IpSource::Builtin],
            latency_ms: 20,
            measured_at_epoch_ms: (i + 1) as i64 * 2000 + 10000,
            expires_at_epoch_ms: 100_000,
        };
        store.upsert(record).expect("write history");
    }
    let (expired, capacity_pruned) = store
        .prune_and_enforce(10_000, 3)
        .expect("prune and enforce");
    assert_eq!(expired, 2);
    assert_eq!(capacity_pruned, 1);
    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.len(), 3);
    // Should only have valid entries, and the 3 newest
    assert!(snapshot.iter().all(|e| e.host.starts_with("valid")));
}
