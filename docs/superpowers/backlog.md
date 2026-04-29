# Backlog

Non-blocking issues deferred for later. Not part of any active plan; pick up once v3 (or whatever is in flight) lands.

---

## 1. `budget_overrun_frequency` metric always samples `0.0`

**Observed (2026-04-23):** After wiring v2.5 emission sites and triggering three `finance_transaction_create` calls against a `food` budget of $50 with txns $30 + $40 + $60 (cumulative $130 — well over), every `TransactionRecorded` domain event payload shows `is_over_budget: false`, and `ai_metric_samples.budget_overrun_frequency` rows all have `value = 0.0`.

**Scope:** The v2.5 pipeline is working end-to-end — the metric is being harvested; the input value is wrong. The `is_over_budget` flag computed inside `finance_transaction_create` (in `crates/app-core/src/handlers/finance/transactions.rs`) is not reflecting actual cumulative spend vs the matching budget limit.

**Suspected causes:**
- Budget lookup might be matching on wrong column (e.g. `name` vs `category`) or currency/period filter wrong.
- Spend aggregation might be missing the just-inserted row, or scanning wrong date range.
- `amount` stored as cents-as-integer; comparison might be against a non-cents value.

**Next steps:**
1. Add a debug log in `finance_transaction_create` right where `is_over_budget` is computed: log `{category, spent, limit, threshold_ok}`.
2. Trigger an overspend; verify the log matches SQLite reality.
3. Fix the condition. Add a regression test that inserts two transactions over a budget and asserts `TransactionRecorded.is_over_budget == true` on the second.

**Does NOT affect:** `BudgetAlert` crossing-edge publish (Agent B wired that with its own `spent_before <= limit < spent_after` comparison, which is independent of the `is_over_budget` flag).

---

## 2. `task_estimation_bias` metric samples `0.0` when time entries exist

**Observed (2026-04-23):** Created task with `estimatedMinutes: 30`, called `task_add_time_entry` with `durationSecs: 2700` (= 45 min), then `task_toggle_complete`. The completion response showed `actualMinutes: null` and the resulting `EstimationRecorded` event carried `deviation_pct: 0`. Expected `deviation_pct: 50.0`.

**Scope:** v2.5 emission is working — `EstimationRecorded` fires. But the upstream aggregate `actual_minutes` or `total_tracked_secs` on the task row isn't reflecting the time entry that was persisted seconds earlier.

**Suspected causes:**
- `task_add_time_entry` writes to a separate table (`task_time_entries`?) and never rolls up into `tasks.total_tracked_secs` or `tasks.actual_minutes`.
- OR rollup exists but is lazy (nightly cron / on-read), so the completion handler reads stale data.
- OR the completion handler's fallback path (`if total_tracked_secs > 0 { Some(total_tracked_secs / 60) }`) is correct but the value is genuinely 0 at that moment.

**Next steps:**
1. After `task_add_time_entry` via API, `sqlite3 data.db "SELECT id, actual_minutes, total_tracked_secs FROM tasks WHERE id = ?"` — see whether the task row reflects the entry.
2. If not, either (a) emit a rollup on insert, or (b) aggregate from `task_time_entries` at completion time instead of reading the task column.
3. Add a regression test: create task + time entry + toggle_complete → assert the emitted `EstimationRecorded.deviation_pct` is non-zero and matches expected math.

**Does NOT affect:** `TaskCompleted` event (correctly fires with `actual_duration_mins: null` which is factually true given the missing rollup) or any other v2.5 metric.

---

## 3. Community intelligence / co-activation tables dormant

**Observed (2026-04-23):** `communities`, `community_members`, `co_activation` all have 0 rows after heavy CRUD traffic.

**Scope:** Expected — these populate only after the ingestion consumer detects co-retrievals across many messages and the nightly reforge runs `apply_intelligence`. The v2.5 wiring (CommunityEvent publishing, CoActivationStrengthened threshold publish) is correct; it just hasn't had the traffic/time to trigger.

**Next steps:**
- Not a bug. To exercise in test: either wait for real usage over days, or write a one-off integration test that seeds many episodic memories, forces `record_co_retrieval` with repeated fact pairs, and asserts `co_activation.strength >= 2.0` triggers a `CoActivationStrengthened` event.

