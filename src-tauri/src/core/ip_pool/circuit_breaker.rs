//! IP 池熔断器模块
//!
//! 为每个 IP 维护失败统计与熔断状态，当失败率超过阈值时自动触发熔断，
//! 冷却期结束后自动恢复。熔断期间该 IP 将被临时拉黑，不参与候选选择。

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

use super::preheat::current_epoch_ms;

/// 熔断器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// 正常状态，允许使用该 IP
    Normal,
    /// 熔断打开状态，该 IP 被临时拉黑
    Open,
    /// 冷却状态，等待自动恢复
    Cooldown,
}

/// 单个 IP 的熔断状态跟踪
#[derive(Debug, Clone)]
struct IpCircuitState {
    /// 当前状态
    state: CircuitState,
    /// 熔断打开时的时间戳（毫秒）
    opened_at_ms: i64,
    /// 进入冷却期的时间戳（毫秒）
    cooldown_at_ms: i64,
    /// 窗口内的失败计数
    window_failures: u32,
    /// 窗口内的成功计数
    window_successes: u32,
    /// 窗口开始时间戳（毫秒）
    window_start_ms: i64,
    /// 连续失败次数（不计时间窗口）
    consecutive_failures: u32,
}

impl Default for IpCircuitState {
    fn default() -> Self {
        Self {
            state: CircuitState::Normal,
            opened_at_ms: 0,
            cooldown_at_ms: 0,
            window_failures: 0,
            window_successes: 0,
            window_start_ms: current_epoch_ms(),
            consecutive_failures: 0,
        }
    }
}

impl IpCircuitState {
    /// 检查是否应该触发熔断
    fn should_trip(&self, config: &CircuitBreakerConfig) -> bool {
        // 条件1: 连续失败次数超过阈值
        if self.consecutive_failures >= config.consecutive_failure_threshold {
            return true;
        }

        // 条件2: 窗口内失败率超过阈值（需要有足够的样本）
        let total = self.window_failures + self.window_successes;
        if total >= config.min_samples_in_window {
            let failure_rate = self.window_failures as f64 / total as f64;
            if failure_rate >= config.failure_rate_threshold {
                return true;
            }
        }

        false
    }

    /// 检查是否应该从冷却期恢复
    fn should_reset(&self, now_ms: i64, cooldown_ms: i64) -> bool {
        matches!(self.state, CircuitState::Cooldown)
            && now_ms >= self.cooldown_at_ms + cooldown_ms
    }

    /// 重置窗口统计
    fn reset_window(&mut self, now_ms: i64) {
        self.window_failures = 0;
        self.window_successes = 0;
        self.window_start_ms = now_ms;
    }

    /// 检查窗口是否过期
    fn is_window_expired(&self, now_ms: i64, window_ms: i64) -> bool {
        now_ms >= self.window_start_ms + window_ms
    }
}

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 启用熔断器
    pub enabled: bool,
    /// 连续失败次数阈值（触发熔断）
    pub consecutive_failure_threshold: u32,
    /// 失败率阈值（0.0-1.0）
    pub failure_rate_threshold: f64,
    /// 时间窗口大小（秒）
    pub window_seconds: u32,
    /// 窗口内最小样本数（用于计算失败率）
    pub min_samples_in_window: u32,
    /// 冷却时间（秒）
    pub cooldown_seconds: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            consecutive_failure_threshold: 3,
            failure_rate_threshold: 0.5,
            window_seconds: 60,
            min_samples_in_window: 5,
            cooldown_seconds: 300,
        }
    }
}

impl CircuitBreakerConfig {
    fn window_ms(&self) -> i64 {
        self.window_seconds as i64 * 1000
    }

    fn cooldown_ms(&self) -> i64 {
        self.cooldown_seconds as i64 * 1000
    }
}

/// IP 熔断器管理器
pub struct CircuitBreaker {
    config: Arc<Mutex<CircuitBreakerConfig>>,
    states: Arc<Mutex<HashMap<IpAddr, IpCircuitState>>>,
}

