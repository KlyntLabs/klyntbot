//! A simple TTL cache for context source data.

use jiff::{SignedDuration, Timestamp};
use std::sync::Mutex;

/// A single-value cache with time-based expiration.
///
/// Designed for context sources that need to cache their `provide()` output
/// for a configurable TTL to avoid repeated expensive queries.
///
/// Uses `std::sync::Mutex` (not `tokio`) since the critical section is
/// CPU-only (timestamp comparison + string clone). This allows concurrent
/// readers across sources without tokio cooperative scheduling overhead.
pub struct TtlCache {
    inner: Mutex<Option<CachedEntry>>,
    ttl_secs: i64,
}

struct CachedEntry {
    content: String,
    expires_at: Timestamp,
}

impl TtlCache {
    /// Create a new cache with the given TTL in seconds.
    pub fn new(ttl_secs: i64) -> Self {
        Self {
            inner: Mutex::new(None),
            ttl_secs,
        }
    }

    /// Return the cached value if it hasn't expired, or `None`.
    pub fn get(&self) -> Option<String> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|entry| {
            if Timestamp::now() < entry.expires_at {
                Some(entry.content.clone())
            } else {
                None
            }
        })
    }

    /// Store a value in the cache, resetting the TTL.
    pub fn set(&self, content: String) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(CachedEntry {
            content,
            expires_at: Timestamp::now() + SignedDuration::from_secs(self.ttl_secs),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_miss_on_empty() {
        let cache = TtlCache::new(60);
        assert!(cache.get().is_none());
    }

    #[test]
    fn test_cache_hit_after_set() {
        let cache = TtlCache::new(60);
        cache.set("hello".to_string());
        assert_eq!(cache.get(), Some("hello".to_string()));
    }

    #[test]
    fn test_cache_miss_after_expiry() {
        let cache = TtlCache::new(-1); // Already expired at insertion
        cache.set("expired".to_string());
        assert!(cache.get().is_none());
    }
}
