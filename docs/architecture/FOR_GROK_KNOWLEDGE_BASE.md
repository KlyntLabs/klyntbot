# Klyntbot Knowledge Base

Comprehensive knowledge transfer document for understanding and extending the Klyntbot codebase.

---

## 1. Top 20 Most Important Types/Traits

### Foundation Types

| Type | Crate | Purpose |
|---|---|---|
| `KlyntbotError` | common | Top-level error enum. All errors flow through this. Alias: `Result<T> = std::result::Result<T, KlyntbotError>` |
| `ChannelName` | common | Newtype for platform identifiers ("telegram", "discord", "cli", "mcp") |
| `ChatId` | common | Newtype for conversation identifiers within a channel |
| `SessionKey` | common | Composite key `"channel:chat_id"` for session lookup |
| `MessageRole` | common | Enum: System, User, Assistant, Tool |
| `Config` | config | Root configuration struct. All `#[serde(rename_all = "camelCase")]`. File at `{KLYNTBOT_HOME}/config.json` |
| `Secret<T>` | config | Wrapper that redacts in Debug/Display. Access via `.expose()` |

### Tool System

| Type | Crate | Purpose |
|---|---|---|
| `Tool` (trait) | tools-core | Primary interface for all agent tools. Methods: `name()`, `description()`, `parameters()`, `execute(args, ctx)` |
| `ToolParams` (trait) | tools-core | Typed parameter parsing + JSON Schema generation. Implemented via `#[derive(ToolParams)]` |
| `ToolExecute` (trait) | tools-core | Typed execution bridge. Macro generates untyped `Tool` impl that delegates to this |
| `FeaturePackage` (trait) | tools-core | Self-contained feature bundle: tools + migrations + config + health. Methods: `name()`, `tools()`, `migrations()` |
| `ToolRegistry` | tools-core | Central registry. `register(tool)`, `execute(name, args, ctx)`, `get_definitions()` |
| `RoutingContext` | tools-core | Carries channel/chat identity through tool execution. Fields: channel, chat_id, delegation_depth, entity_tx |

### Agent & Runtime

| Type | Crate | Purpose |
|---|---|---|
| `AgentRuntime` | agent | 10-step processing pipeline. Owns SkillCatalog, SkillRouter, IntentAnalyzer, ContextEngine, ExecutionRouter |
| `AgentLoop` | agent | Top-level message processing engine. Owns all runtime state |
| `LlmProvider` (trait) | providers | Unified LLM interface. Methods: `chat()`, `chat_stream()`, `name()`, `context_window()` |
| `SkillRouter` | skill-system | Routes messages to best orchestrator via keyword + semantic scoring |
| `ContextEngine` | context_engine | Token budget management, history compression, memory retrieval, context source assembly |

### Memory & Storage

| Type | Crate | Purpose |
|---|---|---|
| `StoragePool` | storage | Clone+Send+Sync SQLite pool wrapper. `connect()`, `connect_in_memory()` |
| `Repos` | storage | Aggregate of 22+ repository structs. `Repos::from_pool(&pool)` |
| `SemanticFact` | cognitive | Primary memory unit -- SPO triple with FSRS decay, bi-temporal markers, scoping |
| `AppCore` | app-core | Central application state. Transport-agnostic. Holds all runtime services |

---

## 2. All Public Handler Traits

### Defined in tools-core (L0)

| Trait | Purpose | Implemented in |
|---|---|---|
| `ProgressHandler` | Cascade KR progress on task completion | agent (`ProgressHandlerImpl`) |
| `InteractionChannel` | Platform-native structured interactions | channels (Telegram, Discord, Slack) |
| `ConfigPersistence` | Runtime config read/write | app-core |

### Defined in tools (L4)

