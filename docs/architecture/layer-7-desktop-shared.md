# Layer 7: desktop-shared

> `crates/desktop-shared/` -- Shared IPC types between the Rust backend and the TypeScript frontend. Defines all request/response structs, event payloads, error types, and permission utilities.

## Overview

`desktop-shared` is a lightweight crate with no runtime logic. It provides the type contract between the Tauri backend (`desktop` crate) and the React frontend (`desktop-ui/`). Both the Tauri commands and the dev server reference these types for serialization.

## Dependencies

```
common, activity-log, serde, serde_json, chrono
```

## Module Structure

```
src/
  lib.rs                  -- Module declarations + re-export of entity_link_types::*
  types.rs                -- Core enums (Priority, Status, EntityKind, CronJob types)
  errors.rs               -- ApiError struct + From<KlyntbotError> conversion
  events.rs               -- 40+ event name constants + 50+ event payload structs
  cognitive_commands.rs    -- Memory/coaching/system DTOs
  entity_link_types.rs     -- Entity link types + cross-entity summary responses
  permissions.rs           -- macOS permission checks (Screen Recording, Accessibility)
  commands/
    mod.rs                -- Submodule declarations + glob re-exports
    agents.rs             -- Agent profile/file types
    annotations.rs        -- Annotation CRUD types + linked context + AI suggestion
    areas.rs              -- Area CRUD types
    calendar.rs           -- Calendar event input type
    capture.rs            -- Ingestion request/response + shell hook status
    chat.rs               -- Chat thread/message responses + session context input
    entities.rs           -- Knowledge graph entity types
    finance.rs            -- Finance CRUD types (accounts, transactions, budgets, goals, liabilities, portfolios, investments, reports)
    integrations.rs       -- AI tool detection + installation types
    language.rs           -- Translation/vocabulary/confusable types
    notes.rs              -- Note/notebook CRUD + insight review + flashcards + persona chat
    okr.rs                -- Objective/KeyResult CRUD types
    productivity.rs       -- Productivity summary, focus, tracking, insights, patterns types
    projects.rs           -- Project CRUD types
    settings.rs           -- MCP config + app info types
    squads.rs             -- Persona squad types
    tasks.rs              -- Task CRUD + suggestions + decomposition + forecast + workflows + groups + columns
    timeline.rs           -- Timeline query/response + entry types
    work_context.rs       -- Work context CRUD + inference stats + dashboard intelligence
    workspace.rs          -- Workspace file types
```

## Type Naming Conventions

All IPC types follow strict naming conventions:

| Pattern | Usage | Example |
|---------|-------|---------|
| `*Response` | Data returned to frontend | `TaskResponse`, `NoteResponse` |
| `*CreateParams` | Fields for creating an entity | `TaskCreateParams`, `NoteCreateParams` |
| `*UpdateParams` | Partial update fields (all optional except `id`) | `TaskUpdateParams`, `NoteUpdateParams` |
| `*FilterParams` | Query filter parameters | `FinanceTransactionFilterParams` |
| `*Payload` | Event payload structs | `ContentChunkPayload`, `FocusTickPayload` |
| `*Input` | Frontend-to-backend input types | `CalendarEventInput`, `SessionContextInput` |

## Serialization

All types use `#[serde(rename_all = "camelCase")]` for JavaScript-friendly field names. Key serialization patterns:

