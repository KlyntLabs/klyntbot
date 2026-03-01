# Agent

## Purpose

The `agent` crate (Layer 5) is the brain of Klyntbot. It owns the processing loop that turns inbound user messages into outbound responses by coordinating LLM calls, tool execution, session management, and a suite of background services. The crate is organized around a single entry point -- `AgentLoop` -- which wires together every subsystem at construction time via a builder pattern, then runs a simple recv-process-respond loop for its lifetime.

No business logic lives in the message handling itself. The `AgentLoop` delegates classification, context assembly, execution routing, and cost tracking to the `IntentPipeline`. Tool execution is handled by `ExecutionCore`. Background tasks (reminders, recurring tasks, learning, session cleanup, memory maintenance, plan cleanup) run as independent tokio tasks managed by cancellation tokens.

## Key Types

### AgentLoop

The central struct. Holds references to every subsystem the agent needs:

| Field | Type | Purpose |
|-------|------|---------|
| `bus` | `Arc<MessageBus>` | Inbound/outbound message routing. |
| `pipeline` | `Arc<IntentPipeline>` | Classify-assemble-route-validate-record pipeline. |
| `context_engine` | `Arc<ContextEngine>` | Builds system prompts from prioritized context sources. |
| `session_manager` | `SessionManager` | Per-session conversation history (SQL-backed). |
| `tool_registry` | `Arc<RwLock<ToolRegistry>>` | All registered tools, read-locked during execution. |
| `skill_manager` | `Arc<SkillManager>` | Loaded skills (built-in + workspace). |
| `reminder_engine` | `Option<Arc<RwLock<ReminderEngine>>>` | Due-date and overdue task notifications. |
| `recurring_task_spawner` | `Option<Arc<RwLock<RecurringTaskSpawner>>>` | Spawns instances from recurring task templates. |
| `learning_service` | `Option<Arc<RwLock<LearningService>>>` | Outcome analysis and threshold adaptation. |
| `conversation_embedding_handler` | `Option<Arc<dyn ConversationEmbeddingHandler>>` | Fire-and-forget semantic embedding of messages. |
| `plan_executing` | `Arc<AtomicBool>` | Tracks whether a plan is running (controls iteration limits). |
| `strategy_repo` | `Option<StrategyRepo>` | Updates satisfaction scores from emoji reactions. |

Construction is exclusively through `AgentLoop::builder(bus, provider, config)`, which returns an `AgentLoopBuilder`.

### AgentLoopBuilder

Consumes required parameters (`bus`, `provider`, `config`) and optional parameters (`pool`, `vector_store`, `cron_service`, `notification_handle`), then wires every subsystem in a single `build()` call. The builder's responsibilities, in order:

1. Create storage repos from the SQLite pool (or in-memory fallback).
2. Load and filter skills based on enabled packs.
3. Assemble context sources (identity, bootstrap, memory, todos, goals, confidence, skill summaries) sorted by priority.
4. Create the session manager from a SQL-backed repo.
5. Build the subagent manager with profile-based tool sets and semaphore concurrency control.
6. Register all tools: filesystem, grep/glob, web search/fetch, browser, message, ask-user, spawn, cron, calendar, todo, goal, plan, memory, finance, learning, and WASM plugins.
7. Wire the calendar sync adapter and enrichment engine into the todo tool.
8. Create the confidence evaluator with per-tool threshold overrides.
9. Start background services: reminder engine, recurring task spawner, learning service, session cleanup, memory maintenance, plan cleanup.
10. Assemble the intent pipeline: `IntentAnalyzer` + `ContextEngine` + `ExecutionRouter` (Direct/Reactive/Planned engines) + `CostTracker`.

### ExecutionCore

Drives a single LLM-tool cycle. Given messages, tool definitions, and execution parameters, `run_cycle()` performs one round of:

1. Call `provider.chat()` with messages and tool definitions.
2. If the LLM returns tool calls: execute all in parallel with per-tool timeout via `join_all`, append tool results to messages, return `CycleOutcome::ToolsExecuted`.
3. If the LLM returns text: check for fabrication, return `CycleOutcome::FinalResponse` or `CycleOutcome::FabricatedResponse`.
4. If neither: return `CycleOutcome::EmptyResponse`.

Three safety mechanisms protect the execution loop:

- **Duplicate tool call prevention** -- a `HashSet<String>` of `"name|args_hash"` keys tracks previously seen calls. If all tool calls in a batch are duplicates, synthetic "already called" results are returned instead of re-executing.
- **Fabrication detection** -- heuristics detect when an LLM skips tool calls and generates fake structured results (fake IDs, field-like patterns, numbered lists). Context-aware: patterns only trigger when the matching tool is actually available.
- **Per-tool timeout** -- each tool call is wrapped in `tokio::time::timeout`. Timed-out tools return an error result without blocking other parallel calls.

### CycleOutcome

Enum returned by `run_cycle()`:

| Variant | Meaning |
|---------|---------|
| `FinalResponse { content }` | LLM produced a text response (no tool calls). |
| `ToolsExecuted { results }` | Tools were called; results appended to messages. Caller should loop. |
| `EmptyResponse` | LLM returned nothing. |
| `FabricatedResponse { content }` | LLM faked a tool result in text. Caller should re-prompt. |

### IntentPipeline

Replaces the former Orchestrator + EngineDispatch + AgentPipeline with a unified flow:

```
IntentAnalyzer -> ContextEngine -> ExecutionRouter -> ResponseValidator -> CostTracker
```

The `IntentAnalyzer` uses a two-stage approach: fast heuristics first (keyword matching for greetings, CRUD, complex tasks), falling back to an LLM classifier for ambiguous messages. It produces an `IntentAnalysis` with an `ExecutionMode` (Direct, Reactive, or Planned) and `ComplexitySignals`.

The `ExecutionRouter` maps the mode to the appropriate engine:

- **DirectEngine** -- single LLM call, no tools. For greetings and simple questions.
- **ReactiveEngine** -- ReAct loop with tool calls up to a configurable iteration limit. For task CRUD, search, calendar operations.
- **PlannedEngine** -- generates a multi-step plan, then executes steps sequentially. For complex multi-tool workflows.

When an engine signals `EngineResult::Escalate`, the router automatically escalates (Direct -> Reactive -> Planned) up to `max_escalations` (default 3).

### MemoryStore

SQL-backed persistent memory with optional LanceDB embeddings. Two storage layers:

- **Daily notes** -- keyed by date string (`"2026-03-01"`), appended throughout the day. Surfaced in the system prompt via `MemorySource`.
- **Long-term memory** -- a single `LONG_TERM` key for persistent facts, preferences, and user context.

When embeddings are enabled, `get_relevant_memory(query, limit)` embeds the query, performs ANN cosine similarity search against the `memory_note_embeddings` table in LanceDB, and returns only notes above the similarity threshold. Falls back to dumping all memory when embeddings are unavailable. Every write (append or upsert) fires a background embedding task via `tokio::spawn` for future semantic retrieval.

### SubagentManager

Manages spawned background agents for parallel task execution. Key design decisions:

- **Builder pattern** -- `SubagentManager::builder(provider, workspace)` with fluent configuration for model, brave API key, workspace restrictions, and concurrency limits.
- **Profile-based tool access** -- three `SubagentProfile` variants control what each subagent can do:

| Profile | Tools | Max Iterations |
|---------|-------|----------------|
| General | Filesystem (read/write) + web + grep/glob | 15 |
| Research | Filesystem (read-only) + web + grep/glob | 10 |
| Analyst | Filesystem (read-only) + grep/glob | 5 |

- **Semaphore concurrency** -- an `Arc<Semaphore>` limits how many subagents can run simultaneously (default 3). Each subagent acquires a permit before execution and releases it when done.
- **Cancellation** -- each subagent gets a `CancellationToken`. The parent can cancel via `cancel_subagent(short_id)`.
- **Result routing** -- on completion, results are announced back to the main agent loop via the `MessageBus` as system messages with the original channel/chat ID encoded for correct routing.
- **Task board coordination** -- all subagents receive an `AgentTaskTool` for shared task board operations (list, claim, complete, fail).

Implements the `SpawnHandler` trait (defined in `tools`, Layer 3) via dependency inversion, allowing the `SpawnTool` to trigger subagent creation without depending on the agent crate.

### SkillManager

Discovers, loads, and manages skill definitions. Skills are Markdown files with YAML frontmatter that define agent capabilities.