impl CircuitBreaker {
    /// 创建新的熔断器实例
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 更新配置
    pub fn set_config(&self, config: CircuitBreakerConfig) {
        if let Ok(mut cfg) = self.config.lock() {
            *cfg = config;
        }
    }

    /// 获取配置副本
    pub fn get_config(&self) -> CircuitBreakerConfig {
        self.config
            .lock()
            .map(|cfg| cfg.clone())
            .unwrap_or_default()
    }

    /// 检查 IP 是否被熔断（不可用）
    pub fn is_tripped(&self, ip: IpAddr) -> bool {
        let now_ms = current_epoch_ms();
        let config = match self.config.lock() {
            Ok(cfg) => cfg.clone(),
            Err(_) => return false,
        };

        if !config.enabled {
            return false;
        }

        let mut states = match self.states.lock() {
            Ok(s) => s,
            Err(_) => return false,
        };

        let state = states.entry(ip).or_default();

        // 检查是否应该从冷却期恢复
        if state.should_reset(now_ms, config.cooldown_ms()) {
            crate::core::ip_pool::events::emit_ip_pool_ip_recovered(ip);
            tracing::info!(
                target: "ip_pool",
                ip = %ip,
                "circuit breaker reset: ip recovered from cooldown"
            );
            *state = IpCircuitState::default();
            return false;
        }

        matches!(state.state, CircuitState::Open | CircuitState::Cooldown)
    }

    /// 记录成功结果
    pub fn record_success(&self, ip: IpAddr) {
        let now_ms = current_epoch_ms();
        self.record_outcome(ip, true, now_ms);
    }

    /// 记录失败结果
    pub fn record_failure(&self, ip: IpAddr) {
        let now_ms = current_epoch_ms();
        self.record_outcome(ip, false, now_ms);
    }

    /// 记录结果并检查是否触发熔断
    fn record_outcome(&self, ip: IpAddr, success: bool, now_ms: i64) {
        let config = match self.config.lock() {
            Ok(cfg) => cfg.clone(),
            Err(_) => return,
        };

        if !config.enabled {
            return;
        }

        let mut states = match self.states.lock() {
            Ok(s) => s,
            Err(_) => return,
        };

        let state = states.entry(ip).or_default();

        // 如果已经熔断，不再更新统计
        if matches!(state.state, CircuitState::Open | CircuitState::Cooldown) {
            return;
        }

        // 检查窗口是否过期，需要重置
        if state.is_window_expired(now_ms, config.window_ms()) {
            state.reset_window(now_ms);
        }

        // 更新统计
        if success {
            state.window_successes = state.window_successes.saturating_add(1);
            state.consecutive_failures = 0;
        } else {
            state.window_failures = state.window_failures.saturating_add(1);
            state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        }

        // 检查是否应该触发熔断
        if state.should_trip(&config) {
            crate::core::ip_pool::events::emit_ip_pool_ip_tripped(ip, "circuit breaker tripped");
            tracing::warn!(
                target: "ip_pool",
                ip = %ip,
                consecutive_failures = state.consecutive_failures,
                window_failures = state.window_failures,
                window_successes = state.window_successes,
                "circuit breaker tripped: ip temporarily blacklisted"
            );

            state.state = CircuitState::Open;
            state.opened_at_ms = now_ms;
            state.cooldown_at_ms = now_ms; // 进入冷却期
            state.state = CircuitState::Cooldown;
        }
    }

    /// 手动重置指定 IP 的熔断状态
    pub fn reset_ip(&self, ip: IpAddr) {
        if let Ok(mut states) = self.states.lock() {
            if let Some(state) = states.get_mut(&ip) {
                let was_tripped = matches!(state.state, CircuitState::Open | CircuitState::Cooldown);
                *state = IpCircuitState::default();
                if was_tripped {
                    tracing::info!(
                        target: "ip_pool",
                        ip = %ip,
                        "circuit breaker manually reset"
                    );
                }
            }
        }
    }

    /// 清除所有熔断状态
    pub fn clear_all(&self) {
        if let Ok(mut states) = self.states.lock() {
            let count = states.len();
            states.clear();
            if count > 0 {
                tracing::info!(
                    target: "ip_pool",
                    cleared_count = count,
                    "all circuit breaker states cleared"
                );
            }
        }
    }

