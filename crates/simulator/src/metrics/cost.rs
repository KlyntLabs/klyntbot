//! Cost efficiency metrics: cost-per-outcome and cache hit rate from usage_records.

/// Measure cost efficiency as total estimated cost divided by the number of
/// successful outcomes since `since`.
///
/// - Returns `0.0` when no usage records exist in the window.
/// - Returns `f64::INFINITY` when `outcomes == 0` but cost > 0.
/// - Otherwise returns `total_cost / outcomes as f64`.
///
/// `since` should be an RFC 3339 / ISO 8601 timestamp string.
pub async fn measure_cost_efficiency(pool: &sqlx::SqlitePool, since: &str, outcomes: u32) -> f64 {
    let result: Result<(f64,), _> = sqlx::query_as(
        "SELECT COALESCE(SUM(estimated_cost_usd), 0.0) \
         FROM usage_records \
         WHERE timestamp >= ?1",
    )
    .bind(since)
    .fetch_one(pool)
    .await;

    let total_cost = result.map(|(c,)| c).unwrap_or(0.0);

    if total_cost == 0.0 {
        return 0.0;
    }
    if outcomes == 0 {
        return f64::INFINITY;
    }
    total_cost / outcomes as f64
}

/// Measure cache hit rate as cache_read_tokens / prompt_tokens for all usage
/// records since `since`.
///
/// - Returns `0.0` when no records exist or prompt_tokens totals zero.
/// - Otherwise returns the ratio (may exceed 1.0 if cache_read > prompt in
///   unusual configurations, but typical values are 0–1).
///
/// `since` should be an RFC 3339 / ISO 8601 timestamp string.
pub async fn measure_cache_hit_rate(pool: &sqlx::SqlitePool, since: &str) -> f64 {
    let result: Result<(f64, f64), _> = sqlx::query_as(
        "SELECT CAST(COALESCE(SUM(cache_read_tokens), 0) AS REAL), \
                CAST(COALESCE(SUM(prompt_tokens), 0) AS REAL) \
         FROM usage_records \
         WHERE timestamp >= ?1",
    )
    .bind(since)
    .fetch_one(pool)
    .await;

    let (cache_read, prompt) = result.unwrap_or((0.0, 0.0));

    if prompt == 0.0 {
        return 0.0;
    }
    cache_read / prompt
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("in-memory pool");
        let inner = pool.inner().clone();
        std::mem::forget(pool);
        inner
    }

    #[tokio::test]
    async fn cost_efficiency_returns_zero_on_empty() {
        let pool = setup_pool().await;
        let r = measure_cost_efficiency(&pool, "2026-01-01T00:00:00Z", 5).await;
        assert!((r - 0.0).abs() < 1e-9, "no records → cost 0.0, got {r}");
    }

    #[tokio::test]
    async fn cost_efficiency_with_records() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO usage_records \
             (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
              estimated_cost_usd, channel, strategy) \
             VALUES ('u1', '2026-01-02T00:00:00Z', 'r1', 'm', 'p', 100, 50, 0.05, 'sim', 'reactive')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = measure_cost_efficiency(&pool, "2026-01-01T00:00:00Z", 2).await;
        assert!((r - 0.025).abs() < 1e-9, "0.05 / 2 = 0.025, got {r}");
    }

    #[tokio::test]
    async fn cost_efficiency_infinity_on_zero_outcomes() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO usage_records \
             (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
              estimated_cost_usd, channel, strategy) \
             VALUES ('u1', '2026-01-02T00:00:00Z', 'r1', 'm', 'p', 100, 50, 0.05, 'sim', 'reactive')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = measure_cost_efficiency(&pool, "2026-01-01T00:00:00Z", 0).await;
        assert!(r.is_infinite(), "zero outcomes → infinity, got {r}");
    }

    #[tokio::test]
    async fn cache_hit_rate_returns_zero_on_empty() {
        let pool = setup_pool().await;
        let r = measure_cache_hit_rate(&pool, "2026-01-01T00:00:00Z").await;
        assert!((r - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn cache_hit_rate_with_records() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO usage_records \
             (id, timestamp, request_id, model, provider, prompt_tokens, completion_tokens, \
              cache_read_tokens, cache_write_tokens, estimated_cost_usd, channel, strategy) \
             VALUES ('u1', '2026-01-02T00:00:00Z', 'r1', 'm', 'p', 200, 50, 80, 20, 0.01, 'sim', 'reactive')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let r = measure_cache_hit_rate(&pool, "2026-01-01T00:00:00Z").await;
        assert!((r - 0.4).abs() < 1e-9, "expected 0.4, got {r}");
    }
}