- **Built-in skills** -- six skills bundled at compile time via `include_str!`: cron, daily-planning, skill-creator, summarize, todo, weather.
- **Workspace skills** -- loaded from `~/.klyntbot/skills/*/SKILL.md` at runtime. Workspace skills override built-in skills with the same name.
- **YAML frontmatter** -- parsed for metadata: `description`, `version`, `always` (always load full content), `triggers` (activation keywords), `requires_bins` and `requires_env` (prerequisite checks).
- **Pack filtering** -- `filter_by_skills()` restricts active skills to those from enabled feature packs. Workspace skills are always kept regardless of pack selection.
- **Cached summary** -- `generate_summary()` produces an XML skills listing for the system prompt, cached via `OnceLock` after first call.

### AgentEvent

Enum for real-time streaming progress. Serialized with `#[serde(tag = "type", rename_all = "camelCase")]` for WebSocket/CLI consumption:

| Variant | When Emitted |
|---------|-------------|
| `ContentChunk` | LLM streams a text delta. |
| `ToolStart` | A tool execution begins (name + args). |
| `ToolEnd` | A tool execution finishes (success, duration, truncated result). |
| `IterationStart` | A new ReAct iteration begins. |
| `ClassificationComplete` | Pipeline classification step finishes (strategy, confidence, source). |
| `ContextAssembled` | Context assembly completes (total tokens, budget). |
| `ExecutionStarted` | An execution engine is selected (engine name, max iterations). |
| `ConfidenceAssessed` | Internal confidence check completes (score, action). |
| `PlanStepCompleted` | A plan step finishes (plan ID, step index, result). |
| `PlanCompleted` | A plan execution finishes (plan ID, summary). |
| `EntityCreated` | A tool created an entity (task, project, goal). |
| `Done` | Processing complete with final accumulated content. |
| `Error` | An error occurred. |

### StreamingHandle

Returned by `process_direct_streaming()` for CLI and dashboard consumers:

| Field | Type | Purpose |
|-------|------|---------|
| `event_rx` | `mpsc::Receiver<AgentEvent>` | Stream of progress events. |
| `interaction_rx` | `mpsc::Receiver<InteractionBundle>` | Structured interaction requests from `ask_user` tool. |
| `cancel_token` | `CancellationToken` | Cancel in-flight processing. |
| `handle` | `JoinHandle<Result<String>>` | Background task producing the final response. |

## How It Works

### Message Processing

The main loop is intentionally simple. `run_with_rx()` receives `InboundMessage` values from the bus with a 1-second timeout poll, calling `process_message()` for each:

1. **Validate** -- reject oversized messages silently.
2. **Classify message kind** -- reactions go to `handle_reaction()` (update satisfaction score in `StrategyRepo`, no LLM call). System messages (subagent results, session resets) go to `process_system_message()`.
3. **Track last active channel** -- stored for notification routing.
4. **Session management** -- `get_or_create()` a per-session `Arc<Mutex<Session>>`, add the user message, extract history (capped at `history_limit`).
5. **Conversation embedding** -- fire-and-forget background task to embed the user message in LanceDB for future cross-session memory retrieval.
6. **Run pipeline** -- build system prompt via `ContextEngine`, convert history to provider `Message` format, call `pipeline.process_message()`.
7. **Save and send** -- save assistant response to session, embed it, publish `OutboundMessage` to the bus.

