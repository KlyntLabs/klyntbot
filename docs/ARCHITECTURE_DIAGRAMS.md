# Klyntbot Architecture Diagrams

> Updated: 2026-03-08 | Scope: Full end-to-end workflow across 26 crates

---

## Table of Contents

1. [High-Level Architecture](#1-high-level-architecture)
2. [End-to-End Message Processing Flow](#2-end-to-end-message-processing-flow)
3. [ReAct Loop + Tool Execution + Multi-Agent Delegation](#3-react-loop--tool-execution--multi-agent-delegation)
4. [Cognitive Memory & Data Pipeline](#4-cognitive-memory--data-pipeline)
5. [Adaptive Learning & Training Loop](#5-adaptive-learning--training-loop)
6. [Context Assembly Waterfall](#6-context-assembly-waterfall)
7. [Boot Sequence](#7-boot-sequence)
8. [Master Comprehensive Workflow Diagram](#8-master-comprehensive-workflow-diagram)
9. [Summary of the Complete Workflow](#9-summary-of-the-complete-workflow)
10. [Implementation Gaps & Technical Debt Analysis](#10-implementation-gaps--technical-debt-analysis)

---

## 1. High-Level Architecture

```mermaid
graph TD
    subgraph "L8 — Entry Point"
        klyntbot["klyntbot<br/><i>Re-export facade + main()</i>"]
    end

    subgraph "L7 — Application Layer"
        appcore["app-core<br/><i>AppCore, init(), handlers</i>"]
        deskshared["desktop-shared<br/><i>30+ IPC event types</i>"]
        desktop["desktop<br/><i>Tauri 2, 100+ commands</i>"]
    end

    subgraph "L6 — Interface Layer"
        cli["cli<br/><i>Clap: serve/init/status/plugin</i>"]
        mcp_crate["mcp<br/><i>MCP client + server</i>"]
    end

    subgraph "L5 — Intelligence Layer"
        channels["channels<br/><i>Telegram, Discord, Slack,<br/>WhatsApp, Email, QQ</i>"]
        agent["agent<br/><i>AgentRuntime, AgentLoop,<br/>ReAct, Learning, Confidence</i>"]
        cognitive["cognitive<br/><i>3-tier memory, FSRS,<br/>consolidation, embedding</i>"]
    end

    subgraph "L4 — Feature Layer"
        tools["tools<br/><i>20+ native tools</i>"]
        ftodo["feature-todo<br/><i>26 actions, subtasks</i>"]
        ffin["feature-finance<br/><i>40+ actions, 8 modules</i>"]
        fnotes["feature-notes<br/><i>10 actions</i>"]
        fprod["feature-productivity<br/><i>Activity, focus, scoring</i>"]
        fcoach["feature-coaching<br/><i>Signals→Patterns→Interventions</i>"]
        plugrt["plugin-runtime<br/><i>Extism WASM host</i>"]
    end

    subgraph "L3 — Service Layer"
        providers["providers<br/><i>14 LLMs, circuit breaker</i>"]
        session["session<br/><i>DashMap + SQL, LRU@1000</i>"]
        scheduling["scheduling<br/><i>CronService: At/Every/Cron</i>"]
        ctxengine["context_engine<br/><i>8-priority token budget</i>"]
    end

    subgraph "L2 — Data Layer"
        storage["storage<br/><i>StoragePool, 24 repos,<br/>VectorStore (LanceDB)</i>"]
        domain["domain<br/><i>OKR + PARA types</i>"]
    end

    subgraph "L1 — Foundation"
        config["config<br/><i>camelCase JSON, Secret&lt;String&gt;</i>"]
        bus["bus<br/><i>MessageBus (mpsc×2, buf=100),<br/>DomainEventBus</i>"]
        toolscore["tools-core<br/><i>Tool/FeaturePackage traits</i>"]
        macros["tools-core-macros<br/><i>#[derive(Tool/ToolParams)]</i>"]
    end

    subgraph "L0 — Common"
        common["common<br/><i>KlyntbotError, MessageRole,<br/>ChannelName, ChatId, SessionKey</i>"]
    end

    subgraph "External Storage"
        sqlite[("SQLite<br/>WAL mode<br/>data.db")]
        lancedb[("LanceDB<br/>384-dim vectors<br/>conv_embeddings<br/>todo_embeddings")]
    end

    klyntbot --> appcore & cli
    desktop --> appcore & deskshared
    appcore --> agent & channels & cognitive & scheduling & mcp_crate
    cli --> agent & channels & mcp_crate
    agent --> ctxengine & providers & session & tools & cognitive
    cognitive --> storage & providers
    channels --> bus
    tools --> toolscore & storage
    ftodo --> toolscore & storage & domain
    ffin --> toolscore & storage
    fnotes --> toolscore & storage
    fprod --> storage & bus
    fcoach --> bus & providers
    ctxengine --> cognitive
    providers --> config
    session --> storage
    storage --> sqlite & lancedb
    bus --> common
    toolscore --> common
    config --> common

    style klyntbot fill:#2d3748,stroke:#4fd1c5,color:#fff
    style agent fill:#553c9a,stroke:#b794f4,color:#fff
    style cognitive fill:#2c5282,stroke:#63b3ed,color:#fff
    style sqlite fill:#c05621,stroke:#fbd38d,color:#fff
    style lancedb fill:#c05621,stroke:#fbd38d,color:#fff
```

---

## 2. End-to-End Message Processing Flow

```mermaid
sequenceDiagram
    autonumber
    participant User
    participant Channel as Channel<br/>(Telegram/Discord/Slack...)
    participant Bus as MessageBus<br/>(mpsc, buf=100)
    participant Loop as AgentLoop<br/>::process_message
    participant Session as SessionManager<br/>(DashMap + SQL)
    participant Runtime as AgentRuntime<br/>::process_message
    participant AgentMgr as AgentManager<br/>::match_agent
    participant Analyzer as IntentAnalyzer<br/>::analyze
    participant ConfEval as ConfidenceEvaluator<br/>(AtomicU32)
    participant CtxEngine as ContextEngine<br/>::assemble
    participant CogCtx as CognitiveContextSource<br/>(UserModel + facts)
    participant Router as ExecutionRouter<br/>::execute
    participant LLM as LLM Provider<br/>(14 providers)
    participant Validator as ResponseValidator<br/>::validate
    participant CostTrack as CostTracker<br/>+ StrategyRepo
    participant DEB as DomainEventBus<br/>(cognitive pipeline)
    participant BusOut as MessageBus<br/>(outbound)

    User->>Channel: Send message
    Channel->>Bus: InboundMessage{channel, chat_id, content}
    Bus->>Loop: rx.recv()

    Loop->>Session: get_or_create(session_key)
    Session-->>Loop: Session{history, context}

    Loop->>Runtime: process_message(text, history, tools, ctx)

    Note over Runtime: Step 1 — Agent Matching
    Runtime->>AgentMgr: match_agent(message)
    AgentMgr-->>Runtime: AgentProfile{name, tools, mcp_tools, skills}

    Note over Runtime: Step 2 — Set active_profile (RwLock)
    Note over Runtime: Step 3 — Filter MCP tools by profile.mcp_tools

    Note over Runtime: Step 4 — Intent Classification
    Runtime->>Analyzer: analyze(message, tool_names)
    Note over Analyzer: Stage 1: Heuristics (zero-cost)<br/>Stage 2: LLM classifier (fallback)
    Analyzer-->>Runtime: IntentAnalysis{mode, confidence, signals}

    Note over Runtime: Step 5 — Confidence Check
    Runtime->>ConfEval: threshold()
    ConfEval-->>Runtime: f32 threshold
    alt confidence < threshold
        Runtime-->>Loop: "Could you clarify?"
    end

    Note over Runtime: Step 6 — Context Assembly
    Runtime->>CtxEngine: assemble(ContextRequest)
    CtxEngine->>CogCtx: build UserModel (6 domains)
    CogCtx-->>CtxEngine: Markdown persona + facts
    Note over CtxEngine: 8-priority waterfall allocation<br/>SHA-256 cached (60s TTL)
    CtxEngine-->>Runtime: AssembledContext{messages, token_count}

    Note over Runtime: Step 7 — Tool Filtering + Delegation injection
    Note over Runtime: Step 7c — Planning prompt if complexity >= 5

    Note over Runtime: Step 8 — Execute
    Runtime->>Router: execute(mode, messages, tools, params)
    Router->>LLM: chat()/chat_stream()
    LLM-->>Router: Response / ToolCalls
    Note over Router: Direct → single call<br/>Reactive → ReAct loop (1..max_iter)
    Router-->>Runtime: RouterResult{content, usage, iterations}

    Note over Runtime: Step 9 — Validate
    Runtime->>Validator: validate(content)
    Validator-->>Runtime: ValidationResult{is_valid, warnings}

    Note over Runtime: Step 10 — Record
    Runtime->>CostTrack: record_usage() + record_strategy()
    Runtime-->>Loop: RuntimeResult{content, mode_used, agent_name}

    Loop->>Session: save(session + new messages)
    Loop->>DEB: ChatTurnCompleted{user_message, session_key}
    Note over DEB: Passive learning — every chat turn<br/>feeds cognitive extraction pipeline
    Loop->>BusOut: OutboundMessage{channel, chat_id, content}
    BusOut->>Channel: send(formatted_content)
    Channel->>User: Formatted response
```

---

## 3. ReAct Loop + Tool Execution + Multi-Agent Delegation

```mermaid
flowchart TD
    Start([ReactiveEngine::execute]) --> Init["Initialize:<br/>messages, scratchpad = Scratchpad::new()<br/>max_iter = params.max_iterations or engine.max_iterations<br/>fabrication_retries = 0<br/>seen_tool_calls: HashSet"]

    Init --> PlanCheck{planning_prompt<br/>is Some?}
    PlanCheck -->|Yes| InjectPlan["messages.push(planning_prompt)<br/>— complexity >= 5 triggers this"]
    PlanCheck -->|No| LoopStart
    InjectPlan --> LoopStart

    LoopStart["for iteration in 1..=max_iterations"] --> CancelCheck{cancel_token<br/>is_cancelled?}
    CancelCheck -->|Yes| ReturnEmpty([EngineResult::Complete<br/>empty content])
    CancelCheck -->|No| EmitIter["emit AgentEvent::IterationStart<br/>{iteration, max}"]

    EmitIter --> RunCycle["ExecutionCore::run_cycle()<br/>→ LLM call with tools<br/>→ parallel tool execution<br/>→ duplicate detection via seen_tool_calls"]

    RunCycle --> Outcome{CycleOutcome?}

    Outcome -->|FinalResponse| PlanIter1{Is planning<br/>iteration 1?}
    PlanIter1 -->|Yes| ParsePlan["try_store_plan(content)<br/>→ ExecutionPlan::parse()<br/>emit PlanGenerated<br/>continue loop"]
    ParsePlan --> LoopStart
    PlanIter1 -->|No| Complete([EngineResult::Complete<br/>{content, usage, iterations, traces}])

    Outcome -->|FabricatedResponse| FabRetry{fabrication_retries<br/>> max_fabrication_retries?<br/><i>default: 2</i>}
    FabRetry -->|Yes| CompleteFab([EngineResult::Complete<br/>return fabricated content as-is])
    FabRetry -->|No| InjectForce["Inject force-tool prompt:<br/>'You MUST call the appropriate tool...'<br/>fabrication_retries += 1"]
    InjectForce --> LoopStart

    Outcome -->|ToolsExecuted| TrackTools["Track last_tool_name<br/>Parse plan from iter 1 if planning<br/>Mark plan steps completed<br/>emit PlanStepCompleted"]
    TrackTools --> DupCheck{All results are<br/>'Skipped: duplicate'?}
    DupCheck -->|Yes| InjectDup["Inject anti-dup prompt:<br/>'Do NOT repeat these calls'"]
    DupCheck -->|No| FailCheck{Any tool<br/>failures?}
    FailCheck -->|Yes| InjectReflect["Inject reflection prompt:<br/>'What went wrong?'"]
    FailCheck -->|No| ContinueLoop
    InjectDup --> ContinueLoop
    InjectReflect --> ContinueLoop
    ContinueLoop --> LoopStart

    Outcome -->|EmptyResponse| AddTrace["Add empty_response trace"] --> ContinueLoop

    LoopStart -->|max_iterations<br/>exhausted| Synthesize["Synthesize final response:<br/>LLM call with NO tools<br/>Include plan progress if available<br/>'Based on work so far...'"]
    Synthesize --> SynthResult([EngineResult::Complete<br/>{synthesized content}])

    subgraph "Tool Execution (ExecutionCore::run_cycle)"
        TC1["LLM returns tool_calls[]"] --> TC2["Bounded parallel execution<br/>Semaphore(10) + join_all<br/>Per-tool timeout (30s default)"]
        TC2 --> TC3["Duplicate detection:<br/>hash(tool_name + args) in seen_tool_calls"]
        TC3 --> TC4["OutcomeRecorder::record()<br/>privacy-safe: no args/content stored"]
        TC4 --> TC5["Append tool results to messages"]
    end

    subgraph "Multi-Agent Delegation"
        D1["IntentAnalyzer sets needs_orchestration=true"] --> D2["Runtime routes to 'general' agent<br/>(ORCHESTRATOR_AGENT)"]
        D2 --> D3["inject_delegation_tool()<br/>adds DelegateTool to filtered_tools"]
        D3 --> D4["DelegateTool::execute()<br/>ctx.delegation_depth check"]
        D4 --> D5{depth <<br/>MAX_DELEGATION_DEPTH?<br/><i>const: 2</i>}
        D5 -->|Yes| D6["Spawn sub-AgentRuntime::process_message<br/>delegation_depth += 1<br/>Match specialized agent profile"]
        D5 -->|No| D7["Return error:<br/>'Max delegation depth reached'"]
        D6 --> D8["5 built-in agents:<br/>general, task, finance,<br/>automation, communication"]
    end

    style Complete fill:#38a169,stroke:#fff,color:#fff
    style CompleteFab fill:#d69e2e,stroke:#fff,color:#fff
    style SynthResult fill:#3182ce,stroke:#fff,color:#fff
    style ReturnEmpty fill:#718096,stroke:#fff,color:#fff
```

---

## 4. Cognitive Memory & Data Pipeline

```mermaid
flowchart TD
    subgraph "Event Sources"
        US["UserStatedFact<br/><i>'I prefer dark mode'</i>"]
        UC["UserCorrectedAI<br/><i>'No, I prefer mornings'</i>"]
        CT["ChatTurnCompleted<br/><i>Every chat turn (passive learning)</i>"]
        BA["BudgetAlert<br/><i>threshold-crossing</i>"]
        CF["CoachingFeedback"]
        PS["ProductivityScoreComputed"]
        TC["TaskCreated / TaskCompleted"]
        TR["TransactionRecorded"]
    end

    US & UC & CT & BA & CF & PS & TC & TR --> DEB["DomainEventBus<br/>(mpsc, buf=256)"]

    DEB --> Salience["evaluate_salience()<br/><i>cognitive/salience.rs</i>"]

    Salience -->|Extract| Immediate["Immediate Processing<br/><i>UserStatedFact, UserCorrectedAI,<br/>ChatTurnCompleted, BudgetAlert,<br/>CoachingFeedback, over-budget transactions</i>"]
    Salience -->|Accumulate| Buffer["Accumulator Buffer<br/><i>TaskCreated, ProductivityScore,<br/>FocusSessionEnded, etc.</i>"]
    Salience -->|Discard| Drop["(no Discard events currently)"]

    Buffer --> PromotionCheck{">=5 occurrences<br/>across >=3 distinct days?"}
    PromotionCheck -->|Yes| Immediate
    PromotionCheck -->|No| StayBuffered["Remain buffered"]

    Immediate --> ExtractionHandler["ExtractionHandler<br/><i>LLM-backed: extract SPO triples<br/>from event text</i>"]

    ExtractionHandler --> ExtractedFacts["ExtractedFact[]<br/>{subject, predicate, object,<br/>source, confidence, domain}"]

    ExtractedFacts --> Consolidation["ConsolidationHandler<br/><i>Mem0-style merge logic</i>"]

    Consolidation --> FindExisting["Find existing facts<br/>matching (subject, predicate)"]

    FindExisting --> Decision{Consolidation<br/>Decision?}
    Decision -->|ADD| Insert["SemanticFactRepo::insert()<br/>New fact with FSRS stability=1.0"]
    Decision -->|UPDATE| Update["SemanticFactRepo::update()<br/>Supersede old fact<br/>(superseded_at, superseded_by)"]
    Decision -->|DELETE| Archive["Mark fact superseded<br/>Will be compacted after 90d"]
    Decision -->|NOOP| Skip["No change needed"]

    Insert & Update --> Embed["SemanticFactEmbedder<br/>::embed_fact(fact_id)"]
    Embed --> LanceDB[("LanceDB<br/>conv_embeddings table<br/>384-dim MiniLM vectors")]

    Insert & Update --> FSRS["FSRS Scoring<br/>stability = 1.0 (initial)<br/>S_new = S + ln(1 + S) on access"]

    subgraph "Retrieval Path"
        Query["retrieve_relevant_facts()"] --> VecSearch["Vector search:<br/>embed(query) → ANN search<br/>cosine similarity"]
        VecSearch --> MinCheck{">=3 results?"}
        MinCheck -->|Yes| VectorPath["Vector Path:<br/>real semantic_similarity"]
        MinCheck -->|No| FallbackPath["Fallback Path:<br/>semantic_similarity = 0.5"]
        VectorPath & FallbackPath --> Score["5-Factor Relevance:<br/>0.30 x semantic_similarity<br/>0.20 x retrievability (FSRS)<br/>0.15 x importance<br/>0.10 x access_frequency<br/>0.25 x situational_boost"]
        Score --> TopK["Return top-K scored facts"]
    end

    subgraph "User Model Assembly"
        TopK --> UserModel["CognitiveContextSource<br/>6 domains:<br/>preferences, background, goals,<br/>relationships, routines, personality"]
        UserModel --> Markdown["Format as Markdown<br/>→ System Prompt Priority 60<br/>60s cache TTL"]
    end

    subgraph "Maintenance (Background)"
        Compact["Daily Compaction:<br/>Archive superseded >90d<br/>Delete episodic <2 accesses >90d<br/>Cap: 10,000 active facts"]
        WeeklyRef["Weekly Reflection:<br/>LLM summarizes 7d episodic<br/>→ new facts + procedural rules"]
    end

    style Salience fill:#805ad5,stroke:#fff,color:#fff
    style ExtractionHandler fill:#2b6cb0,stroke:#fff,color:#fff
    style Consolidation fill:#2b6cb0,stroke:#fff,color:#fff
    style LanceDB fill:#c05621,stroke:#fbd38d,color:#fff
    style Score fill:#38a169,stroke:#fff,color:#fff
```

---

## 5. Adaptive Learning & Training Loop

```mermaid
flowchart LR
    subgraph "Per-Request (Hot Path)"
        ToolExec["Tool Execution<br/><i>ExecutionCore::run_cycle()</i>"] --> Outcome["ToolExecutionResult<br/>{tool_name, success,<br/>duration_ms, confidence}"]
        Outcome --> Recorder["OutcomeRecorder::record()<br/><i>Privacy-safe: no args/content</i><br/>→ learning_outcomes table"]
    end

    subgraph "Hourly Analysis (Background)"
        Timer["LearningService<br/><i>hourly tick</i>"] --> Load["OutcomeStore::list_since()<br/><i>load recent outcomes</i>"]
        Load --> Analyzer["LearningAnalyzer<br/>::analyze()"]
        Analyzer --> Bucket["Bucket by confidence range<br/>Compute success rates per band<br/>Calculate threshold_confidence"]
        Bucket --> AnalysisResult["AnalysisResult<br/>{total_outcomes,<br/>suggested_threshold,<br/>threshold_confidence,<br/>per-band stats}"]
    end

    subgraph "Threshold Adaptation"
        AnalysisResult --> ColdStart{"total_outcomes<br/>>= min_outcomes?<br/><i>default: 50</i>"}
        ColdStart -->|No| Skip["Skip adaptation<br/><i>insufficient data</i>"]
        ColdStart -->|Yes| Clamp["Clamp suggested to<br/>[min_threshold, max_threshold]"]
        Clamp --> Delta["delta = suggested - current<br/>clamped to +/-0.05<br/><i>MAX_THRESHOLD_STEP</i>"]
        Delta --> Significant{"|delta| >= 0.001?"}
        Significant -->|No| NoChange["No change"]
        Significant -->|Yes| Update["AdaptiveThresholds<br/>state.current_threshold += delta<br/>Push ThresholdChange to history<br/>Persist to learning_state table"]
    end

    subgraph "Confidence Evaluation (Per-Request)"
        Update --> Evaluator["ConfidenceEvaluator<br/><i>AtomicU32 <- f32 bits</i><br/>Lock-free hot path"]
        Evaluator --> Decision{"confidence <<br/>threshold?"}
        Decision -->|Yes| Clarify["Return clarification<br/><i>'Could you clarify?'</i>"]
        Decision -->|No| Proceed["Proceed with execution"]
    end

    Recorder -.->|"learning_outcomes<br/>(SQL, 30d retention)"| Load

    style Recorder fill:#2b6cb0,stroke:#fff,color:#fff
    style Analyzer fill:#805ad5,stroke:#fff,color:#fff
    style Evaluator fill:#38a169,stroke:#fff,color:#fff
    style Update fill:#d69e2e,stroke:#fff,color:#fff
```

---

## 6. Context Assembly Waterfall

```mermaid
flowchart TD
    Start["ContextEngine::assemble(ContextRequest)"] --> Budget["BudgetAllocator::new()<br/>total = context_window<br/>reserve = 15% for response"]

    Budget --> P0["Priority 0: SystemIdentity<br/><i>Agent name, core personality</i><br/>System prompt injected first"]
    P0 --> P0Alloc["allocate(SystemIdentity, tokens)"]

    P0Alloc --> P1["Priority 1: ActiveTask<br/><i>Current task context from session</i>"]
    P1 --> P1Alloc["allocate(ActiveTask, tokens)"]

    P1Alloc --> P2["Priority 2: ToolDefinitions<br/><i>JSON tool schemas for LLM</i><br/>Filtered by agent profile"]
    P2 --> P2Alloc["allocate(ToolDefinitions, tokens)"]

    P2Alloc --> P3["Priority 3: RecentHistory<br/><i>Latest conversation messages</i><br/>Most recent first, truncated to budget"]
    P3 --> P3Alloc["allocate(RecentHistory, tokens)"]

    P3Alloc --> P4["Priority 4: RetrievedMemory<br/><i>CognitiveContextSource</i>"]
    P4 --> UserModel["Build UserModel from 6 domains:<br/>preferences, background, goals,<br/>relationships, routines, personality"]
    UserModel --> VecRetrieve["retrieve_relevant_facts()<br/>Vector path (if embedder available)<br/>OR fallback (importance x stability)"]
    VecRetrieve --> StaticFacts["Static facts: high-confidence<br/>user-stated preferences"]
    VecRetrieve --> DynFacts["Dynamic facts: query-relevant<br/>scored by 5-factor formula"]
    StaticFacts & DynFacts --> FormatMD["Format as Markdown section"]
    FormatMD --> P4Alloc["allocate(RetrievedMemory, tokens)<br/><i>60s cache (SHA-256 key)</i>"]

    P4Alloc --> P5["Priority 5: CompressedHistory<br/><i>HistoryCompressor</i><br/>Summarize older messages if budget allows"]
    P5 --> P5Alloc["allocate(CompressedHistory, tokens)"]

    P5Alloc --> P6["Priority 6: BootstrapPersona<br/><i>Base instructions, behavioral rules</i><br/>Procedural rules from cognitive memory"]
    P6 --> P6Alloc["allocate(BootstrapPersona, tokens)"]

    P6Alloc --> P7["Priority 7: Skills<br/><i>Agent skills content</i><br/>Always-loaded + message-activated skills"]
    P7 --> P7Alloc["allocate(Skills, tokens)"]

    P7Alloc --> Assemble["Build final message array:<br/>System prompt (all sources merged)<br/>+ history messages<br/>+ current user message"]

    Assemble --> Result["AssembledContext<br/>{messages: Vec&lt;Message&gt;,<br/>token_count: usize}"]

    subgraph "Token Counting"
        TC["TiktokenCounter<br/><i>accurate, tiktoken-rs</i>"]
        TCF["CharTokenCounter<br/><i>fallback: chars / 4</i>"]
        TC -.->|"fallback"| TCF
    end

    style Start fill:#2b6cb0,stroke:#fff,color:#fff
    style P4 fill:#805ad5,stroke:#fff,color:#fff
    style Result fill:#38a169,stroke:#fff,color:#fff
```

---

## 7. Boot Sequence

```mermaid
sequenceDiagram
    autonumber
    participant Main as main() / Tauri
    participant AppCore as AppCore::init()
    participant Config as config::load_with_env_overrides()
    participant Storage as StoragePool::connect()
    participant VS as VectorStore::connect()
    participant Provider as providers::create_provider()
    participant Bus as MessageBus::new(100)
    participant Cron as CronService::new + start()
    participant Persona as PersonaManager::load()
    participant DEB as DomainEventBus::new(256)
    participant Builder as AgentLoop::builder().build()
    participant ChanMgr as ChannelManager::new()
    participant Coaching as CoachingService::start()
    participant BG as Background Tasks

    Main->>AppCore: init(config_override)

    AppCore->>Config: load_with_env_overrides()
    Config-->>AppCore: Config (camelCase JSON + env KLYNTBOT_*)

    AppCore->>Storage: connect(&data_dir)
    Note over Storage: WAL mode + FK pragma<br/>Run core migrations (35 tables)<br/>Run cognitive migrations (5 tables)<br/>Run feature migrations (notes, productivity)
    Storage-->>AppCore: StoragePool + Repos

    AppCore->>VS: connect(&data_dir)
    Note over VS: LanceDB at {data_dir}/lancedb/
    VS-->>AppCore: VectorStore (optional)

    AppCore->>BG: tokio::spawn ensure_indexes(256)
    Note over BG: IVF-PQ indexes on tables >= 256 rows

    AppCore->>Provider: create_provider(&config)
    Note over Provider: Primary + fallback<br/>Circuit breaker (5 failures / 60s)<br/>Falls back to NoopProvider
    Provider-->>AppCore: DynProvider + resolved_model

    AppCore->>Bus: new(100)
    Note over Bus: Inbound + Outbound channels<br/>Buffer size = 100
    Bus-->>AppCore: Arc<MessageBus>

    AppCore->>Cron: new(repos.cron) + start()
    Note over Cron: Load persisted jobs<br/>Register callbacks<br/>Ensure recurring jobs
    Cron-->>AppCore: Arc<CronService>

    AppCore->>Persona: load(&personas_dir)
    Note over Persona: Load persona YAML from data_dir/personas/
    Persona-->>AppCore: Arc<RwLock<PersonaManager>>

    AppCore->>DEB: new(256)
    Note over DEB: Cross-feature communication<br/>Cognitive + Coaching subscribers
    DEB-->>AppCore: Arc<DomainEventBus>

    AppCore->>Builder: builder(bus, provider, config)
    Note over Builder: Wire: pool, cron, vector_store,<br/>domain_bus, cognitive_provider,<br/>pipeline_tx, notification_handle
    Builder-->>AppCore: AgentLoop (with AgentRuntime inside)

    AppCore->>ChanMgr: new(config, bus)
    Note over ChanMgr: Initialize enabled channels:<br/>Telegram, Discord, Slack,<br/>WhatsApp, Email, QQ
    ChanMgr-->>AppCore: Arc<Mutex<ChannelManager>>

    AppCore->>Coaching: start(domain_bus.subscribe(), ...)
    Note over Coaching: SignalAccumulator → PatternDetector<br/>→ CoachingReasoner → InterventionRouter<br/>→ FeedbackTracker
    Coaching-->>AppCore: CoachingService

    AppCore->>BG: spawn agent_loop.run_with_rx(inbound_rx)
    AppCore->>BG: spawn channel_manager.start_all()
    AppCore->>BG: spawn daily analytics cleanup
    Note over BG: All guarded by CancellationToken

    AppCore-->>Main: (AppCore, EventChannels)
    Note over Main: Caller wires EventChannels<br/>to transport (Tauri events / SSE)
```

---

## 8. Master Comprehensive Workflow Diagram

```mermaid
flowchart TD
    subgraph USER["User"]
        UserMsg["Send Message"]
    end

    subgraph CHANNELS["L5: Channel Layer"]
        Telegram["Telegram<br/><i>HTTP long-poll</i>"]
        Discord["Discord<br/><i>Raw WebSocket</i>"]
        Slack["Slack<br/><i>Socket Mode WS</i>"]
        WhatsApp["WhatsApp<br/><i>WS → Baileys bridge</i>"]
        Email["Email<br/><i>IMAP + SMTP</i>"]
        QQ["QQ<br/><i>WS bridge</i>"]
        Formatter["ChannelFormatter<br/><i>Markdown → platform format</i>"]
    end

    subgraph BUS_IN["L1: MessageBus (Inbound)"]
        InboundQ["InboundMessage<br/>{channel, chat_id, content, kind}"]
    end

    subgraph AGENT_LOOP["L5: AgentLoop"]
        Receive["rx.recv()"]
        ReactionCheck{MessageKind?}
        HandleReaction["handle_reaction()<br/>→ update satisfaction score"]
        SessionGet["SessionManager::get_or_create<br/><i>DashMap + SQL, LRU@1000</i>"]
        LoadHistory["Load session history<br/><i>history_limit messages</i>"]
        ToolDefs["Build tool definitions<br/><i>from ToolRegistry</i>"]
    end

    subgraph RUNTIME["L5: AgentRuntime (10-Step Pipeline)"]
        Step1["1. AgentManager::match_agent<br/><i>keyword trigger scoring</i>"]
        Step2["2. Set active_profile<br/><i>RwLock&lt;Option&lt;Arc&lt;AgentProfile&gt;&gt;&gt;</i>"]
        Step3["3. Filter MCP tools<br/><i>profile.mcp_tools allowlist</i>"]
        Step4["4. IntentAnalyzer::analyze<br/><i>Heuristics → LLM classifier</i>"]
        OrchOverride{needs_orchestration?}
        SwitchOrch["Route to 'general' orchestrator<br/>Set min iterations"]
        Step4b["Override max_iterations<br/><i>from agent profile</i>"]
        Step5["5. ConfidenceEvaluator<br/><i>AtomicU32, lock-free read</i>"]
        LowConf{confidence < threshold?}
        Clarify["Return clarification request"]
        Step6["6. ContextEngine::assemble<br/><i>8-priority waterfall</i>"]
        Step7["7. Filter tools by profile<br/>7b. Inject DelegateTool if allowed<br/>7c. Planning prompt if complexity >= 5"]
        Step8["8. ExecutionRouter::execute"]
        Step9["9. ResponseValidator::validate<br/><i>empty/overlong detection</i>"]
        Step10["10. CostTracker + StrategyRepo<br/>+ InteractionRecorder"]
    end

    subgraph ROUTER["Execution Router"]
        ModeCheck{ExecutionMode?}
        DirectEng["DirectEngine<br/><i>Single LLM call, no tools</i>"]
        ReactEng["ReactiveEngine<br/><i>ReAct loop 1..max_iter</i>"]
        Escalate{EngineResult::Escalate?}
        RetryReactive["Retry with ReactiveEngine<br/><i>combine usage from both</i>"]
    end

    subgraph REACT_LOOP["ReAct Loop (ReactiveEngine)"]
        Iter["Iteration i"]
        LLMCall["ExecutionCore::run_cycle<br/>→ LLM Provider chat()"]
        CycleOut{CycleOutcome?}
        Final["FinalResponse → return"]
        Fabricated["FabricatedResponse<br/>→ inject force-tool prompt<br/><i>max 2 retries</i>"]
        ToolsExec["ToolsExecuted<br/>→ parallel execution<br/>→ duplicate detection<br/>→ failure reflection"]
        EmptyResp["EmptyResponse → continue"]
        Synth["Synthesize at max_iter<br/><i>LLM call, no tools</i>"]
    end

    subgraph TOOLS_EXEC["Tool Execution"]
        ParallelExec["Bounded parallel join_all<br/><i>Semaphore(10), per-tool 30s timeout</i>"]
        OutcomeRec["OutcomeRecorder<br/><i>privacy-safe recording</i>"]
        Delegate["DelegateTool<br/><i>depth < MAX_DELEGATION_DEPTH (2)</i>"]
        SubRuntime["Sub-AgentRuntime<br/><i>specialized agent profile</i>"]
    end

    subgraph CONTEXT["Context Assembly"]
        CTX_P0["P0: SystemIdentity"]
        CTX_P1["P1: ActiveTask"]
        CTX_P2["P2: ToolDefinitions"]
        CTX_P3["P3: RecentHistory"]
        CTX_P4["P4: RetrievedMemory<br/><i>CognitiveContextSource</i>"]
        CTX_P5["P5: CompressedHistory"]
        CTX_P6["P6: BootstrapPersona"]
        CTX_P7["P7: Skills"]
    end

    subgraph COGNITIVE["L5: Cognitive Memory Pipeline"]
        DomainEvt["DomainEvent via DomainEventBus"]
        SalienceFilter["evaluate_salience()<br/><i>Extract / Accumulate / Discard</i>"]
        Extraction["ExtractionHandler<br/><i>LLM-backed SPO extraction</i>"]
        Consolidation["ConsolidationHandler<br/><i>Mem0-style ADD/UPDATE/DELETE/NOOP</i>"]
        FactRepo[("SemanticFactRepo<br/><i>SQLite, bi-temporal</i>")]
        Embedder["SemanticFactEmbedder<br/><i>384-dim MiniLM</i>"]
        VecStore[("LanceDB<br/><i>ANN vector search</i>")]
        FSRSCalc["FSRS Decay<br/><i>R = exp(ln(0.9) x days/S)</i><br/><i>S_new = S + ln(1+S)</i>"]
        Retrieval["retrieve_relevant_facts()<br/><i>5-factor scoring</i>"]
        UserModel["UserModel<br/><i>6 domains → Markdown</i>"]
    end

    subgraph LEARNING["Adaptive Learning"]
        OutcomeDB[("learning_outcomes<br/><i>30d retention</i>")]
        LrnAnalyzer["LearningAnalyzer<br/><i>hourly: bucket by confidence</i>"]
        AdaptThresh["AdaptiveThresholds<br/><i>+/-0.05 max step/cycle</i>"]
        ConfEval["ConfidenceEvaluator<br/><i>AtomicU32 hot path</i>"]
    end

    subgraph LLM_LAYER["L3: LLM Providers"]
        ProviderMgr["ProviderManager<br/><i>14 providers</i>"]
        CircuitBreaker["Circuit Breaker<br/><i>5 failures / 60s reset</i>"]
        Retry["Retry<br/><i>3x exponential backoff</i>"]
        Failover["Primary → Fallback"]
    end

    subgraph BUS_OUT["L1: MessageBus (Outbound)"]
        OutboundQ["OutboundMessage<br/>{channel, chat_id, content}"]
    end

    %% Main Flow
    UserMsg --> Telegram & Discord & Slack & WhatsApp & Email & QQ
    Telegram & Discord & Slack & WhatsApp & Email & QQ --> InboundQ
    InboundQ --> Receive

    Receive --> ReactionCheck
    ReactionCheck -->|Reaction| HandleReaction
    ReactionCheck -->|Text| SessionGet
    SessionGet --> LoadHistory --> ToolDefs

    ToolDefs --> Step1 --> Step2 --> Step3 --> Step4
    Step4 --> OrchOverride
    OrchOverride -->|Yes| SwitchOrch --> Step4b
    OrchOverride -->|No| Step4b
    Step4b --> Step5
    Step5 --> LowConf
    LowConf -->|Yes| Clarify
    LowConf -->|No| Step6

    Step6 -.-> CTX_P0 & CTX_P1 & CTX_P2 & CTX_P3 & CTX_P4 & CTX_P5 & CTX_P6 & CTX_P7
    CTX_P4 -.-> Retrieval
    Retrieval -.-> FSRSCalc & VecStore
    Retrieval --> UserModel

    Step6 --> Step7 --> Step8

    Step8 --> ModeCheck
    ModeCheck -->|Direct| DirectEng
    ModeCheck -->|Reactive| ReactEng
    DirectEng --> Escalate
    Escalate -->|Yes| RetryReactive --> ReactEng
    Escalate -->|No| Step9
    ReactEng --> Iter

    Iter --> LLMCall
    LLMCall --> ProviderMgr --> CircuitBreaker --> Retry --> Failover
    LLMCall --> CycleOut
    CycleOut -->|FinalResponse| Final
    CycleOut -->|FabricatedResponse| Fabricated --> Iter
    CycleOut -->|ToolsExecuted| ToolsExec
    CycleOut -->|EmptyResponse| EmptyResp --> Iter
    ToolsExec --> ParallelExec --> OutcomeRec
    ParallelExec -.-> Delegate --> SubRuntime
    ToolsExec --> Iter
    Iter -->|max_iter reached| Synth
    Final & Synth --> Step9

    Step9 --> Step10

    %% Learning feedback
    OutcomeRec --> OutcomeDB
    OutcomeDB --> LrnAnalyzer --> AdaptThresh --> ConfEval
    ConfEval -.-> Step5

    %% Cognitive pipeline
    ToolsExec -.-> DomainEvt
    Step10 -.->|"ChatTurnCompleted<br/>(passive learning)"| DomainEvt
    DomainEvt --> SalienceFilter --> Extraction --> Consolidation
    Consolidation --> FactRepo --> Embedder --> VecStore

    %% Output
    Step10 --> OutboundQ
    Clarify --> OutboundQ
    OutboundQ --> Formatter --> Telegram & Discord & Slack & WhatsApp & Email & QQ
    Telegram & Discord & Slack & WhatsApp & Email & QQ --> UserMsg

    style RUNTIME fill:#1a1a2e,stroke:#4fd1c5,color:#fff
    style REACT_LOOP fill:#16213e,stroke:#e94560,color:#fff
    style COGNITIVE fill:#1a1a2e,stroke:#63b3ed,color:#fff
    style LEARNING fill:#1a1a2e,stroke:#fbd38d,color:#fff
    style LLM_LAYER fill:#2d3748,stroke:#b794f4,color:#fff
```

---

## 9. Summary of the Complete Workflow

1. **User messages** arrive via 6 platform channels (Telegram, Discord, Slack, WhatsApp, Email, QQ) through the `MessageBus` (mpsc, buffer=100) into the `AgentLoop`.
2. **Session management** (`DashMap` + SQLite, LRU@1000) provides conversation context and history.
3. The **10-step AgentRuntime pipeline** performs: agent matching → profile-based filtering → two-stage intent classification (heuristics→LLM) → confidence gating → 8-priority context assembly → tool filtering + delegation injection → execution routing → validation → cost/strategy recording.
4. **Direct mode** handles simple queries (single LLM call); **Reactive mode** runs a ReAct loop (1..max_iterations) with fabrication detection, duplicate prevention, failure reflection, and chain-of-thought planning for complexity >= 5. Tool calls execute in bounded parallel (semaphore capped at 10, per-tool 30s timeout).
5. **Multi-agent delegation** allows the `general` orchestrator to dispatch to 4 specialized agents (task, finance, automation, communication) with max depth 2.
6. **Cognitive memory** processes domain events through salience filtering → LLM extraction → Mem0-style consolidation → SemanticFactRepo (SQLite) + vector embedding (LanceDB, 384-dim MiniLM). Retrieval uses a 5-factor FSRS-scored relevance formula. **Passive learning** via `ChatTurnCompleted` events feeds every chat turn into the extraction pipeline (importance 0.8), enabling fact discovery from ordinary conversation.
7. **Adaptive learning** records tool outcomes (privacy-safe), analyzes hourly, adjusts confidence thresholds (+/-0.05/cycle, lock-free `AtomicU32`), and feeds back into the confidence gate.
8. Responses flow back through the `MessageBus` (outbound) → `ChannelFormatter` → platform-specific format → user.

---

## 10. Implementation Gaps & Technical Debt Analysis

### Critical

*No critical items remain* — R1 (vector search), R2 (unified memory), and R3 (CLI serve parity) are all marked SOLVED in SYSTEM_ANALYSIS.md.

### High

| # | Title | Location | Why It Matters | Suggested Fix |
|---|-------|----------|---------------|---------------|
| H1 | **Coaching feedback persistence fragility** | SYSTEM_ANALYSIS.md §6.2 #6, #9 — `FeedbackTracker` | `coaching_strategies` table was previously orphaned. R5/R11 wired it, but the integration may be fragile. | Verify `FeedbackTracker::persist()` is called on shutdown and `load_from_db()` on boot in `app-core/init.rs`. Add integration test. |
| H2 | **Vector store non-atomic upsert** | SYSTEM_ANALYSIS.md §6.2 #7 — `storage/src/vector_store.rs` | LanceDB upsert is delete-then-insert. R13 reordered to insert-first-then-delete for crash safety, but LanceDB still lacks true transactions. A crash between insert and delete leaves duplicates. | The insert-first approach (R13 SOLVED) is better but still not atomic. Monitor for duplicate vectors and add a dedup pass to compaction. |
| H3 | **MessageBus has no backpressure** | SYSTEM_ANALYSIS.md §6.3, §6.4 | Buffer=100 with `mpsc::channel`. If the agent loop is slow, senders (channels) will block, and overflow drops messages silently. No persistence for bus messages. | For personal use this is acceptable. For robustness, switch to bounded channel with `try_send()` + warn on capacity, or add a dead-letter queue. |
| H4 | **No external observability** | SYSTEM_ANALYSIS.md §6.2 #4 | Zero Prometheus/OTel metrics, no health endpoints, no distributed tracing. Desktop UI has `TransparencyData` per message, but there's no way to monitor the agent remotely. | Marked DEPRIORITIZED (R4) — justified for a local desktop app. If ever deployed as a service, this becomes critical. |

### Medium

| # | Title | Location | Why It Matters | Suggested Fix |
|---|-------|----------|---------------|---------------|
| M1 | **`escalation_count` field always 0** | SYSTEM_ANALYSIS.md §6.2 #10 | `StrategyRecordRow.escalation_count` is a dead field — never incremented. R9 added Direct→Reactive escalation but doesn't write to this field. | Repurpose to count Direct→Reactive escalations (wire in `ExecutionRouter`), or drop the column. |
| M2 | **`CharTokenCounter` fallback loses accuracy** | SYSTEM_ANALYSIS.md §6.4 | `chars / 4` is a rough approximation. When tiktoken-rs fails to load, context assembly may over- or under-allocate by 20-30%. | Log a warning when falling back. Consider bundling the tiktoken BPE data or using a more accurate character-based heuristic. |
| M3 | **WhatsApp/QQ require external bridges** | SYSTEM_ANALYSIS.md §4.1 | WhatsApp needs `ws://localhost:3001` (Node.js Baileys bridge), QQ needs a similar bridge. These are external processes not managed by the klyntbot binary. | Document the bridge setup clearly. Consider embedding the bridge or providing a Docker Compose config. |
| M4 | **No web chat channel** | SYSTEM_ANALYSIS.md §9.4 R14 | Users can only interact via 6 platform integrations or the desktop app. No browser-based fallback. | SSE streaming added to dev server (`291f4dc4`), enabling browser-based chat in dev mode. Full production web channel still needed. |
| M5 | **Session LRU eviction at 1000** | SYSTEM_ANALYSIS.md §6.4 | `DashMap` in-memory cache evicts at 1000 sessions. For personal use this is fine, but multi-tenant would need per-user limits and smarter eviction. | Acceptable for single-user. Document the limit. |