- **Optional fields**: `#[serde(skip_serializing_if = "Option::is_none")]` to omit null values
- **Triple-option pattern**: `Option<Option<String>>` for distinguishing absent (don't change) vs null (clear) vs value (set). Uses custom `deserialize_nullable_field` deserializer.
- **Tagged enums**: `MessageSegment` uses `#[serde(tag = "type")]` for discriminated unions
- **Default collections**: `#[serde(skip_serializing_if = "Vec::is_empty", default)]` for empty arrays

## ApiError

The standardized error type for all Tauri commands and dev server responses:

```rust
pub struct ApiError {
    pub code: String,    // Machine-readable: "NOT_FOUND", "INVALID_PARAMS", etc.
    pub message: String, // Human-readable description
}
```

Implements `From<KlyntbotError>` with structured code mapping:

| Error Variant | Error Code |
|--------------|------------|
| `ToolError::NotFound` | `TOOL_NOT_FOUND` |
| `ToolError::InvalidParams` | `INVALID_PARAMS` |
| `ToolError::ExecutionFailed` | `EXECUTION_FAILED` |
| `ToolError::PermissionDenied` | `PERMISSION_DENIED` |
| `ProviderError::RateLimited` | `RATE_LIMITED` |
| `ProviderError::AuthFailed` | `AUTH_FAILED` |
| `SessionError::NotFound` | `SESSION_NOT_FOUND` |
| `StorageNotFound` | `NOT_FOUND` |
| `StorageConflict` | `CONFLICT` |

## Event System

### Event Name Constants

40+ event name constants defined as `&str` for type-safe Tauri event emission:

#### Agent Events (prefixed `agent:`)
| Constant | Value | Description |
|----------|-------|-------------|
| `AGENT_CONTENT_CHUNK` | `agent:content_chunk` | Streaming text chunk |
| `AGENT_DONE` | `agent:done` | Streaming complete |
| `AGENT_TOOL_START` | `agent:tool_start` | Tool execution started |
| `AGENT_TOOL_END` | `agent:tool_end` | Tool execution finished |
| `AGENT_ERROR` | `agent:error` | Agent error |
| `AGENT_ENTITY_CREATED` | `agent:entity_created` | Entity created by agent |
| `AGENT_INTERACTION_REQUEST` | `agent:interaction_request` | ask_user form request |
| `AGENT_CLASSIFICATION_COMPLETE` | `agent:classification_complete` | Intent classified |
| `AGENT_EXECUTION_STARTED` | `agent:execution_started` | Execution engine started |
| `AGENT_ITERATION_START` | `agent:iteration_start` | ReAct loop iteration |
| `AGENT_CONFIDENCE_ASSESSED` | `agent:confidence_assessed` | Confidence score |
| `AGENT_USAGE_REPORT` | `agent:usage_report` | Token usage + cost |
| `AGENT_MEMORY_ACCESS` | `agent:memory_access` | Memory retrieval |
| `AGENT_SKILL_LOADED` | `agent:skill_loaded` | Skill activated |
| `AGENT_LEARNING_EVENT` | `agent:learning_event` | Learning event |
| `AGENT_SUBAGENT_SPAWNED` | `agent:subagent_spawned` | Sub-agent created |
| `AGENT_SELECTED` | `agent:agent_selected` | Agent selected for task |
| `AGENT_DELEGATION_STARTED` | `agent:delegation_started` | Multi-agent delegation |
| `AGENT_DELEGATION_COMPLETED` | `agent:delegation_completed` | Delegation finished |
| `AGENT_PLAN_GENERATED` | `agent:plan_generated` | Execution plan created |
| `AGENT_PLAN_STEP_COMPLETED` | `agent:plan_step_completed` | Plan step done |
| `AGENT_BUDGET_WARNING` | `agent:budget_warning` | Cost budget threshold |
| `AGENT_PERSONA_PERSPECTIVE` | `agent:persona_perspective` | Persona perspective content |
| `AGENT_DEBATE_ROUND_STARTED` | `agent:debate_round_started` | Squad debate round |
| `AGENT_DEBATE_ROUND_COMPLETED` | `agent:debate_round_completed` | Debate round done |
| `AGENT_DEBATE_JUDGE_DECISION` | `agent:debate_judge_decision` | Judge decision |
| `AGENT_CONSENSUS_REACHED` | `agent:consensus_reached` | Squad consensus |
| `AGENT_MEMORY_PROMOTED` | `agent:memory_promoted` | Fact scope promotion |

#### Entity Events
| Constant | Value | Description |
|----------|-------|-------------|
| `ENTITY_UPDATED` | `entity:updated` | Entity mutation notification |

#### MCP Events
| Constant | Value | Description |
|----------|-------|-------------|
| `MCP_OAUTH_COMPLETE` | `mcp:oauth_complete` | OAuth flow completed |
| `MCP_OAUTH_ERROR` | `mcp:oauth_error` | OAuth flow error |
| `MCP_SERVER_STATUS` | `mcp:server_status` | MCP server status change |
| `MCP_STARTUP_COMPLETE` | `mcp:startup_complete` | All MCP servers initialized |

#### Productivity Events
| Constant | Value | Description |
|----------|-------|-------------|
| `PRODUCTIVITY_DISTRACTION` | `productivity:distraction` | Distraction detected |
| `PRODUCTIVITY_NUDGE` | `productivity:nudge` | Break/burnout nudge |
| `ACTIVITY_TICK` | `activity:tick` | Activity tracker tick |
| `ACTIVITY_SWITCH` | `activity:switch` | App switch detected |
| `FOCUS_STATE_CHANGED` | `focus:state_changed` | Focus state transition |
| `FOCUS_AUTO_DETECTED` | `focus:auto_detected` | Auto-focus detected |
| `FOCUS_AUTO_STARTED` | `focus:auto_started` | Auto-focus started |
| `FOCUS_TICK` | `focus:tick` | Focus timer tick (1/sec) |
| `FOCUS_COMPLETED` | `focus:completed` | Focus/break timer done |
| `DISTRACTION_DETECTED` | `distraction:detected` | Distracting app banner |
| `SCORE_UPDATED` | `score:updated` | Productivity score change |
| `BUCKET_COMPLETED` | `bucket:completed` | Time bucket completed |
| `INSIGHT_GENERATED` | `insight:generated` | Productivity insight |

#### Coaching Events
| Constant | Value | Description |
|----------|-------|-------------|
| `COACHING_INTERVENTION` | `coaching:intervention` | Coaching nudge delivery |
| `DISTRACTION_INTERVENTION` | `distraction:intervention` | Distraction overlay |
| `DISTRACTION_VERDICT` | `distraction:verdict` | LLM distraction verdict |

### TransparencyData

Accumulated transparency metadata attached to each assistant message. Contains all agent execution details for the frontend's transparency panel:

```
TransparencyData
  +-- usage: TransparencyUsage (prompt/completion/cache tokens)
  +-- cost: TransparencyCost (estimated_usd, model)
  +-- timing: TransparencyTiming (total_ms, classification_ms, context_assembly_ms)
  +-- tools: Vec<TransparencyTool> (name, action, success, duration_ms, estimated_tokens)
  +-- tool_tokens_total: Option<u32>
  +-- memory_accesses: Vec<TransparencyMemoryAccess> (action, query, results_count)
  +-- skills: Vec<TransparencySkill> (name, trigger)
  +-- execution: TransparencyExecution (engine, iterations, max_iterations, escalations)
  +-- classification: TransparencyClassification (strategy, confidence, source)
  +-- agent_selected: TransparencyAgentSelected (name, description)
  +-- subagents: Vec<TransparencySubagent> (label, profile)
  +-- learning: Vec<TransparencyLearning> (event_type, detail)
  +-- delegations: Vec<TransparencyDelegation> (from/to agent, query, depth, status)
  +-- plan: TransparencyPlan (steps, completed_steps)
```

### MessageSegment

Typed segments within structured assistant messages:

```rust
pub enum MessageSegment {
    Text { content: String },
    Tool { name, action, success, duration_ms, result, estimated_tokens, agent },
}
```

## EntityKind

Central enum for entity mutation tracking. Used by `entity:updated` events to tell the frontend which data to invalidate:

```rust
pub enum EntityKind {
    Task, Project, Objective, Area, KeyResult, FocusSession,
    Productivity, Note, Notebook, Finance, Source, Conversation,
}
```

Includes `EntityKind::parse(s: &str)` for case-insensitive string parsing with aliases (e.g., "action" -> Task, "keyresult" -> KeyResult, "finance_account" -> Finance).

## Permissions

macOS-specific permission checks for productivity tracking:

| Function | Purpose |
|----------|---------|
| `check_screen_recording()` | Check if Screen Recording permission is granted |
| `request_screen_recording()` | Prompt the user for Screen Recording permission |
| `open_screen_recording_settings()` | Open System Settings Privacy panel |
| `check_accessibility()` | Check if Accessibility permission is granted |
| `open_accessibility_settings()` | Open System Settings Accessibility panel |

All functions use `extern "C"` bindings (`CGPreflightScreenCaptureAccess`, `CGRequestScreenCaptureAccess`, `AXIsProcessTrusted`) on macOS and return `true` on other platforms.

## Complete IPC Type Catalog

### Tasks Domain
- `TaskResponse`, `TaskCreateParams`, `TaskUpdateParams`, `TodayTaskResponse`
- `SuggestionResponse`, `DecompositionResponse`, `PlannedSubtaskResponse`
- `TaskForecastResponse`, `ForecastRiskResponse`
- `StatusWorkflowResponse`, `StatusLabelResponse`, `WorkflowCreateParams`, `LabelCreateParams`, `LabelUpdateParams`, `LabelReorderParams`
- `TaskGroupResponse`, `TaskGroupCreateParams`, `TaskGroupUpdateParams`, `TaskGroupReorderParams`
- `CustomColumnResponse`, `CustomColumnValueResponse`, `ColumnCreateParams`, `ColumnUpdateParams`, `ColumnReorderParams`, `ColumnValueSetParams`

### Notes Domain
- `NoteResponse`, `NoteCreateParams`, `NoteUpdateParams`
- `NotebookResponse`, `NotebookCreateParams`, `NotebookUpdateParams`
- `NoteLinkResponse`, `NoteVersionResponse`
- `HybridSearchResponse`, `InboxItemResponse`, `InboxCreateParams`
- `NoteSuggestionsResponse`, `ScoredNoteResponse`, `LinkSuggestionResponse`, `BacklinkResponse`
- `InsightReviewStarted`, `InsightReviewResponse`, `TabContent`, `InsightVersionResponse`
- `InsightEvolutionResponse`, `InsightEvolutionPoint`
- `ScenarioChallengeResponse`, `ChangesSummaryResponse`, `KnowledgeGrowthResponse`, `DomainCount`
- `InsightScopeConfigParams`, `InsightSaveFlashcardsParams`, `InsightQuizSubmitParams`
- `FlashcardResponse`, `FlashcardCreateParams`, `FlashcardUpdateParams`, `FlashcardListParams`, `FlashcardReviewParams`
- `DeckSummaryResponse`, `FlashcardGenerateParams`, `FlashcardGenerateResponse`, `GeneratedCardPreview`, `FlashcardSaveGeneratedParams`
- `PersonaResponse`, `PersonaMetaResponse`, `CreatePersonaParams`, `UpdatePersonaParams`, `SetPersonaPinsParams`, `RatePersonaParams`
- `PersonaChatParams`, `PersonaChatMessage`, `PersonaChatResponse`
- `QuizQuestion`

### Annotations Domain
- `AnnotationCreateParams`, `AnnotationUpdateParams`, `AnnotationResponse`
- `LinkedContextParams`, `LinkedContextResponse`, `LinkedFact`, `LinkedMemory`, `LinkedRule`
- `AiSuggestionResponse`

### Language Domain
- `TranslateBreakdownParams`, `TranslateBreakdownResponse`, `WordBreakdown`, `GrammarPattern`
- `EvaluateTranslationParams`, `TranslationEvalResponse`, `EvalGrades`, `Correction`
- `VocabularySaveParams`, `VocabItem`
- `DetectConfusablesParams`, `ConfusableResponse`
- `EnrichAnnotationParams`, `AnnotationEnrichmentResponse`

### Chat Domain
- `ChatThreadResponse`, `ChatMessageResponse`, `SessionContextInput`

### Finance Domain
- `FinanceAccountCreateParams`, `FinanceAccountUpdateParams`
- `FinanceTransactionCreateParams`, `FinanceTransactionUpdateParams`, `FinanceTransactionFilterParams`
- `FinanceBudgetCreateParams`, `FinanceBudgetUpdateParams`
- `FinanceGoalCreateParams`, `FinanceGoalUpdateParams`
- `FinanceLiabilityCreateParams`, `FinanceLiabilityUpdateParams`
- `FinancePortfolioCreateParams`, `FinancePortfolioResponse`
- `FinanceInvestmentCreateParams`, `FinanceInvestmentUpdateParams`
- `FinanceNetWorthResponse`, `CurrencyNetWorth`
- `FinanceDateRangeParams`, `FinanceCategoryReportResponse`, `FinanceCategoryBreakdown`
- `FinanceTrendPoint`, `FinanceMonthlySummaryResponse`, `FinanceDailySpendingResponse`, `DailySpending`, `FinancePeriodSummaryResponse`

### Productivity Domain
- `ProductivitySummaryResponse`, `AppUsageResponse`, `CategoryUsageResponse`, `ProjectUsageResponse`, `TrackedAppResponse`
- `FocusSessionResponse`, `FocusTimerStatusResponse`
- `IntelligenceSessionResponse`, `ActivityTimelineResponse`
- `ActivityCategoryResponse`, `CategoryRulesResponse`
- `GoalProgressResponse`, `TimeEntryResponse`
- `InsightCardResponse`, `WeeklyAssessmentResponse`
- `ProductivityPatternsResponse`, `HourlyBreakdownResponse`
- `ProductivityProjectResponse`, `DistractionResponse`, `LearnedRuleResponse`

### Areas / Projects / OKR Domain
- `AreaResponse`, `AreaCreateParams`, `AreaUpdateParams`
- `ProjectResponse`, `ProjectCreateParams`, `ProjectUpdateParams`
- `ObjectiveResponse`, `ObjectiveCreateParams`, `ObjectiveUpdateParams`
- `KeyResultResponse`, `KeyResultCreateParams`, `KeyResultUpdateParams`

### Entity Links Domain
- `EntityLinkResponse`, `EntityLinkCreateParams`, `EntityLinksForEntityParams`
- `LinkedEntitiesResponse`, `ActionSummaryResponse`, `NoteSummaryResponse`, `SessionSummaryResponse`
- `ObjectiveSummaryResponse`, `KeyResultSummaryResponse`, `ProjectSourceResponse`
- `ProjectSourceCreateParams`, `ProjectSourceUpdateParams`

### Cognitive Domain
- `SemanticFactResponse`, `EpisodicMemoryResponse`, `ProceduralRuleResponse`
- `UserModelSummaryResponse`, `MemoryStatsResponse`
- `UserSituationResponse`, `SignalResponse`, `TriggerConditionResponse`, `SignalWindowResponse`
- `DetectedPatternResponse`, `DeliveredInterventionResponse`
- `StrategyFeedbackResponse`, `RouterStatusResponse`
- `DomainEventPayload`, `ComponentStatusResponse`, `SystemStatusResponse`
- `FactCreateParams`, `FactUpdateParams`, `RuleCreateParams`
- `CompactionResultResponse`, `ReflectionResultResponse`

### Work Context Domain
- `WorkContextResponse`, `WorkResourceResponse`, `WorkContextDetailResponse`, `ActivityEventResponse`
- `ContextTimelineBlockResponse`, `ContextResumeResponse`, `WorkContextUpdateParams`
- `InferenceStatsResponse`, `InferenceConfigUpdate`
- `DashboardIntelligenceResponse`, `WorkContextSummary`, `SessionBlock`, `DashboardNudge`, `ResourceCluster`

### Entities (Knowledge Graph)
- `EntityResponse`, `EntitySearchParams`, `EntityMergeParams`
- `EntityNeighborhoodResponse`, `EntityRelationshipResponse`

### Settings Domain
- `AgentStatusResponse`, `AppInfoResponse`
- `McpConfigResponse`, `McpServerResponse`, `McpAddServerParams`, `McpToggleParams`, `McpRemoveParams`, `McpUpdateServerParams`
- `OAuthStartParams`

### Squads Domain
- `SquadResponse`, `SquadMemberResponse`, `CreateSquadParams`, `UpdateSquadParams`, `SquadMemberParams`

### Agents Domain
- `AgentProfileSummary`, `AgentFileSummary`, `AgentFileContent`

### Workspace / Capture / Integrations
- `WorkspaceFile`, `WorkspaceFileContent`
- `IngestRequest`, `IngestResponse`, `BatchIngestResponse`, `ShellHookStatusResponse`, `CaptureStatusResponse`
- `AiToolInfo`, `AiToolsInstallParams`, `AiToolInstallResult`

### Timeline Domain
- `TimelineQuery`, `TimelineResponse`, `TimelineEntry`, `TimelineSummary`
- `TimelineSource` (enum: Productivity, Focus, Task, Todo, Note, Finance, System, Calendar)
- `TimelineEntryType` (enum: AppUsage, FocusSession, TaskTimeEntry, TaskCreated, TaskCompleted, TaskUpdated, TaskDue, NoteCreated, NoteUpdated, TransactionRecorded, ExpenseRecorded, IncomeRecorded, SystemEvent, CalendarEvent, TaskStatusChanged, TaskPriorityChanged, TaskFieldUpdated)
- `TopAppSummary`, `SourceBreakdown`

### Core Types
- `CronJobResponse`, `CronPayloadResponse`, `CronJobStateResponse`, `CronJobCreateParams`, `CronJobUpdateParams`, `CronStatusResponse`
- `Priority` (P1-P4), `Status` (Todo/Doing/Done/Archived), `AreaFilter`, `ViewMode`, `SidebarItem`, `EntityKind`
