//! A [`MetricSource`] implementation for the simulation harness.
//!
//! The simulator does not run real shadow classification, so most autotuner
//! metrics are derived from the shadow log tables (if populated) or fall back
//! to safe defaults.  This is sufficient to exercise the [`NightlyCycle`]
//! evaluation-and-promotion logic inside the simulation.

use async_trait::async_trait;
use autotuner::traits::{MetricSnapshot, MetricSource};
use jiff::Timestamp;
use uuid::Uuid;

/// Provides [`MetricSnapshot`]s to the autotuner's [`NightlyCycle`] by querying
/// the shadow log tables in the simulation database.
///
/// Fields that cannot be measured inside the simulator (e.g. `user_satisfaction`,
/// `knowledge_retention_score`) are left at their `Default` values (0.0 / None).
pub struct SimMetricSource {
    pool: sqlx::SqlitePool,
}

impl SimMetricSource {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MetricSource for SimMetricSource {
    async fn collect_metrics(
        &self,
        since: Timestamp,
        trial_id: Option<Uuid>,
    ) -> common::Result<MetricSnapshot> {
        let since_str = since.strftime("%Y-%m-%d %H:%M:%S").to_string();
        let tid_str = trial_id.map(|t| t.to_string());

        // ── Shadow log stats (classification) ──────────────────────────
        let (total_msgs, corrected, agreed): (i64, i64, i64) = sqlx::query_as(
            "SELECT
                 COUNT(*) AS total,
                 COALESCE(SUM(CASE WHEN user_corrected = 1 THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN predicted_mode = control_mode THEN 1 ELSE 0 END), 0)
             FROM autotuner_shadow_log
             WHERE (?1 IS NULL OR trial_id = ?1)
               AND control_mode != 'pending'
               AND created_at >= ?2",
        )
        .bind(&tid_str)
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0, 0, 0));

        let total = total_msgs.max(1) as f64;

        let correction_rate = corrected as f64 / total;
        let classification_accuracy = agreed as f64 / total;

        // ── Shadow retrieval log stats (memory) ────────────────────────
        let (retrieval_precision, memory_freshness): (f64, f64) = sqlx::query_as(
            "SELECT
                 COALESCE(AVG(CASE WHEN variant_retrieved_count > 0
                     THEN CAST(overlap_count AS REAL) / variant_retrieved_count
                     ELSE 0.0 END), 0.0),
                 COALESCE(AVG(variant_avg_age_days), 0.0)
             FROM autotuner_shadow_retrieval_log
             WHERE (?1 IS NULL OR trial_id = ?1)
               AND created_at >= ?2",
        )
        .bind(&tid_str)
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0.0, 0.0));

        // ── Token and latency metrics from usage/interaction tables ──
        let (avg_tokens,): (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(CAST(prompt_tokens + completion_tokens AS REAL)), 0.0)
             FROM usage_records WHERE timestamp >= ?1",
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0.0,));

        let (avg_response_time,): (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(CAST(duration_ms AS REAL)), 0.0)
             FROM interaction_log WHERE timestamp >= ?1",
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0.0,));

        // ── Memory relevance: average stability of active semantic facts ──
        let (memory_relevance,): (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(stability), 0.0)
             FROM semantic_facts WHERE superseded_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0.0,));

        // ── Retrieval recall from shadow retrieval log (overlap / control) ──
        let (retrieval_recall,): (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(CASE WHEN control_retrieved_count > 0
                 THEN CAST(overlap_count AS REAL) / control_retrieved_count
                 ELSE 0.0 END), 0.0)
             FROM autotuner_shadow_retrieval_log
             WHERE (?1 IS NULL OR trial_id = ?1)
               AND created_at >= ?2",
        )
        .bind(&tid_str)
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0.0,));

        // ── Promotion accuracy: promoted / (promoted + reverted) ──
        let (promoted, reverted): (i64, i64) = sqlx::query_as(
            "SELECT
                 COALESCE(SUM(CASE WHEN status = 'promoted' THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN status = 'reverted' THEN 1 ELSE 0 END), 0)
             FROM autotuner_trials",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0, 0));
        let promotion_accuracy = if promoted + reverted > 0 {
            promoted as f64 / (promoted + reverted) as f64
        } else {
            0.0
        };

        // ── Knowledge retention: FSRS-5 retrievability across active facts ──
        // R = 1 / (1 + elapsed_days / (9 * stability))
        let (knowledge_retention,): (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(
                 1.0 / (1.0 + (julianday(?1) - julianday(COALESCE(last_reviewed_at, created_at)))
                               / (9.0 * MAX(stability, 0.01)))
             ), 0.0)
             FROM semantic_facts WHERE superseded_at IS NULL",
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0.0,));

        Ok(MetricSnapshot {
            correction_rate,
            classification_accuracy,
            total_messages: total_msgs as u32,
            routing_stability: classification_accuracy, // best proxy in sim
            retrieval_precision,
            retrieval_recall,
            memory_freshness,
            avg_tokens_per_message: avg_tokens,
            avg_response_time_ms: avg_response_time,
            memory_relevance,
            promotion_accuracy,
            knowledge_retention_score: knowledge_retention,
            user_satisfaction: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::repos::trial_repo::MIGRATION_SQL;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(MIGRATION_SQL).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn returns_defaults_on_empty_tables() {
        let pool = setup_pool().await;
        let source = SimMetricSource::new(pool);

        let since = Utc::now() - chrono::Duration::hours(24);
        let snap = source.collect_metrics(since, None).await.unwrap();

        // With no shadow log rows, total_messages should be 0 (but we clamp
        // the divisor to 1 so rates are 0.0 rather than NaN).
        assert_eq!(snap.total_messages, 0);
        assert!((snap.correction_rate - 0.0).abs() < 1e-9);
        assert!((snap.classification_accuracy - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn graceful_when_tables_missing() {
        // Bare in-memory pool without migrations.
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        let source = SimMetricSource::new(pool);

        let since = Utc::now() - chrono::Duration::hours(24);
        let snap = source.collect_metrics(since, None).await.unwrap();

        assert_eq!(snap.total_messages, 0);
    }
}
