//! SchemaMirrorSubscriber — tracks per-database field usage from entity events.

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use bus::DomainEvent;
use sqlx::SqlitePool;

pub struct SchemaMirrorSubscriber {
    pool: SqlitePool,
}

impl SchemaMirrorSubscriber {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run(self, mut rx: broadcast::Receiver<DomainEvent>, shutdown: CancellationToken) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("SchemaMirrorSubscriber: shutdown");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::EntityCreated { database_id, .. }) => {
                            self.record_usage(&database_id, "entity_created", "entity_created").await;
                        }
                        Ok(DomainEvent::EntityUpdated { database_id, changed_fields, .. }) => {
                            for field in &changed_fields {
                                self.record_usage(&database_id, field, "field_updated").await;
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("SchemaMirrorSubscriber lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }

    async fn record_usage(&self, database_id: &str, field_id: &str, usage_type: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO mirror_schema_observations (id, database_id, field_id, usage_type, count, last_used_at)
             VALUES (?, ?, ?, ?, 1, ?)
             ON CONFLICT(database_id, field_id, usage_type)
             DO UPDATE SET count = count + 1, last_used_at = excluded.last_used_at",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(database_id)
        .bind(field_id)
        .bind(usage_type)
        .bind(&now)
        .execute(&self.pool)
        .await;

        if let Err(e) = result {
            warn!("SchemaMirrorSubscriber: failed to record usage: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_schema_subscriber_records_entity_created() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::repos::cognitive_migrations(),
        )
        .await
        .unwrap();

        let bus = Arc::new(bus::DomainEventBus::new(16));
        let sub = SchemaMirrorSubscriber::new(pool.inner().clone());
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(sub.run(bus.subscribe(), shutdown.clone()));

        bus.publish(DomainEvent::EntityCreated {
            database_id: "db-001".into(),
            entity_id: "e-001".into(),
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let count: i64 = sqlx::query_scalar(
            "SELECT count FROM mirror_schema_observations WHERE database_id = 'db-001' AND usage_type = 'entity_created'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert_eq!(count, 1);

        shutdown.cancel();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_schema_subscriber_records_field_updates() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        storage::StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::repos::cognitive_migrations(),
        )
        .await
        .unwrap();

        let bus = Arc::new(bus::DomainEventBus::new(16));
        let sub = SchemaMirrorSubscriber::new(pool.inner().clone());
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(sub.run(bus.subscribe(), shutdown.clone()));

        bus.publish(DomainEvent::EntityUpdated {
            database_id: "db-001".into(),
            entity_id: "e-001".into(),
            changed_fields: vec!["status".into(), "priority".into()],
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT field_id, count FROM mirror_schema_observations WHERE database_id = 'db-001' AND usage_type = 'field_updated' ORDER BY field_id",
        )
        .fetch_all(pool.inner())
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "priority");
        assert_eq!(rows[1].0, "status");

        shutdown.cancel();
        handle.await.unwrap();
    }
}
