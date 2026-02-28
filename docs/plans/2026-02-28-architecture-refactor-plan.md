# Architecture Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce codebase by ~4,650 LOC and improve long-term maintainability via crate merges, DI simplification, derive macros, and deduplication.

**Architecture:** Targeted restructuring — merge 3 thin crates (22→19), remove 4 thin handler traits, build `#[derive(Tool)]` macro, unify finance across 3 layers, consolidate agent internals, extract shared channel utilities.

**Tech Stack:** Rust, sqlx, tokio, proc_macro2/syn/quote (for derive macro), serde_json

**Design doc:** `docs/plans/2026-02-28-architecture-refactor-design.md`

---

## Phase 1: Crate Topology Changes

### Task 1: Create `domain` crate (merge `goal` + `plan`)

**Files:**
- Create: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/lib.rs`
- Create: `crates/domain/src/goal.rs`
- Create: `crates/domain/src/plan.rs`
- Move: `crates/goal/src/types.rs` → `crates/domain/src/goal.rs` (combined)
- Move: `crates/goal/src/conversions.rs` → appended into `crates/domain/src/goal.rs`
- Move: `crates/goal/src/error.rs` → appended into `crates/domain/src/goal.rs`
- Move: `crates/plan/src/types.rs` → `crates/domain/src/plan.rs` (combined)
- Move: `crates/plan/src/conversions.rs` → appended into `crates/domain/src/plan.rs`
- Move: `crates/plan/src/error.rs` → appended into `crates/domain/src/plan.rs`
- Modify: `Cargo.toml` (workspace members)
- Modify: every `Cargo.toml` that depends on `goal` or `plan`
- Delete: `crates/goal/` directory
- Delete: `crates/plan/` directory

**Step 1: Create `crates/domain/Cargo.toml`**

```toml
[package]
name = "domain"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
storage.workspace = true
thiserror.workspace = true
uuid.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
```

**Step 2: Read `crates/goal/src/` and `crates/plan/src/` contents**

Read all files in both crates to understand exact types, imports, and exports.

**Step 3: Create `crates/domain/src/goal.rs`**

Combine `goal/types.rs`, `goal/conversions.rs`, `goal/error.rs` into a single module. Keep all public types. Adjust internal imports (replace `use crate::` with local paths).

**Step 4: Create `crates/domain/src/plan.rs`**

Same approach: combine `plan/types.rs`, `plan/conversions.rs`, `plan/error.rs`.

**Step 5: Create `crates/domain/src/lib.rs`**

```rust
pub mod goal;
pub mod plan;

// Re-export key types at crate root for convenience
pub use goal::{Goal, GoalError, GoalProgress, GoalStatus};
pub use plan::{BacktrackEntry, Plan, PlanError, PlanStatus, PlanStep, PlanVisibility, StepStatus};
```

**Step 6: Update workspace `Cargo.toml`**

- Add `"crates/domain"` to `[workspace.members]`
- Add `domain = { path = "crates/domain" }` to `[workspace.dependencies]`
- Remove `"crates/goal"` and `"crates/plan"` from members
- Remove `goal` and `plan` from workspace dependencies

**Step 7: Update dependent crates' `Cargo.toml` files**

Replace `goal.workspace = true` and/or `plan.workspace = true` with `domain.workspace = true` in:
- `crates/agent/Cargo.toml`
- `crates/tools/Cargo.toml`
- `crates/storage/Cargo.toml` (if it depends on goal or plan)

**Step 8: Update all `use goal::` and `use plan::` imports**

Search-and-replace across workspace:
- `use goal::` → `use domain::goal::`  (or `use domain::` for re-exported types)
- `use plan::` → `use domain::plan::`  (or `use domain::` for re-exported types)

Files to update (from exploration):
- `crates/agent/src/context_sources/goal.rs`
- `crates/agent/src/context_sources/mod.rs`
- `crates/agent/src/goal_handler.rs`
- `crates/agent/src/intent_pipeline/engines/planned.rs`
- `crates/agent/src/intent_pipeline/types.rs`
- `crates/agent/src/plan_executor.rs`
- `crates/agent/src/plan_handler.rs`
- `crates/agent/src/plan_step_generator.rs`
- `crates/storage/src/repos/mod.rs`
- `crates/tools/src/goal_tool.rs`
- `crates/tools/src/plan_tool.rs`
- `src/lib.rs` (facade re-exports)

**Step 9: Update facade re-exports in `src/lib.rs`**

Replace `pub use goal::*` and `pub use plan::*` with `pub use domain::*` or more specific re-exports.

**Step 10: Delete old crate directories**

```bash
rm -rf crates/goal crates/plan
```

**Step 11: Build and test**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
```