    /// 获取指定 IP 的统计信息（用于测试和观测）
    #[cfg(test)]
    pub fn get_stats(&self, ip: IpAddr) -> Option<CircuitStats> {
        let states = self.states.lock().ok()?;
        let state = states.get(&ip)?;
        Some(CircuitStats {
            state: state.state,
            consecutive_failures: state.consecutive_failures,
            window_failures: state.window_failures,
            window_successes: state.window_successes,
        })
    }

    /// 获取所有被熔断的 IP 列表
    pub fn get_tripped_ips(&self) -> Vec<IpAddr> {
        let states = match self.states.lock() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        states
            .iter()
            .filter(|(_, state)| {
                matches!(state.state, CircuitState::Open | CircuitState::Cooldown)
            })
            .map(|(ip, _)| *ip)
            .collect()
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

/// 熔断器统计信息（用于测试）
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitStats {
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub window_failures: u32,
    pub window_successes: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_failures_trigger_circuit_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 3,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        // 前两次失败不应触发
        breaker.record_failure(ip);
        assert!(!breaker.is_tripped(ip));
        breaker.record_failure(ip);
        assert!(!breaker.is_tripped(ip));

        // 第三次失败触发熔断
        breaker.record_failure(ip);
        assert!(breaker.is_tripped(ip));

        let stats = breaker.get_stats(ip).unwrap();
        assert_eq!(stats.state, CircuitState::Cooldown);
        assert_eq!(stats.consecutive_failures, 3);
    }

    #[test]
    fn success_resets_consecutive_failures() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 3,
            failure_rate_threshold: 0.9, // 高阈值，避免失败率触发
            min_samples_in_window: 10,   // 高样本数要求
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        breaker.record_failure(ip);
        breaker.record_failure(ip);
        breaker.record_success(ip); // 重置连续失败计数
        breaker.record_failure(ip);
        breaker.record_failure(ip);

        // 不应触发，因为中间有成功，连续失败被重置
        assert!(!breaker.is_tripped(ip));
    }

    #[test]
    fn failure_rate_triggers_circuit_open() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 100, // 不通过连续失败触发
            failure_rate_threshold: 0.5,
            min_samples_in_window: 5,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        // 2 成功 + 3 失败 = 5 样本，失败率 60%
        breaker.record_success(ip);
        breaker.record_success(ip);
        breaker.record_failure(ip);
        breaker.record_failure(ip);
        breaker.record_failure(ip);

        // 应触发熔断
        assert!(breaker.is_tripped(ip));
    }

    #[test]
    fn manual_reset_clears_circuit_state() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 2,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        breaker.record_failure(ip);
        breaker.record_failure(ip);
        assert!(breaker.is_tripped(ip));

        breaker.reset_ip(ip);
        assert!(!breaker.is_tripped(ip));
    }

    #[test]
    fn disabled_breaker_never_trips() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: false,
            consecutive_failure_threshold: 1,
            ..Default::default()
        });

        let ip = "192.0.2.1".parse().unwrap();

        breaker.record_failure(ip);
        breaker.record_failure(ip);
        breaker.record_failure(ip);

        assert!(!breaker.is_tripped(ip));
    }

    #[test]
    fn get_tripped_ips_returns_only_tripped() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            enabled: true,
            consecutive_failure_threshold: 2,
            ..Default::default()
        });

        let ip1: IpAddr = "192.0.2.1".parse().unwrap();
        let ip2: IpAddr = "192.0.2.2".parse().unwrap();
        let ip3: IpAddr = "192.0.2.3".parse().unwrap();

        breaker.record_failure(ip1);
        breaker.record_failure(ip1); // ip1 熔断

        breaker.record_failure(ip2);
        breaker.record_success(ip2); // ip2 正常

        breaker.record_failure(ip3);
        breaker.record_failure(ip3); // ip3 熔断

        let tripped = breaker.get_tripped_ips();
        assert_eq!(tripped.len(), 2);
        assert!(tripped.contains(&ip1));
        assert!(tripped.contains(&ip3));
        assert!(!tripped.contains(&ip2));
    }
}
