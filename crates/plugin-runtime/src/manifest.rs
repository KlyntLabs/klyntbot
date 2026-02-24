//! Plugin manifest (`klyntbot.plugin.json`) deserialization.
//! TODO: Full implementation in Task 3.

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
