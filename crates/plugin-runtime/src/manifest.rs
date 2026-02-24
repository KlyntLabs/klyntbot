//! Plugin manifest (`klyntbot.plugin.json`) deserialization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    Network,
    Storage,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCronJob {
    pub tool: String,
    pub schedule: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMigrationDef {
    pub version: i64,
    pub description: String,
    pub sql: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigField {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    #[serde(default)]
    pub min_klyntbot_version: Option<String>,
    #[serde(default)]
    pub tools: Vec<PluginToolDef>,
    #[serde(default)]
    pub cron_jobs: Vec<PluginCronJob>,
    #[serde(default)]
    pub migrations: Vec<PluginMigrationDef>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub config_schema: HashMap<String, PluginConfigField>,
}

impl PluginManifest {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn has_permission(&self, perm: &PluginPermission) -> bool {
        self.permissions.contains(perm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let json = r#"{
            "id": "hello",
            "name": "Hello Plugin",
            "version": "0.1.0",
            "description": "A test plugin",
            "author": "Test Author"
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.id, "hello");
        assert_eq!(manifest.name, "Hello Plugin");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.author, "Test Author");
        assert!(manifest.min_klyntbot_version.is_none());
        assert!(manifest.tools.is_empty());
        assert!(manifest.cron_jobs.is_empty());
        assert!(manifest.migrations.is_empty());
        assert!(manifest.permissions.is_empty());
        assert!(manifest.config_schema.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let json = r#"{
            "id": "weather",
            "name": "Weather Plugin",
            "version": "1.2.3",
            "description": "Fetches weather data",
            "author": "Jane Doe",
            "min_klyntbot_version": "0.3.0",
            "tools": [
                {
                    "name": "get_weather",
                    "description": "Get current weather",
                    "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}
                }
            ],
            "cron_jobs": [
                {
                    "tool": "get_weather",
                    "schedule": "0 */6 * * *",
                    "description": "Check weather every 6 hours"
                }
            ],
            "migrations": [
                {
                    "version": 1,
                    "description": "Create weather cache table",
                    "sql": "CREATE TABLE weather_cache (id INTEGER PRIMARY KEY);"
                }
            ],
            "permissions": ["network", "storage"],
            "config_schema": {
                "api_key": {
                    "type": "string",
                    "secret": true,
                    "description": "Weather API key"
                }
            }
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.id, "weather");
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(
            manifest.min_klyntbot_version,
            Some("0.3.0".to_string())
        );
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "get_weather");
        assert_eq!(manifest.cron_jobs.len(), 1);
        assert_eq!(manifest.cron_jobs[0].schedule, "0 */6 * * *");
        assert_eq!(manifest.migrations.len(), 1);
        assert_eq!(manifest.migrations[0].version, 1);
        assert_eq!(manifest.permissions.len(), 2);
        assert!(manifest.has_permission(&PluginPermission::Network));
        assert!(manifest.has_permission(&PluginPermission::Storage));
        assert!(!manifest.has_permission(&PluginPermission::Agent));
        let api_key_field = manifest.config_schema.get("api_key").unwrap();
        assert!(api_key_field.secret);
        assert_eq!(api_key_field.field_type, "string");
    }

    #[test]
    fn test_has_permission() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "version": "0.1.0",
            "description": "Test",
            "author": "Test",
            "permissions": ["network", "agent"]
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert!(manifest.has_permission(&PluginPermission::Network));
        assert!(manifest.has_permission(&PluginPermission::Agent));
        assert!(!manifest.has_permission(&PluginPermission::Storage));
    }

    #[test]
    fn test_missing_required_fields() {
        let json = r#"{"id": "incomplete"}"#;
        let result = PluginManifest::from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_permission_fails() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "version": "0.1.0",
            "description": "Test",
            "author": "Test",
            "permissions": ["unknown_perm"]
        }"#;
        let result = PluginManifest::from_json(json);
        assert!(result.is_err());
    }
}
