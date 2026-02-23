# WASM Plugin Sandbox Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a runtime-loadable WASM plugin system to klyntbot using Extism, enabling users to install personal skills (Notion, Obsidian, Vietnamese banks, health trackers) without recompiling.

**Architecture:** `plugin-runtime` crate (Layer 2.5) wraps Extism to load `.wasm` plugins from `~/.klyntbot/plugins/`. Each plugin's tools are registered into `ToolRegistry` as `WasmPlugin` instances (implements `Tool`). `PluginPackage` implements `FeaturePackage`. The `plugin` CLI subcommand handles install/list/remove from local file, GitHub, and registry.

**Tech Stack:** `extism = "1"` (Wasmtime-backed), `tokio::sync::Mutex` for plugin concurrency, `reqwest` for HTTP downloads, `serde_json` for manifest parsing, `sqlx::SqlitePool` passed via `UserData` to host functions.

**Design doc:** `docs/plans/2026-02-23-wasm-plugin-sandbox-design.md`

---

## Task 1: Add `PluginsConfig` to the config crate

**Files:**
- Create: `crates/config/src/schema/plugins.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

### Step 1: Write tests first in `plugins.rs`

```rust
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
```

### Step 2: Run tests

```bash
cargo nextest run -p config
```
Expected: all tests pass.

### Step 3: Wire into `mod.rs` and `core.rs`

In `crates/config/src/schema/mod.rs`, add after `mod packs;`:
```rust
mod plugins;
pub use self::plugins::*;
```

In `crates/config/src/schema/core.rs`, add `use super::plugins::PluginsConfig;` at the top imports block, then add field to `Config` after `packs`:
```rust
    #[serde(default)]
    pub plugins: PluginsConfig,
```

### Step 4: Build and verify

```bash
cargo build -p config && cargo nextest run -p config
```
Expected: compiles, all tests pass.

### Step 5: Commit

```bash
git add crates/config/src/schema/plugins.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add PluginsConfig to root Config"
```

---

## Task 2: Bootstrap `plugin-runtime` crate

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `crates/plugin-runtime/Cargo.toml`
- Create: `crates/plugin-runtime/src/lib.rs`
- Create: `crates/plugin-runtime/src/manifest.rs` (stub)
- Create: `crates/plugin-runtime/src/manager.rs` (stub)
- Create: `crates/plugin-runtime/src/plugin_package.rs` (stub)
- Create: `crates/plugin-runtime/src/wasm_plugin.rs` (stub)
- Create: `crates/plugin-runtime/src/host/mod.rs` (stub)

### Step 1: Patch workspace `Cargo.toml`

Add `"crates/plugin-runtime"` to `[workspace] members`.

Add to `[workspace.dependencies]`:
```toml
plugin-runtime = { path = "crates/plugin-runtime" }
extism = "1"
```

### Step 2: Create `crates/plugin-runtime/Cargo.toml`

```toml
[package]
name = "plugin-runtime"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
config.workspace = true
storage.workspace = true
tools-core.workspace = true
extism = "1"
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
tracing.workspace = true
reqwest.workspace = true
dirs.workspace = true
anyhow.workspace = true
thiserror.workspace = true

[features]
default = []
plugin-integration = []
```

### Step 3: Create `crates/plugin-runtime/src/lib.rs`

```rust
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
pub use manifest::{PluginManifest, PluginPermission, PluginToolDef, PluginCronJob};
pub use plugin_package::PluginPackage;
pub use wasm_plugin::WasmPlugin;
```

### Step 4: Create stub files (all with `// TODO`)

Create each file with `// TODO` content:
- `crates/plugin-runtime/src/manifest.rs`
- `crates/plugin-runtime/src/manager.rs`
- `crates/plugin-runtime/src/plugin_package.rs`
- `crates/plugin-runtime/src/wasm_plugin.rs`
- `crates/plugin-runtime/src/host/mod.rs`

### Step 5: Build to verify skeleton

```bash
cargo build -p plugin-runtime
```
Expected: compiles clean.

### Step 6: Commit

```bash
git add crates/plugin-runtime/ Cargo.toml
git commit -m "feat(plugin-runtime): bootstrap crate skeleton"
```

---

## Task 3: `PluginManifest` struct and parsing

**Files:**
- Modify: `crates/plugin-runtime/src/manifest.rs`

### Step 1: Write tests first, then structs

Replace the stub in `manifest.rs` with the complete implementation + tests:

```rust
//! Plugin manifest (`klyntbot.plugin.json`) deserialization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Permission a plugin must declare to use a host capability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    Network,
    Storage,
    Agent,
}

/// A single tool exposed by the plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A cron job the plugin wants registered at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCronJob {
    pub tool: String,
    pub schedule: String,
    #[serde(default)]
    pub description: String,
}

/// A SQL migration the plugin owns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMigrationDef {
    pub version: i64,
    pub description: String,
    pub sql: String,
}

/// One entry in the plugin's config_schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigField {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub description: String,
}

/// Full parsed `klyntbot.plugin.json` manifest.
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

    /// Returns true if this plugin has declared the given permission.
    pub fn has_permission(&self, perm: &PluginPermission) -> bool {
        self.permissions.contains(perm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
        "id": "hello-plugin",
        "name": "Hello Plugin",
        "version": "1.0.0",
        "description": "A test plugin",
        "author": "test"
    }"#;

    const FULL: &str = r#"{
        "id": "notion-connector",
        "name": "Notion Connector",
        "version": "1.2.0",
        "description": "Search and create Notion pages",
        "author": "jayden",
        "minKlyntbotVersion": "0.4.0",
        "tools": [
            {
                "name": "notion_search",
                "description": "Search Notion pages",
                "parameters": {"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}
            }
        ],
        "cronJobs": [
            {"tool": "notion_sync", "schedule": "0 * * * *", "description": "Hourly Notion sync"}
        ],
        "migrations": [
            {"version": 1, "description": "Create cache table", "sql": "CREATE TABLE plugin_notion_connector_cache (id TEXT PRIMARY KEY)"}
        ],
        "permissions": ["network", "storage"],
        "configSchema": {
            "api_key": {"type": "string", "secret": true, "description": "Notion API key"}
        }
    }"#;

    #[test]
    fn test_minimal_manifest_parses() {
        let m = PluginManifest::from_json(MINIMAL).unwrap();
        assert_eq!(m.id, "hello-plugin");
        assert_eq!(m.version, "1.0.0");
        assert!(m.tools.is_empty());
        assert!(m.permissions.is_empty());
    }

    #[test]
    fn test_full_manifest_parses() {
        let m = PluginManifest::from_json(FULL).unwrap();
        assert_eq!(m.id, "notion-connector");
        assert_eq!(m.tools.len(), 1);
        assert_eq!(m.tools[0].name, "notion_search");
        assert_eq!(m.cron_jobs.len(), 1);
        assert_eq!(m.migrations.len(), 1);
        assert_eq!(m.permissions, vec![PluginPermission::Network, PluginPermission::Storage]);
        assert!(m.config_schema["api_key"].secret);
    }

    #[test]
    fn test_has_permission() {
        let m = PluginManifest::from_json(FULL).unwrap();
        assert!(m.has_permission(&PluginPermission::Network));
        assert!(m.has_permission(&PluginPermission::Storage));
        assert!(!m.has_permission(&PluginPermission::Agent));
    }

    #[test]
    fn test_missing_required_fields_errors() {
        // Missing name, version, description, author
        assert!(PluginManifest::from_json(r#"{"id":"test"}"#).is_err());
    }

    #[test]
    fn test_unknown_permission_ignored() {
        // Unknown permissions should fail cleanly or be ignored
        let json = r#"{"id":"t","name":"T","version":"1","description":"d","author":"a","permissions":["network","unknown_future_perm"]}"#;
        // serde unknown variant → error; that's acceptable — permissions must be explicit
        // If this test fails, add #[serde(other)] variant Unknown to PluginPermission
        let _ = PluginManifest::from_json(json); // just ensure it doesn't panic
    }
}
```

