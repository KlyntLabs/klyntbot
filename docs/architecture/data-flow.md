# Data Flow

## End-to-End Message Flow

### Chat Platform Message Lifecycle

```
+----------+   +----------+   +----------+   +----------+
| Telegram |   | Discord  |   |  Slack   |   |  Email   |
| (HTTP    |   | (WSS     |   | (WSS     |   | (IMAP    |
|  poll)   |   |  gateway)|   |  socket) |   |  poll)   |
+----+-----+   +----+-----+   +----+-----+   +----+-----+
     |              |              |              |
     |  Normalize to InboundMessage (channel, sender_id, chat_id, content)
     |              |              |              |
     v              v              v              v
+----+--------------+--------------+--------------+-----+
|                    MessageBus.inbound_tx               |
|                    (mpsc channel)                      |
+---------------------------+----------------------------+
                            |
                            v
                    AgentLoop::run_with_rx()
                            |
                      +-----+------+
                      | Validation |  (reject > 64KB)
                      +-----+------+
                            |
                      +-----+------+
                      | Reaction?  |--Yes--> strategy_repo.set_satisfaction()
                      +-----+------+         emit correction signal
                            |No
                      +-----+------+
                      | Correction |--Yes--> DomainEvent::UserCorrectedAI
                      | detected?  |
                      +-----+------+
                            |
                      +-----+------+
                      | Session    |  get_or_create(session_key)
                      | Manager    |  append user message
                      +-----+------+  capture history
                            |
                      +-----+------+
                      | Pipeline   |  context_engine.build_system_prompt()
                      | Execution  |  runtime.process_message()
                      +-----+------+  (12-step pipeline)
                            |
                      +-----+------+
                      | Save to    |  session.add_assistant_message()
                      | Session    |
                      +-----+------+
                            |
                      +-----+------+
                      | Publish    |  DomainEvent::ChatTurnCompleted
                      +-----+------+
                            |
                            v
+---------------------------+----------------------------+
|                    MessageBus.outbound_tx               |
+----+--------------+--------------+--------------+------+
     |              |              |              |
     v              v              v              v
+----+-----+   +----+-----+   +----+-----+   +----+-----+
| Telegram |   | Discord  |   |  Slack   |   |  Email   |
| send     |   | send     |   | send     |   | SMTP     |
+----------+   +----------+   +----------+   +----------+
```

### Channel-Specific Details

**Telegram:**
1. Long-polling `getUpdates` (30s timeout) with offset tracking
2. Voice messages: downloaded → transcribed via Groq Whisper API
3. Images/documents: downloaded to temp path, referenced as `[image: /path]`
4. Allowlist check via `config.channels.telegram.allow_from`
5. Typing indicator: `sendChatAction` every 4s via `TypingManager`
6. Retry: up to 3 attempts per API call

**Discord:**
1. WebSocket Gateway v10 (raw, no library)
2. op 10 (HELLO) → heartbeat loop + IDENTIFY
3. op 0 (DISPATCH) → `MESSAGE_CREATE`, `MESSAGE_REACTION_ADD`
4. Max attachment: 20 MB
5. Shared `WebSocketManager` abstraction

**Slack:**
1. Socket Mode (WebSocket), not Events API
2. Gets WSS URL via `apps.connections.open`
3. Timeout-based heartbeat (ping if no message in 30s)
4. Envelope ACK required for each message
5. Shared `WebSocketManager` abstraction

**Email:**
1. IMAP polling: `SELECT INBOX` → `SEARCH UNSEEN` → `FETCH BODY[]`
2. HTML stripped via `html2text`
3. MIME parsed via `mail-parser`
4. In-memory dedup: `processed_uids: HashSet` (not persisted — restart reprocesses)
5. Reply threading via `In-Reply-To` / `Message-ID` headers
6. Feature-gated: `channels::email` (on by default)
7. Requires `consent_granted = true` in config

### Desktop UI Message Flow

```
User types message
     |
     v
useMutation("chat_send")
     |
     +--[Tauri mode]---------> invoke("chat_send", args)
     |                         Tauri IPC bridge
     |
     +--[Browser dev mode]---> fetch("/api/chat_send", POST)
                                dev server port 3456
     |
     v
AppCore::chat_send(content, session_key, context)
     |
     v
agent.process_direct_streaming()
     |
     v
StreamingHandle { event_rx, interaction_rx, cancel_token, handle }
     |
     +--cancel previous stream (same session_key, via active_streams DashMap)
     |
     v
+----+----+                        +----+----+
| Tauri   |                        | Dev SSE |
| Emitter |                        | Emitter |
+----+----+                        +----+----+
     |                                  |
     v                                  v
tauri::emit()                    broadcast::Sender
(native IPC)                     -> GET /api/events/{key}
     |                                  |
     v                                  v
Frontend event listeners:
  agent:content_chunk    -- streaming text tokens
  agent:tool_start       -- tool execution begins
  agent:tool_end         -- tool execution ends
  agent:done             -- pipeline complete
  agent:error            -- pipeline error
  entity:updated         -- cache invalidation
```