**Step 12: Commit**

```bash
git add -A
git commit -m "refactor: merge goal + plan into domain crate"
```

---

### Task 2: Inline `heartbeat` into CLI

**Files:**
- Create: `crates/cli/src/heartbeat.rs`
- Modify: `crates/cli/src/serve.rs` (update import)
- Modify: `crates/cli/Cargo.toml` (remove heartbeat dep)
- Modify: `Cargo.toml` (remove from workspace)
- Modify: `src/lib.rs` (remove heartbeat re-export)
- Delete: `crates/heartbeat/` directory

**Step 1: Create `crates/cli/src/heartbeat.rs`**

Copy the full contents of `crates/heartbeat/src/service.rs` (200 LOC) into `crates/cli/src/heartbeat.rs`. Update the module declaration.

**Step 2: Add `pub mod heartbeat;` to `crates/cli/src/lib.rs` (or equivalent mod declaration)**

**Step 3: Update `crates/cli/src/serve.rs`**

```rust
// Before:
use heartbeat::HeartbeatService;

// After:
use crate::heartbeat::HeartbeatService;
```

**Step 4: Update `crates/cli/Cargo.toml`**

Remove `heartbeat.workspace = true` from `[dependencies]`.

**Step 5: Update workspace `Cargo.toml`**

- Remove `"crates/heartbeat"` from `[workspace.members]`
- Remove `heartbeat = { path = "crates/heartbeat" }` from `[workspace.dependencies]`

**Step 6: Update `src/lib.rs` facade**

Remove any `pub use heartbeat::*` re-export.

**Step 7: Delete old crate**

```bash
rm -rf crates/heartbeat
```

**Step 8: Build and test**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
```

**Step 9: Commit**

```bash
git add -A
git commit -m "refactor: inline heartbeat service into cli crate"
```

---

## Phase 2: Storage & Finance Unification

### Task 3: Create `crud_repo!` macro for finance repos

**Files:**
- Create: `crates/storage/src/macros.rs`
- Modify: `crates/storage/src/lib.rs` (add `mod macros;`)
- Modify: `crates/storage/src/repos/finance_account_repo.rs` (use macro)
- Modify: `crates/storage/src/repos/finance_budget_repo.rs` (use macro)
- Modify: `crates/storage/src/repos/finance_goal_repo.rs` (use macro)
- Modify: `crates/storage/src/repos/finance_investment_repo.rs` (use macro)
- Modify: `crates/storage/src/repos/finance_liability_repo.rs` (use macro)
- Modify: `crates/storage/src/repos/finance_transaction_repo.rs` (use macro)

**Step 1: Read all 6 finance repos to extract the common CRUD pattern**

Read each repo file and identify:
- Common struct pattern (all have `pool: SqlitePool`)
- Common methods: `new()`, `add()`, `get()`, `get_or_err()`, `update()`, `delete()`, `list()`
- Which methods are domain-specific (keep hand-written)

**Step 2: Create `crates/storage/src/macros.rs`**

Design and implement the `crud_repo!` macro. The macro should generate:
- `pub struct {Name}Repo { pool: SqlitePool }`
- `impl {Name}Repo { pub fn new(pool: SqlitePool) -> Self }`
- `pub async fn add(&self, row: &{Row}) -> Result<{Row}, StorageError>`
- `pub async fn get(&self, id: &str) -> Result<Option<{Row}>, StorageError>`
- `pub async fn get_or_err(&self, id: &str) -> Result<{Row}, StorageError>`
- `pub async fn delete(&self, id: &str) -> Result<bool, StorageError>`

The `update()` method varies too much per repo (different COALESCE patterns), so keep it hand-written. Same for `list()` which has different filters.

**Step 3: Refactor each finance repo to use the macro**

For each of the 6 repos:
1. Replace the struct definition + `new()` + basic CRUD with `crud_repo!` invocation
2. Keep domain-specific methods as `impl` blocks below the macro

**Step 4: Build and test**

```bash
cargo build --workspace
cargo nextest run -p storage
cargo nextest run --workspace
```

**Step 5: Commit**

```bash
git add crates/storage/
git commit -m "refactor(storage): extract crud_repo! macro, reduce finance repo boilerplate"
```

---

### Task 4: Create `FinanceStorage` aggregate

**Files:**
- Create: `crates/storage/src/finance_storage.rs`
- Modify: `crates/storage/src/lib.rs` (add module + re-export)
- Modify: `crates/feature-finance/src/lib.rs` or tool constructor (use aggregate)

**Step 1: Create `crates/storage/src/finance_storage.rs`**

```rust
use sqlx::SqlitePool;
use crate::repos::*;