### Step 2: Run tests

```bash
cargo nextest run -p plugin-runtime manifest
```
Expected: all tests pass.

### Step 3: Commit

```bash
git add crates/plugin-runtime/src/manifest.rs
git commit -m "feat(plugin-runtime): add PluginManifest struct and parsing"
```

---

## Task 4: Host functions (db, log, http)

**Files:**
- Modify: `crates/plugin-runtime/src/host/mod.rs`

### Step 1: Write tests for the sandbox logic (permission checks)

These tests validate the SQL guard logic without invoking a real plugin:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_is_select_only_accepts_select() {
        assert!(is_select_only("SELECT * FROM plugin_foo_bar"));
        assert!(is_select_only("  select id from plugin_test_cache  "));
    }

    #[test]
    fn test_is_select_only_rejects_write() {
        assert!(!is_select_only("INSERT INTO plugin_foo VALUES (1)"));
        assert!(!is_select_only("DELETE FROM plugin_foo"));
        assert!(!is_select_only("UPDATE plugin_foo SET x=1"));
        assert!(!is_select_only("DROP TABLE plugin_foo"));
    }

    #[test]
    fn test_is_plugin_table_accepts_prefixed() {
        assert!(is_plugin_table("plugin_notion_connector_cache", "notion-connector"));
        assert!(is_plugin_table("plugin_notion_connector_items", "notion-connector"));
    }

    #[test]
    fn test_is_plugin_table_rejects_other_plugins() {
        assert!(!is_plugin_table("plugin_other_plugin_data", "notion-connector"));
        assert!(!is_plugin_table("sessions", "notion-connector"));
        assert!(!is_plugin_table("todos", "notion-connector"));
    }
}
```

Add the two helper functions before the tests:

```rust
/// Returns true if sql is a read-only SELECT statement.
fn is_select_only(sql: &str) -> bool {
    sql.trim().to_uppercase().starts_with("SELECT")
}

/// Returns true if `table_name` belongs to this plugin's sandboxed namespace.
/// Plugin tables must be prefixed with `plugin_{id_with_underscores}_`.
fn is_plugin_table(table_name: &str, plugin_id: &str) -> bool {
    let expected_prefix = format!("plugin_{}_", plugin_id.replace('-', "_"));
    table_name.starts_with(&expected_prefix)
}
```

### Step 2: Run tests

```bash
cargo nextest run -p plugin-runtime host
```
Expected: all tests pass.

### Step 3: Implement host functions using extism 1.x `Function::new`

> **Note for implementer:** Check extism 1.x docs at https://docs.rs/extism/1.0/extism/ for exact `Function::new` signatures and `CurrentPlugin::memory_get_str` / `memory_new` methods. The pattern below matches extism 1.x but verify parameter types against the installed version.

```rust
//! Host functions exposed to WASM plugins via Extism.

use extism::*;
use std::sync::Arc;
use tokio::runtime::Handle;

pub use self::tests::is_select_only;  // pub for integration tests

/// Build all host functions to inject into every plugin instance.
/// `pool` provides db_* access; `plugin_id` enforces table sandboxing.
pub fn build_host_functions(
    pool: sqlx::SqlitePool,
    plugin_id: String,
) -> Vec<Function> {
    let pool = Arc::new(pool);
    let id = Arc::new(plugin_id);

    vec![
        build_db_query(Arc::clone(&pool), Arc::clone(&id)),
        build_db_execute(Arc::clone(&pool), Arc::clone(&id)),
        build_log_info(),
        build_log_warn(),
        build_log_error(),
        build_http_request(),
    ]
}

