# Architecture Refactor Design

**Date**: 2026-02-28
**Goal**: Reduce codebase size, eliminate redundancy, improve maintainability
**Approach**: Targeted Restructuring (Approach B)
**Estimated LOC reduction**: ~4,650 LOC (5% of 95k)

---

## 1. Crate Topology (22 → 19 crates)

### Merge: `heartbeat` → `agent`
- `heartbeat` is 232 LOC (timer wrapper, only used by agent)
- Becomes `agent/src/heartbeat.rs`
- Eliminate crate, Cargo.toml, and dependency chain

### Merge: `goal` + `plan` → `domain`
- Both follow identical type+conversion+error pattern (382 + 633 = 1,015 LOC)
- Structure:
  ```
  crates/domain/src/
  ├── lib.rs
  ├── goal/ (types.rs, conversions.rs, error.rs)
  └── plan/ (types.rs, conversions.rs, error.rs)
  ```
- Eliminates goal→plan cross-dependency

### Updated Layer Diagram
```
Layer 0:  common
Layer 1:  config, bus, tools-core, tools-core-macros
Layer 1.5: storage
Layer 2:  providers, domain, session, scheduling, context_engine, calendar
Layer 3:  tools, channels, feature-todo, feature-finance
Layer 4:  agent (absorbs heartbeat)
Layer 5:  cli, dashboard
Layer 6:  klyntbot (facade)
```

---

## 2. Dependency Inversion Simplification

### Remove 4 thin handler traits (-746 LOC)

| Trait | LOC | Replacement |
|-------|-----|-------------|
| `AgentTaskHandler` | 85 | Tools access `AgentTaskRepo` via `RoutingContext` |
| `PlanHandler` | 239 | Tools access `PlanRepo` + `domain` conversions directly |
| `GoalHandler` | 275 | Tools access `GoalRepo` + `domain` conversions directly |
| `LearningHandler` | 147 | Tools access learning repos directly |

### Keep 3 justified traits

| Trait | Why |
|-------|-----|
| `CalendarHandler` | Multi-provider sync, conflict resolution — genuine business logic |
| `EnrichmentHandler` | LLM-powered enrichment requiring agent context |
| `SpawnHandler` / `CronHandler` | Subagent spawning requires agent loop access |

### Mechanism
`RoutingContext` already carries `Repos`. Tools that previously used `Arc<dyn XxxHandler>` will use `ctx.repos.xxx_repo` instead. `AgentLoopBuilder` drops 4 `with_*_handler()` methods.

---

## 3. `#[derive(Tool)]` Proc Macro

### Design: Separate params struct approach

```rust
#[derive(ToolParams)]
struct ReadFileParams {
    /// File path to read
    #[param(required)]
    path: String,
    /// Optional encoding
    encoding: Option<String>,
}

#[derive(Tool)]
#[tool(name = "read_file", description = "Read contents of a file", permission = "ReadOnly")]
pub struct ReadFileTool {
    base: FsToolBase,
}

impl ToolExecute for ReadFileTool {
    type Params = ReadFileParams;
    async fn execute(&self, params: ReadFileParams, ctx: &RoutingContext) -> Result<String> {
        // params.path is typed and validated via serde
    }
}
```

### What the macro generates
- `name()` → returns `"read_file"`
- `description()` → returns the description string
- `permission_level()` → returns `PermissionLevel::ReadOnly`
- `parameters()` → builds JSON schema from `ReadFileParams` field types + doc comments
- `Tool::execute()` → deserializes `Value` into `ReadFileParams`, delegates to `ToolExecute::execute()`

### Implementation location
- `crates/tools-core-macros/` (existing crate, ~200 LOC addition)
- New traits `ToolExecute` and `ToolParams` in `crates/tools-core/`

### Impact
- 30+ tools × ~20 LOC saved = **~600 LOC net reduction** (after macro code)
- Type-safe parameter handling (compile-time errors for mismatches)
- IDE autocompletion on params struct fields

---

## 4. Storage & Finance Unification

### 4a: `crud_repo!` macro for storage repos

