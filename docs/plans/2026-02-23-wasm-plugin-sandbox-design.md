# WASM Plugin Sandbox Design

> Date: 2026-02-23
> Feature: Runtime-loadable WASM plugins for personal skills (Notion, Obsidian, Vietnamese banks, health trackers)
> Status: Approved

---

## Overview

Add a WASM plugin sandbox to klyntbot that lets users install personal skills without recompilation. Plugins are `.wasm` binaries authored in any WASM-targeting language (Rust, TypeScript, Python, Go), distributed via a central registry (`plugins.klyntbot.io`) or GitHub, and sandboxed with explicit permission declarations.

**Key properties:**
- Multi-language authoring: Rust, TypeScript (Javy), Python (py2wasm), Go (TinyGo)
- Full host capabilities: new LLM tools + sandboxed SQLite storage + agent callbacks
- Request-response execution + cron hooks for background work
- Registry distribution with GitHub fallback and local file install
- Explicit permission model: `network`, `storage`, `agent` declared per plugin

**Runtime:** Extism (built on Wasmtime) — chosen for production-ready multi-language PDKs, clean JSON-in/JSON-out boundary, and automatic memory management across the WASM boundary.

---

## Architecture

### New Crates

```
Layer 2.5:  crates/plugin-runtime   ← Extism runtime, host ABI, WasmPlugin wrapper
Published:  crates/plugin-sdk       ← Helper crate for plugin authors (on crates.io)
```

`plugin-runtime` sits between `storage` (Layer 1.5) and `tools` (Layer 3). It depends on `extism`, `common`, `config`, and `storage`. It produces `WasmPlugin` — a struct implementing both `FeaturePackage` and `Tool`, allowing plugin tools to slot into the existing `ToolRegistry` with zero changes to the agent loop.

`plugin-sdk` is published to crates.io for plugin authors. It provides `#[plugin_tool]` macro and typed host-function bindings so plugin authors write idiomatic Rust — no `unsafe`, no manual ptr/len arithmetic.

### Data Flow

```
User message
  → AgentLoop
  → LLM calls "notion_search"
  → ToolRegistry::execute("notion_search", args, ctx)
  → WasmPlugin::execute()              ← identical interface to any built-in tool
    → Extism: invoke WASM function with JSON args
    → Plugin sandbox (calls host fns: db, http, agent)
    → Returns JSON string
  → Back to LLM context
```

### Workspace Layout (updated)

```
Layer 0:    common
Layer 1:    config, bus
Layer 1.5:  storage
Layer 2:    providers, session, scheduling, calendar, context_engine
Layer 2.5:  plugin-runtime              ← NEW
Layer 3:    tools, tools-core, tools-core-macros, feature-todo, feature-finance
Layer 4:    channels, heartbeat
Layer 5:    agent
Layer 6:    cli
Layer 7:    klyntbot
```

---

## Host ABI

Five namespaces of host functions exposed to WASM plugins via Extism `host_fn!`.

### `tool` — execution boundary
```
tool_return(json_ptr, json_len)      ← plugin signals its result string
tool_error(msg_ptr, msg_len)         ← plugin signals an error
```

### `db` — sandboxed storage
Each plugin operates in its own table namespace (`plugin_{id}_*`). Cross-plugin access is rejected at the host level before SQL execution.
```
db_query(sql_ptr, sql_len)   → json_result     ← SELECT only
db_execute(sql_ptr, sql_len) → i32             ← INSERT / UPDATE / DELETE
```
Migrations are declared in the plugin manifest and run at install time via the existing `FeatureMigration` mechanism — no new plumbing required.

### `agent` — callbacks into the agent
```
agent_send_message(channel_ptr, chat_id_ptr, msg_ptr) → i32
agent_ask_user(question_ptr)                          → response_json
agent_emit_event(event_name_ptr, payload_ptr)         ← triggers cron-registered handlers
```

### `log` — structured logging
```
log_debug(msg_ptr, msg_len)
log_info(msg_ptr, msg_len)
log_warn(msg_ptr, msg_len)
log_error(msg_ptr, msg_len)
```

### `http` — outbound network (requires `network` permission)
```
http_request(request_json_ptr) → response_json
```
Requests from plugins without the `network` permission fail at the host before touching the network.

### Permission model

Permissions are additive and explicit. A plugin that only returns tool results declares no permissions. The installer always sees the full permission list before accepting.

| Permission | Unlocks |
|---|---|
| `storage` | `db_query`, `db_execute` |
| `network` | `http_request` |
| `agent` | `agent_send_message`, `agent_ask_user`, `agent_emit_event` |

