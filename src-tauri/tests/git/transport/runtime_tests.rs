use fireworks_collaboration_lib::core::git::transport::runtime::{
    is_fake_disabled, record_fake_attempt, AutoDisableConfig, AutoDisableEvent,
};
use std::time::Duration;

fn cfg(threshold: u8, cooldown: u64) -> AutoDisableConfig {
    AutoDisableConfig {
        threshold_pct: threshold,
        cooldown_sec: cooldown,
    }
}

// 注意：由于 record_fake_attempt_with_now, test_auto_disable_guard, test_reset_auto_disable
// 和 test_metric_counter_values 等函数是内部实现，我们需要通过 public API 来测试
// 或者将这些函数暴露为 public

#[test]
fn auto_disable_triggers_when_ratio_exceeds_threshold() {
    // 使用 testing 模块中的公共测试辅助函数
    use fireworks_collaboration_lib::core::git::transport::testing::{
        auto_disable_guard, reset_auto_disable,
    };
    
    let _guard = auto_disable_guard().lock().unwrap();
    reset_auto_disable();
    
    let cfg = cfg(50, 30);
    
    // 记录4次成功
    for _ in 0..4 {
        let _ = record_fake_attempt(&cfg, false);
    }
    
    // 记录失败直到触发（需要8个样本中4个失败才能达到50%）
    for i in 0..4 {
        let evt = record_fake_attempt(&cfg, true);
        if i < 3 {
            assert!(evt.is_none(), "expected no trigger on failure {}", i + 1);
        } else {
            assert!(matches!(
                evt,
                Some(AutoDisableEvent::Triggered {
                    threshold_pct: 50,
                    cooldown_secs: 30
                })
            ));
        }
    }
    
    assert!(is_fake_disabled(&cfg));
}

#[test]
fn auto_disable_recovers_after_cooldown() {
    use fireworks_collaboration_lib::core::git::transport::testing::{
        auto_disable_guard, reset_auto_disable,
    };
    
    let _guard = auto_disable_guard().lock().unwrap();
    reset_auto_disable();
    
    let cfg = cfg(50, 1); // 1 second cooldown
    
    // 触发自动禁用
    for _ in 0..5 {
        let _ = record_fake_attempt(&cfg, false);
    }
    for _ in 0..5 {
        let evt = record_fake_attempt(&cfg, true);
        if evt.is_some() {
            break;
        }
    }
    
    assert!(is_fake_disabled(&cfg));
    
    // 等待冷却时间（稍长一点确保超时）
    std::thread::sleep(Duration::from_millis(1100));
    
    // 冷却后应该恢复
    let evt = record_fake_attempt(&cfg, false);
    assert!(matches!(evt, Some(AutoDisableEvent::Recovered)));
    assert!(!is_fake_disabled(&cfg));
}

#[test]
fn disabled_feature_returns_none() {
    use fireworks_collaboration_lib::core::git::transport::testing::{
        auto_disable_guard, reset_auto_disable,
    };
    
    let _guard = auto_disable_guard().lock().unwrap();
    reset_auto_disable();
    
    let cfg = cfg(0, 30); // threshold=0 表示禁用该功能
    assert!(!is_fake_disabled(&cfg));
    assert!(record_fake_attempt(&cfg, true).is_none());
}