pub struct FinanceStorage {
    pub accounts: FinanceAccountRepo,
    pub transactions: FinanceTransactionRepo,
    pub budgets: FinanceBudgetRepo,
    pub investments: FinanceInvestmentRepo,
    pub goals: FinanceGoalRepo,
    pub liabilities: FinanceLiabilityRepo,
}

impl FinanceStorage {
    pub fn from_pool(pool: &SqlitePool) -> Self {
        Self {
            accounts: FinanceAccountRepo::new(pool.clone()),
            transactions: FinanceTransactionRepo::new(pool.clone()),
            budgets: FinanceBudgetRepo::new(pool.clone()),
            investments: FinanceInvestmentRepo::new(pool.clone()),
            goals: FinanceGoalRepo::new(pool.clone()),
            liabilities: FinanceLiabilityRepo::new(pool.clone()),
        }
    }
}
```

**Step 2: Re-export from `crates/storage/src/lib.rs`**

**Step 3: Update `feature-finance` to use `FinanceStorage` instead of 6 separate repos**

Read `feature-finance` tool constructor, replace 6 repo fields with single `FinanceStorage` field.

**Step 4: Build and test**

```bash
cargo build --workspace
cargo nextest run --workspace
```

**Step 5: Commit**

```bash
git add crates/storage/ crates/feature-finance/
git commit -m "refactor(storage): add FinanceStorage aggregate, simplify feature-finance"
```

---

## Phase 3: Dependency Inversion Simplification

### Task 5: Remove thin handler traits

This task removes 4 handler traits (`AgentTaskHandler`, `PlanHandler`, `GoalHandler`, `LearningHandler`) and their impl files. Tools will access repos directly via `RoutingContext`.

**Files:**
- Modify: `crates/tools-core/src/lib.rs` (add repos to RoutingContext)
- Modify: `crates/tools/src/goal_tool.rs` (remove GoalHandler trait, use repos)
- Modify: `crates/tools/src/plan_tool.rs` (remove PlanHandler trait, use repos)
- Modify: `crates/tools/src/learning_tool.rs` (remove LearningHandler trait, use repos)
- Modify: `crates/tools/src/agent_task_tool.rs` (remove AgentTaskHandler trait, use repos)
- Delete: `crates/agent/src/goal_handler.rs`
- Delete: `crates/agent/src/plan_handler.rs`
- Delete: `crates/agent/src/learning_handler.rs`
- Delete: `crates/agent/src/agent_task_handler.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs` (remove handler wiring)
- Modify: `crates/agent/src/lib.rs` (remove handler module declarations)

**Step 1: Read current RoutingContext and understand its role**

Read `crates/tools-core/src/lib.rs:68-116` to understand current RoutingContext fields.

**Step 2: Add `Repos` to RoutingContext**

RoutingContext needs access to storage repos. Add an `Option<storage::Repos>` field (Option because some contexts don't have storage, e.g., tests).

```rust
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    pub is_direct_mode: bool,
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
    pub repos: Option<storage::Repos>,  // NEW
}
```

Note: `tools-core` is at Layer 1, `storage` is at Layer 1.5. This dependency is valid. Check if `tools-core` already depends on `storage` — if not, add the dependency.

**Step 3: Read each handler trait definition to understand the interface**

Read the trait definitions in:
- `crates/tools/src/goal_tool.rs` (GoalHandler)
- `crates/tools/src/plan_tool.rs` (PlanHandler)
- `crates/tools/src/learning_tool.rs` (LearningHandler)
- `crates/tools/src/agent_task_tool.rs` (AgentTaskHandler)

**Step 4: Read each handler implementation to understand what they delegate**

Read the impl files in:
- `crates/agent/src/goal_handler.rs`
- `crates/agent/src/plan_handler.rs`
- `crates/agent/src/learning_handler.rs`
- `crates/agent/src/agent_task_handler.rs`

**Step 5: Refactor GoalTool**

Remove `GoalHandler` trait. Replace `handler: Arc<dyn GoalHandler>` with direct repo access via `ctx.repos`. Move handler logic (if any beyond delegation) into the tool's execute methods.

**Step 6: Refactor PlanTool**

Same approach. The PlanHandler has some conversion logic — move that into PlanTool's execute methods using `domain::plan` conversions directly.

**Step 7: Refactor LearningTool**

Same approach. LearningHandler delegates to learning repos.

**Step 8: Refactor AgentTaskTool**

Same approach. AgentTaskHandler delegates to AgentTaskRepo.

**Step 9: Delete handler impl files from agent**

Remove the 4 handler files and their module declarations from `crates/agent/src/lib.rs`.

**Step 10: Update AgentLoopBuilder**

Remove `with_goal_handler()`, `with_plan_handler()`, `with_learning_handler()`, `with_agent_task_handler()` methods. Update tool registration to pass repos through RoutingContext instead.

**Step 11: Build and test**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
```

