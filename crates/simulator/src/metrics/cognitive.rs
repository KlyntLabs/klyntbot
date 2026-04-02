//! Cognitive depth metrics: FSRS-5 retrievability decay, meta-rule counting.

/// Measure average retrievability of all active semantic facts.
///
/// Uses the FSRS-5 stability and elapsed time to compute retrievability
/// for each fact, then returns the average. A score of 1.0 means all facts
/// are perfectly retrievable; a score near 0.0 means memory has decayed.
pub async fn measure_average_retrievability(pool: &sqlx::SqlitePool, simulated_now: &str) -> f64 {
    // Return (stability, recorded_at_unix) to avoid per-row RFC 3339 parsing
    let rows: Vec<(f64, f64)> = sqlx::query_as(
        "SELECT stability, CAST(strftime('%s', recorded_at) AS REAL) \
         FROM semantic_facts \
         WHERE superseded_at IS NULL AND stability > 0",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return 1.0;
    }

    let now = chrono::DateTime::parse_from_rfc3339(simulated_now)
        .unwrap_or_else(|_| chrono::Utc::now().into())
        .timestamp() as f64;

    let mut total_retrievability = 0.0;
    for &(stability, recorded_unix) in &rows {
        let elapsed_days = ((now - recorded_unix) / 86400.0).max(0.0);
        total_retrievability += cognitive::services::fsrs5::retrievability(elapsed_days, stability);
    }

    total_retrievability / rows.len() as f64
}

/// Count the number of meta-rules that were proposed (pending or approved)
/// during the simulation.
pub async fn count_meta_rules(pool: &sqlx::SqlitePool) -> u32 {
    let count: Result<(i64,), _> = sqlx::query_as("SELECT COUNT(*) FROM mirror_meta_rules")
        .fetch_one(pool)
        .await;

    count.map(|(n,)| n as u32).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retrievability_returns_one_for_empty() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let r = measure_average_retrievability(&pool, "2026-04-01T00:00:00Z").await;
        assert!((r - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn meta_rules_returns_zero_on_missing_table() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let count = count_meta_rules(&pool).await;
        assert_eq!(count, 0);
    }
}
