//! WasmPlugin: a single WASM-backed tool implementing the Tool trait.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::debug;

use crate::manifest::{PluginManifest, PluginPermission, PluginToolDef};
use common::Result;
use tools_core::{PermissionLevel, RoutingContext, Tool};

/// A single tool backed by a WASM plugin via Extism.
pub struct WasmPlugin {
    plugin: Arc<Mutex<extism::Plugin>>,
    tool_def: PluginToolDef,
    manifest: Arc<PluginManifest>,
}

impl WasmPlugin {
    /// Create a new WasmPlugin wrapping an Extism plugin instance.
    pub fn new(
        plugin: Arc<Mutex<extism::Plugin>>,
        tool_def: PluginToolDef,
        manifest: Arc<PluginManifest>,
    ) -> Self {
        Self {
            plugin,
            tool_def,
            manifest,
        }
    }

    /// Determine the permission level based on the plugin's declared permissions.
    /// Plugins requesting network or agent access get Elevated; otherwise Standard.
    fn compute_permission_level(manifest: &PluginManifest) -> PermissionLevel {
        if manifest.has_permission(&PluginPermission::Network)
            || manifest.has_permission(&PluginPermission::Agent)
        {
            PermissionLevel::Elevated
        } else {
            PermissionLevel::Standard
        }
    }
}

impl std::fmt::Debug for WasmPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPlugin")
            .field("tool", &self.tool_def.name)
            .field("plugin_id", &self.manifest.id)
            .finish()
    }
}

#[async_trait]
impl Tool for WasmPlugin {
    fn name(&self) -> &str {
        &self.tool_def.name
    }

    fn description(&self) -> &str {
        &self.tool_def.description
    }

    fn parameters(&self) -> Value {
        self.tool_def.parameters.clone()
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let input = serde_json::to_string(&args)?;
        let func_name = &self.tool_def.name;

        debug!(
            plugin_id = %self.manifest.id,
            tool = %func_name,
            "Calling WASM plugin function"
        );

        let mut plugin = self.plugin.lock().await;
        let output = plugin.call::<&str, &str>(func_name, &input).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "WASM plugin call failed: {e}"
            )))
        })?;

        Ok(output.to_string())
    }

    fn permission_level(&self) -> PermissionLevel {
        Self::compute_permission_level(&self.manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PluginManifest;

    fn make_manifest(permissions: Vec<PluginPermission>) -> PluginManifest {
        let mut json = serde_json::json!({
            "id": "test-plugin",
            "name": "Test Plugin",
            "version": "0.1.0",
            "description": "A test plugin",
            "author": "Test"
        });
        let perms: Vec<String> = permissions
            .iter()
            .map(|p| match p {
                PluginPermission::Network => "network".to_string(),
                PluginPermission::Storage => "storage".to_string(),
                PluginPermission::Agent => "agent".to_string(),
            })
            .collect();
        json["permissions"] =
            serde_json::Value::Array(perms.into_iter().map(serde_json::Value::String).collect());
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn test_permission_level_standard_no_perms() {
        let manifest = make_manifest(vec![]);
        assert_eq!(
            WasmPlugin::compute_permission_level(&manifest),
            PermissionLevel::Standard
        );
    }

    #[test]
    fn test_permission_level_standard_storage_only() {
        let manifest = make_manifest(vec![PluginPermission::Storage]);
        assert_eq!(
            WasmPlugin::compute_permission_level(&manifest),
            PermissionLevel::Standard
        );
    }

    #[test]
    fn test_permission_level_elevated_network() {
        let manifest = make_manifest(vec![PluginPermission::Network]);
        assert_eq!(
            WasmPlugin::compute_permission_level(&manifest),
            PermissionLevel::Elevated
        );
    }

    #[test]
    fn test_permission_level_elevated_agent() {
        let manifest = make_manifest(vec![PluginPermission::Agent]);
        assert_eq!(
            WasmPlugin::compute_permission_level(&manifest),
            PermissionLevel::Elevated
        );
    }

    #[test]
    fn test_permission_level_elevated_both() {
        let manifest = make_manifest(vec![PluginPermission::Network, PluginPermission::Agent]);
        assert_eq!(
            WasmPlugin::compute_permission_level(&manifest),
            PermissionLevel::Elevated
        );
    }
}