**Step 12: Commit**

```bash
git add -A
git commit -m "refactor: remove thin DI handler traits, tools access repos directly"
```

---

## Phase 4: Agent Internal Consolidation

### Task 6: Consolidate intent_pipeline classification

**Files:**
- Create: `crates/agent/src/intent_pipeline/analysis.rs`
- Delete: `crates/agent/src/intent_pipeline/heuristics.rs`
- Delete: `crates/agent/src/intent_pipeline/classifier.rs`
- Delete: `crates/agent/src/intent_pipeline/analyzer.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs`

**Step 1: Read all three files in full**

Read `heuristics.rs`, `classifier.rs`, `analyzer.rs` completely.

**Step 2: Create `analysis.rs` combining all three**

Structure:
```rust
//! Intent analysis: heuristic classification → LLM classifier fallback.

// --- Heuristic classification (from heuristics.rs) ---
pub fn analyze_heuristic(message: &str) -> Option<IntentAnalysis> { ... }
// helper functions: is_greeting, has_any_action_keyword, etc.

// --- LLM classifier (from classifier.rs) ---
pub struct IntentClassifier { ... }
impl IntentClassifier { ... }

// --- Two-stage analyzer (from analyzer.rs) ---
pub struct IntentAnalyzer { ... }
impl IntentAnalyzer { ... }
```

**Step 3: Update `mod.rs` to use `analysis` module instead of 3 separate modules**

**Step 4: Update any imports in `pipeline.rs` and `router.rs`**

**Step 5: Merge `escalation.rs` (46 LOC) into `router.rs`**

The escalation module is too small to warrant its own file — merge the escalation logic directly into the router.

**Step 6: Build and test**

```bash
cargo build --workspace
cargo nextest run -p agent
```

**Step 7: Commit**

```bash
git add crates/agent/src/intent_pipeline/
git commit -m "refactor(agent): consolidate intent classification into analysis.rs"
```

---

