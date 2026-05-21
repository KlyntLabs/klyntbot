//! Repository for the `ai_signal_index` table.
//!
//! This index is the retrieval-side projection of the `AiSignal` stream:
//! every non-Discard signal above a minimum importance threshold lands here so
//! `CognitiveContextSource` can query "what are the recent high-importance
//! signals from domain X?" without walking the full event log.

use ai_core::RecallDomain;
use common::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IndexedSignal {
    pub id: String,
    pub domain: String,
    pub event_kind: String,
    pub content: String,
    pub importance: f64,
    pub salience: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub timestamp: String,
}

#[derive(Clone)]
pub struct AiSignalIndexRepo {
    pool: SqlitePool,
}

impl AiSignalIndexRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Persist one indexed signal row. `domain` is written as its canonical
    /// string form so reads can filter by typed `RecallDomain` without
    /// additional joins.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert(
        &self,
        id: &str,
        domain: RecallDomain,
        event_kind: &str,
        content: &str,
        importance: f64,
        salience: &str,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        timestamp: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO ai_signal_index
                (id, domain, event_kind, content, importance, salience, entity_type, entity_id, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(id)
        .bind(domain.as_str())
        .bind(event_kind)
        .bind(content)
        .bind(importance)
        .bind(salience)
        .bind(entity_type)
        .bind(entity_id)
        .bind(timestamp)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("ai_signal_index insert: {e}")))?;
        Ok(())
    }

    /// Most-recent signals in a given domain, newest first. Used by
    /// `CognitiveContextSource` when the query is domain-scoped.
    pub async fn recent_by_domain(
        &self,
        domain: RecallDomain,
        limit: i64,
    ) -> Result<Vec<IndexedSignal>> {
        sqlx::query_as::<_, IndexedSignal>(
            "SELECT id, domain, event_kind, content, importance, salience, entity_type, entity_id, timestamp
             FROM ai_signal_index
             WHERE domain = ?1
             ORDER BY timestamp DESC
             LIMIT ?2",
        )
        .bind(domain.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("ai_signal_index recent: {e}")))
    }

    /// Highest-importance signals in a given domain since a timestamp.
    /// Ordered by importance DESC, then timestamp DESC for stable ranking.
    pub async fn high_importance_by_domain(
        &self,
        domain: RecallDomain,
        since: &str,
        limit: i64,
    ) -> Result<Vec<IndexedSignal>> {
        sqlx::query_as::<_, IndexedSignal>(
            "SELECT id, domain, event_kind, content, importance, salience, entity_type, entity_id, timestamp
             FROM ai_signal_index
             WHERE domain = ?1 AND timestamp >= ?2
             ORDER BY importance DESC, timestamp DESC
             LIMIT ?3",
        )
        .bind(domain.as_str())
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("ai_signal_index high_imp: {e}")))
    }

    /// Simple LIKE-based content search within a domain. Not a full-text
    /// index — intended for fallback when the embedding path isn't available.
    pub async fn search_content(
        &self,
        domain: RecallDomain,
        needle: &str,
        limit: i64,
    ) -> Result<Vec<IndexedSignal>> {
        let pattern = format!("%{needle}%");
        sqlx::query_as::<_, IndexedSignal>(
            "SELECT id, domain, event_kind, content, importance, salience, entity_type, entity_id, timestamp
             FROM ai_signal_index
             WHERE domain = ?1 AND content LIKE ?2
             ORDER BY importance DESC, timestamp DESC
             LIMIT ?3",
        )
        .bind(domain.as_str())
        .bind(pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("ai_signal_index search: {e}")))
    }

    /// Total rows — for tests and telemetry.
    pub async fn count(&self) -> Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ai_signal_index")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("ai_signal_index count: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> AiSignalIndexRepo {
        let pool = crate::repos::cognitive_test_pool().await;
        AiSignalIndexRepo::new(pool)
    }

    #[tokio::test]
    async fn insert_and_recent_round_trips() {
        let repo = setup().await;
        repo.insert(
            "s1",
            RecallDomain::Tasks,
            bus::DomainEvent::KIND_TASK_CREATED,
            "wrote a task",
            0.7,
            "extract",
            Some("task"),
            Some("t1"),
            "2026-04-20T10:00:00Z",
        )
        .await
        .unwrap();
        repo.insert(
            "s2",
            RecallDomain::Tasks,
            bus::DomainEvent::KIND_TASK_COMPLETED,
            "finished a task",
            0.8,
            "extract",
            Some("task"),
            Some("t1"),
            "2026-04-20T11:00:00Z",
        )
        .await
        .unwrap();

        let recent = repo
            .recent_by_domain(RecallDomain::Tasks, 10)
            .await
            .unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, "s2"); // newest first
        assert_eq!(recent[1].id, "s1");
    }

    #[tokio::test]
    async fn domain_filter_isolates() {
        let repo = setup().await;
        repo.insert(
            "t",
            RecallDomain::Tasks,
            "X",
            "task content",
            0.5,
            "extract",
            None,
            None,
            "2026-04-20T10:00:00Z",
        )
        .await
        .unwrap();
        repo.insert(
            "f",
            RecallDomain::Productivity,
            "X",
            "productivity content",
            0.5,
            "extract",
            None,
            None,
            "2026-04-20T10:00:00Z",
        )
        .await
        .unwrap();

        let tasks = repo
            .recent_by_domain(RecallDomain::Tasks, 10)
            .await
            .unwrap();
        let productivity = repo
            .recent_by_domain(RecallDomain::Productivity, 10)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(productivity.len(), 1);
        assert_eq!(tasks[0].id, "t");
        assert_eq!(productivity[0].id, "f");
    }

    #[tokio::test]
    async fn high_importance_orders_by_importance() {
        let repo = setup().await;
        for (id, imp) in [("low", 0.3), ("mid", 0.6), ("high", 0.9)] {
            repo.insert(
                id,
                RecallDomain::Learning,
                "X",
                "content",
                imp,
                "extract",
                None,
                None,
                "2026-04-20T10:00:00Z",
            )
            .await
            .unwrap();
        }
        let out = repo
            .high_importance_by_domain(RecallDomain::Learning, "2026-01-01T00:00:00Z", 10)
            .await
            .unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].id, "high");
        assert_eq!(out[1].id, "mid");
        assert_eq!(out[2].id, "low");
    }

    #[tokio::test]
    async fn search_content_like() {
        let repo = setup().await;
        repo.insert(
            "a",
            RecallDomain::Notes,
            "NoteCreated",
            "meeting with alice",
            0.5,
            "extract",
            None,
            None,
            "2026-04-20T10:00:00Z",
        )
        .await
        .unwrap();
        repo.insert(
            "b",
            RecallDomain::Notes,
            "NoteCreated",
            "coffee with bob",
            0.5,
            "extract",
            None,
            None,
            "2026-04-20T10:00:00Z",
        )
        .await
        .unwrap();
        let out = repo
            .search_content(RecallDomain::Notes, "alice", 10)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "a");
    }
}
