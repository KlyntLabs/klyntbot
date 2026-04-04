//! Repository for the `review_sessions` table — tracks active recall review sessions.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

// ── Row type ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewSessionRow {
    pub id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub cards_reviewed: i64,
    pub avg_score: Option<f64>,
    pub duration_seconds: Option<i64>,
    pub modes_used: Option<String>,
    pub propagation_count: i64,
    pub weak_card_ids: Option<String>,
    pub session_data: Option<String>,
    pub status: String,
}

// ── Repository ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReviewSessionRepo {
    pool: SqlitePool,
}

impl ReviewSessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new active review session.
    pub async fn create(&self, id: &str) -> Result<ReviewSessionRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO review_sessions (id, started_at, cards_reviewed, propagation_count, status)
            VALUES (?1, ?2, 0, 0, 'active')
            "#,
        )
        .bind(id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Mark a session as completed with summary data.
    #[allow(clippy::too_many_arguments)]
    pub async fn complete(
        &self,
        id: &str,
        cards_reviewed: i64,
        avg_score: Option<f64>,
        duration_seconds: Option<i64>,
        modes_used: Option<&str>,
        propagation_count: i64,
        weak_card_ids: Option<&str>,
        session_data: Option<&str>,
    ) -> Result<ReviewSessionRow, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE review_sessions
            SET completed_at = ?1,
                cards_reviewed = ?2,
                avg_score = ?3,
                duration_seconds = ?4,
                modes_used = ?5,
                propagation_count = ?6,
                weak_card_ids = ?7,
                session_data = ?8,
                status = 'completed'
            WHERE id = ?9
            "#,
        )
        .bind(&now)
        .bind(cards_reviewed)
        .bind(avg_score)
        .bind(duration_seconds)
        .bind(modes_used)
        .bind(propagation_count)
        .bind(weak_card_ids)
        .bind(session_data)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Mark a session as abandoned with the card count reviewed so far.
    pub async fn abandon(
        &self,
        id: &str,
        cards_reviewed: i64,
    ) -> Result<ReviewSessionRow, sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE review_sessions
            SET cards_reviewed = ?1, status = 'abandoned'
            WHERE id = ?2
            "#,
        )
        .bind(cards_reviewed)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(sqlx::Error::RowNotFound)
    }

    /// Get the currently active session (if any).
    pub async fn get_active(&self) -> Result<Option<ReviewSessionRow>, sqlx::Error> {
        sqlx::query_as::<_, ReviewSessionRow>(
            "SELECT * FROM review_sessions WHERE status = 'active' ORDER BY started_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get a session by ID.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<ReviewSessionRow>, sqlx::Error> {
        sqlx::query_as::<_, ReviewSessionRow>("SELECT * FROM review_sessions WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> ReviewSessionRepo {
        let pool = crate::repos::cognitive_test_pool().await;
        ReviewSessionRepo::new(pool)
    }

    #[tokio::test]
    async fn test_create_session() {
        let repo = setup().await;
        let session = repo.create("test-session-1").await.unwrap();
        assert_eq!(session.id, "test-session-1");
        assert_eq!(session.status, "active");
        assert_eq!(session.cards_reviewed, 0);
        assert_eq!(session.propagation_count, 0);
        assert!(session.completed_at.is_none());
    }

    #[tokio::test]
    async fn test_complete_session() {
        let repo = setup().await;
        repo.create("test-session-2").await.unwrap();

        let completed = repo
            .complete(
                "test-session-2",
                5,
                Some(0.8),
                Some(300),
                Some("typed,multiple_choice"),
                2,
                Some("[\"card-1\",\"card-2\"]"),
                None,
            )
            .await
            .unwrap();

        assert_eq!(completed.status, "completed");
        assert_eq!(completed.cards_reviewed, 5);
        assert!((completed.avg_score.unwrap() - 0.8).abs() < f64::EPSILON);
        assert_eq!(completed.duration_seconds, Some(300));
        assert_eq!(completed.propagation_count, 2);
        assert!(completed.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_abandon_session() {
        let repo = setup().await;
        repo.create("test-session-3").await.unwrap();

        let abandoned = repo.abandon("test-session-3", 2).await.unwrap();
        assert_eq!(abandoned.status, "abandoned");
        assert_eq!(abandoned.cards_reviewed, 2);
    }

    #[tokio::test]
    async fn test_get_active() {
        let repo = setup().await;
        assert!(repo.get_active().await.unwrap().is_none());

        repo.create("active-session").await.unwrap();
        let active = repo.get_active().await.unwrap();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, "active-session");
    }

    #[tokio::test]
    async fn test_get_active_excludes_completed() {
        let repo = setup().await;
        repo.create("done-session").await.unwrap();
        repo.complete("done-session", 3, None, None, None, 0, None, None)
            .await
            .unwrap();

        assert!(repo.get_active().await.unwrap().is_none());
    }
}