---

## 4. Launcher: `FocusDashboard` widget — decide whether to remove backend or implement

**Observed (2026-04-27):** Backend `build_dashboard_data()` populates `DashboardData.focus: Option<FocusDashboard>` (active session, elapsed/target seconds, task name) but the React `Dashboard.tsx` never renders a focus widget. User has indicated they do **not** want the focus dashboard shown in the launcher.

**Scope:** Backend code at `crates/app-core/src/handlers/launcher/dashboard.rs:19-48` and `FocusDashboard` type in `crates/feature-launcher/src/types.rs` are dead from the launcher's perspective. The data is still computed and serialized but ignored on the frontend.

**Next steps:**
1. Confirm `FocusDashboard` data is not consumed elsewhere (search for `FocusDashboard` usages outside the launcher).
2. Remove `focus` field from `DashboardData`, drop the `build_focus_dashboard` helper, drop `FocusDashboard` type if unreferenced.
3. Update `desktop-ui/src/features/launcher/types.ts` to remove `FocusDashboard` interface and `focus` field.

**Does NOT affect:** Other dashboard widgets (calendar, tasks, productivity) which are wired and rendered.

---

## 5. Launcher: `onOpenTask` callback never wired

**Observed (2026-04-27):** `Dashboard.tsx` accepts an `onOpenTask?: (taskId: string) => void` prop and `TasksWidget` invokes it on click, but `Launcher.tsx:205` renders `<Dashboard />` without passing the callback. Dashboard task rows are silently unclickable.

**Scope:** Pure frontend prop-drilling gap. Deferred until **task UI re-integration** completes — there is no "open task in main window" route to call yet.

**Next steps:**
1. After task UI is re-integrated and main-window task routes exist, pass `onOpenTask={(id) => emit("navigate", { path: "/tasks/" + id })}` from `Launcher.tsx`.
2. Add a regression test that verifies clicking a task row in the dashboard emits the navigation event.

---

## 6. Launcher: silent IPC error handling — no user-facing toast

**Observed (2026-04-27):** All Tauri IPC errors in `useExecuteItem.ts`, `ActionMenu.tsx`, and `FocusActiveChip.tsx` are caught with `.catch(console.error)` and never surfaced to the user. 17+ execute paths fail invisibly (e.g., `launcher_open_app` failure on a missing path, `focus_activate` failure mid-DND).

**Scope:** UX quality gap. The `show_status_badge` Tauri command already exists (used by `noView` items in `useExecuteItem.ts:225`) — the same plumbing can be reused for error toasts.

**Next steps:**
1. Add a small `notifyError(message: string)` helper that calls `show_status_badge` with `kind: "error"` and a sensible duration.
2. Replace every `.catch(console.error)` in launcher hooks/components with `.catch((err) => notifyError("Could not …: " + err))`.
3. Distinguish user-fixable failures (path not found, permission denied) from internal errors with separate copy.

---

## 7. Launcher: DND duration threading deferred (originally "Task 3.4")

**Observed (2026-04-27):** `crates/desktop/src/commands/launcher.rs:89-91` reads `let _ = args;` with comment *"DND duration threading deferred to Task 3.4 when SystemCommands::execute gains a duration parameter."* The frontend works around this via the separate `focus_activate` IPC path, but the Rust `SystemCommands::execute` still ignores duration args.

**Scope:** Cosmetic from the user's perspective (DND duration works via the workaround), but the workaround diverges system-command DND from script-runner args, complicating future args-bearing commands.

**Next steps:**
1. Add a `duration_secs: Option<u64>` parameter to `SystemCommands::execute(&self, action: &str, duration_secs: Option<u64>)`.
2. Inside `ToggleDoNotDisturb`, if `duration_secs` is provided, schedule auto-deactivation; else toggle indefinitely.
3. Plumb the value from `launcher_system_command(args: HashMap<String,String>)` by parsing `args.get("duration_secs").and_then(|s| s.parse().ok())`.
4. Once stable, drop the frontend-side `focus_activate` workaround in `useExecuteItem.ts:86-97`.

## 9. Agent: tree builders are 6 near-identical copy-pasted files

