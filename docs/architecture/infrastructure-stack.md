# Infrastructure Stack

## Runtime Environments

### Desktop Application (Tauri 2)

The primary runtime is a macOS desktop application built with Tauri 2.

**App lifecycle:**
1. CLI args parsed with `clap`. No subcommand → `run_desktop_app()`. `mcp serve --stdio` → `run_mcp_stdio()` (short-circuits before Tauri).
2. `tauri::async_runtime::block_on(app_core::init(handle))` — synchronous `AppCore` initialization before window shows.
3. Dev HTTP server starts in detached task (debug builds only).
4. Embedded MCP HTTP server starts if `config.mcp.server.enabled`.
5. `app.manage(Arc<AppCore>)` — registered as Tauri state for all commands.
6. Global shortcuts registered from config.
7. Tray icon built, countdown timer spawned.

**Windows:**

| Window | Size | Properties | Route |
|--------|------|-----------|-------|
| `main` | 1200x800 | Decorations, starts hidden | `/` |
| `launcher` | 660x580 | No decorations, transparent, always-on-top, HUD blur | `/#/launcher` |
| `tray` | 320x600 | No decorations, transparent, always-on-top, HUD blur | `/#/tray` |
| `distraction-overlay` | 340x300 | No decorations, transparent, always-on-top, focused | `/#/distraction-overlay` |
| `quick-capture` | 500x200 | No decorations, transparent, always-on-top | `/#/quick-capture` |

**macOS-specific:**
- `macOSPrivateApi: true` — required for `ActivationPolicy::Accessory` (hide from Dock) and native vibrancy
- `hudWindow` effect on popup windows
- Multi-monitor support for distraction overlay positioning

**Shutdown behavior:** Pure tray app — window close hides, doesn't quit. Actual shutdown only via quit command or `CancellationToken`.

**Auto-updater:** `tauri-plugin-updater` with GitHub Releases endpoint, minisign signature verification.

### Development Server

Debug-only (`#[cfg(debug_assertions)]`) HTTP server on `127.0.0.1:3456`.

**Routes:**

| Method | Path | Handler |
|--------|------|---------|
| `GET` | `/api/events/{sessionKey}` | SSE for per-session chat streaming |
| `GET` | `/api/cognitive/stream` | SSE for domain + pipeline events |
| `GET` | `/api/insight/events` | SSE for insight review chunks |
| `POST` | `/api/v1/ingest` | Activity ingestion (single) |
| `POST` | `/api/v1/ingest/batch` | Activity ingestion (batch) |
| `POST` | `/api/{cmd}` | Generic Tauri command mirror |

**CORS:** Only `http://localhost:1420` (Vite dev origin). Methods: GET, POST, OPTIONS.

**SSE channels:** `Arc<DashMap<String, broadcast::Sender>>` keyed by session_key. Capacity 256. Atomic entry API prevents TOCTOU race.

**Parity enforcement:** Compile-time test asserts every Tauri command is covered by dev server dispatch.

### MCP Server

Same binary, two transport modes:

| Transport | Invocation | Details |
|-----------|-----------|---------|
| **stdio** | `klyntbot-mcp serve --stdio` | JSON-RPC over stdin/stdout. Tracing to stderr. |
| **HTTP** | `klyntbot-mcp serve --http` | Axum router on configurable port (default 3100). Optional Bearer token auth. |

Also embedded in desktop app as optional HTTP server (configured via `config.mcp.server`).

**Capabilities:** `tools` + `resources`. Resources: `klyntbot://status`, `klyntbot://memory/recent`, `klyntbot://tasks/today`, `klyntbot://config/skills`.

**Session isolation:** Each connection gets a UUID session ID for routing context isolation.

## Compute Layers

### Rust Async Runtime (Tokio)

- **Runtime:** `tokio` multi-thread with `rt-multi-thread`, `macros`, `time`, `sync`, `io-util`, `net`, `signal`, `fs`, `process`
- **In desktop:** Tauri manages tokio runtime. `tauri::async_runtime::spawn` used during setup hook (before full runtime exposure). `tokio::spawn` used elsewhere.
- **CancellationToken:** Primary shutdown mechanism. `tokio_util::sync::CancellationToken` stored in `AppCore`, propagated to all background tasks.
- **File watcher bridge:** `notify_debouncer_mini` sync thread bridged to async via `tokio::task::spawn_blocking` + `mpsc::channel(256)`.

### WASM Plugin Runtime

- **Library:** Extism 1.x
- **Loading:** `std::fs::read(wasm_path)` → host functions injected → `extism::Manifest` with memory cap → `extism::Plugin::new()` with fuel metering
- **Sandbox:** Memory capped at `sandbox_memory_mb` config (converted to WASM pages). Fuel metering enabled.
- **Permission model:** `Network` or `Agent` → `PermissionLevel::Elevated`. `Storage` only → `Standard`.
- **Execution:** `Arc<Mutex<extism::Plugin>>` — serialized calls per plugin
- **Discovery:** Scanned from `{KLYNTBOT_HOME}/plugins/`. Each plugin: `klyntbot.plugin.json` manifest + `plugin.wasm` binary.
- **Disabled by default:** `config.plugins.enabled = false`

