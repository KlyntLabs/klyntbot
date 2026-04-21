# Feature Layer Architecture

The feature layer (workspace layer L4) provides klyntbot's domain capabilities as self-contained packages. The `FeaturePackage` trait is defined in L1 (`tools-core`), but all feature crate implementations live in L4. Each feature crate owns its tools, storage migrations, configuration, and health checks. The agent discovers and registers features at startup.

Related docs: [agent-runtime.md](agent-runtime.md), [core-infrastructure.md](core-infrastructure.md)

---

## Feature Package Pattern

Every feature crate exports a struct implementing `FeaturePackage` (defined in `crates/tools-core/src/feature.rs`):

```rust
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<DynTool>;
    fn migrations(&self) -> Vec<FeatureMigration>;
    fn config_key(&self) -> &str;
    fn default_config(&self) -> Value;
    async fn health_check(&self) -> Result<HealthStatus>;  // default: Healthy
}
```

Each method serves a specific role:

| Method | Purpose |
|---|---|
| `name()` | Unique identifier used in logs and the `_feature_migrations` table |
| `tools()` | Returns `Vec<DynTool>` -- zero or more tools registered into `ToolRegistry` |
| `migrations()` | Ordered `FeatureMigration` list -- applied idempotently via `StoragePool::run_feature_migrations()` |
| `config_key()` | Key under which this feature's config lives in `config.json` (camelCase) |
| `default_config()` | Merged when the config section is missing -- ensures feature works out of the box |
| `health_check()` | Returns `Healthy`, `Degraded(reason)`, or `Unhealthy(reason)` |

### Migrations

Each `FeatureMigration` carries a `feature_name`, `version`, `description`, and raw `sql`. The storage layer tracks applied versions in `_feature_migrations` -- already-applied migrations are skipped. Pre-release, migrations are consolidated in-place (single version bump, rewritten SQL) rather than appended incrementally.

Migration SQL is embedded at compile time via `include_str!("../migrations/*.sql")`.

### Tool Trait

Tools implement `Tool` from `crates/tools-core/src/lib.rs`:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;              // JSON Schema
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
    fn permission_level(&self) -> PermissionLevel;  // default: Standard
    fn metadata(&self) -> ToolMetadata;             // category, tags, cost hints
}
```

Most feature tools are **multi-action**: a single tool struct dispatches dozens of actions via an `action` parameter in the JSON schema. This keeps the tool count low for the LLM while providing broad functionality.

For typed tool implementations, the `ToolExecute` trait + `#[derive(Tool)]` macro generates the `Tool` bridge automatically from a typed `Params` struct.

### Tool Wiring Exception

Not all feature tools flow through `FeaturePackage::tools()`. Some tools require handler injection that depends on higher-layer crates (agent, context engine). These are wired directly in `crates/agent/src/agent_loop/builder.rs`:

- `TaskTool` -- needs `EmbeddingHandler`, `ProgressHandler`, alarm writers, and a domain event bus
- `ProductivityTool` -- needs `FocusManager`, `DailyAggregator`
- `LearningTool` -- needs `LearningHandler`

Their `FeaturePackage::tools()` returns an empty vec. The package still provides migrations, config, and health checks.

---

## Handler Injection

Feature crates at L4 cannot depend on the agent crate at L5. When a tool needs LLM calls, embedding generation, or other agent-level capabilities, the feature crate defines a handler trait and the agent crate implements it.

```
feature-tasks (L4)                    agent (L5)
--------------------                  -------------------
trait EmbeddingHandler  <-----------  struct EmbeddingHandlerImpl
  async fn embed(text)                  impl EmbeddingHandler
                                          calls embedding service

TaskTool::new()
  .with_embedding_handler(Arc<dyn EmbeddingHandler>)
```

The builder in `agent_loop/builder.rs` constructs handler implementations and injects them as `Arc<dyn Trait>` via builder methods.

This pattern appears across multiple features:

| Feature | Handler Trait | Agent Provides |
|---|---|---|
| feature-tasks | `EmbeddingHandler` | Vector embedding generation + LanceDB storage |
| feature-tasks | `ProgressHandler` | Cascading KR progress updates on task complete |
| feature-finance | `FinanceHandler` | Proactive financial analysis |
| feature-coaching | `CoachingReasonerHandler` | LLM coaching decision pipeline |
| tools | `LearningHandler` | Strategy retrieval + adaptive thresholds |

Handler traits live in the feature crate's `handlers/` module (e.g., `crates/feature-tasks/src/handlers/mod.rs`). All use `#[async_trait]` and require `Send + Sync`.

---

## Integration Flow

