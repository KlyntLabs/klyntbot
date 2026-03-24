use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Per-server circuit breaker for MCP connections.
///
/// Tracks failures per server name. Opens the circuit after `threshold`
/// failures within a window, blocking calls for `cooldown` duration.
/// Auto-resets when cooldown expires. Can be manually reset via `record_success()`.
pub struct McpCircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    state: DashMap<String, CircuitState>,
}

struct CircuitState {
    failure_count: u32,
    first_failure: Instant,
}

impl McpCircuitBreaker {
    pub fn new(threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            threshold,
            cooldown: Duration::from_secs(cooldown_secs),
            state: DashMap::new(),
        }
    }

    /// Check if the circuit is open (calls should be blocked).
    /// Returns `false` if cooldown has expired (auto-resets).
    pub fn is_open(&self, server: &str) -> bool {
        if let Some(entry) = self.state.get(server) {
            if entry.failure_count >= self.threshold {
                if entry.first_failure.elapsed() > self.cooldown {
                    drop(entry);
                    self.state.remove(server);
                    false
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Record a failure. Returns `true` if the circuit just opened.
    pub fn record_failure(&self, server: &str) -> bool {
        let mut entry = self
            .state
            .entry(server.to_string())
            .or_insert(CircuitState {
                failure_count: 0,
                first_failure: Instant::now(),
            });

        if entry.first_failure.elapsed() > self.cooldown {
            entry.failure_count = 1;
            entry.first_failure = Instant::now();
            return false;
        }

        entry.failure_count += 1;
        entry.failure_count >= self.threshold
    }

    /// Record a success — explicitly reset the circuit for this server.
    pub fn record_success(&self, server: &str) {
        self.state.remove(server);
    }

    /// Check if cooldown has expired for a previously-open circuit.
    pub fn cooldown_expired(&self, server: &str) -> bool {
        if let Some(entry) = self.state.get(server) {
            entry.failure_count >= self.threshold && entry.first_failure.elapsed() > self.cooldown
        } else {
            false
        }
    }

    /// Remove stale entries whose cooldown has expired.
    pub fn cleanup(&self) {
        self.state
            .retain(|_, state| state.first_failure.elapsed() <= self.cooldown);
    }

    /// Insert a circuit state with a past `first_failure` for testing.
    #[cfg(test)]
    fn insert_opened_at(&self, server: &str, failure_count: u32, first_failure: Instant) {
        self.state.insert(
            server.to_string(),
            CircuitState {
                failure_count,
                first_failure,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = McpCircuitBreaker::new(3, 60);
        assert!(!cb.is_open("test-server"));
    }

    #[test]
    fn test_circuit_opens_after_threshold() {
        let cb = McpCircuitBreaker::new(3, 60);
        assert!(!cb.record_failure("srv"));
        assert!(!cb.record_failure("srv"));
        assert!(cb.record_failure("srv"));
        assert!(cb.is_open("srv"));
    }

    #[test]
    fn test_circuit_blocks_when_open() {
        let cb = McpCircuitBreaker::new(2, 60);
        cb.record_failure("srv");
        cb.record_failure("srv");
        assert!(cb.is_open("srv"));
    }

    #[test]
    fn test_circuit_auto_resets_after_cooldown() {
        let cb = McpCircuitBreaker::new(2, 60);
        // Simulate an opened circuit whose cooldown has expired
        let past = Instant::now() - Duration::from_secs(120);
        cb.insert_opened_at("srv", 2, past);
        // is_open should auto-reset because cooldown expired
        assert!(!cb.is_open("srv"));
    }

    #[test]
    fn test_record_success_resets_circuit() {
        let cb = McpCircuitBreaker::new(2, 60);
        cb.record_failure("srv");
        cb.record_failure("srv");
        assert!(cb.is_open("srv"));
        cb.record_success("srv");
        assert!(!cb.is_open("srv"));
    }

    #[test]
    fn test_per_server_isolation() {
        let cb = McpCircuitBreaker::new(2, 60);
        cb.record_failure("srv-a");
        cb.record_failure("srv-a");
        assert!(cb.is_open("srv-a"));
        assert!(!cb.is_open("srv-b"));
    }

    #[test]
    fn test_cooldown_expired() {
        let cb = McpCircuitBreaker::new(2, 60);
        // Simulate an opened circuit whose first_failure was long ago
        let past = Instant::now() - Duration::from_secs(120);
        cb.insert_opened_at("srv", 2, past);
        assert!(cb.cooldown_expired("srv"));
    }

    #[test]
    fn test_cleanup_removes_stale() {
        let cb = McpCircuitBreaker::new(2, 60);
        // Simulate an opened circuit whose cooldown has expired
        let past = Instant::now() - Duration::from_secs(120);
        cb.insert_opened_at("srv", 2, past);
        cb.cleanup();
        assert!(!cb.is_open("srv"));
    }
}