### Task 7: Extract shared engine logic

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/shared.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/direct.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/planned.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`

**Step 1: Read all three engine files completely**

Identify duplicated patterns: outcome matching, error handling, usage accumulation, message formatting.

**Step 2: Create `shared.rs` with extracted common logic**

Extract duplicated outcome matching and error handling into shared helper functions.

**Step 3: Simplify each engine to use shared helpers**

**Step 4: Build and test**

```bash
cargo build --workspace
cargo nextest run -p agent
```

**Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/
git commit -m "refactor(agent): extract shared engine logic, reduce duplication"
```

---

### Task 8: Consolidate learning module

**Files:**
- Create: `crates/agent/src/learning/tool_tracking.rs` (merge of tool_confidence + strategy_tracker)
- Delete: `crates/agent/src/learning/tool_confidence.rs`
- Delete: `crates/agent/src/learning/strategy_tracker.rs`
- Modify: `crates/agent/src/learning/recorder.rs` (absorb outcome_store)
- Delete: `crates/agent/src/learning/outcome_store.rs`
- Modify: `crates/agent/src/learning/mod.rs`

**Step 1: Read all 4 files to be consolidated**

Read `tool_confidence.rs`, `strategy_tracker.rs`, `recorder.rs`, `outcome_store.rs`.

**Step 2: Merge `tool_confidence.rs` + `strategy_tracker.rs` → `tool_tracking.rs`**

Both handle tool-level tracking. Combine into one cohesive module.

**Step 3: Merge `outcome_store.rs` into `recorder.rs`**

Recorder and outcome_store are closely related — recording outcomes and storing them.

**Step 4: Update `mod.rs` imports and re-exports**

**Step 5: Build and test**

```bash
cargo build --workspace
cargo nextest run -p agent
```

**Step 6: Commit**

```bash
git add crates/agent/src/learning/
git commit -m "refactor(agent): consolidate learning module from 9 to 6 files"
```

---

### Task 9: Merge small adapter files

**Files:**
- Modify: `crates/agent/src/plan_executor.rs` (absorb plan_completion_handler)
- Delete: `crates/agent/src/plan_completion_handler.rs`
- Modify: `crates/agent/src/calendar_sync_adapter.rs` (absorb todo_calendar_sync_adapter)
- Delete: `crates/agent/src/todo_calendar_sync_adapter.rs`
- Modify: `crates/agent/src/lib.rs` (remove module declarations)

**Step 1: Read `plan_completion_handler.rs` (59 LOC)**

Understand what it does and where to inline it.

**Step 2: Inline plan_completion_handler into plan_executor.rs**

**Step 3: Read `todo_calendar_sync_adapter.rs` (41 LOC)**

**Step 4: Merge into `calendar_sync_adapter.rs`**

**Step 5: Update module declarations in `lib.rs`**

**Step 6: Build and test**

```bash
cargo build --workspace
cargo nextest run -p agent
```

**Step 7: Commit**

```bash
git add crates/agent/
git commit -m "refactor(agent): merge thin adapter files into parent modules"
```

---

### Task 10: Simplify AgentLoop builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

**Step 1: Read builder.rs completely (573 LOC)**

Identify which `with_*()` methods can be removed (handler-related from Task 5) and which can be simplified.

**Step 2: Simplify by using struct defaults and reducing setter methods**

After Task 5 removes 4 handler setters, further reduce by:
- Using `Default` trait where possible
- Combining related optional fields
- Removing setters that just assign a value (use struct literal in `build()` instead)

**Step 3: Build and test**

```bash
cargo build --workspace
cargo nextest run -p agent
```

**Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "refactor(agent): simplify AgentLoop builder"
```

---

## Phase 5: `#[derive(Tool)]` Proc Macro

### Task 11: Implement `ToolParams` derive macro

**Files:**
- Modify: `crates/tools-core-macros/src/lib.rs` (add derive entry point)
- Create: `crates/tools-core-macros/src/tool_params.rs`
- Modify: `crates/tools-core-macros/Cargo.toml` (ensure syn, quote, proc-macro2)
- Modify: `crates/tools-core/src/lib.rs` (add ToolParams trait + re-export)

**Step 1: Read current tools-core-macros to understand existing patterns**

Read all files in `crates/tools-core-macros/src/`.

