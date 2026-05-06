//! WasmPlugin: a single WASM-backed tool implementing the Tool trait.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::debug;

use crate::manifest::{PluginManifest, PluginToolDef};
use common::Result;
use tools_core::{RoutingContext, Tool};

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
}
