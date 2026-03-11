//! Content registry configuration.
//!
//! Controls multi-source documentation and skills loading from
//! builtin, local filesystem, and remote sources.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the content registry system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentConfig {
    /// Content sources (local directories or remote URLs).
    #[serde(default)]
    pub sources: Vec<ContentSourceConfig>,

    /// Trust policy for content sources (comma-separated: "official", "community", "maintainer").
    #[serde(default = "default_trust_policy")]
    pub trust_policy: String,

    /// How often to refresh remote content sources, in seconds.
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_secs: u64,

    /// Directory for content cache and local storage.
    #[serde(default)]
    pub content_dir: PathBuf,
}

impl Default for ContentConfig {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            trust_policy: default_trust_policy(),
            refresh_interval_secs: default_refresh_interval(),
            content_dir: PathBuf::new(),
        }
    }
}

/// A single content source definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSourceConfig {
    /// Unique name for this source.
    pub name: String,

    /// Remote URL to fetch content from (for remote sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Local filesystem path (for local sources).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

fn default_trust_policy() -> String {
    "official,maintainer".into()
}

fn default_refresh_interval() -> u64 {
    86400
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_config_default() {
        let config = ContentConfig::default();
        assert!(config.sources.is_empty());
        assert_eq!(config.trust_policy, "official,maintainer");
        assert_eq!(config.refresh_interval_secs, 86400);
    }

    #[test]
    fn test_content_config_serde_roundtrip() {
        let config = ContentConfig {
            sources: vec![ContentSourceConfig {
                name: "community-docs".into(),
                url: Some("https://example.com/docs".into()),
                path: None,
            }],
            trust_policy: "official,community".into(),
            refresh_interval_secs: 3600,
            content_dir: PathBuf::from("/tmp/content"),
        };

        let json = serde_json::to_string(&config).unwrap();
        let loaded: ContentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].name, "community-docs");
        assert_eq!(loaded.trust_policy, "official,community");
        assert_eq!(loaded.refresh_interval_secs, 3600);
    }

    #[test]
    fn test_content_config_camel_case() {
        let config = ContentConfig {
            refresh_interval_secs: 7200,
            content_dir: PathBuf::from("/tmp"),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("refreshIntervalSecs"));
        assert!(json.contains("contentDir"));
        assert!(json.contains("trustPolicy"));
    }

    #[test]
    fn test_content_source_config_local() {
        let source = ContentSourceConfig {
            name: "local-docs".into(),
            url: None,
            path: Some("/home/user/docs".into()),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert!(!json.contains("url")); // skip_serializing_if None
        assert!(json.contains("path"));
    }
}