**Step 2: Define the `ToolParams` trait in tools-core**

```rust
pub trait ToolParams: Sized {
    fn json_schema() -> serde_json::Value;
    fn from_args(args: serde_json::Value) -> common::Result<Self>;
}
```

**Step 3: Implement `#[derive(ToolParams)]` proc macro**

The macro reads struct fields and generates:
- `json_schema()` → builds JSON schema from field types and doc comments
- `from_args()` → deserializes Value into the struct (using serde_json::from_value)
- `#[param(required)]` attribute marks required fields
- Doc comments on fields become `"description"` in the schema

**Step 4: Write tests for the derive macro**

Create test cases with various param types (String, Option<String>, bool, i64, Vec<String>).

**Step 5: Build and test**

```bash
cargo build -p tools-core-macros
cargo build -p tools-core
cargo nextest run -p tools-core
```

**Step 6: Commit**

```bash
git add crates/tools-core-macros/ crates/tools-core/
git commit -m "feat(tools-core): add #[derive(ToolParams)] macro for typed tool parameters"
```

---

### Task 12: Implement `#[derive(Tool)]` macro

**Files:**
- Modify: `crates/tools-core-macros/src/lib.rs` (add derive entry point)
- Create: `crates/tools-core-macros/src/tool_derive.rs`
- Modify: `crates/tools-core/src/lib.rs` (add ToolExecute trait + re-export)

**Step 1: Define `ToolExecute` trait in tools-core**

```rust
#[async_trait]
pub trait ToolExecute: Send + Sync {
    type Params: ToolParams;
    async fn execute(&self, params: Self::Params, ctx: &RoutingContext) -> common::Result<String>;
}
```

**Step 2: Implement `#[derive(Tool)]` proc macro**

The macro reads `#[tool(name = "...", description = "...", permission = "...")]` attributes and generates the full `Tool` trait implementation by combining with `ToolExecute`.

**Step 3: Write tests**

**Step 4: Build and test**

```bash
cargo build -p tools-core-macros
cargo build -p tools-core
cargo nextest run -p tools-core
```

**Step 5: Commit**

```bash
git add crates/tools-core-macros/ crates/tools-core/
git commit -m "feat(tools-core): add #[derive(Tool)] macro for tool metadata generation"
```

---

### Task 13: Migrate existing tools to use derive macros

**Files:**
- Modify: All tool files in `crates/tools/src/` (30+ files)
- Modify: `crates/feature-todo/src/` (TodoTool)
- Modify: `crates/feature-finance/src/` (FinanceTool)

**Step 1: Start with a simple tool (GlobTool)**

Convert `crates/tools/src/glob_tool.rs` to use `#[derive(Tool)]` + `#[derive(ToolParams)]`. Verify it compiles and tests pass.

**Step 2: Convert filesystem tools (ReadFile, WriteFile, EditFile, ListDir)**

**Step 3: Convert web tools (WebSearch, WebFetch)**

**Step 4: Convert remaining core tools (Message, Spawn, Cron, Grep)**

**Step 5: Convert domain tools (Calendar, Project, Memory, Browser, AskUser)**

**Step 6: Convert feature pack tools (TodoTool, FinanceTool)**

These are large tools with many actions — the params struct pattern applies to each action's parameter set.