| Trait | Purpose | Implemented in |
|---|---|---|
| `CronHandler` | Bridge cron tool to scheduling service | agent (`CronHandlerAdapter`) |
| `SpawnHandler` | Sub-agent creation | agent (`SubagentManager`) |
| `DelegationHandler` | Agent-to-agent delegation | agent (`AgentRuntime`) |
| `LearningHandler` | Adaptive threshold learning | agent (`LearningHandlerImpl`) |
| `ConversationRecallHandler` | Conversation memory embedding/search | agent (`ConversationRecallHandlerImpl`) |
| `ContextExpansionHandler` | Mid-execution context expansion | agent |
| `ContentRegistryHandler` | Documentation/content search | agent (`ContentRegistryImpl`) |
| `AgentTaskHandler` | Sub-agent task board | agent |

### Defined in feature-tasks (L4)

| Trait | Purpose | Implemented in |
|---|---|---|
| `EnrichmentHandler` | LLM task enrichment (priority, tags, project) | agent (`EnrichmentEngine`) |
| `EmbeddingHandler` | Generate embeddings for semantic search | agent (`TextEmbedderImpl`) |
| `DecompositionHandler` | AI subtask generation | agent (`LlmDecompositionHandler`) |
| `TaskExecutionHandler` | Agentic task execution | agent (`LlmTaskExecutionHandler`) |
| `DayPlanningHandler` | Daily task plan generation | agent (`LlmDayPlanningHandler`) |
| `ProactiveHandler` | Proactive task suggestions | agent (`LlmProactiveHandler`) |
| `SuggestionApplier` | Execute accepted suggestions | agent (`TaskSuggestionApplier`) |
| `ForecastHandler` | Estimation accuracy forecasting | agent (`LlmForecastHandler`) |

### Defined in feature-finance (L4)

| Trait | Purpose | Implemented in |
|---|---|---|
| `FinanceHandler` | Proactive finance operations, budget alerts | agent (`FinanceHandlerImpl`) |

### Defined in feature-productivity (L4)

| Trait | Purpose | Implemented in |
|---|---|---|
| `ProductivityHandler` | LLM productivity insights | agent (`ProductivityHandlerImpl`) |

### Defined in feature-coaching (L4)

| Trait | Purpose | Implemented in |
|---|---|---|
| `CoachingReasonerHandler` | LLM-powered coaching decisions | agent |

### Defined in feature-insights (L4)

| Trait | Purpose | Implemented in |
|---|---|---|
| `FlashcardAccessor` | Access flashcard success rates | app-core (`FlashcardAccessorImpl`) |
| `InsightEmbedder` | Embed insight content | app-core (`InsightEmbedderImpl`) |
| `ScopeResolver` | Resolve related note IDs | app-core (`ScopeResolverImpl`) |

### Defined in cognitive (L5)

| Trait | Purpose | Implemented in |
|---|---|---|
| `ExtractionHandler` | Convert observations to facts | agent (`LlmExtractionHandler`, `HeuristicExtractionHandler`) |
| `ConsolidationHandler` | ADD/UPDATE/DELETE/NOOP decisions | agent (`LlmConsolidationHandler`, `HeuristicConsolidationHandler`) |
| `ReflectionHandler` | Weekly cross-domain pattern synthesis | agent |
| `TextEmbedder` | Text-to-vector embedding | agent (`TextEmbedderImpl`) |
| `SemanticFactEmbedder` | Fact embedding + vector storage | agent (`SemanticFactEmbedderImpl`) |

### Defined in context_engine (L3)

| Trait | Purpose | Implemented in |
|---|---|---|
| `ContextSource` | Pluggable system prompt injection | agent (13 implementations), skill-system |
| `MemoryRetriever` | Memory retrieval for context assembly | cognitive (`UnifiedMemoryService`) |
| `SummaryProvider` | Abstractive history compression | agent |
| `QueryDecomposer` | Query decomposition for InsightForge | agent |
| `DomainSearcher` | Domain-specific search for InsightForge | agent (4 implementations) |
| `TokenCounter` | Token estimation | context_engine (`TiktokenCounter`, `CharTokenCounter`) |

---

## 3. All FeaturePackage Implementations

