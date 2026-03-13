# app-core Phase 11 Refactor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize `crates/app-core/` into domain-grouped handler subdirectories and a phased `init/` directory — zero logic changes, zero behavior changes.

**Architecture:** Split the 1,377-line `init.rs` into 8 phase-modules under `init/`; move infra files to `infrastructure/`; split 7 oversized handler files into subdirectories grouping related functionality; 15 small flat handler files stay untouched.

**Tech Stack:** Rust stable, Cargo workspace, `cargo check`, `cargo nextest run`

---

## Dependency Order

```
Task 1 (infrastructure move)
    ├── Task 2 (init/ split)          ← depends on Task 1 for file_watcher path
    ├── Task 3 (tasks/ split)         ← independent after Task 1
    ├── Task 4 (cognitive/ split)     ← independent after Task 1
    ├── Task 5 (notes/ split)         ← independent after Task 1
    ├── Task 6 (finance/ split)       ← independent after Task 1
    ├── Task 7 (productivity/ split)  ← independent after Task 1
    ├── Task 8 (chat/ split)          ← independent after Task 1
    └── Task 9 (settings/ split)      ← independent after Task 1
                    ↓
        Task 10 (final verification)
```

Tasks 2–9 can execute in **parallel** after Task 1 completes.

**Zero-logic-change rule:** Every step is a file move, directory creation, `pub mod`/`pub use` addition, or path update. No function bodies, algorithms, or error paths may be modified. Run `cargo check -p app-core` after every substep and `cargo nextest run --workspace` at the end of each task.

---

## Chunk 1: Infrastructure + Init

---

### Task 1: Move infrastructure modules

**Files:**
- Create: `crates/app-core/src/infrastructure/mod.rs`
- Create: `crates/app-core/src/infrastructure/file_watcher.rs` (moved)
- Create: `crates/app-core/src/infrastructure/shell_hook.rs` (moved)
- Modify: `crates/app-core/src/lib.rs`
- Modify: `crates/app-core/src/handlers/capture.rs`
- Delete: `crates/app-core/src/file_watcher.rs`
- Delete: `crates/app-core/src/shell_hook.rs`

- [ ] **Step 1: Create the `infrastructure/` directory and `mod.rs`**

```bash
mkdir -p crates/app-core/src/infrastructure
```

Create `crates/app-core/src/infrastructure/mod.rs`:

```rust
pub mod file_watcher;
pub mod shell_hook;
```

- [ ] **Step 2: Copy `file_watcher.rs` to `infrastructure/`**

Copy the entire contents of `crates/app-core/src/file_watcher.rs` verbatim to `crates/app-core/src/infrastructure/file_watcher.rs`. No changes to the file content.

- [ ] **Step 3: Copy `shell_hook.rs` to `infrastructure/`**

Copy the entire contents of `crates/app-core/src/shell_hook.rs` verbatim to `crates/app-core/src/infrastructure/shell_hook.rs`. No changes to the file content.

- [ ] **Step 4: Add `infrastructure` to `lib.rs`**

In `crates/app-core/src/lib.rs`, add `pub mod infrastructure;` alongside the existing module declarations:

```rust
pub mod errors;
pub mod events;
pub mod file_watcher;    // keep temporarily until old files deleted
pub mod handlers;
pub mod infrastructure;  // add this
pub mod init;
pub mod shell_hook;      // keep temporarily until old files deleted
pub mod state;

pub use init::EventChannels;
pub use state::{AppCore, EntityUpdate, HandlerResult};
```

- [ ] **Step 5: `cargo check` — both old and new paths should compile**

```bash
cd /Users/jayden/Projects/Klynt/nanobot/klyntbot
cargo check -p app-core
```

Expected: compiles cleanly (both `crate::file_watcher` and `crate::infrastructure::file_watcher` exist simultaneously).

- [ ] **Step 6: Update `capture.rs` — 7 `crate::shell_hook::` references**

In `crates/app-core/src/handlers/capture.rs`, replace every occurrence of `crate::shell_hook::` with `crate::infrastructure::shell_hook::`. There are exactly 7 call sites — use find-and-replace, do not edit any other content.

- [ ] **Step 7: Update `init.rs` — `crate::file_watcher::` references**

In `crates/app-core/src/init.rs`, replace every occurrence of `crate::file_watcher::` with `crate::infrastructure::file_watcher::`. Do not edit any other content.

- [ ] **Step 8: `cargo check` — verify path updates compile**

```bash
cargo check -p app-core
```

Expected: compiles cleanly.

- [ ] **Step 9: Remove old `file_watcher.rs` and `shell_hook.rs` from `lib.rs` and delete files**

In `crates/app-core/src/lib.rs`, remove the lines:
```rust
pub mod file_watcher;
pub mod shell_hook;
```

Then delete the old files:
```bash
rm crates/app-core/src/file_watcher.rs
rm crates/app-core/src/shell_hook.rs
```

- [ ] **Step 10: `cargo check` — verify clean after deletion**

```bash
cargo check -p app-core
```

