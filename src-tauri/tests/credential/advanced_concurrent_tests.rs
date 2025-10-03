//! 高级并发和竞态条件测试
//!
//! 测试多线程环境下的各种竞态场景。

use fireworks_collaboration_lib::core::credential::{
    config::CredentialConfig,
    file_store::EncryptedFileStore,
    model::Credential,
    storage::{CredentialStore, MemoryCredentialStore},
};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;

fn get_test_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("fireworks_concurrent_test_{}.enc", name))
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_concurrent_add_same_credential() {
    let store = Arc::new(MemoryCredentialStore::new());
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 10 个线程同时尝试添加相同的凭证
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // 等待所有线程就绪
            barrier_clone.wait();

            let cred = Credential::new(
                "github.com".to_string(),
                "alice".to_string(),
                format!("token_from_thread_{}", i),
            );

            // 尝试添加，只有一个应该成功
            store_clone.add(cred)
        });
        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("线程应该完成"))
        .collect();

    // 应该只有一个成功，其他的失败（重复凭证）
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    let failure_count = results.iter().filter(|r| r.is_err()).count();

    assert_eq!(success_count, 1, "应该只有一个线程成功添加");
    assert_eq!(failure_count, 9, "其他线程应该失败（重复）");

    // 验证只有一个凭证
    let list = store.list().expect("应该列出凭证");
    assert_eq!(list.len(), 1, "应该只有一个凭证");
}

#[test]
fn test_concurrent_read_write_same_credential() {
    let store = Arc::new(MemoryCredentialStore::new());

    // 预先添加一个凭证
    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        "initial_password".to_string(),
    );
    store.add(cred).expect("应该添加初始凭证");

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 5 个读线程 + 5 个写线程
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            if i < 5 {
                // 读线程
                for _ in 0..100 {
                    let _ = store_clone.get("test.com", Some("user"));
                }
            } else {
                // 写线程（尝试更新 last_used）
                for _ in 0..100 {
                    let _ = store_clone.update_last_used("test.com", "user");
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }

    // 验证凭证仍然存在且未损坏
    let retrieved = store.get("test.com", Some("user"))
        .expect("应该能查询")
        .expect("凭证应该存在");

    assert_eq!(retrieved.host, "test.com");
    assert_eq!(retrieved.username, "user");
}

#[test]
fn test_concurrent_add_and_remove() {
    let store = Arc::new(MemoryCredentialStore::new());
    let barrier = Arc::new(Barrier::new(20));
    let mut handles = vec![];

    // 10 个添加线程 + 10 个删除线程
    for i in 0..20 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let host = format!("host{}.com", i % 10);
            let username = format!("user{}", i % 10);

            if i < 10 {
                // 添加线程
                let cred = Credential::new(
                    host,
                    username,
                    format!("token{}", i),
                );
                let _ = store_clone.add(cred);
            } else {
                // 删除线程（尝试删除对应的凭证）
                thread::sleep(std::time::Duration::from_millis(10)); // 稍微延迟
                let _ = store_clone.remove(&host, &username);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }

    // 验证存储状态一致（可能有 0 到 10 个凭证，取决于时序）
    let list = store.list().expect("应该列出凭证");
    assert!(list.len() <= 10, "凭证数量应该合理");
}

#[test]
fn test_concurrent_list_operations() {
    let store = Arc::new(MemoryCredentialStore::new());

    // 预先添加一些凭证
    for i in 0..50 {
        let cred = Credential::new(
            format!("host{}.com", i),
            format!("user{}", i),
            format!("token{}", i),
        );
        store.add(cred).expect("应该添加凭证");
    }

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 10 个线程同时执行 list 操作
    for _ in 0..10 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for _ in 0..100 {
                let list = store_clone.list().expect("应该列出凭证");
                assert_eq!(list.len(), 50, "列表长度应该一致");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }
}