### MCP Tool Call Flow

```
External Client (Claude Code, Cursor)
     |
     v
klyntbot-mcp serve --stdio
     |
     v
JSON-RPC over stdin/stdout (rmcp)
     |
     +--initialize--------> KlyntbotServerHandler::get_info()
     |                       capabilities: tools + resources
     |
     +--tools/list---------> list_tools()
     |                       get_status + agent + bridge.list_tools()
     |
     +--tools/call---------> call_tool(name, arguments)
     |   |
     |   +--"get_status"---> inline handler (system info)
     |   |
     |   +--"agent"--------> AgentBridge::execute()
     |   |                   AppCore::chat_send() → full pipeline
     |   |                   collects AgentEvent stream
     |   |                   returns concatenated response + tool log
     |   |
     |   +--other----------> ToolRegistryBridge::execute()
     |                       whitelist check → registry.prepare()
     |                       tool.execute(args, ctx)
     |                       → CallToolResult::success(Content::text(...))
     |
     +--resources/list-----> 4 static URIs:
     |                       klyntbot://status
     |                       klyntbot://memory/recent
     |                       klyntbot://tasks/today
     |                       klyntbot://config/skills
     |
     +--resources/read-----> read_resource(uri)
```

## Event-Driven Architecture

### Domain Event Bus

`DomainEventBus` wraps `tokio::sync::broadcast::Sender<DomainEvent>` (fan-out to all subscribers):

```
                    DomainEventBus
                         |
         +-------+-------+-------+-------+
         |       |       |       |       |
         v       v       v       v       v
     Cognitive Activity  Mirror  Tauri  Dev
     Background  Log    Engine  Event   SSE
     Service   Subscriber       Relay   Relay
```

### Other Event Buses

**MessageBus** (`tokio::mpsc`, single-consumer): Inbound messages (channels → AgentLoop) and outbound messages (AgentLoop → ChannelManager). Receivers taken once via `take_inbound_rx()` / `take_outbound_rx()`.

**ContextUpdateQueue** (`Mutex<Vec>`, drain-based): Producers push live context updates (e.g., memory promotions). `LiveContextRefresher` drains at each ReAct iteration boundary. 30s dedup window.

**LearningEventBus** (`tokio::broadcast`, fan-out): Carries `LearningEvent::AnalysisCompleted` from the learning background analysis loop. Consumer: adaptive confidence threshold adjuster in `feature-learning`.

### Event Producer Map

| Producer | Events Published |
|----------|-----------------|
| `AgentLoop` | `ChatTurnCompleted`, `UserCorrectedAI` |
| `AppCore` (chat handler) | `ChatTurnCompleted` (Desktop UI and MCP paths via `chat_send()`) |
| `AgentRuntime` | `SkillRouted` |
| `TaskTool` | `TaskCreated`, `TaskCompleted`, `TaskUpdated`, `TaskDeleted` |
| `FinanceTool` | `TransactionRecorded`, `BudgetAlert` |
| `FocusManager` | `FocusSessionStarted`, `FocusSessionEnded` |
| `ProductivityEngine` | `ProductivityScoreComputed`, `ActivitySessionCompleted` |
| `BackgroundConsolidation` | (direct) `ContextUpdateQueue::push(MemoryPromoted)` on memory promotion |
| `MirrorFacade` | `MirrorTrialKilled` |
| `AutoTuner` | `AutotunerDecision`, `TrialActivated` |

### Event Consumer Map

| Consumer | Events Consumed | Action |
|----------|----------------|--------|
| `BackgroundConsolidationService` | All events | Salience → Extract/Accumulate → Memory writes |
| `ActivityLogSubscriber` | All events | Normalize → Activity ingestion |
| `RoutingMirrorSubscriber` | `SkillRouted` | Hourly snapshots, drift detection |
| `MetaRuleDetector` | `UserCorrectedAI`, `SkillRouted` | Correction streak → meta-rule proposals |
| `ConfigArchiver` | `AutotunerDecision` | Brain version archival |
| `TrialPreviewSubscriber` | `TrialActivated` | 4h early evaluation timer |
| Tauri event relay | All forwarded events | `tauri::emit()` to frontend |
| Dev SSE relay | All forwarded events | `broadcast::Sender` → SSE stream |

### Key Event Chains

**Chat Turn → Memory Extraction:**
```
ChatTurnCompleted
  → BackgroundConsolidation.collect_batch() (5s window)
  → evaluate_salience() per event
  → ExtractionHandler::extract() [LLM/heuristic]
  → ConsolidationHandler::consolidate() [LLM: ADD/UPDATE/DELETE/NOOP]
  → SemanticFactRepo::upsert()
  → SemanticFactEmbedder::embed() → LanceDB
  → ContextUpdateQueue::push(MemoryPromoted)
```

**Skill Routing → Mirror Reflection:**
```
SkillRouted
  → RoutingMirrorSubscriber accumulates in DashMap
  → (hourly) flush RoutingSnapshot to MirrorRepo
  → (on drift: fallback >70% or shift >15pp) write NarrativeSnippet
```

