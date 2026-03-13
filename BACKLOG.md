# Klyntbot — Engineering Backlog
> Last updated: 2026-03-13
> Format: `[ID] Priority | Crate(s) | Short description`
> Priorities: 🔴 High | 🟡 Medium | 🟢 Low

---

## Section A — Issues Found During Refactor Analysis

These items were identified during the 2026-03-12 structural refactor analysis.

---

### A-002 🟡 Medium | `feature-tasks`, `agent/enrichment` — Due Dates Systematically Underutilized

**Summary:** `due_date` field exists on `Task` but tasks are routinely created without deadlines. The enrichment engine does not aggressively infer due dates from natural language (e.g., "finish report by Friday", "monthly tax deadline"). The UI doesn't surface urgency visually. Scoring assigns urgency=1 (minimum) to most tasks.

**Impact:** Urgency scoring is near-useless. No overdue reminders. Day-planning quality degrades because priorities are flat.

**Proposed Fix:**
1. `agent/enrichment/engine.rs` — Enhance LLM prompt to be more aggressive about inferring due dates from message context and extracting explicit date mentions
2. `desktop-ui` — Color-code due dates: red = overdue, yellow = due today/tomorrow
3. Skill update — Add "When is this due?" follow-up question in the task creation skill
4. `agent/reminders.rs` — Add overdue task reminder scan on daily schedule

**References:** `crates/feature-tasks/src/types.rs`, `crates/agent/src/enrichment/engine.rs`

---

### A-006 🟡 Medium | `app-core/handlers/work_context` — Merge Tracking Not Implemented

**Summary:** TODO comments exist for tracking context merges and exposing them from the inference loop. The work context inference loop can detect context merges but does not record them.

**Impact:** Work context history may have gaps. Merge analytics are unavailable.

**Proposed Fix:**
- Add `context_merges` table to `activity-log` migrations
- Record merge events in `work_context_repo.rs` when inference detects a merge
- Expose merge count in `WorkContextSummary` response type

**References:** `crates/app-core/src/handlers/work_context.rs`

---

### A-007 🟢 Low | `app-core/handlers/work_context` — "Compare With Yesterday" Not Implemented

**Summary:** The daily work summary handler has placeholders for cross-day comparison (productivity trends) and pulling context from cognitive memory.

**Impact:** Daily summaries are less actionable. No trend visibility.

**Proposed Fix:**
- Add yesterday comparison in `WorkContextSummaryHandler::summarize_day()`
- Query `cognitive` semantic facts for work-related context before generating summary

**References:** `crates/app-core/src/handlers/work_context.rs`

---

### A-008 🟡 Medium | `agent/handlers/forecast.rs` — Trend Computation Not Implemented

**Summary:** The forecast handler has a placeholder for computing accuracy trends (is the agent getting better at estimating task durations?). The raw data is available in `strategy_records` and `enrichment_feedback` tables.

**Impact:** The confidence evaluator cannot improve over time based on forecast accuracy. The adaptive learning loop is incomplete.

**Proposed Fix:**
- Query last 30 days vs last 90 days of forecast accuracy records
- Compute trend direction and magnitude
- Feed into `ConfidenceEvaluator` threshold adjustment

**References:** `crates/agent/src/handlers/forecast.rs`

---

### A-010 🟡 Medium | `feature-tasks` — Phase 3 Implementation Pending

**Summary:** Feature-tasks Phase 2 is complete. Phase 3 covers proactive task suggestions, workload forecasting improvements, cognitive integration, and recurring task generation with FSRS decay.

**Impact:** Task management lacks proactivity. Agent doesn't suggest tasks based on calendar/context.

**References:** `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md`

---

### A-012 🟢 Low | `agent/agent_loop/builder.rs` (1,210 LOC) — Monolithic Builder

**Summary:** The `AgentLoopBuilder` has 50+ setter methods in a single file. Related concerns (channel setup, cognitive wiring, feature registration) are interleaved.

