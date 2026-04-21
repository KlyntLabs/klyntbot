# Remove Built-in AI Task Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Strip LLM-orchestrated "built-in AI" behaviors (daily planning, agentic task execution, LLM decomposition, proactive suggestions, forecasting) out of the task system, leaving a clean CRUD + scoring surface that users compose into workflows via cron/skills/automations.

**Architecture:** Delete top-down through the dependency graph: cron jobs → agent-crate LLM impls → feature-tasks trait declarations + action handlers + types. Preserve pure-data modules (`scoring.rs`, CRUD actions, recurrence, alarms) and domain-event publishing (`TaskCreated` / `TaskCompleted`) that feed cognitive/reforge. MCP whitelist needs no changes — removed actions drop out of the runtime JSON schema automatically.

**Tech Stack:** Rust 1.93 (MSRV), `cargo nextest`, `cargo clippy`, SQLx, Tauri 2. Workspace at `/Users/jayden/Projects/Klynt/bot`.

**Out of scope for this plan (next session's brainstorm):** Part B of the audit — deepening cognitive/feedback/reforge integration with the task domain (semantic-fact extraction from task completion, a `tasks` rule domain in Reforge, per-event feedback collectors). That requires design work, not deletion.

**Verification philosophy:** Since this is a deletion refactor, "tests" are compile + clippy + nextest + runtime schema checks. Each task ends with explicit verification commands.

---

## File Structure

**Files to delete entirely:**
- `crates/agent/src/handlers/decomposition.rs`
- `crates/agent/src/handlers/planning.rs`
- `crates/agent/src/handlers/execution.rs`
- `crates/agent/src/handlers/proactive.rs`
- `crates/agent/src/handlers/forecast.rs`
- `crates/agent/src/handlers/suggestion_applier.rs`
- `crates/feature-tasks/src/handlers/decomposition.rs`
- `crates/feature-tasks/src/handlers/planning.rs`
- `crates/feature-tasks/src/handlers/execution.rs`
- `crates/feature-tasks/src/handlers/proactive.rs`
- `crates/feature-tasks/src/handlers/forecast.rs`
- `crates/feature-tasks/src/handlers/enrichment.rs`
- `crates/feature-tasks/src/handlers/suggestion_applier.rs`
- `crates/feature-tasks/src/tool/actions/decompose.rs`
- `crates/feature-tasks/src/tool/actions/execute.rs`
- `crates/feature-tasks/src/tool/actions/plan.rs`
- `crates/feature-tasks/src/tool/actions/suggest.rs`
- `crates/feature-tasks/src/tool/actions/forecast.rs`
- `crates/feature-tasks/src/types/planning.rs`
- `crates/feature-tasks/src/types/execution.rs`
- `crates/feature-tasks/src/types/suggestion.rs`
- `crates/app-core/src/handlers/tasks/proactive.rs`
- `skills/task-management/references/daily-planner.md`
- `skills/task-management/references/task-decompose.md`

**Files to modify:**
- `crates/app-core/src/init/cron.rs` — remove `JOB_DAILY_PLANNING` and `JOB_PROACTIVE_SCAN` registrations and handler constructor plumbing.
- `crates/app-core/src/handlers/tasks/mod.rs` — drop `pub mod proactive;`.
- `crates/app-core/src/init/mod.rs` (or wherever `CronInitResult` is defined) — remove `proactive_handler` / `suggestion_applier` fields.
- `crates/agent/src/handlers/mod.rs` — remove module declarations for deleted files.
- `crates/feature-tasks/src/handlers/mod.rs` — same.
- `crates/feature-tasks/src/tool/actions/mod.rs` — drop module declarations and `handle_*` re-exports for deleted actions.
- `crates/feature-tasks/src/tool/mod.rs` — drop optional handler fields, builder methods, action-name array entries, match arms.
- `crates/feature-tasks/src/types/mod.rs` — drop module declarations and re-exports for deleted types.
- `crates/feature-tasks/src/lib.rs` — drop any top-level re-exports of removed handler traits and types.
- `crates/agent/src/lib.rs` (or `builder.rs`) — drop any `.with_decomposition_handler(...)`, `.with_planning_handler(...)`, etc. wiring when `TaskTool` is constructed.

**Files preserved (critical — do not touch):**
- `crates/feature-tasks/src/tool/actions/create.rs:182` — `TaskCreated` publish.
- `crates/feature-tasks/src/tool/actions/mutate.rs:281` — `TaskCompleted` publish.
- `crates/feature-tasks/src/scoring.rs` — pure urgency math.
- `crates/feature-tasks/src/{complexity,forecast,alarms,alarm_side_effects,focus_alarms,recurrence_repo,rrule_utils,cognitive_bridge}.rs` — pure helpers.
- All other `tool/actions/*.rs` (create, mutate, query, batch, deps, recurrence, search, focus).

---

## Task 1: Create feature branch and baseline

**Files:**
- Modify: none (git state only)

- [ ] **Step 1: Confirm clean working tree on feature branch**

Run:
```bash
cd /Users/jayden/Projects/Klynt/bot
git status
git checkout -b refactor/remove-builtin-ai-task-features
```

Expected: branch created, tree may have pre-existing modifications noted in CLAUDE.md — those should be committed or stashed first if unrelated. If unsure, ask the user before proceeding.

- [ ] **Step 2: Record baseline test state**

Run:
```bash
cargo nextest run -p feature-tasks -p agent -p app-core 2>&1 | tail -20
```

Expected: all tests pass (baseline). If any fail *before* you change anything, STOP and report — don't proceed into a broken tree.

- [ ] **Step 3: Commit baseline marker (empty commit)**

```bash
git commit --allow-empty -m "chore: baseline before removing built-in AI task features"
```

---

## Task 2: Remove cron-registered automations

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`
- Delete: `crates/app-core/src/handlers/tasks/proactive.rs`
- Modify: `crates/app-core/src/handlers/tasks/mod.rs`

- [ ] **Step 1: Find the exact line ranges for `JOB_DAILY_PLANNING` and `JOB_PROACTIVE_SCAN`**

Run:
```bash
grep -n "JOB_DAILY_PLANNING\|JOB_PROACTIVE_SCAN\|run_proactive_scan\|daily-planning" crates/app-core/src/init/cron.rs
```

Expected: matches around lines 82–104 (handler construction), ~413 (daily planning block), ~770–800 (proactive scan block). Note actual line numbers — they may have shifted.

- [ ] **Step 2: Delete the two job registration blocks**

Open `crates/app-core/src/init/cron.rs`. Delete the full `register_cron_job(..., JOB_DAILY_PLANNING, ...)` call and the full `register_cron_job(..., JOB_PROACTIVE_SCAN, ...)` call, including any closures they define. Also delete the `const JOB_DAILY_PLANNING: &str = ...;` and `const JOB_PROACTIVE_SCAN: &str = ...;` lines.

- [ ] **Step 3: Remove proactive handler / suggestion applier construction in `init_cron`**

In the same file, find the block (~lines 82–104) that constructs `ProactiveHandler` and `SuggestionApplier` impls and threads them into `register_cron_callbacks`. Delete it. Remove those parameters from any call sites.

- [ ] **Step 4: Delete the orchestrator file**

```bash
rm crates/app-core/src/handlers/tasks/proactive.rs
```

- [ ] **Step 5: Drop the `pub mod proactive;` line**

Edit `crates/app-core/src/handlers/tasks/mod.rs` and remove the `pub mod proactive;` declaration and any re-exports of `run_proactive_scan`.

- [ ] **Step 6: Remove `proactive_handler` / `suggestion_applier` / `forecast_handler` / `planning_handler` / `decomposition_handler` / `execution_handler` fields from `CronInitResult` (or equivalent struct) if present**

Run:
```bash
grep -rn "proactive_handler\|suggestion_applier\|forecast_handler\|planning_handler\|decomposition_handler\|execution_handler" crates/app-core/src/
```

For each hit in a struct definition or construction inside `app-core`, remove the field. Also remove any `use` statements that imported the deleted trait types.

- [ ] **Step 7: Verify app-core compiles**

Run:
```bash
cargo check -p app-core 2>&1 | tail -30
```

Expected: compiles clean, or errors only about missing traits in the `agent` crate wiring — those get fixed in Task 3. If errors mention `feature-tasks` handler traits still being referenced in `app-core`, resolve now (delete those references).

- [ ] **Step 8: Commit**

```bash
git add crates/app-core
git commit -m "refactor(app-core): remove JOB_DAILY_PLANNING and JOB_PROACTIVE_SCAN cron jobs"
```

---

## Task 3: Remove agent-crate LLM handler implementations

**Files:**
- Delete: `crates/agent/src/handlers/{decomposition,planning,execution,proactive,forecast,suggestion_applier}.rs`
- Modify: `crates/agent/src/handlers/mod.rs`
- Modify: the agent builder / `TaskTool` wiring site (likely `crates/agent/src/builder.rs` or `crates/agent/src/lib.rs`)

- [ ] **Step 1: Delete the six impl files**

```bash
rm crates/agent/src/handlers/decomposition.rs \
   crates/agent/src/handlers/planning.rs \
   crates/agent/src/handlers/execution.rs \
   crates/agent/src/handlers/proactive.rs \
   crates/agent/src/handlers/forecast.rs \
   crates/agent/src/handlers/suggestion_applier.rs
```

- [ ] **Step 2: Drop module declarations**

Edit `crates/agent/src/handlers/mod.rs`. Remove the `mod decomposition;`, `mod planning;`, `mod execution;`, `mod proactive;`, `mod forecast;`, `mod suggestion_applier;` lines and any `pub use` re-exports of types from those modules.

- [ ] **Step 3: Remove builder wiring that attaches handlers to `TaskTool`**

Run:
```bash
grep -rn "with_decomposition_handler\|with_planning_handler\|with_execution_handler\|with_proactive_handler\|with_forecast_handler\|with_suggestion_applier\|with_enrichment_handler" crates/agent/
```

For each hit, delete the `.with_*_handler(...)` call chain entry and any preceding `Arc::new(SomeLlmImpl::new(...))` construction it relied on.

- [ ] **Step 4: Verify agent compiles**

Run:
```bash
cargo check -p agent 2>&1 | tail -40
```

Expected: either compiles clean, or errors point only at trait imports that no longer exist. If errors still reference the *trait declarations* (e.g. `feature_tasks::handlers::DecompositionHandler`), that means Task 4 must come next — that's fine. If errors reference *other* unrelated things, investigate before proceeding.

- [ ] **Step 5: Commit**

```bash
git add crates/agent
git commit -m "refactor(agent): delete LLM handler impls for decompose/plan/execute/proactive/forecast"
```

---

## Task 4: Remove feature-tasks trait declarations, action handlers, and types

**Files:**
- Delete (handlers): `crates/feature-tasks/src/handlers/{decomposition,planning,execution,proactive,forecast,enrichment,suggestion_applier}.rs`
- Delete (actions): `crates/feature-tasks/src/tool/actions/{decompose,execute,plan,suggest,forecast}.rs`
- Delete (types): `crates/feature-tasks/src/types/{planning,execution,suggestion}.rs`
- Modify: `crates/feature-tasks/src/handlers/mod.rs`
- Modify: `crates/feature-tasks/src/tool/actions/mod.rs`
- Modify: `crates/feature-tasks/src/types/mod.rs`
- Modify: `crates/feature-tasks/src/tool/mod.rs`
- Modify: `crates/feature-tasks/src/lib.rs`

- [ ] **Step 1: Delete the trait files**

```bash
rm crates/feature-tasks/src/handlers/decomposition.rs \
   crates/feature-tasks/src/handlers/planning.rs \
   crates/feature-tasks/src/handlers/execution.rs \
   crates/feature-tasks/src/handlers/proactive.rs \
   crates/feature-tasks/src/handlers/forecast.rs \
   crates/feature-tasks/src/handlers/enrichment.rs \
   crates/feature-tasks/src/handlers/suggestion_applier.rs
```

Preserved in the directory: `embedding.rs`, `progress.rs`, `mod.rs`.

- [ ] **Step 2: Prune `handlers/mod.rs`**

Edit `crates/feature-tasks/src/handlers/mod.rs`. Remove the seven `pub mod ...;` lines for the deleted files. Remove any `pub use` re-exports of the deleted trait/type names.

- [ ] **Step 3: Delete action handler files**

```bash
rm crates/feature-tasks/src/tool/actions/decompose.rs \
   crates/feature-tasks/src/tool/actions/execute.rs \
   crates/feature-tasks/src/tool/actions/plan.rs \
   crates/feature-tasks/src/tool/actions/suggest.rs \
   crates/feature-tasks/src/tool/actions/forecast.rs
```

- [ ] **Step 4: Prune `tool/actions/mod.rs`**

Edit `crates/feature-tasks/src/tool/actions/mod.rs`. Remove the five `pub(super) mod ...;` lines and any re-exports of `handle_plan_day`, `handle_decompose`, `handle_execute`, `handle_cancel_execution`, `handle_suggest`, `handle_apply_suggestion`, `handle_dismiss_suggestion`, `handle_list_suggestions`, `handle_forecast_task`, `handle_forecast_project`, `handle_accuracy_report`.

- [ ] **Step 5: Delete type files**

```bash
rm crates/feature-tasks/src/types/planning.rs \
   crates/feature-tasks/src/types/execution.rs \
   crates/feature-tasks/src/types/suggestion.rs
```

- [ ] **Step 6: Prune `types/mod.rs`**

Edit `crates/feature-tasks/src/types/mod.rs`. Remove the three `pub mod ...;` lines and any `pub use` re-exports of `DayPlan`, `PlanSlot`, `PlanningContext`, `TaskExecution`, `ExecutionConfig`, `ExecuteResult`, `SuggestionCandidate`, `SuggestionAction`, etc.

**Caveat:** `EnergyLevel` may live in `types/planning.rs` but be referenced elsewhere. Before deleting `planning.rs`, run:
```bash
grep -rn "EnergyLevel" crates/
```
If `EnergyLevel` is used outside of the removed code paths, move the enum definition into `crates/feature-tasks/src/types/entity.rs` (or another preserved module) before deleting `planning.rs`.

- [ ] **Step 7: Update `TaskTool` struct in `tool/mod.rs`**

Edit `crates/feature-tasks/src/tool/mod.rs`:

- Remove the optional handler fields (around lines 40–55): `decomposition_handler`, `planning_handler`, `execution_handler`, `proactive_handler`, `suggestion_applier`, `forecast_handler`, `enrichment_handler` (and anything of that shape).
- Remove their `None` initializations (~lines 85–90) in the constructor.
- Remove the builder methods `with_*_handler` (~lines 186–202).
- Remove the action-name string entries (lines 318–321): delete `"plan_day"`, `"decompose"`, `"execute"`, `"cancel_execution"`, `"suggest"`, `"apply_suggestion"`, `"dismiss_suggestion"`, `"list_suggestions"`, `"forecast_task"`, `"forecast_project"`, `"accuracy_report"`.
- Remove the JSON schema property for `suggestion_id` (~line 405) if it has no other consumer.
- Remove the match arms in `execute()` (~lines 498–508): `plan_day`, `decompose`, `execute`, `cancel_execution`, `suggest`, `apply_suggestion`, `dismiss_suggestion`, `list_suggestions`, `forecast_task`, `forecast_project`, `accuracy_report`.
- Remove any `use` imports for deleted traits/types at the top of the file.

- [ ] **Step 8: Prune `feature-tasks/src/lib.rs` re-exports**

Run:
```bash
grep -n "DecompositionHandler\|DayPlanningHandler\|TaskExecutionHandler\|ProactiveHandler\|ForecastHandler\|EnrichmentHandler\|SuggestionApplier\|DayPlan\|TaskExecution\|SuggestionCandidate" crates/feature-tasks/src/lib.rs
```

Delete every matching `pub use` line.

- [ ] **Step 9: Verify feature-tasks compiles**

Run:
```bash
cargo check -p feature-tasks 2>&1 | tail -40
```

Expected: clean compile. If there are errors about tests referencing removed types, note the test files — fix in Step 10.

- [ ] **Step 10: Remove test modules that exercised deleted actions**

Run:
```bash
grep -rn "plan_day\|decompose\|\"execute\"\|suggest\|forecast_task" crates/feature-tasks/src/tool/mod.rs crates/feature-tasks/tests/ 2>/dev/null
```

For each test referring to the deleted actions, delete the `#[test]` or `#[tokio::test]` function entirely. Do not leave empty test shells.

- [ ] **Step 11: Verify compile + tests**

Run:
```bash
cargo nextest run -p feature-tasks 2>&1 | tail -20
cargo check --workspace 2>&1 | tail -40
```

Expected: `feature-tasks` tests pass. Workspace-wide check may still show errors in `agent`/`app-core` if earlier tasks left dangling refs — fix now before committing.

- [ ] **Step 12: Commit**

```bash
git add crates/feature-tasks
git commit -m "refactor(feature-tasks): remove LLM handlers, actions, and types for plan/decompose/execute/suggest/forecast"
```

---

## Task 5: Remove skill references and update task-management SKILL.md

**Files:**
- Delete: `skills/task-management/references/daily-planner.md`
- Delete: `skills/task-management/references/task-decompose.md`
- Modify: `skills/task-management/SKILL.md`

- [ ] **Step 1: Delete the two LLM-skill reference files**

```bash
rm skills/task-management/references/daily-planner.md \
   skills/task-management/references/task-decompose.md
```

- [ ] **Step 2: Audit SKILL.md for references to the deleted behaviors**

Run:
```bash
grep -n "plan_day\|decompose\|daily.planning\|task.decompose\|proactive\|forecast_task\|suggest" skills/task-management/SKILL.md
```

For each match, edit `skills/task-management/SKILL.md`:
- Remove sentences/examples that describe `plan_day`, `decompose`, `execute`, `suggest`, `forecast_task`, `forecast_project`, `accuracy_report`.
- Remove any `references/daily-planner.md` / `references/task-decompose.md` entries from the frontmatter `references:` list.
- Leave references to CRUD actions (`create`, `update`, `list`, `search`, `focus`) untouched.

- [ ] **Step 3: Verify skill still parses**

Run:
```bash
cargo check -p skill-system 2>&1 | tail -10
```

Expected: clean. `skill-system` uses `include_str!` so missing files would fail at compile if they were referenced there. If it complains about a missing `include_str!` target, update the relevant entry in `crates/skill-system/src/` or remove the include.

- [ ] **Step 4: Commit**

```bash
git add skills/ crates/skill-system/
git commit -m "refactor(skills): drop daily-planner and task-decompose references"
```

---

## Task 6: Full workspace verification

**Files:** none modified.

- [ ] **Step 1: Run clippy on the workspace**

Run:
```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | tail -60
```

Expected: 0 warnings (the project's zero-clippy policy per CLAUDE.md). Unused-import warnings in modules you edited mean you left dangling `use` lines — clean them.

- [ ] **Step 2: Run full test suite**

Run:
```bash
cargo nextest run --workspace 2>&1 | tail -30
```

Expected: all tests pass. Any regressions here almost certainly mean a preserved code path referenced a deleted symbol — trace the failure to its root.

- [ ] **Step 3: Run doctests**

Run:
```bash
cargo test --workspace --doc 2>&1 | tail -10
```

Expected: pass.

- [ ] **Step 4: Check formatting**

Run:
```bash
cargo fmt --all --check
```

Expected: no diff. If there is one, run `cargo fmt --all` and include in the next commit.

- [ ] **Step 5: Verify MCP tool schema no longer advertises removed actions**

Run:
```bash
cargo run -p klyntbot-mcp -- tools --schema tasks 2>&1 | grep -E "plan_day|decompose|\"execute\"|suggest|forecast_task|accuracy_report" || echo "CLEAN: no removed actions present"
```

Expected: prints `CLEAN: no removed actions present`. If any action name appears, a match arm or schema entry was missed in Task 4 Step 7.

- [ ] **Step 6: Verify `TaskCreated` / `TaskCompleted` publishing still present**

Run:
```bash
grep -n "TaskCreated\|TaskCompleted" crates/feature-tasks/src/tool/actions/create.rs crates/feature-tasks/src/tool/actions/mutate.rs
```

Expected: both events still published. If either is missing, restore it — the cognitive/reforge integration depends on these.

- [ ] **Step 7: Commit fixup if anything was cleaned in Steps 1–4**

```bash
git status
git add -A
git diff --cached --stat
git commit -m "chore: post-refactor fmt/clippy cleanup" # only if there are changes
```

---

## Task 7: Final documentation and handoff note

**Files:**
- Modify: `CLAUDE.md` (add a short "removed features" note)

- [ ] **Step 1: Add a removed-features note under Gotchas**

Open `CLAUDE.md`. Under the `## Gotchas` section, append:

```markdown
- **Built-in AI task automations removed (2026-04-20).** The task tool no longer supports `plan_day`, `decompose`, `execute`, `suggest`/`apply_suggestion`/`dismiss_suggestion`/`list_suggestions`, `forecast_task`, `forecast_project`, `accuracy_report`. These LLM-driven behaviors are now meant to be composed by users via cron + skill + `agent` tool. `TaskCreated` / `TaskCompleted` still publish to the domain bus so cognitive and reforge continue to receive task signals.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: note removal of built-in AI task automations"
```

- [ ] **Step 3: Summarize for user**

Write a brief summary listing: files deleted, files modified, test results, clippy status, and the followup item parked for the next brainstorming session (Part B — deepen cognitive/feedback/reforge wiring on the task domain).

---

## Self-Review Checklist (run before considering plan done)

1. **Spec coverage** — every item in Parts A and C of the audit report has a concrete task: cron jobs (Task 2), agent impls (Task 3), feature-tasks traits + actions + types (Task 4), skill refs (Task 5). Cognitive/reforge integration Part B is explicitly out of scope.
2. **Placeholders** — no "TBD" / "add appropriate handling" / "similar to above." Every deletion has an explicit file path. Every verification has an explicit command.
3. **Type consistency** — handler trait names (`DecompositionHandler`, `DayPlanningHandler`, `TaskExecutionHandler`, `ProactiveHandler`, `ForecastHandler`, `EnrichmentHandler`, `SuggestionApplier`) used identically across Tasks 3 and 4. Action names (`plan_day`, `decompose`, `execute`, `cancel_execution`, `suggest`, `apply_suggestion`, `dismiss_suggestion`, `list_suggestions`, `forecast_task`, `forecast_project`, `accuracy_report`) consistent between Task 4 Step 7 and Task 6 Step 5 grep.
4. **Preservation explicit** — Tasks call out that `TaskCreated`/`TaskCompleted` publishes, `scoring.rs`, and CRUD actions must remain; Task 6 Step 6 actively verifies this.
5. **Dependency order honored** — cron → agent impls → feature-tasks traits → skill refs. A reader executing in order cannot hit an "orphan impl references missing trait" error.