Expected: compiles cleanly with zero warnings.

- [ ] **Step 11: Commit**

```bash
git add crates/app-core/src/infrastructure/ \
        crates/app-core/src/lib.rs \
        crates/app-core/src/handlers/capture.rs \
        crates/app-core/src/init.rs
git rm crates/app-core/src/file_watcher.rs \
       crates/app-core/src/shell_hook.rs
git commit -m "refactor(app-core): move file_watcher + shell_hook into infrastructure/"
```

---

### Task 2: Split `init.rs` into `init/` directory

**Context:** `init.rs` is 1,377 lines containing `EventChannels`, `AppCore::init()`, and `AppCore::init_with_sender()`. The init function initializes subsystems in sequence. The split extracts each phase into a `pub(super) async fn` helper in its own module, then has `init/mod.rs` call them in the original order. **No logic changes.**

**Files:**
- Create: `crates/app-core/src/init/mod.rs`
- Create: `crates/app-core/src/init/storage.rs`
- Create: `crates/app-core/src/init/agent.rs`
- Create: `crates/app-core/src/init/channels.rs`
- Create: `crates/app-core/src/init/cron.rs`
- Create: `crates/app-core/src/init/productivity.rs`
- Create: `crates/app-core/src/init/coaching.rs`
- Create: `crates/app-core/src/init/cognitive.rs`
- Delete: `crates/app-core/src/init.rs`

- [ ] **Step 1: Read `init.rs` in full before starting**

Open `crates/app-core/src/init.rs` and read it completely. There are two kinds of content to identify:

**A. Inline initialization phases** inside `init_with_sender()` — extract these into `pub(super) async fn` helpers in their respective phase modules.

**B. Standalone private helper functions** (outside `init_with_sender()`, roughly lines 597–1377). Assign each to its phase module as follows:
- `register_cron_callbacks()`, `ensure_cron_jobs()`, `parse_time_to_cron()` → `cron.rs`
- `spawn_event_log_persistence()`, `domain_for_event()` → `cognitive.rs`
- `build_situation_inputs()`, `spawn_situation_recompute()` → `coaching.rs`
- `spawn_background()` → `mod.rs` (general-purpose background spawner)

Move each function verbatim to its assigned module. If a standalone function calls another standalone function from a different module, add a `use super::module_name::function_name;` import.

- [ ] **Step 2: Create `init/` directory**

```bash
mkdir -p crates/app-core/src/init
```

- [ ] **Step 3: Create `init/storage.rs`**

Extract the storage initialization phase from `init_with_sender()` into a helper. The helper takes `data_dir: &std::path::Path` and `config: &config::Config` and returns the storage components (StoragePool, Repos, VectorStore, NoteRepo, etc.). The function should be `pub(super)`.

Read the storage section of `init.rs` carefully and move those lines verbatim into the helper body. The orchestrating `init_with_sender()` in `mod.rs` will call `storage::init_storage(...)` and destructure the result.

- [ ] **Step 4: Create `init/agent.rs`**

Extract the AgentLoop + PersonaManager initialization into `pub(super) async fn init_agent(...)`. Takes storage pool, config, provider, etc. as parameters; returns `(Arc<AgentLoop>, Arc<RwLock<PersonaManager>>)`. Move lines verbatim from `init.rs`.

- [ ] **Step 5: Create `init/channels.rs`**

Extract the ChannelManager initialization into `pub(super) async fn init_channels(...)`. Move lines verbatim.

- [ ] **Step 6: Create `init/cron.rs`**

Extract the CronService setup and all cron job registrations (cognitive reflection cron, productivity cron, etc.) into `pub(super) async fn init_cron(...)`. Move lines verbatim.

- [ ] **Step 7: Create `init/productivity.rs`**

Extract the productivity feature initialization block (guarded by the productivity feature flag) into `pub(super) async fn init_productivity(...)`. Move lines verbatim.

- [ ] **Step 8: Create `init/coaching.rs`**

Extract the coaching pipeline setup (SignalAccumulator, PatternDetector, InterventionRouter, FeedbackTracker, UserSituation, CoachingService) into `pub(super) async fn init_coaching(...)`. Move lines verbatim.

- [ ] **Step 9: Create `init/cognitive.rs`**

Extract the cognitive event log, pipeline broadcast, domain bus wiring, and ActivityIngestionService setup into `pub(super) async fn init_cognitive(...)`. Move lines verbatim.

- [ ] **Step 10: Create `init/mod.rs`**

`init/mod.rs` contains:
- All `use` imports from the original `init.rs`
- The `EventChannels` struct (verbatim)
- `AppCore::init()` (verbatim — it just delegates to `init_with_sender`)
- `AppCore::init_with_sender()` — the orchestrator that calls each phase helper in the original order, passing results between phases as before