```
Startup (builder.rs)
    |
    v
FeaturePackage::migrations() --> StoragePool::run_feature_migrations()
    |
    v
FeaturePackage::tools()  ──┐
Direct tool construction ──┤──> ToolRegistry::register()
MCP tools ─────────────────┘
    |
    v
ToolRegistry (Arc<RwLock<ToolRegistry>>)
    |
    v
ExecutionCore::execute() --> Tool::execute(args, RoutingContext) --> Result<String>
    |
    v
DomainEventBus (cross-feature side effects)
```

Tools return plain `String` results. The `ToolOutput` enum provides an opt-in upgrade path for structured responses (`summary` + `data` JSON), detected via a `__STRUCTURED__` prefix convention.

---

## Feature Reference

### feature-tasks -- Task Management

Task CRUD + search + recurrence + focus + alarms. `TaskTool` is wired directly in the builder (not via `FeaturePackage::tools()`) because it needs an embedding handler, a progress handler, an alarm writer, and the domain event bus.

| | |
|---|---|
| **Crate** | `crates/feature-tasks` |
| **Tool name** | `tasks` |
| **Key types** | `Task`, `TaskActivity`, `Attachment`, `TimeEntry` |
| **Config struct** | `TasksConfig` |
| **Storage tables** | tasks, task_activities, task_attachments, task_dependencies, task_estimations, time_entries, task_alarms |

**Action groups:**

| Group | Actions |
|---|---|
| CRUD | create, update, complete, reopen, delete, show, list, summary, tree |
| Search | search (FTS + semantic hybrid with RRF ranking) |
| Focus | focus, unfocus, log_time |
| Dependencies | add_dep, remove_dep |
| Batch | batch (batch create/update) |
| Recurrence | recur, list_recurring, delete_recurring |

**Philosophy:** The task tool is a pure CRUD + scoring surface. LLM-driven behaviors (daily planning, decomposition, forecasting, proactive suggestions, auto-enrichment) were removed in April 2026 — users now compose those via cron + skills + the `agent` tool. Pure-math helpers (`scoring::calculate_score`, estimation history) remain for non-LLM ranking. `TaskCreated` / `TaskCompleted` events continue to publish so cognitive and reforge receive task signals.

**Handler traits** (defined in `crates/feature-tasks/src/handlers/`):

- `EmbeddingHandler` -- generates vector embeddings on create/update for semantic search via LanceDB.
- `ProgressHandler` -- cascades key-result progress updates when a task completes.

---

### feature-finance -- Personal Finance and FIRE

| | |
|---|---|
| **Crate** | `crates/feature-finance` |
| **Tool name** | `finance` |
| **Actions** | 55+ |
| **Key types** | `AccountType`, `TransactionType`, `BudgetMethod`, `AssetType`, `GoalType`, `LiabilityType`, `InvestmentTxType`, `JarType` |
| **Config struct** | `FinanceConfig` |
| **Storage tables** | accounts, transactions, budgets, portfolios, investments, investment_txs, goals, liabilities |

**Action groups:**

| Group | Actions |
|---|---|
| Accounts (4) | account_add, account_list, account_update, account_delete |
| Transactions (6) | tx_add, tx_list, tx_update, tx_delete, tx_search, tx_recurring_add |
| Budgets (5) | budget_create, budget_list, budget_status, budget_update, budget_delete |
| Investments (14) | portfolio_create/list/delete, investment_add/update/delete, investment_tx, investment_summary, price_fetch/refresh, portfolio_drift/rebalance/returns/correlation |
| Goals (6) | goal_create, goal_list, goal_update, goal_delete, goal_fire, goal_whatif |
| Liabilities (5) | liability_add/list/update/delete, net_worth |
| Reports (5) | report_spending, report_income, report_trends, report_net_worth_history, daily_review |
| Analysis (4) | analyze_spending_anomalies, analyze_spending_trends, analyze_recurring_charges, analyze_category_correlation |
| FIRE Planning (7) | fire_traditional, fire_coast, fire_lean, fire_fat, fire_withdrawal_sim, fire_backtest, fire_sensitivity |
| Allocations (3) | allocation_target_set/list/delete |
| Snapshots (2) | snapshot_record, snapshot_history |
| Settings (2) | settings_get, settings_update |
| Health (1) | finance_health_check |

**Budget methods:** 50/30/20, Zero-Based, Envelope, Flow (jar-based).

**FIRE simulations** use the `analytics` crate for pure Monte Carlo simulations with configurable random seeds, standard deviations, and withdrawal strategies. Backtest mode replays historical return sequences.

**Account types:** Bank, Cash, E-wallet, Crypto Wallet, Brokerage, Other.

Amounts are stored in smallest currency units (cents, dong) as integers. `PriceService` handles live market data; `RateCache` provides two-layer exchange rate caching (in-memory + SQLite).

---

### feature-notes -- Knowledge System

