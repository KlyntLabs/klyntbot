//! Repository for the `accumulated_observations` table.
//!
//! Persists buffered observations so they survive app restarts.

use std::collections::{HashMap, HashSet};

use jiff::Timestamp;
use sqlx::SqlitePool;
use tracing::warn;

use crate::types::Observation;

/// Row stored in `accumulated_observations`.
#[derive(Debug, sqlx::FromRow)]
struct AccumulatedRow {
    #[allow(dead_code)]
    id: String,
    event_type_key: String,
    domain: String,
    content: String,
    importance: f64,
    source_event: String,
    observed_at: String,
    day_key: String,
}

/// Reconstructed in-memory entry from persisted rows.
#[derive(Debug, Clone)]
pub struct PersistedAccumulatedEntry {
    pub observations: Vec<Observation>,
    pub days_seen: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct AccumulatedObservationRepo {
    pool: SqlitePool,
}

impl AccumulatedObservationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Load all accumulated entries, grouped by event_type_key.
    pub async fn load_all(&self) -> HashMap<String, PersistedAccumulatedEntry> {
        let rows: Vec<AccumulatedRow> = match sqlx::query_as(
            "SELECT id, event_type_key, domain, content, importance, source_event, observed_at, day_key FROM accumulated_observations",
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to load accumulated observations: {e}");
                return HashMap::new();
            }
        };