**Proposed Fix:** Split into builder extension files by concern.

---

### A-014 🟡 Medium | `cognitive` — No Semantic Fact Expiry/Pruning

**Summary:** The cognitive memory system accumulates semantic facts over time but has no automatic pruning. Facts with very low salience are never deleted.

**Impact:** Memory retrieval will slow down. Disk usage grows indefinitely.

**Proposed Fix:**
- Add `prune_expired_facts(threshold: f32)` to `SemanticFactRepo`
- Schedule weekly prune in `cognitive/background/` pipeline
- Default threshold: salience < 0.05 AND last_accessed > 180 days

**References:** `crates/cognitive/src/repos/semantic_fact.rs`

---

### A-015 🟡 Medium | `providers` — Pricing Tables Not Up to Date

**Summary:** `agent/src/output/cost_tracker.rs` contains hardcoded pricing tables for 12 providers. Provider pricing changes frequently.

**Impact:** Monthly budget alerts may be inaccurate.

**Proposed Fix:**
- Add `updated_at` comment to each pricing table entry
- Consider moving pricing to `config.json` so users can update without recompiling

**References:** `crates/agent/src/output/cost_tracker.rs`

---

### A-016 🟢 Low | `channels` — WebSocket Channel Not Listed in `ChannelName` Enum

**Summary:** The WebSocket channel (`ws_manager.rs`) is used by the Desktop UI but has no corresponding `ChannelName::WebSocket` variant.

**Proposed Fix:** Add `ChannelName::WebSocket` variant and update all match statements.

**References:** `crates/common/src/types.rs`, `crates/channels/src/ws_manager.rs`

---

### A-017 🟢 Low | `config` — No Config Schema Validation at Startup

**Summary:** No semantic validation beyond serde type checking. Invalid combinations are silently ignored until runtime failure.

**Impact:** Confusing startup errors. Users can set `enabled: true` on a channel without the required API keys.

**Proposed Fix:**
- Add `Config::validate() -> Result<(), Vec<ConfigError>>` in `config/loader/`
- Check: required keys present if channel enabled, model name valid for selected provider, budget > 0 if set

**References:** `crates/config/src/loader.rs`

---

### A-018 🟢 Low | `agent/content_registry` — Skills Not Hot-Reloaded

**Summary:** Agent skills are loaded at startup via `include_str!()`. Changes require a full rebuild + restart.

**Impact:** Developer experience only (no production impact).

**Proposed Fix:**
- Add optional runtime skill loading via filesystem read (controlled by `config.development.hot_reload_skills = true`)

**References:** `crates/agent/src/content_registry/loader.rs`, `crates/agent/src/skill_loader.rs`

---

### A-019 🟡 Medium | `providers` — Circuit Breaker State is In-Memory Only

**Summary:** `ProviderManager` circuit breaker state is in-memory. On restart, all circuit breakers reset to closed even if a provider was failing repeatedly.

**Impact:** After restart, the agent will hammer a failing provider again until the circuit reopens.

**Proposed Fix:**
- Persist circuit breaker state in SQLite (`circuit_breaker_state` table)
- Include `open_until: Option<DateTime<Utc>>` so breakers survive restarts

**References:** `crates/providers/src/manager.rs`

---

### A-020 🟢 Low | `scheduling` — No Missed-Run Catchup Logic

**Summary:** If the app was offline for days, ALL missed cron jobs fire immediately on startup.

**Impact:** Spike load and potentially duplicate notifications after extended downtime.

**Proposed Fix:**
- Add `max_catchup_age: Duration` to `CronService` configuration (default: 1 hour)
- Skip jobs whose missed time is older than `max_catchup_age`
- For jobs that should always run, add an `always_catchup: bool` flag

**References:** `crates/scheduling/src/service/mod.rs`

---

### A-022 🟢 Low | `mcp` — MCP Server Does Not Support Tool Filtering by Client

**Summary:** `McpServerRunner` exposes all registered tools unconditionally. No filtering by calling agent identity or capability scope.

