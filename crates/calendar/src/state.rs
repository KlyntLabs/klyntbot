// Calendar sync state persistence

use crate::types::SyncState;
use common::Result;
use std::path::PathBuf;
use tokio::fs;

/// Get sync state file path (~/.klyntbot/calendar_sync.json)
pub fn get_sync_state_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".klyntbot")
        .join("calendar_sync.json")
}

/// Load sync state from file
pub async fn load_sync_state() -> Result<SyncState> {
    let path = get_sync_state_path();

    if !path.exists() {
        // Return default state if file doesn't exist
        return Ok(SyncState {
            sync_token: None,
            last_sync: None,
        });
    }

    let contents = fs::read_to_string(&path).await?;
    let state: SyncState = serde_json::from_str(&contents)?;
    Ok(state)
}

/// Save sync state to file
pub async fn save_sync_state(state: &SyncState) -> Result<()> {
    let path = get_sync_state_path();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let contents = serde_json::to_string_pretty(state)?;
    fs::write(&path, contents).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_sync_state_path() {
        let path = get_sync_state_path();
        assert!(path.to_string_lossy().contains(".klyntbot"));
        assert!(path.to_string_lossy().ends_with("calendar_sync.json"));
    }

    #[tokio::test]
    async fn test_state_serialization() {
        // Test state can be serialized and deserialized
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
        // Test default state structure
        let state = SyncState {
            sync_token: None,
            last_sync: None,
        };

        assert!(state.sync_token.is_none());
        assert!(state.last_sync.is_none());

        // Verify it can be serialized
        let json = serde_json::to_string(&state).unwrap();
        let loaded: SyncState = serde_json::from_str(&json).unwrap();
        assert!(loaded.sync_token.is_none());
    }
}
