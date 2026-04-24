//! Repository for the `domain_event_log` and `pipeline_event_log` tables.

use bus::EventDomain;
use jiff::Timestamp;
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

/// Parameters for inserting a pipeline event (avoids too-many-arguments).
pub struct PipelineEventRecord<'a> {
    pub id: &'a str,
    pub event_kind: &'a str,
    pub observation: Option<&'a str>,
    pub facts_extracted: Option<i64>,
    pub operation: Option<&'a str>,
    pub fact_triple: Option<&'a str>,
    pub timestamp: &'a str,
}

impl EventLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a domain event into the log. `domain` is the typed
    /// `EventDomain`; it's converted to string once at the SQL binding
    /// boundary, eliminating stringly-typed call sites.
    pub async fn insert_domain_event(
        &self,
        id: &str,
        event_type: &str,
        domain: &EventDomain,
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
        .bind(domain.as_str())
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
        record: &PipelineEventRecord<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO pipeline_event_log
                (id, event_kind, observation, facts_extracted, operation, fact_triple, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(record.id)
        .bind(record.event_kind)
        .bind(record.observation)
        .bind(record.facts_extracted)
        .bind(record.operation)
        .bind(record.fact_triple)
        .bind(record.timestamp)
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

    /// Fetch domain events within a date range (inclusive), oldest first.
    /// Dates are ISO date strings like "2026-03-09".
    pub async fn query_domain_events_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DomainEventRow>, sqlx::Error> {
        // Use direct string comparison to allow idx_domain_event_log_timestamp.
        // Timestamps are stored as ISO-8601 strings, so "YYYY-MM-DD" compares
        // correctly against "YYYY-MM-DDThh:mm:ss".
        sqlx::query_as::<_, DomainEventRow>(
            r#"
            SELECT id, event_type, domain, salience, payload, timestamp
            FROM domain_event_log
            WHERE timestamp >= ?1
              AND timestamp < ?2
            ORDER BY timestamp ASC
            "#,
        )
        .bind(start_date)
        .bind(format!("{end_date}T23:59:59Z"))
        .fetch_all(&self.pool)
        .await
    }

    /// List domain events of a specific type since a timestamp (RFC 3339).
    pub async fn list_by_event_type_since(
        &self,
        event_type: &str,
        since: &str,
    ) -> Result<Vec<DomainEventRow>, sqlx::Error> {
        sqlx::query_as::<_, DomainEventRow>(
            "SELECT id, event_type, domain, salience, payload, timestamp
             FROM domain_event_log
             WHERE event_type = ?1 AND timestamp >= ?2
             ORDER BY timestamp ASC",
        )
        .bind(event_type)
        .bind(since)
        .fetch_all(&self.pool)
        .await
    }

    /// Count domain events of a specific type since a given timestamp.
    pub async fn count_by_event_type(
        &self,
        event_type: &str,
        since: Timestamp,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM domain_event_log
             WHERE event_type = ?1 AND timestamp >= ?2",
        )
        .bind(event_type)
        .bind(since.to_string())
        .fetch_one(&self.pool)
        .await
    }

    /// Count domain events where the JSON data contains a specific substring.
    pub async fn count_by_event_type_and_data(
        &self,
        event_type: &str,
        data_contains: &str,
        since: Timestamp,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM domain_event_log
             WHERE event_type = ?1 AND json_extract(payload, '$') LIKE ?2 AND timestamp >= ?3",
        )
        .bind(event_type)
        .bind(format!("%{data_contains}%"))
        .bind(since.to_string())
        .fetch_one(&self.pool)
        .await
    }

    /// Extraction yield by domain: average facts_extracted per extraction event,
    /// grouped by the domain of facts created in the same time window.
    pub async fn extraction_yield_by_domain(
        &self,
        since: &str,
    ) -> Result<Vec<(String, f64)>, sqlx::Error> {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT sf.domain, AVG(COALESCE(pe.facts_extracted, 0)) as avg_yield
             FROM pipeline_event_log pe
             JOIN semantic_facts sf ON sf.recorded_at >= pe.timestamp
                AND sf.recorded_at < datetime(pe.timestamp, '+1 hour')
             WHERE pe.event_kind = 'extraction'
                AND pe.timestamp > ?1
             GROUP BY sf.domain
             HAVING COUNT(*) >= 2",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete events older than the given number of days.
    pub async fn prune_old_events(&self, older_than_days: i64) -> Result<u64, sqlx::Error> {
        let cutoff = format!("-{older_than_days} days");
        let r1 = sqlx::query("DELETE FROM domain_event_log WHERE timestamp < datetime('now', ?1)")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        let r2 =
            sqlx::query("DELETE FROM pipeline_event_log WHERE timestamp < datetime('now', ?1)")
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
            bus::DomainEvent::KIND_CHAT_TURN_COMPLETED,
            &EventDomain::General,
            "extract",
            r#"{"msg":"hello"}"#,
            "2026-03-09T00:00:00Z",
        )
        .await
        .unwrap();

        repo.insert_domain_event(
            "evt-2",
            bus::DomainEvent::KIND_DISTRACTION_DETECTED,
            &EventDomain::Energy,
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
    async fn query_range_filters_by_date() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        repo.insert_domain_event(
            "evt-a",
            bus::DomainEvent::KIND_TASK_CREATED,
            &EventDomain::Work,
            "extract",
            r#"{"task_id":"t1"}"#,
            "2026-03-08T10:00:00Z",
        )
        .await
        .unwrap();

        repo.insert_domain_event(
            "evt-b",
            bus::DomainEvent::KIND_NOTE_CREATED,
            &EventDomain::Notes,
            "extract",
            r#"{"note_id":"n1"}"#,
            "2026-03-09T14:30:00Z",
        )
        .await
        .unwrap();

        repo.insert_domain_event(
            "evt-c",
            bus::DomainEvent::KIND_TASK_COMPLETED,
            &EventDomain::Work,
            "extract",
            r#"{"task_id":"t2"}"#,
            "2026-03-10T08:00:00Z",
        )
        .await
        .unwrap();

        // Query only March 9
        let results = repo
            .query_domain_events_range("2026-03-09", "2026-03-09")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "evt-b");

        // Query March 8-9
        let results = repo
            .query_domain_events_range("2026-03-08", "2026-03-09")
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn insert_and_list_pipeline_events() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        repo.insert_pipeline_event(&PipelineEventRecord {
            id: "pipe-1",
            event_kind: "extraction",
            observation: Some("user said hello"),
            facts_extracted: Some(1),
            operation: None,
            fact_triple: None,
            timestamp: "2026-03-09T00:00:00Z",
        })
        .await
        .unwrap();

        repo.insert_pipeline_event(&PipelineEventRecord {
            id: "pipe-2",
            event_kind: "consolidation",
            observation: None,
            facts_extracted: None,
            operation: Some("add"),
            fact_triple: Some("user.greeting = hello"),
            timestamp: "2026-03-09T00:00:01Z",
        })
        .await
        .unwrap();

        let events = repo.list_recent_pipeline_events(10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_kind, "consolidation"); // newest first
        assert_eq!(events[1].event_kind, "extraction");
    }

    #[tokio::test]
    async fn count_by_event_type_filters_correctly() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        // Insert events with different types and timestamps
        repo.insert_domain_event(
            "evt-c1",
            bus::DomainEvent::KIND_TASK_CREATED,
            &EventDomain::Work,
            "extract",
            r#"{"task_id":"t1"}"#,
            "2026-03-09T10:00:00Z",
        )
        .await
        .unwrap();

        repo.insert_domain_event(
            "evt-c2",
            bus::DomainEvent::KIND_TASK_CREATED,
            &EventDomain::Work,
            "extract",
            r#"{"task_id":"t2"}"#,
            "2026-03-09T14:00:00Z",
        )
        .await
        .unwrap();

        repo.insert_domain_event(
            "evt-c3",
            bus::DomainEvent::KIND_NOTE_CREATED,
            &EventDomain::Notes,
            "extract",
            r#"{"note_id":"n1"}"#,
            "2026-03-09T12:00:00Z",
        )
        .await
        .unwrap();

        // Old event that should be excluded by the since filter
        repo.insert_domain_event(
            "evt-c4",
            bus::DomainEvent::KIND_TASK_CREATED,
            &EventDomain::Work,
            "extract",
            r#"{"task_id":"t0"}"#,
            "2026-03-08T09:00:00Z",
        )
        .await
        .unwrap();

        let since: Timestamp = "2026-03-09T00:00:00Z".parse().unwrap();

        // Should count only the two TaskCreated events on March 9
        let count = repo
            .count_by_event_type(bus::DomainEvent::KIND_TASK_CREATED, since)
            .await
            .unwrap();
        assert_eq!(count, 2);

        // NoteCreated should return 1
        let count = repo
            .count_by_event_type(bus::DomainEvent::KIND_NOTE_CREATED, since)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Non-existent event type should return 0
        let count = repo.count_by_event_type("Unknown", since).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn count_by_event_type_and_data_does_not_panic() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        repo.insert_domain_event(
            "evt-d1",
            bus::DomainEvent::KIND_TASK_CREATED,
            &EventDomain::Work,
            "extract",
            r#"{"task_id":"t1"}"#,
            "2026-03-09T10:00:00Z",
        )
        .await
        .unwrap();

        let since: Timestamp = "2026-03-09T00:00:00Z".parse().unwrap();

        let count = repo
            .count_by_event_type_and_data(bus::DomainEvent::KIND_TASK_CREATED, "t1", since)
            .await
            .unwrap();
        assert_eq!(count, 1);

        let count = repo
            .count_by_event_type_and_data(bus::DomainEvent::KIND_TASK_CREATED, "nonexistent", since)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