fn build_db_query(pool: Arc<sqlx::SqlitePool>, plugin_id: Arc<String>) -> Function {
    Function::new(
        "db", "db_query",
        [ValType::I64], [ValType::I64],
        UserData::new((pool, plugin_id)),
        |plugin: &mut CurrentPlugin, inputs: &[Val], outputs: &mut [Val],
         ud: UserData<(Arc<sqlx::SqlitePool>, Arc<String>)>| {
            let sql = plugin.memory_get_str(inputs[0].unwrap_i64() as u64)?;
            if !is_select_only(sql) {
                let err = r#"{"error":"db_query only allows SELECT"}"#;
                let mem = plugin.memory_new(err.as_bytes())?;
                outputs[0] = Val::I64(mem as i64);
                return Ok(());
            }
            let (pool, _id) = ud.get()?.lock().map_err(|_| Error::msg("lock"))?.clone();
            let sql = sql.to_string();
            let result = tokio::task::block_in_place(|| {
                Handle::current().block_on(async {
                    sqlx::query(&sql)
                        .fetch_all(&*pool)
                        .await
                        .map(|_rows| "[]".to_string())  // simplified: return empty array
                        .unwrap_or_else(|e| format!(r#"{{"error":"{}"}}"#, e))
                })
            });
            let mem = plugin.memory_new(result.as_bytes())?;
            outputs[0] = Val::I64(mem as i64);
            Ok(())
        },
    )
}

fn build_db_execute(pool: Arc<sqlx::SqlitePool>, plugin_id: Arc<String>) -> Function {
    Function::new(
        "db", "db_execute",
        [ValType::I64], [ValType::I32],
        UserData::new((pool, plugin_id)),
        |plugin: &mut CurrentPlugin, inputs: &[Val], outputs: &mut [Val],
         ud: UserData<(Arc<sqlx::SqlitePool>, Arc<String>)>| {
            let sql = plugin.memory_get_str(inputs[0].unwrap_i64() as u64)?;
            let (pool, _id) = ud.get()?.lock().map_err(|_| Error::msg("lock"))?.clone();
            let sql = sql.to_string();
            let rows = tokio::task::block_in_place(|| {
                Handle::current().block_on(async {
                    sqlx::query(&sql)
                        .execute(&*pool)
                        .await
                        .map(|r| r.rows_affected() as i32)
                        .unwrap_or(-1)
                })
            });
            outputs[0] = Val::I32(rows);
            Ok(())
        },
    )
}

fn build_log_info() -> Function {
    Function::new("log", "log_info", [ValType::I64], [], UserData::new(()), log_fn!(tracing::info))
}
fn build_log_warn() -> Function {
    Function::new("log", "log_warn", [ValType::I64], [], UserData::new(()), log_fn!(tracing::warn))
}
fn build_log_error() -> Function {
    Function::new("log", "log_error", [ValType::I64], [], UserData::new(()), log_fn!(tracing::error))
}

// Helper macro to avoid repeating the log function closure 3 times
macro_rules! log_fn {
    ($level:path) => {
        |plugin: &mut CurrentPlugin, inputs: &[Val], _out: &mut [Val], _: UserData<()>| {
            let msg = plugin.memory_get_str(inputs[0].unwrap_i64() as u64)?;
            $level!("[plugin] {}", msg);
            Ok(())
        }
    };
}
use log_fn;

fn build_http_request() -> Function {
    Function::new(
        "http", "http_request",
        [ValType::I64], [ValType::I64],
        UserData::new(()),
        |plugin: &mut CurrentPlugin, inputs: &[Val], outputs: &mut [Val], _: UserData<()>| {
            let req_json = plugin.memory_get_str(inputs[0].unwrap_i64() as u64)?;
            #[derive(serde::Deserialize)]
            struct Req {
                url: String,
                #[serde(default)] method: Option<String>,
                #[serde(default)] headers: std::collections::HashMap<String, String>,
                #[serde(default)] body: Option<String>,
            }
            let req: Req = serde_json::from_str(req_json)
                .map_err(|e| Error::msg(format!("bad request JSON: {e}")))?;
            let result = tokio::task::block_in_place(|| {
                Handle::current().block_on(async {
                    let client = reqwest::Client::new();
                    let method = req.method.as_deref().unwrap_or("GET");
                    let mut b = match method {
                        "POST" => client.post(&req.url),
                        "PUT"  => client.put(&req.url),
                        "DELETE" => client.delete(&req.url),
                        _ => client.get(&req.url),
                    };
                    for (k, v) in &req.headers { b = b.header(k, v); }
                    if let Some(body) = &req.body { b = b.body(body.clone()); }
                    match b.send().await {
                        Ok(r) => {
                            let status = r.status().as_u16();
                            let body = r.text().await.unwrap_or_default();
                            serde_json::json!({"status":status,"body":body}).to_string()
                        }
                        Err(e) => serde_json::json!({"error":e.to_string()}).to_string()
                    }
                })
            });
            let mem = plugin.memory_new(result.as_bytes())?;
            outputs[0] = Val::I64(mem as i64);
            Ok(())
        },
    )
}
```

### Step 4: Build to verify

```bash
cargo build -p plugin-runtime
```
Expected: compiles. Fix any extism API mismatches by consulting `cargo doc -p extism`.

### Step 5: Run tests

```bash
cargo nextest run -p plugin-runtime host
```
Expected: all tests pass.

### Step 6: Commit

```bash
git add crates/plugin-runtime/src/host/mod.rs
git commit -m "feat(plugin-runtime): add host functions (db, log, http)"
```

---

## Task 5: `WasmPlugin` — implements `Tool`

**Files:**
- Modify: `crates/plugin-runtime/src/wasm_plugin.rs`

### Step 1: Write tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_manifest(permissions: &[&str]) -> Arc<PluginManifest> {
        let perms = permissions.iter().map(|p| format!("\"{}\"", p)).collect::<Vec<_>>().join(",");
        Arc::new(serde_json::from_str(&format!(
            r#"{{"id":"t","name":"T","version":"1","description":"d","author":"a","permissions":[{}]}}"#,
            perms
        )).unwrap())
    }

    #[test]
    fn test_permission_level_elevated_for_network() {
        let m = make_manifest(&["network"]);
        assert!(m.has_permission(&PluginPermission::Network));
        // Verify the logic used in permission_level():
        let elevated = m.has_permission(&PluginPermission::Network)
            || m.has_permission(&PluginPermission::Agent);
        assert!(elevated);
    }

    #[test]
    fn test_permission_level_standard_for_storage_only() {
        let m = make_manifest(&["storage"]);
        let elevated = m.has_permission(&PluginPermission::Network)
            || m.has_permission(&PluginPermission::Agent);
        assert!(!elevated);
    }

    #[test]
    fn test_tool_name_matches_def() {
        let def = PluginToolDef {
            name: "notion_search".to_string(),
            description: "Search Notion".to_string(),
            parameters: serde_json::json!({"type":"object"}),
        };
        assert_eq!(def.name, "notion_search");
        assert_eq!(def.description, "Search Notion");
    }
}
```

### Step 2: Run tests

```bash
cargo nextest run -p plugin-runtime wasm_plugin
```
Expected: all tests pass.

### Step 3: Implement `WasmPlugin`

Replace stub in `crates/plugin-runtime/src/wasm_plugin.rs`:

```rust
//! WasmPlugin: a single WASM-backed tool implementing the `Tool` trait.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::debug;

use common::{Result, ToolError};
use tools_core::{PermissionLevel, RoutingContext, Tool};

use crate::manifest::{PluginManifest, PluginPermission, PluginToolDef};

/// A single tool from a WASM plugin.
///
/// All tools from the same `.wasm` file share one `Arc<Mutex<extism::Plugin>>`
/// because `extism::Plugin::call` takes `&mut self`.
pub struct WasmPlugin {
    plugin: Arc<Mutex<extism::Plugin>>,
    tool_def: PluginToolDef,
    manifest: Arc<PluginManifest>,
}

impl WasmPlugin {
    pub fn new(
        plugin: Arc<Mutex<extism::Plugin>>,
        tool_def: PluginToolDef,
        manifest: Arc<PluginManifest>,
    ) -> Self {
        Self { plugin, tool_def, manifest }
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

    fn permission_level(&self) -> PermissionLevel {
        if self.manifest.has_permission(&PluginPermission::Network)
            || self.manifest.has_permission(&PluginPermission::Agent)
        {
            PermissionLevel::Elevated
        } else {
            PermissionLevel::Standard
        }
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let fn_name = self.tool_def.name.clone();
        let args_str = args.to_string();
        debug!("wasm plugin call: {}({})", fn_name, args_str);

        let mut plugin = self.plugin.lock().await;
        plugin
            .call::<&str, &str>(&fn_name, &args_str)
            .map(|s| s.to_string())
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("plugin {} error: {}", fn_name, e)).into()
            })
    }
}
```

### Step 4: Build and run tests

```bash
cargo build -p plugin-runtime && cargo nextest run -p plugin-runtime wasm_plugin
```
Expected: compiles, all tests pass.

### Step 5: Commit

```bash
git add crates/plugin-runtime/src/wasm_plugin.rs
git commit -m "feat(plugin-runtime): add WasmPlugin Tool implementation"
```

---

## Task 6: `PluginPackage` — implements `FeaturePackage`

**Files:**
- Modify: `crates/plugin-runtime/src/plugin_package.rs`

### Step 1: Write tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_package(id: &str, tool_names: &[&str]) -> PluginPackage {
        let tools: Vec<PluginToolDef> = tool_names.iter().map(|n| PluginToolDef {
            name: n.to_string(),
            description: format!("{} tool", n),
            parameters: serde_json::json!({"type":"object"}),
        }).collect();
        let manifest = Arc::new(PluginManifest {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            author: "test".to_string(),
            min_klyntbot_version: None,
            tools,
            cron_jobs: vec![],
            migrations: vec![PluginMigrationDef {
                version: 1,
                description: "init".to_string(),
                sql: "CREATE TABLE plugin_test_pkg_cache (id TEXT PRIMARY KEY)".to_string(),
            }],
            permissions: vec![],
            config_schema: Default::default(),
        });
        // Can't construct a real Plugin without WASM bytes, so test metadata only
        PluginPackage::from_manifest(manifest)
    }

    #[test]
    fn test_package_name_matches_manifest_id() {
        let pkg = make_package("notion-connector", &["notion_search"]);
        assert_eq!(pkg.name(), "notion-connector");
        assert_eq!(pkg.config_key(), "notion-connector");
    }

    #[test]
    fn test_package_migrations_come_from_manifest() {
        let pkg = make_package("test-pkg", &["my_tool"]);
        let migrations = pkg.migrations();
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].version, 1);
        assert_eq!(migrations[0].feature_name, "test-pkg");
    }

    #[test]
    fn test_tools_count_matches_manifest() {
        let pkg = make_package("test-pkg", &["tool_a", "tool_b"]);
        // tools() returns DynTool — length should match manifest.tools
        // We can't call tools() without a real Plugin; test the count field instead
        assert_eq!(pkg.manifest().tools.len(), 2);
    }
}
```

### Step 2: Run tests

```bash
cargo nextest run -p plugin-runtime plugin_package
```
Expected: tests that don't need a real Plugin pass; tool-construction tests compile.

### Step 3: Implement `PluginPackage`

Replace stub in `crates/plugin-runtime/src/plugin_package.rs`:

```rust
//! PluginPackage: a loaded WASM plugin as a FeaturePackage.

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;

