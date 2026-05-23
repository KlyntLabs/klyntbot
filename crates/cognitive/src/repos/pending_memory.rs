//! Repository for memory writes pending user confirmation.

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct PendingMemoryRepo {
    pool: SqlitePool,
}

const MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS pending_memories (
    id         TEXT PRIMARY KEY,
    fact_json  TEXT NOT NULL,
    reason     TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

impl PendingMemoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(MIGRATION).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn insert(
        &self,
        fact: &crate::types::SemanticFact,
        reason: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO pending_memories (id, fact_json, reason, created_at) \
             VALUES (?1, ?2, ?3, datetime('now'))",
        )
        .bind(&fact.id)
        .bind(serde_json::to_string(fact).unwrap_or_default())
        .bind(reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<PendingMemoryRow>, sqlx::Error> {
        sqlx::query_as::<_, PendingMemoryRow>("SELECT * FROM pending_memories WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn list_pending(&self, limit: i64) -> Vec<PendingMemoryRow> {
        sqlx::query_as::<_, PendingMemoryRow>(
            "SELECT * FROM pending_memories ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }

    /// Remove a pending memory entry (used for both approve and reject).
    /// The caller is responsible for persisting the fact to SemanticFactRepo on approval.
    pub async fn remove(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM pending_memories WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Delete pending entries older than `days` days. Returns count deleted.
    pub async fn cleanup_older_than(&self, days: i64) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM pending_memories \
             WHERE julianday('now') - julianday(created_at) > ?1",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingMemoryRow {
    pub id: String,
    pub fact_json: String,
    pub reason: String,
    pub created_at: String,
}