| | |
|---|---|
| **Crate** | `crates/feature-notes` |
| **Tool name** | `notes` |
| **Actions** | 19 |
| **Key types** | `Note`, `Notebook`, `NoteVersion`, `EntityMention`, `NoteLink` |
| **Config struct** | inline JSON (`maxVersionsPerNote`, `versionCooldownMinutes`) |
| **Storage tables** | notebooks, notes, tags, note_links, entity_mentions, note_versions |

**Actions:** create_note, get_note, update_note, delete_note, list_notes, search_notes, tag_note, link_notes, create_notebook, list_notebooks, delete_notebook, update_notebook, archive_note, unarchive_note, list_archived, get_backlinks, capture_inbox, list_inbox, delete_inbox_item.

**Capabilities:**
- Note versioning with configurable max versions and cooldown period
- Entity mention extraction from note body
- Inter-note linking with backlink traversal
- Notebook containers for organization
- Inbox capture for quick notes
- Full-text search
- Front matter parsing (`crates/feature-notes/src/front_matter.rs`)
- Link parsing for wiki-style `[[references]]` (`crates/feature-notes/src/link_parser.rs`)

---

### feature-productivity -- Focus and Activity Analytics

| | |
|---|---|
| **Crate** | `crates/feature-productivity` |
| **Tool name** | `productivity` |
| **Actions** | 17 |
| **Key types** | `FocusSession`, `ActivityRecord`, `ProductivityGoal`, `DailySummary`, `ActivityCategory` |
| **Config struct** | `ProductivityConfig` |
| **Storage tables** | productivity tables (activity records, focus sessions, goals, daily summaries, categories) |

**Actions:** focus_start, focus_end, focus_status, pomodoro_start, activity_today, activity_summary, activity_week, activity_score, activity_compare, set_goal, check_goals, list_goals, remove_goal, log_time, activity_export, list_categories, set_category.

**Internal services** (in `crates/feature-productivity/src/`):

| Service | Role |
|---|---|
| `FocusManager` | Session lifecycle, deadline integration, Pomodoro mode |
| `DailyAggregator` | Hourly + daily metric rollups |
| `ProductivityPatternAnalyzer` | Weekly/monthly trend detection |
| `NudgeService` | Contextual nudge generation |
| `ProductivityEngine` | Orchestrates tracker, aggregator, intelligence |
| `DistractionAnalyzer` | Context-aware distraction pattern detection |

**Category types:** `productive`, `neutral`, `distracting` -- assigned per-app via bundle ID or URL pattern matching.

---

### feature-coaching -- Behavioral Intelligence

| | |
|---|---|
| **Crate** | `crates/feature-coaching` |
| **Tool name** | None (service-only, no direct tool) |
| **Key types** | `CoachingDecision`, `TriggerCondition`, `InterventionChannel`, `PendingBehavioral` |

Coaching is a background service, not a user-facing tool. It runs a signal-driven pipeline:

```
Signal Accumulation --> Pattern Detection --> LLM Reasoning --> Intervention Routing --> Feedback Loop
```

**Components:**

| Component | Role |
|---|---|
| `SignalAccumulator` | Collects behavioral signals with configurable decay |
| `PatternDetector` | Detects patterns from accumulated signals, evaluates `TriggerCondition` thresholds |
| `CoachingReasonerHandler` | Trait for LLM-powered coaching decisions (implemented in agent crate) |
| `InterventionRouter` | Routes decisions to channels: notes, chat messages, reminders |
| `FeedbackTracker` | Tracks `PendingBehavioral` items, closes the feedback loop |
| `CoachingService` | Background service orchestrating the pipeline with cancellation token |

Does not implement `FeaturePackage`. The `CoachingService` is started directly by the agent builder.

---

### feature-insights -- Cross-Domain Analysis

| | |
|---|---|
| **Crate** | `crates/feature-insights` |
| **Tool name** | None (service-only, consumed by other features) |
| **Key types** | `InsightContent`, `ScopeConfig`, `ScopeType` |

Provides `InsightService` for generating versioned insight reviews across notes, tasks, and cognitive memory.

**Components:**

| Component | Role |
|---|---|
| `InsightService` | Orchestrates insight generation with scope resolution |
| `SmartMergeEngine` | Detects and merges duplicate/overlapping insights |
| `ProgressComputer` | Tracks learning progress across insight versions |
| `PromptBuilder` | Assembles LLM prompts from scoped context |
| `InsightReviewRepo` | Persistence for versioned reviews |
| `ScopeResolver` | Trait for resolving scope configs to note/entity sets |

**Scope types:** Backlinks, Semantic, Project, Manual, Notebook (recursive).

**Insight content tabs:** synthesis, gap_analysis, self_assessment, concept_map, perspectives.

Does not implement `FeaturePackage`. Used as a library by other features and the nightly batch pipeline.

