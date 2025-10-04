use rand::Rng;

use crate::core::{
    config::{loader::load_or_init, model::RetryCfg},
    git::errors::{ErrorCategory, GitError},
};

#[derive(Debug, Clone)]
pub struct RetryPlan {
    pub max: u32,
    pub base_ms: u64,
    pub factor: f64,
    pub jitter: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryDiff {
    pub changed: Vec<&'static str>,
}

/// 计算新旧重试策略差异；返回 (diff, `changed_flag`)
pub fn compute_retry_diff(old: &RetryPlan, new: &RetryPlan) -> (RetryDiff, bool) {
    let mut changed = Vec::new();
    if old.max != new.max {
        changed.push("max");
    }
    if old.base_ms != new.base_ms {
        changed.push("baseMs");
    }
    if (old.factor - new.factor).abs() > f64::EPSILON {
        changed.push("factor");
    }
    if old.jitter != new.jitter {
        changed.push("jitter");
    }
    let changed_flag = !changed.is_empty();
    (RetryDiff { changed }, changed_flag)
}

impl From<RetryCfg> for RetryPlan {
    fn from(c: RetryCfg) -> Self {
        Self {
            max: c.max,
            base_ms: c.base_ms,
            factor: c.factor,
            jitter: c.jitter,
        }
    }
}

pub fn load_retry_plan() -> RetryPlan {
    match load_or_init() {
        Ok(cfg) => cfg.retry.into(),
        Err(_) => RetryPlan {
            max: 3,
            base_ms: 300,
            factor: 1.5,
            jitter: true,
        },
    }
}

pub fn is_retryable(err: &GitError) -> bool {
    let (cat, msg) = match err {
        GitError::Categorized { category, message } => (*category, message.to_ascii_lowercase()),
    };
    match cat {
        ErrorCategory::Network => true,
        ErrorCategory::Protocol => {
            // 粗略判断是否为 5xx 类错误
            msg.contains(" 5")
                || msg.contains("http 5")
                || msg.contains(" 50")
                || msg.contains(" 51")
                || msg.contains(" 52")
                || msg.contains(" 53")
                || msg.contains(" 54")
                || msg.contains(" 55")
                || msg.contains(" 56")
                || msg.contains(" 57")
                || msg.contains(" 58")
                || msg.contains(" 59")
        }
        _ => false,
    }
}

pub fn categorize(err: &GitError) -> ErrorCategory {
    match err {
        GitError::Categorized { category, .. } => *category,
    }
}

/// Exponential backoff with optional jitter. `attempt_idx` starts from 0 for the first retry.
pub fn backoff_delay_ms(plan: &RetryPlan, attempt_idx: u32) -> u64 {
    let pow = plan.factor.powi(attempt_idx as i32);
    let base = (plan.base_ms as f64 * pow).round() as u64;
    if plan.jitter {
        // ±50% jitter
        let low = (base as f64 * 0.5) as u64;
        let high = (base as f64 * 1.5) as u64;
        if low >= high {
            base
        } else {
            rand::thread_rng().gen_range(low..=high)
        }
    } else {
        base
    }
}