**Observed (2026-04-29):** `adapters/finance_tree_builder.rs`, `learning_tree_builder.rs`, `note_tree_builder.rs`, `okr_tree_builder.rs`, `productivity_tree_builder.rs`, and `task_tree_builder.rs` each duplicate:
- Identical 5-field struct (`tree_repo`, `vector_store`, `embedder`, `context_update_queue`, `domain_event_bus`)
- Identical `new()` constructor
- Identical `tokio::select!` event loop (`shutdown` + `rx.recv()` + `Lagged`/`Closed` handling)
- Identical `persist_nodes()` (SQLite insert + embedding upsert + `ContextUpdate` push)
- Identical `compose_embedding_text()` helper

**Scope:** ~2,600 lines across 6 files where only the event variant names and node constructors differ.

**Next steps:**
1. Extract a `TreeBuilderBase<S: EventSource>` trait or `TreeIndexer` struct that owns common fields, the `run()` loop, `persist_nodes()`, and `compose_embedding_text()`.
2. Each domain file implements only event handling (`handle_transaction`, `handle_note_changed`, etc.) and domain-specific node construction.
3. Estimated savings: ~600+ lines.

---

## 10. Agent: `AgentLoopBuilder::build()` is a ~1,750-line god function

**Observed (2026-04-29):** `crates/agent/src/agent_loop/builder.rs:214` assembles the entire dependency graph inline — context sources, cognitive background services, tree builder subscribers, tool registration, runtime wiring, and learning services. Nested `if let` blocks reach 4–5 levels deep, making unit testing impossible.

**Scope:** Critical readability and testability issue. The builder works correctly today; the problem is maintainability.

**Next steps:**
1. Split `build()` into private phase methods: `build_context_engine()`, `build_tree_builders()`, `build_tool_registry()`, `build_runtime()`, etc.
2. Each phase returns its intermediate product, making individual phases testable.
3. Use early-return guards (`if x.is_none() { return Ok(...); }`) to flatten deep nesting.

---

## 11. Agent: correction handling logic duplicated in `agent_loop/mod.rs`

**Observed (2026-04-29):** The exact same two-phase correction detection is copy-pasted in `process_message` (lines 559–654) and `process_direct_streaming` (lines 998–1083):
1. `detect_correction_prefix()` / `detect_memory_miss()`
2. Read last assistant message + decrement cooldown under session lock
3. Rate-limited `emit_correction_signal()` call
4. Build `CorrectionContext` for query rewriting

**Scope:** ~100 lines of duplicated logic. Any bug fix or tuning would need to be applied in both places.

**Next steps:**
1. Introduce a `CorrectionState { strength, skill, original, emitted }` struct.
2. Extract a single `handle_correction(&self, msg, session_arc) -> CorrectionState` helper.
3. Call it from both `process_message` and `process_direct_streaming`.

---

## 12. Agent: parameter sprawl across hot paths

**Observed (2026-04-29):** Multiple functions carry 5+ parameters that travel together:

| Function | File | Line | Params |
|---|---|---|---|
| `process_message` | `agent_runtime/runtime.rs` | 245 | 8 (`&self`, `message`, `history`, `tool_definitions`, `ctx`, `event_tx`, `cancel_token`, `depth`) |
| `emit_correction_signal` | `agent_loop/mod.rs` | 207 | 7 |
| `run_pipeline` | `agent_loop/mod.rs` | 887 | 7 |
| `run_cycle` | `execution/core.rs` | 404 | 6 |
| `init_agent` | `app-core/src/init/agent.rs` | 27 | 13 |
| `relay_chat_stream` | `app-core/src/handlers/chat/streaming.rs` | 371 | 10 |

**Scope:** `#[allow(clippy::too_many_arguments)]` is used to silence the lint rather than fix the underlying design.

**Next steps:**
1. Introduce `CycleContext { messages, tools, params, routing_ctx }` for `run_cycle` and downstream callers.
2. Introduce `CorrectionSignal` struct for `emit_correction_signal`.
3. Convert `init_agent` to builder-style (`AgentInit::new().with_autotuner(...).build()`).
4. Remove `#[allow(clippy::too_many_arguments)]` annotations once resolved.

---

## 13. Agent: stringly-typed event fields where enums would be safer