System messages carry a compound `chat_id` (`"channel:chat_id"`) that is parsed to route the response back to the originating conversation. Session resets (from Telegram's `/reset` command) clear the session without invoking the pipeline.

### Direct and Streaming Modes

Two additional entry points bypass the bus for CLI usage:

- `process_direct(content, session_key)` -- synchronous pipeline execution, returns the response string directly.
- `process_direct_streaming(content, session_key)` -- spawns pipeline execution in a background task, returns a `StreamingHandle` with event and interaction channels. The spawned task emits `AgentEvent::ContentChunk` and `AgentEvent::Done` events, saves to session on completion, and propagates errors via `AgentEvent::Error`.

### Reaction Handling

Emoji reactions are mapped to satisfaction scores: positive reactions (thumbs up, heart, party) map to 1.0, negative reactions (thumbs down, confused) map to 0.0. The score is written to the most recent `strategy_record` for the chat (within 5 minutes), enabling the learning system to correlate user satisfaction with strategy choices.

### Execution Cycle

`ExecutionCore::run_cycle()` is the inner engine used by all three execution modes. A typical ReAct loop (driven by `ReactiveEngine`) calls `run_cycle()` repeatedly until it gets a `FinalResponse` or hits the iteration limit:

1. Call `provider.chat()` with accumulated messages and tool definitions.
2. If tool calls are returned, check for duplicates against the `seen_tool_calls` set. If all calls are duplicates, return synthetic skip results.
3. Execute non-duplicate tool calls in parallel via `join_all`. Each tool runs inside `tokio::time::timeout` with the configured duration. `ToolStart`/`ToolEnd` events are emitted for streaming consumers.
4. Append tool results as `Message::tool()` entries to the conversation.
5. If a text response is returned instead of tool calls, run fabrication detection heuristics. Fabricated responses cause the engine to re-prompt the LLM.

Entity cards (task/project/goal metadata) created by tools during execution are collected via an `mpsc` channel and emitted as `EntityCreated` events.

### Feature Adapters (Dependency Inversion)

Several traits are defined in Layer 3 crates (`tools`, `feature-todo`, `feature-finance`) but implemented in the `agent` crate (Layer 5) to break circular dependencies:

| Adapter | Trait | What It Does |
|---------|-------|-------------|
| `CalendarSyncAdapter` | `CalendarHandler` | Multi-provider CalDAV sync (Apple, Google, Generic). Bidirectional sync with conflict resolution. |
| `CronHandlerAdapter` | `CronHandler` | Bridges `CronService` (from `scheduling` crate) to the `CronHandler` trait for the `CronTool`. |
| `GoalHandlerImpl` | `GoalHandler` | Bridges `GoalRepo` to `GoalHandler`. Includes LLM-based plan generation for goals. |
| `PlanHandlerImpl` | `PlanHandler` | Bridges `PlanRepo` to `PlanHandler`. LLM-based step generation. |
| `SubagentManager` | `SpawnHandler` | Converts spawn requests from the `SpawnTool` into background subagent tasks. |
| `FinanceHandlerImpl` | `FinanceHandler` | Bridges storage repos and price service to the finance tool. |
| `LearningHandlerImpl` | `LearningHandler` | Bridges strategy repo and adaptive thresholds to the learning tool. |
| `EnrichmentEngine` | `EnrichmentHandler` | Priority inference, duration prediction, and due-date suggestion for tasks. |

All adapters are injected as `Arc<dyn Trait>` during builder construction.

### EnrichmentEngine

Auto-infers missing task fields using keyword analysis across three dimensions:

- **Priority** -- detects urgency keywords ("urgent", "critical", "blocker" -> P1; "bug", "fix" -> P2; "feature", "refactor" -> P3; "cleanup", "typo" -> P4).
- **Duration** -- estimates time based on scope keywords ("typo" -> 15 min; "fix" -> 30 min; "feature" -> 60 min; "refactor" -> 120 min).
- **Scheduling** -- suggests due dates based on inferred priority (urgent -> today; important -> this week).

Optionally enhanced with LLM-based inference when `use_llm` is enabled in config. Implements `EnrichmentHandler` from the `tools` crate.

### Background Services

All background services follow the same pattern: a periodic check loop driven by `tokio::select!` over a `CancellationToken` and `tokio::time::sleep`. The `AgentLoop::shutdown()` method cancels all tokens and awaits task completion.

| Service | Interval | Purpose |
|---------|----------|---------|
| `ReminderEngine` | 5 minutes | Checks todos and calendar events for due-date alerts (2h), focused deadline alerts (1h), overdue nagging (daily), and calendar event alerts (30 min). Sends notifications via `NotificationDispatcher`. |
| `RecurringTaskSpawner` | 60 seconds | Reads template tasks with recurrence rules, checks if instances are due, clones them as concrete todos, and advances the template's `next_instance_date`. |
| `LearningService` | Configurable | Analyzes recorded tool outcomes, computes adaptive confidence thresholds, and publishes `ThresholdChanged` events via the `LearningEventBus`. |
| `SessionCleanupService` | Configurable (hours) | Deletes expired sessions older than `ttl_days`. |
| `MemoryMaintenanceService` | Configurable (hours) | Removes stale conversation embeddings older than `max_age_days` from LanceDB. |
| `PlanCleanupService` | Hourly | Deletes stale plans based on visibility rules: silent plans after 24h, on_failure plans after 7 days. |

### Confidence and Learning

The confidence system evaluates LLM intent understanding before tool execution:

- **ConfidenceEvaluator** -- compares a numeric confidence score against a threshold (global default + per-tool overrides). Returns a `DecisionAction`: `Proceed`, `AskClarification`, or `Defer`.
- **ConfidenceSource** -- a context source that injects the current threshold into the system prompt via an `AtomicU32` handle, allowing live updates from the learning system.

The learning system closes the feedback loop:

1. **OutcomeStore** -- records tool execution outcomes (tool name, success, duration, confidence score) in SQLite. Privacy-by-omission: no tool arguments or user messages are stored.
2. **AdaptiveThresholds** -- analyzes outcome history to compute optimal per-tool confidence thresholds. Bounded between `min_threshold` and `max_threshold`.
3. **LearningService** -- periodic background task that runs analysis and publishes `ThresholdChanged` events via the `LearningEventBus` (a `tokio::broadcast` channel).
4. **Event subscriber** -- a spawned task in the builder listens for `ThresholdChanged` events and updates both the `ConfidenceSource` (for system prompt) and the `ConfidenceEvaluator` (for runtime decisions) via their atomic threshold handles.

### NotificationDispatcher

Routes notifications to configured targets. Supports two target types:

- **`os_native`** -- sends macOS/Linux desktop notifications via the `notify-rust` crate.
- **Channel names** (e.g., `"telegram"`) -- sends an `OutboundMessage` to the last active chat on that channel.

Used by `ReminderEngine` for task and calendar event alerts.

### Context Assembly

The `ContextEngine` (from the `context_engine` crate) builds system prompts from prioritized `ContextSource` implementations. The agent crate provides these sources:

| Source | Priority | Content |
|--------|----------|---------|
| `IdentitySource` | Highest | Agent identity, workspace path, timezone, current date/time. |
| `BootstrapSource` | High | Workspace-level instructions from `AGENT.md` or similar bootstrap files. |
| `MemorySource` | High | Long-term memory and today's notes from `MemoryStore`. |
| `TodoSource` | Medium | Active and focused todo summaries from `TodoRepo`. |
| `GoalSource` | Medium | Active goals from `GoalRepo`. |
| `ConfidenceSource` | Low | Current confidence threshold (live-updated via atomic handle). |
| `SkillSummarySource` | Low | XML listing of available skills with descriptions and triggers. |
| `SkillContentSource` | Low | Full content of `always`-loaded skills. |

When conversation embedding is enabled, a `ConversationMemoryRetriever` provides cross-session semantic recall with time-decay weighting (configurable half-life).

## Connections

### Dependencies (what agent imports)

- `common` (Layer 0): error types, `ChannelName`, `ChatId`, `SessionKey`, `EntityCard`, utility functions
- `config` (Layer 1): `Config` struct for all agent settings
- `bus` (Layer 1): `MessageBus`, `InboundMessage`, `OutboundMessage`, `LearningEventBus`, `LearningEvent`
- `storage` (Layer 1.5): `StoragePool`, `Repos`, all `*Repo` structs, `TodoFilter`, `TodoPatch`, `VectorStore`
- `providers` (Layer 2): `DynProvider`, `Message`, `ChatParams`, `Usage`
- `session` (Layer 2): `SessionManager`, `SessionMessage`
- `scheduling` (Layer 2): `CronService`
- `calendar` (Layer 2): calendar providers, sync state, conflict resolution
- `context_engine` (Layer 2): `ContextEngine`, `ContextSource`, token counting
- `tools` (Layer 3): `ToolRegistry`, `RoutingContext`, all tool structs, handler traits (`SpawnHandler`, `CronHandler`, `CalendarHandler`, `GoalHandler`, `PlanHandler`, `LearningHandler`, `ConversationEmbeddingHandler`, `EmbeddingHandler`), `EmbeddingEngine`
- `tools_core` (Layer 3): `FeaturePackage`
- `feature_todo` (Layer 3): `TodoTool`, `TodoRepo`, `CalendarSyncHandler`, `EmbeddingHandler`, `EnrichmentHandler`, `rrule_utils`
- `feature_finance` (Layer 3): `FinanceTool`, `PriceService`, `FinanceHandler`
- `plugin_runtime` (Layer 3): `PluginManager` for WASM plugin loading

### Dependents (what imports agent)

- `klyntbot` (Layer 7): constructs `AgentLoop` via the builder, calls `run()` or `run_with_rx()` in the `serve` command, provides `process_direct` and `process_direct_streaming` for CLI and dashboard
