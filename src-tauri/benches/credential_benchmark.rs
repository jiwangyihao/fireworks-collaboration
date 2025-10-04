use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use fireworks_collaboration_lib::core::credential::{
    Credential, CredentialConfig, CredentialStore,
};
use fireworks_collaboration_lib::core::credential::storage::MemoryCredentialStore;
use std::time::SystemTime;

/// Benchmark: 添加凭证到内存存储
fn benchmark_add_credential(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_credential");
    
    // 小数据集（10个凭证）
    group.bench_function(BenchmarkId::new("memory", 10), |b| {
        b.iter(|| {
            let store = MemoryCredentialStore::new();
            for i in 0..10 {
                let cred = Credential::new(
                    black_box("github.com".to_string()),
                    black_box(format!("user{i}")),
                    black_box(format!("token_{i}")),
                );
                let _ = store.add(cred);
            }
        })
    });
    
    // 中等数据集（100个凭证）
    group.bench_function(BenchmarkId::new("memory", 100), |b| {
        b.iter(|| {
            let store = MemoryCredentialStore::new();
            for i in 0..100 {
                let cred = Credential::new(
                    black_box(format!("host{}.com", i % 10)),
                    black_box(format!("user{i}")),
                    black_box(format!("token_{i}")),
                );
                let _ = store.add(cred);
            }
        })
    });
    
    // 大数据集（1000个凭证）
    group.bench_function(BenchmarkId::new("memory", 1000), |b| {
        b.iter(|| {
            let store = MemoryCredentialStore::new();
            for i in 0..1000 {
                let cred = Credential::new(
                    black_box(format!("host{}.com", i % 100)),
                    black_box(format!("user{i}")),
                    black_box(format!("token_{i}")),
                );
                let _ = store.add(cred);
            }
        })
    });
    
    group.finish();
}

/// Benchmark: 获取凭证从内存存储
fn benchmark_get_credential(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_credential");
    
    // 准备不同大小的数据集
    for size in [10, 100, 1000].iter() {
        let store = MemoryCredentialStore::new();
        for i in 0..*size {
            let cred = Credential::new(
                format!("host{}.com", i % 10),
                format!("user{i}"),
                format!("token_{i}"),
            );
            let _ = store.add(cred);
        }
        
        group.bench_with_input(BenchmarkId::new("memory", size), size, |b, _| {
            b.iter(|| {
                let _ = store.get(black_box("host5.com"), black_box(Some("user5")));
            })
        });
    }
    
    group.finish();
}

/// Benchmark: 列举所有凭证
fn benchmark_list_credentials(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_credentials");
    
    // 准备不同大小的数据集
    for size in [10, 100, 1000].iter() {
        let store = MemoryCredentialStore::new();
        for i in 0..*size {
            let cred = Credential::new(
                format!("host{}.com", i % 10),
                format!("user{i}"),
                format!("token_{i}"),
            );
            let _ = store.add(cred);
        }
        
        group.bench_with_input(BenchmarkId::new("memory", size), size, |b, _| {
            b.iter(|| {
                let _ = store.list();
            })
        });
    }
    
    group.finish();
}

