# Plugin System

Klyntbot supports third-party plugins compiled to WebAssembly (WASM). Two crates implement the system: `plugin-runtime` (the host that loads and runs plugins inside the agent) and `plugin-sdk` (an out-of-workspace SDK crate that plugin authors depend on). Plugins are sandboxed via [Extism](https://extism.org/) and interact with the host through a controlled set of host functions.

---

## Section 1: Narrative Overview

### Plugin Architecture

The plugin system uses a WASM sandbox model. Each plugin ships as a `plugin.wasm` binary plus a `klyntbot.plugin.json` manifest. At agent startup the `PluginManager` scans `~/.klyntbot/plugins/`, loads each valid plugin into an Extism WASM runtime, wraps every tool the plugin declares as a `WasmPlugin` (which implements the `Tool` trait), and registers them into the agent's `ToolRegistry`. From the LLM's perspective, plugin tools are indistinguishable from built-in tools.

```
~/.klyntbot/plugins/
  notion-connector/
    klyntbot.plugin.json   <- manifest
    plugin.wasm            <- compiled WASM module
  weather/
    klyntbot.plugin.json
    plugin.wasm
```

The runtime lives at **Layer 3** (same as `tools`) and integrates with the agent builder at **Layer 5**.

### Plugin Manifest Format

Every plugin requires a `klyntbot.plugin.json` manifest in camelCase JSON. The manifest declares the plugin's identity, the tools it exposes, any cron jobs it needs, database migrations, permissions, and configuration schema.

**Minimal manifest:**

```json
{
  "id": "hello-plugin",
  "name": "Hello Plugin",
  "version": "1.0.0",
  "description": "A test plugin",
  "author": "test"
}
```

**Full manifest (all fields):**

```json
{
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
      "parameters": {
        "type": "object",
        "properties": { "query": { "type": "string" } },
        "required": ["query"]
      }
    }
  ],
  "cronJobs": [
    {
      "tool": "notion_sync",
      "schedule": "0 * * * *",
      "description": "Hourly Notion sync"
    }
  ],
  "migrations": [
    {
      "version": 1,
      "description": "Create cache table",
      "sql": "CREATE TABLE plugin_notion_connector_cache (id TEXT PRIMARY KEY)"
    }
  ],
  "permissions": ["network", "storage"],
  "configSchema": {
    "api_key": {
      "type": "string",
      "secret": true,
      "description": "Notion API key"
    }
  }
}
```

Required fields: `id`, `name`, `version`, `description`, `author`. All other fields default to empty/absent.

Source: `crates/plugin-runtime/src/manifest.rs` lines 46-66.

### Plugin Lifecycle

#### 1. Discovery

`PluginManager::scan_manifests()` reads the plugins directory, iterates over subdirectories, and looks for `klyntbot.plugin.json` in each. Subdirectories without a manifest or with an invalid one are skipped with a warning. Returns `(PluginManifest, PathBuf)` pairs where the path points to the expected `plugin.wasm` location.

Source: `crates/plugin-runtime/src/manager.rs` lines 32-70.

#### 2. Loading

`PluginManager::load_all()` orchestrates the full load sequence:

1. If `config.plugins.enabled` is `false`, returns an empty manager immediately.
2. If the plugins directory does not exist, returns an empty manager.
3. Calls `scan_manifests()` to discover plugins.
4. For each discovered plugin, calls `load_plugin()` which:
   - Reads the `.wasm` bytes from disk.
   - Builds host functions via `host::build_host_functions()`, injecting the `SqlitePool`, plugin ID, permissions, and optional bus sender.
   - Converts `sandbox_memory_mb` from the config into WASM memory pages (1 page = 64 KiB).
   - Creates an `extism::Plugin` with the WASM data, host functions, and WASI enabled (`true`).
   - Wraps the result in a `PluginPackage`.
5. Failed loads are logged as warnings but do not prevent other plugins from loading.

Source: `crates/plugin-runtime/src/manager.rs` lines 77-162.

#### 3. Initialization (Agent Builder)

The agent builder in `crates/agent/src/agent_loop/builder.rs` (around line 520) performs two steps for each loaded package:

- **Cron job registration**: For each `cronJob` in the manifest, registers it with the `CronHandler` using the naming pattern `plugin:{plugin_id}:{tool_name}`.
- **Tool registration**: Calls `package.tools()` and registers each `DynTool` via `tool_registry.register_dyn()`.

Source: `crates/agent/src/agent_loop/builder.rs` lines 519-580.

#### 4. Execution

When the LLM calls a plugin tool, the `WasmPlugin::execute()` method:

1. Serializes the arguments to JSON.
2. Acquires the `Mutex<extism::Plugin>` lock.
3. Calls `plugin.call::<&str, &str>(func_name, &input)` -- the Extism runtime invokes the named WASM export.
4. Returns the string output.

The WASM function name matches the `tool_def.name` from the manifest. The plugin's exported function receives a JSON string and must return a JSON string.

Source: `crates/plugin-runtime/src/wasm_plugin.rs` lines 71-88.

### WASM Host Environment

All host functions are registered under the `"klyntbot"` Extism namespace. Each function enforces permission checks before executing. There are five functional namespaces:

#### db namespace (requires `storage` permission)

| Function | Signature | Description |
|----------|-----------|-------------|
| `db_query` | `(PTR) -> PTR` | Execute a read-only SELECT query. Rejects mutations, multi-statement injections, and non-SELECT statements. Uses `is_select_only()` for validation. |
| `db_execute` | `(PTR) -> PTR` | Execute a write query (INSERT, UPDATE, DELETE, CREATE TABLE, etc.) on sandboxed plugin tables. Returns `{"rows_affected": N}` on success. |

Both functions bridge async SQLite operations by spawning a scoped thread that calls `handle.block_on()` on the tokio runtime.

Source: `crates/plugin-runtime/src/host/mod.rs` lines 86-195.

**SQL injection protection**: `is_select_only()` (lines 26-57) rejects statements that don't start with SELECT/WITH/EXPLAIN, contain multiple statements (internal semicolons), or include mutation keywords (INSERT, UPDATE, DELETE, DROP, ALTER, CREATE, REPLACE, ATTACH, DETACH, PRAGMA, VACUUM, REINDEX).

**Table namespacing**: `is_plugin_table()` (lines 61-64) validates that table names follow the `plugin_{plugin_id}_` prefix convention (hyphens in plugin IDs are replaced with underscores).

#### log namespace (no permission required)

| Function | Signature | Description |
|----------|-----------|-------------|
| `log_debug` | `(PTR) -> ()` | Emit a debug log tagged with the plugin ID. |
| `log_info` | `(PTR) -> ()` | Emit an info log tagged with the plugin ID. |
| `log_warn` | `(PTR) -> ()` | Emit a warning log tagged with the plugin ID. |
| `log_error` | `(PTR) -> ()` | Emit an error log tagged with the plugin ID. |

Source: `crates/plugin-runtime/src/host/mod.rs` lines 197-269.

#### http namespace (requires `network` permission)

| Function | Signature | Description |
|----------|-----------|-------------|
| `http_request` | `(PTR) -> PTR` | Make an HTTP request. Input: JSON `{"url": "...", "method": "GET|POST|PUT|DELETE|PATCH", "body": ...}`. Returns `{"status": 200, "body": "..."}`. Defaults to GET if method omitted. |

Source: `crates/plugin-runtime/src/host/mod.rs` lines 271-354.

#### agent namespace (requires `agent` permission)

| Function | Signature | Description |
|----------|-----------|-------------|
| `agent_send_message` | `(PTR) -> PTR` | Send a message via the outbound bus. Input: `{"channel": "...", "chat_id": "...", "content": "..."}`. Returns `{"ok": true}` on success. |
| `agent_ask_user` | `(PTR) -> PTR` | **Stub** -- returns `{"error": "agent callbacks not connected"}`. Reserved for future interactive plugin workflows (Task #8). |
| `agent_emit_event` | `(PTR) -> PTR` | Emit a custom event. Currently logged at info level. Returns `{"ok": true}`. |

Source: `crates/plugin-runtime/src/host/mod.rs` lines 356-475.

#### tool namespace (no permission required)

| Function | Signature | Description |
|----------|-----------|-------------|
| `tool_return` | `(PTR) -> ()` | Signal a successful tool result. Currently logged; the host reads the actual return value from the WASM function's return. |
| `tool_error` | `(PTR) -> ()` | Signal a tool error. Logged at error level. |

Source: `crates/plugin-runtime/src/host/mod.rs` lines 477-521.

### PluginManager

`PluginManager` is the top-level coordinator for plugin discovery and loading. It holds the loaded `PluginPackage` instances and the path to the plugins directory.

Key behaviors:
- Default plugins directory: `~/.klyntbot/plugins/` (via `default_plugins_dir()`).
- Scanning is separate from loading -- `scan_manifests()` is a static method usable independently (the CLI `list` and `update` commands use it directly).
- `load_all()` is the primary entry point, consuming a `PluginsConfig`, `SqlitePool`, and optional bus sender.
- `into_packages()` consumes the manager and returns ownership of the packages to the caller (the agent builder).

Source: `crates/plugin-runtime/src/manager.rs` lines 14-173.

### Plugin Packaging (PluginPackage and FeaturePackage)

`PluginPackage` implements the `FeaturePackage` trait from `tools-core`, which is the same abstraction used by built-in feature crates like `feature-todo` and `feature-finance`. This means plugins participate in the same tool registration, migration, config, and health-check infrastructure as first-party features.

Key `FeaturePackage` methods implemented by `PluginPackage`:

- `name()` returns the plugin's `id` (not its display name).
- `tools()` creates `WasmPlugin` instances for each tool definition, sharing a single `Arc<Mutex<extism::Plugin>>`. Returns empty if no WASM plugin is attached.
- `migrations()` maps manifest migration definitions to `FeatureMigration` structs.
- `config_key()` returns the plugin's `id`.
- `default_config()` generates default values from the `configSchema`: empty strings for `"string"`, 0 for `"integer"`/`"number"`, false for `"boolean"`, null otherwise.
- `health_check()` returns `Healthy` if the WASM plugin is loaded, `Degraded` otherwise.

Source: `crates/plugin-runtime/src/plugin_package.rs` lines 14-104.
Trait definition: `crates/tools-core/src/feature.rs` lines 29-50.

### How Plugins Register Tools and Cron Jobs

**Tools**: Each tool declared in the manifest's `tools` array becomes a `WasmPlugin` instance wrapping the shared Extism plugin. The `WasmPlugin` implements `Tool` with `name()`, `description()`, and `parameters()` sourced from the manifest's `PluginToolDef`. The WASM export function name must exactly match the tool name.

**Permission level**: Computed from the manifest's `permissions` array. Plugins requesting `network` or `agent` permission get `PermissionLevel::Elevated`; all others get `PermissionLevel::Standard`. This affects which channels can invoke the tool.

Source: `crates/plugin-runtime/src/wasm_plugin.rs` lines 37-45.

**Cron jobs**: The agent builder iterates `package.manifest().cron_jobs` and registers each with the `CronHandler` using:
- Name: `plugin:{plugin_id}:{tool_name}`
- Schedule: the cron expression from the manifest
- Message: `"Run plugin tool: {tool_name}"`
- Marked as `internal: true`

Source: `crates/agent/src/agent_loop/builder.rs` lines 531-563.

### Plugin SDK

The `klyntbot-plugin-sdk` crate (`crates/plugin-sdk/`) is excluded from the workspace and published independently. It targets `wasm32-wasip1` (crate-type: `cdylib` + `rlib`) and depends on `extism-pdk`, `serde`, and `serde_json`.

The SDK re-exports everything from `extism_pdk` and provides a `prelude` module with convenience wrappers:

| Function | Description |
|----------|-------------|
| `config_get(key)` | Retrieve a user-configured value for this plugin. |
| `http_get(url, headers)` | Make an HTTP GET request (requires `network` permission). |
| `db_query(sql)` | Execute a SELECT query on sandboxed tables (requires `storage` permission). Currently a placeholder returning `"[]"`. |
| `log_info(msg)` | Log at info level via the host. |
| `log_warn(msg)` | Log at warning level via the host. |
| `log_error(msg)` | Log at error level via the host. |

**Example plugin** (Rust):

```rust
use klyntbot_plugin_sdk::prelude::*;

#[plugin_fn]
pub fn my_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input)?;
    Ok(format!("Got: {}", args))
}
```

Source: `crates/plugin-sdk/src/lib.rs` lines 1-68.
Cargo.toml: `crates/plugin-sdk/Cargo.toml` lines 1-15.

### Feature Gate: `plugin-integration`

Integration tests that require a pre-built WASM binary are gated behind the `plugin-integration` Cargo feature. This feature is defined at:

- Workspace root: `Cargo.toml` line 134 -- `plugin-integration = ["plugin-runtime/plugin-integration"]`
- Plugin runtime crate: `crates/plugin-runtime/Cargo.toml` lines 30-31

To run integration tests:

```bash
cd tests/fixtures/hello_plugin && ./build.sh
cargo nextest run --features plugin-integration --test plugins
```

The test fixture lives at `tests/fixtures/hello_plugin/` and includes a minimal Rust plugin (`hello_tool`) that returns a greeting string.

Source: `tests/plugins.rs` lines 1-183.

### CLI Plugin Commands

The `klyntbot plugin` subcommand provides seven operations, implemented in `crates/cli/src/plugin_cmd/`:

| Command | Description | Source File |
|---------|-------------|-------------|
| `klyntbot plugin install <source>` | Install from local path (`./ or /abs`), GitHub release (`github:user/repo`), or registry (`plugin-id[@version]`). Displays requested permissions before installing. | `install.rs` (lines 1-218) |
| `klyntbot plugin list` | List all installed plugins by scanning manifests. Shows ID, version, and description in a table. | `list.rs` (lines 1-33) |
| `klyntbot plugin remove <id>` | Delete a plugin's directory from the plugins folder. | `remove.rs` (lines 1-17) |
| `klyntbot plugin search <query>` | Query the plugin registry index. Matches against ID, name, and description (case-insensitive). | `search.rs` (lines 1-54) |
| `klyntbot plugin update [id]` | Update a specific plugin or all plugins to the latest registry version. Compares installed vs. registry versions. | `update.rs` (lines 1-90) |
| `klyntbot plugin new <name> [--lang rust\|typescript\|python]` | Scaffold a new plugin project with Cargo.toml/package.json, source template, and manifest. Defaults to Rust. | `new_plugin.rs` (lines 1-223) |
| `klyntbot plugin publish` | Print instructions for publishing: push to GitHub, create a release with `plugin.wasm` and `klyntbot.plugin.json` as assets, then open a PR against the registry repo. | `publish.rs` (lines 1-44) |

Routing: `crates/cli/src/plugin_cmd/mod.rs` lines 13-68.

**Install sources**:
- **Local**: `./path/to/plugin.wasm` or a directory containing both `plugin.wasm` and `klyntbot.plugin.json`.
- **GitHub**: `github:user/repo` -- fetches the latest release and downloads the `plugin.wasm` and `klyntbot.plugin.json` assets.
- **Registry**: `plugin-id` or `plugin-id@1.0.0` -- fetches `index.json` from the configured registry URL, finds the plugin, downloads the WASM binary.

### Configuration

Plugin system configuration is in the `plugins` section of `~/.klyntbot/config.json`:

```json
{
  "plugins": {
    "enabled": true,
    "registryUrl": "https://plugins.klyntbot.io/index.json",
    "sandboxMemoryMb": 64,
    "allowNetworkByDefault": false
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | bool | `true` | Master switch for the plugin system. |
| `registryUrl` | string | `"https://plugins.klyntbot.io/index.json"` | URL of the plugin registry index. |
| `sandboxMemoryMb` | u32 | `64` | Maximum WASM memory per plugin (converted to pages at load time). |
| `allowNetworkByDefault` | bool | `false` | Reserved for future use. |

Source: `crates/config/src/schema/plugins.rs` lines 6-41.

---

## Section 2: API Reference

### PluginManager

Defined in `crates/plugin-runtime/src/manager.rs` lines 14-17.

```rust
pub struct PluginManager {
    packages: Vec<PluginPackage>,
    plugins_dir: PathBuf,
}
```

#### Methods

| Method | Signature | Description | Line |
|--------|-----------|-------------|------|
| `default_plugins_dir` | `fn() -> PathBuf` | Returns `~/.klyntbot/plugins/`. Falls back to `./.klyntbot/plugins/` if home dir unavailable. | 21-26 |
| `scan_manifests` | `fn(dir: &Path) -> common::Result<Vec<(PluginManifest, PathBuf)>>` | Scans a directory for plugin subdirectories containing `klyntbot.plugin.json`. Returns manifest + wasm path pairs. Skips invalid manifests with warnings. | 32-70 |
| `load_all` | `fn(plugins_dir: &Path, pool: SqlitePool, config: &PluginsConfig, bus_sender: Option<Sender<OutboundMessage>>) -> common::Result<Self>` | Discovers and loads all plugins. Returns early if disabled or directory missing. | 77-127 |
| `packages` | `fn(&self) -> &[PluginPackage]` | Borrow the loaded packages. | 165-167 |
| `into_packages` | `fn(self) -> Vec<PluginPackage>` | Consume the manager and return owned packages. | 170-172 |

Private:

| Method | Signature | Description | Line |
|--------|-----------|-------------|------|
| `load_plugin` | `fn(manifest, wasm_path, pool, config, bus_sender) -> common::Result<PluginPackage>` | Load a single plugin: read WASM bytes, build host functions, create Extism plugin, wrap in PluginPackage. | 130-162 |

### WasmPlugin

Defined in `crates/plugin-runtime/src/wasm_plugin.rs` lines 15-19.

```rust
pub struct WasmPlugin {
    plugin: Arc<Mutex<extism::Plugin>>,
    tool_def: PluginToolDef,
    manifest: Arc<PluginManifest>,
}
```

#### Methods

| Method | Signature | Description | Line |
|--------|-----------|-------------|------|
| `new` | `fn(plugin: Arc<Mutex<extism::Plugin>>, tool_def: PluginToolDef, manifest: Arc<PluginManifest>) -> Self` | Create a WasmPlugin wrapping an Extism plugin instance. | 23-33 |
| `compute_permission_level` | `fn(manifest: &PluginManifest) -> PermissionLevel` | Private. Returns `Elevated` if plugin has `network` or `agent` permission, `Standard` otherwise. | 37-45 |

#### Tool trait implementation (lines 58-93)

| Method | Return | Description |
|--------|--------|-------------|
| `name()` | `&str` | Returns `tool_def.name`. |
| `description()` | `&str` | Returns `tool_def.description`. |
| `parameters()` | `Value` | Clones `tool_def.parameters`. |
| `execute(args, ctx)` | `Result<String>` | Serializes args to JSON, calls the WASM export function by tool name, returns the string result. Errors mapped to `KlyntbotError::Tool(ToolError::ExecutionFailed(...))`. |
| `permission_level()` | `PermissionLevel` | Delegates to `compute_permission_level()`. |

### PluginManifest

Defined in `crates/plugin-runtime/src/manifest.rs` lines 46-66.

```rust
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub min_klyntbot_version: Option<String>,
    pub tools: Vec<PluginToolDef>,
    pub cron_jobs: Vec<PluginCronJob>,
    pub migrations: Vec<PluginMigrationDef>,
    pub permissions: Vec<PluginPermission>,
    pub config_schema: HashMap<String, PluginConfigField>,
}
```

#### Fields

| Field | Type | Required | JSON Key | Description |
|-------|------|----------|----------|-------------|
| `id` | `String` | yes | `id` | Unique plugin identifier (used as directory name, config key, and `FeaturePackage::name()`). |
| `name` | `String` | yes | `name` | Human-readable display name. |
| `version` | `String` | yes | `version` | Semantic version string. |
| `description` | `String` | yes | `description` | Short description. |
| `author` | `String` | yes | `author` | Plugin author. |
| `min_klyntbot_version` | `Option<String>` | no | `minKlyntbotVersion` | Minimum klyntbot version required. |
| `tools` | `Vec<PluginToolDef>` | no | `tools` | Tool definitions. Defaults to empty. |
| `cron_jobs` | `Vec<PluginCronJob>` | no | `cronJobs` | Cron job definitions. Defaults to empty. |
| `migrations` | `Vec<PluginMigrationDef>` | no | `migrations` | SQL migration definitions. Defaults to empty. |
| `permissions` | `Vec<PluginPermission>` | no | `permissions` | Required permissions. Defaults to empty. |
| `config_schema` | `HashMap<String, PluginConfigField>` | no | `configSchema` | Configuration fields the user can set. Defaults to empty. |

#### Methods

| Method | Signature | Description | Line |
|--------|-----------|-------------|------|
| `from_json` | `fn(json: &str) -> Result<Self, serde_json::Error>` | Deserialize from a JSON string. | 69-71 |
| `from_file` | `fn(path: &Path) -> anyhow::Result<Self>` | Read and deserialize from a file path. | 73-76 |
| `has_permission` | `fn(&self, perm: &PluginPermission) -> bool` | Check if the plugin declares a specific permission. | 78-80 |

### Supporting Manifest Types

#### PluginPermission (line 6-12)

```rust
pub enum PluginPermission {
    Network,   // HTTP requests
    Storage,   // Database read/write
    Agent,     // Send messages, emit events
}
```

Serialized as `snake_case` strings: `"network"`, `"storage"`, `"agent"`. Unknown values cause deserialization errors.

#### PluginToolDef (lines 14-19)

```rust
pub struct PluginToolDef {
    pub name: String,          // Must match the WASM export function name
    pub description: String,   // Shown to the LLM
    pub parameters: Value,     // JSON Schema for tool parameters
}
```

#### PluginCronJob (lines 21-27)

```rust
pub struct PluginCronJob {
    pub tool: String,       // Tool name to invoke
    pub schedule: String,   // Cron expression (e.g., "0 * * * *")
    pub description: String, // Optional, defaults to ""
}
```

#### PluginMigrationDef (lines 29-34)

```rust
pub struct PluginMigrationDef {
    pub version: i64,        // Migration version number
    pub description: String, // Human-readable description
    pub sql: String,         // Raw SQL to execute
}
```

Convention: prefix table names with `plugin_{plugin_id}_` (hyphens replaced by underscores) to namespace storage.

#### PluginConfigField (lines 36-44)

```rust
pub struct PluginConfigField {
    pub field_type: String,    // JSON key: "type". Values: "string", "integer", "number", "boolean"
    pub secret: bool,          // If true, value should be treated as sensitive (default: false)
    pub description: String,   // Human-readable description (default: "")
}
```

### PluginPackage

Defined in `crates/plugin-runtime/src/plugin_package.rs` lines 15-18.

```rust
pub struct PluginPackage {
    manifest: Arc<PluginManifest>,
    plugin: Option<Arc<Mutex<extism::Plugin>>>,
}
```

#### Methods

| Method | Signature | Description | Line |
|--------|-----------|-------------|------|
| `from_manifest` | `fn(manifest: PluginManifest) -> Self` | Create a package from a parsed manifest (no WASM loaded yet). | 22-27 |
| `attach_plugin` | `fn(&mut self, plugin: extism::Plugin)` | Attach a loaded Extism plugin instance, wrapping it in `Arc<Mutex<>>`. | 30-32 |
| `manifest` | `fn(&self) -> &PluginManifest` | Access the manifest. | 35-37 |

#### FeaturePackage trait implementation (lines 41-103)

| Method | Return | Description |
|--------|--------|-------------|
| `name()` | `&str` | Returns `manifest.id`. |
| `tools()` | `Vec<DynTool>` | Creates `WasmPlugin` instances for each tool definition, sharing the `Arc<Mutex<extism::Plugin>>`. Empty if no plugin attached. |
| `migrations()` | `Vec<FeatureMigration>` | Maps manifest migrations to `FeatureMigration` structs with `feature_name` set to the plugin ID. |
| `config_key()` | `&str` | Returns `manifest.id`. |
| `default_config()` | `Value` | Generates default values from `config_schema` by type. |
| `health_check()` | `Result<HealthStatus>` | `Healthy` if WASM plugin is loaded, `Degraded("WASM plugin not loaded")` otherwise. |

### Host Functions

Built by `host::build_host_functions()` in `crates/plugin-runtime/src/host/mod.rs` lines 69-524.

```rust
pub fn build_host_functions(
    pool: SqlitePool,
    plugin_id: String,
    permissions: Vec<PluginPermission>,
    bus_sender: Option<Sender<OutboundMessage>>,
) -> Vec<Function>
```

Returns a `Vec<extism::Function>`, each registered in the `"klyntbot"` namespace.

**Summary of all 11 host functions:**

| Namespace | Function | Permission | Input | Output | Line |
|-----------|----------|-----------|-------|--------|------|
| db | `db_query` | `Storage` | SQL string | Query result or error string | 89-145 |
| db | `db_execute` | `Storage` | SQL string | `{"rows_affected": N}` or error | 148-195 |
| log | `log_debug` | none | Message string | (void) | 200-215 |
| log | `log_info` | none | Message string | (void) | 217-232 |
| log | `log_warn` | none | Message string | (void) | 235-250 |
| log | `log_error` | none | Message string | (void) | 253-268 |
| http | `http_request` | `Network` | `{"url","method","body"}` | `{"status","body"}` or error | 274-354 |
| agent | `agent_send_message` | `Agent` | `{"channel","chat_id","content"}` | `{"ok":true}` or error | 360-409 |
| agent | `agent_ask_user` | `Agent` | Question string | `{"error":"agent callbacks not connected"}` (stub) | 412-442 |
| agent | `agent_emit_event` | `Agent` | Event JSON string | `{"ok":true}` | 444-475 |
| tool | `tool_return` | none | Result string | (void) | 480-502 |
| tool | `tool_error` | none | Error string | (void) | 505-521 |

**Internal helpers:**

| Function | Signature | Description | Line |
|----------|-----------|-------------|------|
| `is_select_only` | `fn(sql: &str) -> bool` | Validates that a SQL string is a read-only query. | 26-57 |
| `is_plugin_table` | `fn(table_name: &str, plugin_id: &str) -> bool` | Validates table name follows plugin namespace prefix. | 61-64 |

### HostContext (private)

Defined in `crates/plugin-runtime/src/host/mod.rs` lines 12-18. Shared state passed to all host functions via Extism `UserData`:

```rust
struct HostContext {
    pool: SqlitePool,
    plugin_id: String,
    permissions: Vec<PluginPermission>,
    bus_sender: Option<Sender<OutboundMessage>>,
    http_client: reqwest::Client,
}
```

### Plugin SDK Exports

Defined in `crates/plugin-sdk/src/lib.rs`.

The `klyntbot-plugin-sdk` crate (`0.1.0`, MIT, edition 2021) provides:

**Re-exports** (line 17): `pub use extism_pdk::*` -- the full Extism PDK including `#[plugin_fn]`, `FnResult`, `Json`, `Host`, logging macros, and memory management.

**Prelude** (lines 19-25):
```rust
pub mod prelude {
    pub use extism_pdk::*;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;
    pub use super::{config_get, db_query, http_get, log_error, log_info, log_warn};
}
```

**Helper functions:**

| Function | Signature | Description | Line |
|----------|-----------|-------------|------|
| `config_get` | `fn(key: &str) -> Option<String>` | Get a config value set by the user for this plugin. | 28-30 |
| `http_get` | `fn(url: &str, headers: &[(&str, &str)]) -> String` | Make an HTTP GET request with optional headers. Returns body or error JSON. | 33-42 |
| `db_query` | `fn(sql: &str) -> String` | Execute a SELECT query on sandboxed tables. Currently returns `"[]"` (placeholder for host function wiring). | 46-53 |
| `log_info` | `fn(msg: &str)` | Log info message via host. | 56-58 |
| `log_warn` | `fn(msg: &str)` | Log warning message via host. | 61-63 |
| `log_error` | `fn(msg: &str)` | Log error message via host. | 66-68 |

**Build targets**: `cdylib` (for WASM output) and `rlib` (for Rust dependency resolution).

### Error Types

Plugin errors flow through the standard `KlyntbotError` hierarchy:

| Error | Variant | Context |
|-------|---------|---------|
| WASM function call failure | `KlyntbotError::Tool(ToolError::ExecutionFailed("WASM plugin call failed: ..."))` | `wasm_plugin.rs` line 82-85 |
| Extism plugin creation failure | `KlyntbotError::Tool(ToolError::ExecutionFailed("failed to create extism plugin: ..."))` | `manager.rs` lines 151-155 |
| Permission denied (host functions) | Returns error string in output: `"error: storage permission denied"`, `"error: network permission denied"`, `"error: agent permission denied"` | `host/mod.rs` passim |
| SQL validation failure | Returns `"error: only SELECT queries allowed in db_query"` | `host/mod.rs` line 108 |
| Missing manifest fields | `serde_json::Error` (deserialization failure) | `manifest.rs` line 70 |

Host function errors are returned as error strings in the WASM output memory rather than as Rust `Result::Err` values, since the Extism FFI boundary uses string-based communication.

### PluginsConfig

Defined in `crates/config/src/schema/plugins.rs` lines 6-20.

```rust
pub struct PluginsConfig {
    pub enabled: bool,                  // default: true
    pub registry_url: String,           // default: "https://plugins.klyntbot.io/index.json"
    pub sandbox_memory_mb: u32,         // default: 64
    pub allow_network_by_default: bool, // default: false
}
```

Serialization uses `camelCase` keys: `registryUrl`, `sandboxMemoryMb`, `allowNetworkByDefault`.

### Test Fixture: hello_plugin

Location: `tests/fixtures/hello_plugin/`

| File | Description |
|------|-------------|
| `klyntbot.plugin.json` | Manifest declaring `hello_tool` with no permissions. |
| `src/lib.rs` | Minimal Rust plugin: `hello_tool(input) -> "hello from wasm, {name}!"` |
| `build.sh` | Build script: `cargo build --target wasm32-wasip1 --release`, copies to `plugin.wasm`. |

Build and test:
```bash
cd tests/fixtures/hello_plugin && ./build.sh
cargo nextest run --features plugin-integration --test plugins
```