| Package | Crate | Tools | Config Key | Migrations |
|---|---|---|---|---|
| `TasksFeature` | feature-tasks | (wired directly, not via tools()) | `tasks` | v1: tasks, task_activity, task_executions, task_suggestions |
| `FinanceFeature` | feature-finance | `FinanceTool` | `finance` | v1: accounts, transactions, budgets, portfolios, investments |
| `NotesFeature` | feature-notes | `NotesTool` | `notes` | v6: notebooks, notes, tags, links, versions, FTS5 |
| `ProductivityFeature` | feature-productivity | `ProductivityTool` | `productivity` | v1: activity events, focus sessions, daily summaries |
| `LauncherFeature` | feature-launcher | (none, UI-driven) | `launcher` | v1: frequencies, clipboard, FTS5 |

Note: `cognitive` and `activity-log` provide migrations separately (not via FeaturePackage). WASM plugins provide migrations via `PluginPackage`.

---

## 4. Key Flows

### Message -> Agent Runtime -> Tool -> Response

```
Channel.start() polls/receives message
  -> InboundMessage published to MessageBus
  -> AgentLoop.process_message()
    -> SessionManager.get_or_create(session_key)
    -> AgentRuntime.process_message()
      1. SkillRouter.select_orchestrator() -> SkillPackage
      2. Set active_profile
      3. Filter MCP tools by skill.mcp_tools
      4. IntentAnalyzer.analyze() -> IntentAnalysis {mode, signals, confidence}
      5. Confidence check (downgrade if low)
      6. ContextEngine.assemble() -> AssembledContext
      7. filter_tools_for_profile()
      8. ExecutionRouter.execute()
         Direct: single LLM call, no tools
         Reactive: ReAct loop (LLM call -> tool calls -> append results -> repeat)
      9. ResponseValidator.validate()
      10. CostTracker.record()
    <- RuntimeResult {content, mode_used, classification, agent_name}
  -> Session.save(assistant response)
  -> MessageBus.publish_outbound()
  -> Channel.send(formatted, split message)
```

### AppCore Initialization Sequence

```
Phase 1: Storage -- config load, SQLite connect, LanceDB connect, LLM provider with failover
Phase 2: Cron -- CronService with 10+ handlers, AI handlers (decomposition, forecast, proactive)
Phase 3: Agent -- PersonaManager, ActivityLog, AgentLoop builder (tools, context sources, skills)
Phase 4: Channels -- ChannelManager (Telegram, Discord, Slack, Email)
Phase 5: Productivity -- Engine, FocusManager, NudgeService, IntelligenceLayer, DistractionMonitor
Phase 6: Coaching -- SignalAccumulator, PatternDetector, InterventionRouter, CoachingService
Phase 7: Cognitive -- Persona seeding, file watcher, work context inference
Phase 8: Launcher -- 16 search sources, background refreshers
Post-init: insight refresh, note embedding, activity subscriber, analytics retention
```

### MCP Call Handling

```
Claude Code -> stdio JSON-RPC -> KlyntbotServerHandler
  tools/list -> ToolRegistryBridge.list_tools() -> filtered by exposedTools whitelist
  tools/call "tasks" -> ToolRegistryBridge.execute()
    -> whitelist check
    -> ToolRegistry.prepare() (clone Arc<dyn Tool>, drop lock)
    -> tool.execute(args, RoutingContext{channel: "mcp", chat_id: "mcp-session"})
    -> emit entity:updated event
  tools/call "agent" -> AgentBridge.execute()
    -> AppCore.chat_send(message, "mcp:{uuid}")
    -> collect AgentEvent stream into text response
```

---

## 5. Config Schema Highlights

Root struct: `Config` with 30+ sections. All `#[serde(rename_all = "camelCase")]`. File at `{KLYNTBOT_HOME}/config.json`.

### Key Sections

