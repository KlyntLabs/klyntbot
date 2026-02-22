//! Conversation memory configuration: embedding and search settings.

use serde::{Deserialize, Serialize};

use super::core::{default_semantic_threshold, default_true};

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    /// Maximum number of history messages to load (default: 50)
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    /// Number of days before an inactive session is considered stale and deleted (default: 30)
    #[serde(default = "default_ttl_days")]
    pub ttl_days: u32,
    /// How often the cleanup service runs, in hours (default: 1)
    #[serde(default = "default_cleanup_interval_hours")]
    pub cleanup_interval_hours: u32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
            ttl_days: default_ttl_days(),
            cleanup_interval_hours: default_cleanup_interval_hours(),
        }
    }
}

fn default_history_limit() -> usize {
    50
}

fn default_ttl_days() -> u32 {
    30
}

fn default_cleanup_interval_hours() -> u32 {
    1
}

/// Memory decay and consolidation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryConfig {
    /// Half-life in days for time-decay scoring (default: 138, i.e. 0.995^138 ≈ 0.5)
    #[serde(default = "default_decay_half_life_days")]
    pub decay_half_life_days: u32,
    /// Maximum age in days before an embedding is pruned by maintenance (default: 90)
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
    /// Enable background consolidation of old embeddings (default: false)
    #[serde(default)]
    pub consolidation_enabled: bool,
    /// How often the memory maintenance service runs, in hours (default: 24)
    #[serde(default = "default_maintenance_interval_hours")]
    pub maintenance_interval_hours: u32,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            decay_half_life_days: default_decay_half_life_days(),
            max_age_days: default_max_age_days(),
            consolidation_enabled: false,
            maintenance_interval_hours: default_maintenance_interval_hours(),
        }
    }
}

fn default_decay_half_life_days() -> u32 {
    138
}

fn default_max_age_days() -> u32 {
    90
}

fn default_maintenance_interval_hours() -> u32 {
    24
}

/// Conversation memory configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationConfig {
    #[serde(default)]
    pub embedding: ConversationEmbeddingConfig,
    #[serde(default)]
    pub search: ConversationSearchConfig,
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
}

/// Conversation embedding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationEmbeddingConfig {
    /// Enable automatic conversation embedding (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Channels to exclude from conversation embedding (default: [])
    #[serde(default)]
    pub exclude_channels: Vec<String>,
    /// Message roles to exclude from embedding (default: ["system", "tool"])
    #[serde(default = "default_exclude_roles")]
    pub exclude_roles: Vec<String>,
}

impl Default for ConversationEmbeddingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            exclude_channels: Vec::new(),
            exclude_roles: default_exclude_roles(),
        }
    }
}

/// Conversation search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSearchConfig {
    /// Enable conversation search (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Semantic similarity threshold for search results (0.0-1.0, default: 0.5)
    #[serde(default = "default_semantic_threshold")]
    pub semantic_threshold: f64,
    /// Maximum number of search results to return (default: 20)
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

impl Default for ConversationSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            semantic_threshold: default_semantic_threshold(),
            max_results: default_max_results(),
        }
    }
}

fn default_exclude_roles() -> Vec<String> {
    vec!["system".to_string(), "tool".to_string()]
}

fn default_max_results() -> usize {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_history_limit_default() {
        let json = serde_json::json!({});
        let config: ConversationConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.session.history_limit, 50);
    }

    #[test]
    fn test_session_history_limit_custom() {
        let json = serde_json::json!({
            "session": { "historyLimit": 100 }
        });
        let config: ConversationConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.session.history_limit, 100);
    }

    #[test]
    fn test_session_ttl_days_default() {
        let config = SessionConfig::default();
        assert_eq!(config.ttl_days, 30);
    }

    #[test]
    fn test_session_cleanup_interval_hours_default() {
        let config = SessionConfig::default();
        assert_eq!(config.cleanup_interval_hours, 1);
    }

    #[test]
    fn test_session_ttl_custom() {
        let json = serde_json::json!({
            "session": { "ttlDays": 7, "cleanupIntervalHours": 6 }
        });
        let config: ConversationConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.session.ttl_days, 7);
        assert_eq!(config.session.cleanup_interval_hours, 6);
    }

    #[test]
    fn test_memory_config_defaults() {
        let config = MemoryConfig::default();
        assert_eq!(config.decay_half_life_days, 138);
        assert_eq!(config.max_age_days, 90);
        assert!(!config.consolidation_enabled);
        assert_eq!(config.maintenance_interval_hours, 24);
    }

    #[test]
    fn test_memory_config_decay_half_life_math() {
        // 0.995^138 ≈ 0.5 (half-life property)
        let decay_factor = 0.995_f64;
        let days = MemoryConfig::default().decay_half_life_days as f64;
        let weight = decay_factor.powf(days);
        assert!(
            (weight - 0.5).abs() < 0.01,
            "Expected ~0.5 at half-life, got {}",
            weight
        );
    }

    #[test]
    fn test_memory_config_deserialize() {
        let json = serde_json::json!({
            "memory": {
                "decayHalfLifeDays": 60,
                "maxAgeDays": 30,
                "consolidationEnabled": true,
                "maintenanceIntervalHours": 12
            }
        });
        let config: ConversationConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.memory.decay_half_life_days, 60);
        assert_eq!(config.memory.max_age_days, 30);
        assert!(config.memory.consolidation_enabled);
        assert_eq!(config.memory.maintenance_interval_hours, 12);
    }

    #[test]
    fn test_conversation_config_defaults() {
        let config = ConversationConfig::default();

        assert!(config.embedding.enabled);
        assert!(config.embedding.exclude_channels.is_empty());
        assert_eq!(config.embedding.exclude_roles, vec!["system", "tool"]);

        assert!(config.search.enabled);
        assert_eq!(config.search.semantic_threshold, 0.5);
        assert_eq!(config.search.max_results, 20);
    }

    #[test]
    fn test_conversation_config_deserialize() {
        let json = serde_json::json!({
            "embedding": {
                "enabled": false,
                "excludeChannels": ["whatsapp", "telegram"],
                "excludeRoles": ["system"]
            },
            "search": {
                "enabled": true,
                "semanticThreshold": 0.7,
                "maxResults": 50
            }
        });

        let config: ConversationConfig = serde_json::from_value(json).unwrap();

        assert!(!config.embedding.enabled);
        assert_eq!(config.embedding.exclude_channels.len(), 2);
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"whatsapp".to_string()));
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"telegram".to_string()));
        assert_eq!(config.embedding.exclude_roles, vec!["system"]);

        assert!(config.search.enabled);
        assert_eq!(config.search.semantic_threshold, 0.7);
        assert_eq!(config.search.max_results, 50);
    }

    #[test]
    fn test_exclude_channels_config() {
        let json = serde_json::json!({
            "embedding": {
                "excludeChannels": ["discord", "slack"]
            }
        });

        let config: ConversationConfig = serde_json::from_value(json).unwrap();

        assert_eq!(config.embedding.exclude_channels.len(), 2);
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"discord".to_string()));
        assert!(config
            .embedding
            .exclude_channels
            .contains(&"slack".to_string()));

        // Verify other fields use defaults
        assert!(config.embedding.enabled);
        assert_eq!(config.embedding.exclude_roles, vec!["system", "tool"]);
    }
}