        let mut map: HashMap<String, PersistedAccumulatedEntry> = HashMap::new();
        for row in rows {
            let entry =
                map.entry(row.event_type_key)
                    .or_insert_with(|| PersistedAccumulatedEntry {
                        observations: Vec::new(),
                        days_seen: HashSet::new(),
                    });
            entry.days_seen.insert(row.day_key);

            let timestamp = row
                .observed_at
                .parse::<Timestamp>()
                .unwrap_or_else(|_| Timestamp::now());
            entry.observations.push(Observation {
                domain: row.domain,
                content: row.content,
                importance: row.importance,
                source_event: row.source_event,
                timestamp,
            });
        }
        map
    }

    /// Persist a single observation for an event type.
    pub async fn insert(&self, event_type_key: &str, obs: &Observation) {
        let id = uuid::Uuid::new_v4().to_string();
        let observed_at = obs.timestamp.to_string();
        let day_key = obs.timestamp.strftime("%Y-%m-%d").to_string();

        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO accumulated_observations
                (id, event_type_key, domain, content, importance, source_event, observed_at, day_key)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&id)
        .bind(event_type_key)
        .bind(&obs.domain)
        .bind(&obs.content)
        .bind(obs.importance)
        .bind(&obs.source_event)
        .bind(&observed_at)
        .bind(&day_key)
        .execute(&self.pool)
        .await
        {
            warn!("Failed to persist accumulated observation: {e}");
        }
    }

    /// Delete observations older than `days` days.
    /// Returns the number of rows removed.
    pub async fn delete_older_than(&self, days: i64) -> Result<u64, sqlx::Error> {
        let cutoff = (Timestamp::now() - jiff::SignedDuration::from_secs(days * 86400)).to_string();
        let result = sqlx::query("DELETE FROM accumulated_observations WHERE observed_at < ?1")
            .bind(&cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Increment the near-miss counter for a given event type key.
    pub async fn increment_near_miss(&self, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE accumulated_observations SET near_miss_count = near_miss_count + 1
             WHERE event_type_key = ?1",
        )
        .bind(key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List the most recent observations across all event types.
    pub async fn list_recent(&self, limit: i64) -> Vec<Observation> {
        let rows: Vec<AccumulatedRow> = match sqlx::query_as(
            "SELECT id, event_type_key, domain, content, importance, source_event, observed_at, day_key \
             FROM accumulated_observations \
             ORDER BY observed_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to list recent accumulated observations: {e}");
                return Vec::new();
            }
        };

        rows.into_iter()
            .map(|row| Observation {
                domain: row.domain,
                content: row.content,
                importance: row.importance,
                source_event: row.source_event,
                timestamp: row.observed_at.parse().unwrap_or_else(|_| Timestamp::now()),
            })
            .collect()
    }

    /// Delete all observations for an event type (after promotion).
    pub async fn delete_by_key(&self, event_type_key: &str) {
        if let Err(e) =
            sqlx::query("DELETE FROM accumulated_observations WHERE event_type_key = ?1")
                .bind(event_type_key)
                .execute(&self.pool)
                .await
        {
            warn!("Failed to delete accumulated observations for '{event_type_key}': {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn test_insert_and_load() {
        let pool = setup().await;
        let repo = AccumulatedObservationRepo::new(pool);

        let obs = Observation {
            domain: "productivity".into(),
            content: "Score: 72".into(),
            importance: 0.5,
            source_event: "ProductivityScoreComputed".into(),
            timestamp: Timestamp::now(),
        };
        repo.insert("ProductivityScoreComputed", &obs).await;

        let loaded = repo.load_all().await;
        assert_eq!(loaded.len(), 1);
        let entry = &loaded["ProductivityScoreComputed"];
        assert_eq!(entry.observations.len(), 1);
        assert_eq!(entry.observations[0].content, "Score: 72");
        assert_eq!(entry.days_seen.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_by_key() {
        let pool = setup().await;
        let repo = AccumulatedObservationRepo::new(pool);

        let obs = Observation {
            domain: "productivity".into(),
            content: "Score: 72".into(),
            importance: 0.5,
            source_event: "ProductivityScoreComputed".into(),
            timestamp: Timestamp::now(),
        };
        repo.insert("ProductivityScoreComputed", &obs).await;
        repo.insert("ProductivityScoreComputed", &obs).await;
        repo.insert("OtherEvent", &obs).await;

        repo.delete_by_key("ProductivityScoreComputed").await;

        let loaded = repo.load_all().await;
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("OtherEvent"));
    }

    #[tokio::test]
    async fn test_delete_older_than() {
        let pool = setup().await;
        let repo = AccumulatedObservationRepo::new(pool.clone());

        // Insert a row with an old timestamp directly
        sqlx::query(
            "INSERT INTO accumulated_observations \
             (id, event_type_key, domain, content, importance, source_event, observed_at, day_key) \
             VALUES ('old-id', 'OldEvent', 'productivity', 'old content', 0.5, 'src', '2020-01-01T00:00:00Z', '2020-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a recent observation that should NOT be deleted
        let obs = Observation {
            domain: "productivity".into(),
            content: "recent".into(),
            importance: 0.5,
            source_event: "RecentEvent".into(),
            timestamp: Timestamp::now(),
        };
        repo.insert("RecentEvent", &obs).await;

        let deleted = repo.delete_older_than(7).await.unwrap();
        assert_eq!(deleted, 1);

        let loaded = repo.load_all().await;
        assert!(loaded.contains_key("RecentEvent"));
        assert!(!loaded.contains_key("OldEvent"));
    }

    #[tokio::test]
    async fn test_days_seen_reconstruction() {
        let pool = setup().await;
        let repo = AccumulatedObservationRepo::new(pool);

        let day1: Timestamp = "2026-03-01T10:00:00Z".parse().unwrap();
        let day2: Timestamp = "2026-03-02T14:00:00Z".parse().unwrap();
        let day1_again: Timestamp = "2026-03-01T16:00:00Z".parse().unwrap();

        for ts in &[day1, day2, day1_again] {
            let obs = Observation {
                domain: "productivity".into(),
                content: "test".into(),
                importance: 0.5,
                source_event: "test".into(),
                timestamp: *ts,
            };
            repo.insert("TestEvent", &obs).await;
        }

        let loaded = repo.load_all().await;
        let entry = &loaded["TestEvent"];
        assert_eq!(entry.observations.len(), 3);
        assert_eq!(entry.days_seen.len(), 2); // Only 2 distinct days
    }
}