| Section | Key Fields | Purpose |
|---|---|---|
| `agents.defaults` | `model`, `maxTokens`, `temperature`, `maxToolIterations` | Default LLM settings |
| `providers.*` | `apiKey` (Secret), `apiBase`, `native`, `extendedThinking` | Per-provider config (12 providers) |
| `channels.*` | `enabled`, `token`, `allowFrom` | Platform channel config |
| `mcp.servers` | `name`, `transport`, `enabledTools`, `disabledTools` | External MCP server connections |
| `mcp.server` | `exposedTools`, `auth` | Klyntbot's own MCP server |
| `cognitive` | `dynamicFactsEnabled`, relevance weights, InsightForge config | Memory system tuning |
| `productivity` | `tracking`, `focus`, `nudges`, `privacy` | Productivity feature config |
| `finance` | `defaultCurrency`, `fire`, `budgeting`, `expectedReturns` | Finance feature config |
| `skills` | `extraSkillDirs`, `activationThreshold`, `maxActivatedSkills` | Skill system config |

### Adding New Config

1. Add a new struct to `crates/config/src/schema/` with `#[serde(rename_all = "camelCase")]` and `Default`
2. Add field to `Config` struct in `schema/core.rs`
3. Add env var handling in `env.rs` if needed
4. Config is auto-loaded with defaults on startup

---

## 6. Extension Points

### Adding a New Tool

1. Create params struct with `#[derive(ToolParams)]`
2. Create tool struct with `#[derive(Tool)]` + `#[tool(name = "...", params = "...")]`
3. Implement `ToolExecute` for business logic
4. Register in `AgentLoopBuilder` (agent crate)
5. For multi-action: use `#[tool_actions]` on impl block with `#[action(name = "...")]` methods

### Adding a Feature Package

1. Create `crates/feature-{name}/`
2. Implement `FeaturePackage` trait: `name()`, `tools()`, `migrations()`, `config_key()`, `default_config()`
3. Register in `app-core/src/init/` during the appropriate phase
4. Run migrations via `StoragePool::run_feature_migrations()`

### Adding a Channel

1. Create a module in `crates/channels/src/`
2. Implement the `Channel` trait: `start()`, `stop()`, `send()`, `is_allowed()`
3. Add config section in `crates/config/src/schema/channels.rs`
4. Register in `ChannelManager::initialize_channels()` via the `init_channel!` macro

### Exposing a New Tool via MCP

