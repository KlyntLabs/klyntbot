# Skill System, MCP, and Plugin Architecture

Klyntbot uses three complementary extension systems to shape agent behavior and integrate external capabilities:

- **Skills** -- domain-specific instruction sets (Markdown + YAML frontmatter) that configure what the agent knows and which tools it may use.
- **MCP (Model Context Protocol)** -- bidirectional protocol for connecting to external tool servers and exposing klyntbot's own tools to external AI clients.
- **WASM Plugins** -- sandboxed WebAssembly modules that add tools, cron jobs, and migrations via the Extism runtime.

```
                        +------------------+
                        |   Agent Runtime  |
                        |  (crates/agent)  |
                        +--------+---------+
                                 |
              +------------------+------------------+
              |                  |                  |
     +--------v--------+ +------v------+ +---------v---------+
     |  Skill System   | |    MCP      | |   Plugin Runtime  |
     | (skill-system)  | | (crates/mcp)| | (plugin-runtime)  |
     +-----------------+ +------+------+ +---------+---------+
              |                  |                  |
     YAML frontmatter    JSON-RPC over     WASM via Extism
     in .md files        stdio / HTTP      from ~/.klyntbot/plugins/
```

All three systems register their capabilities into a shared `ToolRegistry` (defined in `tools-core`), making them uniformly available to the agent's execution loop.

---

## 1. Skill Types and Scopes

### Types

| Type | Purpose | Example |
|------|---------|---------|
| `Skill` | Domain workflow instructions for a specific capability | `finance-management`, `notebook` |
| `Orchestrator` | Delegates to other skills, coordinates multi-skill workflows | `task-management` |
| `Persona` | User personality overlay with expertise areas, tone, and cognitive bias | `deep-analyst` |

### Scopes

| Scope | Location | Trust |
|-------|----------|-------|
| `BuiltIn` | Compiled into the binary via `include_str!` | Trusted |
| `User` | `~/.klyntbot/skills/` | Trusted |
| `Project` | Workspace-local skills directory | Untrusted |

Source: `crates/skill-system/src/types.rs`

---

## 2. Skill Package Structure

Every loaded skill is represented as a `SkillPackage`:

```rust
struct SkillPackage {
    name: String,
    description: String,
    skill_type: SkillType,         // Skill | Orchestrator | Persona
    scope: SkillScope,             // BuiltIn | User | Project
    location: PathBuf,
    body: String,                  // Full markdown body (instructions)
    metadata: SkillMetadata,
    resources: Vec<String>,        // Bundled files: scripts/, references/, assets/
    loaded_at: SystemTime,
    trusted: bool,
    summary: String,               // One-line summary for progressive loading
}

struct SkillMetadata {
    license: Option<String>,
    compatibility: Option<String>,
    custom: HashMap<String, Value>,
    klyntbot: Option<KlyntbotMeta>,
}

struct KlyntbotMeta {
    skill_type: SkillType,
    tools: Option<Vec<String>>,       // Tool allowlist (None = all tools)
    mcp_tools: Vec<String>,           // MCP server allowlist (["*"] = all)
    can_delegate_to: Vec<String>,     // Orchestrator delegation targets
    max_iterations: Option<u32>,      // ReAct loop cap (default: 10)
    always_skills: Vec<String>,       // Always-loaded reference files
    invokes: Vec<String>,             // Chaining targets
    triggers: Vec<String>,            // Routing boost phrases
    summary: Option<String>,          // Explicit one-line summary
}
```

---

## 3. Frontmatter Format

Skills are Markdown files with YAML frontmatter. The parser (`crates/skill-system/src/parser.rs`) supports lenient YAML -- it auto-fixes unquoted colons, warns on name violations without failing, and validates names per the Agent Skills spec (lowercase alphanumeric + hyphens, max 64 chars).

```yaml
---
name: task-management
description: >
  Create, organize, and track tasks using OKR+PARA.
  Use when the user mentions todos or tasks.
metadata:
  klyntbot:
    type: orchestrator
    tools: [tasks, project, area]
    mcp_tools: [google-calendar]
    can_delegate_to: [finance-management]
    max_iterations: 12
    always_skills: [todo, daily-planner]
    triggers:
      - "add task"
      - "create todo"
    summary: Handles task CRUD and project management.
---

You are the task management specialist.

## Behavior
- Create tasks efficiently
```