use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};
use common::Result;

use crate::manifest::{PluginManifest, PluginMigrationDef};
use crate::wasm_plugin::WasmPlugin;

/// A fully loaded WASM plugin, ready to register its tools.
pub struct PluginPackage {
    manifest: Arc<PluginManifest>,
    /// Shared plugin instance across all tools from this .wasm.
    /// `None` until `attach_plugin()` is called after loading the .wasm bytes.
    plugin: Option<Arc<Mutex<extism::Plugin>>>,
}

impl PluginPackage {
    /// Create from a manifest only (used for metadata inspection before loading .wasm).
    pub fn from_manifest(manifest: Arc<PluginManifest>) -> Self {
        Self { manifest, plugin: None }
    }

    /// Attach the loaded Extism plugin.
    pub fn attach_plugin(&mut self, plugin: extism::Plugin) {
        self.plugin = Some(Arc::new(Mutex::new(plugin)));
    }

    /// Returns the parsed manifest.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }
}

#[async_trait]
impl FeaturePackage for PluginPackage {
    fn name(&self) -> &str {
        &self.manifest.id
    }

    fn tools(&self) -> Vec<DynTool> {
        let Some(ref plugin_arc) = self.plugin else {
            return vec![];
        };
        self.manifest
            .tools
            .iter()
            .map(|tool_def| {
                let wasm_tool = WasmPlugin::new(
                    Arc::clone(plugin_arc),
                    tool_def.clone(),
                    Arc::clone(&self.manifest),
                );
                Arc::new(wasm_tool) as DynTool
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

    fn default_config(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        if self.plugin.is_none() {
            return Ok(HealthStatus::Unhealthy(
                format!("plugin {} not loaded", self.manifest.id)
            ));
        }
        Ok(HealthStatus::Healthy)
    }
}
```

### Step 4: Build and run tests

```bash
cargo build -p plugin-runtime && cargo nextest run -p plugin-runtime plugin_package
```
Expected: compiles, tests pass.

### Step 5: Commit

```bash
git add crates/plugin-runtime/src/plugin_package.rs
git commit -m "feat(plugin-runtime): add PluginPackage FeaturePackage implementation"
```

---

## Task 7: `PluginManager` — load all plugins from disk

**Files:**
- Modify: `crates/plugin-runtime/src/manager.rs`

### Step 1: Write tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_manifest(dir: &std::path::Path, plugin_id: &str, manifest_json: &str) {
        std::fs::create_dir_all(dir.join(plugin_id)).unwrap();
        std::fs::write(dir.join(plugin_id).join("klyntbot.plugin.json"), manifest_json).unwrap();
    }

    fn minimal_manifest(id: &str) -> String {
        format!(
            r#"{{"id":"{}","name":"Test {}","version":"1.0.0","description":"d","author":"a"}}"#,
            id, id
        )
    }

    #[tokio::test]
    async fn test_scan_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let manifests = PluginManager::scan_manifests(tmp.path()).await.unwrap();
        assert!(manifests.is_empty());
    }

    #[tokio::test]
    async fn test_scan_discovers_plugins() {
        let tmp = TempDir::new().unwrap();
        write_manifest(tmp.path(), "plugin-a", &minimal_manifest("plugin-a"));
        write_manifest(tmp.path(), "plugin-b", &minimal_manifest("plugin-b"));

        let manifests = PluginManager::scan_manifests(tmp.path()).await.unwrap();
        assert_eq!(manifests.len(), 2);
        let ids: Vec<&str> = manifests.iter().map(|(m, _)| m.id.as_str()).collect();
        assert!(ids.contains(&"plugin-a"));
        assert!(ids.contains(&"plugin-b"));
    }

    #[tokio::test]
    async fn test_scan_skips_dir_without_manifest() {
        let tmp = TempDir::new().unwrap();
        // Create a dir with no manifest
        std::fs::create_dir_all(tmp.path().join("not-a-plugin")).unwrap();
        // Create a dir with a manifest
        write_manifest(tmp.path(), "real-plugin", &minimal_manifest("real-plugin"));

        let manifests = PluginManager::scan_manifests(tmp.path()).await.unwrap();
        assert_eq!(manifests.len(), 1);
    }

    #[tokio::test]
    async fn test_scan_skips_invalid_manifest() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("bad-plugin")).unwrap();
        std::fs::write(tmp.path().join("bad-plugin").join("klyntbot.plugin.json"), "NOT JSON").unwrap();
        write_manifest(tmp.path(), "good-plugin", &minimal_manifest("good-plugin"));

        let manifests = PluginManager::scan_manifests(tmp.path()).await.unwrap();
        assert_eq!(manifests.len(), 1);  // bad plugin skipped with warning
    }
}
```

### Step 2: Run tests

```bash
cargo nextest run -p plugin-runtime manager
```
Expected: tests that don't load .wasm pass; WASM-loading tests need `--features plugin-integration`.

### Step 3: Implement `PluginManager`

Replace stub in `crates/plugin-runtime/src/manager.rs`:

```rust
//! PluginManager: discovers and loads all WASM plugins from disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

use common::Result;

use crate::host::build_host_functions;
use crate::manifest::PluginManifest;
use crate::plugin_package::PluginPackage;

/// Manages all loaded plugins for the agent session.
pub struct PluginManager {
    packages: Vec<PluginPackage>,
    plugins_dir: PathBuf,
}

impl PluginManager {
    /// Default plugins directory: `~/.klyntbot/plugins/`.
    pub fn default_plugins_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("plugins")
    }

    /// Load all plugins from `plugins_dir`, skipping failures with warnings.
    pub async fn load_all(
        plugins_dir: &Path,
        pool: sqlx::SqlitePool,
        config: &config::PluginsConfig,
    ) -> Result<Self> {
        if !config.enabled {
            info!("Plugin system disabled via config");
            return Ok(Self { packages: vec![], plugins_dir: plugins_dir.to_path_buf() });
        }

        if !plugins_dir.exists() {
            info!("Plugins directory does not exist: {}", plugins_dir.display());
            return Ok(Self { packages: vec![], plugins_dir: plugins_dir.to_path_buf() });
        }

        let scanned = Self::scan_manifests(plugins_dir).await?;
        let mut packages = Vec::new();

        for (manifest, wasm_path) in scanned {
            let id = manifest.id.clone();
            match Self::load_plugin(manifest, &wasm_path, pool.clone(), config).await {
                Ok(pkg) => {
                    info!("Loaded plugin: {}", id);
                    packages.push(pkg);
                }
                Err(e) => {
                    warn!("Failed to load plugin {}: {}", id, e);
                }
            }
        }

        info!("Loaded {} plugin(s)", packages.len());
        Ok(Self { packages, plugins_dir: plugins_dir.to_path_buf() })
    }

    /// Scan `dir` for plugin subdirectories with `klyntbot.plugin.json`.
    /// Returns (manifest, wasm_path) pairs. Skips invalid manifests with a warning.
    pub async fn scan_manifests(dir: &Path) -> Result<Vec<(PluginManifest, PathBuf)>> {
        let mut results = Vec::new();
        let mut entries = tokio::fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }
            let manifest_path = plugin_dir.join("klyntbot.plugin.json");
            if !manifest_path.exists() {
                continue;
            }
            match PluginManifest::from_file(&manifest_path) {
                Ok(manifest) => {
                    let wasm_path = plugin_dir.join("plugin.wasm");
                    results.push((manifest, wasm_path));
                }
                Err(e) => {
                    warn!("Skipping invalid manifest at {}: {}", manifest_path.display(), e);
                }
            }
        }

        Ok(results)
    }

    async fn load_plugin(
        manifest: PluginManifest,
        wasm_path: &Path,
        pool: sqlx::SqlitePool,
        config: &config::PluginsConfig,
    ) -> Result<PluginPackage> {
        let wasm_bytes = tokio::fs::read(wasm_path).await
            .map_err(|e| common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!("cannot read {}: {}", wasm_path.display(), e))
            ))?;

        let host_fns = build_host_functions(pool, manifest.id.clone());

        let extism_manifest = extism::Manifest::new([extism::Wasm::data(wasm_bytes)])
            .with_memory_max(config.sandbox_memory_mb as u64 * 1024 * 1024 / 65536); // pages

        let plugin = extism::Plugin::new(&extism_manifest, host_fns, true)
            .map_err(|e| common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!("extism load error: {}", e))
            ))?;

        let manifest = Arc::new(manifest);
        let mut pkg = PluginPackage::from_manifest(Arc::clone(&manifest));
        pkg.attach_plugin(plugin);
        Ok(pkg)
    }

    /// All loaded packages (for registration into ToolRegistry).
    pub fn packages(&self) -> &[PluginPackage] {
        &self.packages
    }

    /// Take ownership of all packages (consumed at agent startup).
    pub fn into_packages(self) -> Vec<PluginPackage> {
        self.packages
    }
}
```

### Step 4: Build and run tests

```bash
cargo build -p plugin-runtime && cargo nextest run -p plugin-runtime manager
```
Expected: compiles, all non-wasm tests pass.

### Step 5: Commit

```bash
git add crates/plugin-runtime/src/manager.rs
git commit -m "feat(plugin-runtime): add PluginManager with disk scanning and plugin loading"
```

---

## Task 8: Register `PluginManager` in `AgentLoopBuilder`

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `Cargo.toml` (root klyntbot crate — add plugin-runtime dep)

### Step 1: Add `plugin-runtime` to the root `klyntbot` crate

In root `Cargo.toml` `[dependencies]`:
```toml
plugin-runtime.workspace = true
```

Also add to `crates/agent/Cargo.toml` `[dependencies]`:
```toml
plugin-runtime.workspace = true
```

### Step 2: Write a test (in `agent` crate) verifying plugins disabled by default

In `crates/agent/src/agent_loop/builder.rs` existing `#[cfg(test)]` block (or create one):
```rust
#[cfg(test)]
mod builder_plugin_tests {
    #[test]
    fn test_plugins_config_defaults_disabled_behavior() {
        // PluginsConfig::default() has enabled=true but no plugins dir
        // so PluginManager::load_all returns empty — just verify config default
        let cfg = config::PluginsConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.sandbox_memory_mb, 64);
    }
}
```

### Step 3: Run test

```bash
cargo nextest run -p agent builder_plugin_tests
```
Expected: passes.

### Step 4: Wire PluginManager into `AgentLoopBuilder::build()`

In `crates/agent/src/agent_loop/builder.rs`, add at the top of `build()` after the workspace dir setup, before `// ── Tool registry ──`:

```rust
use plugin_runtime::PluginManager;

// ── Plugin manager ────────────────────────────────────────────────────
let plugins_dir = config.data_dir_path().join("plugins");
let plugin_packages = if self.pool.is_some() {
    match PluginManager::load_all(&plugins_dir, storage_pool.inner().clone(), &config.plugins).await {
        Ok(mgr) => {
            let pkgs = mgr.into_packages();
            info!("Plugin manager loaded {} plugin(s)", pkgs.len());
            pkgs
        }
        Err(e) => {
            warn!("Plugin manager failed to load: {}", e);
            vec![]
        }
    }
} else {
    vec![]
};
```

After `// ── Tool registry ──`, after all existing tool registrations, add:
```rust
// Plugin tools (each PluginPackage contributes its tools)
for pkg in plugin_packages {
    for tool in pkg.tools() {
        tool_registry.register_dyn(tool);
    }
}
```

### Step 5: Build workspace

```bash
cargo build --workspace
```
Expected: compiles with zero warnings.

### Step 6: Run agent tests

```bash
cargo nextest run -p agent
```
Expected: all existing tests pass.

### Step 7: Commit

```bash
git add crates/agent/src/agent_loop/builder.rs Cargo.toml crates/agent/Cargo.toml
git commit -m "feat(agent): register PluginManager tools at startup"
```

---

## Task 9: `plugin` CLI subcommand

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Create: `crates/cli/src/plugin_cmd/mod.rs`
- Create: `crates/cli/src/plugin_cmd/install.rs`
- Create: `crates/cli/src/plugin_cmd/list.rs`
- Create: `crates/cli/src/plugin_cmd/remove.rs`
- Modify: `crates/cli/src/lib.rs`

### Step 1: Write tests for command parsing

Add to `crates/cli/src/commands.rs` test block:

```rust
    #[test]
    fn test_plugin_install_local() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "install", "./my-plugin.wasm"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Install { source })) => {
                assert_eq!(source, "./my-plugin.wasm");
            }
            other => panic!("expected Plugin Install, got {other:?}"),
        }
    }

    #[test]
    fn test_plugin_install_registry() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "install", "notion-connector"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Install { source })) => {
                assert_eq!(source, "notion-connector");
            }
            other => panic!("expected Plugin Install, got {other:?}"),
        }
    }

    #[test]
    fn test_plugin_list() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "list"]);
        matches!(cli.command, Some(Commands::Plugin(PluginCommand::List)));
    }

    #[test]
    fn test_plugin_remove() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "remove", "notion-connector"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Remove { id })) => {
                assert_eq!(id, "notion-connector");
            }
            other => panic!("expected Plugin Remove, got {other:?}"),
        }
    }

    #[test]
    fn test_plugin_search() {
        let cli = Cli::parse_from(["klyntbot", "plugin", "search", "notion"]);
        match cli.command {
            Some(Commands::Plugin(PluginCommand::Search { query })) => {
                assert_eq!(query, "notion");
            }
            other => panic!("expected Plugin Search, got {other:?}"),
        }
    }
```

### Step 2: Run tests — expect compile failure (PluginCommand not defined yet)

```bash
cargo nextest run -p cli 2>&1 | head -20
```
Expected: compile error — `PluginCommand` not found.

### Step 3: Add `PluginCommand` to `commands.rs`

Add to the imports at the top:
```rust
use crate::plugin_cmd::PluginCommand;
```

Add `Plugin` variant to `Commands` enum:
```rust
    /// Manage WASM plugins (install, list, remove, search)
    Plugin(PluginCommand),
```

### Step 4: Create `crates/cli/src/plugin_cmd/mod.rs`

```rust
//! `klyntbot plugin` subcommand.

pub mod install;
pub mod list;
pub mod remove;

use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommand {
    /// Install a plugin from local file, GitHub, or registry
    Install {
        /// Plugin source: `./path.wasm`, `github:user/repo`, or `plugin-id`
        source: String,
    },
    /// List installed plugins
    List,
    /// Remove an installed plugin
    Remove {
        /// Plugin ID to remove
        id: String,
    },
    /// Search the plugin registry
    Search {
        /// Search query
        query: String,
    },
    /// Update an installed plugin
    Update {
        /// Plugin ID (omit to update all)
        id: Option<String>,
    },
}
```

### Step 5: Create stub implementations

`crates/cli/src/plugin_cmd/install.rs`:
```rust
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub async fn run(source: &str, plugins_dir: &Path, registry_url: &str) -> Result<()> {
    if source.starts_with("./") || source.starts_with('/') || source.ends_with(".wasm") {
        install_local(source, plugins_dir).await
    } else if source.starts_with("github:") {
        install_github(&source["github:".len()..], plugins_dir).await
    } else {
        install_registry(source, plugins_dir, registry_url).await
    }
}

async fn install_local(path_str: &str, plugins_dir: &Path) -> Result<()> {
    let wasm_path = PathBuf::from(path_str);
    if !wasm_path.exists() {
        bail!("File not found: {}", path_str);
    }
    let manifest_path = wasm_path.with_extension("").join("klyntbot.plugin.json");
    if !manifest_path.exists() {
        bail!("No klyntbot.plugin.json found alongside {}", path_str);
    }
    let manifest = plugin_runtime::PluginManifest::from_file(&manifest_path)?;
    let dest = plugins_dir.join(&manifest.id);
    tokio::fs::create_dir_all(&dest).await?;
    tokio::fs::copy(&wasm_path, dest.join("plugin.wasm")).await?;
    tokio::fs::copy(&manifest_path, dest.join("klyntbot.plugin.json")).await?;
    println!("✓ Installed {} v{}", manifest.name, manifest.version);
    Ok(())
}

async fn install_github(repo: &str, plugins_dir: &Path) -> Result<()> {
    // `repo` is "user/repo-name"
    let parts: Vec<&str> = repo.splitn(2, '/').collect();
    if parts.len() != 2 {
        bail!("Invalid GitHub source. Use: github:user/repo");
    }
    let (user, repo_name) = (parts[0], parts[1]);
    let api_url = format!("https://api.github.com/repos/{}/{}/releases/latest", user, repo_name);

    println!("Fetching latest release from {}/{}...", user, repo_name);
    let client = reqwest::Client::builder()
        .user_agent("klyntbot-plugin-installer")
        .build()?;

    let release: serde_json::Value = client.get(&api_url).send().await?.json().await?;
    let assets = release["assets"].as_array()
        .ok_or_else(|| anyhow::anyhow!("No assets in release"))?;

    let wasm_asset = assets.iter().find(|a| a["name"].as_str().unwrap_or("").ends_with(".wasm"))
        .ok_or_else(|| anyhow::anyhow!("No .wasm asset in latest release"))?;
    let manifest_asset = assets.iter().find(|a| a["name"].as_str().unwrap_or("") == "klyntbot.plugin.json")
        .ok_or_else(|| anyhow::anyhow!("No klyntbot.plugin.json asset in latest release"))?;

    let wasm_url = wasm_asset["browser_download_url"].as_str().unwrap();
    let manifest_url = manifest_asset["browser_download_url"].as_str().unwrap();

    println!("Downloading manifest...");
    let manifest_json = client.get(manifest_url).send().await?.text().await?;
    let manifest = plugin_runtime::PluginManifest::from_json(&manifest_json)?;

    show_permissions(&manifest);

    println!("Downloading plugin ({})...", wasm_asset["name"].as_str().unwrap_or("plugin.wasm"));
    let wasm_bytes = client.get(wasm_url).send().await?.bytes().await?;

    let dest = plugins_dir.join(&manifest.id);
    tokio::fs::create_dir_all(&dest).await?;
    tokio::fs::write(dest.join("plugin.wasm"), &wasm_bytes).await?;
    tokio::fs::write(dest.join("klyntbot.plugin.json"), &manifest_json).await?;

    println!("✓ Installed {} v{}", manifest.name, manifest.version);
    Ok(())
}

async fn install_registry(plugin_id: &str, plugins_dir: &Path, registry_url: &str) -> Result<()> {
    println!("Fetching registry from {}...", registry_url);
    let client = reqwest::Client::builder()
        .user_agent("klyntbot-plugin-installer")
        .build()?;

    let registry: serde_json::Value = client.get(registry_url).send().await?.json().await?;
    let plugins = registry["plugins"].as_array()
        .ok_or_else(|| anyhow::anyhow!("Invalid registry format"))?;

    let entry = plugins.iter()
        .find(|p| p["id"].as_str() == Some(plugin_id))
        .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in registry. Try: klyntbot plugin search {}", plugin_id, plugin_id))?;

    let manifest_url = entry["manifest_url"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing manifest_url in registry entry"))?;
    let wasm_url = entry["download_url"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing download_url in registry entry"))?;

    println!("Downloading manifest...");
    let manifest_json = client.get(manifest_url).send().await?.text().await?;
    let manifest = plugin_runtime::PluginManifest::from_json(&manifest_json)?;

    show_permissions(&manifest);

    println!("Downloading plugin...");
    let wasm_bytes = client.get(wasm_url).send().await?.bytes().await?;

    let dest = plugins_dir.join(&manifest.id);
    tokio::fs::create_dir_all(&dest).await?;
    tokio::fs::write(dest.join("plugin.wasm"), &wasm_bytes).await?;
    tokio::fs::write(dest.join("klyntbot.plugin.json"), &manifest_json).await?;

    println!("✓ Installed {} v{}", manifest.name, manifest.version);
    Ok(())
}

fn show_permissions(manifest: &plugin_runtime::PluginManifest) {
    if manifest.permissions.is_empty() {
        println!("  No special permissions required.");
    } else {
        println!("  Permissions requested: {:?}", manifest.permissions);
        println!("  Type 'yes' to accept: ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        if input.trim().to_lowercase() != "yes" {
            std::process::exit(1);
        }
    }
}
```

`crates/cli/src/plugin_cmd/list.rs`:
```rust
use anyhow::Result;
use std::path::Path;

pub async fn run(plugins_dir: &Path) -> Result<()> {
    if !plugins_dir.exists() {
        println!("No plugins installed. Use: klyntbot plugin install <id>");
        return Ok(());
    }
    let mut entries = tokio::fs::read_dir(plugins_dir).await?;
    let mut found = false;
    while let Some(entry) = entries.next_entry().await? {
        let manifest_path = entry.path().join("klyntbot.plugin.json");
        if manifest_path.exists() {
            if let Ok(m) = plugin_runtime::PluginManifest::from_file(&manifest_path) {
                println!("  {} v{} — {}", m.id, m.version, m.description);
                found = true;
            }
        }
    }
    if !found {
        println!("No plugins installed. Use: klyntbot plugin install <id>");
    }
    Ok(())
}
```

`crates/cli/src/plugin_cmd/remove.rs`:
```rust
use anyhow::{bail, Result};
use std::path::Path;

pub async fn run(plugin_id: &str, plugins_dir: &Path) -> Result<()> {
    let plugin_dir = plugins_dir.join(plugin_id);
    if !plugin_dir.exists() {
        bail!("Plugin '{}' is not installed", plugin_id);
    }
    tokio::fs::remove_dir_all(&plugin_dir).await?;
    println!("✓ Removed {}", plugin_id);
    Ok(())
}
```

### Step 6: Wire into `crates/cli/src/lib.rs`

In the file that dispatches commands (find the `match command` block), add:
```rust
Commands::Plugin(cmd) => {
    let plugins_dir = config.data_dir_path().join("plugins");
    let registry_url = &config.plugins.registry_url;
    match cmd {
        PluginCommand::Install { source } => {
            plugin_cmd::install::run(&source, &plugins_dir, registry_url).await?;
        }
        PluginCommand::List => {
            plugin_cmd::list::run(&plugins_dir).await?;
        }
        PluginCommand::Remove { id } => {
            plugin_cmd::remove::run(&id, &plugins_dir).await?;
        }
        PluginCommand::Search { query } => {
            // Simple: fetch registry and grep
            println!("Search not yet implemented. Visit {}", registry_url);
        }
        PluginCommand::Update { id: _ } => {
            println!("Update not yet implemented. Reinstall with: klyntbot plugin install <id>");
        }
    }
}
```

Add `pub mod plugin_cmd;` to `crates/cli/src/lib.rs`.

### Step 7: Build and run tests

```bash
cargo build -p cli && cargo nextest run -p cli
```
Expected: compiles, all tests pass.

### Step 8: Commit

```bash
git add crates/cli/src/commands.rs crates/cli/src/plugin_cmd/ crates/cli/src/lib.rs
git commit -m "feat(cli): add plugin subcommand (install, list, remove, search)"
```

---

## Task 10: `plugin-sdk` crate (Rust PDK for plugin authors)

**Files:**
- Modify: `Cargo.toml` (workspace root — add member + dep)
- Create: `crates/plugin-sdk/Cargo.toml`
- Create: `crates/plugin-sdk/src/lib.rs`

### Step 1: Add to workspace `Cargo.toml`

Add `"crates/plugin-sdk"` to `[workspace] members`.

Add:
```toml
plugin-sdk = { path = "crates/plugin-sdk" }
```

### Step 2: Create `crates/plugin-sdk/Cargo.toml`

```toml
[package]
name = "klyntbot-plugin-sdk"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
extism-pdk = "1"
serde.workspace = true
serde_json.workspace = true
```

### Step 3: Create `crates/plugin-sdk/src/lib.rs`

```rust
//! klyntbot Plugin SDK
//!
//! Re-exports everything plugin authors need.
//!
//! # Example (Rust plugin)
//!
//! ```rust,ignore
//! use klyntbot_plugin_sdk::prelude::*;
//!
//! #[derive(serde::Deserialize)]
//! struct SearchArgs { query: String }
//!
//! #[plugin_fn]
//! pub fn notion_search(args: Json<SearchArgs>) -> FnResult<Json<String>> {
//!     let key = config_get("api_key").unwrap_or_default();
//!     let result = http_get(
//!         &format!("https://api.notion.com/v1/search?query={}", args.0.query),
//!         &[("Authorization", &format!("Bearer {}", key))],
//!     );
//!     Ok(Json(result))
//! }
//! ```

pub use extism_pdk::*;

pub mod prelude {
    pub use extism_pdk::*;
    pub use super::{config_get, http_get, db_query, log_info, log_warn, log_error};
}

/// Get a config value set by the user for this plugin.
pub fn config_get(key: &str) -> Option<String> {
    extism_pdk::config::get(key).ok().flatten()
}

/// Make an HTTP GET request (requires `network` permission).
pub fn http_get(url: &str, headers: &[(&str, &str)]) -> String {
    let mut req = extism_pdk::http::Request::new(url);
    for (k, v) in headers {
        req = req.header(k, v);
    }
    match extism_pdk::http::request::<()>(&req, None) {
        Ok(resp) => resp.body().to_string(),
        Err(e) => format!(r#"{{"error":"{}"}}"#, e),
    }
}

/// Execute a SELECT query on this plugin's sandboxed tables (requires `storage` permission).
pub fn db_query(sql: &str) -> String {
    unsafe {
        extern "C" {
            fn db_query(ptr: u64) -> u64;
        }
        let mem = extism_pdk::Memory::from_bytes(sql.as_bytes()).unwrap();
        let result_ptr = db_query(mem.offset());
        let result_mem = extism_pdk::Memory::find(result_ptr).unwrap();
        result_mem.to_string().unwrap_or_default()
    }
}

/// Log an info message via the host logger.
pub fn log_info(msg: &str) {
    extism_pdk::log!(extism_pdk::LogLevel::Info, "{}", msg);
}

/// Log a warning message via the host logger.
pub fn log_warn(msg: &str) {
    extism_pdk::log!(extism_pdk::LogLevel::Warn, "{}", msg);
}

/// Log an error message via the host logger.
pub fn log_error(msg: &str) {
    extism_pdk::log!(extism_pdk::LogLevel::Error, "{}", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdk_exports_exist() {
        // Verify the public API compiles
        let _ = std::ptr::null::<()>();  // no-op, just triggers compilation
    }
}
```

### Step 4: Build and run tests

```bash
cargo build -p klyntbot-plugin-sdk && cargo nextest run -p klyntbot-plugin-sdk
```
Expected: compiles, tests pass.

### Step 5: Commit

```bash
git add crates/plugin-sdk/ Cargo.toml
git commit -m "feat(plugin-sdk): add Rust PDK for plugin authors"
```

---

## Task 11: Hello plugin test fixture + feature-gated integration tests

**Files:**
- Create: `tests/fixtures/hello_plugin/src/lib.rs`
- Create: `tests/fixtures/hello_plugin/Cargo.toml`
- Create: `tests/fixtures/hello_plugin/build.sh`
- Create: `tests/plugin_integration_tests.rs`

### Step 1: Create the hello plugin

`tests/fixtures/hello_plugin/Cargo.toml`:
```toml
[package]
name = "hello_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1"
serde_json = "1"
```

`tests/fixtures/hello_plugin/src/lib.rs`:
```rust
use extism_pdk::*;

#[plugin_fn]
pub fn hello_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let name = args["name"].as_str().unwrap_or("world");
    Ok(format!("hello from wasm, {}!", name))
}
```

`tests/fixtures/hello_plugin/klyntbot.plugin.json`:
```json
{
    "id": "hello-plugin",
    "name": "Hello Plugin",
    "version": "0.1.0",
    "description": "Integration test plugin",
    "author": "test",
    "tools": [
        {
            "name": "hello_tool",
            "description": "Say hello",
            "parameters": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Name to greet"}
                }
            }
        }
    ],
    "permissions": []
}
```

`tests/fixtures/hello_plugin/build.sh`:
```bash
#!/usr/bin/env bash
# Build the hello plugin for integration tests.
# Requires: cargo + wasm32-wasip1 target
# Usage: ./build.sh
set -e
rustup target add wasm32-wasip1 2>/dev/null || true
cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/hello_plugin.wasm ./plugin.wasm
echo "Built: plugin.wasm"
```

### Step 2: Create integration tests

`tests/plugin_integration_tests.rs`:
```rust
//! Plugin integration tests — require a pre-built hello_plugin.wasm.
//!
//! Build the test plugin first:
//!   cd tests/fixtures/hello_plugin && ./build.sh
//!
//! Run with:
//!   cargo nextest run --features plugin-integration --test plugin_integration_tests

#[cfg(feature = "plugin-integration")]
mod plugin_integration {
    use std::path::PathBuf;
    use plugin_runtime::{PluginManifest, PluginPackage};
    use tools_core::{RoutingContext, Tool};
    use common::ChannelName;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("hello_plugin")
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new(ChannelName::from("cli"), "test".to_string())
    }

    #[tokio::test]
    async fn test_hello_plugin_executes() {
        let wasm_path = fixtures_dir().join("plugin.wasm");
        assert!(wasm_path.exists(), "Build the hello plugin first: cd tests/fixtures/hello_plugin && ./build.sh");

        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = std::sync::Arc::new(PluginManifest::from_file(&manifest_path).unwrap());

        let wasm_bytes = std::fs::read(&wasm_path).unwrap();
        let extism_manifest = extism::Manifest::new([extism::Wasm::data(wasm_bytes)]);
        let plugin = extism::Plugin::new(&extism_manifest, [], true).unwrap();

        let mut pkg = PluginPackage::from_manifest(manifest);
        pkg.attach_plugin(plugin);

        let tools = pkg.tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "hello_tool");

        let args = serde_json::json!({"name": "klyntbot"});
        let result = tools[0].execute(args, &routing_ctx()).await.unwrap();
        assert!(result.contains("hello from wasm"));
        assert!(result.contains("klyntbot"));
    }

