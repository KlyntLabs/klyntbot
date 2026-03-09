use sqlx::SqlitePool;

use crate::types::ProductivitySession;

#[derive(Debug, Clone)]
pub struct IntelligenceSessionRepo {
    pool: SqlitePool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct TodaySummaryRow {
    total_sessions: i64,
    total_duration: i64,
    avg_quality: Option<f64>,
}

/// Summary of today's productivity sessions.
#[derive(Debug, Clone)]
pub struct TodaySummary {
    pub total_sessions: i64,
    pub total_duration: i64,
    pub avg_quality: Option<f64>,
}

impl IntelligenceSessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, session: &ProductivitySession) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_sessions (id, session_type, started_at, ended_at, duration_secs, dominant_category, category_purity, quality_score, source, app_breakdown, context_switches, distraction_count, predicted_energy, okr_alignment, notes, tags, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"#,
        )
        .bind(&session.id)
        .bind(&session.session_type)
        .bind(&session.started_at)
        .bind(&session.ended_at)
        .bind(session.duration_secs)
        .bind(&session.dominant_category)
        .bind(session.category_purity)
        .bind(session.quality_score)
        .bind(&session.source)
        .bind(&session.app_breakdown)
        .bind(session.context_switches)
        .bind(session.distraction_count)
        .bind(session.predicted_energy)
        .bind(session.okr_alignment)
        .bind(&session.notes)
        .bind(&session.tags)
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> common::Result<Option<ProductivitySession>> {
        let row = sqlx::query_as::<_, ProductivitySession>(
            "SELECT * FROM productivity_sessions WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn get_active(&self) -> common::Result<Option<ProductivitySession>> {
        let row = sqlx::query_as::<_, ProductivitySession>(
            "SELECT * FROM productivity_sessions WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn end_session(
        &self,
        id: &str,
        ended_at: &str,
        duration_secs: i64,
        quality_score: Option<f64>,
    ) -> common::Result<()> {
        sqlx::query(
            r#"UPDATE productivity_sessions SET
                   ended_at = ?2, duration_secs = ?3, quality_score = ?4,
                   updated_at = datetime('now')
               WHERE id = ?1"#,
        )
        .bind(id)
        .bind(ended_at)
        .bind(duration_secs)
        .bind(quality_score)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn update_stats(
        &self,
        id: &str,
        context_switches: i64,
        distraction_count: i64,
        category_purity: f64,
        app_breakdown: Option<&str>,
    ) -> common::Result<()> {
        sqlx::query(
            r#"UPDATE productivity_sessions SET
                   context_switches = ?2, distraction_count = ?3,
                   category_purity = ?4, app_breakdown = ?5,
                   updated_at = datetime('now')
               WHERE id = ?1"#,
        )
        .bind(id)
        .bind(context_switches)
        .bind(distraction_count)
        .bind(category_purity)
        .bind(app_breakdown)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_range(
        &self,
        start: &str,
        end: &str,
    ) -> common::Result<Vec<ProductivitySession>> {
        let rows = sqlx::query_as::<_, ProductivitySession>(
            "SELECT * FROM productivity_sessions WHERE started_at >= ?1 AND started_at < ?2 ORDER BY started_at DESC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn list_by_type(
        &self,
        session_type: &str,
        start: &str,
        end: &str,
    ) -> common::Result<Vec<ProductivitySession>> {
        let rows = sqlx::query_as::<_, ProductivitySession>(
            "SELECT * FROM productivity_sessions WHERE session_type = ?1 AND started_at >= ?2 AND started_at < ?3 ORDER BY started_at DESC",
        )
        .bind(session_type)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn today_summary(&self) -> common::Result<TodaySummary> {
        let row = sqlx::query_as::<_, TodaySummaryRow>(
            r#"SELECT
                   COUNT(*) as total_sessions,
                   COALESCE(SUM(duration_secs), 0) as total_duration,
                   AVG(quality_score) as avg_quality
               FROM productivity_sessions
               WHERE started_at >= date('now')"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(TodaySummary {
            total_sessions: row.total_sessions,
            total_duration: row.total_duration,
            avg_quality: row.avg_quality,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn sample_session(id: &str) -> ProductivitySession {
        ProductivitySession {
            id: id.to_string(),
            session_type: "focus".to_string(),
            started_at: "2026-03-09T10:00:00Z".to_string(),
            ended_at: None,
            duration_secs: None,
            dominant_category: Some("coding".to_string()),
            category_purity: Some(0.9),
            quality_score: None,
            source: "auto".to_string(),
            app_breakdown: None,
            context_switches: 2,
            distraction_count: 1,
            predicted_energy: Some(0.8),
            okr_alignment: None,
            notes: None,
            tags: None,
            created_at: "2026-03-09T10:00:00Z".to_string(),
            updated_at: "2026-03-09T10:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let pool = setup_pool().await;
        let repo = IntelligenceSessionRepo::new(pool);

        let session = sample_session("isess-1");
        repo.create(&session).await.unwrap();

        let fetched = repo.get("isess-1").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.session_type, "focus");
        assert_eq!(fetched.context_switches, 2);
    }

    #[tokio::test]
    async fn test_get_active() {
        let pool = setup_pool().await;
        let repo = IntelligenceSessionRepo::new(pool.clone());

        // Clear any migrated sessions that might be active
        sqlx::query("DELETE FROM productivity_sessions WHERE ended_at IS NULL")
            .execute(&pool)
            .await
            .unwrap();

        let session = sample_session("isess-active-1");
        repo.create(&session).await.unwrap();

        let active = repo.get_active().await.unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, "isess-active-1");
    }

    #[tokio::test]
    async fn test_end_session() {
        let pool = setup_pool().await;
        let repo = IntelligenceSessionRepo::new(pool);

        let session = sample_session("isess-end-1");
        repo.create(&session).await.unwrap();

        repo.end_session("isess-end-1", "2026-03-09T11:00:00Z", 3600, Some(0.85))
            .await
            .unwrap();

        let fetched = repo.get("isess-end-1").await.unwrap().unwrap();
        assert_eq!(fetched.ended_at.as_deref(), Some("2026-03-09T11:00:00Z"));
        assert_eq!(fetched.duration_secs, Some(3600));
        assert_eq!(fetched.quality_score, Some(0.85));
    }

    #[tokio::test]
    async fn test_list_range() {
        let pool = setup_pool().await;
        let repo = IntelligenceSessionRepo::new(pool.clone());

        // Clear migrated data
        sqlx::query("DELETE FROM productivity_sessions")
            .execute(&pool)
            .await
            .unwrap();

        let mut s1 = sample_session("isess-range-1");
        s1.started_at = "2026-03-09T08:00:00Z".to_string();
        let mut s2 = sample_session("isess-range-2");
        s2.started_at = "2026-03-09T12:00:00Z".to_string();
        let mut s3 = sample_session("isess-range-3");
        s3.started_at = "2026-03-10T08:00:00Z".to_string();

        repo.create(&s1).await.unwrap();
        repo.create(&s2).await.unwrap();
        repo.create(&s3).await.unwrap();

        let results = repo
            .list_range("2026-03-09T00:00:00Z", "2026-03-10T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        // Ordered DESC
        assert_eq!(results[0].id, "isess-range-2");
        assert_eq!(results[1].id, "isess-range-1");
    }
}
