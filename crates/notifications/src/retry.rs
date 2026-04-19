//! Exponential backoff retry policy (1s → 4s → 16s by default).
use config::schema::RetryConfig;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_secs(1),
        }
    }
}

impl RetryPolicy {
    pub fn from_config(cfg: &RetryConfig) -> Self {
        Self {
            max_attempts: cfg.max_attempts.max(1),
            base_delay: Duration::from_secs(cfg.base_delay_secs.max(1)),
        }
    }

    /// Delay *before* attempt `attempt` (1-indexed; no delay before attempt 1).
    /// For 1s base: attempt 2 → 1s, attempt 3 → 4s, attempt 4 → 16s.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        if attempt <= 1 {
            return Duration::ZERO;
        }
        let multiplier = 4u64.pow(attempt.saturating_sub(2));
        self.base_delay.saturating_mul(multiplier as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_schedule_is_0_1_4_16() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_for(1), Duration::ZERO);
        assert_eq!(p.delay_for(2), Duration::from_secs(1));
        assert_eq!(p.delay_for(3), Duration::from_secs(4));
        assert_eq!(p.delay_for(4), Duration::from_secs(16));
    }

    #[test]
    fn custom_base_scales() {
        let p = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_secs(2),
        };
        assert_eq!(p.delay_for(2), Duration::from_secs(2));
        assert_eq!(p.delay_for(3), Duration::from_secs(8));
    }
}