---

## Plugin Manifest (`klyntbot.plugin.json`)

```json
{
  "id": "notion-connector",
  "name": "Notion Connector",
  "version": "1.2.0",
  "description": "Search, read, and create Notion pages from klyntbot",
  "author": "jayden/notion-klyntbot",
  "min_klyntbot_version": "0.4.0",
  "tools": [
    {
      "name": "notion_search",
      "description": "Search Notion pages by query",
      "parameters": {
        "type": "object",
        "properties": { "query": { "type": "string" } },
        "required": ["query"]
      }
    },
    {
      "name": "notion_create_page",
      "description": "Create a new Notion page",
      "parameters": {
        "type": "object",
        "properties": {
          "title": { "type": "string" },
          "content": { "type": "string" }
        },
        "required": ["title"]
      }
    }
  ],
  "cron_jobs": [
    {
      "tool": "notion_sync",
      "schedule": "0 * * * *",
      "description": "Hourly Notion sync"
    }
  ],
  "migrations": [
    {
      "version": 1,
      "description": "Create notion cache table",
      "sql": "CREATE TABLE plugin_notion_connector_cache (id TEXT PRIMARY KEY, title TEXT, last_synced INTEGER)"
    }
  ],
  "permissions": ["network", "storage"],
  "config_schema": {
    "api_key": { "type": "string", "secret": true, "description": "Notion API integration token" }
  }
}
```

---

## Distribution

### Three install paths

All three converge on the same install flow: fetch `.wasm` + manifest → verify → show permissions → confirm → write to `~/.klyntbot/plugins/{id}/` → run migrations → register tools.

```bash
# 1. From registry
klyntbot plugin install notion-connector
klyntbot plugin install notion-connector@1.2.0

# 2. From GitHub release
klyntbot plugin install github:jayden/notion-klyntbot

# 3. Local file (development / testing)
klyntbot plugin install ./notion-connector.wasm
```

### Registry (`plugins.klyntbot.io`)

A static JSON index hosted on GitHub Pages — no server, no auth, no database. Community submissions via pull request to the registry repo.

```json
{
  "plugins": [
    {
      "id": "notion-connector",
      "latest": "1.2.0",
      "description": "Search and create Notion pages",
      "author": "jayden",
      "download_url": "https://github.com/jayden/notion-klyntbot/releases/download/v1.2.0/plugin.wasm",
      "manifest_url": "https://github.com/jayden/notion-klyntbot/releases/download/v1.2.0/klyntbot.plugin.json"
    }
  ]
}
```

### Plugin directory layout

```
~/.klyntbot/plugins/
  notion-connector/
    plugin.wasm
    klyntbot.plugin.json
    config.json          ← user-set API keys and preferences (secrets wrapped)
  vietcombank/
    plugin.wasm
    klyntbot.plugin.json
    config.json
```

### CLI commands

```bash
klyntbot plugin install <id|github:user/repo|./path>
klyntbot plugin list
klyntbot plugin remove <id>
klyntbot plugin update [<id>]
klyntbot plugin search <query>
klyntbot plugin new <name> --lang rust|typescript|python    ← scaffold a new plugin
klyntbot plugin publish                                     ← opens PR on registry repo
```

---

## Plugin Development Experience

### Rust (via `klyntbot-plugin-sdk`)

```rust
use klyntbot_plugin_sdk::prelude::*;

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
}

#[plugin_tool]
fn notion_search(args: SearchArgs) -> PluginResult {
    let key = config_get("api_key")?;
    let results = http_get(
        &format!("https://api.notion.com/v1/search?query={}", args.query),
        &[("Authorization", &format!("Bearer {}", key))],
    )?;
    PluginResult::ok(results)
}

#[plugin_tool]
fn notion_sync(_args: NoArgs) -> PluginResult {
    let rows = db_query("SELECT id, last_synced FROM plugin_notion_connector_cache")?;
    // sync logic
    PluginResult::ok("Synced 12 pages")
}
```

The `#[plugin_tool]` macro generates the WASM export signature and JSON deserialization boilerplate. No `unsafe`, no manual memory management.

### TypeScript (via Extism JS PDK + Javy)