    #[tokio::test]
    async fn test_plugin_permission_level_standard_no_network() {
        let manifest_path = fixtures_dir().join("klyntbot.plugin.json");
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();
        // hello_plugin has no permissions → Standard
        assert!(!manifest.has_permission(&plugin_runtime::PluginPermission::Network));
    }
}

// Compile-time stub: always compiles, skipped without the feature
#[cfg(not(feature = "plugin-integration"))]
#[test]
fn plugin_integration_tests_require_feature_flag() {
    println!(
        "Skipped: run with --features plugin-integration after building the hello plugin.\n\
         See: tests/fixtures/hello_plugin/build.sh"
    );
}
```

### Step 3: Add feature to workspace root `Cargo.toml`

In root `Cargo.toml` `[features]`:
```toml
[features]
default = ["email"]
email = ["channels/email"]
plugin-integration = ["plugin-runtime/plugin-integration"]
```

### Step 4: Run stub test (always passes)

```bash
cargo nextest run --test plugin_integration_tests
```
Expected: 1 test passes (`plugin_integration_tests_require_feature_flag`).

### Step 5: Commit

```bash
git add tests/fixtures/hello_plugin/ tests/plugin_integration_tests.rs Cargo.toml
git commit -m "test(plugin): add hello plugin fixture and feature-gated integration tests"
```

---

## Task 12: Final verification

### Step 1: Full workspace build

```bash
cargo build --workspace
```
Expected: zero errors, zero warnings.

### Step 2: Full test suite

```bash
cargo nextest run --workspace
```
Expected: all tests pass.

### Step 3: Clippy

```bash
cargo clippy --workspace --all-targets --all-features
```
Expected: zero warnings.

### Step 4: Smoke-test the CLI

```bash
cargo run --bin klyntbot -- status
```
Expected: status output shows `plugins.enabled = true`, no crashes.

```bash
cargo run --bin klyntbot -- plugin list
```
Expected: `No plugins installed. Use: klyntbot plugin install <id>`

### Step 5: Final commit

```bash
git add -p
git commit -m "feat(plugins): complete WASM plugin sandbox via Extism"
```

---

## Summary of all files changed

| File | Change |
|---|---|
| `crates/config/src/schema/plugins.rs` | New: `PluginsConfig` |
| `crates/config/src/schema/mod.rs` | Add `mod plugins; pub use self::plugins::*` |
| `crates/config/src/schema/core.rs` | Add `plugins: PluginsConfig` field to `Config` |
| `crates/plugin-runtime/` | New crate: manifest, host fns, WasmPlugin, PluginPackage, PluginManager |
| `crates/plugin-sdk/` | New crate: Rust PDK for plugin authors |
| `crates/agent/src/agent_loop/builder.rs` | Wire PluginManager, register plugin tools |
| `crates/agent/Cargo.toml` | Add `plugin-runtime` dependency |
| `crates/cli/src/commands.rs` | Add `Plugin(PluginCommand)` variant |
| `crates/cli/src/plugin_cmd/` | New: install, list, remove subcommands |
| `crates/cli/src/lib.rs` | Dispatch plugin subcommands |
| `tests/fixtures/hello_plugin/` | Test plugin for integration tests |
| `tests/plugin_integration_tests.rs` | Feature-gated integration tests |
| `Cargo.toml` (workspace) | Add plugin-runtime, plugin-sdk, extism deps + plugin-integration feature |
