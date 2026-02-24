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
#[serde(rename_all = "camelCase")]
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

    const MINIMAL: &str = r#"{"id":"hello-plugin","name":"Hello Plugin","version":"1.0.0","description":"A test plugin","author":"test"}"#;

    const FULL: &str = r#"{
        "id": "notion-connector",
        "name": "Notion Connector",
        "version": "1.2.0",
        "description": "Search and create Notion pages",
        "author": "jayden",
        "minKlyntbotVersion": "0.4.0",
        "tools": [{"name":"notion_search","description":"Search Notion pages","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}],
        "cronJobs": [{"tool":"notion_sync","schedule":"0 * * * *","description":"Hourly Notion sync"}],
        "migrations": [{"version":1,"description":"Create cache table","sql":"CREATE TABLE plugin_notion_connector_cache (id TEXT PRIMARY KEY)"}],
        "permissions": ["network", "storage"],
        "configSchema": {"api_key":{"type":"string","secret":true,"description":"Notion API key"}}
    }"#;

    #[test]
    fn test_minimal_manifest_parses() {
        let manifest = PluginManifest::from_json(MINIMAL).unwrap();
        assert_eq!(manifest.id, "hello-plugin");
        assert_eq!(manifest.name, "Hello Plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.author, "test");
        assert!(manifest.min_klyntbot_version.is_none());
        assert!(manifest.tools.is_empty());
        assert!(manifest.cron_jobs.is_empty());
        assert!(manifest.migrations.is_empty());
        assert!(manifest.permissions.is_empty());
        assert!(manifest.config_schema.is_empty());
    }

    #[test]
    fn test_full_manifest_parses() {
        let manifest = PluginManifest::from_json(FULL).unwrap();
        assert_eq!(manifest.id, "notion-connector");
        assert_eq!(manifest.name, "Notion Connector");
        assert_eq!(manifest.version, "1.2.0");
        assert_eq!(manifest.min_klyntbot_version.as_deref(), Some("0.4.0"));

        // Tools
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.tools[0].name, "notion_search");
        assert_eq!(manifest.tools[0].parameters["required"][0], "query");

        // Cron jobs
        assert_eq!(manifest.cron_jobs.len(), 1);
        assert_eq!(manifest.cron_jobs[0].tool, "notion_sync");
        assert_eq!(manifest.cron_jobs[0].schedule, "0 * * * *");

        // Migrations
        assert_eq!(manifest.migrations.len(), 1);
        assert_eq!(manifest.migrations[0].version, 1);
        assert!(manifest.migrations[0]
            .sql
            .contains("plugin_notion_connector_cache"));

        // Permissions
        assert_eq!(manifest.permissions.len(), 2);
        assert!(manifest.has_permission(&PluginPermission::Network));
        assert!(manifest.has_permission(&PluginPermission::Storage));
        assert!(!manifest.has_permission(&PluginPermission::Agent));

        // Config schema
        let api_key = &manifest.config_schema["api_key"];
        assert_eq!(api_key.field_type, "string");
        assert!(api_key.secret);
        assert_eq!(api_key.description, "Notion API key");
    }

    #[test]
    fn test_has_permission() {
        let manifest = PluginManifest::from_json(FULL).unwrap();
        assert!(manifest.has_permission(&PluginPermission::Network));
        assert!(manifest.has_permission(&PluginPermission::Storage));
        assert!(!manifest.has_permission(&PluginPermission::Agent));
    }

    #[test]
    fn test_missing_required_fields_errors() {
        let json = r#"{"id":"incomplete"}"#;
        assert!(PluginManifest::from_json(json).is_err());
    }

    #[test]
    fn test_unknown_permission_errors() {
        let json = r#"{
            "id": "bad",
            "name": "Bad",
            "version": "0.1.0",
            "description": "Bad perms",
            "author": "test",
            "permissions": ["unknown_perm"]
        }"#;
        let result = PluginManifest::from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("klyntbot.plugin.json");
        std::fs::write(&path, MINIMAL).unwrap();

        let manifest = PluginManifest::from_file(&path).unwrap();
        assert_eq!(manifest.id, "hello-plugin");
    }

    #[test]
    fn test_from_file_missing() {
        let path = std::path::Path::new("/nonexistent/klyntbot.plugin.json");
        assert!(PluginManifest::from_file(path).is_err());
    }

    #[test]
    fn test_roundtrip_serialize() {
        let manifest = PluginManifest::from_json(FULL).unwrap();
        let serialized = serde_json::to_string(&manifest).unwrap();
        let restored = PluginManifest::from_json(&serialized).unwrap();
        assert_eq!(manifest.id, restored.id);
        assert_eq!(manifest.tools.len(), restored.tools.len());
        assert_eq!(manifest.permissions, restored.permissions);
    }

    #[test]
    fn test_cron_job_default_description() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "version": "0.1.0",
            "description": "Test",
            "author": "test",
            "cronJobs": [{"tool": "ping", "schedule": "* * * * *"}]
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.cron_jobs[0].description, "");
    }

    #[test]
    fn test_config_field_defaults() {
        let json = r#"{
            "id": "test",
            "name": "Test",
            "version": "0.1.0",
            "description": "Test",
            "author": "test",
            "configSchema": {"url": {"type": "string"}}
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        let field = &manifest.config_schema["url"];
        assert!(!field.secret);
        assert_eq!(field.description, "");
    }

    #[test]
    fn test_serialized_keys_are_camel_case() {
        let manifest = PluginManifest::from_json(FULL).unwrap();
        let serialized = serde_json::to_string(&manifest).unwrap();
        assert!(serialized.contains("minKlyntbotVersion"));
        assert!(serialized.contains("cronJobs"));
        assert!(serialized.contains("configSchema"));
        assert!(!serialized.contains("min_klyntbot_version"));
        assert!(!serialized.contains("cron_jobs"));
        assert!(!serialized.contains("config_schema"));
    }
}