#[test]
fn test_concurrent_file_store_operations() {
    let test_file = get_test_file("concurrent_file_ops");
    cleanup(&test_file);

    let config = CredentialConfig::new()
        .with_file_path(test_file.to_string_lossy().to_string());

    let store = Arc::new(
        EncryptedFileStore::new(&config).expect("应该创建文件存储")
    );
    store.set_master_password("concurrent_test".to_string())
        .expect("应该设置主密码");

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 10 个线程并发操作文件存储
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // 每个线程添加 5 个凭证
            for j in 0..5 {
                let cred = Credential::new(
                    format!("host{}_{}.com", i, j),
                    format!("user{}_{}", i, j),
                    format!("token_{}_{}", i, j),
                );
                
                let result = store_clone.add(cred);
                // 由于文件锁，可能有些操作会失败
                if result.is_err() {
                    // 重试一次
                    thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }

    // 验证文件未损坏且凭证可访问
    let list = store.list().expect("文件应该仍然有效");
    assert!(list.len() > 0, "应该至少添加了一些凭证");

    cleanup(&test_file);
}

#[test]
fn test_concurrent_get_nonexistent() {
    let store = Arc::new(MemoryCredentialStore::new());
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 10 个线程同时查询不存在的凭证
    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for _ in 0..100 {
                let result = store_clone.get(
                    &format!("nonexistent{}.com", i),
                    Some(&format!("user{}", i))
                );

                assert!(result.is_ok(), "查询不存在的凭证应该返回 Ok(None)");
                assert!(result.unwrap().is_none(), "应该返回 None");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }
}

#[test]
fn test_stress_many_threads_many_operations() {
    let store = Arc::new(MemoryCredentialStore::new());
    let mut handles = vec![];

    // 50 个线程，每个执行 100 次随机操作
    for i in 0..50 {
        let store_clone = Arc::clone(&store);

        let handle = thread::spawn(move || {
            for j in 0..100 {
                let op = (i + j) % 4;

                match op {
                    0 => {
                        // 添加
                        let cred = Credential::new(
                            format!("host{}_{}.com", i, j),
                            format!("user{}_{}", i, j),
                            format!("token_{}_{}", i, j),
                        );
                        let _ = store_clone.add(cred);
                    }
                    1 => {
                        // 查询
                        let _ = store_clone.get(
                            &format!("host{}_{}.com", i, j),
                            Some(&format!("user{}_{}", i, j))
                        );
                    }
                    2 => {
                        // 更新 last_used
                        let _ = store_clone.update_last_used(
                            &format!("host{}_{}.com", i, j),
                            &format!("user{}_{}", i, j)
                        );
                    }
                    3 => {
                        // 列表
                        let _ = store_clone.list();
                    }
                    _ => unreachable!()
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("线程应该完成");
    }

    // 验证存储仍然正常工作
    let list = store.list().expect("应该能列出凭证");
    println!("压力测试后凭证数量: {}", list.len());
}

#[test]
fn test_concurrent_remove_same_credential() {
    let store = Arc::new(MemoryCredentialStore::new());

    // 预先添加凭证
    let cred = Credential::new(
        "test.com".to_string(),
        "user".to_string(),
        "password".to_string(),
    );
    store.add(cred).expect("应该添加凭证");

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    // 10 个线程同时尝试删除同一个凭证
    for _ in 0..10 {
        let store_clone = Arc::clone(&store);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();
            store_clone.remove("test.com", "user")
        });
        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("线程应该完成"))
        .collect();

    // 应该只有一个成功删除，其他的失败（凭证不存在）
    let success_count = results.iter().filter(|r| r.is_ok()).count();

    assert_eq!(success_count, 1, "应该只有一个线程成功删除");

    // 验证凭证确实被删除
    let retrieved = store.get("test.com", Some("user"))
        .expect("应该能查询");
    assert!(retrieved.is_none(), "凭证应该已被删除");
}
