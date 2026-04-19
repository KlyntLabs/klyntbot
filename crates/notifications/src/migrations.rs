//! Registers the notifications-crate SQL migrations with the storage pool.
use tools_core::FeatureMigration;

pub fn migration() -> FeatureMigration {
    FeatureMigration {
        feature_name: "notifications".to_string(),
        version: 1,
        description: "Initial notification tables".to_string(),
        sql: include_str!("../migrations/001_notification_tables.sql").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn migration_applies_cleanly() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let mig = migration();
        sqlx::query(&mig.sql).execute(pool.inner()).await.unwrap();
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' \
             AND name IN ('notification_log','held_notifications')",
        )
        .fetch_all(pool.inner())
        .await
        .unwrap();
        assert_eq!(tables.len(), 2);
    }
}