Two file layouts are supported:
- **Flat file:** `skills/task-management.md`
- **Subdirectory:** `skills/task-management/SKILL.md` (with optional `references/`, `scripts/`, `assets/` folders)

Persona skills use `PERSONA.md` with additional frontmatter fields:

```yaml
metadata:
  expertise_areas: [DCF valuation, ratio analysis]
  analysis_frameworks: [bottom-up, comparative]
  questioning_style: interrogative
  tone: rigorous
  cognitive_bias: precision
  references: [dcf-guide]
```

Source: `crates/skill-system/src/persona.rs`

---

## 4. SkillStore

`SkillStore` manages the in-memory skill catalog.

**Loading:** `SkillStore::load(skills_dir)` creates the directory if missing, installs built-in defaults if empty, then loads all `.md` files and subdirectories containing `SKILL.md`. Each file is parsed via `split_frontmatter()` to extract YAML + body.

**Hot-reload:** `reload()` re-reads everything from disk. The store is wrapped in `Arc<RwLock<SkillStore>>` for concurrent access.

**Default skills** are compiled into the binary via `include_str!` in `crates/skill-system/src/store.rs` and installed on first run:

| Skill | Type | Description |
|-------|------|-------------|
| `task-management` | Orchestrator | Task CRUD, OKR+PARA, project/area management |
| `finance-management` | Orchestrator | Transaction tracking, budgeting, FIRE planning |
| `automation` | Skill | Cron job scheduling and management |
| `notebook` | Skill | Note creation and organization |
| `learning` | Skill | Flashcard generation and spaced repetition |

Full skill content with references is also embedded via `compiled_skill_defaults()` in `crates/skill-system/src/defaults.rs`, used by the Reforge system for strategy file seeding.

**Context injection:** `SkillListingSource` (implements `ContextSource`, priority 40) formats a compact skill listing for the system prompt. Each entry shows name + description + whenToUse, truncated to 250 chars.

**Soul file:** `SoulContextSource` (priority 50, highest) loads `KLYNTBOT.md` from the data directory as always-present personality context. Installed with defaults on first run, supports hot-reload.

---

## 5. Progressive Skill Loading

To minimize token usage, skills are loaded progressively based on activation state:

```
Message arrives
    |
    v
+-------------------+
| Skill Listing     |  <-- Always in system prompt (compact: name + description)
| (SkillListingSource)
+-------------------+
    |
    v  (routing selects a skill)
+-------------------+
| Orchestrator?     |
+---+----------+----+
    |          |
   Yes         No
    |          |
    v          v
Full body    Summary only
injected     injected
(deduped     (agent calls skill_reference
per session)  tool for full instructions)
```

- **Orchestrator skills:** Full body injected on first activation, deduplicated per session to avoid re-injection.
- **Non-orchestrator skills:** Only the `summary` field is injected. The agent calls the `skill_reference` tool to load full instructions when it determines they are needed.
- **Always-loaded references:** Controlled by the `always_skills` field. Single-token reference names always load; multi-token names require a keyword match against the current message.

---

## 6. Tool and MCP Authorization

Each skill controls which tools and MCP servers the agent may access during its activation.

### Tool allowlist

```rust
// SkillPackage::allowed_tool_names()
// Returns None -> all tools allowed (tools field omitted)
// Returns Some(set) -> explicit allowlist (ask_user always included)
```

When `tools` is `None` (omitted from frontmatter), the skill has unrestricted tool access. When `tools` is `Some(vec![])` (empty list), all tools are denied except `ask_user` which is always injected.

### MCP server allowlist

```rust
// SkillPackage::allows_mcp_server(server_name)
// mcp_tools: ["*"] -> all MCP servers allowed
// mcp_tools: ["google-calendar"] -> only that server
// mcp_tools: [] (or omitted) -> no MCP tools
```

The `mcp_tools` field controls which external MCP servers the skill can access. This prevents a note-taking skill from accidentally invoking calendar or finance tools.

---

## 7. MCP Client Integration