/// Benchmark: 删除凭证
fn benchmark_remove_credential(c: &mut Criterion) {
    let mut group = c.benchmark_group("remove_credential");
    
    // 准备不同大小的数据集
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("memory", size), size, |b, &size| {
            b.iter_batched(
                || {
                    // Setup: 创建存储并添加凭证
                    let store = MemoryCredentialStore::new();
                    for i in 0..size {
                        let cred = Credential::new(
                            format!("host{}.com", i % 10),
                            format!("user{i}"),
                            format!("token_{i}"),
                        );
                        let _ = store.add(cred);
                    }
                    store
                },
                |store| {
                    // Benchmark: 删除一个凭证
                    let _ = store.remove(black_box("host5.com"), black_box("user5"));
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }
    
    group.finish();
}

/// Benchmark: 凭证过期检测
fn benchmark_credential_expiry(c: &mut Criterion) {
    let mut group = c.benchmark_group("credential_expiry");
    
    // 创建已过期凭证
    let now = SystemTime::now();
    let one_day_ago = now
        .checked_sub(std::time::Duration::from_secs(86400))
        .unwrap();
    let one_day_later = now
        .checked_add(std::time::Duration::from_secs(86400))
        .unwrap();
    
    group.bench_function("is_expired_past", |b| {
        let cred = Credential::new_with_expiry(
            "github.com".to_string(),
            "user".to_string(),
            "token".to_string(),
            one_day_ago,
        );
        b.iter(|| {
            black_box(cred.is_expired());
        })
    });
    
    group.bench_function("is_expired_future", |b| {
        let cred = Credential::new_with_expiry(
            "github.com".to_string(),
            "user".to_string(),
            "token".to_string(),
            one_day_later,
        );
        b.iter(|| {
            black_box(cred.is_expired());
        })
    });
    
    group.bench_function("is_expired_none", |b| {
        let cred = Credential::new(
            "github.com".to_string(),
            "user".to_string(),
            "token".to_string(),
        );
        b.iter(|| {
            black_box(cred.is_expired());
        })
    });
    
    group.finish();
}

/// Benchmark: 清理过期凭证（模拟）
fn benchmark_cleanup_expired(c: &mut Criterion) {
    let mut group = c.benchmark_group("cleanup_expired");
    
    // 准备不同大小的数据集（50%过期）
    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("memory", size), size, |b, &size| {
            b.iter_batched(
                || {
                    // Setup: 创建存储，一半凭证已过期
                    let store = MemoryCredentialStore::new();
                    let now = SystemTime::now();
                    let past = now.checked_sub(std::time::Duration::from_secs(86400)).unwrap();
                    let future = now.checked_add(std::time::Duration::from_secs(86400)).unwrap();
                    
                    for i in 0..size {
                        let expires_at = if i % 2 == 0 { past } else { future };
                        let cred = Credential::new_with_expiry(
                            format!("host{}.com", i % 10),
                            format!("user{i}"),
                            format!("token_{i}"),
                            expires_at,
                        );
                        let _ = store.add(cred);
                    }
                    store
                },
                |store| {
                    // Benchmark: 清理过期凭证（列举 + 过滤 + 删除）
                    let all_creds = store.list().unwrap_or_default();
                    let expired: Vec<_> = all_creds.iter().filter(|c| c.is_expired()).collect();
                    for cred in expired {
                        let _ = store.remove(&cred.host, &cred.username);
                    }
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }
    
    group.finish();
}

/// Benchmark: 凭证模型创建
fn benchmark_credential_creation(c: &mut Criterion) {
    c.bench_function("credential_new", |b| {
        b.iter(|| {
            Credential::new(
                black_box("github.com".to_string()),
                black_box("user123".to_string()),
                black_box("ghp_1234567890abcdef".to_string()),
            )
        })
    });
    
    c.bench_function("credential_new_with_expiry", |b| {
        let expires_at = SystemTime::now()
            .checked_add(std::time::Duration::from_secs(90 * 86400))
            .unwrap();
        b.iter(|| {
            Credential::new_with_expiry(
                black_box("github.com".to_string()),
                black_box("user123".to_string()),
                black_box("ghp_1234567890abcdef".to_string()),
                black_box(expires_at),
            )
        })
    });
}

/// Benchmark: 凭证配置创建和验证
fn benchmark_credential_config(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(|| {
            CredentialConfig::default()
        })
    });
    
    c.bench_function("config_validate", |b| {
        let config = CredentialConfig::default();
        b.iter(|| {
            let _ = black_box(config.validate());
        })
    });
}

criterion_group!(
    benches,
    benchmark_add_credential,
    benchmark_get_credential,
    benchmark_list_credentials,
    benchmark_remove_credential,
    benchmark_credential_expiry,
    benchmark_cleanup_expired,
    benchmark_credential_creation,
    benchmark_credential_config
);
criterion_main!(benches);