1. Implement the tool (register in ToolRegistry)
2. Add tool name to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`
3. Verify name matches: `cargo nextest run -p klyntbot-server`
4. Users can override in `config.json` -> `mcp.server.exposedTools`

### Adding a Skill

1. Create `skills/{name}/SKILL.md` with YAML frontmatter
2. Add reference files to `skills/{name}/references/`
3. Add `include_skill!("{name}")` to `BUILTIN_SKILLS` in `skill-system/src/discovery.rs`
4. Add `include_skill_reference!("{name}", "{ref}")` for each reference
5. Set `type: orchestrator` if it should be a routing target

### Adding a Plugin

1. Create directory in `~/.klyntbot/plugins/{name}/`
2. Write `klyntbot.plugin.json` manifest with tools, permissions, migrations
3. Compile WASM binary to `plugin.wasm`
4. Plugin tools auto-registered on startup via `PluginManager`

---

## 7. Tauri Command List (by Domain)

| Domain | Count | Key Commands |
|---|---|---|
| Tasks | 17 | `task_create`, `task_update`, `task_delete`, `task_toggle_complete`, `task_decompose`, `task_forecast` |
| Notes | 62 | `note_create`, `note_update`, `note_search`, `note_insight_review`, `flashcard_*` (14), `note_insight_*` (20) |
| Productivity | 33 | `productivity_today`, `productivity_focus_start/end`, `productivity_sessions`, `productivity_goals` |
| Finance | 27 | `finance_accounts`, `finance_transactions`, `finance_budget_*`, `finance_net_worth`, `finance_report_*` |
| Cognitive | 27 | `cognitive_user_model`, `cognitive_facts_list`, `coaching_situation`, `coaching_signals` |
| Work Contexts | 11 | `list_work_contexts`, `get_work_context_detail`, `get_dashboard_intelligence` |
| Launcher | 9 | `launcher_search`, `launcher_execute`, `launcher_clipboard_*` |
| Chat | 8 | `chat_send`, `chat_threads`, `chat_messages`, `chat_cancel` |
| Settings | 7+ | `mcp_get_config`, `mcp_add_server`, `config_get_section`, `config_update_section` |
| OKR | 8 | `objective_create/get/update/delete`, `key_result_create/update/delete/update_metric` |
| Workflows | 8 | `workflow_list/get/create/delete`, `label_create/update/delete/reorder` |
| Columns | 8 | `custom_column_list/create/update/delete/reorder`, `custom_column_values/value_set/value_delete` |
| Squads | 7 | `list_squads`, `create_squad`, `add_squad_member` |
| Cron | 7 | `cron_list`, `cron_create`, `cron_run`, `cron_enable` |
| Projects | 7 | `project_create/get/update/delete/archive`, `project_update_instructions` |
| Focus Timer | 7 | `focus_timer_start/stop/status/pause/resume/extend`, `focus_break_start` |
| Agents | 6 | `agent_list_profiles`, `agent_read_file`, `agent_create_profile` |
| Capture | 6 | `capture_status`, `capture_shell_hook_*` |
| Areas | 5 | `area_list/create/update/delete/reorder` |
| Annotations | 5 | `annotation_create/update/delete/list_for_note/get_ai_suggestion` |
| Language | 5 | `language_translate_breakdown`, `language_evaluate_translation` |
| Distraction | 5 | `distraction_dismiss/allow_temp/allow_session/learned_rules/delete_rule` |
| Entities | 3 | `entity_search`, `entity_merge`, `entity_get_neighborhood` |
| Window | 4 | `resize_window`, `open_url`, `show_dashboard`, `quit_app` |
| Permissions | 2 | `permissions_check_accessibility`, `permissions_open_accessibility` |
| Shortcuts | 2 | `shortcuts_get`, `shortcuts_update` |

---

## 8. Sub-agent Pattern and Dependency Inversion

### The Arc<dyn Trait> Pattern

Lower-layer crates define handler traits. Higher-layer crates provide implementations. This prevents circular dependencies.

```rust
// In feature-tasks (L4):
#[async_trait]
pub trait DecompositionHandler: Send + Sync {
    async fn decompose(&self, task: &Task, context: &str) -> Result<DecompositionPlan>;
}

// In agent (L5):
pub struct LlmDecompositionHandler {
    provider: DynProvider,
    repos: Repos,
}

#[async_trait]
impl DecompositionHandler for LlmDecompositionHandler {
    async fn decompose(&self, task: &Task, context: &str) -> Result<DecompositionPlan> {
        // LLM call to decompose task
    }
}

// Injection in AgentLoopBuilder:
task_tool.with_decomposition_handler(Arc::new(handler) as Arc<dyn DecompositionHandler>)
```

### Construction Pattern

Tools are built with a builder pattern:

```rust
TaskTool::new(repo, max_focus_slots, focus_deadline_hours, timezone)
    .with_enrichment_handler(handler)
    .with_decomposition_handler(decomp_handler)
    .with_execution_handler(exec_handler)
    .with_planning_handler(plan_handler)
    .with_domain_bus(bus)
```

Each `.with_*()` method takes `Arc<dyn Trait>`. Missing handlers cause those features to gracefully degrade (return "feature not available" instead of panicking).

### Sub-agent Delegation

`AgentRuntime` implements `DelegationHandler` for multi-agent composition:

1. Orchestrator calls `delegate` tool with target skill name and query
2. `AgentRuntime.handle_delegation()` is called
3. Looks up delegated skill's `SkillPackage` from catalog
4. Sets delegated skill as `active_profile`
5. Builds context with delegated skill's instructions
6. Filters tools to delegated skill's whitelist
7. Executes via router with reduced iteration budget (max 8)
8. Returns result to calling orchestrator

Maximum delegation depth: 2 (prevents infinite recursion).

### Key Principle

If a new tool needs access to the agent runtime, LLM providers, or cross-crate services: define a trait in the tool's crate, implement it in the agent crate, inject as `Arc<dyn Trait>`. Never import agent from a feature crate.
