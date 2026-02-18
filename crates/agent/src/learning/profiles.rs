//! User and channel profiles — tracks preferences and usage patterns.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use common::Result;

/// Preferred response verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseLength {
    Short,
    Medium,
    Long,
}

impl Default for ResponseLength {
    fn default() -> Self {
        Self::Medium
    }
}

/// Persistent user profile with preferences and usage patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub preferred_response_length: ResponseLength,
    pub frequently_used_tools: Vec<String>,
    pub satisfaction_score: f32,
    pub total_interactions: u64,
}

impl UserProfile {
    pub fn new(user_id: &str) -> Self {
        Self {
            user_id: user_id.to_string(),
            preferred_response_length: ResponseLength::default(),
            frequently_used_tools: Vec::new(),
            satisfaction_score: 0.5, // neutral starting point
            total_interactions: 0,
        }
    }
}

/// JSONL-backed store for user profiles.
pub struct ProfileStore {
    data_dir: PathBuf,
}

impl ProfileStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    fn profile_path(&self, user_id: &str) -> PathBuf {
        // Sanitize user_id for filename
        let safe_id: String = user_id
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.data_dir.join(format!("profile_{}.json", safe_id))
    }

    /// Get an existing profile or create a new default one.
    pub async fn get_or_create(&self, user_id: &str) -> Result<UserProfile> {
        let path = self.profile_path(user_id);

        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            match serde_json::from_str::<UserProfile>(&content) {
                Ok(profile) => return Ok(profile),
                Err(_) => {} // corrupt file, create fresh
            }
        }

        Ok(UserProfile::new(user_id))
    }

    /// Persist a user profile to disk.
    pub async fn update(&self, profile: &UserProfile) -> Result<()> {
        tokio::fs::create_dir_all(&self.data_dir).await?;
        let path = self.profile_path(&profile.user_id);
        let content = serde_json::to_string_pretty(profile).map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to serialize profile: {}", e))
        })?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_profile_defaults() {
        let profile = UserProfile::new("user-123");
        assert_eq!(profile.user_id, "user-123");
        assert_eq!(profile.preferred_response_length, ResponseLength::Medium);
        assert_eq!(profile.satisfaction_score, 0.5);
        assert!(profile.frequently_used_tools.is_empty());
    }

    #[tokio::test]
    async fn test_get_or_create_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().to_path_buf());
        let profile = store.get_or_create("new-user").await.unwrap();
        assert_eq!(profile.user_id, "new-user");
    }

    #[tokio::test]
    async fn test_update_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().to_path_buf());

        let mut profile = UserProfile::new("test-user");
        profile.preferred_response_length = ResponseLength::Short;
        profile.satisfaction_score = 0.9;
        profile.total_interactions = 42;
        store.update(&profile).await.unwrap();

        let loaded = store.get_or_create("test-user").await.unwrap();
        assert_eq!(loaded.preferred_response_length, ResponseLength::Short);
        assert_eq!(loaded.satisfaction_score, 0.9);
        assert_eq!(loaded.total_interactions, 42);
    }

    #[tokio::test]
    async fn test_profile_path_sanitizes_special_chars() {
        let store = ProfileStore::new(PathBuf::from("/tmp"));
        let path = store.profile_path("telegram:12345");
        assert!(path.to_str().unwrap().contains("telegram_12345"));
    }
}