**User Correction → Meta-Rule:**
```
UserCorrectedAI
  → MetaRuleDetector.record_correction()
  → (if ≥3 cross-session or ≥2 same-session corrections)
  → MetaRule written with status="pending"
  → NarrativeSnippet for user visibility
```

**Autotuner Promotion → Brain Version:**
```
AutotunerDecision (promotion)
  → ConfigArchiver creates BrainVersion record
  → Champion params become active
  → MirrorFacade can rollback via revert_to_version()
```

## AI Pipeline Data Transformations

### Stage-by-Stage Transformation

```
Stage 1: Raw platform message
  InboundMessage { channel, sender_id, chat_id, content, media, kind }

Stage 2: Session context
  Session { messages: Vec<SessionMessage>, correction_cooldown, metadata }
  SessionMessage { role, content, timestamp, tool_calls }

Stage 3: Context assembly
  AssembledContext {
    system_prompt: String,      -- from all ContextSources
    history: Vec<Message>,      -- compressed + recent
    memory_context: String,     -- retrieved semantic facts
    budget_report: BudgetReport -- token allocation tracking
  }

Stage 4: LLM request
  Vec<Message> = [System(prompt), ...history, User(content)]
  + Optional tools: Vec<Value> (JSON schemas)
  + ChatParams { model, temperature, max_tokens }

Stage 5: LLM response
  LlmResponse { content, tool_calls, finish_reason, usage, reasoning_content }

Stage 6: Tool execution results
  Vec<ToolExecutionResult { tool_call_id, name, result: String }>

Stage 7: Final response
  RuntimeResult { content, mode_used, classification, agent_name }

Stage 8: Outbound message
  OutboundMessage { channel, chat_id, content, media, reply_to }
```

## Scheduling & Cron

### Job Types

| Schedule Type | Example | Storage |
|--------------|---------|---------|
| One-time | `CronSchedule::At { at_ms }` | Deleted after run |
| Fixed interval | `CronSchedule::Every { every_ms }` | Persisted |
| Cron expression | `CronSchedule::Cron { expr, tz }` | Persisted |

### Execution Flow

```
CronService::start()
     |
     +-- Load all jobs from CronRepo (SQLite)
     +-- Timer loop: wake at min(next_run_at) across all jobs
     |
     v
Job triggers:
     |
     +-- Named handler? (registered via register_handler())
     |     Yes --> handler(job)
     |     No  --> on_job closure
     |
     +-- "agent_turn" payload kind:
     |     AgentLoop::process_direct(job.message)
     |     if deliver=true: OutboundMessage to channel:to
     |
     +-- Update job state: next_run_at, last_status, last_error
     +-- If delete_after_run: remove job
```

### System Cron Jobs

| Job | Schedule | Action |
|-----|----------|--------|
| Proactive scan | Every N minutes (config) | Analyze overdue/stale tasks → `ProactiveSuggestion` |
| Mirror weekly narrative | Sunday 10am UTC | LLM narrative generation |
| Mirror cleanup | Sunday 4am UTC | Delete mirror data > 90 days |

## Data Persistence Flows

### Session Persistence

```
get_or_create(key) --> Cache hit? --> Return Arc<Mutex<Session>>
                           |
                           No
                           |
                    Load from SQLite? --> Found? --> Cache + Return
                           |                           |
                           No                          v
                           |                    Update LRU cache
                     Create new Session          (evict if > max_size)
                     Cache + Return
```

Sessions saved after each message pair (user + assistant) to SQLite. Background `SessionCleanupService` deletes sessions older than configured TTL.

### Memory Persistence Pipeline

```
DomainEvent
     |
     v
[Salience: Extract]              [Salience: Accumulate]
     |                                   |
     v                                   v
LLM Extraction                    AccumulatedObservationRepo
(subject, predicate, object)      (buffer across days)
     |                                   |
     v                            [Promote when threshold met]
LLM Consolidation                        |
(ADD/UPDATE/DELETE/NOOP)                  v
     |                            Join extraction pipeline
     v
SemanticFactRepo (SQLite)
     |
     v
SemanticFactEmbedder → LanceDB (384-dim vectors)
     |
     v
ContextUpdateQueue → LiveContextRefresher (mid-ReAct injection)
```

### Activity Logging (4 Push Paths)

| Path | Source | Trigger |
|------|--------|---------|
| Domain events | `ActivityLogSubscriber` | DomainEvent → normalize → ingest |
| Chat messages | `AgentLoop` | Each message → `ChatMessageNormalizer` → ingest (fire-and-forget) |
| File changes | `FileWatcherService` | `notify` crate → debounce → ingest |
| Screen activity | `platform-macos` | macOS APIs → activity detection → ingest |

All paths converge on `ActivityIngestionService.ingest()` → `ActivityLogRepo` (SQLite) + `activity_embeddings` (LanceDB).

`ContextInferenceEngine` periodically clusters activity into work context segments using embedding similarity.