The MCP client (`crates/mcp/src/client/`) connects to external MCP servers and adapts their tools for use in klyntbot's agent loop.

### McpManager

`McpManager::connect_all()` performs parallel connection to all enabled servers in `config.json`, with per-server startup timeouts. Each connection:

1. Spawns the server process (stdio) or opens an HTTP connection
2. Discovers available tools via `tools/list`
3. Filters tools by per-server allowlist/denylist
4. Wraps each tool as an `McpTool` for registration in `ToolRegistry`

Startup progress is reported via `McpStartupEvent` for UI observability.

### McpTool

Each discovered MCP tool becomes an `McpTool` instance implementing the `Tool` trait:

- **Naming:** `mcp_{sanitized_server}_{sanitized_tool}` (max 64 chars, hash suffix if truncated)
- **Extraction:** `extract_server_name("mcp_linear_list_issues")` returns `"linear"`
- **Permission level:** Always `Elevated` (makes network calls)
- **Retry:** 3 attempts with exponential backoff (500ms, 1s, 2s) for transient errors
- **Timeout:** Per-server configurable via `tool_timeout_sec`

Source: `crates/mcp/src/client/tool_adapter.rs`, `crates/mcp/src/client/sanitize.rs`

### Circuit Breaker

`McpCircuitBreaker` tracks failures per server. After `threshold` (default: 3) failures, the circuit opens and all calls to that server are blocked for `cooldown` (default: 60s). Auto-resets when cooldown expires or on first success.

### Health Check Loop

`McpManager::start_health_check()` spawns a background task that:

1. **Polls every 30s** for servers whose circuit breaker cooldown has expired, attempting reconnection
2. **Listens for `notifications/tools/list_changed`** signals from MCP servers, triggering immediate re-discovery
3. Re-registers tools in `ToolRegistry` on successful reconnect

### KlyntbotClientHandler

Handles server-initiated requests:

- **Sampling:** Delegates LLM completion requests to a `SamplingDelegate` trait (implemented where the LLM provider is available)
- **Roots listing:** Returns the klyntbot data directory
- **Notifications:** Logs server messages, triggers tool re-discovery on `tool_list_changed`

### Transports

| Transport | Use Case |
|-----------|----------|
| `Stdio` (child process) | Local MCP servers (most common) |
| `StreamableHttpClient` | Remote MCP servers over HTTP |

OAuth tokens can be injected via `McpServerDef.oauth` configuration, passed as environment variables to stdio processes or as Bearer auth headers for HTTP.

---

## 8. MCP Server

Klyntbot also acts as an MCP server, exposing its internal tools to external AI clients (Claude Code, Cursor, etc.).

### Architecture

```
External Client (Claude Code)
    |
    v  (stdio or HTTP)
+-------------------+
| ToolRegistryBridge|  <-- Translates MCP protocol to internal Tool::execute()
+-------------------+
    |
    v
+-------------------+
|  ToolRegistry     |  <-- Same registry used by the agent loop
+-------------------+
    |
    v
+-------------------+
|  AgentBridge      |  <-- "agent" tool: delegates natural language
|                   |      to the full AI pipeline
+-------------------+
```

### ToolRegistryBridge

`ToolRegistryBridge` (in `crates/klyntbot-server/src/bridge/registry.rs`) bridges the internal `ToolRegistry` to the MCP protocol:

- **Whitelisting:** Only tools in `exposed_tools` are discoverable and callable
- **Runtime update:** `update_whitelist()` allows changing the exposed set without restart
- **Tool schema translation:** Internal `Tool::parameters()` JSON Schema is served directly as MCP `inputSchema`

### Default Exposed Tools

Configured in `default_exposed_tools()` at `crates/config/src/schema/mcp.rs`:

```
tasks, project, area, notes, memory, okr, finance,
productivity, work_context, agent, annotate, learning,
cron, mirror, temporal
```

Users can override via `config.json` -> `mcp.server.exposedTools`.

### Security

`crates/mcp/src/server/security.rs` provides:

- **Path traversal protection:** `validate_path()` canonicalizes paths and ensures they stay within the allowed base directory
- **Input sanitization:** `sanitize_input()` strips control characters and enforces a 50,000 character limit

