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

/// Legacy path for backward compatibility (~/.klyntbot/calendar_sync.json).
pub fn get_sync_state_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".klyntbot")
        .join("calendar_sync.json")
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

/// Load sync state from the legacy file (backward compat).
pub async fn load_sync_state() -> Result<SyncState> {
    let path = get_sync_state_path();

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

/// Save sync state to the legacy file (backward compat).
pub async fn save_sync_state(state: &SyncState) -> Result<()> {
    let path = get_sync_state_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let contents = serde_json::to_string_pretty(state)?;
    fs::write(&path, contents).await?;
    Ok(())
}

/// Migrate legacy sync state to per-provider format.
/// If ~/.klyntbot/calendar_sync.json exists and no apple state exists,
/// move it to ~/.klyntbot/calendar_sync_states/apple.json.
pub async fn migrate_legacy_sync_state() -> Result<()> {
    let legacy_path = get_sync_state_path();
    let apple_path = get_provider_sync_state_path("apple");

    if legacy_path.exists() && !apple_path.exists() {
        tracing::info!("Migrating legacy calendar_sync.json to calendar_sync_states/apple.json");

        if let Some(parent) = apple_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let contents = fs::read_to_string(&legacy_path).await?;
        fs::write(&apple_path, contents).await?;

        // Keep old file around for safety — don't delete
        tracing::info!("Legacy sync state migrated successfully");
    }

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
