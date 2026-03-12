# Klyntbot — Engineering Backlog
> Last updated: 2026-03-12
> Format: `[ID] Priority | Crate(s) | Short description`
> Priorities: 🔴 High | 🟡 Medium | 🟢 Low

---

## Section A — Issues Found During Refactor Analysis

These items were identified during the 2026-03-12 structural refactor analysis. They represent functional gaps, technical debt, incomplete features, and design issues that should be resolved AFTER the structural refactor is complete.

---

### A-001 🔴 High | `agent/intent_pipeline` — Short-Message Heuristic Preempts Domain Patterns

**Summary:** Short messages (< 20 chars or < 4 words) without generic action keywords are routed to Direct mode (single LLM call, no tools), even when they contain domain-specific verbs like "plan my day", "list tasks", "my todos", or "show budget".

**Root Cause:** In `crates/agent/src/intent_pipeline/analysis.rs`, rule evaluation order is:
1. Greeting check
2. ⚠️ Short-message check (< 4 words → Direct) ← fires before domain checks
3. Domain pattern matching
4. Action keyword check (only covers generic code/dev keywords, not domain verbs)

**Impact:** All features affected — tasks, finance, notes, automations. Agent responds with a description of what it _would_ do rather than executing tools.

**Proposed Fix:**
- Restructure rule priority: (1) Greetings, (2) Multi-agent delegation, (3) ALL domain patterns + domain verb check, (4) Short-message fallback, (5) Direct questions, (6) Complex workflows
- Expand `has_any_action_keyword()` to include domain verbs: `plan`, `decompose`, `execute`, `list`, `track`, `log`, `budget`, `remind`, `schedule`, `search`, `find`, `show`, `create`, `add`, `complete`, `note`, `annotate`
- Add tests for short domain commands: `"plan my day"`, `"list tasks"`, `"my todos"`, `"show budget"`

**References:** `crates/agent/src/intent_pipeline/analysis.rs` | `BACKLOG.json:BACK-001`

---

### A-002 🟡 Medium | `feature-tasks`, `agent/enrichment` — Due Dates Systematically Underutilized

**Summary:** `due_date` field exists on `Task` but tasks are routinely created without deadlines. The enrichment engine does not aggressively infer due dates from natural language (e.g., "finish report by Friday", "monthly tax deadline"). The UI doesn't surface urgency visually. Scoring assigns urgency=1 (minimum) to most tasks.

**Impact:** Urgency scoring is near-useless. No overdue reminders. Day-planning quality degrades because priorities are flat.

**Proposed Fix:**
1. `agent/enrichment/engine.rs` — Enhance LLM prompt to be more aggressive about inferring due dates from message context and extracting explicit date mentions
2. `desktop-ui` — Color-code due dates: red = overdue, yellow = due today/tomorrow
3. Skill update — Add "When is this due?" follow-up question in the task creation skill
4. `agent/reminders.rs` — Add overdue task reminder scan on daily schedule

**References:** `crates/feature-tasks/src/types.rs`, `crates/agent/src/enrichment/engine.rs` | `BACKLOG.json:BACK-002`

---

### A-003 🔴 High | `channels` — Feishu/Lark Channel: Config Present, Implementation Missing

**Summary:** `FeishuConfig` exists in `crates/config/src/schema/channels.rs` with full field definitions (`app_id`, `app_secret`, `encrypt_key`, `verification_token`, `allow_from`). No corresponding `Channel` trait implementation exists anywhere in `crates/channels/src/`. The `ChannelManager` silently ignores enabled Feishu config.

**Impact:** Users who set `channels.feishu.enabled = true` get no error and no channel. Silent misconfiguration.

**Proposed Fix:**
- Short-term: Add validation in `ChannelManager::from_config()` — if `feishu.enabled = true`, log a clear `tracing::error!("Feishu channel is not yet implemented")` and return an error
- Long-term: Implement `FeishuChannel` using Feishu Open Platform webhook/event subscription API
- Add stub in `channels/src/stubs/feishu.rs` that `panic!`s with a clear message

**References:** `crates/config/src/schema/channels.rs`, `crates/channels/src/manager.rs`

---

### A-004 🔴 High | `channels` — DingTalk Channel: Config Present, Implementation Missing

**Summary:** Same as A-003 for DingTalk. `DingTalkConfig` (`client_id`, `client_secret`, `allow_from`) exists in config but no `Channel` implementation exists.

**Proposed Fix:** Same pattern as A-003 — add validation warning + stub, then implement using DingTalk Chatbot API.

**References:** `crates/config/src/schema/channels.rs`, `crates/channels/src/manager.rs`

---

### A-005 🟡 Medium | `channels` — Mochat Channel: Config Present, Implementation Missing

