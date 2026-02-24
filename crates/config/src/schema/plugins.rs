//! Plugin system configuration.

use serde::{Deserialize, Serialize};

/// Plugin system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsConfig {
    #[serde(default = "default_plugins_enabled")]
    pub enabled: bool,

    #[serde(default = "default_registry_url")]
    pub registry_url: String,

    #[serde(default = "default_sandbox_memory_mb")]
    pub sandbox_memory_mb: u32,

    #[serde(default)]
    pub allow_network_by_default: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: default_plugins_enabled(),
            registry_url: default_registry_url(),
            sandbox_memory_mb: default_sandbox_memory_mb(),
            allow_network_by_default: false,
        }
    }
}

fn default_plugins_enabled() -> bool {
    true
}
fn default_registry_url() -> String {
    "https://plugins.klyntbot.io/index.json".to_string()
}
fn default_sandbox_memory_mb() -> u32 {
    64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugins_config_defaults() {
        let cfg = PluginsConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.registry_url, "https://plugins.klyntbot.io/index.json");
        assert_eq!(cfg.sandbox_memory_mb, 64);
        assert!(!cfg.allow_network_by_default);
    }

    #[test]
    fn test_plugins_config_serde_roundtrip() {
        let json = r#"{"enabled":false,"registryUrl":"https://example.com","sandboxMemoryMb":128,"allowNetworkByDefault":true}"#;
        let cfg: PluginsConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.registry_url, "https://example.com");
        assert_eq!(cfg.sandbox_memory_mb, 128);
        assert!(cfg.allow_network_by_default);
    }

    #[test]
    fn test_plugins_config_camel_case_keys() {
        let cfg = PluginsConfig::default();
        let json = serde_json::to_value(&cfg).unwrap();
        assert!(json.get("registryUrl").is_some());
        assert!(json.get("sandboxMemoryMb").is_some());
        assert!(json.get("allowNetworkByDefault").is_some());
    }
}
