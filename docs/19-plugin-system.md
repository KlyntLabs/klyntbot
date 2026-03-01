# Plugin System

## Purpose

The plugin system allows Klyntbot to be extended with third-party functionality through sandboxed WASM modules. It spans two workspace crates: `plugin-runtime` handles the host-side loading, sandboxing, and execution of plugins, while `plugin-sdk` provides the guest-side helpers that plugin authors use when writing their WASM modules. Together they form a capability-based plugin architecture where each plugin declares what it needs (network, storage, agent access) in a manifest, and the host enforces those boundaries at runtime.

Plugins are distributed as pairs of files -- a `plugin.wasm` binary and a `klyntbot.plugin.json` manifest -- installed into subdirectories of `~/.klyntbot/plugins/`. The CLI's `plugin` subcommand handles installation from local paths, GitHub releases, and a centralized registry.

## Key Types

### plugin-runtime (Host Side)

**`PluginManager`** -- discovers and loads all WASM plugins from disk. Its `scan_manifests(dir)` class method walks the plugins directory looking for subdirectories containing a `klyntbot.plugin.json` file, returning `(PluginManifest, PathBuf)` pairs. Invalid manifests and directories without manifests are silently skipped with a log warning. The `load_all()` method is the primary entry point: it checks whether plugins are enabled in config, scans the directory, and calls `load_plugin()` for each valid entry. Failed loads do not prevent other plugins from loading.

**`PluginManifest`** -- the deserialized form of `klyntbot.plugin.json`. Uses `#[serde(rename_all = "camelCase")]` to match the JSON convention. Fields:

| Field | Type | Required | Purpose |
|-------|------|----------|---------|
| `id` | `String` | Yes | Unique plugin identifier (also used as subdirectory name) |
| `name` | `String` | Yes | Human-readable display name |
| `version` | `String` | Yes | Semantic version |
| `description` | `String` | Yes | Short description |
| `author` | `String` | Yes | Plugin author |
| `min_klyntbot_version` | `Option<String>` | No | Minimum compatible Klyntbot version |
| `tools` | `Vec<PluginToolDef>` | No | Tool definitions the plugin exposes |
| `cron_jobs` | `Vec<PluginCronJob>` | No | Cron job definitions for periodic execution |
| `migrations` | `Vec<PluginMigrationDef>` | No | SQL migrations for plugin-specific tables |
| `permissions` | `Vec<PluginPermission>` | No | Capability grants the plugin requests |
| `config_schema` | `HashMap<String, PluginConfigField>` | No | User-configurable settings the plugin accepts |

**`PluginToolDef`** -- a tool declared in the manifest. Has `name`, `description`, and `parameters` (a `serde_json::Value` containing a JSON Schema object with properties, types, and required fields). The tool name doubles as the WASM function name that the host calls.

**`PluginCronJob`** -- a cron job defined by a plugin: `tool` (the tool to invoke), `schedule` (a cron expression), and `description`.

**`PluginMigrationDef`** -- an SQL migration the plugin needs: `version` (integer for ordering), `description`, and `sql` (the DDL/DML statement). Tables must be prefixed with `plugin_{plugin_id}_` to stay within the sandbox namespace.

**`PluginPermission`** -- an enum with three variants:

| Permission | Grants access to |
|------------|-----------------|
| `Network` | `http_request` host function |
| `Storage` | `db_query` and `db_execute` host functions |
| `Agent` | `agent_send_message`, `agent_ask_user`, `agent_emit_event` host functions |

Plugins requesting `Network` or `Agent` are assigned `PermissionLevel::Elevated`; storage-only or no-permission plugins get `PermissionLevel::Standard`.

**`PluginConfigField`** -- a user-configurable setting: `field_type` (string/integer/boolean), `secret` (whether the value should be masked), and `description`.

**`WasmPlugin`** -- wraps a single Extism `Plugin` instance (behind `Arc<Mutex<>>`) and implements the `Tool` trait. Each `PluginToolDef` in the manifest becomes a separate `WasmPlugin` instance sharing the same underlying Extism plugin. On `execute()`, it serializes the `args` JSON, calls the WASM function by the tool's name via `plugin.call()`, and returns the string output.