**Summary:** `MochatConfig` (`base_url`, `socket_url`, `claw_token`, `agent_user_id`, `sessions`, `panels`) exists in config. Mochat is a WhatsApp-style business messaging platform. No implementation exists.

**Proposed Fix:** Same pattern — add validation warning + stub.

**References:** `crates/config/src/schema/channels.rs`

---

### A-006 🟡 Medium | `app-core/handlers/work_context.rs` — Merge Tracking Not Implemented

**Summary:** Lines 343-344 contain two `TODO` comments:
```rust
// TODO: track merges when merge logging is added
// TODO: expose from inference loop
```
The work context inference loop can detect context merges (e.g., multiple browser windows resolved to one work context) but does not record them. The data exists for reporting but is silently discarded.

**Impact:** Work context history may have gaps. Merge analytics are unavailable.

**Proposed Fix:**
- Add `context_merges` table to `activity-log` migrations
- Record merge events in `work_context_repo.rs` when inference detects a merge
- Expose merge count in `WorkContextSummary` response type

**References:** `crates/app-core/src/handlers/work_context.rs:343-344`

---

### A-007 🟢 Low | `app-core/handlers/work_context.rs` — "Compare With Yesterday" Not Implemented

**Summary:** Lines 485-486:
```rust
// TODO: compare with yesterday
// TODO: pull from cognitive semantic facts
```
The daily work summary handler has placeholders for cross-day comparison (productivity trends) and pulling context from cognitive memory (relevant facts about the user's ongoing work).

**Impact:** Daily summaries are less actionable. No trend visibility.

**Proposed Fix:**
- Add yesterday comparison in `WorkContextSummaryHandler::summarize_day()`
- Query `cognitive` semantic facts for work-related context before generating summary

**References:** `crates/app-core/src/handlers/work_context.rs:485-486`

---

### A-008 🟡 Medium | `agent/handlers/forecast.rs` — Trend Computation Not Implemented

**Summary:** Line 307:
```rust
// TODO: compute trend from recent vs older accuracy
```
The forecast handler has a placeholder for computing accuracy trends (is the agent getting better at estimating task durations?). The raw data is available in `strategy_records` and `enrichment_feedback` tables.

**Impact:** The confidence evaluator cannot improve over time based on forecast accuracy. The adaptive learning loop is incomplete.

**Proposed Fix:**
- Query last 30 days vs last 90 days of forecast accuracy records
- Compute trend direction and magnitude
- Feed into `ConfidenceEvaluator` threshold adjustment

**References:** `crates/agent/src/handlers/forecast.rs:307`

---

### A-009 🔴 High | `feature-todo` / `feature-tasks` — Deprecation Not Marked, No Migration Path

**Summary:** `feature-todo` is the legacy task crate (25 action types, `ActionRepo`, `ActionPatch`, etc.). `feature-tasks` is the new agentic replacement. CLAUDE.md states: *"Legacy `feature-todo` crate can be removed once `feature-tasks` fully replaces it."*

Currently:
- No `#[deprecated]` markers anywhere in `feature-todo`
- No migration tooling to move data from `actions` table to `tasks` table
- Both crates are compiled into every build (double maintenance, double binary size)
- No documentation on which crate to use for new features

**Impact:** New contributors may accidentally extend `feature-todo` instead of `feature-tasks`. Data migration will be more complex the longer it's deferred.

**Proposed Fix:**
1. Add `#[deprecated(note = "Use feature-tasks. Removal planned post-1.0.")]` to `feature-todo/src/lib.rs`
2. Design a one-time migration script: `actions` → `tasks` table with status/priority mapping
3. Update CLAUDE.md with explicit deprecation timeline
4. After migration: remove `feature-todo` from `Cargo.toml` workspace members

**References:** `crates/feature-todo/src/lib.rs`, CLAUDE.md

---

### A-010 🟡 Medium | `feature-tasks` — Phase 3 Implementation Pending

**Summary:** Feature-tasks Phase 2 is complete (task execution, decomposition, day planning handlers). Phase 3 design spec exists in `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md` and covers:
- Proactive task suggestions (BACK-001 prerequisite)
- Workload forecasting improvements
- Cognitive integration (pull task context from semantic memory)
- Recurring task generation improvements with FSRS decay

**Impact:** Task management lacks proactivity. Agent doesn't suggest tasks based on calendar/context.

**References:** `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md`

---

### A-011 🟢 Low | `storage` — `task_repo.rs` (2,450 LOC) — Largest Single File

**Summary:** `crates/storage/src/repos/task_repo.rs` is the largest Rust file in the workspace (2,450 lines). It contains CRUD, filtering, complex queries, analytics, embedding operations, and search — all in one file. This makes it hard for AI agents and humans to navigate.

**Impact:** Structural debt, not a functional issue. Navigation and onboarding difficulty.

**Proposed Fix:** Split as described in refactor Phase 3 into `crud.rs`, `query.rs`, `analytics.rs` submodules within `repos/tasks/task_repo/`.

---

### A-012 🟢 Low | `agent/agent_loop/builder.rs` (1,199 LOC) — Monolithic Builder

**Summary:** The `AgentLoopBuilder` has 50+ setter methods in a single file. While the builder pattern is correct, the file is unwieldy. Related concerns (channel setup, cognitive wiring, feature registration) are interleaved.

**Proposed Fix:** Split into four builder extension files by concern as described in refactor Phase 8b.

---

### A-013 🟢 Low | `desktop-shared/commands.rs` (1,516 LOC) — All IPC Types in One File

**Summary:** All Tauri IPC command types (CreateTask, UpdateTask, GetFinanceReport, etc.) are in one flat file. No domain grouping. Adding new commands from any team requires editing the same file, increasing merge conflicts.

**Proposed Fix:** Split by domain as described in refactor Phase 12 (`commands/tasks.rs`, `commands/finance.rs`, etc.).

---

### A-014 🟡 Medium | `cognitive` — No Semantic Fact Expiry/Pruning

**Summary:** The cognitive memory system accumulates semantic facts over time but has no automatic pruning. FSRS decay reduces salience scores, but facts with very low salience are never deleted. Over months of use, the `semantic_fact_embeddings` LanceDB table and `semantic_facts` SQLite table will grow unboundedly.

**Impact:** Memory retrieval will slow down. Disk usage grows indefinitely.

**Proposed Fix:**
- Add `prune_expired_facts(threshold: f32)` to `SemanticFactRepo`
- Schedule weekly prune in `cognitive/background/` pipeline
- Default threshold: salience < 0.05 AND last_accessed > 180 days

**References:** `crates/cognitive/src/repos/semantic_fact.rs`

---

### A-015 🟡 Medium | `providers` — Pricing Tables Not Up to Date

**Summary:** `agent/src/output/cost_tracker.rs` contains hardcoded pricing tables for 12 providers. Provider pricing changes frequently (Anthropic, OpenAI, Groq). The tables were last updated at codebase creation.

**Impact:** Monthly budget alerts may be inaccurate. Cost estimates drifted from reality.

**Proposed Fix:**
- Add `updated_at` comment to each pricing table entry
- Consider moving pricing to `config.json` (`providers.anthropic.input_cost_per_1m_tokens`) so users can update without recompiling
- Or add a periodic pricing refresh from a public pricing API

**References:** `crates/agent/src/output/cost_tracker.rs`

---

### A-016 🟢 Low | `channels` — WebSocket Channel Not Listed in `ChannelName` Enum

**Summary:** `crates/common/src/types.rs` defines `ChannelName` enum (`Telegram`, `Discord`, `Slack`, `Email`, `Cli`, `Desktop`). The WebSocket channel (`ws_manager.rs`) is used by the Desktop UI but has no corresponding `ChannelName::WebSocket` variant. This makes channel identification in logs and routing ambiguous.

**Proposed Fix:** Add `ChannelName::WebSocket` variant and update all match statements.

**References:** `crates/common/src/types.rs`, `crates/channels/src/ws_manager.rs`

---

### A-017 🟢 Low | `config` — No Config Schema Validation at Startup

**Summary:** Config is deserialized from JSON with `serde`, which handles type errors, but there is no semantic validation (e.g., "if `providers.anthropic.api_key` is set, `agents.defaults.provider = 'anthropic'` must also work"). Invalid combinations are silently ignored until runtime failure.

**Impact:** Confusing startup errors. Users can set `enabled: true` on a channel without the required API keys.

**Proposed Fix:**
- Add `Config::validate() -> Result<(), Vec<ConfigError>>` in `config/loader/`
- Check: required keys present if channel enabled, model name valid for selected provider, MCP server commands resolvable (via `which`), budget > 0 if set

**References:** `crates/config/src/loader.rs`

---

### A-018 🟢 Low | `agent/content_registry` — Skills Not Hot-Reloaded

**Summary:** Agent skills (Markdown files in `agents/{name}/skills/`) are loaded at startup via `include_str!()` macros. Changes to skill files require a full rebuild + restart. In development, this slows the iterate-on-prompts workflow.

**Impact:** Developer experience only (no production impact).

**Proposed Fix:**
- Add optional runtime skill loading via filesystem read (controlled by a `config.development.hot_reload_skills = true` flag)
- `content_registry/loader.rs` — add `load_from_filesystem(agents_dir: &Path)` fallback

**References:** `crates/agent/src/content_registry/loader.rs`, `crates/agent/src/skill_loader.rs`

---

### A-019 🟡 Medium | `providers` — Circuit Breaker State is In-Memory Only

**Summary:** `ProviderManager` implements a circuit breaker (open/half-open/closed states) but the state is in-memory. On app restart, all circuit breakers reset to closed even if a provider was failing repeatedly.

**Impact:** After restart, the agent will hammer a failing provider again until the circuit reopens, wasting tokens and budget.

**Proposed Fix:**
- Persist circuit breaker state in SQLite (`circuit_breaker_state` table)
- Include `open_until: Option<DateTime<Utc>>` so breakers survive restarts

**References:** `crates/providers/src/manager.rs`

---

### A-020 🟢 Low | `scheduling` — No Missed-Run Catchup Logic

**Summary:** `CronService` evaluates schedules on startup and fires any jobs whose `next_run` is in the past. However, if the app was offline for days, ALL missed jobs fire immediately on startup, potentially flooding the system with concurrent runs.

**Impact:** After extended downtime, queued jobs (reminders, reports, memory consolidation) all execute simultaneously, causing spike load and potentially duplicate notifications.

**Proposed Fix:**
- Add `max_catchup_age: Duration` to `CronService` configuration (default: 1 hour)
- Skip jobs whose missed time is older than `max_catchup_age` (log as skipped, don't execute)
- For jobs that should always run (e.g., memory consolidation), add an `always_catchup: bool` flag

**References:** `crates/scheduling/src/service/mod.rs`

---

### A-021 🟡 Medium | `feature-productivity` — macOS-Only Activity Tracker

**Summary:** `crates/feature-productivity/src/tracker/` contains macOS-specific code for capturing active application windows and titles. No Linux or Windows implementation exists. The crate is not feature-gated for macOS.

**Impact:** The crate compiles on all platforms but the tracker silently produces no data on non-macOS systems. No warning or error is surfaced.

**Proposed Fix:**
- Add `#[cfg(target_os = "macos")]` to macOS tracker code
- Add `#[cfg(not(target_os = "macos"))]` stub that logs a single `tracing::warn!("Activity tracking is only supported on macOS")` on startup
- Consider adding `linux` tracker as a future enhancement (using `xdotool` or `wnck`)

**References:** `crates/feature-productivity/src/tracker/`

---

### A-022 🟢 Low | `mcp` — MCP Server Does Not Support Tool Discovery by External Agents

**Summary:** `McpServerRunner` exposes klyntbot tools to external MCP clients. The current implementation exposes all registered tools unconditionally. There is no filtering by calling agent identity or capability scope.

**Impact:** Any MCP client that connects to klyntbot's MCP server can call any tool, including destructive ones (filesystem write, spawn shell command). This is a security gap if the server is exposed beyond localhost.

**Proposed Fix:**
- Add `allowed_tools: Vec<String>` to `McpServerConfig`
- Filter tool list in `McpServerRunner` based on allowlist (similar to agent profile `tools` field)
- Default: localhost-only binding (already implemented via `127.0.0.1`)

**References:** `crates/mcp/src/server/handler.rs`

---

### A-023 🟢 Low | `plugin-runtime` — Plugin Manifest Validation Missing

**Summary:** WASM plugin manifests (`plugin-sdk`) are loaded from the configured plugin directory. There is no cryptographic signature verification or capability capability-based sandboxing beyond what Extism provides by default.

**Impact:** A malicious plugin (or an accidentally corrupt one) could execute unexpected host functions. Extism's default sandbox prevents most attacks, but klyntbot's host function exposure is not audited against the principle of least privilege.

**Proposed Fix:**
- Audit `host/mod.rs` host functions — restrict to read-only filesystem access for untrusted plugins
- Add `plugin.trusted: bool` field to plugin manifest
- Only expose destructive host functions (write, spawn) to `trusted: true` plugins

**References:** `crates/plugin-runtime/src/host/mod.rs`, `crates/plugin-runtime/src/manifest.rs`

---

### A-024 🟡 Medium | `cognitive` — Reflection Runs Without Minimum Data Guard

**Summary:** The reflection handler (`cognitive/src/reflection.rs`) runs weekly and generates pattern-extraction prompts from episodic memory. If the user is new (< 7 days, < 50 conversations), the reflection will run on sparse data and generate low-quality or hallucinated "patterns".

**Impact:** Early users may see spurious coaching suggestions or memory facts derived from insufficient data.

**Proposed Fix:**
- Add `min_episode_count: u32` guard in `ReflectionHandler::should_run()`
- Default: skip reflection if `episodic_memory` count < 20
- Skip weekly consolidation if total `semantic_facts` < 10

**References:** `crates/cognitive/src/reflection.rs`, `crates/cognitive/src/consolidation.rs`

---

### A-025 🟡 Medium | `desktop` — Tauri `tauri.conf.json` Uses `npm` Instead of `bun`

**Summary:** `crates/desktop/tauri.conf.json` specifies `beforeDevCommand: "npm run dev"` but the project requires `bun` (never `npm`). Running `cargo tauri dev` directly will fail with ENOENT.

**Workaround (current):** Start Vite manually (`cd desktop-ui && bun run dev`) then run `cargo tauri dev`.

**Proposed Fix:**
- Change `tauri.conf.json` `beforeDevCommand` from `"npm run dev"` to `"bun run dev"`
- Verify `cargo tauri dev` works end-to-end after this change

**References:** `crates/desktop/tauri.conf.json`, CLAUDE.md Gotchas

---

### A-026 🟢 Low | `common/types.rs` — `ChatId` Lacks Typed Constructors

**Summary:** `ChatId` is a newtype wrapper around `String` used to identify chats. It is constructed via raw string literals in many places (`ChatId("123".to_string())`), making it easy to accidentally construct invalid IDs.

**Impact:** No functional bug, but defensive programming is weaker than it could be.

**Proposed Fix:**
- Add `ChatId::from_telegram(u64) -> Self`, `ChatId::from_discord(u64) -> Self`, `ChatId::from_slack(String) -> Self`
- Add `ChatId::parse(s: &str) -> Result<Self, ChatIdError>` with format validation

**References:** `crates/common/src/types.rs`

---

### A-027 🟢 Low | `storage` — `vector_store.rs` — No Connection Pool for LanceDB

**Summary:** `VectorStore` holds a single `lancedb::Connection` (not pooled). Under concurrent tool calls (parallel ReAct tool execution), multiple coroutines compete for the same connection. LanceDB uses file locks internally, so this is safe but may serialize concurrent vector queries.

**Impact:** Performance under load. Not a correctness issue.

**Proposed Fix:**
- Add a `Semaphore` with configurable parallelism limit (default: 4) around LanceDB operations
- Or evaluate if LanceDB's internal connection manages this already (document the finding either way)

**References:** `crates/storage/src/vector_store.rs`

---

### A-028 🟡 Medium | `agent/agent_profile` — Keyword-Based Agent Selection Is Brittle

**Summary:** `AgentManager::match_agent()` selects the active agent based on keyword matching against the user's message. The keyword lists in each `AGENT.md` frontmatter are small and cover only obvious trigger phrases. Semantic routing would be more robust.

**Impact:** Users typing natural variations ("track this as an expense" vs "log a transaction") may not trigger the `finance` agent, falling back to `general`.

**Proposed Fix:**
- Add semantic embedding similarity as a secondary scoring signal (after keyword match)
- Cache agent profile embeddings at startup
- Score: `final_score = keyword_match_score * 0.7 + semantic_similarity * 0.3`

**References:** `crates/agent/src/agent_profile/manager.rs`

---

### A-029 🟢 Low | `activity-log` — No Deduplication for Rapid Context Switches

**Summary:** The activity inference engine (`inference.rs`) emits a new work context every time the active window changes. Rapid context switches (e.g., CMD+Tab back and forth 10 times in a minute) emit 10 separate context records even if the effective work context never changed.

**Impact:** `work_context_repo` grows faster than necessary. Summaries over-count context switches.

**Proposed Fix:**
- Add debounce: only emit new context if the window stays active for > 5 seconds
- Add deduplication: skip emit if context is identical to last emitted context

**References:** `crates/activity-log/src/inference.rs`

---

### A-030 🟢 Low | All Feature Crates — `FeatureMigration` Version Comments Missing

**Summary:** Each feature crate defines `FeatureMigration` with a `version` integer and SQL. There are no inline comments explaining what each migration version changed. After many versions, it's impossible to audit the schema history.

**Impact:** Debugging migration issues is difficult. Rollback planning is impossible without history.

**Proposed Fix:**
- Add `-- Migration N: [description]` comment to the top of each migration SQL block
- Example: `-- Migration 7: Add productivity_distraction_count to focus_sessions`

---

### A-031 🔴 High | `domain` — Domain Types Dead at Runtime (No Row Bridge)

**Summary:** The `domain` crate defines rich typed entities (`Area`, `Objective`, `KeyResult`, `Project`) with status enums and progress logic. However, **no runtime code path uses them**. All callers (tools, app-core, agent) work directly with `*Row` types from `storage`. No `From<AreaRow> for Area` or inverse bridge exists anywhere in the workspace.

**Impact:** The domain crate is effectively dead code at the application level. Domain invariants (e.g., `KeyResult::recalculate_metric_progress()`) are never called in production.

**Proposed Fix:** Either (a) add `From<XxxRow> for Xxx` bridges + use domain types in tool handlers and app-core, or (b) acknowledge the crate is only a conceptual model and annotate it clearly; strip the unneeded methods.

**References:** `crates/domain/src/`, all `*Row` types in `crates/storage/src/rows/`

---

### A-032 🔴 High | `domain` — `generate_id()` Produces 8-Char Truncated UUIDs

**Summary:** `Area::generate_id()`, `Objective::generate_id()`, `KeyResult::generate_id()`, and `Project::generate_id()` all call `uuid::Uuid::new_v4().to_string()[..8]`, producing 8-character IDs. App-core and tool handlers produce full 36-character UUIDs. Mixed ID lengths exist in the database.

**Impact:** IDs in the `areas`, `objectives`, `key_results`, `projects` tables are 8 chars; IDs in `tasks`, `actions`, `sessions` are 36 chars. Cross-table FK lookups and LLM context injection are more error-prone with inconsistent formats.

**Proposed Fix:** Remove `generate_id()` from domain types. All ID generation should use `uuid::Uuid::new_v4().to_string()` (full UUID) from a single utility in `common`.

**References:** `crates/domain/src/area.rs:22`, `crates/domain/src/key_result.rs:25`, `crates/domain/src/objective.rs:22`, `crates/domain/src/project.rs:19`

---

### A-033 🟡 Medium | `storage` — `resources` and `archive_items` Tables Schema-Only

**Summary:** Two tables in `migrations/001_initial.sql` have explicit "schema-only, no tool support yet" comments: `resources` (L558) and `archive_items` (L574). Neither has a row struct, repo, or any tool implementation. They consume schema space and create false impressions of implemented features.

**Proposed Fix:** Either implement `ResourceRepo` + `ArchiveItemRepo` with associated tools, or drop the tables (pre-release — no migration script needed per CLAUDE.md).

**References:** `crates/storage/migrations/001_initial.sql:558,574`

---

### A-034 🟡 Medium | `storage` — `calendar_sync_state` / `calendar_event_cache` Have No Repo

**Summary:** Both tables are defined in `001_initial.sql:285-309` with full schemas but have no repo struct or row type in the `storage` crate. They are accessed via raw SQL in the `channels` crate, bypassing the storage abstraction layer.

**Proposed Fix:** Add `CalendarSyncStateRepo` and `CalendarEventCacheRepo` with row types to the `storage` crate. Update `channels` to use them.

**References:** `crates/storage/migrations/001_initial.sql:285-309`

---

### A-035 🟡 Medium | `storage` — `tool_usage` Table Has No Dedicated Repo

**Summary:** The `tool_usage` table tracks per-tool call statistics but has no `ToolUsageRepo`. Cleanup is done via raw SQL directly inside `Repos::cleanup_analytics()` with the comment "tool_usage has no dedicated repo — use direct SQL." This bypasses the established repo pattern.

**Proposed Fix:** Create `ToolUsageRepo` with at minimum: `insert()`, `delete_older_than()`, and `aggregate_by_tool()` (for the analytics dashboard).

**References:** `crates/storage/src/repos/mod.rs:146-152`

---

### A-036 🟡 Medium | `storage` — `FinancePortfolioRow` Defined Without `FinancePortfolioRepo`

**Summary:** `rows/finance.rs` defines `FinancePortfolioRow` and `PortfolioSummaryRow`, but there is no `FinancePortfolioRepo`. Finance investments reference portfolios via FK (`finance_investments.portfolio_id`) but portfolios cannot be created or managed through the storage API.

**Proposed Fix:** Add `FinancePortfolioRepo` to the `finance_storage` module alongside the other 6 finance repos. Add to `FinanceStorage` aggregate.

**References:** `crates/storage/src/rows/finance.rs`, `crates/storage/src/finance_storage.rs`

---

### A-037 🟡 Medium | `storage` — `custom_column_values.task_id` FK Points to `actions`, Not `tasks`

**Summary:** In `migrations/006_custom_columns.sql:17`, the column is named `task_id` but the FK is `REFERENCES actions(id)`. This is inconsistent: the field name implies `tasks` (the newer `feature-tasks` entity) but points to `actions` (the legacy `feature-todo` entity). When the `feature-todo` → `feature-tasks` migration happens (see A-009), this FK needs updating.

**Proposed Fix:** When deprecating `feature-todo`, migrate `custom_column_values.task_id` FK from `actions(id)` to `tasks(id)`.

**References:** `crates/storage/migrations/006_custom_columns.sql:17`

---

### A-038 🟢 Low | `storage` — `FinanceInvestmentTxRow` Not Re-Exported from `lib.rs`

**Summary:** `rows::finance::FinanceInvestmentTxRow` is defined in `rows/finance.rs` but is the only row type not re-exported from `lib.rs`. It is accessible only via `storage::rows::finance::FinanceInvestmentTxRow` — breaking the flat public API convention.

**Proposed Fix:** Add `pub use rows::finance::FinanceInvestmentTxRow;` to `crates/storage/src/lib.rs`.

**References:** `crates/storage/src/lib.rs`, `crates/storage/src/rows/finance.rs`

---

### A-039 🟢 Low | `storage` — `session_context.rs` Tests Use File-Based Pool + `dir.keep()`

**Summary:** `repos/session_context.rs:175-183` is the only repo test file that uses `StoragePool::connect(dir.path())` (file-based) instead of `connect_in_memory()`. It also calls `dir.keep()` to prevent cleanup, which leaves orphaned temp directories on the filesystem after each test run.

**Proposed Fix:** Migrate to `connect_in_memory()`. The only reason file-based was needed is if the tests relied on `session_context` requiring a prior `sessions` FK — which `connect_in_memory()` handles via migrations. *(Note: `session_context` has a FK to `sessions` — ensure `sessions` is seeded first.)*

**References:** `crates/storage/src/repos/session_context.rs:174-183`

---

### A-040 🟢 Low | `storage` — `session_context.rs` Manual Timestamp Formatting

**Summary:** Four places in `repos/session_context.rs` (L31, L81, L145, L157) manually format timestamps with `.format("%Y-%m-%dT%H:%M:%SZ").to_string()` and then bind them as strings. All other repos bind `chrono::DateTime<Utc>` directly via sqlx, which handles the format correctly. This inconsistency can cause subtle timestamp comparison bugs if formats diverge.

**Proposed Fix:** Replace manual format strings with direct `Utc::now()` chrono binding.

**References:** `crates/storage/src/repos/session_context.rs:31,81,145,157`

---

### A-041 🟢 Low | `domain` — `from_str_loose()` / `as_str()` / `is_terminal()` Are Dead Code

**Summary:** All status and color enums in the `domain` crate implement `from_str_loose()`, `as_str()`, and (for terminal-state enums) `is_terminal()`. None of these methods have callers outside `#[cfg(test)]` blocks in the same file. They exist in anticipation of the domain-bridge work (A-031) but are currently dead.

**Proposed Fix:** Either (a) implement A-031 and these methods become live, or (b) remove them and replace with standard `Display`/`FromStr` derives via `strum` to reduce boilerplate.

**References:** `crates/domain/src/area.rs`, `crates/domain/src/key_result.rs`, `crates/domain/src/objective.rs`, `crates/domain/src/project.rs`

### A-042 🟡 Medium | `agents/communication` — No Skills Defined

**Summary:** The communication agent has zero skills and is the only agent without a `skills/` directory. It can route messages but has no specialized workflows for message templates, broadcast coordination, or cross-channel formatting.

**Impact:** Communication agent is the thinnest agent — delegates everything to raw tool calls with no workflow guidance.

**Proposed Fix:**
- Add a `messaging` skill with workflows for single/broadcast messaging, confirmation patterns, and channel-specific formatting
- Add a `notification` skill for alert routing preferences and batching

**References:** `agents/communication/AGENT.md`

---

### A-043 🟡 Medium | `agents/general/skills/summarize.md` — References External CLI Binary

**Summary:** The `summarize` skill references an external CLI tool (`summarize` binary) with flags like `--model`, `--youtube`, `--extract-only`. This binary is not part of the klyntbot workspace, not tracked as a dependency, and has no fallback if missing.

**Impact:** Skill silently fails if the CLI is not installed. No error guidance for users.

**Proposed Fix:**
- Either (a) replace with `web_fetch`-based summarization using the agent's LLM, or (b) add a check step in the skill instructions ("verify `summarize` is installed, if not, use `web_fetch` to get the content and summarize with the LLM directly")

**References:** `agents/general/skills/summarize.md`

---

### A-044 🟢 Low | `agents` — Skill Metadata Contains Redundant `agent` Field

**Summary:** Every skill's YAML frontmatter includes `metadata.agent: <agent_name>`, but the agent is already determined by directory path (`agents/{agent}/skills/{skill}.md`). This creates a maintenance burden — if a skill is moved between agents, the metadata must be manually updated.

**Proposed Fix:** Remove the `agent` field from skill metadata. Update `AgentSkill::parse()` if it reads this field (currently it doesn't — the field is informational only).

**References:** All `agents/*/skills/*.md` files

---

## Section B — Pre-Existing Backlog Items

These items were in `BACKLOG.json` at time of refactor analysis.

### B-001 🔴 High | `agent/intent_pipeline` — Short-Message Heuristic Preempts Domain Patterns
*(Same as A-001 above — promoted from JSON backlog)*

### B-002 🟡 Medium | `feature-tasks`, `agent/enrichment` — Due Dates Underutilized
*(Same as A-002 above — promoted from JSON backlog)*

---

## Section C — Planned Features (Not Yet Started)

### C-001 🟡 Medium | Feature-Tasks Phase 3 — Proactive Suggestions + Forecasting + Cognitive Integration
**Spec:** `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md`
- Proactive daily task suggestions based on cognitive context
- Workload forecasting with trend accuracy tracking (prerequisite: A-008)
- Cognitive integration: pull relevant facts before task creation/planning
- Recurring task generation with FSRS-adjusted intervals

### C-002 🟡 Medium | Feishu/Lark Channel Implementation
**Prerequisites:** A-003 stub in place
**Spec:** Feishu Open Platform event subscription + custom robot API
- OAuth App Token flow
- Receive events via webhook (message, action card)
- Send text, card, file messages
- Interactive card with button/select support

### C-003 🟡 Medium | DingTalk Channel Implementation
**Prerequisites:** A-004 stub in place
**Spec:** DingTalk Chatbot webhook
- Outgoing webhook message receive
- `access_token` sign + send messages
- Action card interactions

### C-004 🟢 Low | Mochat Channel Implementation
**Prerequisites:** A-005 stub in place
**Spec:** Mochat WebSocket + REST API
- Socket.io connection to `socket_url`
- `claw_token` auth
- Multi-session support (personal vs business panels)

### C-005 🟢 Low | Web Dashboard (Browser UI)
**Context:** `desktop/src/dev_server.rs` already serves HTTP on port 3456. A browser-accessible dashboard without Tauri is architecturally already supported.
- React SPA served from dev_server
- Auth: local token (no cloud account needed)
- Feature parity with Desktop UI phase 1

### C-006 🟢 Low | CLI Channel Improvements
**Summary:** The CLI channel exists in the `ChannelName` enum but no dedicated `CliChannel` struct was found. The binary likely uses stdin/stdout directly. A proper `CliChannel` implementation with readline, history, and `TerminalMarkdown` rendering would improve the TUI experience.

---

## Section D — Technical Debt Without Immediate Fix

| ID | Priority | Crate | Description |
|---|---|---|---|
| D-001 | 🟢 Low | `storage` | SQLx compile-time queries (`query!()`) require `DATABASE_URL` at build time in CI — workaround needed for offline builds |
| D-002 | 🟢 Low | `providers` | `OpenAiCompatProvider` doesn't support streaming cancellation — no CancellationToken propagation |
| D-003 | 🟢 Low | `agent` | Persona injection via `identity.rs` hardcodes some personality traits — should be fully config-driven |
| D-004 | 🟢 Low | `desktop` | Tauri `tauri.conf.json` `externalBin` setup for Discord subprocess may fail on non-macOS (untested) |
| D-005 | 🟢 Low | `cognitive` | LanceDB table names use hardcoded strings — should be constants in `vector_store/tables.rs` |
| D-006 | 🟢 Low | `feature-finance` | Price service (`price_service.rs`) fetches live prices but has no circuit breaker — if API is down, tool calls hang |
| D-007 | 🟢 Low | `feature-finance` | 6-jar system is hardcoded in tool descriptions — should be configurable per user preference |
| D-008 | 🟢 Low | `bus` | `DomainEventBus` is a `broadcast::Sender` — slow consumers will miss events if they lag > `CHANNEL_CAPACITY`. No dead-letter queue. |
| D-009 | 🟢 Low | `session` | Session locking via `Arc<Mutex<Session>>` can deadlock if a tool call re-enters the session manager |
| D-010 | 🟢 Low | `scheduling` | `CronService` doesn't persist `last_run` across restarts for jobs that fired between persistence checkpoints |

---

## Refactor Phase Completion Checklist

Use this to track refactor progress:

- [x] Phase 0 — Baseline established (tests pass, clippy clean)
- [x] Phase 1 — `common` restructured
- [x] Phase 2 — `config` loader split
- [x] Phase 3 — `storage` vertical-slice grouping
- [x] Phase 4 — `providers` port/adapter split
- [x] Phase 5 — `channels` platform split into `adapters/`
- [x] Phase 6 — `cognitive` service layer split into `services/`
- [x] Phase 7 — `tools` domain grouping into `system/`, `domain/`, `embedding/`
- [x] Phase 8 — `agent` internal reorganization (8a–8c: adapters/, services/, skill_loader)
- [x] Phase 9 — `context_engine` restructure (assembler/ + history_compressor/ splits)
- [x] Phase 10 — feature crates restructure (types/, repo/, tool/ splits across 5 crates)
- [ ] Phase 11 — `app-core` restructure
- [ ] Phase 12 — `desktop-shared` domain split
- [ ] Phase 13 — `desktop` dev server split
- [ ] Phase 14 — `docs/ai-coding-rules.md` created
- [ ] Phase 15 — Final verification (all tests pass, zero warnings)
