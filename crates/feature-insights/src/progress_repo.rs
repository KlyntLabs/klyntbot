//! Repository for the `insight_progress_snapshots` table.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::types::{ProgressSnapshotRow, ProgressWeights};

#[derive(Debug, Clone)]
pub struct InsightProgressRepo {
    pool: SqlitePool,
}

impl InsightProgressRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update a progress snapshot for a specific insight version.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert(
        &self,
        insight_review_id: &str,
        version: i64,
        flashcard_success: f64,
        semantic_drift: f64,
        gap_closure: f64,
        quiz_score: f64,
        weights: &ProgressWeights,
    ) -> Result<ProgressSnapshotRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let overall = weights.flashcard * flashcard_success
            + weights.drift * (1.0 - semantic_drift)
            + weights.gap * gap_closure
            + weights.quiz * quiz_score;

        sqlx::query(
            r#"
            INSERT INTO insight_progress_snapshots
                (id, insight_review_id, version, flashcard_success, semantic_drift,
                 gap_closure, quiz_score, overall_progress, computed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(insight_review_id, version) DO UPDATE SET
                flashcard_success = excluded.flashcard_success,
                semantic_drift = excluded.semantic_drift,
                gap_closure = excluded.gap_closure,
                quiz_score = excluded.quiz_score,
                overall_progress = excluded.overall_progress,
                computed_at = excluded.computed_at
            "#,
        )
        .bind(&id)
        .bind(insight_review_id)
        .bind(version)
        .bind(flashcard_success)
        .bind(semantic_drift)
        .bind(gap_closure)
        .bind(quiz_score)
        .bind(overall)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let row = sqlx::query_as::<_, ProgressSnapshotRow>(
            "SELECT * FROM insight_progress_snapshots WHERE insight_review_id = ?1 AND version = ?2",
        )
        .bind(insight_review_id)
        .bind(version)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get the progress timeline for a note (all versions, ordered by version).
    pub async fn get_timeline(
        &self,
        note_id: &str,
    ) -> Result<Vec<ProgressSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, ProgressSnapshotRow>(
            r#"
            SELECT p.* FROM insight_progress_snapshots p
            INNER JOIN insight_reviews r ON p.insight_review_id = r.id
            WHERE r.note_id = ?1
            ORDER BY p.version ASC
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get the latest progress snapshot for a specific insight.
    pub async fn get_latest(
        &self,
        insight_review_id: &str,
    ) -> Result<Option<ProgressSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, ProgressSnapshotRow>(
            "SELECT * FROM insight_progress_snapshots WHERE insight_review_id = ?1 ORDER BY version DESC LIMIT 1",
        )
        .bind(insight_review_id)
        .fetch_optional(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::InsightReviewRepo;
    use crate::types::ScopeConfig;

    async fn setup() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::migrate!("../storage/migrations")
            .run(&pool)
            .await
            .unwrap();
        let migrations = cognitive::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(&pool, &migrations)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = setup().await;
        let insight_repo = InsightReviewRepo::new(pool.clone());
        let progress_repo = InsightProgressRepo::new(pool);
        let scope = ScopeConfig::default();
        let weights = ProgressWeights::default();

        let insight = insight_repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();

        let snapshot = progress_repo
            .upsert(&insight.id, 1, 0.8, 0.1, 0.5, 0.7, &weights)
            .await
            .unwrap();

        assert_eq!(snapshot.insight_review_id, insight.id);
        assert!((snapshot.flashcard_success - 0.8).abs() < f64::EPSILON);
        // overall = 0.40*0.8 + 0.25*(1-0.1) + 0.20*0.5 + 0.15*0.7
        //         = 0.32 + 0.225 + 0.10 + 0.105 = 0.75
        assert!((snapshot.overall_progress - 0.75).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_timeline() {
        let pool = setup().await;
        let insight_repo = InsightReviewRepo::new(pool.clone());
        let progress_repo = InsightProgressRepo::new(pool);
        let scope = ScopeConfig::default();
        let weights = ProgressWeights::default();

        let v1 = insight_repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();
        let v2 = insight_repo
            .insert("note-1", r#"{"synthesis":"v2"}"#, "hash-2", &scope, &[], None)
            .await
            .unwrap();

        progress_repo
            .upsert(&v1.id, 1, 0.5, 0.0, 0.0, 0.3, &weights)
            .await
            .unwrap();
        progress_repo
            .upsert(&v2.id, 2, 0.8, 0.2, 0.6, 0.7, &weights)
            .await
            .unwrap();

        let timeline = progress_repo.get_timeline("note-1").await.unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].version, 1);
        assert_eq!(timeline[1].version, 2);
        assert!(timeline[1].overall_progress > timeline[0].overall_progress);
    }
}