```rust
crud_repo!(FinanceAccountRepo, finance_accounts, FinanceAccountRow, FinanceAccountPatch);
// Generates: add(), get(), get_or_err(), update(), delete(), list()
```

- Applies to 6 finance repos (1,400 → ~600 LOC, 57% reduction)
- Domain-specific methods (balance calculations, aggregations) stay hand-written
- Located in `crates/storage/src/macros.rs`

### 4b: Dashboard delegates to feature-finance

Dashboard `finance.rs` (719 LOC) delegates to `FinanceTool::execute()` instead of reimplementing logic. Reduces to ~300 LOC of REST endpoint wiring.

### 4c: `FinanceStorage` aggregate

```rust
pub struct FinanceStorage {
    pub accounts: FinanceAccountRepo,
    pub transactions: FinanceTransactionRepo,
    pub budgets: FinanceBudgetRepo,
    pub investments: FinanceInvestmentRepo,
    pub goals: FinanceGoalRepo,
    pub liabilities: FinanceLiabilityRepo,
}
```

Feature-finance receives `FinanceStorage` instead of 6 separate repos. Decouples feature from individual repo changes.

### Total impact: ~1,200 LOC reduction

---

## 5. Agent Crate Internal Consolidation

### 5a: Absorb heartbeat (232 LOC → module)
`crates/heartbeat/` → `agent/src/heartbeat.rs`

### 5b: Remove thin handler files (-746 LOC)
Delete: `agent_task_handler.rs`, `plan_handler.rs`, `goal_handler.rs`, `learning_handler.rs`

### 5c: Consolidate intent_pipeline classification (-200 LOC)
Merge `heuristics.rs` + `classifier.rs` + `analyzer.rs` → `analysis.rs` (~700 LOC)

### 5d: Extract shared engine logic (-200 LOC)
Common outcome matching and error handling from 3 engines → `engines/shared.rs`

### 5e: Consolidate learning module (-150 LOC)
- `tool_confidence.rs` + `strategy_tracker.rs` → `tool_tracking.rs`
- `recorder.rs` + `outcome_store.rs` → `recorder.rs`
- 9 files → 6 files

### 5f: Merge small adapter files (-100 LOC)
- `plan_completion_handler.rs` → inline into plan execution
- `todo_calendar_sync_adapter.rs` → merge into `calendar_sync_adapter.rs`
- `cron_handler_adapter.rs` → simplify

### 5g: Simplify builder (-150 LOC)
Reduce `builder.rs` from 573 LOC using typed builder or default struct pattern.

### Total impact: ~1,750 LOC reduction (19.5k → ~17.7k)

---

## 6. Channel Deduplication

### Shared channel utilities

Extract into `channels/src/shared/`:

| Module | Extracts from | Purpose |
|--------|--------------|---------|
| `interaction.rs` | Discord, Slack, Telegram | `PendingCallback` enum + `InteractionTracker` |
| `typing.rs` | Discord, Telegram | `TypingIndicatorManager` |
| `http.rs` | Telegram (extend to all) | `HttpApiClient` with configurable retry |

### Impact: ~250 LOC reduction + all channels get retry logic and consistent interaction handling

---

## Summary

| Section | Change | LOC Impact |
|---------|--------|----------:|
| 1. Crate topology | 22 → 19 crates | -100 |
| 2. DI simplification | Remove 4 thin handler traits | -746 |
| 3. `#[derive(Tool)]` | Params struct + derive for 30+ tools | -600 |
| 4. Storage & finance | CRUD macro + dashboard delegation + aggregate | -1,200 |
| 5. Agent consolidation | 7 internal restructuring items | -1,750 |
| 6. Channel deduplication | Shared interaction/typing/http utilities | -250 |
| **Total** | | **~4,650** |

### Non-LOC improvements
- 19 crates (from 22) — fewer build units, simpler dependency graph
- Type-safe tool parameters — compile-time errors instead of runtime extraction failures
- Single source of truth for finance logic — feature-finance is authoritative
- Fewer abstraction layers — 4 fewer handler traits, simpler mental model
- Shared channel utilities — consistent retry, interaction, typing across platforms
