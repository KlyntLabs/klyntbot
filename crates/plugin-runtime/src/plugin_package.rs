//! PluginPackage: a loaded WASM plugin as a FeaturePackage.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::manifest::PluginManifest;
use crate::wasm_plugin::WasmPlugin;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

/// A loaded WASM plugin, presented as a FeaturePackage for the agent.
pub struct PluginPackage {
    manifest: Arc<PluginManifest>,
    plugin: Option<Arc<Mutex<extism::Plugin>>>,
}

impl PluginPackage {
    /// Create a PluginPackage from a parsed manifest (without a loaded plugin).
    pub fn from_manifest(manifest: PluginManifest) -> Self {
        Self {
            manifest: Arc::new(manifest),
            plugin: None,
        }
    }

    /// Attach a loaded Extism plugin instance.
    pub fn attach_plugin(&mut self, plugin: extism::Plugin) {
        self.plugin = Some(Arc::new(Mutex::new(plugin)));
    }

    /// Access the manifest.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

#[async_trait]
impl FeaturePackage for PluginPackage {
    #[allow(clippy::misnamed_getters)]
    fn name(&self) -> &str {
        // FeaturePackage::name() returns the unique plugin id, not the display name
        &self.manifest.id
    }

    fn tools(&self) -> Vec<DynTool> {
        let Some(ref plugin) = self.plugin else {
            return Vec::new();
        };

        self.manifest
            .tools
            .iter()
            .map(|tool_def| {
                Arc::new(WasmPlugin::new(
                    Arc::clone(plugin),
                    tool_def.clone(),
                    Arc::clone(&self.manifest),
                )) as DynTool
            })
            .collect()
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        self.manifest
            .migrations
            .iter()
            .map(|m| FeatureMigration {
                feature_name: self.manifest.id.clone(),
                version: m.version,
                description: m.description.clone(),
                sql: m.sql.clone(),
            })
            .collect()
    }

    fn config_key(&self) -> &str {
        &self.manifest.id
    }

    fn default_config(&self) -> Value {
        let mut config = serde_json::Map::new();
        for (key, field) in &self.manifest.config_schema {
            let default_val = match field.field_type.as_str() {
                "string" => Value::String(String::new()),
                "integer" | "number" => Value::Number(serde_json::Number::from(0)),
                "boolean" => Value::Bool(false),
                _ => Value::Null,
            };
            config.insert(key.clone(), default_val);
        }
        Value::Object(config)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        if self.plugin.is_some() {
            Ok(HealthStatus::Healthy)
        } else {
            Ok(HealthStatus::Degraded(
                "WASM plugin not loaded".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PluginManifest;

    fn minimal_manifest() -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "id": "hello",
            "name": "Hello Plugin",
            "version": "0.1.0",
            "description": "A hello world plugin",
            "author": "Test"
        }))
        .unwrap()
    }

    fn manifest_with_tools_and_migrations() -> PluginManifest {
        serde_json::from_value(serde_json::json!({
            "id": "weather",
            "name": "Weather Plugin",
            "version": "1.0.0",
            "description": "Weather data",
            "author": "Test",
            "tools": [
                {
                    "name": "get_weather",
                    "description": "Get weather",
                    "parameters": {"type": "object"}
                }
            ],
            "migrations": [
                {
                    "version": 1,
                    "description": "Create cache",
                    "sql": "CREATE TABLE cache (id INTEGER PRIMARY KEY);"
                }
            ],
            "configSchema": {
                "api_key": {
                    "type": "string",
                    "secret": true,
                    "description": "API key"
                },
                "refresh_interval": {
                    "type": "integer",
                    "description": "Refresh interval in seconds"
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn test_package_name_matches_manifest_id() {
        let pkg = PluginPackage::from_manifest(minimal_manifest());
        assert_eq!(pkg.name(), "hello");
    }

    #[test]
    fn test_config_key_matches_manifest_id() {
        let pkg = PluginPackage::from_manifest(minimal_manifest());
        assert_eq!(pkg.config_key(), "hello");
    }

    #[test]
    fn test_tools_empty_without_plugin() {
        let pkg = PluginPackage::from_manifest(manifest_with_tools_and_migrations());
        assert!(pkg.tools().is_empty());
    }

    #[test]
    fn test_migrations_from_manifest() {
        let pkg = PluginPackage::from_manifest(manifest_with_tools_and_migrations());
        let migrations = pkg.migrations();
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].feature_name, "weather");
        assert_eq!(migrations[0].version, 1);
        assert_eq!(migrations[0].description, "Create cache");
    }

    #[test]
    fn test_default_config_from_schema() {
        let pkg = PluginPackage::from_manifest(manifest_with_tools_and_migrations());
        let config = pkg.default_config();
        let obj = config.as_object().unwrap();
        assert_eq!(obj.get("api_key").unwrap(), &Value::String(String::new()));
        assert_eq!(
            obj.get("refresh_interval").unwrap(),
            &Value::Number(serde_json::Number::from(0))
        );
    }

    #[test]
    fn test_default_config_empty_for_no_schema() {
        let pkg = PluginPackage::from_manifest(minimal_manifest());
        let config = pkg.default_config();
        assert_eq!(config, Value::Object(serde_json::Map::new()));
    }

    #[tokio::test]
    async fn test_health_check_degraded_without_plugin() {
        let pkg = PluginPackage::from_manifest(minimal_manifest());
        let status = pkg.health_check().await.unwrap();
        assert!(matches!(status, HealthStatus::Degraded(_)));
    }
}
