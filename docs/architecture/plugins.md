# WASM Plugin System

## Overview

Klyntbot supports extending its capabilities through WebAssembly (WASM) plugins built on the [Extism](https://extism.org/) runtime. Plugins are self-contained `.wasm` binaries that live in `~/.klyntbot/plugins/` and are discovered, loaded, and sandboxed at startup. Each plugin declares its tools, permissions, migrations, and configuration schema in a `klyntbot.plugin.json` manifest.

The plugin system is implemented in the `plugin-runtime` crate (layer L4) and consists of five modules:

- **`manager`** -- Discovery and loading (`PluginManager`)
- **`manifest`** -- Manifest parsing (`PluginManifest`)
- **`wasm_plugin`** -- Tool trait adapter (`WasmPlugin`)
- **`plugin_package`** -- FeaturePackage integration (`PluginPackage`)
- **`host`** -- Host functions exposed to WASM guests

## Plugin Structure

A plugin is a directory inside `~/.klyntbot/plugins/` containing two files:

```
~/.klyntbot/plugins/
  my-plugin/
    klyntbot.plugin.json   # Manifest (required)
    plugin.wasm            # Compiled WASM binary (required)
```

### Manifest Format (`klyntbot.plugin.json`)

The manifest uses camelCase JSON and declares everything about the plugin:

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
        "properties": {
          "query": { "type": "string" }
        },
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

**Required fields:** `id`, `name`, `version`, `description`, `author`.

**Optional fields:** `minKlyntbotVersion`, `tools`, `cronJobs`, `migrations`, `permissions`, `configSchema`. All default to empty.

### Manifest Types

| Type | Fields | Description |
|------|--------|-------------|
| `PluginToolDef` | `name`, `description`, `parameters` (JSON Schema) | Defines a tool the plugin exposes |
| `PluginCronJob` | `tool`, `schedule`, `description` | Registers a recurring cron job |
| `PluginMigrationDef` | `version` (i64), `description`, `sql` | SQL migration for plugin storage |
| `PluginConfigField` | `type`, `secret` (bool), `description` | Declares a configuration field |

## WasmPlugin -- Tool Trait Implementation

Each tool declared in the manifest becomes a `WasmPlugin` instance that implements the `Tool` trait from `tools-core`. When the agent calls the tool:

1. The input arguments are serialized to a JSON string.
2. The Extism plugin's function (matched by `tool_def.name`) is called with the JSON string.
3. The WASM function's string output is returned as the tool result.

The Extism `Plugin` is wrapped in `Arc<Mutex<extism::Plugin>>` and shared across all tools from the same plugin, ensuring thread-safe access.

### Permission Level Mapping

The `WasmPlugin` computes its `PermissionLevel` from the manifest's declared permissions:

| Manifest Permissions | Tool PermissionLevel |
|---------------------|---------------------|
| None, or `storage` only | `Standard` |
| `network` or `agent` (or both) | `Elevated` |

## Permission Model

Plugins declare the host capabilities they need via the `permissions` array in the manifest. Three permissions are available:

| Permission | Grants Access To | Host Functions |
|-----------|-----------------|----------------|
| `storage` | SQLite database (sandboxed to `plugin_{id}_*` tables) | `db_query` (SELECT only), `db_execute` (write) |
| `network` | Outbound HTTP requests | `http_request` |
| `agent` | Message bus and agent interaction | `agent_send_message`, `agent_ask_user`, `agent_emit_event` |

Every host function checks the plugin's permissions before executing. Unauthorized calls return an error string rather than executing the operation.

### Host Function Namespaces

All host functions are registered under the `klyntbot` namespace with five groups:

- **db** -- `db_query` (read-only SELECT with SQL injection protection), `db_execute` (write)
- **log** -- `log_debug`, `log_info`, `log_warn`, `log_error`
- **http** -- `http_request` (JSON input: `{url, method, body}`, JSON output: `{status, body}`)
- **agent** -- `agent_send_message`, `agent_ask_user`, `agent_emit_event`
- **tool** -- `tool_return`, `tool_error`

Logging functions require no permissions; they are always available to all plugins.

### SQL Safety

The `db_query` function enforces read-only access through `is_select_only()`, which:

- Accepts only statements starting with `SELECT`, `WITH`, or `EXPLAIN`
- Rejects multi-statement strings (blocks `;`-based injection)
- Rejects statements containing mutation keywords (`INSERT`, `UPDATE`, `DELETE`, `DROP`, `ALTER`, `CREATE`, etc.)

Plugin tables are namespaced with the convention `plugin_{id}_*` (hyphens in the ID are replaced with underscores).

## Plugin Loading

### `PluginManager::load_all()` Flow

1. **Check enabled** -- If `PluginsConfig.enabled` is `false`, return an empty manager immediately.
2. **Check directory** -- If `~/.klyntbot/plugins/` does not exist, return an empty manager.
3. **Scan manifests** -- Iterate subdirectories looking for `klyntbot.plugin.json`. Parse each manifest with `PluginManifest::from_file()`. Invalid manifests are logged as warnings and skipped.
4. **Load each plugin:**
   a. Read the `plugin.wasm` binary from disk.
   b. Build host functions via `host::build_host_functions()`, passing the SQLite pool, plugin ID, permissions, and bus sender.
   c. Create an Extism manifest with memory limits from `PluginsConfig.sandbox_memory_mb` (converted to WASM pages at 64KB each).
   d. Instantiate the Extism `Plugin` with host functions and WASI enabled.
   e. Create a `PluginPackage` from the manifest and attach the live plugin.
5. **Return** -- The `PluginManager` holds all successfully loaded `PluginPackage` instances. Failed loads are logged but do not block other plugins.

### PluginPackage as FeaturePackage

`PluginPackage` implements the `FeaturePackage` trait, which lets it integrate into the agent's tool registry like any built-in feature crate:

- **`name()`** -- Returns the plugin's `id` (not display name)
- **`tools()`** -- Creates a `WasmPlugin` (as `DynTool`) for each tool in the manifest. Returns empty if no WASM binary is attached.
- **`migrations()`** -- Maps manifest migrations to `FeatureMigration` structs
- **`config_key()`** -- Returns the plugin `id` for config namespacing
- **`default_config()`** -- Generates default values from `configSchema` (empty string for strings, 0 for numbers, false for booleans)
- **`health_check()`** -- Returns `Healthy` if the WASM plugin is loaded, `Degraded` otherwise

## Plugin SDK

The `plugin-sdk` crate (excluded from the workspace) provides a Rust development kit for building klyntbot plugins. Plugins are standard Extism guest modules and can also be written in any language that compiles to WASM and supports Extism's PDK.

For Rust plugins, the `extism-pdk` crate provides the `#[plugin_fn]` macro for declaring exported functions.

## Creating a Plugin

### Step 1: Create the Project

```bash
mkdir my-plugin && cd my-plugin
cargo init --lib
```

Add dependencies to `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1"
serde_json = "1"
```

### Step 2: Write the Plugin Code

Each tool declared in the manifest must have a corresponding exported function. The function receives a JSON string of arguments and returns a string result.

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn hello_tool(input: String) -> FnResult<String> {
    let args: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let name = args["name"].as_str().unwrap_or("world");
    Ok(format!("hello from wasm, {}!", name))
}
```

### Step 3: Create the Manifest

Create `klyntbot.plugin.json` alongside the source:

```json
{
  "id": "hello-plugin",
  "name": "Hello Plugin",
  "version": "0.1.0",
  "description": "A simple greeting plugin",
  "author": "your-name",
  "tools": [
    {
      "name": "hello_tool",
      "description": "Say hello",
      "parameters": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "Name to greet" }
        }
      }
    }
  ],
  "permissions": []
}
```

### Step 4: Build

```bash
rustup target add wasm32-wasip1
cargo build --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/my_plugin.wasm ./plugin.wasm
```

### Step 5: Install

Copy the plugin directory to the plugins folder:

```bash
mkdir -p ~/.klyntbot/plugins/hello-plugin
cp klyntbot.plugin.json plugin.wasm ~/.klyntbot/plugins/hello-plugin/
```

The plugin will be discovered and loaded on the next klyntbot startup (assuming `plugins.enabled` is `true` in the configuration).

### Key Constraints

- **Function names must match** -- The exported WASM function name must exactly match the `name` field in the tool definition.
- **JSON in, string out** -- All tool functions receive JSON string input and must return a string.
- **Table naming** -- SQL migrations must use the `plugin_{id}_` prefix (hyphens replaced with underscores) for all table names.
- **Memory limits** -- Plugin memory is capped by `PluginsConfig.sandbox_memory_mb`.
- **No circular dependencies** -- Plugins cannot import other plugins' tools.
