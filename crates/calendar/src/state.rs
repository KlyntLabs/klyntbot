// Calendar sync state persistence (SQL-only)

use crate::types::SyncState;
use common::Result;

/// Load sync state for a provider from the SQL backend.
pub async fn load_provider_sync_state(
    repo: &storage::CalendarSyncRepo,
    provider_id: &str,
) -> Result<SyncState> {
    match repo.get(provider_id).await {
        Ok(row) => Ok(SyncState {
            sync_token: row.sync_token,
            last_sync: row.last_sync_at,
        }),
        Err(storage::StorageError::NotFound(_)) => Ok(SyncState {
            sync_token: None,
            last_sync: None,
        }),
        Err(e) => Err(common::ToolError::ExecutionFailed(e.to_string()).into()),
    }
}

/// Save sync state for a provider to the SQL backend.
pub async fn save_provider_sync_state(
    repo: &storage::CalendarSyncRepo,
    provider_id: &str,
    state: &SyncState,
) -> Result<()> {
    repo.upsert(provider_id, state.sync_token.as_deref(), state.last_sync)
        .await
        .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_state_serialization() {
        let original_state = SyncState {
            sync_token: Some("test-token-456".to_string()),
            last_sync: Some(Utc::now()),
        };

        let json = serde_json::to_string(&original_state).unwrap();
        let loaded_state: SyncState = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded_state.sync_token, original_state.sync_token);
        assert!(loaded_state.last_sync.is_some());
    }
}
