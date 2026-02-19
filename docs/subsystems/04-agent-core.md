# Subsystem Analysis: Agent Core (Layer 5)

> **Crate**: `crates/agent/` | **56 source files** | **~12,000 lines**
> **Dependencies**: common, config, bus, providers, session, tools, scheduling, calendar, goal, plan, context_engine, storage
> **Analysed**: 2026-02-19

---

## Table of Contents

1. [Overview](#1-overview)
2. [AgentLoop](#2-agentloop)
3. [Execution Core](#3-execution-core)
4. [Pipeline (v2)](#4-pipeline-v2)
5. [Orchestrator](#5-orchestrator)
6. [Context Builder](#6-context-builder)
7. [Plan Executor (Legacy)](#7-plan-executor-legacy)
8. [Learning System](#8-learning-system)
9. [Skill Manager](#9-skill-manager)
10. [Subagent Manager](#10-subagent-manager)
11. [Enrichment Engine](#11-enrichment-engine)
12. [Memory Store](#12-memory-store)
13. [Reminder Engine](#13-reminder-engine)
14. [Calendar Reconciliation & Sync](#14-calendar-reconciliation--sync)
15. [Confidence Evaluator](#15-confidence-evaluator)
16. [Handler Implementations (Dependency Inversion)](#16-handler-implementations)
17. [Output Module (CostTracker & ResponseValidator)](#17-output-module)
18. [Supporting Modules](#18-supporting-modules)
19. [Gap Analysis & Recommendations](#19-gap-analysis--recommendations)

---

## 1. Overview

The `agent` crate is the orchestration heart of klyntbot. It sits at **Layer 5** in the architecture, above tools (L3) and channels (L4), and below the CLI facade (L6-7). It wires together every other crate:

```
CLI (L7) → agent (L5) → tools (L3), providers (L2), session (L2), scheduling (L2)
                       → calendar (L2), goal (L2), plan (L2), storage (L1.5)
                       → context_engine (L2), bus (L1), config (L1), common (L0)
```

**Key responsibilities**:
- Message bus loop (receive inbound → process → send outbound)
- LLM call orchestration (multi-turn, streaming, tool execution)
- Confidence evaluation and clarification gating
- Plan creation, execution, and backtracking
- Enrichment, reminders, recurring tasks, calendar sync
- Learning from outcomes (adaptive thresholds)
- Subagent spawning and lifecycle management

**Module structure** (from `lib.rs`):

| Module | Purpose |
|--------|---------|
| `agent_loop` | Central processing engine (~2,357 lines) |
| `context` | System prompt builder with TTL caching |
| `pipeline` | 5-stage v2 processing pipeline |
| `orchestrator/` | Strategy classification (heuristic + LLM) |
| `execution/` | Execution engines (Direct, ReactPlus, PlanExecute) |
| `confidence/` | Confidence evaluation and decision gating |
| `learning/` | Outcome recording, analysis, adaptive thresholds |
| `enrichment/` | Task enrichment (priority, duration, scheduling) |
| `output/` | CostTracker and ResponseValidator |
| `chat/` | Channel-aware response formatting |
| `memory` | File-based memory store |
| `skills` | Skill discovery, loading, trigger matching |
| `subagent` | Background subagent spawning |
| `reminders` | Deterministic reminder engine |
| `recurring_tasks` | Recurring task instance spawner |
| `notifications` | Multi-target notification dispatcher |
| `events` | AgentEvent enum for streaming |
| `calendar_reconcile` | Calendar↔todo reconciliation logic |
| `calendar_sync_adapter` | Multi-provider CalDAV two-way sync |
| Handler adapters | 7 dependency-inversion implementations |

---

## 2. AgentLoop

**File**: `agent_loop.rs` (~2,357 lines)
**Struct**: `AgentLoop` with ~30 fields

### 2.1 Construction

`AgentLoop::new_with_cron()` is a ~450-line constructor that:
1. Creates `SessionManager`, `ContextBuilder`, `ConfidenceEvaluator`
2. Registers 15+ tools in `ToolRegistry` (read_file, write_file, list_dir, shell, web_search, web_scrape, message, ask_user, spawn, cron, todo, project, calendar, goal, plan, learning)
3. Wires dependency-inverted handlers:
   - `CronHandlerAdapter` → `CronTool`
   - `CalendarSyncAdapter` → `CalendarTool`
   - `GoalHandlerImpl` → `GoalTool`
   - `PlanHandlerImpl` + `PlanCompletionHandlerImpl` → `PlanTool`
   - `EnrichmentEngine` → `TodoTool`
   - `LearningHandlerImpl` → `LearningTool`
   - `ConversationEmbeddingHandlerImpl` → `ConversationEmbeddingTool`
4. Initializes `LearningService` with event bus subscriber
5. Creates `ReminderEngine`, `RecurringTaskSpawner`, `NotificationDispatcher`
6. Builds `AgentPipeline` (v2 path)

### 2.2 Message Processing

Two processing paths exist:

**Legacy path** (`run_agent_loop`):
- Max 20 iterations (50 during plan execution)
- Each iteration: LLM call → tool execution → accumulate content
- Confidence evaluation on first iteration (pre-tool)
- Streaming support via `StreamingHandle`

**Pipeline path** (v2, `process_message` → `pipeline.process_message`):
- 5-stage: Orchestrator → ContextEngine → EngineDispatch → ResponseValidator → CostTracker
- Returns `PipelineResult` with content, strategy, classification, escalations, validation

### 2.3 Plan Execution

`run_plan_execution()`:
- Drives step-by-step plan execution via `PlanExecutor::execute_step()`
- RAII guard (`PlanExecutingGuard`) clears `plan_executing` flag on all exit paths
- Backtracking: up to `MAX_BACKTRACK_ATTEMPTS` (3) via `PlanExecutor::regenerate_from()`
- Step state updates: status, timestamps, results, attempt_count
- Outcome recording per step via `OutcomeRecorder`
- Emits `AgentEvent::PlanStepCompleted` and `AgentEvent::PlanCompleted`

### 2.4 Tool Execution

`execute_tool_calls()`:
- Parallel execution via `join_all` futures
- Per-tool timeout from `ExecutionParams`
- Outcome recording for learning system
- Cache invalidation: memory, todo, goals caches cleared on relevant tool mutations
- `IterationOutcome` enum: `ToolCallsProcessed`, `FinalContent`, `Empty`

### 2.5 Streaming

`process_direct_streaming()`:
- Spawns background tokio task
- Event channel (`mpsc::Sender<AgentEvent>`) for real-time content chunks
- Interaction channel for tool approval/rejection (future use)

### 2.6 Bus Integration

`run()`:
- Listens on `inbound_rx` (from channels) with 1s poll timeout
- Dispatches `InboundMessage` to `process_message()`
- Sends responses via `outbound_tx` (to channels)
- Handles session persistence via `SessionManager`

---

## 3. Execution Core

**Directory**: `execution/`
**Files**: `mod.rs`, `core.rs`, `types.rs`, `dispatch.rs`, `direct.rs`, `react_plus.rs`, `plan_execute.rs`, `scratchpad.rs`

### 3.1 ExecutionCore (`core.rs`)

The foundational single-cycle engine:

```
ExecutionCore::run_cycle()
  1. provider.chat(messages, tools, params) → LlmResponse
  2. If tool_calls: execute all in parallel with timeout → ToolsExecuted
  3. If text content: FinalResponse
  4. Otherwise: EmptyResponse
```

- Parallel tool execution via `join_all`
- Per-tool timeout via `tokio::time::timeout`
- Returns `(CycleOutcome, Usage)`

### 3.2 CycleOutcome & Types (`types.rs`)

```rust
enum CycleOutcome {
    ToolsExecuted { results: Vec<ToolExecutionResult> },
    FinalResponse { content: String },
    EmptyResponse,
}

struct ExecutionParams {
    tool_timeout: Duration,     // default 120s
    chat_params: ChatParams,    // model, temperature, etc.
}

struct ToolExecutionResult {
    tool_call_id: String,
    tool_name: String,
    result: String,
    duration_ms: u64,
    success: bool,
}
```

### 3.3 DirectEngine (`direct.rs`)

Single LLM call with no tools. Returns:
- `DirectOutcome::Response(String)` — final answer
- `DirectOutcome::EscalateToToolAssisted` — if response suggests tool need

Escalation detection: checks for phrases like "I would need to", "I can't access", "let me search".

### 3.4 ReactPlusEngine (`react_plus.rs`)

Extended ReAct loop with scratchpad, reflection, and escalation:

```
ReflectionMode: OnFailure | EveryN(usize) | Never

ReactPlusEngine::run()
  for cycle in 0..max_cycles:
    1. Run ExecutionCore::run_cycle()
    2. Record reasoning trace in Scratchpad
    3. If reflection triggered → inject reflection prompt
    4. If >80% tools fail → EscalateToAutonomous
    5. If final response → return Response
  → MaxIterationsReached (with scratchpad summary)
```

**Scratchpad** (`scratchpad.rs`):
- Accumulates `ReasoningTrace` entries (cycle, thought, planned_actions, actual_action, reflection)
- `summarize()` caps at 20 traces
- Injected into LLM context as reasoning history

### 3.5 PlanExecuteEngine (`plan_execute.rs`)

Redesigned plan execution with full parameter generation:

```
PlanExecuteEngine::run()
  for each step in plan:
    1. Build rich context (accumulated results + remaining steps)
    2. Run ExecutionCore for the step (up to MAX_CYCLES_PER_STEP=5)
    3. Every 5 steps: inject reflection checkpoint
    4. On failure: record and continue
```

Key differences from legacy `PlanExecutor`:
- Real parameter generation (not `{}` for all tools)
- Rich step context with accumulated results
- Reflection checkpoints every 5 steps
- Integrated with the v2 pipeline

### 3.6 EngineDispatch (`dispatch.rs`)

Maps `ExecutionStrategy` to engines with escalation chain:

```
DirectResponse → DirectEngine
    ↓ escalate
ToolAssisted → ReactPlusEngine
    ↓ escalate
AutonomousTask → PlanExecuteEngine
```

- Max 2 escalations allowed
- Escalation counter prevents infinite loops

---

## 4. Pipeline (v2)

**File**: `pipeline.rs`

### 4.1 Architecture

5-stage pipeline:

```
AgentPipeline::process_message()
  1. Orchestrator.classify()           → ExecutionStrategy + confidence
  2. ContextEngine.assemble()          → token-budgeted system prompt
  3. EngineDispatch.execute()          → response content (with escalation)
  4. ResponseValidator.validate()      → safety/quality checks
  5. CostTracker.record()              → usage tracking
```

### 4.2 PipelineResult

```rust
struct PipelineResult {
    content: String,
    strategy_used: String,
    classification: Option<ClassificationResult>,
    escalations: Vec<String>,
    validation: Option<ValidationResult>,
}
```

### 4.3 Integration with AgentLoop

The pipeline is invoked from `AgentLoop::process_message()` when the v2 path is active. Falls back to legacy `run_agent_loop()` when pipeline components aren't initialized.

---

## 5. Orchestrator

**Directory**: `orchestrator/`
**Files**: `mod.rs`, `heuristics.rs`, `classifier.rs`

### 5.1 Two-Stage Classification

```
Orchestrator::classify(message)
  1. HeuristicClassifier::classify()  → Option<Classification>
  2. If None → LlmClassifier::classify()
  3. Confidence gate: <0.5 → safe fallback (ToolAssisted)
```

### 5.2 Heuristic Classifier (`heuristics.rs`)

Zero-cost pattern matching (no LLM call):

| Pattern Set | Strategy | Max Iterations |
|------------|----------|---------------|
| GREETINGS ("hi", "hello", ...) | DirectResponse | default |
| PLAN_KEYWORDS ("plan", "multi-step", ...) | AutonomousTask | 50 |
| AUTONOMOUS_KEYWORDS ("research", "investigate", ...) | AutonomousTask | 15 |
| TOOL_KEYWORDS ("read", "write", "search", ...) | ToolAssisted | 5 |
| CODE_KEYWORDS ("code", "implement", "debug", ...) | ToolAssisted | 10 |
| DIRECT_KEYWORDS ("explain", "summarize", ...) | DirectResponse | default |

**Conflict resolution**: If keywords from multiple groups match, returns `None` (defers to LLM classifier).

### 5.3 LLM Classifier (`classifier.rs`)

- 2-second timeout (falls back to ToolAssisted on timeout)
- Lightweight JSON prompt: asks LLM to classify into `direct_response`, `tool_assisted`, or `autonomous_task`
- Parses JSON from response, handles embedded JSON in text
- Fallback on parse failure: `ToolAssisted(10)`

---

## 6. Context Builder

**File**: `context.rs`

### 6.1 System Prompt Assembly

`ContextBuilder::build_system_prompt()` assembles from:

1. **Identity section** (always fresh): date/time, OS, workspace, channel, chat_id
2. **Bootstrap files** (cached once): AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md, RESPONSE.md
3. **Memory** (TTL-cached 60s): daily notes + long-term MEMORY.md
4. **Todos** (TTL-cached 60s): SQL (preferred) or JSONL fallback
5. **Goals** (TTL-cached 60s): active goals from GoalStore
6. **Confidence prompt**: threshold-parameterized instructions
7. **Skills summary**: all skill names + descriptions
8. **Always-loaded skills**: full content injected

### 6.2 Message Building

`build_messages()`:
- System prompt + up to 50 history messages + current user message
- Skill trigger matching on user message (case-insensitive substring match)
- Multipart message support (images via base64 data URLs)
- MIME type detection via `mime_guess`

### 6.3 Caching

| Cache | TTL | Invalidation |
|-------|-----|-------------|
| Bootstrap files | Permanent (until explicit invalidate) | `invalidate_cache()` |
| Memory context | 60 seconds | `invalidate_memory_cache()` |
| Todo context | 60 seconds | `invalidate_todo_cache()` |
| Goals context | 60 seconds | `invalidate_goals_cache()` |

### 6.4 Plan Context

`build_plan_context(plan)` (free function):
- Context window: current step + next 3 steps
- Markers: `>>> CURRENT`, `NEXT 1`, `NEXT 2`, `NEXT 3`
- Injected into system prompt during plan execution

---

## 7. Plan Executor (Legacy)

**File**: `plan_executor.rs`

### 7.1 Architecture

Single-cycle implementation (one LLM call per step):

```
PlanExecutor::execute_step()
  1. Build prompt from plan_context + step details
  2. Get tool definitions from registry
  3. Call LLM provider
  4. If tool calls → execute via registry → StepExecutionResult
  5. If text → use as output → StepExecutionResult
```

### 7.2 Backtracking

`regenerate_from()`:
- Summarizes completed steps for context
- Prompts LLM for replacement steps from failure point
- Parses JSON array of step objects
- Fallback: single "Retry: <step>" step on parse failure

### 7.3 Constants

- `MAX_BACKTRACK_ATTEMPTS = 3` (full backtrack events, not per-step retries)

### 7.4 Known Limitations

- Single-cycle per step (no multi-turn reasoning within a step)
- The v2 `PlanExecuteEngine` in `execution/plan_execute.rs` provides richer execution with reflection checkpoints

---

## 8. Learning System

**Directory**: `learning/`
**Files**: `mod.rs`, `types.rs`, `outcome_store.rs`, `recorder.rs`, `analyzer.rs`, `service.rs`, `adaptive.rs`, `strategy_store.rs`, `strategy_tracker.rs`, `tool_confidence.rs`

### 8.1 Architecture

```
Tool Execution → OutcomeRecorder → OutcomeStore (JSONL + SQL)
                                        ↓
                              LearningAnalyzer.analyze()
                                        ↓
                              AdaptiveThresholds.propose_adjustment()
                                        ↓
                              LearningService (background) → event bus
                                        ↓
                              ConfidenceEvaluator threshold update
```

### 8.2 OutcomeStore (`outcome_store.rs`)

Dual-mode persistence (JSONL + SQL):
- Journal entries: `Record(OutcomeRecord)` and `Feedback(FeedbackEntry)`
- Auto-compact when journal > live entries + 200
- `outcomes_since(date)` for date-range queries
- SQL path: delegates to `storage::OutcomeRepo`

### 8.3 OutcomeRecord (`types.rs`)

```rust
struct OutcomeRecord {
    id: String,
    session_key: String,      // FNV-1a hashed for privacy
    tool_name: String,
    success: bool,
    error_category: Option<String>,
    duration_ms: u64,
    confidence_score: Option<f32>,
    confidence_dimensions: Option<ConfidenceDimensions>,
    execution_mode: ExecutionMode,  // Chat | PlanStep
    created_at: DateTime<Utc>,
}
```

**Privacy-by-omission**: No tool arguments or user message content stored.

### 8.4 OutcomeRecorder (`recorder.rs`)

- Best-effort recording (errors logged, not propagated)
- FNV-1a session key hashing
- `categorize_error()`: classifies errors into categories (timeout, not_found, permission, etc.)
- Implements `EnrichmentFeedbackHandler` for enrichment acceptance tracking

### 8.5 LearningAnalyzer (`analyzer.rs`)

`analyze()` computes:
- Per-tool stats with 5 confidence bands (0.0-0.2, 0.2-0.4, ..., 0.8-1.0)
- Suggested threshold: finds band where success rate >= 80%
- Enrichment acceptance stats (accepted / total suggestions)

### 8.6 AdaptiveThresholds (`adaptive.rs`)

- `MAX_THRESHOLD_STEP = 0.05` (max change per adjustment)
- Configurable bounds: `min_threshold`, `max_threshold`
- `min_outcomes` protection (won't adjust with too few data points)
- Persisted to file (atomic write via `.tmp` rename)
- Threshold history tracking (`ThresholdChange` records)

### 8.7 LearningService (`service.rs`)

Background analysis task:
- `CancellationToken` for graceful shutdown
- `Notify` for manual trigger (e.g., after enough outcomes accumulate)
- Publishes events via bus: `LearningEvent::ThresholdChanged`, `AnalysisCompleted`
- Updates `ConfidenceEvaluator` threshold via `Arc<AtomicU32>` handle

### 8.8 Strategy Tracking

**StrategyLearningStore** (`strategy_store.rs`):
- Records predicted vs actual strategy, escalation counts, iteration usage
- `get_strategy_accuracy()`: fraction where predicted == actual

**StrategyTracker** (`strategy_tracker.rs`):
- `compute_stats()`: accuracy, avg_escalations, avg_iterations

### 8.9 ToolConfidenceMap (`tool_confidence.rs`)

Per-tool confidence thresholds:
- Tools can have individual thresholds different from the global default
- Fallback to global threshold for unconfigured tools

---

## 9. Skill Manager

**File**: `skills.rs`

### 9.1 Architecture

```rust
struct SkillManager {
    built_in: Vec<Skill>,
    workspace: Vec<Skill>,
}

struct Skill {
    name: String,
    description: String,
    triggers: Vec<String>,
    content: Option<String>,
    always_loaded: bool,
    requirements: SkillRequirements,
}
```

### 9.2 Built-in Skills

10 built-in skills loaded via `include_str!()` at compile time:
- summarize, skill-creator, github, tmux, weather, cron, todo, and more

### 9.3 Skill Loading

- YAML frontmatter parsing (between `---` markers)
- Requirement checking: binary presence (via `which`) and environment variables
- Workspace skills loaded from `skills/` directory
- Always-loaded skills have full content injected into system prompt
- Non-always-loaded skills are summarized (name + description only)

### 9.4 Trigger Matching

`match_skill_triggers()`:
- Case-insensitive substring matching against each skill's trigger list
- Returns first match
- Used by `ContextBuilder` to prepend skill trigger context to user messages

---

## 10. Subagent Manager

**File**: `subagent.rs`

### 10.1 Architecture

```rust
struct SubagentManager {
    provider: DynProvider,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    inbound_tx: mpsc::Sender<InboundMessage>,
    config: AgentConfig,
    session_manager: Arc<RwLock<SessionManager>>,
    // ...
}
```

### 10.2 Builder Pattern

```rust
SubagentManager::builder()
    .provider(provider)
    .tool_registry(registry)
    .inbound_tx(tx)
    .config(config)
    .session_manager(session)
    .build()
```

### 10.3 Spawning

- Creates background tokio tasks
- Limited tool set: filesystem, shell, web (no message/spawn/cron to prevent recursion)
- 15-iteration cap per subagent
- Reports results back via inbound bus (system messages)
- Each subagent gets its own `RoutingContext`

---

## 11. Enrichment Engine

**Directory**: `enrichment/`
**Files**: `mod.rs`, `engine.rs`, `priority.rs`, `duration.rs`, `scheduling.rs`

### 11.1 Architecture

`EnrichmentEngine` implements `EnrichmentHandler` (tools crate trait):

```
EnrichmentEngine::enrich_task(todo)
  1. If priority not set → priority::infer_priority()
  2. If duration not set → duration::predict_duration()
  3. If due_date not set → scheduling::suggest_due_date()
  → EnrichmentResult (with suggestions + confidence scores)
```

### 11.2 Priority Inference (`priority.rs`)

Keyword-to-priority mapping with confidence scores:

| Keywords | Priority | Confidence |
|----------|----------|------------|
| urgent, critical, blocker, hotfix, emergency, asap, p0, sev1, production, outage | P1 (High) | 0.90 |
| important, bug, fix, broken, regression, p1, sev2, security | P2 (Medium-High) | 0.82 |
| feature, enhance, improvement, update, p2, refactor | P3 (Medium) | 0.75 |
| nice to have, low priority, cleanup, chore, docs, typo, minor, p3, p4 | P4 (Low) | 0.87 |

**Conflict resolution**: When multiple keyword groups match, picks highest confidence score.
**Searchable text**: title + description + tags (all combined, lowercased).
**Default**: P3 with 0.50 confidence when no keywords match.

### 11.3 Duration Prediction (`duration.rs`)

| Keywords | Duration | Confidence |
|----------|----------|------------|
| typo, rename, tweak, bump, toggle, minor | 15 min | 0.80 |
| fix, patch, update, adjust, lint, format | 30 min | 0.75 |
| feature, implement, add, create, build, design, migrate | 60 min | 0.70 |
| refactor, overhaul, rewrite, redesign, architecture, system, integration | 120 min | 0.65 |

**Note**: LARGE keywords are checked before MEDIUM to ensure "refactor" isn't caught by "feature" first.
**Default**: 45 min with 0.45 confidence.

### 11.4 Scheduling Suggestions (`scheduling.rs`)

Keyword-based due date suggestions:
- Urgency keywords (urgent, asap, critical, etc.) → +8 hours
- "today"/"tonight" → +8 hours (0.85 confidence)
- "tomorrow" → +1 day (0.85 confidence)
- "this week"/"eow" → +5 days (0.75 confidence)
- "next week" → +7 days (0.75 confidence)
- Priority fallback: P1→1d, P2→3d, P3→7d, P4→14d (0.50 confidence)
- No signal → None (does not suggest)

---

## 12. Memory Store

**File**: `memory.rs`

### 12.1 Architecture

File-based memory system:
- **Daily notes**: `workspace/memory/YYYY-MM-DD.md` — ephemeral per-day context
- **Long-term memory**: `workspace/memory/MEMORY.md` — persistent facts and preferences

### 12.2 Operations

- `get_memory_context()`: reads today's daily note + MEMORY.md, combines into context string
- `add_memory(content)`: appends to MEMORY.md
- `add_daily_note(content)`: appends to today's daily note file

### 12.3 Context Integration

Memory context is injected into system prompt by `ContextBuilder` with 60-second TTL cache. Invalidated after memory writes.

---

## 13. Reminder Engine

**File**: `reminders.rs`

### 13.1 Architecture

Background task with periodic checks:

```rust
struct ReminderEngine {
    todo_store: Arc<RwLock<TodoStore>>,
    sql_todo_repo: Option<storage::TodoRepo>,    // dual-mode
    calendar_handler: Option<Arc<dyn CalendarHandler>>,
    dispatcher: Arc<NotificationDispatcher>,
    check_interval: StdDuration,
    cancel_token: CancellationToken,
}
```

### 13.2 Reminder Rules

| Rule | Condition | Cooldown |
|------|-----------|----------|
| Due date alert | Within 2 hours, not in past | Once (via last_reminded_at) |
| Focused deadline | Focused task, deadline within 1 hour | Once |
| Overdue nagging | Past due date | Once per 24 hours |
| Calendar event | Event starts within 30 minutes | Once |

### 13.3 Dual-Mode Support

- SQL path: reads from `storage::TodoRepo`, updates via `storage::TodoPatch`
- JSONL path: reads/writes via `TodoStore` with `TodoPatch`
- Calendar events: fetched via `CalendarHandler::list_events()`, parsed as `CalendarEvent`

### 13.4 Lifecycle

`start()` → spawns tokio task with `tokio::select!` (cancel token + sleep interval)
`stop()` → cancels token, awaits task handle

---

## 14. Calendar Reconciliation & Sync

### 14.1 Reconciliation Engine (`calendar_reconcile.rs`)

Pure decision logic (no side effects):

```rust
fn determine_action(event: &CalendarEvent, todo: &Todo) -> ReconcileAction {
    Priority 1: Event CANCELLED → ClearCalendarLink
    Priority 2: Event COMPLETED (todo not done) → CompleteTodo
    Priority 3: Due date mismatch → UpdateDueDate
    Default: NoChange
}
```

`reconcile_calendar_events()`:
- Builds HashMap for O(1) event lookup
- For each todo with `calendar_event_uid`: determine action → apply via `TodoStore`
- Returns `ReconcileReport` (due_dates_updated, todos_completed, links_cleared, errors)

### 14.2 Calendar Sync Adapter (`calendar_sync_adapter.rs`)

**Multi-provider two-way sync** implementing `CalendarHandler`:

```rust
struct CalendarSyncAdapter {
    providers: Vec<(String, Box<dyn CalendarProvider>)>,
    todo_store: Arc<RwLock<TodoStore>>,
    sql_todo_repo: Option<storage::TodoRepo>,    // dual-mode
    auto_sync_due_dates: bool,
    bidirectional_sync: bool,
    dispatcher: Option<Arc<NotificationDispatcher>>,
}
```

**Supported providers**: Apple CalDAV, Google Calendar, Generic CalDAV

**Sync flow per provider**:
1. Load per-provider sync state (sync token, last_sync)
2. Fetch remote events (delta sync via token)
3. For each remote event:
   - Find linked todo by `calendar_event_uid`
   - Detect conflicts via `calendar::detect_conflict()`
   - Resolve via `calendar::resolve_conflict()` (server-wins default)
   - Update todo from event or create new todo
4. Push local changes (todos with due_date → PUT events)
5. Run reconciliation if bidirectional sync enabled
6. Save updated sync state

**CalendarHandler trait implementation**:
- `sync_calendar()` → sync all providers, returns JSON report
- `list_events(limit)` → upcoming events from all providers, deduplicated by UID
- `create_event()` → creates on all providers
- `get_status()` → provider statuses, sync counts
- `get_event(uid)` → fetch single event by UID
- `get_events_for_reconciliation()` → all events deduplicated

**Todo↔Event conversion**:
- `TodoStatus::Todo` → `TENTATIVE`
- `TodoStatus::Doing` → `CONFIRMED`
- `TodoStatus::Done` → `CONFIRMED`
- `TodoStatus::Archived` → `CANCELLED`
- Event duration: `estimated_minutes` (default 60)

---

## 15. Confidence Evaluator

**Directory**: `confidence/`
**Files**: `mod.rs`, `evaluator.rs`, `types.rs`, `prompt.rs`, `log.rs`

### 15.1 Architecture

```rust
struct ConfidenceEvaluator {
    threshold: Arc<AtomicU32>,  // f32 bits, lock-free reads
}
```

### 15.2 Assessment Flow

```
LLM Response → parse <confidence> XML block → ConfidenceAssessment
    → decide(assessment) → Proceed | Clarify { questions } | Skip
```

### 15.3 Dimensions

```rust
struct ConfidenceDimensions {
    intent_clarity: f32,      // How clear is the user's intent?
    tool_fit: f32,            // Do available tools match the need?
    info_sufficiency: f32,    // Is there enough info to proceed?
}
```

### 15.4 Decision Logic

- `score >= threshold` → `Proceed`
- `score < threshold` → `Clarify` with questions based on low-scoring dimensions
- All values clamped to [0.0, 1.0]

### 15.5 Threshold Updates

The threshold is `Arc<AtomicU32>` (f32 stored as bits):
- Lock-free reads via `Ordering::SeqCst`
- External updates via `threshold_handle()` (used by `LearningService`)
- Also reflected in `ContextBuilder::set_confidence_threshold()`

### 15.6 Supporting Components

**`prompt.rs`**: `confidence_prompt(threshold)` — generates system prompt instructions telling the LLM to emit `<confidence>` blocks.

**`log.rs`**: `DecisionLogger` — append-only JSONL at `~/.klyntbot/decision_log.jsonl` for debugging and auditing.

**`evaluator.rs`**: `strip_confidence_blocks(content)` — removes `<confidence>...</confidence>` from content before showing to user.

---

## 16. Handler Implementations

The agent crate implements 7 dependency-inverted handlers:

| Handler Trait (tools L3) | Implementation (agent L5) | File |
|--------------------------|---------------------------|------|
| `CalendarHandler` | `CalendarSyncAdapter` | `calendar_sync_adapter.rs` |
| `CronHandler` | `CronHandlerAdapter` | `cron_handler_adapter.rs` |
| `GoalHandler` | `GoalHandlerImpl` | `goal_handler.rs` |
| `PlanHandler` | `PlanHandlerImpl` | `plan_handler.rs` |
| `PlanCompletionHandler` | `PlanCompletionHandlerImpl` | `plan_completion_handler.rs` |
| `EnrichmentHandler` | `EnrichmentEngine` | `enrichment/engine.rs` |
| `LearningHandler` | `LearningHandlerImpl` | `learning_handler.rs` |
| `ConversationEmbeddingHandler` | `ConversationEmbeddingHandlerImpl` | `conversation_embedding_handler.rs` |

### 16.1 Pattern

All follow the same pattern:
1. Trait defined in `tools` crate (Layer 3) with `async_trait`
2. Implementation in `agent` crate (Layer 5) wrapping a store/service
3. Injected as `Arc<dyn Trait>` into the tool at construction time
4. Breaks circular dependency: tools → handler trait ← agent

### 16.2 PlanCompletionHandler

Records plan outcomes on linked goals:
- Writes metadata to GoalStore: `last_completed_plan_id`, `last_plan_outcome`, `last_plan_summary`, `plans_completed` counter
- No-op if plan isn't linked to a goal

### 16.3 ConversationEmbeddingHandler

Production handler for conversation embeddings:
- Reuses shared `EmbeddingEngine` (fastembed)
- Composes text with role prefix ("User: ...", "Assistant: ...")
- CPU-bound embedding generation via `spawn_blocking`
- Best-effort: errors logged but not propagated
- Stores in `ConversationEmbeddingStore` (JSONL-based)
- Search: cosine similarity with threshold filtering

---

## 17. Output Module

**Directory**: `output/`
**Files**: `mod.rs`, `cost_tracker.rs`, `validator.rs`

### 17.1 CostTracker (`cost_tracker.rs`)

Tracks LLM usage and costs:

```rust
struct CostTracker {
    data_dir: PathBuf,
    sql_repo: Option<storage::UsageRepo>,
}
```

**Pricing model** (per million tokens):

| Model | Input | Output |
|-------|-------|--------|
| Opus | $15.00 | $75.00 |
| Sonnet | $3.00 | $15.00 |
| Haiku | $0.25 | $1.25 |
| GPT-4o | $2.50 | $10.00 |

**Dual-mode**: SQL (preferred) or JSONL fallback.

`record()`: persists `UsageRecord` with estimated cost.
`report(days)`: aggregates by model, by day, totals for the last N days.

### 17.2 ResponseValidator (`validator.rs`)

Safety and quality checks before delivering to user:

1. **Length check**: truncates at `max_response_tokens * 4` chars, cuts at word boundary, appends `…`
2. **System prompt leak detection**: checks for 11 patterns (e.g., "you are klyntbot", `<system>`, "my system prompt says"). Redacts matches with `[redacted]`.
3. **Quality checks**: flags empty responses (invalid) and very short responses (warning but valid)

---

## 18. Supporting Modules

### 18.1 Events (`events.rs`)

```rust
enum AgentEvent {
    ContentChunk(String),
    ToolStart { name, args },
    ToolEnd { name, success, duration_ms },
    IterationStart { iteration, max },
    Done(String),
    ConfidenceAssessed { score, action },
    Error(String),
    PlanStepCompleted { plan_id, step_index, result },
    PlanCompleted { plan_id, summary },
}
```

Used for real-time streaming updates from agent loop to CLI/channels.

### 18.2 Chat Formatter (`chat/formatter.rs`)

Channel-aware response formatting:

| Channel | Max Chars | Treatment |
|---------|-----------|-----------|
| Telegram | 4096 | Preserve markdown, truncate |
| Discord | 2000 | Preserve markdown, truncate |
| WhatsApp | 4096 | Strip markdown, truncate |
| CLI/other | unlimited | Pass through |

`strip_markdown()`: removes `**`, `__`, `` ` ``, `#` markers.
`truncate_at_boundary()`: UTF-8 safe, cuts at word boundary, appends `...`.

### 18.3 Recurring Task Spawner (`recurring_tasks.rs`)

Background job following the ReminderEngine pattern:

```
RecurringTaskSpawner::check_and_spawn()
  1. List all template todos (is_template=true)
  2. For each with recurrence_rule:
     a. Check should_spawn_instance(next_instance_date, now)
     b. Clone template → concrete todo instance
     c. Advance next_instance_date via rrule_utils::next_occurrence()
```

Dual-mode: SQL and JSONL paths.

### 18.4 Notification Dispatcher (`notifications.rs`)

Multi-target notification:
- `os_native`: OS-level notification via `common::utils::notify`
- Channel names: sends via outbound message bus to last active channel/chat

### 18.5 Free Function: `build_plan_context` (`context.rs`)

Builds system prompt addendum for plan execution:
- Context window: current step + next 3 steps
- Progress indicator: "step N/M"
- Step markers: `>>> CURRENT`, `NEXT 1-3`

---

## 19. Gap Analysis & Recommendations

### 19.1 Critical Issues

| # | Issue | Impact | Location |
|---|-------|--------|----------|
| 1 | **AgentLoop is 2,357 lines** — far too large for a single file | Maintainability, testability | `agent_loop.rs` |
| 2 | **Legacy vs Pipeline dual path** — both maintained simultaneously | Code duplication, confusion about which path is active | `agent_loop.rs` |
| 3 | **SQL TodoPatch missing fields** — `calendar_event_uid`, `next_instance_date`, `last_reminded_at` not in `storage::TodoPatch` | Calendar sync, recurring tasks, and reminders have best-effort-only SQL support | Multiple files |

### 19.2 Architectural Concerns

| # | Issue | Impact | Location |
|---|-------|--------|----------|
| 4 | **~30 fields in AgentLoop struct** — god object pattern | Hard to reason about state, difficult to test | `agent_loop.rs` |
| 5 | **Constructor is ~450 lines** — does too much | Hidden coupling, hard to modify initialization order | `new_with_cron()` |
| 6 | **Two plan executors** — `plan_executor.rs` (legacy) and `execution/plan_execute.rs` (v2) | Unclear which is used when, potential divergence | Both files |
| 7 | **Dual-mode (JSONL+SQL) throughout** — every component has branching for both backends | Code duplication, inconsistent behavior between modes | reminders, recurring_tasks, calendar_sync, etc. |
| 8 | **File-based memory** — daily notes and MEMORY.md aren't in PostgreSQL | Inconsistent with the "all state in PostgreSQL" migration goal | `memory.rs` |
| 9 | **File-based learning stores** — OutcomeStore JSONL, AdaptiveThresholds file, DecisionLogger JSONL | Same as above — should be SQL-backed | `learning/` |

### 19.3 Missing Features / Incomplete Implementations

| # | Feature | Status | Notes |
|---|---------|--------|-------|
| 10 | **Real-time plan progress** | Not implemented | Plan progress only visible between executions |
| 11 | **Streaming in pipeline path** | Partial | Legacy path has streaming; v2 pipeline doesn't integrate it fully |
| 12 | **Tool approval/rejection** | Channel exists but unused | `interaction_tx` in streaming, no UI integration |
| 13 | **ToolConfidenceMap integration** | Struct exists | Not wired into ConfidenceEvaluator decision logic |
| 14 | **StrategyTracker feedback loop** | Tracking only | Stats computed but not used to adjust strategy selection |

### 19.4 Testing Gaps

| # | Area | Gap |
|---|------|-----|
| 15 | AgentLoop integration tests | Constructor too complex to unit test; no end-to-end test of message processing |
| 16 | Pipeline v2 end-to-end | Pipeline tests exist but don't test full Orchestrator→...→CostTracker chain |
| 17 | CalendarSyncAdapter sync flow | Only tests construction and conversion; no mock provider tests for sync logic |
| 18 | Learning service background loop | No test of the periodic analysis + threshold update cycle |

### 19.5 Recommendations (Prioritized)

**P0 — Must Fix**:
1. Extend `storage::TodoPatch` with `calendar_event_uid`, `next_instance_date`, `last_reminded_at` to complete SQL migration
2. Choose one plan executor path and deprecate the other

**P1 — Should Fix**:
3. Split `AgentLoop` into focused subcomponents (MessageProcessor, PlanRunner, ToolExecutor)
4. Migrate memory store to PostgreSQL
5. Unify JSONL+SQL dual-mode into SQL-only (remove JSONL fallback code)

**P2 — Nice to Have**:
6. Wire `ToolConfidenceMap` into `ConfidenceEvaluator`
7. Add strategy feedback loop (use accuracy stats to adjust classification)
8. Add streaming to pipeline v2 path
9. Integration tests for full message processing flow

---

*Analysis by agent-analyst, 2026-02-19*
