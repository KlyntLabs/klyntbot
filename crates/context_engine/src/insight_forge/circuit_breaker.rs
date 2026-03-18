use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Per-session circuit breaker.
///
/// After `threshold` failures within `cooldown`, `is_open()` returns `true`
/// until the cooldown period expires — at which point the circuit resets.
pub struct CircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    /// Maps session key → (failure count, first failure instant).
    state: DashMap<String, (u32, Instant)>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// - `threshold`: number of failures before the circuit opens.
    /// - `cooldown_secs`: seconds to wait before resetting a tripped circuit.
    pub fn new(threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            threshold,
            cooldown: Duration::from_secs(cooldown_secs),
            state: DashMap::new(),
        }
    }

    /// Record a failure for the given session key.
    ///
    /// If the cooldown has elapsed since the first failure, the counter resets
    /// to 1.  Returns `true` if the circuit is now open (count >= threshold).
    pub fn record_failure(&self, session_key: &str) -> bool {
        let mut entry = self
            .state
            .entry(session_key.to_string())
            .or_insert_with(|| (0, Instant::now()));

        let (count, first_failure) = entry.value_mut();

        if first_failure.elapsed() > self.cooldown {
            // Cooldown elapsed — reset the window.
            *count = 1;
            *first_failure = Instant::now();
        } else {
            *count += 1;
        }

        *count >= self.threshold
    }

    /// Check whether the circuit is open for the given session key.
    ///
    /// Returns `true` if the failure count has reached the threshold and the
    /// cooldown has *not* yet expired.  If the cooldown has expired the entry
    /// is removed and `false` is returned.
    pub fn is_open(&self, session_key: &str) -> bool {
        if let Some(entry) = self.state.get(session_key) {
            let (count, first_failure) = entry.value();
            if *count >= self.threshold {
                if first_failure.elapsed() > self.cooldown {
                    // Cooldown expired — remove and allow retry.
                    drop(entry);
                    self.state.remove(session_key);
                    return false;
                }
                return true;
            }
        }
        false
    }

    /// Remove stale entries whose cooldown has expired.
    pub fn cleanup(&self) {
        self.state
            .retain(|_, (_, first_failure)| first_failure.elapsed() <= self.cooldown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, 300);

        assert!(!cb.is_open("s1"));
        assert!(!cb.record_failure("s1")); // 1
        assert!(!cb.record_failure("s1")); // 2
        assert!(cb.record_failure("s1")); // 3 — trips
        assert!(cb.is_open("s1"));
    }

    #[test]
    fn test_circuit_breaker_per_session() {
        let cb = CircuitBreaker::new(2, 300);

        cb.record_failure("s1");
        cb.record_failure("s1"); // s1 open
        assert!(cb.is_open("s1"));
        assert!(!cb.is_open("s2")); // s2 unaffected
    }

    #[test]
    fn test_circuit_breaker_resets_after_cooldown() {
        // 0-second cooldown means the window expires almost immediately.
        let cb = CircuitBreaker::new(2, 0);

        cb.record_failure("s1");
        cb.record_failure("s1");
        // Cooldown is 0s, so by the time we check it should have expired.
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!cb.is_open("s1"));
    }
}
