//! Two-layer exchange rate cache: L1 in-memory (DashMap) + L2 SQLite.

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use storage::repos::FinanceExchangeRateRepo;

struct CachedRate {
    rate: f64,
    fetched_at: Instant,
}

/// Two-layer exchange rate cache.
///
/// **L1** — in-memory `DashMap` with TTL-based expiry (cheap, sub-microsecond).
/// **L2** — SQLite via `FinanceExchangeRateRepo` (survives restarts).
///
/// On a cache miss in L1, we check L2 and promote back to L1 on hit.
#[derive(Clone)]
pub struct RateCache {
    l1: Arc<DashMap<String, CachedRate>>,
    l2: FinanceExchangeRateRepo,
    ttl_minutes: i64,
}

impl RateCache {
    /// Create a new two-layer cache backed by the given repo.
    pub fn new(repo: FinanceExchangeRateRepo, ttl_minutes: i64) -> Self {
        Self {
            l1: Arc::new(DashMap::new()),
            l2: repo,
            ttl_minutes,
        }
    }

    /// Build a canonical cache key: "FROM:TO" (uppercase).
    fn cache_key(from: &str, to: &str) -> String {
        format!("{}:{}", from.to_uppercase(), to.to_uppercase())
    }

    /// Look up a rate, checking L1 then L2.
    ///
    /// When `fresh_only` is `true`, only returns rates within the TTL window.
    /// When `false`, returns any cached rate regardless of age.
    pub async fn get(&self, from: &str, to: &str, fresh_only: bool) -> Option<f64> {
        let key = Self::cache_key(from, to);
        let ttl = std::time::Duration::from_secs(self.ttl_minutes as u64 * 60);

        // L1 check
        if let Some(entry) = self.l1.get(&key) {
            if !fresh_only || entry.fetched_at.elapsed() < ttl {
                return Some(entry.rate);
            }
        }

        // L2 check
        let row = if fresh_only {
            self.l2
                .get_fresh(&from.to_uppercase(), &to.to_uppercase(), self.ttl_minutes)
                .await
                .ok()
                .flatten()
        } else {
            self.l2
                .get_stale(&from.to_uppercase(), &to.to_uppercase())
                .await
                .ok()
                .flatten()
        };

        if let Some(row) = row {
            // Promote to L1
            self.l1.insert(
                key,
                CachedRate {
                    rate: row.rate,
                    fetched_at: Instant::now(),
                },
            );
            return Some(row.rate);
        }

        None
    }

    /// Write a rate to both L1 and L2.
    pub async fn put(&self, from: &str, to: &str, rate: f64) -> common::Result<()> {
        let key = Self::cache_key(from, to);

        // L1
        self.l1.insert(
            key,
            CachedRate {
                rate,
                fetched_at: Instant::now(),
            },
        );

        // L2
        self.l2
            .upsert(&from.to_uppercase(), &to.to_uppercase(), rate)
            .await?;

        Ok(())
    }

    /// Batch-write rates from a single base currency to both layers.
    pub async fn put_batch(&self, base: &str, rates: &[(String, f64)]) -> common::Result<()> {
        let base_upper = base.to_uppercase();

        // L1
        for (target, rate) in rates {
            let key = Self::cache_key(base, target);
            self.l1.insert(
                key,
                CachedRate {
                    rate: *rate,
                    fetched_at: Instant::now(),
                },
            );
        }

        // L2
        let refs: Vec<(&str, f64)> = rates
            .iter()
            .map(|(target, rate)| (target.as_str(), *rate))
            .collect();
        self.l2.upsert_batch(&base_upper, &refs).await?;

        Ok(())
    }

    /// Access the underlying repo (e.g. for sentinel operations).
    pub fn repo(&self) -> &FinanceExchangeRateRepo {
        &self.l2
    }
}