---

### feature-learning -- Flashcard Generation

| | |
|---|---|
| **Crate** | `crates/feature-learning` |
| **Tool name** | `learning` (via `LearningTool` in the `tools` crate) |
| **Key types** | `GeneratedCard`, `CardGenerationContext` |

A small library crate providing flashcard generation from notes and documents:

- `build_generation_prompt()` -- assembles LLM prompt from source content + existing cards
- `parse_generated_cards()` -- parses LLM output into structured `GeneratedCard` items
- `summarize_existing_cards()` -- deduplication context for the generation prompt

Does not implement `FeaturePackage`. The `LearningTool` in the `tools` crate wraps this and is wired in the builder with a `LearningHandler`.

---

### feature-language-learning -- Pronunciation and Practice

| | |
|---|---|
| **Crate** | `crates/feature-language-learning` |
| **Tool name** | `language_practice` |
| **Actions** | 5 |
| **Key types** | `PhonemeMastery`, `PronunciationLog`, `ExamAttempt` |
| **Config struct** | inline JSON (`enabled`, `feedback.defaultLevel`, `feedback.escalationThreshold`) |
| **Storage tables** | phoneme_mastery, pronunciation_logs, exam_attempts |

**Actions:** start_session, end_session, get_feedback, get_weak_phonemes, log_exam.

Implements `FeaturePackage`. Disabled by default (`"enabled": false`). Tracks phoneme-level mastery with escalation thresholds for feedback detail.

---

### feature-launcher -- Application Launcher

| | |
|---|---|
| **Crate** | `crates/feature-launcher` |
| **Tool name** | None (tools returned via `FeaturePackage::tools()` is empty) |
| **Key types** | `ClipboardEntry`, `LaunchFrequency` |
| **Config struct** | inline JSON (`enabled`, `clipboardHistoryEnabled`, `clipboardMaxEntries`, `scriptsDir`) |
| **Storage tables** | launcher frequencies, clipboard_history (with FTS5 index) |

**Components:**

| Component | Role |
|---|---|
| `ClipboardMonitor` | Watches clipboard changes, stores history |
| `WindowManager` | Application window management |
| FTS5 search | Full-text search over clipboard history |
| Frequency tracking | Tracks app launch frequencies for ranking |

Implements `FeaturePackage` but returns no tools -- it provides migrations and config only. Clipboard and window management are consumed by the desktop app layer directly.

---

## Summary Table

| Feature | Crate | Tool Name | Actions | FeaturePackage | Handler Injection |
|---|---|---|---|---|---|
| Tasks | feature-tasks | `tasks` | 30 | Yes (migrations/config only) | 9 handler traits |
| Finance | feature-finance | `finance` | 55+ | Yes (full) | `FinanceHandler` |
| Notes | feature-notes | `notes` | 19 | Yes (full) | None |
| Productivity | feature-productivity | `productivity` | 17 | Yes (migrations/config only) | Via constructor args |
| Coaching | feature-coaching | -- | -- | No | `CoachingReasonerHandler` |
| Insights | feature-insights | -- | -- | No | `ScopeResolver` trait |
| Learning | feature-learning | `learning`* | -- | No | `LearningHandler` |
| Language Learning | feature-language-learning | `language_practice` | 5 | Yes (full) | None |
| Launcher | feature-launcher | -- | -- | Yes (migrations/config only) | None |

\* `LearningTool` lives in the `tools` crate, not `feature-learning`. The feature crate is a library providing prompt building and card parsing.

---

## Adding a New Feature

1. **Create the crate** -- `crates/feature-<name>/` with `src/lib.rs` exporting a struct implementing `FeaturePackage`.
2. **Write migrations** -- `migrations/*.sql` embedded via `include_str!`. Use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` for idempotency.
3. **Implement the tool** -- multi-action pattern with `action` enum parameter. Implement `Tool` directly or use `#[derive(Tool)]` + `ToolExecute`.
4. **Define handler traits** -- if LLM or embedding capabilities are needed, define `#[async_trait] pub trait MyHandler: Send + Sync` in the feature crate.
5. **Wire in builder** -- in `crates/agent/src/agent_loop/builder.rs`:
   - Call `StoragePool::run_feature_migrations()` with the feature's migrations.
   - Construct the tool with injected handlers.
   - Call `tool_registry.register(tool)`.
6. **Add config** -- add the config struct with `#[serde(rename_all = "camelCase")]`. The `default_config()` return ensures the feature works without user configuration.
7. **Gate behind config flag** -- wrap registration in `if config.<feature>.enabled { ... }` for optional features.

---

*Related docs: [Agent Runtime](agent-runtime.md) | [Core Infrastructure](core-infrastructure.md) | [Context Engine](context-engine.md) | [Channels](channels.md)*
