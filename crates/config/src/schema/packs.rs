//! Feature pack configuration.

use serde::{Deserialize, Serialize};

/// Which feature packs are enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PacksConfig {
    /// List of enabled pack IDs.
    #[serde(default = "default_enabled_packs")]
    pub enabled: Vec<String>,

    /// Skill names derived from enabled packs (computed by wizard, saved to config).
    #[serde(default)]
    pub enabled_skills: Vec<String>,
}

impl Default for PacksConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled_packs(),
            enabled_skills: Vec::new(),
        }
    }
}

fn default_enabled_packs() -> Vec<String> {
    vec![
        "task-management".to_string(),
        "productivity".to_string(),
        "ai-intelligence".to_string(),
        "developer-tools".to_string(),
    ]
}

/// Pack tier determines default selection state in the wizard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackTier {
    Core,
    Recommended,
    Optional,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packs_config_default() {
        let config = PacksConfig::default();
        assert_eq!(
            config.enabled,
            vec![
                "task-management",
                "productivity",
                "ai-intelligence",
                "developer-tools",
            ]
        );
        assert!(config.enabled_skills.is_empty());
    }

    #[test]
    fn test_packs_config_serialization_camel_case() {
        let config = PacksConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"enabled\""));
        assert!(json.contains("\"enabledSkills\""));
    }

    #[test]
    fn test_packs_config_round_trip() {
        let config = PacksConfig {
            enabled: vec!["task-management".to_string(), "finance".to_string()],
            enabled_skills: vec!["todo".to_string(), "finance".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: PacksConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.enabled, vec!["task-management", "finance"]);
        assert_eq!(loaded.enabled_skills, vec!["todo", "finance"]);
    }

    #[test]
    fn test_packs_config_deserialize_without_enabled_skills() {
        let json = r#"{"enabled":["task-management"]}"#;
        let loaded: PacksConfig = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.enabled, vec!["task-management"]);
        assert!(loaded.enabled_skills.is_empty());
    }

    #[test]
    fn test_pack_tier_ordering() {
        assert!(PackTier::Core < PackTier::Recommended);
        assert!(PackTier::Recommended < PackTier::Optional);
    }

    #[test]
    fn test_pack_tier_serialization() {
        let json = serde_json::to_string(&PackTier::Core).unwrap();
        assert_eq!(json, "\"core\"");
        let json = serde_json::to_string(&PackTier::Recommended).unwrap();
        assert_eq!(json, "\"recommended\"");
        let json = serde_json::to_string(&PackTier::Optional).unwrap();
        assert_eq!(json, "\"optional\"");
    }
}