```typescript
import { Config, Http, Memory } from "@extism/js-pdk";

export function notion_search(): i32 {
  const args = JSON.parse(Memory.fromPtr(input()).readString());
  const key = Config.get("api_key");
  const resp = Http.request({
    url: `https://api.notion.com/v1/search?query=${args.query}`,
    headers: { Authorization: `Bearer ${key}` },
  });
  output(Memory.fromString(resp.body).ptr);
  return 0;
}
```

Compiled with `javy`. Same `klyntbot.plugin.json` manifest regardless of source language.

### Python (via Extism Python PDK)

```python
from klyntbot_sdk import tool, http_get, config_get

@tool
def notion_search(query: str) -> str:
    key = config_get("api_key")
    return http_get(
        f"https://api.notion.com/v1/search?query={query}",
        headers={"Authorization": f"Bearer {key}"}
    )
```

---

## Plugin Lifecycle

### Startup sequence

```
klyntbot serve
  → PluginManager::load_all("~/.klyntbot/plugins/")
    → for each plugin dir:
        read manifest → load .wasm into Extism (compiled module cached in memory)
        run pending migrations (FeatureMigration mechanism)
        wrap each tool as WasmPlugin → register into ToolRegistry
        register cron jobs via CronHandler → stored in CronRepo
    → log: "Loaded 3 plugins: notion-connector, vietcombank, health-tracker"
```

Failed plugin loads are logged as warnings — they never crash the agent. The `PluginManager` is constructed in `AgentLoopBuilder` alongside other feature packages.

### Cron integration

Cron jobs declared in the manifest are registered exactly like `CronTool`-created jobs — stored in `CronRepo`, firing as agent turns. No special plugin-cron pathway:

```
CronRepo: { schedule: "0 * * * *", payload: agent_turn { message: "call notion_sync" } }
  → agent turn fires at the scheduled time
  → LLM sees context + skill guidance: invoke notion_sync
  → LLM calls tool: notion_sync {}
  → WasmPlugin::execute() → WASM sandbox → returns result
```

---

## Configuration

### Config schema (`~/.klyntbot/config.json`)

```json
{
  "plugins": {
    "enabled": true,
    "registry_url": "https://plugins.klyntbot.io/index.json",
    "sandbox_memory_mb": 64,
    "allow_network_by_default": false
  }
}
```

### Per-plugin config (`~/.klyntbot/plugins/{id}/config.json`)

Stores user-supplied values matching the plugin's `config_schema`. Fields marked `"secret": true` are wrapped in `Secret<String>` on the host side and never appear in logs.

---

## Testing Strategy

### Unit tests (inline `#[cfg(test)]` in `plugin-runtime`)
- `PluginManifest` parsing: valid manifests, missing required fields, unknown permissions
- `HostFunctions`: mock Extism context, verify `db_query` rejects cross-plugin table access
- Permission enforcement: `http_request` without `network` permission returns error before network touch

### Integration tests (feature-gated `--features plugin-integration`)
A minimal test plugin shipped in the repo at `tests/fixtures/hello_plugin/` — a tiny Rust WASM exposing one tool returning `"hello from wasm"`. Tests:
- Full install → load → execute round-trip
- Cron job registration persists in `CronRepo`
- Plugin migration runs on install, skipped on reload
- `agent_send_message` routes to mock channel

### Plugin SDK tests (in `plugin-sdk` crate)
- `#[plugin_tool]` macro generates correct WASM exports
- `PluginResult::ok` / `PluginResult::err` serialize correctly
- `config_get` / `db_query` call the correct Extism host function symbols

---

## Summary of Files Changed

| File / Crate | Change |
|---|---|
| `crates/plugin-runtime/` | New crate: Extism runtime, `WasmPlugin`, `PluginManager`, `HostFunctions` |
| `crates/plugin-sdk/` | New crate (published): `#[plugin_tool]`, typed host bindings |
| `crates/config/src/schema/` | Add `PluginsConfig` to root config |
| `crates/agent/src/agent_loop/builder.rs` | Load `PluginManager`, register plugin tools + cron jobs |
| `crates/cli/src/commands/` | Add `plugin` subcommand (install, list, remove, update, search, new, publish) |
| `Cargo.toml` (workspace) | Add `plugin-runtime`, `plugin-sdk`, `extism` dependency |
| `tests/fixtures/hello_plugin/` | Minimal test plugin for integration tests |

---

## Out of Scope (this iteration)

- Plugin-to-plugin communication (no shared state between plugins)
- Hot reload without restart (plugins load at startup only)
- WASM Component Model (adopt when Python/JS tooling matures — Extism plans to support it)
- Plugin signing / code verification (future: GPG signatures on release artifacts)
- Full background daemon (Option C lifecycle — upgrade path via `PluginEventSource` at Layer 4)