### Frontend (Vite + React)

- **Build:** Vite 6.3.5, target `esnext`, esbuild minification (production)
- **Framework:** React 19 with `babel-plugin-react-compiler` (auto-memoization)
- **Styling:** Tailwind CSS v4 via `@tailwindcss/vite` plugin, all config in CSS variables
- **Linting:** Biome 2.0 (line width 100, auto-import organization)
- **Path aliases:** `@/*` → `src/*`, `@shared/*` → `src/shared/*`, `@features/*` → `src/features/*`, `@app/*` → `src/app/*`
- **Testing:** Vitest + `@testing-library/react` + jsdom
- **UI libraries:** Radix UI, Tiptap, Recharts, Cytoscape, D3-force, Motion, cmdk, dnd-kit

## Storage Systems

### SQLite (data.db)

**Pool:** `StoragePool` wraps `sqlx::SqlitePool`. `Clone+Send+Sync` (internal Arc). No `Arc<RwLock>` wrapper needed.

**Connection:** `sqlite:{data_dir}/data.db?mode=rwc`

**PRAGMAs:**
- `journal_mode = WAL` (concurrent readers)
- `foreign_keys = ON`
- `busy_timeout = 5000` (5s wait before SQLITE_BUSY)

**Migration system:**
- **Base:** `sqlx::migrate!` runs `crates/storage/migrations/001_initial.sql` (all base tables)
- **Feature:** `StoragePool::run_feature_migrations()` checks `_feature_migrations` table, runs SQL in explicit transactions

**Table inventory (80+ tables):**

> This is a representative inventory grouped by domain. Some wildcard entries (e.g., `flashcard_*`) expand to multiple tables.

| Domain | Tables |
|--------|--------|
| Core | `areas`, `projects`, `sessions`, `session_messages`, `cron_jobs`, `entity_links` |
| Workflows | `status_workflows`, `status_labels`, `task_groups`, `custom_columns`, `custom_column_values` |
| OKR | `objectives`, `key_results` |
| Tasks | `tasks`, `task_activity`, `task_attachments`, `task_dependencies`, `task_time_entries`, `task_decompositions`, `task_estimation_history`, `task_executions`, `task_suggestions` |
| Finance | `finance_accounts`, `finance_transactions`, `finance_budgets`, `finance_portfolios`, `finance_investments`, `finance_investment_transactions`, `finance_goals`, `finance_liabilities`, `finance_exchange_rates`, `finance_allocation_targets`, `finance_net_worth_snapshots` |
| Notes | `notes`, `notebooks`, `note_versions`, `note_tags`, `note_links`, `note_entity_mentions`, `inbox_items`, `notes_fts` (FTS5), `practice_sessions` |
| Cognitive | `semantic_facts`, `semantic_facts_archive`, `episodic_memories`, `procedural_rules`, `accumulated_observations`, `failed_observations`, `domain_event_log`, `pipeline_event_log`, `coaching_strategies`, `coaching_intervention_log`, `flashcard_*`, `knowledge_atom_*`, `annotation_*`, `blackboard_entries`, `entity_*`, `relationship_*`, `pending_memories`, `review_sessions`, `squads`, `squad_members`, `insight_personas`, `insight_persona_pins`, `persona_accuracy`, `book_tree_nodes`, `entity_tree_links`, `atom_extraction_cache`, `deck_preferences`, `knowledge_topics`, `fsrs_parameters`, `review_log`, `insight_reviews`, `insight_progress_snapshots` |
| Mirror | `mirror_routing_snapshots`, `mirror_trend_narratives`, `mirror_snippets`, `mirror_meta_rules`, `mirror_brain_versions`, `mirror_trial_previews` |
| Activity Log | `unified_activity_log`, `work_contexts`, `work_resources`, `resource_edges`, `work_context_resources`, `work_context_actions`, `context_merges`, `inference_state` |
| Launcher | `launcher_frequencies`, `clipboard_history` |
| Analytics | `strategy_records`, `learning_outcomes`, `usage_records`, `decision_log`, `tool_usage`, `interaction_log`, `enrichment_feedback` |
| Infrastructure | `_feature_migrations`, `agent_tasks`, `circuit_breaker_state`, `calendar_sync_state`, `calendar_event_cache`, `project_sources`, `session_context`, `learning_state` |

**Repos aggregate:** `Repos::from_pool()` instantiates 23 typed repository handles sharing one `SqlitePool` clone (tasks, sessions, areas, projects, objectives, key_results, outcomes, strategies, usage, cron, learning_state, decision_log, session_context, finance, interaction_log, status_workflows, task_groups, custom_columns, entity_links, project_sources, tool_usage, plus raw pool access).

**Analytics retention cleanup:** Parallel deletes via `try_join!`: strategy_records (90d), learning_outcomes (30d), interaction_log (60d), tool_usage (90d), enrichment_feedback (90d).

