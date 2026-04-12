//! entity-store: Dynamic schema management and entity CRUD for klyntbot.

pub mod evolution;
pub mod query;
pub mod relations;
pub mod schema_ops;
pub mod skill_binding;
pub mod store;
pub mod templates;
pub mod types;
pub mod views;

pub use types::*;

use tools_core::FeatureMigration;

pub struct EntityStoreFeature;

impl EntityStoreFeature {
    pub fn migrations() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "entity_store".to_string(),
            version: 1,
            description:
                "Foundation tables: databases, fields, views, dashboards, relations, evolution"
                    .to_string(),
            sql: include_str!("../migrations/001_entity_store.sql").to_string(),
        }]
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::store::EntityStore;

    pub async fn setup_test_store() -> EntityStore {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let sql = include_str!("../migrations/001_entity_store.sql");
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(pool.inner()).await.unwrap();
            }
        }
        EntityStore::new(pool.inner().clone())
    }

    /// Setup returning raw pool (for modules that operate on SqlitePool directly).
    pub async fn setup_test_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let sql = include_str!("../migrations/001_entity_store.sql");
        for statement in sql.split(';') {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(pool.inner()).await.unwrap();
            }
        }
        pool.inner().clone()
    }
}