---

## 9. WASM Plugin Runtime

The `plugin-runtime` crate (`crates/plugin-runtime/`) provides sandboxed extensibility via WebAssembly.

### Plugin Structure

```
~/.klyntbot/plugins/
  notion-connector/
    klyntbot.plugin.json    <-- Manifest
    plugin.wasm             <-- Compiled WASM module
```

### PluginManifest

Defined in `crates/plugin-runtime/src/manifest.rs`:

```json
{
  "id": "notion-connector",
  "name": "Notion Connector",
  "version": "1.2.0",
  "description": "Search and create Notion pages",
  "author": "jayden",
  "minKlyntbotVersion": "0.4.0",
  "tools": [{
    "name": "notion_search",
    "description": "Search Notion pages",
    "parameters": {"type": "object", "properties": {"query": {"type": "string"}}}
  }],
  "cronJobs": [{
    "tool": "notion_sync",
    "schedule": "0 * * * *",
    "description": "Hourly Notion sync"
  }],
  "migrations": [{
    "version": 1,
    "description": "Create cache table",
    "sql": "CREATE TABLE plugin_notion_connector_cache (id TEXT PRIMARY KEY)"
  }],
  "permissions": ["network", "storage"],
  "configSchema": {
    "api_key": {"type": "string", "secret": true, "description": "Notion API key"}
  }
}
```

### PluginManager

`PluginManager::load_all()` scans `~/.klyntbot/plugins/` for subdirectories containing `klyntbot.plugin.json`. For each valid manifest:

1. Reads the `.wasm` binary
2. Builds host functions with permission-gated access
3. Creates an Extism plugin with configurable memory limits (`sandbox_memory_mb`)
4. Wraps everything in a `PluginPackage`

Plugins are disabled by default via `PluginsConfig.enabled`.

### PluginPackage

`PluginPackage` implements `FeaturePackage`, the same trait used by built-in feature crates. This means plugin tools, migrations, and config are registered identically to native features:

- `tools()` -- returns `WasmPlugin` instances for each declared tool
- `migrations()` -- returns `FeatureMigration` entries from the manifest
- `config_key()` -- uses the plugin's `id`
- `health_check()` -- reports `Healthy` if the WASM module is loaded, `Degraded` otherwise

### WasmPlugin Execution

Each tool call invokes a WASM function by name, passing JSON arguments as a string:

```rust
let output = plugin.call::<&str, &str>(func_name, &input)?;
```

Permission level is computed from the manifest's declared permissions:
- `Network` or `Agent` permission -> `Elevated`
- `Storage` only or none -> `Standard`

### Host Functions

Plugins access klyntbot capabilities through host functions in the `klyntbot` namespace. All functions enforce permission checks before executing.

| Namespace | Function | Permission | Purpose |
|-----------|----------|------------|---------|
| `db` | `db_query` | Storage | SELECT-only queries on sandboxed tables |
| `db` | `db_execute` | Storage | Write queries on sandboxed tables |
| `log` | `log_debug/info/warn/error` | None | Structured logging |
| `http` | `http_request` | Network | HTTP requests (GET/POST/PUT/DELETE/PATCH) |
| `agent` | `agent_send_message` | Agent | Send messages via the message bus |
| `agent` | `agent_ask_user` | Agent | Ask user a question (stub) |
| `agent` | `agent_emit_event` | Agent | Emit custom events |
| `tool` | `tool_return` | None | Signal successful result |
| `tool` | `tool_error` | None | Signal an error |

### Database Sandboxing

Database access is double-gated:

1. **Query type:** `db_query` only allows SELECT/WITH/EXPLAIN statements. Multi-statement injection (semicolons) is rejected.
2. **Table namespace:** All table references must match the pattern `plugin_{id}_*` (with hyphens replaced by underscores). Access to system tables or other plugins' tables is denied.

---

## Related Documentation

- [Agent Runtime](agent-runtime.md) -- execution pipeline, budget-bounded loops, context compression
- [Context Engine](context-engine.md) -- context assembly, token budgets, source priorities
- [Core Infrastructure](core-infrastructure.md) -- storage, config, error handling, message bus
