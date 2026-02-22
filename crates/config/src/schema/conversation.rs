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
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
        }
    }
}

fn default_history_limit() -> usize {
    50
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
