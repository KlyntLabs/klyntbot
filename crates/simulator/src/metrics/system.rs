/// Measure community stability as the average `stability` value across all
/// communities that have a non-NULL stability column.
///
/// Returns `0.0` if no rows match or the table does not exist.
pub async fn measure_community_stability(pool: &sqlx::SqlitePool) -> f64 {
    let result: Result<(f64,), _> = sqlx::query_as(
        "SELECT COALESCE(AVG(stability), 0.0) FROM communities WHERE stability IS NOT NULL",
    )
    .fetch_one(pool)
    .await;

    result.map(|(avg,)| avg).unwrap_or(0.0)
}

/// Count the number of brain versions promoted after a given timestamp.
///
/// `since` should be an RFC 3339 / ISO 8601 timestamp string.
///
/// Returns `0` if the table does not exist or any error occurs.
pub async fn count_brain_versions_since(pool: &sqlx::SqlitePool, since: &str) -> u32 {
    let result: Result<(i64,), _> =
        sqlx::query_as("SELECT COUNT(*) FROM mirror_brain_versions WHERE promoted_at > ?1")
            .bind(since)
            .fetch_one(pool)
            .await;

    result.map(|(n,)| n as u32).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Tables don't exist in a bare in-memory pool — functions should return
    /// their safe defaults rather than panicking.
    #[tokio::test]
    async fn graceful_on_missing_tables() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        let stability = measure_community_stability(&pool).await;
        assert!((stability - 0.0).abs() < 1e-9);

        let count = count_brain_versions_since(&pool, "2026-01-01T00:00:00Z").await;
        assert_eq!(count, 0);
    }
}
