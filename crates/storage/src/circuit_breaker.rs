//! Persistence for circuit breaker state across restarts.

use jiff::Timestamp;

use crate::{StorageError, StoragePool};

/// Create the `circuit_breaker_state` table if it doesn't exist.
pub async fn ensure_table(pool: &StoragePool) -> Result<(), StorageError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS circuit_breaker_state \
         (id INTEGER PRIMARY KEY, open_until_utc TEXT NOT NULL)",
    )
    .execute(pool.inner())
    .await?;
    Ok(())
}

/// Load the persisted circuit-open deadline, if any.
pub async fn load(pool: &StoragePool) -> Result<Option<Timestamp>, StorageError> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT open_until_utc FROM circuit_breaker_state WHERE id = 1")
            .fetch_optional(pool.inner())
            .await?;
    Ok(row.and_then(|(ts,)| ts.parse::<Timestamp>().ok()))
}

/// Persist the circuit-open deadline (upsert — only one row ever exists).
pub async fn save(pool: &StoragePool, open_until: Timestamp) -> Result<(), StorageError> {
    sqlx::query("INSERT OR REPLACE INTO circuit_breaker_state (id, open_until_utc) VALUES (1, ?1)")
        .bind(open_until.to_string())
        .execute(pool.inner())
        .await?;
    Ok(())
}
