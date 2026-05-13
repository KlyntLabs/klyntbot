//! KCA Track 7 — predictive cache.

use crate::services::retrieval::ScoredFact;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

struct CacheEntry {
    inserted_at: Instant,
    value: Vec<ScoredFact>,
}

pub struct PredictiveCache {
    inner: Mutex<LruCache<String, CacheEntry>>,
    ttl: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
    min_hit_rate: f64,
    disabled: std::sync::atomic::AtomicBool,
    disabled_until: Mutex<Option<Instant>>,
}

impl PredictiveCache {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            ttl,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            min_hit_rate: 0.0,
            disabled: std::sync::atomic::AtomicBool::new(false),
            disabled_until: Mutex::new(None),
        }
    }

    pub fn with_auto_disable(capacity: usize, ttl: Duration, min_hit_rate: f64) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            ttl,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            min_hit_rate,
            disabled: std::sync::atomic::AtomicBool::new(false),
            disabled_until: Mutex::new(None),
        }
    }

    pub async fn put(&self, key: String, value: Vec<ScoredFact>) {
        let mut g = self.inner.lock().await;
        g.put(
            key,
            CacheEntry {
                inserted_at: Instant::now(),
                value,
            },
        );
    }

    pub async fn get(&self, key: &str) -> Option<Vec<ScoredFact>> {
        if self.is_disabled().await {
            return None;
        }
        let mut g = self.inner.lock().await;
        let entry = g.get(key);
        match entry {
            Some(e) if e.inserted_at.elapsed() < self.ttl => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(e.value.clone())
            }
            Some(_) => {
                g.pop(key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                self.maybe_disable().await;
                None
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                self.maybe_disable().await;
                None
            }
        }
    }

    pub async fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    pub async fn size(&self) -> usize {
        self.inner.lock().await.len()
    }

    pub async fn is_disabled(&self) -> bool {
        if self.disabled.load(Ordering::Relaxed) {
            let mut g = self.disabled_until.lock().await;
            if let Some(until) = *g {
                if Instant::now() >= until {
                    self.disabled.store(false, Ordering::Relaxed);
                    self.hits.store(0, Ordering::Relaxed);
                    self.misses.store(0, Ordering::Relaxed);
                    *g = None;
                    return false;
                }
            }
            return true;
        }
        false
    }

    async fn maybe_disable(&self) {
        if self.min_hit_rate <= 0.0 {
            return;
        }
        let h = self.hits.load(Ordering::Relaxed);
        let m = self.misses.load(Ordering::Relaxed);
        let total = h + m;
        if total < 100 {
            return;
        }
        let rate = h as f64 / total as f64;
        if rate < self.min_hit_rate {
            self.disabled.store(true, Ordering::Relaxed);
            *self.disabled_until.lock().await = Some(Instant::now() + Duration::from_secs(86400));
            tracing::info!(rate, "predictive cache auto-disabled for 24h");
        }
    }
}

pub fn query_hash(query: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(query.trim().to_lowercase().as_bytes());
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SemanticFact;

    fn sample(s: &str) -> Vec<ScoredFact> {
        vec![ScoredFact {
            fact: SemanticFact {
                id: format!("{s}_id"),
                domain: "test".into(),
                subject: s.into(),
                predicate: "p".into(),
                object: "o".into(),
                confidence: 0.5,
                source: "t".into(),
                valid_from: "2026-01-01".into(),
                valid_until: None,
                recorded_at: "2026-01-01".into(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                convergence_score: 0.0,
                project_id: None,
                memory_type: "fact".into(),
                scope_type: "system".into(),
                scope_id: None,
                scope_repo_id: None,
                metadata: None,
                speaker: None,
            },
            score: 0.7,
            similarity: None,
        }]
    }

    #[tokio::test]
    async fn cache_returns_value_within_ttl() {
        let cache = PredictiveCache::new(100, Duration::from_secs(60));
        cache.put("hash1".into(), sample("alpha")).await;
        let got = cache.get("hash1").await;
        assert!(got.is_some());
        assert_eq!(got.unwrap()[0].fact.subject, "alpha");
    }

    #[tokio::test]
    async fn cache_expires_after_ttl() {
        let cache = PredictiveCache::new(100, Duration::from_millis(50));
        cache.put("h".into(), sample("z")).await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(cache.get("h").await.is_none());
    }

    #[tokio::test]
    async fn cache_tracks_hit_rate() {
        let cache = PredictiveCache::new(100, Duration::from_secs(60));
        cache.put("h1".into(), sample("a")).await;
        let _ = cache.get("h1").await; // hit
        let _ = cache.get("missing").await; // miss
        let _ = cache.get("h1").await; // hit
        let stats = cache.stats().await;
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 2.0 / 3.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn cache_disables_after_low_hit_rate_window() {
        let cache = PredictiveCache::with_auto_disable(100, Duration::from_secs(60), 0.2);
        // Force 100 misses + 0 hits.
        for i in 0..100 {
            let _ = cache.get(&format!("missing-{i}")).await;
        }
        assert!(cache.is_disabled().await);
    }
}