**Step 7: Build and test entire workspace**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
```

**Step 8: Commit**

```bash
git add crates/tools/ crates/feature-todo/ crates/feature-finance/
git commit -m "refactor(tools): migrate all tools to #[derive(Tool)] macro"
```

---

## Phase 6: Channel Deduplication & Dashboard

### Task 14: Extract shared channel utilities

**Files:**
- Create: `crates/channels/src/shared/mod.rs`
- Create: `crates/channels/src/shared/interaction.rs`
- Create: `crates/channels/src/shared/typing.rs`
- Modify: `crates/channels/src/lib.rs` (add `pub mod shared;`)
- Modify: `crates/channels/src/telegram.rs` (use shared)
- Modify: `crates/channels/src/discord.rs` (use shared)
- Modify: `crates/channels/src/slack.rs` (use shared)

**Step 1: Read PendingCallback definitions in all 3 channels**

Read the enum definition and usage in telegram.rs, discord.rs, slack.rs.

**Step 2: Create `shared/interaction.rs`**

Extract `PendingCallback` enum and `InteractionTracker` (the `DashMap<String, PendingCallback>` pattern).

**Step 3: Create `shared/typing.rs`**

Extract `TypingIndicatorManager` from the typing indicator code in telegram.rs and discord.rs.

**Step 4: Update telegram, discord, slack to use shared utilities**

Replace inline definitions with imports from `crate::shared::`.

**Step 5: Build and test**

```bash
cargo build --workspace
cargo nextest run -p channels
```

**Step 6: Commit**

```bash
git add crates/channels/
git commit -m "refactor(channels): extract shared interaction and typing utilities"
```

---

### Task 15: Dashboard finance delegation

**Files:**
- Modify: `crates/dashboard/src/api/finance.rs` (delegate to FinanceTool)
- Modify: `crates/dashboard/src/state.rs` (add FinanceTool to AppState)
- Modify: `crates/dashboard/Cargo.toml` (add feature-finance dependency)

**Step 1: Read current `dashboard/src/api/finance.rs` (719 LOC)**

Understand which REST endpoints mirror FinanceTool actions.

**Step 2: Read `feature-finance` tool execute dispatch**

Map dashboard endpoints to tool action names.

**Step 3: Add FinanceTool (or FinanceStorage) to dashboard AppState**

**Step 4: Refactor finance.rs handlers to delegate**

For each handler, replace direct repo calls with delegation to FinanceTool::execute() or direct FinanceStorage method calls.

**Step 5: Build and test**

```bash
cargo build --workspace
cargo nextest run -p dashboard
```

**Step 6: Commit**

```bash
git add crates/dashboard/
git commit -m "refactor(dashboard): delegate finance handlers to feature-finance"
```

---

## Phase 7: Final Verification

### Task 16: Full workspace verification

**Step 1: Build**

```bash
cargo build --workspace
cargo build --workspace --release
```

**Step 2: Test**

```bash
cargo nextest run --workspace
cargo test --workspace --doc
```

**Step 3: Lint**

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

**Step 4: Verify crate count**

```bash
cargo metadata --no-deps --format-version 1 | jq '.packages | length'
# Should be 19 (down from 22)
```

**Step 5: LOC count comparison**

```bash
find crates/ src/ -name '*.rs' | xargs wc -l | tail -1
# Compare with pre-refactor baseline
```

**Step 6: Update CLAUDE.md**

Update the architecture section, crate list, layer diagram, and any references to removed crates.

**Step 7: Update docs/00-architecture-overview.md**

Update the system map, dependency layers table, and crate size distribution.

**Step 8: Final commit**

```bash
git add -A
git commit -m "docs: update architecture documentation for post-refactor state"
```

---

## Task Dependency Graph

```
Task 1 (domain crate) ──┐
Task 2 (heartbeat)  ─────┼──→ Task 5 (DI simplification) ──→ Task 10 (builder simplify)
                          │
Task 3 (CRUD macro) ──→ Task 4 (FinanceStorage) ──→ Task 15 (dashboard delegation)
                          │
Task 6 (classification) ──┤
Task 7 (shared engines) ──┼──→ Task 16 (verification)
Task 8 (learning merge) ──┤
Task 9 (adapter merge) ───┤
                          │
Task 11 (ToolParams) ──→ Task 12 (derive Tool) ──→ Task 13 (migrate tools)
                          │
Task 14 (channel utils) ──┘
```

**Parallelizable groups:**
- Group A: Tasks 1, 2 (crate topology)
- Group B: Tasks 3, 4 (storage)
- Group C: Tasks 6, 7, 8, 9 (agent consolidation — sequential within group)
- Group D: Tasks 11, 12, 13 (tool macro — sequential within group)
- Group E: Task 14 (channels — independent)
- After A+B: Task 5 (DI simplification)
- After B: Task 15 (dashboard)
- After 5: Task 10 (builder)
- After all: Task 16 (verification)