**`PluginPackage`** -- presents a loaded plugin as a `FeaturePackage` (the agent's extension point trait). Implements:

- `name()` -- returns the plugin's ID (not display name)
- `tools()` -- wraps each `PluginToolDef` in a `WasmPlugin` and returns them as `DynTool` (only if an Extism plugin is attached)
- `migrations()` -- converts manifest migrations into `FeatureMigration` structs
- `config_key()` -- returns the plugin ID (used for namespaced config lookup)
- `default_config()` -- generates zero-value defaults from the config schema
- `health_check()` -- returns `Healthy` if the WASM plugin is loaded, `Degraded` otherwise

### plugin-sdk (Guest Side)

**`klyntbot_plugin_sdk`** -- a thin wrapper crate that re-exports `extism_pdk` and provides convenience functions for common plugin operations. Plugin authors add this as a dependency and use the `prelude` module.

**`prelude`** -- re-exports everything a plugin needs: `extism_pdk::*`, `serde::{Serialize, Deserialize}`, `serde_json`, and the SDK helper functions.

Helper functions:

| Function | Purpose |
|----------|---------|
| `config_get(key)` | Read a user-configured value for this plugin |
| `http_get(url, headers)` | Make an HTTP GET request (requires `network` permission) |
| `db_query(sql)` | Execute a SELECT query on sandboxed tables (requires `storage` permission) |
| `log_info(msg)` | Log at info level via the host logger |
| `log_warn(msg)` | Log at warn level via the host logger |
| `log_error(msg)` | Log at error level via the host logger |

### Host Functions

The host exposes functions to the WASM sandbox across five namespaces, all registered under the `"klyntbot"` Extism namespace. Each function enforces permission checks before executing.

**`HostContext`** -- internal shared state passed to all host functions via Extism's `UserData` mechanism. Contains the `SqlitePool`, plugin ID, permissions list, optional bus sender, and an HTTP client.

**Database namespace:**

- `db_query(sql) -> result` -- SELECT-only queries. Enforces `Storage` permission. Validates the SQL is read-only via `is_select_only()`, which checks that the statement starts with SELECT/WITH/EXPLAIN, contains no internal semicolons (preventing injection), and has no mutation keywords (INSERT, UPDATE, DELETE, DROP, ALTER, CREATE, etc.).
- `db_execute(sql) -> result` -- write queries. Enforces `Storage` permission. Returns `{"rows_affected": N}` on success. No read-only restriction.

**Log namespace:**
- `log_debug(msg)`, `log_info(msg)`, `log_warn(msg)`, `log_error(msg)` -- emit structured log messages tagged with the plugin ID. No permission required.

**HTTP namespace:**
- `http_request(json) -> response` -- full HTTP client. Enforces `Network` permission. Accepts a JSON object with `url`, `method` (GET/POST/PUT/DELETE/PATCH), and optional `body`. Returns `{"status": N, "body": "..."}`.

**Agent namespace:**
- `agent_send_message(json) -> result` -- sends a message through the bus. Enforces `Agent` permission. Accepts `{"channel": "...", "chat_id": "...", "content": "..."}`.
- `agent_ask_user(json) -> response` -- stub for interactive prompts (returns an error until wired to the agent loop).
- `agent_emit_event(json) -> result` -- emits a custom event (currently logged, extensible for plugin-to-plugin communication).

**Tool namespace:**
- `tool_return(result)` -- plugin signals a successful result.
- `tool_error(msg)` -- plugin signals an error.

## How It Works

### Plugin Loading Flow

1. At startup, `PluginManager::load_all()` is called with the plugins directory, SQLite pool, plugin config, and optional bus sender.
2. If `config.plugins.enabled` is false, loading is skipped entirely.
3. `scan_manifests()` reads the plugins directory, looking for subdirectories with a `klyntbot.plugin.json` file. Each valid manifest is paired with the expected `plugin.wasm` path.
4. For each discovered plugin, `load_plugin()`:
   a. Reads the WASM binary from disk.
   b. Calls `host::build_host_functions()` to create the Extism host function array, injecting the pool, plugin ID, permissions, and bus sender into the `HostContext`.
   c. Converts the configured `sandbox_memory_mb` to WASM pages (64KB each) for memory limits.
   d. Creates an `extism::Manifest` from the WASM bytes with the computed memory maximum.
   e. Instantiates `extism::Plugin::new()` with the manifest, host functions, and WASI enabled.
   f. Wraps the result in a `PluginPackage` and attaches the Extism plugin instance.
5. The returned `Vec<PluginPackage>` is handed to the agent, which calls `.tools()` on each package to collect `DynTool` implementations and registers them in the `ToolRegistry`.

### Plugin Execution Flow

When the LLM generates a tool call that matches a plugin-provided tool:

1. The `ToolRegistry` looks up the tool by name and finds the `WasmPlugin` instance.
2. `WasmPlugin::execute()` serializes the arguments to a JSON string.
3. The Extism plugin is locked (`Arc<Mutex<Plugin>>`) and `plugin.call(func_name, input)` invokes the WASM function.
4. Inside the WASM sandbox, the plugin function can call host functions (database queries, HTTP requests, logging) based on its declared permissions.
5. The WASM function returns a string result, which `execute()` returns to the agent.

### Permission Enforcement

Permissions are checked at two levels:

1. **Tool registration** -- `WasmPlugin::compute_permission_level()` examines the manifest's permissions to assign `PermissionLevel::Elevated` (for network or agent access) or `Standard` (for storage-only or no permissions). Elevated tools may be subject to additional agent-level approval flows.
2. **Host function calls** -- each host function checks `ctx.permissions.contains(&PluginPermission::X)` before executing. Unauthorized calls return an error string rather than panicking.

### Database Sandboxing

Plugins access SQLite through the host's connection pool, but with restrictions:
- `db_query` only allows SELECT statements (validated by `is_select_only()` with multi-statement injection prevention).
- Tables should follow the `plugin_{plugin_id}_` naming convention (IDs with hyphens are converted to underscores).
- Plugin migrations define the tables a plugin needs, and the `FeatureMigration` system runs them during setup.

### Building a Plugin

The `klyntbot plugin new` command scaffolds a project in one of three languages:

**Rust:**
- Creates a `Cargo.toml` with `crate-type = ["cdylib"]` and `extism-pdk` + `serde` dependencies.
- Generates a `src/lib.rs` with a `#[plugin_fn]` annotated function that accepts `Json<T>` input.
- Build command: `cargo build --target wasm32-wasip1 --release`

**TypeScript:**
- Creates a `package.json` with an `extism-js` build script.
- Generates a `src/index.ts` using `Host.inputString()` / `Host.outputString()`.
- Build command: `npm run build`

**Python:**
- Generates a `src/main.py` with `@extism.plugin_fn` decorator.
- Build command: `extism-py src/main.py -o plugin.wasm`

All scaffolds include a `klyntbot.plugin.json` manifest with a single tool definition.

Using the SDK in a Rust plugin:

```rust
use klyntbot_plugin_sdk::prelude::*;

#[plugin_fn]
pub fn my_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input)?;
    // Use SDK helpers:
    log_info("Processing request");
    let data = http_get("https://api.example.com/data", &[]);
    let config_val = config_get("api_key");
    Ok(format!("Result: {}", data))
}
```

### Plugin Distribution

Plugins can be distributed through three channels:

1. **Local files** -- copy the `plugin.wasm` and `klyntbot.plugin.json` to the target machine and install with `klyntbot plugin install ./path/`.
2. **GitHub releases** -- attach `plugin.wasm` and `klyntbot.plugin.json` as release assets. Install with `klyntbot plugin install github:user/repo`.
3. **Registry** -- submit a PR to the klyntbot plugin registry repo adding the plugin's metadata to `index.json`. Users install with `klyntbot plugin install plugin-id` or `klyntbot plugin install plugin-id@version`. The registry stores download URLs pointing to GitHub release assets.

## Connections

**`plugin-runtime` depends on:**
- `common` (Layer 0) -- `KlyntbotError`, `Result`, `ToolError`
- `tools-core` -- `Tool` trait, `PermissionLevel`, `RoutingContext`, `FeaturePackage`, `DynTool`, `FeatureMigration`, `HealthStatus`
- `bus` (Layer 1) -- `OutboundMessage` for agent namespace host functions
- `config` (Layer 1) -- `PluginsConfig` schema for enabled flag, sandbox memory, registry URL
- `storage` (Layer 1.5) -- `SqlitePool` for database host functions (connection pool shared with plugins)
- `extism` -- WASM runtime (Extism host SDK)
- `reqwest` -- HTTP client injected into host context
- `sqlx` -- direct SQL execution for host database functions

**`plugin-sdk` depends on:**
- `extism-pdk` -- Extism Plugin Development Kit (re-exported entirely)
- `serde` / `serde_json` -- serialization for plugin I/O

**Depended on by:**
- `cli` (Layer 6) -- the `plugin` subcommand uses `PluginManager::scan_manifests()` and `PluginManifest` for list, install, remove, search, and update operations
- `agent` (Layer 5) -- calls `PluginManager::load_all()` during agent initialization, then integrates the resulting `PluginPackage` tools into the `ToolRegistry`