```rust
mod storage;
mod agent;
mod channels;
mod cron;
mod productivity;
mod coaching;
mod cognitive;

// ... all imports from original init.rs ...

pub struct EventChannels { ... }  // verbatim

impl AppCore {
    pub async fn init(...) -> Result<(Self, EventChannels), String> { ... }
    pub async fn init_with_sender(...) -> Result<(Self, EventChannels), String> {
        // Phase 1: storage
        let storage_components = storage::init_storage(...).await?;
        // Phase 2: agent
        let (agent, persona_manager) = agent::init_agent(...).await?;
        // ... etc., calling each phase in original order ...
        // Final: construct AppCore and EventChannels exactly as before
    }
}
```

- [ ] **Step 11: Update `lib.rs` to declare `init` as a module directory**

`lib.rs` already has `pub mod init;`. Since Rust resolves `pub mod init;` to either `init.rs` OR `init/mod.rs`, you only need to delete the old `init.rs` — no `lib.rs` changes required.

- [ ] **Step 12: Delete `init.rs`**

Rust hard-errors if both `init.rs` and `init/mod.rs` exist simultaneously — do NOT run `cargo check` before this step.

```bash
rm crates/app-core/src/init.rs
```

- [ ] **Step 13: `cargo check` — final verification**

```bash
cargo check -p app-core
cargo nextest run -p app-core
```

Expected: all tests pass, zero warnings.

- [ ] **Step 14: Commit**

```bash
git add crates/app-core/src/init/
git rm crates/app-core/src/init.rs
git commit -m "refactor(app-core): split init.rs into init/ phase modules"
```

---

## Chunk 2: Handler Splits (tasks, cognitive, notes)

---

### Task 3: Split `handlers/tasks.rs`

**Context:** `tasks.rs` (597 lines) contains row converters, helper functions, and `impl AppCore` handler methods. After the split, `objectives.rs`, `key_results.rs`, `entity_links.rs`, and `status.rs` (all UNCHANGED) call into `super::tasks::*` — those symbols must be re-exported from `tasks/mod.rs`.

**Files:**
- Create: `crates/app-core/src/handlers/tasks/mod.rs`
- Create: `crates/app-core/src/handlers/tasks/converters.rs`
- Create: `crates/app-core/src/handlers/tasks/crud.rs`
- Create: `crates/app-core/src/handlers/tasks/queries.rs`
- Delete: `crates/app-core/src/handlers/tasks.rs`

- [ ] **Step 1: Create `handlers/tasks/` directory**

```bash
mkdir -p crates/app-core/src/handlers/tasks
```

- [ ] **Step 2: Create `handlers/tasks/converters.rs`**

Move the following from `tasks.rs` into `converters.rs` — copy verbatim, keep all `use` imports needed:
- `priority_label()` — `pub(crate) fn`
- `row_to_task_response()` — `pub(crate) fn`
- `action_to_today_task()` — `pub(crate) fn`
- `objective_to_response()` — `pub(crate) fn`
- `kr_to_response()` — `pub(crate) fn`
- `rows_to_tasks()` — `pub(crate) async fn`
- `resolve_status_label()` — `pub(super) async fn` (called by `crud.rs`; must not be private)
- `row_to_task()` — `pub(crate) async fn`

- [ ] **Step 3: Create `handlers/tasks/crud.rs`**

Move the `impl AppCore` methods from `tasks.rs` that handle CRUD + subtask operations — copy verbatim with their `use` imports:
- `task_get()`
- `task_list()`
- `task_create()`
- `task_update()`
- `task_delete()`
- `task_toggle_complete()`
- `task_list_children()`

Add `use super::converters::{row_to_task, rows_to_tasks, resolve_status_label, row_to_task_response};` at the top.

- [ ] **Step 4: Create `handlers/tasks/queries.rs`**

Move the remaining `impl AppCore` methods — copy verbatim with their `use` imports:
- `today_tasks()`
- `project_list_for_tasks()`
- `objective_list_for_tasks()`

