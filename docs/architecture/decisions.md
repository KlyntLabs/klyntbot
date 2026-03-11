# Architecture Decision Records

## ADR-001: 9-Layer Crate Architecture

**Status:** Accepted

**Context:** Klyntbot is a large Rust workspace with 26 crates spanning diverse functionality: common types, configuration, storage, LLM providers, tools, chat platform integrations, agent runtime, MCP protocol, and a Tauri desktop app. Without a disciplined dependency structure, circular dependencies between crates would quickly emerge (e.g., tools needing agent context, channels needing tool definitions, agent needing everything).

**Decision:** Organize the workspace into 9 strict layers (L0 through L8) where dependencies flow strictly upward. Each crate belongs to exactly one layer and may only depend on crates in lower layers:

- L0: `common` -- foundational types (`KlyntbotError`, `MessageRole`, `ChannelName`, `ChatId`, `SessionKey`)
- L1: `config`, `bus`, `tools-core`, `tools-core-macros` -- configuration, message bus, tool trait definitions
- L2: `storage`, `domain` -- SQLite pool, migrations, repository structs, domain types
- L3: `providers`, `session`, `scheduling`, `context_engine` -- LLM clients, session persistence, cron, token budgets
- L4: `tools`, `feature-*`, `plugin-runtime` -- tool implementations, feature packages, WASM plugins
- L5: `channels`, `agent`, `cognitive` -- platform integrations, agent runtime, cognitive memory
- L6: `mcp` -- MCP server/client
- L7: `app-core`, `desktop-shared`, `desktop` -- application core, Tauri adapter
- L8: `klyntbot` -- re-export facade

**Consequences:**
- Circular dependencies are structurally impossible. Compilation fails immediately if a lower layer tries to import a higher layer.
- Cross-layer communication requires dependency inversion: handler traits (e.g., `SpawnHandler`, `CronHandler`, `ProgressHandler`) are defined in lower layers and implemented in higher layers, injected via `Arc<dyn Trait>`.
- Adding new functionality requires identifying the correct layer. Misplacing a crate creates cascading import issues.
- Build parallelism improves because lower layers compile independently.

---

## ADR-002: SQLite + LanceDB for Storage

**Status:** Accepted

**Context:** Klyntbot needs persistent storage for relational data (tasks, sessions, projects, config state) and vector embeddings (semantic search for conversations, todos, cognitive facts). Options considered: PostgreSQL, embedded key-value stores (sled, RocksDB), cloud vector databases, and local embedded databases.

**Decision:** Use SQLite for relational data and LanceDB for vector embeddings. Both are embedded, file-based databases requiring no external process.

- `StoragePool` wraps `SqlitePool` (from `sqlx`) with `Clone+Send+Sync` semantics. No `Arc<RwLock>` wrapper needed.
- Relational data stored in `{data_dir}/data.db`.
- Vector data stored in `{data_dir}/lance/`.
- Data directory defaults to `~/.klyntbot`.
- Migrations run automatically on `StoragePool::connect()` (but not `from_existing()`).
- Feature crates contribute additional migrations via `FeatureMigration`.

**Consequences:**
- Zero infrastructure requirements. No database server to install, configure, or maintain.
- Single-user only. SQLite's write locking is acceptable for a personal agent but would not scale to concurrent multi-user access.
- Backup is a file copy. Users can back up `~/.klyntbot/` to preserve all state.
- LanceDB provides IVF-PQ vector indexing for tables exceeding a row threshold, giving sub-linear search performance.
- Testing is trivial: `connect_in_memory()` creates ephemeral databases with full migration support.

---

## ADR-003: AppCore + Thin Adapter Pattern

**Status:** Accepted

**Context:** Klyntbot needs to serve the same business logic through multiple transports: the Tauri desktop app (via IPC commands), a dev HTTP server (for browser-only development), and potentially future transports. Duplicating handler logic across transports would create maintenance burden and behavioral inconsistencies.

**Decision:** Extract all shared business logic into the `app-core` crate. Transport-specific crates (`desktop`, `dev-api`) are thin adapters that delegate to `AppCore` methods.

- `AppCore` holds references to storage, agent runtime, and configuration.
- Desktop `commands/*.rs` files are Tauri command handlers that extract parameters, call `AppCore`, and emit UI update events via `emit_updates(&app, &updates)`.
- The dev HTTP server (`dev_server.rs`) calls the same `AppCore` methods but discards entity update events (no Tauri event bus available).
- Mutations return update payloads that the adapter layer decides how to deliver.

**Consequences:**
- Business logic is tested once, in `app-core`, independent of transport.
- Adding a new transport (e.g., REST API, gRPC) requires only a thin adapter.
- The desktop crate contains minimal logic: parameter extraction, error mapping, and event emission.
- Entity update events (for reactive UI) are handled differently per transport, which is the adapter's responsibility.

