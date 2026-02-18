// Calendar sync state persistence

use crate::types::SyncState;
use common::Result;
use std::path::PathBuf;
use tokio::fs;

/// Get the base directory for sync state files (~/.klyntbot/calendar_sync_states/).
fn sync_states_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".klyntbot")
        .join("calendar_sync_states")
}

/// Get sync state file path for a specific provider.
/// Path: ~/.klyntbot/calendar_sync_states/{provider_id}.json
pub fn get_provider_sync_state_path(provider_id: &str) -> PathBuf {
    sync_states_dir().join(format!("{}.json", provider_id))
}

/// Load sync state for a specific provider.
pub async fn load_provider_sync_state(provider_id: &str) -> Result<SyncState> {
    let path = get_provider_sync_state_path(provider_id);

    if !path.exists() {
        return Ok(SyncState {
            sync_token: None,
            last_sync: None,
        });
    }

    let contents = fs::read_to_string(&path).await?;
    let state: SyncState = serde_json::from_str(&contents)?;
    Ok(state)
}

/// Save sync state for a specific provider.
pub async fn save_provider_sync_state(provider_id: &str, state: &SyncState) -> Result<()> {
    let path = get_provider_sync_state_path(provider_id);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let contents = serde_json::to_string_pretty(state)?;
    fs::write(&path, contents).await?;
    Ok(())
}

// ── SQL-backed variants ──────────────────────────────────────────────

/// Load sync state for a provider from the SQL backend.
pub async fn load_provider_sync_state_sql(
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
pub async fn save_provider_sync_state_sql(
    repo: &storage::CalendarSyncRepo,
    provider_id: &str,
    state: &SyncState,
) -> Result<()> {
    repo.upsert(
        provider_id,
        state.sync_token.as_deref(),
        state.last_sync,
    )
    .await
    .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_provider_sync_state_path() {
        let path = get_provider_sync_state_path("apple");
        assert!(path.to_string_lossy().contains("calendar_sync_states"));
        assert!(path.to_string_lossy().ends_with("apple.json"));

        let path = get_provider_sync_state_path("google");
        assert!(path.to_string_lossy().ends_with("google.json"));

        let path = get_provider_sync_state_path("generic-nextcloud");
        assert!(path.to_string_lossy().ends_with("generic-nextcloud.json"));
    }

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

    #[tokio::test]
    async fn test_default_state() {
        let state = SyncState {
            sync_token: None,
            last_sync: None,
        };

        assert!(state.sync_token.is_none());
        assert!(state.last_sync.is_none());

        let json = serde_json::to_string(&state).unwrap();
        let loaded: SyncState = serde_json::from_str(&json).unwrap();
        assert!(loaded.sync_token.is_none());
    }

    #[tokio::test]
    async fn test_provider_sync_state_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let provider_id = "test-provider";

        // We can't easily test with real paths, but verify serialization
        let state = SyncState {
            sync_token: Some("token-abc".to_string()),
            last_sync: Some(Utc::now()),
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let path = temp_dir.path().join(format!("{}.json", provider_id));
        tokio::fs::write(&path, &json).await.unwrap();

        let loaded_json = tokio::fs::read_to_string(&path).await.unwrap();
        let loaded: SyncState = serde_json::from_str(&loaded_json).unwrap();
        assert_eq!(loaded.sync_token, state.sync_token);
    }
}
