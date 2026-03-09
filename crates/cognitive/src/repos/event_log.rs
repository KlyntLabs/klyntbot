//! Repository for the `domain_event_log` and `pipeline_event_log` tables.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct EventLogRepo {
    pool: SqlitePool,
}

/// A persisted domain event row.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DomainEventRow {
    pub id: String,
    pub event_type: String,
    pub domain: String,
    pub salience: String,
    pub payload: String,
    pub timestamp: String,
}

/// A persisted pipeline event row.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PipelineEventRow {
    pub id: String,
    pub event_kind: String,
    pub observation: Option<String>,
    pub facts_extracted: Option<i64>,
    pub operation: Option<String>,
    pub fact_triple: Option<String>,
    pub timestamp: String,
}

impl EventLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a domain event into the log.
    pub async fn insert_domain_event(
        &self,
        id: &str,
        event_type: &str,
        domain: &str,
        salience: &str,
        payload: &str,
        timestamp: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO domain_event_log (id, event_type, domain, salience, payload, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(id)
        .bind(event_type)
        .bind(domain)
        .bind(salience)
        .bind(payload)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Insert a pipeline event into the log.
    pub async fn insert_pipeline_event(
        &self,
        id: &str,
        event_kind: &str,
        observation: Option<&str>,
        facts_extracted: Option<i64>,
        operation: Option<&str>,
        fact_triple: Option<&str>,
        timestamp: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO pipeline_event_log
                (id, event_kind, observation, facts_extracted, operation, fact_triple, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(id)
        .bind(event_kind)
        .bind(observation)
        .bind(facts_extracted)
        .bind(operation)
        .bind(fact_triple)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch recent domain events, newest first.
    pub async fn list_recent_domain_events(
        &self,
        limit: i64,
    ) -> Result<Vec<DomainEventRow>, sqlx::Error> {
        sqlx::query_as::<_, DomainEventRow>(
            r#"
            SELECT id, event_type, domain, salience, payload, timestamp
            FROM domain_event_log
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Fetch recent pipeline events, newest first.
    pub async fn list_recent_pipeline_events(
        &self,
        limit: i64,
    ) -> Result<Vec<PipelineEventRow>, sqlx::Error> {
        sqlx::query_as::<_, PipelineEventRow>(
            r#"
            SELECT id, event_kind, observation, facts_extracted, operation, fact_triple, timestamp
            FROM pipeline_event_log
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete events older than the given number of days.
    pub async fn prune_old_events(&self, older_than_days: i64) -> Result<u64, sqlx::Error> {
        let cutoff = format!("-{older_than_days} days");
        let r1 = sqlx::query(
            "DELETE FROM domain_event_log WHERE timestamp < datetime('now', ?1)",
        )
        .bind(&cutoff)
        .execute(&self.pool)
        .await?;
        let r2 = sqlx::query(
            "DELETE FROM pipeline_event_log WHERE timestamp < datetime('now', ?1)",
        )
        .bind(&cutoff)
        .execute(&self.pool)
        .await?;
        Ok(r1.rows_affected() + r2.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn insert_and_list_domain_events() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        repo.insert_domain_event(
            "evt-1",
            "ChatTurnCompleted",
            "general",
            "extract",
            r#"{"msg":"hello"}"#,
            "2026-03-09T00:00:00Z",
        )
        .await
        .unwrap();

        repo.insert_domain_event(
            "evt-2",
            "DistractionDetected",
            "energy",
            "accumulate",
            r#"{"app":"twitter"}"#,
            "2026-03-09T00:01:00Z",
        )
        .await
        .unwrap();

        let events = repo.list_recent_domain_events(10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, "evt-2"); // newest first
        assert_eq!(events[1].id, "evt-1");
    }

    #[tokio::test]
    async fn insert_and_list_pipeline_events() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        repo.insert_pipeline_event(
            "pipe-1",
            "extraction",
            Some("user said hello"),
            Some(1),
            None,
            None,
            "2026-03-09T00:00:00Z",
        )
        .await
        .unwrap();

        repo.insert_pipeline_event(
            "pipe-2",
            "consolidation",
            None,
            None,
            Some("add"),
            Some("user.greeting = hello"),
            "2026-03-09T00:00:01Z",
        )
        .await
        .unwrap();

        let events = repo.list_recent_pipeline_events(10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_kind, "consolidation"); // newest first
        assert_eq!(events[1].event_kind, "extraction");
    }
}
