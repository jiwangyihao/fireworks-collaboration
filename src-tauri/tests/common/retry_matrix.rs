//! Retry / Backoff 参数矩阵骨架 (Pre-12.9)
//! ----------------------------------------
//! 目的：集中列举 push/retry 相关参数组合，后续在 `git_push_and_retry.rs` 中直接引用。
//! 范围：仅定义枚举与结构；不包含真实计时或 sleep 行为；测试中通过注入逻辑时间或 mock 计数器验证。
//! 未来扩展：
//!   * BackoffKind::Jitter 实现随机策略时，测试改为断言单调递增 + 上界。
//!   * Policy 可扩展自定义策略（如指数退避上限 / 快速失败阈值）。
//!   * 将 attempts>1 且策略=Constant 与 attempts=1 场景合并对比。

#![allow(dead_code)]

use crate::common::CaseDescribe;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy)]
pub enum BackoffKind {
    Constant, // 固定间隔
    Linear,   // 线性递增 (base * n)
    Exponential, // 指数递增 (base * 2^n)
              // Jitter,    // 预留：带抖动随机分布
}

#[derive(Debug, Clone, Copy)]
pub enum PolicyOverride {
    None,
    ForceSuccessEarly, // 模拟策略：在第 k 次前直接成功（降低测试时长）
    AbortAfter(u8),    // 在指定次数后主动放弃
}

#[derive(Debug, Clone, Copy)]
pub struct RetryCase {
    pub attempts: u8,       // 最大尝试次数
    pub base_delay_ms: u64, // 逻辑基准延迟（不真实 sleep）
    pub backoff: BackoffKind,
    pub policy: PolicyOverride,
}

impl Display for RetryCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Retry(attempts={},base={},backoff={:?},policy={:?})",
            self.attempts, self.base_delay_ms, self.backoff, self.policy
        )
    }
}

impl RetryCase {
    /// 便于参数化测试/日志的紧凑描述。
    pub fn describe(&self) -> String {
        format!(
            "a{}-b{}-{:?}-{}",
            self.attempts,
            self.base_delay_ms,
            self.backoff,
            self.policy_code()
        )
    }
    /// 根据策略估算实际可执行尝试次数（AbortAfter(n) -> n-1 尝试，最少 0）。
    pub fn effective_attempts(&self) -> u8 {
        match self.policy {
            PolicyOverride::AbortAfter(n) => n.saturating_sub(1).min(self.attempts),
            _ => self.attempts,
        }
    }
    /// 策略是否为 Abort。
    pub fn is_abort(&self) -> bool {
        matches!(self.policy, PolicyOverride::AbortAfter(_))
    }
    /// 策略是否为提前成功。
    pub fn is_force_success(&self) -> bool {
        matches!(self.policy, PolicyOverride::ForceSuccessEarly)
    }
    fn policy_code(&self) -> &'static str {
        match self.policy {
            PolicyOverride::None => "N",
            PolicyOverride::ForceSuccessEarly => "F",
            PolicyOverride::AbortAfter(_) => "A",
        }
    }
}

impl CaseDescribe for RetryCase {
    fn describe(&self) -> String {
        self.describe()
    }
}

/// 返回一组代表性 retry 组合（避免组合爆炸）。
/// 说明：
///   * attempts: 1 表示无重试；3 表示常见短重试；5 作为上界代表。
///   * base_delay_ms: 按逻辑值区分（无需真实等待），事件可记录这些延迟值供断言。
///   * backoff: 选取 Constant/Linear/Exponential；Jitter 未来添加并以属性断言。
///   * policy: 仅列举 None + Abort/ForceSuccessEarly 两种干扰策略代表。
pub fn retry_cases() -> Vec<RetryCase> {
    use BackoffKind::*;
    use PolicyOverride::*;
    vec![
        RetryCase {
            attempts: 1,
            base_delay_ms: 50,
            backoff: Constant,
            policy: None,
        },
        RetryCase {
            attempts: 3,
            base_delay_ms: 100,
            backoff: Constant,
            policy: None,
        },
        RetryCase {
            attempts: 3,
            base_delay_ms: 80,
            backoff: Linear,
            policy: None,
        },
        RetryCase {
            attempts: 3,
            base_delay_ms: 60,
            backoff: Exponential,
            policy: None,
        },
        RetryCase {
            attempts: 5,
            base_delay_ms: 40,
            backoff: Exponential,
            policy: AbortAfter(3),
        },
        RetryCase {
            attempts: 5,
            base_delay_ms: 40,
            backoff: Linear,
            policy: ForceSuccessEarly,
        },
    ]
}

/// 逻辑计算下一次延迟（不真实等待），供测试中对 backoff 序列进行期望生成。
pub fn compute_backoff_sequence(case: &RetryCase) -> Vec<u64> {
    let mut seq = Vec::new();
    for n in 0..case.attempts {
        // n 表示第 n 次尝试前的等待
        let delay = match case.backoff {
            BackoffKind::Constant => case.base_delay_ms,
            BackoffKind::Linear => case.base_delay_ms * (n as u64 + 1),
            BackoffKind::Exponential => case.base_delay_ms * (1u64 << n),
        };
        seq.push(delay);
    }
    seq
}

/// 计算理论总逻辑延迟（不考虑提前成功/中止实际缩短，供上层做上界断言）。
pub fn total_delay(case: &RetryCase) -> u64 {
    compute_backoff_sequence(case).into_iter().sum()
}

#[cfg(test)]
mod tests_internal {
    use super::*;
    #[test]
    fn backoff_sequence_shapes() {
        for c in retry_cases() {
            let seq = compute_backoff_sequence(&c);
            assert_eq!(seq.len() as u8, c.attempts, "len mismatch for {c}");
            // 轻量趋势校验（允许 Equal 用于 Constant）。
            for w in seq.windows(2) {
                if !matches!(c.backoff, BackoffKind::Constant) {
                    assert!(w[1] >= w[0], "non-monotonic for {c} => {:?}", seq);
                }
            }
        }
    }

    #[test]
    fn describe_unique_and_effective_attempts_bounds() {
        let cases = retry_cases();
        let _descs = crate::common::assert_unique_describe(&cases);
        for c in cases {
            assert!(c.effective_attempts() <= c.attempts);
        }
    }

    #[test]
    fn total_delay_non_zero_when_attempts_gt0() {
        for c in retry_cases() {
            if c.attempts > 0 {
                assert!(total_delay(&c) > 0);
            }
        }
    }
}