**Impact:** Security gap if the server is exposed beyond localhost.

**Proposed Fix:**
- Add `allowed_tools: Vec<String>` to `McpServerConfig`
- Filter tool list based on allowlist

**References:** `crates/mcp/src/server/handler.rs`

---

### A-023 🟢 Low | `plugin-runtime` — Plugin Manifest Validation Missing

**Summary:** No cryptographic signature verification or capability-based sandboxing beyond Extism defaults.

**Impact:** Klyntbot's host function exposure is not audited against least privilege.

**Proposed Fix:**
- Audit `host/mod.rs` host functions — restrict to read-only filesystem access for untrusted plugins
- Add `plugin.trusted: bool` field to plugin manifest

**References:** `crates/plugin-runtime/src/host/mod.rs`, `crates/plugin-runtime/src/manifest.rs`

---

### A-024 🟡 Medium | `cognitive` — Reflection Runs Without Minimum Data Guard

**Summary:** The reflection handler runs weekly on episodic memory. New users with sparse data get low-quality or hallucinated "patterns".

**Impact:** Early users may see spurious coaching suggestions.

**Proposed Fix:**
- Add `min_episode_count: u32` guard in `ReflectionHandler::should_run()`
- Default: skip reflection if `episodic_memory` count < 20

**References:** `crates/cognitive/src/reflection.rs`, `crates/cognitive/src/consolidation.rs`

---

### A-026 🟢 Low | `common/types.rs` — `ChatId` Lacks Typed Constructors

**Summary:** `ChatId` is constructed via raw string literals everywhere, making it easy to construct invalid IDs.

**Proposed Fix:**
- Add `ChatId::from_telegram(u64)`, `ChatId::from_discord(u64)`, `ChatId::from_slack(String)`
- Add `ChatId::parse(s: &str) -> Result<Self, ChatIdError>`

**References:** `crates/common/src/types.rs`

---

### A-027 🟢 Low | `storage` — No Connection Pool for LanceDB

**Summary:** `VectorStore` holds a single `lancedb::Connection`. Under concurrent tool calls, queries may serialize.

**Impact:** Performance under load. Not a correctness issue.

**Proposed Fix:** Add a `Semaphore` with configurable parallelism limit around LanceDB operations.

**References:** `crates/storage/src/vector_store.rs`

---

### A-028 🟡 Medium | `agent/agent_profile` — Keyword-Based Agent Selection Is Brittle

**Summary:** `AgentManager::match_agent()` uses keyword matching. Natural variations may miss the right agent.

**Impact:** Users typing natural variations may fall back to `general` agent.

**Proposed Fix:**
- Add semantic embedding similarity as a secondary scoring signal
- Score: `final_score = keyword_match_score * 0.7 + semantic_similarity * 0.3`

**References:** `crates/agent/src/agent_profile/manager.rs`

---

### A-029 🟢 Low | `activity-log` — No Deduplication for Rapid Context Switches

**Summary:** Rapid CMD+Tab switches emit separate context records even when the effective work context never changed.

**Impact:** `work_context_repo` grows faster than necessary. Summaries over-count switches.

**Proposed Fix:**
- Add debounce: only emit new context if window stays active for > 5 seconds
- Add deduplication: skip if context is identical to last emitted

**References:** `crates/activity-log/src/inference.rs`

---

### A-031 🔴 High | `domain` — Domain Types Dead at Runtime (No Row Bridge)

**Summary:** The `domain` crate defines rich typed entities but no runtime code path uses them. All callers work directly with `*Row` types from `storage`.

**Impact:** Domain crate is effectively dead code. Domain invariants are never called in production.

**Proposed Fix:** Either (a) add `From<XxxRow> for Xxx` bridges + use domain types in handlers, or (b) strip the unneeded methods and annotate as conceptual model only.

**References:** `crates/domain/src/`, `crates/storage/src/rows/`

---

