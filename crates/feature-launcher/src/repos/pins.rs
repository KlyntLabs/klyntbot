use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use storage::StoragePool;
use common::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Pin {
    pub item_id: String,
    pub kind: String,
    pub position: i64,
}

pub struct PinsRepo {
    pool: SqlitePool,
}

impl PinsRepo {
    pub fn new(pool: &StoragePool) -> Self {
        Self { pool: pool.inner().clone() }
    }

    pub async fn pin(&self, item_id: &str, kind: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO launcher_pins (item_id, kind, position) \
             SELECT ?, ?, COALESCE(MAX(position), -1) + 1 FROM launcher_pins \
             WHERE NOT EXISTS (SELECT 1 FROM launcher_pins WHERE item_id = ? AND kind = ?)",
        )
        .bind(item_id).bind(kind).bind(item_id).bind(kind)
        .execute(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(())
    }

    pub async fn unpin(&self, item_id: &str, kind: &str) -> Result<()> {
        sqlx::query("DELETE FROM launcher_pins WHERE item_id = ? AND kind = ?")
            .bind(item_id).bind(kind)
            .execute(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(())
    }

    pub async fn list_pinned(&self) -> Result<Vec<Pin>> {
        let pins = sqlx::query_as::<_, Pin>("SELECT item_id, kind, position FROM launcher_pins ORDER BY position ASC")
            .fetch_all(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(pins)
    }

    pub async fn is_pinned(&self, item_id: &str, kind: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM launcher_pins WHERE item_id = ? AND kind = ? LIMIT 1")
            .bind(item_id).bind(kind)
            .fetch_optional(&self.pool).await.map_err(storage::StorageError::from)?;
        Ok(row.is_some())
    }

    pub async fn pinned_set(&self) -> Result<std::collections::HashSet<(String, String)>> {
        let pins = self.list_pinned().await?;
        Ok(pins.into_iter().map(|p| (p.item_id, p.kind)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> StoragePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(&pool, &crate::launcher_migrations()).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn pin_and_list() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        repo.pin("app:/Applications/Slack.app", "application").await.unwrap();
        repo.pin("app:/Applications/VSCode.app", "application").await.unwrap();
        let pins = repo.list_pinned().await.unwrap();
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].position, 0);
        assert_eq!(pins[1].position, 1);
    }

    #[tokio::test]
    async fn unpin_removes() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        repo.pin("a", "application").await.unwrap();
        repo.unpin("a", "application").await.unwrap();
        assert_eq!(repo.list_pinned().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn is_pinned_query() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        assert!(!repo.is_pinned("a", "k").await.unwrap());
        repo.pin("a", "k").await.unwrap();
        assert!(repo.is_pinned("a", "k").await.unwrap());
    }

    #[tokio::test]
    async fn pin_is_idempotent() {
        let pool = setup().await;
        let repo = PinsRepo::new(&pool);
        repo.pin("a", "k").await.unwrap();
        repo.pin("a", "k").await.unwrap();
        let pins = repo.list_pinned().await.unwrap();
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].position, 0); // position preserved, not bumped
    }
}