---

## ADR-004: Derive-Based Tool System

**Status:** Accepted

**Context:** The agent needs 20+ tools with consistent JSON Schema generation, parameter validation, permission declaration, metadata, and execution. Hand-writing the `Tool` trait implementation for each tool involves significant boilerplate: `name()`, `description()`, `parameters()`, `permission_level()`, `metadata()`, and the JSON Schema construction.

**Decision:** Implement a derive macro system (`#[derive(Tool)]`, `#[derive(ToolParams)]`) in `tools-core-macros` that generates the `Tool` trait bridge from annotations:

```rust
#[derive(ToolParams)]
pub struct ReadFileParams {
    #[param(required)]
    pub path: String,
}

#[derive(Tool)]
#[tool(name = "read_file", description = "...", params = "ReadFileParams",
       permission = "read_only", category = "FileSystem", tags = "file,read", cost = "Free")]
pub struct ReadFileTool { ... }
```

Developers implement only `ToolExecute::execute()` with typed parameters. The macro generates `Tool::name()`, `Tool::description()`, `Tool::parameters()` (JSON Schema from `ToolParams`), `Tool::permission_level()`, `Tool::metadata()`, and the untyped `Tool::execute()` bridge that parses `serde_json::Value` into the typed params struct.

Multi-action tools use `#[tool_actions]` and `#[derive(ActionParams)]` for tools with multiple verbs (e.g., todo create/update/delete).

**Consequences:**
- Adding a new tool is ~20 lines of code: a params struct and an execute method.
- JSON Schema is always in sync with the actual parameter types.
- Permission levels and metadata are declarative, not buried in method implementations.
- The macro complexity is contained in `tools-core-macros`; tool authors do not need to understand it.

---

## ADR-005: Dual Execution Modes (Direct / Reactive)

**Status:** Accepted

**Context:** Not all user messages require tool use. Simple questions ("What time is it in Tokyo?") can be answered with a single LLM call, while complex requests ("Create a project plan for the Q2 launch") require multiple tool calls in a reasoning loop. Running every message through the full ReAct loop wastes tokens and adds latency.

**Decision:** Implement two execution modes in the agent runtime:

- **Direct mode:** Single LLM call with no tools. Used for simple questions, greetings, and factual queries. The intent analyzer classifies messages and routes accordingly.
- **Reactive mode:** ReAct (Reasoning + Acting) loop with tool calls. The agent iterates: reason about the next step, call a tool, observe the result, repeat. Terminates when the agent produces a final response or hits `max_iterations`.

The `IntentAnalyzer` in the intent pipeline determines which mode to use based on the message content and available context.

**Consequences:**
- Simple queries are 2-5x cheaper (one LLM call vs. multiple).
- Latency for simple queries drops from seconds to sub-second.
- The intent analyzer adds a classification step, but this is lightweight compared to unnecessary tool-calling loops.
- The reactive loop includes cost tracking via `CostTracker` to monitor token usage across iterations.
- Synthesizes a final response at `max_iterations` even if the loop has not naturally concluded.

---

## ADR-006: Feature Packages

**Status:** Accepted

**Context:** Klyntbot has multiple feature domains (todos, finance, notes, productivity, coaching) that each need their own tools, database migrations, configuration, and health checks. Monolithically registering all of these in the agent crate would create a massive, tightly-coupled module.

**Decision:** Define the `FeaturePackage` trait in `tools-core` (Layer 1) and implement it in dedicated `feature-*` crates (Layer 4):

```rust
trait FeaturePackage {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<DynTool>;
    fn migrations(&self) -> Vec<FeatureMigration>;
    fn health(&self) -> HealthStatus;
}
```

Each feature package is self-contained: it provides its own tools, contributes its own database migrations, declares its own configuration section, and reports its own health status. The agent runtime discovers and loads feature packages at startup.

**Consequences:**
- Features are isolated. Adding or removing a feature does not affect others.
- Feature crates can be developed and tested independently.
- Database migrations are modular: each feature contributes its own migration set.
- The `packs` config section controls which features are active at runtime.
- Feature health can be checked individually for diagnostics.

---

## ADR-007: Message Bus Topology

**Status:** Accepted

**Context:** Chat channels (Telegram, Discord, Slack, etc.) need to send messages to the agent and receive responses. Events like learning updates need to be broadcast to multiple subscribers. The communication pattern differs: channel-agent messaging is point-to-point (one producer, one consumer), while events are fan-out (one producer, many consumers).

**Decision:** Use two distinct Tokio channel types in the `bus` crate:

- **`mpsc` (multi-producer, single-consumer)** for channel-agent transport. `MessageQueue` uses `mpsc::channel` for both inbound (channels to agent) and outbound (agent to channels) message flows. Multiple channels can send inbound messages; the agent loop is the single consumer. Multiple tools can queue outbound messages; each channel adapter consumes its own.
- **`broadcast` (multi-producer, multi-consumer)** for event fan-out. `LearningEventBus` uses `broadcast::channel` so that multiple subscribers (context engine, coaching system, analytics) can all receive every learning event.

**Consequences:**
- The message queue is backpressure-aware: `mpsc::channel` with a configurable buffer size naturally slows producers when the consumer falls behind.
- Broadcast channels handle subscriber lag via Tokio's built-in lagged-message handling.
- Channel adapters are decoupled from the agent: they only interact through the message bus, not through direct function calls.
- Adding a new channel requires implementing the adapter and connecting it to the existing `MessageQueue`.

---

## ADR-008: Agent Profiles as Compiled Markdown

**Status:** Accepted

**Context:** The agent supports multiple personalities/specializations (general, task, finance, automation, communication). Each profile needs a system prompt, skill definitions, tool access configuration, and MCP server allowlists. Options: store profiles in the database, load from config files at runtime, or compile into the binary.

**Decision:** Define agent profiles as Markdown files with YAML frontmatter in the `agents/` directory. Each profile has an `AGENT.md` file and an optional `skills/` folder. These are compiled into the binary via `include_str!`.

The YAML frontmatter declares structured configuration (model preferences, MCP tool access, temperature settings), while the Markdown body serves as the system prompt. Skills are additional Markdown files that provide domain-specific instructions.

**Consequences:**
- Profiles are version-controlled alongside the code.
- No runtime file loading or missing-file errors.
- Profile changes require recompilation, which is acceptable for built-in agents.
- The Markdown format is human-readable and editable by non-developers.
- YAML frontmatter provides structured configuration without a separate config file.
- MCP access control is declarative: `mcp_tools: ["*"]` or `mcp_tools: ["google-calendar"]`.

---

## ADR-009: Secret\<T\> Without Encryption at Rest

**Status:** Accepted

**Context:** API keys and tokens need protection from accidental exposure. Options range from full encryption at rest (keychain integration, encrypted config files) to simple redaction in logs.

**Decision:** Implement `Secret<T>` as a newtype wrapper that redacts `Debug` and `Display` output but stores the inner value in plaintext. The config file (`~/.klyntbot/config.json`) contains secrets in plain JSON. Access to the inner value requires calling `.expose()`.

**Consequences:**
- Secrets never appear in log output, error messages, or debug prints.
- Code review can easily find all secret access points by searching for `.expose()`.
- The config file contains plaintext secrets, which is a security limitation.
- File permissions (`chmod 600`) are the primary defense for the config file.
- Environment variable overrides (`KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=...`) allow avoiding plaintext config entirely.
- The implementation is simple: no external dependencies, no key management, no encryption/decryption code.
- Future keychain integration can be added without changing the `Secret<T>` API -- only the serialization/deserialization layer would change.

---

## ADR-010: Custom useQuery Instead of React Query

**Status:** Accepted

**Context:** The desktop-ui needs data fetching with caching, deduplication, and stale-while-revalidate semantics for Tauri IPC commands. React Query (TanStack Query) is the standard library for this, but it adds a significant dependency and requires a provider wrapper.

**Decision:** Implement a custom `useQuery` hook (`desktop-ui/src/shared/hooks/useQuery.ts`) that provides:

- **SWR caching:** Serves cached data immediately, then refetches if stale (default stale time: 30 seconds).
- **Request deduplication:** In-flight promises are reused when multiple components request the same data.
- **Module-level cache:** A `Map<string, CacheEntry>` shared across all hook instances, keyed by `cmd:JSON.stringify(args)`.
- **Skip fetching:** Pass `null` for args to disable the query (conditional fetching).
- **Cache invalidation:** `invalidateQueries(cmdPrefix)` clears all cache entries matching a command prefix.
- **Error handling:** Failed fetches preserve stale data and surface errors via `ApiError`.

The companion `useMutation` hook handles write operations with automatic cache invalidation.

**Consequences:**
- No external dependency for data fetching. The entire implementation is ~80 lines.
- The API is simpler than React Query: `useQuery(cmd, args, fallback, staleTime)` returns `{ data, loading, error, refetch }`.
- Cache behavior is predictable and debuggable (module-level Map).
- Missing features compared to React Query: no garbage collection of stale entries, no retry logic, no infinite queries, no optimistic updates. These are not needed for the current UI complexity.
- Tight integration with Tauri IPC: the hook calls `ipc<T>(cmd, args)` directly rather than wrapping `fetch()`.