### A-033 🟡 Medium | `storage` — `resources` and `archive_items` Tables Schema-Only

**Summary:** Two tables have no row struct, repo, or tool implementation. They create false impressions of implemented features.

**Proposed Fix:** Either implement repos + tools, or drop the tables (pre-release).

**References:** `crates/storage/migrations/001_initial.sql`

---

### A-034 🟡 Medium | `storage` — `calendar_sync_state` / `calendar_event_cache` Have No Repo

**Summary:** Both tables are accessed via raw SQL in the `channels` crate, bypassing the storage abstraction.

**Proposed Fix:** Add `CalendarSyncStateRepo` and `CalendarEventCacheRepo` to the `storage` crate.

**References:** `crates/storage/migrations/001_initial.sql`

---

### A-035 🟡 Medium | `storage` — `tool_usage` Table Has No Dedicated Repo

**Summary:** Cleanup is done via raw SQL bypassing the repo pattern.

**Proposed Fix:** Create `ToolUsageRepo` with `insert()`, `delete_older_than()`, `aggregate_by_tool()`.

**References:** `crates/storage/src/repos/mod.rs`

---

### A-036 🟡 Medium | `storage` — `FinancePortfolioRow` Defined Without `FinancePortfolioRepo`

**Summary:** Portfolios cannot be created or managed through the storage API despite having a row type and FK references.

**Proposed Fix:** Add `FinancePortfolioRepo` to the `finance_storage` module.

**References:** `crates/storage/src/rows/finance.rs`, `crates/storage/src/finance_storage.rs`

---

---

### A-041 🟢 Low | `domain` — Dead Code: `from_str_loose()` / `as_str()` / `is_terminal()`

**Summary:** Status/color enum methods have no callers outside `#[cfg(test)]`. Exist in anticipation of A-031.

**Proposed Fix:** Either implement A-031 (methods become live), or replace with `strum` derives.

**References:** `crates/domain/src/`

---

### A-042 🟡 Medium | `agents/communication` — No Skills Defined

**Summary:** The communication agent has zero skills — the only agent without a `skills/` directory.

**Proposed Fix:**
- Add a `messaging` skill for single/broadcast messaging and channel-specific formatting
- Add a `notification` skill for alert routing and batching

**References:** `agents/communication/AGENT.md`

---

### A-043 🟡 Medium | `agents/general/skills/summarize.md` — References External CLI Binary

**Summary:** The `summarize` skill references an external CLI tool not tracked as a dependency. No fallback if missing.

**Proposed Fix:** Add a check step or replace with `web_fetch`-based summarization using the agent's LLM.

**References:** `agents/general/skills/summarize.md`

---

### A-044 🟢 Low | `agents` — Skill Metadata Contains Redundant `agent` Field

**Summary:** Every skill's YAML frontmatter includes `metadata.agent` but the agent is already determined by directory path.

**Proposed Fix:** Remove the `agent` field from skill metadata.

**References:** All `agents/*/skills/*.md` files

---

## Section C — Planned Features (Not Yet Started)

### C-001 🟡 Medium | Feature-Tasks Phase 3 — Proactive Suggestions + Forecasting + Cognitive Integration
**Spec:** `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md`
- Proactive daily task suggestions based on cognitive context
- Workload forecasting with trend accuracy tracking (prerequisite: A-008)
- Cognitive integration: pull relevant facts before task creation/planning
- Recurring task generation with FSRS-adjusted intervals

### C-005 🟢 Low | Web Dashboard (Browser UI)
**Context:** `desktop/src/dev_server/` already serves HTTP on port 3456. A browser-accessible dashboard without Tauri is architecturally already supported.
- React SPA served from dev_server
- Auth: local token (no cloud account needed)
- Feature parity with Desktop UI phase 1

### C-006 🟢 Low | CLI Channel Improvements
**Summary:** No dedicated `CliChannel` struct. A proper implementation with readline, history, and `TerminalMarkdown` rendering would improve the TUI experience.

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
