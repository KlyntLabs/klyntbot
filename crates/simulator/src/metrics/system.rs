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

/// Counts of promoted and reverted autotuner trials.
pub struct AutotunerStats {
    pub success_rate: f64,
    pub promoted: u32,
    pub reverted: u32,
}

/// Measure autotuner trial success as the ratio of promoted trials to all
/// terminal trials (promoted + reverted), along with raw counts.
///
/// Returns zero stats if no terminal trials exist or the `autotuner_trials`
/// table does not exist.
pub async fn measure_autotuner_success(pool: &sqlx::SqlitePool) -> AutotunerStats {
    let row: Result<(i64, i64), _> = sqlx::query_as(
        "SELECT \
             COALESCE(SUM(CASE WHEN status = 'promoted' THEN 1 ELSE 0 END), 0), \
             COALESCE(SUM(CASE WHEN status = 'reverted' THEN 1 ELSE 0 END), 0) \
         FROM autotuner_trials \
         WHERE status IN ('promoted', 'reverted')",
    )
    .fetch_one(pool)
    .await;

    let (promoted, reverted) = row.unwrap_or((0, 0));
    let total = promoted + reverted;

    AutotunerStats {
        success_rate: if total == 0 {
            0.0
        } else {
            promoted as f64 / total as f64
        },
        promoted: promoted as u32,
        reverted: reverted as u32,
    }
}

/// Measure insight usefulness: fraction of cross-domain insights that exist.
/// In simulation, each insight row represents a successful cross-domain connection.
/// Usefulness = count of insights / total simulated days so far (normalized to 0-1).
pub async fn measure_insight_usefulness(pool: &sqlx::SqlitePool, day: u32) -> f64 {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cross_domain_insights")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    if day == 0 {
        return 0.0;
    }
    // Normalize: insights per week, capped at 1.0
    // CrossDomainInsight fires daily, so max theoretical = day count
    // A score of 1.0 means an insight was generated every day
    (count.0 as f64 / day as f64).min(1.0)
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

        let autotuner = measure_autotuner_success(&pool).await;
        assert!((autotuner.success_rate - 0.0).abs() < 1e-9);
        assert_eq!(autotuner.promoted, 0);
        assert_eq!(autotuner.reverted, 0);
    }

    #[tokio::test]
    async fn autotuner_success_with_trials() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(storage::repos::trial_repo::MIGRATION_SQL)
            .execute(&pool)
            .await
            .unwrap();

        // Insert an experiment first (FK constraint).
        sqlx::query(
            "INSERT INTO autotuner_experiments (id, hypothesis, trend_analysis, recommendation_for_next)
             VALUES ('exp-1', 'h', 't', 'r')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert 2 promoted and 1 reverted trial.
        for (id, status) in [("t1", "promoted"), ("t2", "promoted"), ("t3", "reverted")] {
            sqlx::query(
                "INSERT INTO autotuner_trials (id, experiment_id, params, generation_reasoning, status)
                 VALUES (?1, 'exp-1', '{}', 'test', ?2)",
            )
            .bind(id)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        }

        let stats = measure_autotuner_success(&pool).await;
        // 2 promoted / (2 promoted + 1 reverted) = 0.666...
        assert!((stats.success_rate - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(stats.promoted, 2);
        assert_eq!(stats.reverted, 1);
    }
}
