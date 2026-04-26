//! Supersede outdated skill versions.

use common::{KlyntbotError, Result};
use storage::StoragePool;

/// Mark all but the latest version per `skill_name` as `status = 'superseded'`.
pub async fn supersede_outdated_versions(
    pool: &StoragePool,
    skill_name: &str,
) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE skill_versions
         SET status = 'superseded'
         WHERE skill_name = ?1
           AND status = 'active'
           AND version < (
               SELECT MAX(version) FROM skill_versions WHERE skill_name = ?1
           )",
    )
    .bind(skill_name)
    .execute(pool.inner())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("supersede_outdated_versions: {e}")))?;

    Ok(result.rows_affected())
}
