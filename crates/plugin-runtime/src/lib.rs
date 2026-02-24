//! WASM plugin sandbox for klyntbot.
//!
//! Loads .wasm plugins from `~/.klyntbot/plugins/` at startup,
//! wraps each plugin's tools as `Tool` impls, and registers them
//! into the agent's `ToolRegistry` via `PluginPackage`.

pub mod host;
pub mod manifest;
pub mod manager;
pub mod plugin_package;
pub mod wasm_plugin;

pub use manager::PluginManager;
pub use manifest::{
    PluginConfigField, PluginCronJob, PluginManifest, PluginMigrationDef, PluginPermission,
    PluginToolDef,
};
pub use plugin_package::PluginPackage;
pub use wasm_plugin::WasmPlugin;
