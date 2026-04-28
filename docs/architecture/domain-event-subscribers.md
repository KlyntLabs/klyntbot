# Domain Event Subscriber Registry

Living index of which subsystems consume which `DomainEvent` variants. Update
when adding a new subscriber or event variant.

## Events → Subscribers

| Event | Subscriber | File | Effect |
|---|---|---|---|
| `ChatTurnCompleted` | ChatTurnCollector | `cognitive/src/pipeline/chat_turn_collector.rs` | Emits `CognitiveSignal` for extraction |
| `ChatTurnCompleted` | RecallCollector | `cognitive/src/pipeline/recall_collector.rs` | Buffers turn for recall ranking |
| `ChatTurnCompleted` | AiPipeline | `app-core/src/init/ai_pipeline.rs` | Converts to `AiSignal` for routing |
| `SessionEnded` | SessionCollector | `cognitive/src/pipeline/session_collector.rs` | Buffers session for episodic memory |
| `ToolCallExecuted` | CodingMemoryDistiller | `coding-memory/src/distiller/mod.rs` | Feeds turn buffer for distillation |
| `ToolCallExecuted` | ToolRegistryBridge (publisher) | `klyntbot-server/src/bridge/registry.rs` | Publishes after MCP execution |
| `TaskCreated` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Upserts entity into semantic memory |
| `TaskCompleted` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Counter + extraction trigger |
| `TaskDeferred` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `TransactionRecorded` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Upserts entity |
| `BudgetAlert` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `ActivitySessionCompleted` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `FocusSessionStarted` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `FocusSessionEnded` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `DistractionDetected` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `ProductivityScoreComputed` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `UserStatedFact` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `UserCorrectedAI` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger + mirror routing |
| `CoachingFeedback` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `NoteCreated` | BackgroundConsolidationService | `cognitive/src/services/background.rs` | Extraction trigger |
| `UserCorrectedAI` | MirrorRoutingSource | `cognitive/src/mirror/sources/routing.rs` | Records correction for drift detection |
| `BudgetAlert` | FinanceDriftSource | `cognitive/src/mirror/sources/finance_drift.rs` | Tracks budget drift |
| `NoteContentChanged` | NoteTreeBuilder | `agent/src/adapters/note_tree_builder.rs` | Rebuilds note tree |
| `NoteContentChanged` | CommunityBuilder | `agent/src/adapters/community_builder.rs` | Updates community context |
| `NoteEditingFinished` | AtomExtractionService | `cognitive/src/services/atom_extraction.rs` | Triggers knowledge-atom extraction |
| `FocusSessionStarted` | LiveContextRefresher | `agent/src/execution/live_context_refresher.rs` | Pushes `ContextUpdate` |
| `FocusSessionStarted` | AgentLoop | `agent/src/agent_loop/mod.rs` | Updates focus state |
| `FocusSessionStarted` | BrainVoice | `app-core/src/brain_voice.rs` | Voice notification |
| `FocusSessionEnded` | AgentLoop | `agent/src/agent_loop/mod.rs` | Clears focus state |
| `FocusSessionEnded` | BrainVoice | `app-core/src/brain_voice.rs` | Voice notification |
| `FocusSessionEnded` | ProductivityTreeBuilder | `agent/src/adapters/productivity_tree_builder.rs` | Updates productivity tree |
| `ProductivityScoreComputed` | ProductivityTreeBuilder | `agent/src/adapters/productivity_tree_builder.rs` | Updates productivity tree |
| `KnowledgeAtomAccepted` | LearningTreeBuilder | `agent/src/adapters/learning_tree_builder.rs` | Updates learning tree |
| `TransactionRecorded` | FinanceTreeBuilder | `agent/src/adapters/finance_tree_builder.rs` | Updates finance tree |
| `BudgetAlert` | FinanceTreeBuilder | `agent/src/adapters/finance_tree_builder.rs` | Updates finance tree |
| `TaskCreated` | TimelineHandler | `app-core/src/handlers/timeline.rs` | Appends timeline entry |
| `TaskCompleted` | TimelineHandler | `app-core/src/handlers/timeline.rs` | Appends timeline entry |
| `NoteCreated` | TimelineHandler | `app-core/src/handlers/timeline.rs` | Appends timeline entry |
| `NoteUpdated` | TimelineHandler | `app-core/src/handlers/timeline.rs` | Appends timeline entry |
| `TransactionRecorded` | TimelineHandler | `app-core/src/handlers/timeline.rs` | Appends timeline entry |
| `SystemDidWake` | TemporalScheduler | `app-core/src/init/temporal_scheduler.rs` | Reschedules alarms after wake |
| `UserReturned` | WakeOrchestrator | `app-core/src/wake_orchestrator.rs` | Triggers context refresh |
| `AlarmFired` | FocusEndSubscriber | `app-core/src/focus/end_subscriber.rs` | Ends focus session on alarm |
| `AlarmFired` | AlarmSideEffects | `feature-tasks/src/alarm_side_effects.rs` | Executes task alarm side effects |
| `AlarmFired` | FocusAlarms | `feature-tasks/src/focus_alarms.rs` | Manages focus alarm lifecycle |
| `AlarmFired` | CronExecutor | `scheduling/src/temporal/cron_executor.rs` | Dispatches cron job execution |
| `TaskFocusChanged` | FocusAlarms | `feature-tasks/src/focus_alarms.rs` | Updates focus alarm deadline |
| `HeldNotificationReleased` | NotificationDispatcher | `notifications/src/dispatcher.rs` | Dispatches released notification |
| `UserStatedFact` | MemoryTool | `tools/src/domain/memory_tool.rs` | Stores user-stated fact |
| `AutotunerDecision` | FsrsWriteback | `agent/src/adapters/autotuner_bridge.rs` | Writes FSRS params on promotion |
| `CodingSessionEnded` | SessionEndPass | `app-core/src/coding_memory/reforge.rs` | Runs light session-end reforge |

## Adding a new subscriber

1. Find the event variant in `bus/src/domain_events.rs`.
2. Add a `bus.subscribe()` call in your subsystem's init path.
3. Append a row to the table above.
4. If the event doesn't exist, add it to `DomainEvent` and document why this
   subsystem is the canonical publisher.