**Observed (2026-04-29):** `crates/agent/src/events.rs` uses `String` for discriminant-like fields:

| Line | Field | Better Type |
|---|---|---|
| 96 | `ConfidenceAssessed { action: String }` | `enum ConfidenceAction { Retry, Escalate, Continue, … }` |
| 151 | `LearningEvent { event_type: String, … }` | `enum LearningEventType { ThresholdAdjusted, PatternDetected, … }` |
| 187 | `McpServerStatus { status: String, … }` | `enum McpStatus { Starting, Ready, Failed, Skipped }` |

Also in `agent_runtime/runtime.rs`:
- `mode_used: String` — always `"normal"`, `"deep_think"`, or `"ultra"`. Should be `DepthMode`.
- `agent_name: String` — always `"klyntbot"` in flat runtime. Should be a const or `AgentName` enum.

**Scope:** Serialization compatibility risk. Changing these to enums may break downstream consumers (desktop-ui, MCP clients) that rely on the current string values.

**Next steps:**
1. Audit all consumers of these structs to confirm string expectations.
2. Add enums with `#[serde(rename_all = "snake_case")]` or equivalent to preserve wire format.
3. Migrate fields incrementally, starting with internal-only structs.

---

## 14. Agent: leaky abstractions — internal details exposed via public accessors

**Observed (2026-04-29):** `crates/agent/src/agent_loop/mod.rs:99–113` exposes:
- `pub fn tool_registry(&self) -> Arc<RwLock<ToolRegistry>>`
- `pub fn skill_store(&self) -> Arc<RwLock<SkillStore>>`
- `pub fn hot_config(&self) -> Arc<RwLock<HotConfig>>`

These exist solely so `klyntbot-server/src/handler.rs` and `app-core` can mutate agent internals directly.

**Scope:** Architectural encapsulation break. The server should interact with the agent via messages/commands, not by directly mutating the tool registry.

**Next steps:**
1. Design a command channel or message-based API for tool registry mutations (register, unregister, list).
2. Migrate `klyntbot-server` and `app-core` call sites to use the new API.
3. Make the accessors `pub(crate)` or remove them entirely.

---

## 15. Agent: `MockProvider` duplicated in 6 test modules

**Observed (2026-04-29):** Every test module defines its own `MockProvider` implementing `LlmProvider`:
- `handlers/coding_synthesis.rs` — single `String` response
- `handlers/rule_artifacts.rs` — single `String` response
- `execution/core.rs` — `Mutex<Vec<LlmResponse>>` with `with_text` / `with_tool_call` helpers
- `agent_runtime/runtime.rs` — single `String` response + `context_window()`
- `adapters/cognitive_handlers.rs` — `Result<LlmResponse, String>` + streaming stubs
- `agent_loop/refactor_tests.rs` — single `String` response

**Scope:** ~250 lines of duplicated mock code. Changes to the `LlmProvider` trait require updating all 6 copies.

**Next steps:**
1. Design a unified `MockProvider` in `agent/src/test_utils.rs` (behind `#[cfg(test)]`) that supports all observed variants: queued responses, error injection, streaming stubs, tool calls.
2. Provide constructors: `MockProvider::with_text()`, `MockProvider::with_tool_call()`, `MockProvider::with_error()`, `MockProvider::with_responses(vec)`.
3. Replace all 6 local copies with the shared mock.

---

## 16. Agent: redundant / derived state in `RuntimeResult`

**Observed (2026-04-29):** `crates/agent/src/agent_runtime/runtime.rs:34–36`:
```rust
pub struct RuntimeResult {
    pub content: String,
    pub mode_used: String,   // always depth.to_string()
    pub agent_name: String,  // always "klyntbot"
    …
}
```

Both fields are derivable constants. `agent_name` never changes in the flat runtime; `mode_used` is identical to the `DepthMode` input.

**Scope:** Low-severity noise. Every call site that constructs `RuntimeResult` must supply these redundant values.

**Next steps:**
1. Change `mode_used` to `DepthMode` (its source type) or derive it automatically.
2. Replace `agent_name: String` with a `const DEFAULT_AGENT_NAME: &str` or an `AgentName` enum with a `Default` impl.
3. Update all construction sites. Check downstream consumers (desktop-ui, server) for breakage.