### LanceDB (Vector Store)

**Location:** `{data_dir}/lance/`

**Connection:** `lancedb::connect(path).execute()` wrapped in `Arc<lancedb::Connection>`.

**Tables (10 total):**

| Table | Schema | Purpose |
|-------|--------|---------|
| `todo_embeddings` | id, vector(384), model, updated_at | Task embeddings |
| `task_embeddings` | id, vector(384), model, updated_at | Task embeddings |
| `note_embeddings` | id, vector(384), model, updated_at | Note content |
| `conv_embeddings` | id, vector(384), session_key, role, content_preview, full_content, created_at | Conversation recall |
| `cognitive_fact_embeddings` | id, vector(384), domain, text, importance, stability, confidence, updated_at | Semantic facts |
| `activity_embeddings` | id, vector(384), source, work_context_id, timestamp, updated_at | Activity log |
| `work_context_embeddings` | id, vector(384), updated_at | Work context clusters |
| `flashcard_embeddings` | id, vector(384), card_id, side, timestamp | Flashcard content |
| `insight_embeddings` | id, vector(384), updated_at | Note insights |
| `entity_embeddings` | id, vector(384), name, entity_type, description, updated_at | Knowledge graph |

**Vector dimension:** All tables use `FixedSizeList<Float32, 384>` — consistent 384-dim embedding space.

**ANN indexes:** Background `tokio::spawn` task creates indexes after startup (requires 256+ rows to train). Non-fatal on failure.

## Build & Deployment

### Build Pipeline

| Target | Command | Notes |
|--------|---------|-------|
| All crates | `cargo build --workspace` | MSRV 1.75 |
| Desktop app | `cargo tauri dev` | Starts Vite separately |
| Production | `cargo tauri build` | Runs `bun run build` as `beforeBuildCommand` |
| MCP binary | `cargo build -p klyntbot-mcp` | Separate binary |
| Frontend | `cd desktop-ui && bun run build` | Vite production build |
| Tests | `cargo nextest run --workspace` | Parallel test runner |
| Lint | `cargo clippy --workspace --all-targets --all-features` | Zero warnings policy |

### Feature Flags

**Cargo features:**
- `channels::email` (default ON) — gates IMAP/SMTP dependencies
- `plugin-runtime::plugin-integration` — WASM integration tests

**Runtime flags (config-driven):**
- `config.plugins.enabled` — WASM plugin system
- `config.mcp.enabled` — MCP client connections
- `config.mcp.server.enabled` — embedded MCP HTTP server
- `config.cognitive.*` — memory extraction, reflection, FSRS, coaching toggles
- `config.packs.*` — feature pack toggles

### Platform-Specific Code

**`platform-macos` crate:** Uses `objc2`, `objc2-app-kit`, `core-graphics`, `core-foundation` for:
- Running app detection (`NSWorkspace`, `NSRunningApplication`)
- Active browser tab detection
- Keyboard/mouse input monitoring (CoreGraphics events)
- Clipboard access (`NSPasteboard`)
- Window management

### Launcher (feature-launcher)

Spotlight-style search engine with 15+ search sources:
- App index, file search, browser history, Git repos, brew packages
- SSH hosts, contacts, bookmarks, calculator, URL navigation
- Script runner, content grep, running apps, system commands, system preferences

**Ranking:** Frequency-based result ordering via `launcher_frequencies` SQLite table.

**Clipboard monitor:** Tracks clipboard history, stored in `clipboard_history` table.

**Window management:** Uses macOS Accessibility API for frontmost window detection.

**Conditional compilation:**
- `#[cfg(debug_assertions)]` — dev server, verbose logging
- `#[cfg(target_os = "macos")]` — platform-macos bindings
- `#[windows_subsystem = "windows"]` — suppress console window in release

## Testing Infrastructure

### Test Binary Types

The root crate has 4 test binaries in `tests/`:

| Binary | Location | Purpose |
|--------|----------|---------|
| `integration/` | `tests/integration/` | Cross-crate tests via facade (e.g., agent + tools + session) |
| `e2e/` | `tests/e2e/` | Full agent loop + reminders |
| `unit/` | `tests/unit/` | Config, providers, utilities |
| `plugins.rs` | `tests/plugins.rs` | WASM plugin tests (needs `--features plugin-integration` + pre-built plugin) |

### Test Infrastructure

- **Ephemeral SQLite:** All tests use `StoragePool::connect_in_memory()` — creates `sqlite::memory:` pools with same PRAGMAs as production (except WAL). No external DB needed.
- **Shared fixtures:** `tests/common/` provides mock providers, mock embedding engines, and conversation recall mocks.
- **Inline tests:** Feature crates use `#[cfg(test)] mod tests` inline.
- **Dev server parity:** Each Tauri command module exports `pub const DEV_COMMANDS: &[&str]`. The `dev_server_covers_all_tauri_commands` compile-time test asserts every registered command is covered.
- **Doctests:** `cargo test --workspace --doc` (nextest doesn't support doctests).