In `queries.rs`, the call to `super::projects::build_project_response` becomes `super::super::projects::build_project_response` (one extra `super` because we're now in a subdirectory).

Add at the top: `use super::converters::{kr_to_response, objective_to_response};`

- [ ] **Step 5: Create `handlers/tasks/mod.rs`**

```rust
mod converters;
mod crud;
mod queries;

// Re-exports required by sibling handler files (UNCHANGED files that call super::tasks::*)
pub(crate) use converters::{
    action_to_today_task,
    kr_to_response,
    objective_to_response,
    priority_label,
    row_to_task,
    row_to_task_response,
    rows_to_tasks,
};
```

- [ ] **Step 6: `cargo check` — with both old `tasks.rs` and new `tasks/` present**

```bash
cargo check -p app-core
```

Rust will error with "file found for module `tasks` in both `tasks.rs` and `tasks/mod.rs`" — that's expected. Proceed to next step.

- [ ] **Step 7: Delete `handlers/tasks.rs`**

```bash
rm crates/app-core/src/handlers/tasks.rs
```

- [ ] **Step 8: `cargo check` — verify compilation**

```bash
cargo check -p app-core
```

Expected: compiles cleanly. If `status.rs`, `objectives.rs`, `key_results.rs`, or `entity_links.rs` fail to resolve `super::tasks::*`, ensure `mod.rs` re-exports the missing symbol.

- [ ] **Step 9: Commit**

```bash
git add crates/app-core/src/handlers/tasks/
git rm crates/app-core/src/handlers/tasks.rs
git commit -m "refactor(app-core): split handlers/tasks.rs into tasks/ subdirectory"
```

---

### Task 4: Split `handlers/cognitive.rs`

**Context:** `cognitive.rs` (662 lines) contains helper functions (`fact_to_response`, `rule_to_response`, `fact_preview`, `build_reflection_handlers`) and `impl AppCore` handler methods. `project_memories.rs` (UNCHANGED) calls `super::cognitive::fact_to_response` — this must be re-exported from `cognitive/mod.rs`.

**Files:**
- Create: `crates/app-core/src/handlers/cognitive/mod.rs`
- Create: `crates/app-core/src/handlers/cognitive/memory.rs`
- Create: `crates/app-core/src/handlers/cognitive/mutations.rs`
- Create: `crates/app-core/src/handlers/cognitive/operations.rs`
- Delete: `crates/app-core/src/handlers/cognitive.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p crates/app-core/src/handlers/cognitive
```

- [ ] **Step 2: Create `handlers/cognitive/memory.rs`**

Move into `memory.rs` — copy verbatim:
- `fact_to_response()` — `pub(crate) fn`
- `rule_to_response()` — `pub(crate) fn`
- `fact_preview()` — `pub(crate) fn`
- `build_reflection_handlers()` — `pub(crate) fn`
- `impl AppCore` methods for read-only memory access:
  - `cognitive_user_model()`
  - `cognitive_facts_list()`
  - `cognitive_episodic_list()`
  - `cognitive_rules_list()`
  - `cognitive_memory_stats()`
  - `cognitive_system_status()`

- [ ] **Step 3: Create `handlers/cognitive/mutations.rs`**

Move into `mutations.rs` — copy verbatim with needed `use` imports:
- `impl AppCore` CRUD methods:
  - `cognitive_fact_create()`
  - `cognitive_fact_update()`
  - `cognitive_fact_delete()`
  - `cognitive_rule_create()`
  - `cognitive_rule_deactivate()`

Add `use super::memory::{fact_to_response, rule_to_response};` at the top.

- [ ] **Step 4: Create `handlers/cognitive/operations.rs`**

Move into `operations.rs` — copy verbatim:
- `cognitive_run_compaction()`
- `cognitive_run_reflection()`
- `cognitive_event_log()`
- `cognitive_pipeline_log()`
- `cognitive_inject_event()`

Add `use super::memory::build_reflection_handlers;` at the top.

- [ ] **Step 5: Create `handlers/cognitive/mod.rs`**

```rust
mod memory;
mod mutations;
mod operations;

// Re-exports required by project_memories.rs (UNCHANGED, calls super::cognitive::fact_to_response)
pub(crate) use memory::{
    build_reflection_handlers,
    fact_preview,
    fact_to_response,
    rule_to_response,
};
```

- [ ] **Step 6: Delete `handlers/cognitive.rs` and verify**

```bash
rm crates/app-core/src/handlers/cognitive.rs
cargo check -p app-core
```

Expected: compiles cleanly. If `project_memories.rs` fails, ensure `fact_to_response` is re-exported from `cognitive/mod.rs`.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/cognitive/
git rm crates/app-core/src/handlers/cognitive.rs
git commit -m "refactor(app-core): split handlers/cognitive.rs into cognitive/ subdirectory"
```

---

### Task 5: Split `handlers/notes.rs`

**Context:** `notes.rs` (573 lines) contains row converters, wiki-link extraction, note CRUD, and notebook CRUD.

**Files:**
- Create: `crates/app-core/src/handlers/notes/mod.rs`
- Create: `crates/app-core/src/handlers/notes/converters.rs`
- Create: `crates/app-core/src/handlers/notes/notes.rs`
- Create: `crates/app-core/src/handlers/notes/notebooks.rs`
- Delete: `crates/app-core/src/handlers/notes.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p crates/app-core/src/handlers/notes
```

- [ ] **Step 2: Create `handlers/notes/converters.rs`**

Move into `converters.rs` — copy verbatim with needed `use` imports:
- `note_row_to_response()` — `pub(crate) fn`
- `note_with_tags()` — `pub(crate) async fn`
- `notebook_row_to_response()` — `pub(crate) fn`
- `notes_with_tags_batch()` — `pub(crate) async fn`
- `version_row_to_response()` — `pub(crate) fn`
- `extract_links_and_mentions()` — `pub(crate) async fn`

- [ ] **Step 3: Create `handlers/notes/notes.rs`**

Move note CRUD + attachment methods into `notes.rs` — copy verbatim:
- `note_list()`
- `note_get()`
- `note_search()`
- `note_links_all()`
- `note_list_by_entity()`
- `note_version_list()`
- `note_create()`
- `note_update()`
- `note_delete()`
- `note_version_create()`
- `note_version_restore()`
- `note_save_attachment()`

Add at the top: `use super::converters::{note_with_tags, notes_with_tags_batch, note_row_to_response, version_row_to_response, extract_links_and_mentions};`

- [ ] **Step 4: Create `handlers/notes/notebooks.rs`**

Move notebook methods into `notebooks.rs` — copy verbatim:
- `notebook_list()`
- `notebook_create()`
- `notebook_update()`
- `notebook_delete()`

Add at the top: `use super::converters::notebook_row_to_response;`

- [ ] **Step 5: Create `handlers/notes/mod.rs`**

```rust
mod converters;
mod notebooks;
mod notes;
```

No external re-exports needed — no unchanged sibling files call into `super::notes::*`.

- [ ] **Step 6: Delete `handlers/notes.rs` and verify**

```bash
rm crates/app-core/src/handlers/notes.rs
cargo check -p app-core
```

Expected: compiles cleanly.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/notes/
git rm crates/app-core/src/handlers/notes.rs
git commit -m "refactor(app-core): split handlers/notes.rs into notes/ subdirectory"
```

---

## Chunk 3: Handler Splits (finance, productivity, chat, settings)

---

### Task 6: Split `handlers/finance.rs`

**Context:** `finance.rs` (791 lines). Internal sections: helper fns, read-only queries, account/transaction/budget/goal/liability/portfolio/investment mutations, filtered queries, reports.

**Files:**
- Create: `crates/app-core/src/handlers/finance/mod.rs`
- Create: `crates/app-core/src/handlers/finance/accounts.rs`
- Create: `crates/app-core/src/handlers/finance/transactions.rs`
- Create: `crates/app-core/src/handlers/finance/budgets.rs`
- Create: `crates/app-core/src/handlers/finance/investments.rs`
- Create: `crates/app-core/src/handlers/finance/reports.rs`
- Delete: `crates/app-core/src/handlers/finance.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p crates/app-core/src/handlers/finance
```

- [ ] **Step 2: Identify shared helpers in `finance.rs`**

Read `finance.rs` and identify the two private helper methods on `AppCore`:
- `default_currency()` — used across accounts, budgets, goals, liabilities, investments
- `finance_updates()` — used by every mutation handler

These helpers live in `mod.rs` since they're called from all sub-modules.

- [ ] **Step 3: Create `handlers/finance/mod.rs`**

```rust
mod accounts;
mod budgets;
mod investments;
mod reports;
mod transactions;

use crate::state::{AppCore, EntityUpdate};
use desktop_shared::types::EntityKind;
use desktop_shared::errors::ApiError;

// Shared helpers called by child sub-modules (accounts, budgets, etc.) — must be pub(crate),
// NOT pub(super): pub(super) would only be visible to handlers/, not to finance/ children.
impl AppCore {
    pub(crate) async fn default_currency(&self) -> String {
        self.config.read().await.finance.default_currency.clone()
    }

    pub(crate) fn finance_updates(id: String) -> Vec<EntityUpdate> {
        vec![EntityUpdate {
            kind: EntityKind::Finance,
            id,
        }]
    }
}
```

Copy the helper method bodies verbatim from `finance.rs`.

- [ ] **Step 4: Create `handlers/finance/accounts.rs`**

Move `impl AppCore` methods verbatim with needed `use` imports:
- `finance_accounts()`
- `finance_account_create()`
- `finance_account_update()`
- `finance_account_delete()`

Add `use super::*;` or explicit `use crate::state::{AppCore, EntityUpdate, HandlerResult}; use super::AppCore as _; ` — use whatever import style is needed for the `default_currency()` and `finance_updates()` helpers visible via `super`.

- [ ] **Step 5: Create `handlers/finance/transactions.rs`**

Move verbatim:
- `finance_transactions()`
- `finance_transaction_create()`
- `finance_transaction_delete()`
- `finance_transactions_filtered()`

- [ ] **Step 6: Create `handlers/finance/budgets.rs`**

Move verbatim:
- `finance_budget_usage()`
- `finance_budget_create()`
- `finance_budget_update()`
- `finance_budget_delete()`

- [ ] **Step 7: Create `handlers/finance/investments.rs`**

Move verbatim:
- `finance_portfolios()`
- `finance_investments()`
- `finance_portfolio_create()`
- `finance_investment_create()`
- `finance_investment_update()`
- `finance_investments_filtered()`

- [ ] **Step 8: Create `handlers/finance/reports.rs`**

Move verbatim:
- `finance_goals()`
- `finance_liabilities()`
- `finance_net_worth()`
- `finance_exchange_rates()`
- `finance_goal_create()`
- `finance_goal_update()`
- `finance_goal_delete()`
- `finance_liability_create()`
- `finance_liability_update()`
- `finance_liability_delete()`
- `finance_report_spending()`
- `finance_report_income()`
- `finance_report_by_type()` (private helper)
- `finance_report_trends()`

- [ ] **Step 9: Delete `handlers/finance.rs` and verify**

```bash
rm crates/app-core/src/handlers/finance.rs
cargo check -p app-core
```

Expected: compiles cleanly.

- [ ] **Step 10: Commit**

```bash
git add crates/app-core/src/handlers/finance/
git rm crates/app-core/src/handlers/finance.rs
git commit -m "refactor(app-core): split handlers/finance.rs into finance/ subdirectory"
```

---

### Task 7: Split `handlers/productivity.rs`

**Context:** `productivity.rs` (983 lines). Sections: converter functions (pub), AppCore methods for summaries, focus/pomodoro/breaks, auto-focus, categories/tracking, goals, projects, calendar, weekly assessment.

**Files:**
- Create: `crates/app-core/src/handlers/productivity/mod.rs`
- Create: `crates/app-core/src/handlers/productivity/converters.rs`
- Create: `crates/app-core/src/handlers/productivity/summaries.rs`
- Create: `crates/app-core/src/handlers/productivity/focus.rs`
- Create: `crates/app-core/src/handlers/productivity/tracking.rs`
- Create: `crates/app-core/src/handlers/productivity/calendar.rs`
- Delete: `crates/app-core/src/handlers/productivity.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p crates/app-core/src/handlers/productivity
```

- [ ] **Step 2: Create `handlers/productivity/converters.rs`**

Move verbatim — these are used both internally and re-exported for desktop.
**Visibility note:** Functions called by sibling sub-modules (`tracking.rs`, `calendar.rs`) must be
at least `pub(super)` — plain private `fn` cannot be imported across module boundaries.
- `rules_to_response()` — `pub(super) fn` (called by `tracking.rs`)
- `rules_from_response()` — `pub(super) fn` (called by `tracking.rs`)
- `summary_to_response()` — `pub fn` (used by desktop commands)
- `session_to_response()` — `pub fn`
- `project_to_response()` — `pub fn`
- `assessment_to_response()` — `pub(super) fn` (called by `calendar.rs`)
- `insight_to_response()` — `pub fn`
- `event_to_timeline()` — `pub fn`

- [ ] **Step 3: Create `handlers/productivity/summaries.rs`**

Move verbatim with `use super::converters::*;`:
- `productivity_today()`
- `productivity_timeline()`
- `productivity_weekly()`
- `productivity_summary_range()`
- `productivity_activity_feed()`

- [ ] **Step 4: Create `handlers/productivity/focus.rs`**

Move verbatim:
- `productivity_focus_start()`
- `productivity_focus_end()`
- `productivity_focus_status()`
- `productivity_sessions()`
- `productivity_intelligence_sessions()`
- `productivity_pomodoro_start()`
- `productivity_pomodoro_start_with_action()`
- `productivity_break_start()`
- `productivity_break_end()`
- `productivity_auto_focus_start()`
- `productivity_auto_focus_end()`

- [ ] **Step 5: Create `handlers/productivity/tracking.rs`**

Move verbatim:
- `productivity_categories()`
- `productivity_tracked_apps()`
- `productivity_goals()`
- `productivity_time_entries()`
- `productivity_goal_create()`
- `productivity_goal_delete()`
- `productivity_goal_toggle()`
- `productivity_time_entry_create()`
- `productivity_time_entry_delete()`
- `productivity_category_upsert()`
- `productivity_category_delete()`
- `productivity_recategorize_app()`
- `productivity_insights()`
- `productivity_insight_dismiss()`
- `productivity_projects_list()`
- `productivity_project_upsert()`
- `productivity_project_delete()`

- [ ] **Step 6: Create `handlers/productivity/calendar.rs`**

Move verbatim:
- `productivity_calendar_events()`
- `calendar_sync_events()`
- `productivity_weekly_assessment()`

- [ ] **Step 7: Create `handlers/productivity/mod.rs`**

```rust
pub mod converters;
mod calendar;
mod focus;
mod summaries;
mod tracking;

// Re-export pub converter fns that desktop commands import
pub use converters::{
    event_to_timeline,
    insight_to_response,
    project_to_response,
    session_to_response,
    summary_to_response,
};
```

- [ ] **Step 8: Delete `handlers/productivity.rs` and verify**

```bash
rm crates/app-core/src/handlers/productivity.rs
cargo check -p app-core
```

Expected: compiles cleanly. If `desktop` crate imports these converters via `app_core::handlers::productivity::*`, check that `handlers/mod.rs` re-exports or that paths still resolve.

- [ ] **Step 9: Commit**

```bash
git add crates/app-core/src/handlers/productivity/
git rm crates/app-core/src/handlers/productivity.rs
git commit -m "refactor(app-core): split handlers/productivity.rs into productivity/ subdirectory"
```

---

### Task 8: Split `handlers/chat.rs`

**Context:** `chat.rs` (1,114 lines) uses a "free functions + thin `impl AppCore` wrappers" pattern:
all business logic lives in top-level `pub async fn chat_xxx(repos, ...)` free functions; the
`impl AppCore` methods (lines 492+) are thin wrappers that pull out `&self.repos` and delegate.
Private helper free functions (`format_interaction_summary`, `tool_domain`, `entity_kind_for_tool`,
`is_mutating_action`, `entity_kind_for`, `auto_detect_context`, `resolve_ancestry`) support them.
Each free function must be co-located in the same sub-file as its `impl AppCore` caller.

**Files:**
- Create: `crates/app-core/src/handlers/chat/mod.rs`
- Create: `crates/app-core/src/handlers/chat/streaming.rs`
- Create: `crates/app-core/src/handlers/chat/sessions.rs`  ← may be minimal; see Step 4
- Create: `crates/app-core/src/handlers/chat/threads.rs`
- Delete: `crates/app-core/src/handlers/chat.rs`

- [ ] **Step 1: Read `chat.rs` in full before starting**

Read `crates/app-core/src/handlers/chat.rs` completely. Make two lists:

**A. Free functions** (not inside `impl AppCore`) — identify every `fn`, `async fn`, `pub async fn`
at module level. For each, determine which `impl AppCore` method calls it.

**B. `impl AppCore` methods** — the thin wrappers (lines 492+). Group by target file:
- `streaming.rs`: `chat_send`, `chat_cancel`, `spawn_chat_relay`
- `threads.rs`: `chat_threads`, `chat_messages`, `chat_pin_thread`, `chat_rename_thread`,
  `chat_delete_thread`, `chat_respond_interaction`
- `sessions.rs`: any session CRUD methods not in the above two groups (e.g. `session_list`,
  `session_get`, `session_delete`, `session_update`); if none exist, `sessions.rs` can be empty
  or hold only the `mod` declarations

Assign each free function to the same file as the `impl AppCore` method that calls it.

- [ ] **Step 2: Create directory**

```bash
mkdir -p crates/app-core/src/handlers/chat
```

- [ ] **Step 3: Create `handlers/chat/streaming.rs`**

Move verbatim — the `impl AppCore` streaming methods and all free functions they call:
- `ChatStreamInfo` struct
- `ActiveStreams` and `PendingInteractions` type aliases
- Private helpers called by streaming code:
  `format_interaction_summary`, `tool_domain`, `entity_kind_for_tool`, `is_mutating_action`,
  `entity_kind_for`, `auto_detect_context` — and any others found in Step 1
- Free functions: `chat_send`, `chat_cancel`, `chat_respond_interaction`, `relay_chat_stream`
- `impl AppCore` methods: `chat_send`, `chat_cancel`, `chat_respond_interaction`, `spawn_chat_relay`

- [ ] **Step 4: Create `handlers/chat/sessions.rs`**

If the Step 1 read found session CRUD `impl AppCore` methods (`session_list`, `session_get`, etc.),
move them and their free-function delegates verbatim here.

If no dedicated session CRUD methods exist in `chat.rs`, create `sessions.rs` as an empty file
with just a comment: `// Session CRUD (none in this file — managed via thread operations above)`.

- [ ] **Step 5: Create `handlers/chat/threads.rs`**

Move verbatim — the `impl AppCore` thread methods and all free functions they call:
- Free functions: `chat_threads`, `chat_messages`, `chat_pin_thread`, `chat_rename_thread`,
  `chat_delete_thread`, `resolve_ancestry` (private, used by thread queries)
- `impl AppCore` methods: `chat_threads`, `chat_messages`, `chat_pin_thread`,
  `chat_rename_thread`, `chat_delete_thread`

If any private helper is called from both `streaming.rs` and `threads.rs`, place it in
`streaming.rs` (earlier in file) and declare it `pub(super)` so `threads.rs` can import it.

- [ ] **Step 6: Create `handlers/chat/mod.rs`**

```rust
mod sessions;
mod streaming;
mod threads;

pub use streaming::ChatStreamInfo;
```

- [ ] **Step 7: Update `handlers/mod.rs`**

If `handlers/mod.rs` currently exports `ChatStreamInfo` via `pub use chat::ChatStreamInfo;`,
verify this still works after the split (it should, since `chat/mod.rs` re-exports it).

- [ ] **Step 8: Delete `handlers/chat.rs` and verify**

```bash
rm crates/app-core/src/handlers/chat.rs
cargo check -p app-core
cargo check --workspace
```

Expected: compiles cleanly across the workspace. `desktop` crate uses `ChatStreamInfo` — ensure
its import path still resolves.

- [ ] **Step 9: Commit**

```bash
git add crates/app-core/src/handlers/chat/
git rm crates/app-core/src/handlers/chat.rs
git commit -m "refactor(app-core): split handlers/chat.rs into chat/ subdirectory"
```

---

### Task 9: Split `handlers/settings.rs`

**Context:** `settings.rs` (445 lines). Contains MCP server CRUD + helper functions + generic config section handlers + `deep_merge` + a `#[cfg(test)]` module with 9 tests.

**Files:**
- Create: `crates/app-core/src/handlers/settings/mod.rs`
- Create: `crates/app-core/src/handlers/settings/mcp.rs`
- Create: `crates/app-core/src/handlers/settings/config.rs`
- Delete: `crates/app-core/src/handlers/settings.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p crates/app-core/src/handlers/settings
```

- [ ] **Step 2: Create `handlers/settings/mcp.rs`**

Move verbatim with all needed `use` imports:
- `server_to_response()` — `pub` helper
- `build_mcp_response()` — `pub` helper
- `find_server_mut()` — `pub` helper
- `build_transport()` — `pub` helper
- `mcp_get_config()`
- `mcp_add_server()`
- `mcp_remove_server()`
- `mcp_toggle_server()`
- `mcp_update_server()`

- [ ] **Step 3: Create `handlers/settings/config.rs`**

Move verbatim with all needed `use` imports:
- `deep_merge()` — private fn
- `app_info()`
- `config_get_section()`
- `config_update_section()`
- `config_mark_setup_completed()`
- The entire `#[cfg(test)] mod tests { ... }` block — copy verbatim including all 9 test functions

- [ ] **Step 4: Create `handlers/settings/mod.rs`**

```rust
pub mod config;
pub mod mcp;
```

- [ ] **Step 5: Delete `handlers/settings.rs` and verify**

```bash
rm crates/app-core/src/handlers/settings.rs
cargo check -p app-core
cargo nextest run -p app-core
```

Expected: compiles cleanly; all 9 `deep_merge` tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/handlers/settings/
git rm crates/app-core/src/handlers/settings.rs
git commit -m "refactor(app-core): split handlers/settings.rs into settings/ subdirectory"
```

---

## Chunk 4: Final Cleanup + Verification

---

### Task 10: Update `lib.rs` and full workspace verification

**Context:** Add the `infrastructure` module to `lib.rs` exports, optionally add `AppEventEmitter` top-level re-export, and run the full verification suite across the entire workspace.

**Files:**
- Modify: `crates/app-core/src/lib.rs`

- [ ] **Step 1: Finalize `lib.rs`**

Update `crates/app-core/src/lib.rs` to its final form:

```rust
pub mod errors;
pub mod events;
pub mod handlers;
pub mod infrastructure;
pub mod init;
pub mod state;

// AppEventEmitter: additive top-level re-export (not currently at crate root)
pub use events::AppEventEmitter;
pub use init::EventChannels;
pub use state::{AppCore, EntityUpdate, HandlerResult};
```

- [ ] **Step 2: `cargo check --workspace`**

```bash
cargo check --workspace
```

Expected: all 27 crates compile cleanly.

- [ ] **Step 3: `cargo clippy --workspace --all-targets --all-features`**

```bash
cargo clippy --workspace --all-targets --all-features
```

Expected: zero new warnings introduced by this refactor. If any `dead_code` warnings appear on moved functions, add `#[allow(dead_code)]` if they were already present before the refactor; otherwise fix the visibility.

- [ ] **Step 4: `cargo nextest run --workspace`**

```bash
cargo nextest run --workspace
```

Expected: all tests pass. This is the definitive proof that no logic was changed.

- [ ] **Step 5: `cargo test --workspace --doc`**

```bash
cargo test --workspace --doc
```

Expected: all doctests pass.

- [ ] **Step 6: Verify `handlers/mod.rs` is complete**

Read `crates/app-core/src/handlers/mod.rs` and verify it declares `pub mod` for every handler — both the unchanged flat files and all new subdirectories:

```rust
pub mod areas;
pub mod capture;
pub mod chat;          // now a directory
pub mod coaching;
pub mod cognitive;     // now a directory
pub mod columns;
pub mod cron;
pub mod distraction;
pub mod entity_links;
pub mod finance;       // now a directory
pub mod groups;
pub mod key_results;
pub mod notes;         // now a directory
pub mod objectives;
pub mod productivity;  // now a directory
pub mod project_conversations;
pub mod project_memories;
pub mod project_sources;
pub mod projects;
pub mod settings;      // now a directory
pub mod status;
pub mod tasks;         // now a directory
pub mod timeline;
pub mod work_context;
pub mod workflows;
```

- [ ] **Step 7: Final commit**

```bash
git add crates/app-core/src/lib.rs crates/app-core/src/handlers/mod.rs
git commit -m "refactor(app-core): finalize lib.rs + handlers/mod.rs after phase 11 split"
```

- [ ] **Step 8: Verify file count**

```bash
find crates/app-core/src -name "*.rs" | wc -l
```

Expected: significantly more files than the original 33 (each split task adds ~3-6 files), but all previously deleted files are gone. The exact count will be around 60-65 files.

---

## Summary of Parallel Execution

When using `superpowers:subagent-driven-development`:

1. **Sequential:** Execute Task 1 (infrastructure move) first — it's a prerequisite for all path updates.
2. **Parallel:** Dispatch Tasks 2–9 simultaneously (8 independent tasks, no shared files between them).
3. **Sequential:** Execute Task 10 (final verification) after all parallel tasks complete.
