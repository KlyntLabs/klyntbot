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


### A-010 🟡 Medium | `feature-tasks` — Phase 3 Implementation Pending

**Summary:** Feature-tasks Phase 2 is complete. Phase 3 covers proactive task suggestions, workload forecasting improvements, cognitive integration, and recurring task generation with FSRS decay.

**Impact:** Task management lacks proactivity. Agent doesn't suggest tasks based on calendar/context.

**References:** `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md`

---

### A-012 🟢 Low | `agent/agent_loop/builder.rs` (1,210 LOC) — Monolithic Builder

**Summary:** The `AgentLoopBuilder` has 50+ setter methods in a single file. Related concerns (channel setup, cognitive wiring, feature registration) are interleaved.

**Proposed Fix:** Split into builder extension files by concern.

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

## Section C — Planned Features (Not Yet Started)

### C-001 🟡 Medium | Feature-Tasks Phase 3 — Proactive Suggestions + Forecasting + Cognitive Integration
**Spec:** `docs/superpowers/plans/2026-03-12-feature-tasks-phase3.md`
- Proactive daily task suggestions based on cognitive context
- Workload forecasting with trend accuracy tracking (A-008 resolved)
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
