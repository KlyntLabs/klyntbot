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



