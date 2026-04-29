use sqlx::SqlitePool;

use crate::types::QualityScore;

#[derive(Debug, Clone)]
pub struct QualityScoreRepo {
    pool: SqlitePool,
}

impl QualityScoreRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, score: &QualityScore) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_quality_scores (id, score_date, session_id, overall_score, focus_depth, okr_alignment, distraction_inv, task_completion, continuity, deep_work_ratio, avg_session_length, meeting_focus_ratio, weights_json, explanation, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
               ON CONFLICT(id) DO UPDATE SET
                   overall_score = excluded.overall_score,
                   focus_depth = excluded.focus_depth,
                   okr_alignment = excluded.okr_alignment,
                   distraction_inv = excluded.distraction_inv,
                   task_completion = excluded.task_completion,
                   continuity = excluded.continuity,
                   deep_work_ratio = excluded.deep_work_ratio,
                   avg_session_length = excluded.avg_session_length,
                   meeting_focus_ratio = excluded.meeting_focus_ratio,
                   weights_json = excluded.weights_json,
                   explanation = excluded.explanation"#,
        )
        .bind(&score.id)
        .bind(&score.score_date)
        .bind(&score.session_id)
        .bind(score.overall_score)
        .bind(score.focus_depth)
        .bind(score.okr_alignment)
        .bind(score.distraction_inv)
        .bind(score.task_completion)
        .bind(score.continuity)
        .bind(score.deep_work_ratio)
        .bind(score.avg_session_length)
        .bind(score.meeting_focus_ratio)
        .bind(&score.weights_json)
        .bind(&score.explanation)
        .bind(&score.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_daily(&self, date: &str) -> common::Result<Option<QualityScore>> {
        let row = sqlx::query_as::<_, QualityScore>(
            "SELECT * FROM productivity_quality_scores WHERE score_date = ?1 AND session_id IS NULL LIMIT 1",
        )
        .bind(date)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn get_for_session(&self, session_id: &str) -> common::Result<Option<QualityScore>> {
        let row = sqlx::query_as::<_, QualityScore>(
            "SELECT * FROM productivity_quality_scores WHERE session_id = ?1 LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn list_range(&self, start: &str, end: &str) -> common::Result<Vec<QualityScore>> {
        let rows = sqlx::query_as::<_, QualityScore>(
            "SELECT * FROM productivity_quality_scores WHERE score_date >= ?1 AND score_date < ?2 ORDER BY score_date DESC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn average_range(&self, start: &str, end: &str) -> common::Result<Option<f64>> {
        let avg: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(overall_score) FROM productivity_quality_scores WHERE score_date >= ?1 AND score_date < ?2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(avg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SessionSource;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn sample_score(id: &str, date: &str, session_id: Option<&str>) -> QualityScore {
        QualityScore {
            id: id.to_string(),
            score_date: date.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            overall_score: 0.85,
            focus_depth: 0.9,
            okr_alignment: 0.7,
            distraction_inv: 0.8,
            task_completion: 0.75,
            continuity: 0.95,
            deep_work_ratio: 0.6,
            avg_session_length: 0.8,
            meeting_focus_ratio: 1.0,
            weights_json: None,
            explanation: Some("Good focus day".to_string()),
            created_at: "2026-03-09T12:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = setup_pool().await;
        let repo = QualityScoreRepo::new(pool);

        let score = sample_score("qs-1", "2026-03-09", None);
        repo.upsert(&score).await.unwrap();

        let fetched = repo.get_daily("2026-03-09").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.overall_score, 0.85);

        // Upsert with updated score
        let updated = QualityScore {
            overall_score: 0.92,
            ..score
        };
        repo.upsert(&updated).await.unwrap();
        let fetched = repo.get_daily("2026-03-09").await.unwrap().unwrap();
        assert_eq!(fetched.overall_score, 0.92);
    }

    #[tokio::test]
    async fn test_get_for_session() {
        let pool = setup_pool().await;
        let repo = QualityScoreRepo::new(pool);

        // Create a session first (needed for FK)
        sqlx::query(
            "INSERT INTO productivity_sessions (id, session_type, started_at, source, created_at, updated_at) VALUES ('sess-qs', 'focus', '2026-03-09T10:00:00Z', ?1, '2026-03-09T10:00:00Z', '2026-03-09T10:00:00Z')",
        )
        .bind(SessionSource::AutoDetected.to_string())
        .execute(&repo.pool)
        .await
        .unwrap();

        let score = sample_score("qs-sess-1", "2026-03-09", Some("sess-qs"));
        repo.upsert(&score).await.unwrap();

        let fetched = repo.get_for_session("sess-qs").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().session_id.as_deref(), Some("sess-qs"));
    }

    #[tokio::test]
    async fn test_average_range() {
        let pool = setup_pool().await;
        let repo = QualityScoreRepo::new(pool);

        let s1 = QualityScore {
            overall_score: 0.80,
            ..sample_score("qs-avg-1", "2026-03-07", None)
        };
        let s2 = QualityScore {
            overall_score: 0.90,
            ..sample_score("qs-avg-2", "2026-03-08", None)
        };
        repo.upsert(&s1).await.unwrap();
        repo.upsert(&s2).await.unwrap();

        let avg = repo
            .average_range("2026-03-07", "2026-03-09")
            .await
            .unwrap();
        assert!(avg.is_some());
        let avg = avg.unwrap();
        assert!((avg - 0.85).abs() < 0.01);
    }
}
