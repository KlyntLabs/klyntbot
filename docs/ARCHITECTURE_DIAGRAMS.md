# Klyntbot Architecture Diagrams

## 1. High-Level Architecture

```mermaid
graph TD
    subgraph "L0: Foundation"
        COMMON["common<br/>KlyntbotError, MessageRole,<br/>ChannelName, ChatId, SessionKey"]
    end

    subgraph "L1: Infrastructure"
        CONFIG["config<br/>camelCase JSON, Secret&lt;String&gt;,<br/>env overrides"]
        BUS["bus<br/>MessageBus (mpsc 100),<br/>DomainEventBus (broadcast 256),<br/>LearningEventBus"]
        TOOLS_CORE["tools-core<br/>Tool trait, FeaturePackage,<br/>ToolRegistry, RoutingContext"]
        TOOLS_MACROS["tools-core-macros<br/>#[derive(Tool)],<br/>#[derive(ToolParams)],<br/>#[tool_actions]"]
    end

    subgraph "L2: Data"
        STORAGE["storage<br/>StoragePool (SqlitePool),<br/>22 Repos, WAL mode,<br/>_feature_migrations"]
        DOMAIN["domain<br/>OKR, PARA types"]
        SQLITE[(SQLite<br/>data.db)]
        LANCE[(LanceDB<br/>cognitive_fact_embeddings<br/>conv_embeddings)]
    end

    subgraph "L3: Services"
        PROVIDERS["providers<br/>DynProvider, LlmProvider,<br/>chat_stream(), NoopProvider"]
        SESSION["session<br/>SessionManager,<br/>SessionMessage"]
        SCHEDULING["scheduling<br/>CronService, CronJob"]
        CTX_ENGINE["context_engine<br/>ContextEngine, BudgetAllocator,<br/>8-Priority waterfall,<br/>HistoryCompressor, SHA-256 cache"]
    end

    subgraph "L4: Features"
        TOOLS["tools<br/>20+ built-in tools,<br/>EmbeddingEngine (384-dim)"]
        FEAT_TODO["feature-todo"]
        FEAT_FIN["feature-finance"]
        FEAT_NOTES["feature-notes"]
        FEAT_PROD["feature-productivity"]
        FEAT_COACH["feature-coaching"]
        PLUGIN_RT["plugin-runtime<br/>WASM plugins"]
    end

    subgraph "L5: Intelligence"
        CHANNELS["channels<br/>Telegram, Discord,<br/>Slack, Email"]
        AGENT["agent<br/>AgentLoop, AgentRuntime,<br/>IntentAnalyzer, ReactiveEngine,<br/>LearningService, OutcomeRecorder"]
        COGNITIVE["cognitive<br/>BackgroundConsolidation,<br/>SemanticFact, FSRS decay,<br/>Retrieval, Reflection"]
    end

    subgraph "L6: Protocol"
        MCP["mcp<br/>MCP server/client,<br/>sanitize: mcp_{server}_{tool}"]
    end

    subgraph "L7: Application"
        APP_CORE["app-core<br/>AppCore, handlers/*,<br/>init(), EventChannels"]
        DESKTOP_SHARED["desktop-shared<br/>IPC types"]
        DESKTOP["desktop<br/>Tauri 2 adapter,<br/>commands/*, dev_server.rs"]
    end

    subgraph "L8: Facade"
        KLYNTBOT["klyntbot<br/>re-export facade"]
    end

    subgraph "Frontend"
        UI["desktop-ui<br/>React + Tailwind v4 + Vite<br/>Biome 2.0, bun"]
    end

    %% Layer dependencies (upward flow)
    CONFIG --> COMMON
    BUS --> COMMON
    TOOLS_CORE --> COMMON
    STORAGE --> COMMON
    DOMAIN --> COMMON
    STORAGE --> SQLITE
    STORAGE --> LANCE
    PROVIDERS --> CONFIG
    SESSION --> STORAGE
    SCHEDULING --> STORAGE
    CTX_ENGINE --> PROVIDERS
    TOOLS --> TOOLS_CORE
    TOOLS --> TOOLS_MACROS
    FEAT_TODO --> STORAGE
    FEAT_TODO --> TOOLS_CORE
    FEAT_NOTES --> STORAGE
    CHANNELS --> BUS
    AGENT --> CTX_ENGINE
    AGENT --> PROVIDERS
    AGENT --> SESSION
    AGENT --> TOOLS
    AGENT --> COGNITIVE
    COGNITIVE --> STORAGE
    COGNITIVE --> LANCE
    MCP --> TOOLS_CORE
    APP_CORE --> AGENT
    APP_CORE --> BUS
    APP_CORE --> CHANNELS
    APP_CORE --> COGNITIVE
    APP_CORE --> SCHEDULING
    DESKTOP --> APP_CORE
    DESKTOP --> DESKTOP_SHARED
    KLYNTBOT --> APP_CORE
    UI --> DESKTOP
```

## 2. End-to-End Message Processing Flow

```mermaid
sequenceDiagram
    participant User
    participant Channel as Channel<br/>(Telegram/Discord/Slack)
    participant Bus as MessageBus<br/>(mpsc cap=100)
    participant AL as AgentLoop<br/>(mod.rs)
    participant SM as SessionManager
    participant AR as AgentRuntime<br/>(runtime.rs)
    participant AM as AgentManager<br/>(manager.rs)
    participant IA as IntentAnalyzer<br/>(analysis.rs)
    participant CE as ContextEngine<br/>(assembler.rs)
    participant ER as ExecutionRouter<br/>(router.rs)
    participant LLM as LLM Provider<br/>(DynProvider)
    participant RV as ResponseValidator
    participant CT as CostTracker
    participant DEB as DomainEventBus<br/>(broadcast 256)

    User->>Channel: sends message
    Channel->>Bus: bus.publish(InboundMessage)
    Bus->>AL: run_with_rx() receives msg

    Note over AL: Step 1: Validate message size
    Note over AL: Step 2: Handle Reactions → satisfaction score
    Note over AL: Step 3: SYSTEM_CHANNEL → process_system_message()

    AL->>SM: get_or_create(session_key)
    SM-->>AL: Session + history
    AL->>AL: spawn_embed_message() [background]

    AL->>AR: process_message(message, history, tools, event_tx)

    Note over AR: ── Step 1: Agent Selection ──
    AR->>AM: match_agent(message)
    AM-->>AR: AgentProfile (trigger-weighted scoring, fallback="general")
    Note over AR: emit AgentSelected + SkillLoaded events

    Note over AR: ── Step 2: Set active_profile ──
    AR->>AR: active_profile.write() = matched profile

    Note over AR: ── Step 3: Filter MCP tools ──
    AR->>AR: filter by profile.allows_mcp_server()
    Note over AR: Step 3b: if needs_orchestration → switch to "general",<br/>boost max_iterations ≥ 15

    Note over AR: ── Step 4: Intent Analysis ──
    AR->>IA: analyze(message, filtered_tool_names)
    Note over IA: Stage 1: analyze_heuristic()<br/>greeting→Direct(0.95), task→Reactive<br/>ambiguous→None
    opt confidence < heuristic_threshold
        IA->>LLM: IntentClassifier::classify() [JSON schema]
        LLM-->>IA: ExecutionMode + ComplexitySignals
    end
    IA-->>AR: IntentAnalysis { mode, confidence, needs_orchestration }

    Note over AR: ── Step 5: Confidence Check ──
    AR->>AR: if confidence < evaluator.threshold() → downgrade to Direct

    Note over AR: ── Step 6: Context Assembly ──
    AR->>CE: assemble(ContextRequest)
    Note over CE: 8-priority waterfall:<br/>Identity→Bootstrap→Area→Todo→<br/>Agent→Persona→Page→Cognitive
    CE-->>AR: AssembledContext { messages, token_usage }

    Note over AR: ── Step 7: Tool Filtering ──
    AR->>AR: filter_tools_for_profile()
    Note over AR: Step 7b: inject_delegation_tool() if depth < 2<br/>Step 7c: inject planning prompt if complexity ≥ 4

    Note over AR: ── Step 8: Execute ──
    AR->>ER: execute(mode, messages, tools, params)
    ER->>LLM: chat() or chat_stream()
    LLM-->>ER: LlmResponse (text or tool_calls)
    ER-->>AR: EngineResult::Complete { content, usage }

    Note over AR: ── Step 9: Validate ──
    AR->>RV: validate(content)
    Note over RV: strip &lt;confidence&gt; blocks<br/>truncate at max_response_chars<br/>system leak detection

    Note over AR: ── Step 10: Record ──
    AR->>CT: record_usage()
    AR->>AR: StrategyRepo::create(StrategyRecordRow)
    AR->>AR: InteractionRecorder::record()
    AR-->>AL: RuntimeResult { content, mode_used, agent_name }

    AL->>SM: save_to_session(assistant message)
    AL->>AL: spawn_embed_message() [background]
    AL->>DEB: publish(ChatTurnCompleted)
    AL->>Bus: publish_outbound(OutboundMessage)
    Bus->>Channel: send response
    Channel->>User: delivers reply
```

## 3. ReAct Loop + Tool Execution + Multi-Agent Delegation

```mermaid
flowchart TD
    START([ReactiveEngine::execute<br/>reactive.rs:L62]) --> INIT["iteration = 1<br/>max = analysis.iteration_budget()<br/>traces: Vec&lt;ToolTrace&gt;"]
    INIT --> CANCEL{CancellationToken<br/>cancelled?}
    CANCEL -->|yes| ABORT([Return empty EngineResult])
    CANCEL -->|no| ITER_EVENT["emit IterationStart {iteration, max}"]
    ITER_EVENT --> RUN_CYCLE

    subgraph CYCLE ["ExecutionCore::run_cycle() — core.rs:L315"]
        RUN_CYCLE["call_provider_streaming()<br/>or provider.chat()"] --> PARSE_RESP{LlmResponse<br/>has tool_calls?}
        PARSE_RESP -->|"no text"| EMPTY_OUT([CycleOutcome::EmptyResponse])
        PARSE_RESP -->|"text only"| CHECK_FAB{"is_fabricated_tool_response()?<br/>(context-aware heuristics)"}
        CHECK_FAB -->|yes| FAB_OUT([CycleOutcome::FabricatedResponse])
        CHECK_FAB -->|no| FINAL_OUT([CycleOutcome::FinalResponse])

        PARSE_RESP -->|"tool_calls present"| DEDUP{"Dedup check:<br/>hash(tool_name|args)<br/>vs seen_keys"}
        DEDUP -->|"all duplicates"| INJECT_FORCE["Inject force-different-action prompt"]
        INJECT_FORCE --> TOOLS_OUT
        DEDUP -->|"has new calls"| PAR_EXEC

        subgraph PAR_EXEC ["Parallel Tool Execution"]
            direction TB
            SEM["Semaphore(10) — max concurrency"]
            SEM --> TOOL1["Tool 1: acquire permit<br/>→ emit ToolStart<br/>→ registry.prepare() + tool.execute()<br/>→ timeout (30s default, 600s ask_user)<br/>→ emit ToolEnd<br/>→ record outcome"]
            SEM --> TOOL2["Tool 2: same flow"]
            SEM --> TOOLN["Tool N: same flow"]
        end

        PAR_EXEC --> TOOLS_OUT([CycleOutcome::ToolsExecuted])
    end

    %% Match on CycleOutcome
    FINAL_OUT --> RETURN([EngineResult::Complete<br/>{content, usage, iterations, traces}])
    EMPTY_OUT --> CONT_LOOP{iteration < max?}
    TOOLS_OUT --> ADD_REFLECT["Add reflection prompt<br/>if failures or duplicates"] --> CONT_LOOP
    FAB_OUT --> FAB_RETRY{fabrication_retries < 2?}
    FAB_RETRY -->|yes| FORCE_TOOL["Inject force-tool prompt<br/>fabrication_retries += 1"] --> RUN_CYCLE
    FAB_RETRY -->|no| RETURN

    CONT_LOOP -->|yes| INC["iteration += 1"] --> CANCEL
    CONT_LOOP -->|"no (at max)"| SYNTH["Push synthesis_prompt<br/>→ run_cycle(tools=[])<br/>→ force text output"] --> RETURN

    %% Direct Engine + Escalation
    DIRECT_START([DirectEngine::execute]) --> SINGLE_CYCLE["ExecutionCore::run_cycle()<br/>with empty tools slice"]
    SINGLE_CYCLE --> DIRECT_MATCH{CycleOutcome?}
    DIRECT_MATCH -->|FinalResponse| RETURN
    DIRECT_MATCH -->|FabricatedResponse| RETURN
    DIRECT_MATCH -->|ToolsExecuted| ESCALATE([EngineResult::Escalate<br/>→ Router re-runs as Reactive])
    DIRECT_MATCH -->|EmptyResponse| RETURN

    ESCALATE -.->|"auto-escalation"| START

    %% Delegation
    subgraph DELEGATION ["Multi-Agent Delegation — runtime.rs:L784"]
        direction TB
        DEL_TOOL["DelegationTool.execute()<br/>(injected if depth < MAX=2)"]
        DEL_TOOL --> DEL_HANDLER["DelegationHandler::delegate(<br/>agent_name, query, ctx, depth)"]
        DEL_HANDLER --> DEL_MATCH["AgentManager::get(agent_name)<br/>→ delegated AgentProfile"]
        DEL_MATCH --> DEL_CTX["ContextEngine::assemble()<br/>with delegated agent instructions"]
        DEL_CTX --> DEL_FILTER["filter_tools_for_profile()"]
        DEL_FILTER --> DEL_INJECT{"depth+1 < MAX_DEPTH(2)?"}
        DEL_INJECT -->|yes| DEL_ADD_TOOL["inject_delegation_tool(depth+1)"]
        DEL_INJECT -->|no| DEL_NO_TOOL["no delegation tool available"]
        DEL_ADD_TOOL --> DEL_EXEC["ExecutionRouter::execute(Reactive)<br/>max_iters=min(profile.max_iters, 8)"]
        DEL_NO_TOOL --> DEL_EXEC
        DEL_EXEC --> DEL_EVENT_FILTER["delegation_event_filter():<br/>suppress ContentChunk, IterationStart<br/>annotate ToolStart/ToolEnd with agent name"]
        DEL_EVENT_FILTER --> DEL_RETURN["Return content as String<br/>→ parent tool result"]
    end

    TOOL1 -.->|"if tool is delegate_to_agent"| DEL_TOOL
```

## 4. Cognitive Memory & Data Pipeline

```mermaid
flowchart TD
    subgraph EVENTS ["Domain Events — bus crate"]
        DE["DomainEventBus::publish()<br/>(broadcast, cap=256)"]
        DE --> E1["ChatTurnCompleted"]
        DE --> E2["TaskCreated/Updated"]
        DE --> E3["AreaCreated"]
        DE --> E4["FinanceRecorded"]
        DE --> E5["NoteCreated/Updated"]
    end

    subgraph SALIENCE ["Salience Filter — salience.rs"]
        E1 & E2 & E3 & E4 & E5 --> SAL["evaluate_salience(&event)"]
        SAL --> EXTRACT([SalienceVerdict::Extract])
        SAL --> ACCUMULATE([SalienceVerdict::Accumulate])
        SAL --> DISCARD([SalienceVerdict::Discard])
    end

    subgraph ACCUM ["Accumulation Buffer — background.rs"]
        ACCUMULATE --> BUF["HashMap&lt;event_type, AccumulatedEntry&gt;<br/>in-memory buffer"]
        BUF --> GATE{observations ≥ 5<br/>AND days_seen ≥ 3?}
        GATE -->|no| WAIT["Keep accumulating"]
        GATE -->|yes| PROMOTE["summarize_accumulated()<br/>→ synthetic Observation"]
    end

    subgraph OBS ["Observation — background.rs:L252"]
        EXTRACT --> OBS_CREATE["event_to_observation(&event)<br/>→ Observation {domain, content,<br/>importance, source_event}"]
        PROMOTE --> OBS_CREATE
    end

    subgraph EXTRACTION ["Extraction — extraction.rs"]
        OBS_CREATE --> EXT["ExtractionHandler::extract_facts(&obs)"]
        EXT --> EXT_LLM["LlmExtractionHandler<br/>POST with EXTRACTION_SYSTEM_PROMPT<br/>→ JSON {facts: [{domain, subject,<br/>predicate, object, confidence}]}"]
        EXT --> EXT_HEUR["HeuristicExtractionHandler<br/>(fallback on LLM error)"]
        EXT_LLM --> TO_FACT["to_semantic_fact(candidate, &obs)<br/>→ SemanticFact {id: uuid,<br/>stability: 1.0, ...}"]
        EXT_HEUR --> TO_FACT
    end

    subgraph CONSOLIDATION ["Consolidation — consolidation.rs"]
        TO_FACT --> BATCH["consolidate_batch(&facts, repo, handler, embedder)"]
        BATCH --> PER_FACT["consolidate_fact(candidate, repo, handler)"]
        PER_FACT --> FIND_SIM["repo.find_similar(subject, predicate)"]
        FIND_SIM --> NO_EXISTING{existing<br/>facts found?}
        NO_EXISTING -->|no| ADD["MemoryOp::Add<br/>repo.upsert(candidate)<br/>embedder.embed_and_store_fact()"]
        NO_EXISTING -->|yes| DECIDE["ConsolidationHandler::decide<br/>(candidate, existing)"]
        DECIDE --> UPDATE["MemoryOp::Update<br/>repo.supersede(old, new)<br/>re-embed"]
        DECIDE --> DELETE["MemoryOp::Delete<br/>repo.supersede(id, by)"]
        DECIDE --> NOOP["MemoryOp::Noop"]
    end

    subgraph STORAGE ["Storage Layer"]
        ADD & UPDATE --> SQLITE_FACT[("SQLite<br/>semantic_facts table<br/>bi-temporal: valid_from,<br/>valid_until, superseded_at")]
        ADD & UPDATE --> LANCE_VEC[("LanceDB<br/>cognitive_fact_embeddings<br/>384-dim fastembed<br/>text: '{subject} {predicate} {object}'")]
        DELETE --> SQLITE_FACT
    end

    subgraph EPISODIC ["Episodic Memory"]
        OBS_CREATE --> IMP_CHECK{importance ≥ 0.7?}
        IMP_CHECK -->|yes| EP_STORE["EpisodicMemoryRepo::insert()<br/>EpisodicMemory {domain,<br/>content, importance}"]
        IMP_CHECK -->|no| SKIP_EP["Skip episodic storage"]
    end

    subgraph RETRIEVAL ["Retrieval — retrieval.rs"]
        QUERY["ContextEngine request<br/>(message or intent_summary)"] --> VEC_SEARCH["SemanticFactEmbedder::search_similar<br/>(query, domains, top_k=30,<br/>min_similarity=0.55)"]
        VEC_SEARCH --> GET_FACTS["repo.get_batch(ids)"]
        GET_FACTS --> SCORE["relevance_score():<br/>semantic: 0.3 × cosine_sim<br/>retrievability: 0.2 × FSRS<br/>importance: 0.15 × confidence<br/>frequency: 0.1 × access_count<br/>situational: 0.25 × boost"]
        SCORE --> FSRS_UPDATE["record_access(id)<br/>update_stability(current, true)<br/>new = current + ln(1+current).max(0.1)"]
        VEC_SEARCH --> FALLBACK{results < 3?}
        FALLBACK -->|yes| SQL_FALLBACK["SQL: list_active per domain<br/>score with semantic_sim=0.5"]
        FALLBACK -->|no| FILTER["filter score > 0.3"]
        SQL_FALLBACK --> MERGE["Merge + deduplicate by ID"]
        MERGE --> FILTER
    end

    subgraph USER_MODEL ["User Model — context_source.rs"]
        FILTER --> STATIC["Static tier: top 10 facts<br/>sorted by confidence × stability<br/>across 6 USER_MODEL_DOMAINS"]
        FILTER --> DYNAMIC["Dynamic tier: vector-searched<br/>facts specific to current query"]
        STATIC & DYNAMIC --> INJECT["Inject as '# User Understanding'<br/>block in LLM system prompt"]
    end

    subgraph REFLECTION ["Weekly Reflection — reflection.rs"]
        CRON["CronService: Monday 9am<br/>'__klyntbot_cognitive_weekly_reflection'"]
        CRON --> LOAD["Load 7-day episodic memories<br/>+ UserModel + ProceduralRules"]
        LOAD --> REFLECT_LLM["ReflectionHandler::reflect(&input)<br/>→ JSON {fact_updates, rule_updates, summary}"]
        REFLECT_LLM --> VALIDATE["Filter: source=='user_stated'<br/>OR confidence ≥ 0.7"]
        VALIDATE --> BATCH
        REFLECT_LLM --> RULES["ProceduralRuleRepo::upsert(rule)"]
        REFLECT_LLM --> EP_REFLECT["Store reflection as EpisodicMemory<br/>stability=5.0, importance=0.9"]
    end

    subgraph DECAY ["FSRS Decay — decay.rs"]
        FSRS_FORMULA["retrievability(elapsed, stability)<br/>= exp(ln(0.9) × elapsed / stability)"]
        COMPACT["compaction.rs: archive_superseded()<br/>after 90 days"]
    end
```

## 5. Adaptive Learning & Training Loop

```mermaid
flowchart LR
    subgraph RECORDING ["Outcome Recording — recorder.rs"]
        TOOL_EXEC["Tool execution<br/>in ReactiveEngine"] --> RECORD["OutcomeRecorder::record()<br/>(privacy-by-omission:<br/>no tool_args, no user_message)"]
        RECORD --> HASH["FNV-1a hash session_key<br/>'telegram:abc123' →<br/>'telegram:a1b2c3d4'"]
        HASH --> STORE[("OutcomeStore<br/>(SQLite via OutcomeRepo)<br/>OutcomeRow {id, tool_name,<br/>success, duration_ms,<br/>channel_prefix}")]
    end

    subgraph SERVICE ["LearningService — service.rs:L1-L208"]
        LOOP["Background tokio::select! loop<br/>interval from config"] --> LOAD_OUTCOMES["Load recent outcomes<br/>from OutcomeStore"]
        LOAD_OUTCOMES --> ANALYZER
    end

    subgraph ANALYZER ["LearningAnalyzer"]
        direction TB
        ANALYZE["analyze(outcomes, feedback)"] --> BANDS["Compute per-tool ConfidenceBands:<br/>[0.0,0.3) [0.3,0.5) [0.5,0.7)<br/>[0.7,0.85) [0.85,1.0]"]
        BANDS --> SUGGEST["suggest_threshold:<br/>lowest band with ≥5 samples<br/>AND ≥80% success rate<br/>(default: 0.7)"]
        SUGGEST --> CONF["threshold_confidence:<br/>0.2 (≤10 pts) → 0.5 (≤50)<br/>→ 0.7 (≤200) → 0.9 (>200)"]
        CONF --> PATTERN["PatternAnalyzer::analyze()<br/>behavioral pattern extraction"]
    end

    subgraph ADAPTIVE ["AdaptiveThresholds — adaptive.rs"]
        SUGGEST --> APPLY["apply_analysis(analysis)"]
        APPLY --> COLD_CHECK{total_outcomes ≥<br/>min_outcomes (50)?}
        COLD_CHECK -->|no| SKIP["Cold-start protection:<br/>keep default threshold"]
        COLD_CHECK -->|yes| DELTA["Compute delta =<br/>suggested - current"]
        DELTA --> CLAMP["Clamp to ±MAX_THRESHOLD_STEP<br/>(0.05 per cycle)"]
        CLAMP --> BOUNDS["Clamp to [min_threshold,<br/>max_threshold]"]
        BOUNDS --> PERSIST["Persist to LearningStateRepo<br/>(key: 'adaptive_thresholds',<br/>JSON blob)"]
    end

    subgraph BROADCAST ["Event Propagation"]
        PERSIST --> PUB["LearningEventBus::publish()"]
        PUB --> THR_CHANGED["LearningEvent::ThresholdChanged"]
        PUB --> ANALYSIS_DONE["LearningEvent::AnalysisCompleted"]
        THR_CHANGED --> ATOMIC["ConfidenceEvaluator<br/>Arc&lt;AtomicU32&gt;::store(<br/>new_threshold.to_bits())"]
    end

    subgraph EVALUATOR ["ConfidenceEvaluator — evaluator.rs"]
        ATOMIC --> EVAL["evaluate(llm_output)"]
        EVAL --> PARSE["Parse &lt;confidence&gt; JSON:<br/>{intent_clarity, tool_fit,<br/>info_sufficiency}"]
        PARSE --> AVG["weighted_avg ≥ threshold()?"]
        AVG -->|yes| PROCEED["Allow current ExecutionMode"]
        AVG -->|no| DOWNGRADE["Downgrade to<br/>ExecutionMode::Direct"]
    end

    subgraph STRATEGY ["Strategy Feedback — StrategyRepo"]
        STRAT_RECORD["Every pipeline execution →<br/>StrategyRecordRow {<br/>predicted_strategy,<br/>actual_strategy,<br/>escalation_count,<br/>complexity_signals}"]
        STRAT_RECORD --> STRAT_CACHE["IntentAnalyzer caches<br/>StrategySummaryRow (60s TTL)<br/>→ injected into LLM<br/>classifier prompt"]
    end
```

## 6. Context Assembly Waterfall

```mermaid
flowchart TD
    START([ContextEngine::assemble<br/>assembler.rs:L120]) --> BUDGET["BudgetAllocator::new()<br/>total = model_context_window<br/>response_reserve = 15%<br/>available = 85% of total"]

    BUDGET --> P0["Priority 0: SystemIdentity<br/>───────────────────────<br/>IdentitySource: app name + version<br/>BootstrapSource: base instructions<br/>→ allocate(tokens used)"]

    P0 --> P1["Priority 1: ToolDefinitions<br/>───────────────────────<br/>Tool schemas (OpenAI function format)<br/>Only if mode ≠ Direct<br/>→ allocate(schema_tokens)"]

    P1 --> P2["Priority 2: AgentInstructions<br/>───────────────────────<br/>AgentContextSource reads active_profile<br/>→ AGENT.md content + skills/<br/>→ allocate(agent_tokens)"]

    P2 --> P3["Priority 3: PersonaContext<br/>───────────────────────<br/>PersonaContextSource<br/>→ active persona rules + style<br/>→ allocate(persona_tokens)"]

    P3 --> P4["Priority 4: CognitiveMemory<br/>───────────────────────<br/>CognitiveContextSource (60s TTL cache):<br/> Static: top 10 facts by confidence×stability<br/>   across 6 domains: identity, energy, work,<br/>   finance, learning, preferences<br/> Dynamic: vector search (384-dim fastembed)<br/>   top_k=30, min_sim=0.55, score>0.3<br/>   FSRS retrievability weighting<br/>→ '# User Understanding' block<br/>→ allocate(cognitive_tokens)"]

    P4 --> P5["Priority 5: RetrievedMemory<br/>───────────────────────<br/>CognitiveMemoryRetriever<br/>→ ConversationRecallService::search()<br/>   embed query → cosine search<br/>   time-decay: score × decay^days<br/>   (half_life=138 days)<br/>   threshold=0.4, limit=5<br/>→ allocate(memory_tokens)<br/>(skipped for Clarification mode)"]

    P5 --> P6["Priority 6: PageContext<br/>───────────────────────<br/>PageContextSource<br/>→ TODO items, area context<br/>→ allocate(page_tokens)"]

    P6 --> P7["Priority 7: Skills<br/>───────────────────────<br/>ProductivityContextSource<br/>→ focus state, nudges, scores<br/>→ allocate(skill_tokens)"]

    P7 --> REMAINING["remaining_budget =<br/>available - Σ(allocated)"]

    REMAINING --> COMPRESS["HistoryCompressor::compress_async()<br/>───────────────────────<br/>min_recent = 4 messages verbatim<br/>expand recent if budget allows<br/>older → chunks of 5 →<br/>  Abstractive: LLM per chunk<br/>  (fallback: Extractive)<br/>→ allocate(RecentHistory)<br/>→ allocate(CompressedHistory)"]

    COMPRESS --> ASSEMBLE["Build final message list:<br/>┌─ System prompt (all sources merged) ─┐<br/>│ MemorySystem (retrieved memories)    │<br/>│ SummarySystem(s) (compressed chunks)  │<br/>└─ Recent messages (min 4, verbatim)  ─┘"]

    ASSEMBLE --> CACHE["SHA-256 cache key:<br/>system_prompt + history_len +<br/>last_message + strategy +<br/>tool_count + window<br/>LRU capacity=8, generation-invalidated"]

    CACHE --> RESULT([AssembledContext<br/>{messages, token_usage,<br/>sources_used}])
```

## 7. Boot Sequence

```mermaid
sequenceDiagram
    participant Main as main.rs<br/>(desktop)
    participant Tauri as Tauri::Builder
    participant DA as desktop/app_core.rs
    participant AC as AppCore::init()<br/>(app-core/init.rs)
    participant SP as StoragePool
    participant VS as VectorStore
    participant PM as ProviderManager
    participant MB as MessageBus
    participant CS as CronService
    participant PrM as PersonaManager
    participant DEB as DomainEventBus
    participant ALB as AgentLoopBuilder
    participant AL as AgentLoop
    participant CM as ChannelManager
    participant PE as ProductivityEngine
    participant COS as CoachingService

    Main->>Tauri: Builder::default()<br/>.plugin(global_shortcut Alt+Space)
    Tauri->>DA: setup(|app| block_on(init(handle)))
    DA->>AC: AppCore::init(None)

    Note over AC: Step 1: Config
    AC->>AC: config::load_with_env_overrides()

    Note over AC: Step 2: Storage
    AC->>SP: StoragePool::connect(&data_dir)<br/>SQLite WAL + foreign keys + sqlx migrations
    SP-->>AC: storage_pool
    AC->>AC: Repos::from_pool() → 22 repo handles
    AC->>VS: VectorStore::connect(&data_dir)<br/>LanceDB + ANN index [background]
    AC->>SP: run_feature_migrations(notes)
    AC->>AC: NoteRepo::new(notes_pool)

    Note over AC: Step 3: Providers
    AC->>PM: create_provider(&config)<br/>(graceful NoopProvider fallback)
    AC->>PM: create_cognitive_provider(&config)<br/>(optional, separate LLM)

    Note over AC: Step 4: MessageBus
    AC->>MB: MessageBus::new(100)<br/>mpsc inbound + outbound

    Note over AC: Step 5: Scheduling
    AC->>CS: CronService::new(repos.cron)
    CS->>CS: start() tick loop
    AC->>AC: register_cron_callbacks()<br/>single Arc<Fn> match dispatch
    AC->>AC: ensure_cron_jobs()<br/>idempotent job registration

    Note over AC: Step 6: Personas
    AC->>PrM: PersonaManager::load(&personas_dir)
    PrM->>PrM: resolve_scopes(&repos)

    Note over AC: Step 7: Event Buses
    AC->>DEB: DomainEventBus::new(256)
    AC->>AC: broadcast::channel::<PipelineEvent>(256)

    Note over AC: Step 8: AgentLoop Build
    AC->>ALB: AgentLoop::builder(bus, provider, config)
    ALB->>ALB: .with_pool().with_cron_service()<br/>.with_domain_bus().with_cognitive_provider()<br/>.with_pipeline_tx()
    ALB->>ALB: build().await
    Note over ALB: ├─ StoragePool::from_existing()<br/>├─ AgentManager::load_builtin_agents() [5]<br/>├─ EmbeddingEngine::new() [fastembed 384-dim]<br/>├─ Assemble 8+ ContextSources by priority<br/>├─ Run cognitive migrations<br/>├─ BackgroundConsolidationService::start()<br/>├─ ToolRegistry: register 20+ tools<br/>├─ Load WASM plugins<br/>├─ Connect MCP servers<br/>├─ ConfidenceEvaluator + LearningService.start()<br/>├─ IntentAnalyzer + ExecutionRouter<br/>├─ AgentRuntime (2-phase Arc init)<br/>└─ SessionCleanupService + MemoryMaintenance
    ALB-->>AC: AgentLoop

    Note over AC: Step 9: Channels
    AC->>CM: ChannelManager::new(config, bus)

    Note over AC: Step 10: Productivity
    AC->>PE: ProductivityEngine::start()<br/>FocusManager, DailyAggregator,<br/>NudgeService, DistractionInterceptor

    Note over AC: Step 11: Coaching
    AC->>COS: CoachingService::start()<br/>SignalAccumulator, PatternDetector,<br/>InterventionRouter, FeedbackTracker

    Note over AC: Step 12: Assemble AppCore struct

    Note over AC: Step 13: Spawn background tasks
    AC->>AL: tokio::spawn(agent_loop.run_with_rx(inbound_rx))
    AC->>CM: tokio::spawn(channel_manager.start_all())

    Note over AC: Steps 14-16
    AC->>AC: spawn daily analytics cleanup (24h)
    AC->>AC: spawn_event_log_persistence<br/>(domain + pipeline → SQLite)
    AC-->>DA: (AppCore, EventChannels)

    DA->>DA: wire_event_channels()
    Note over DA: ├─ auto_focus_rx → emit "productivity:auto_focus"<br/>├─ dashboard_tick_rx → DashboardEmitter<br/>├─ nudge_rx → emit "productivity:nudge"<br/>├─ intervention_rx → emit "coaching:intervention"<br/>├─ domain_event_bus → emit "cognitive:domain_event"<br/>└─ pipeline_rx → emit "cognitive:extraction/consolidation"

    DA-->>Tauri: app.manage(core)
    Tauri->>Main: Register 100+ Tauri commands
    Main->>Main: app.run() — event loop started
```

## 8. Master Comprehensive Workflow Diagram

```mermaid
flowchart TD
    USER(["👤 User Message"]) --> CHANNEL

    subgraph CHANNELS ["L5: Platform Channels"]
        CHANNEL["Telegram / Discord /<br/>Slack / Email"]
    end

    CHANNEL --> BUS_IN

    subgraph BUS ["L1: MessageBus (mpsc cap=100)"]
        BUS_IN["InboundMessage<br/>{channel, chat_id, content, role}"]
        BUS_OUT["OutboundMessage<br/>{channel, chat_id, content}"]
    end

    BUS_IN --> AGENT_LOOP

    subgraph AGENT_CRATE ["L5: Agent Crate"]
        subgraph AGENT_LOOP_SG ["AgentLoop — mod.rs"]
            AGENT_LOOP["run_with_rx() receives msg"]
            VALIDATE["Validate size + handle reactions"]
            SESSION_LOAD["SessionManager::get_or_create()<br/>→ load history"]
            EMBED_BG["spawn_embed_message()<br/>(background)"]
            AGENT_LOOP --> VALIDATE --> SESSION_LOAD --> EMBED_BG
        end

        EMBED_BG --> RUNTIME

        subgraph RUNTIME_SG ["AgentRuntime — runtime.rs (10 steps)"]
            RUNTIME["process_message()"]

            subgraph STEP1 ["Step 1: Agent Selection"]
                AGENT_MATCH["AgentManager::match_agent()<br/>trigger-weighted scoring<br/>5 built-in: general, task,<br/>finance, automation, communication"]
            end

            subgraph STEP4 ["Step 4: Intent Analysis"]
                HEURISTIC["analyze_heuristic()<br/>greeting→Direct(0.95)<br/>task CRUD→Reactive<br/>question→Direct"]
                LLM_CLASSIFY["IntentClassifier::classify()<br/>(LLM fallback if ambiguous)"]
                HEURISTIC -->|"confidence < threshold"| LLM_CLASSIFY
            end

            subgraph STEP5 ["Step 5: Confidence Gate"]
                CONF_EVAL["ConfidenceEvaluator<br/>Arc&lt;AtomicU32&gt; threshold<br/>→ downgrade if below"]
            end

            RUNTIME --> STEP1 --> STEP4 --> STEP5
        end

        STEP5 --> CTX_ASM

        subgraph CTX_ENGINE_SG ["L3: ContextEngine"]
            CTX_ASM["assemble(ContextRequest)"]

            subgraph WATERFALL ["8-Priority Token Waterfall"]
                WF0["P0: SystemIdentity"]
                WF1["P1: ToolDefinitions"]
                WF2["P2: AgentInstructions"]
                WF3["P3: PersonaContext"]
                WF4["P4: CognitiveMemory<br/>(static + dynamic)"]
                WF5["P5: RetrievedMemory<br/>(conversation recall)"]
                WF6["P6: PageContext"]
                WF7["P7: Skills"]
                WF_HIST["History: compress_async()<br/>min 4 verbatim + summaries"]
                WF0 --> WF1 --> WF2 --> WF3 --> WF4 --> WF5 --> WF6 --> WF7 --> WF_HIST
            end
            CTX_ASM --> WATERFALL
        end

        WF4 -.->|"vector search"| COGNITIVE_RETRIEVE
        WF5 -.->|"embed + cosine"| CONV_RECALL

        WF_HIST --> TOOL_FILTER

        subgraph STEP7 ["Step 7: Tool Filtering"]
            TOOL_FILTER["filter_tools_for_profile()<br/>+ inject_delegation_tool(depth<2)<br/>+ planning prompt (complexity≥4)"]
        end

        TOOL_FILTER --> EXEC_ROUTER

        subgraph EXECUTION ["Step 8: Execution"]
            EXEC_ROUTER["ExecutionRouter::execute()"]
            EXEC_ROUTER --> DIRECT_PATH
            EXEC_ROUTER --> REACTIVE_PATH

            subgraph DIRECT_PATH ["DirectEngine"]
                DIRECT["Single LLM call<br/>no tools"]
                DIRECT --> DIRECT_OUT{outcome?}
                DIRECT_OUT -->|"ToolsExecuted"| ESCALATE["Auto-escalate<br/>to Reactive"]
                DIRECT_OUT -->|"FinalResponse"| DONE_D["Complete"]
            end

            subgraph REACTIVE_PATH ["ReactiveEngine — ReAct Loop"]
                REACT_START["iteration 1..max_iterations"]
                REACT_LLM["ExecutionCore::run_cycle()<br/>→ provider.chat_stream()"]
                REACT_START --> REACT_LLM

                REACT_LLM --> REACT_MATCH{CycleOutcome}
                REACT_MATCH -->|FinalResponse| DONE_R["Complete"]
                REACT_MATCH -->|EmptyResponse| REACT_CONT["continue loop"]
                REACT_MATCH -->|FabricatedResponse| FAB_RETRY["retry ≤ 2"]
                REACT_MATCH -->|ToolsExecuted| TOOLS_SG

                subgraph TOOLS_SG ["Parallel Tool Execution"]
                    TOOL_SEM["Semaphore(10)"]
                    TOOL_EXEC["join_all(tool futures)<br/>timeout: 30s default<br/>600s for ask_user"]
                    TOOL_DEDUP["Hash dedup:<br/>tool_name|args_hash"]
                    TOOL_SEM --> TOOL_EXEC
                    TOOL_DEDUP --> TOOL_EXEC
                end

                TOOLS_SG --> OUTCOME_REC["OutcomeRecorder::record()"]
                OUTCOME_REC --> REACT_CONT
                REACT_CONT --> REACT_START
                FAB_RETRY --> REACT_LLM

                REACT_MATCH -->|"at max_iterations"| SYNTH["Synthesis prompt<br/>→ run_cycle(tools=[])"]
                SYNTH --> DONE_R
            end

            ESCALATE -.-> REACT_START

            %% Delegation
            TOOL_EXEC -->|"delegate_to_agent tool"| DELEGATION

            subgraph DELEGATION ["Multi-Agent Delegation (depth≤2)"]
                DEL["DelegationHandler::delegate()"]
                DEL --> DEL_PROFILE["Load delegated AgentProfile"]
                DEL_PROFILE --> DEL_CTX["ContextEngine::assemble()<br/>with delegated instructions"]
                DEL_CTX --> DEL_EXEC["ReactiveEngine<br/>max_iters=min(profile, 8)"]
                DEL_EXEC --> DEL_RETURN["Return content to parent"]
            end
        end

        DONE_D --> VALIDATE_RESP
        DONE_R --> VALIDATE_RESP

        subgraph STEP9_10 ["Steps 9-10: Validate & Record"]
            VALIDATE_RESP["ResponseValidator::validate()<br/>strip confidence, truncate,<br/>system leak detection"]
            COST["CostTracker::record()"]
            STRAT["StrategyRepo::create()<br/>(StrategyRecordRow)"]
            INTERACT["InteractionRecorder::record()"]
            VALIDATE_RESP --> COST --> STRAT --> INTERACT
        end
    end

    INTERACT --> SAVE_SESSION["SessionManager::save()"]
    SAVE_SESSION --> DOMAIN_PUB["DomainEventBus::publish()<br/>(ChatTurnCompleted)"]
    DOMAIN_PUB --> BUS_OUT

    BUS_OUT --> CHANNEL
    CHANNEL --> USER

    %% Cognitive Pipeline (background)
    DOMAIN_PUB --> COG_BG

    subgraph COGNITIVE_SG ["L5: Cognitive Pipeline (background)"]
        COG_BG["BackgroundConsolidationService"]
        COG_SAL["Salience Filter<br/>Extract / Accumulate / Discard"]
        COG_EXT["ExtractionHandler<br/>(LLM + heuristic fallback)"]
        COG_CON["ConsolidationHandler<br/>Add / Update / Delete / Noop"]
        COG_BG --> COG_SAL --> COG_EXT --> COG_CON
    end

    subgraph COGNITIVE_STORAGE ["Cognitive Storage"]
        SEMANTIC_FACTS[("SQLite: semantic_facts<br/>bi-temporal, supersedable")]
        VEC_EMB[("LanceDB: 384-dim<br/>cognitive_fact_embeddings")]
        EPISODIC[("SQLite: episodic_memories")]
        RULES[("SQLite: procedural_rules")]
    end

    COG_CON --> SEMANTIC_FACTS
    COG_CON --> VEC_EMB
    COG_BG -->|"importance ≥ 0.7"| EPISODIC

    subgraph COGNITIVE_RETRIEVE ["Cognitive Retrieval"]
        RETRIEVE["retrieve_relevant_facts()<br/>vector search + FSRS scoring"]
        RETRIEVE --> VEC_EMB
        RETRIEVE --> SEMANTIC_FACTS
    end

    subgraph CONV_RECALL ["Conversation Recall"]
        CONV_STORE["ConversationRecallService::store()"]
        CONV_SEARCH["search(query, limit, threshold=0.4)<br/>time-decay half_life=138 days"]
        EMBED_BG -.-> CONV_STORE
    end

    subgraph REFLECTION_SG ["Weekly Reflection (Cron Monday 9am)"]
        REFLECT["run_weekly_reflection()"]
        REFLECT -->|"load 7-day episodic"| EPISODIC
        REFLECT -->|"consolidate facts"| COG_CON
        REFLECT -->|"upsert rules"| RULES
    end

    subgraph LEARNING_SG ["Adaptive Learning (background)"]
        LEARN_SVC["LearningService<br/>(periodic loop)"]
        LEARN_ANALYZE["LearningAnalyzer<br/>per-tool ConfidenceBands"]
        LEARN_ADAPT["AdaptiveThresholds<br/>±0.05 max step, min 50 outcomes"]
        LEARN_ATOMIC["AtomicU32::store()<br/>→ ConfidenceEvaluator"]
        OUTCOME_REC -.-> LEARN_SVC --> LEARN_ANALYZE --> LEARN_ADAPT --> LEARN_ATOMIC
        LEARN_ATOMIC -.->|"updates threshold"| CONF_EVAL
    end
```

---

## Summary of the Complete Workflow

1. **Message ingestion**: User messages arrive via platform channels (Telegram/Discord/Slack/Email), are published to the `MessageBus` (mpsc, cap 100), and received by `AgentLoop::run_with_rx()`.

2. **Session + embedding**: `SessionManager` loads/creates session history. Messages are embedded in background via `ConversationRecallService` (384-dim fastembed → LanceDB).

3. **Agent matching**: `AgentManager` scores the message against 5 built-in agent profiles using trigger-weighted matching. Orchestration detection can override to the "general" agent with boosted iterations (≥15).

4. **Two-stage intent analysis**: Heuristic patterns (0ms) classify greetings/tasks/questions. Ambiguous messages fall through to an LLM classifier. `ConfidenceEvaluator` (lock-free `AtomicU32` threshold, continuously tuned by `LearningService`) can downgrade Reactive→Direct.

5. **Context assembly**: An 8-priority token waterfall fills the context window — system identity, tools, agent instructions, persona, cognitive memory (static user model + dynamic vector search with FSRS scoring), conversation recall, page context, and skills — then compresses history (min 4 verbatim + abstractive summaries).

6. **Execution**: `ExecutionRouter` dispatches to `DirectEngine` (single call) or `ReactiveEngine` (ReAct loop, 1..max_iterations). Tools execute in parallel (Semaphore(10), `join_all`). Hash-based dedup, fabrication detection with retry, and auto-escalation Direct→Reactive on unexpected tool calls.

7. **Delegation**: The orchestrator can delegate to specialized agents via `DelegationTool` (max depth=2). Sub-agents get their own context assembly, tool filtering, and Reactive execution with event filtering.

8. **Validation + recording**: `ResponseValidator` strips confidence blocks, truncates, and detects system leaks. `CostTracker`, `StrategyRepo`, and `InteractionRecorder` log everything.

9. **Cognitive pipeline**: `DomainEventBus` broadcasts `ChatTurnCompleted` → `BackgroundConsolidationService` applies salience filtering → LLM extraction → consolidation (Add/Update/Delete/Noop) → SQLite semantic facts + LanceDB embeddings. Episodic memories stored for importance ≥ 0.7.

10. **Learning loop**: `OutcomeRecorder` feeds tool results to `LearningService` → `LearningAnalyzer` computes per-tool confidence bands → `AdaptiveThresholds` adjusts the evaluator threshold (±0.05/cycle, min 50 outcomes) → lock-free propagation to `ConfidenceEvaluator`.

11. **Weekly reflection**: Monday 9am cron loads 7-day episodic memories + user model + procedural rules → LLM synthesis → consolidates new facts and upserts rules. The reflection itself is stored as an episodic memory (stability=5.0).

12. **Response delivery**: The validated response is published as `OutboundMessage` via `MessageBus`, routed through the originating channel back to the user.

---

## Implementation Gaps & Technical Debt Analysis

### High

**3. HistoryCompressor makes N/5 LLM calls for abstractive compression**
- **Location**: `crates/context_engine/src/` — history compression
- **Why it matters**: 100 messages → 19 LLM calls for compression, each blocking. No batching, no parallelism. Adds latency to every request with long histories.
- **Fix**: Batch chunks into a single LLM call with structured output, or parallelize chunk compression with `join_all`.

### Medium

**4. Single monolithic cron callback**
- **Location**: `crates/app-core/src/init.rs:L434-L618` — `register_cron_callbacks()`
- **Why it matters**: All cron job types share one `match job_name.as_str()` dispatch. Adding new jobs requires editing this growing match block.
- **Fix**: Use a trait-based callback registry where each job type registers its own handler.

**5. Context cache invalidated on every tool execution**
- **Location**: `crates/context_engine/src/assembler.rs` — generation counter
- **Why it matters**: `invalidate_cache()` bumps a generation counter that marks all 8 LRU entries stale. Since tool execution always triggers re-assembly, the cache only benefits the first assembly in a request cycle.
- **Fix**: Use a more granular invalidation strategy — only invalidate when specific context sources change (e.g., session history appended, new cognitive facts).

**6. `block_on` in Tauri setup blocks UI thread**
- **Location**: `crates/desktop/src/main.rs:L53`
- **Why it matters**: `tauri::async_runtime::block_on(app_core::init(handle))` blocks the Tauri setup thread for the entire boot sequence (potentially several seconds). Users see a blank window during initialization.
- **Fix**: Show a loading/splash screen, then complete initialization asynchronously. Or move heavy init (LLM provider, MCP connections, embedding engine) to a post-setup task.

### Resolved

The following gaps have been addressed:

- ~~Synthesis at max_iterations can return empty string~~ — Fixed: fallback to trace summary instead of empty string (`reactive.rs`)
- ~~BudgetAllocator silent failure on small context windows~~ — Fixed: warning emitted when remaining budget < 15% (`budget.rs`)
- ~~Reflexive stability inflation in FSRS~~ — Fixed: capped at `MAX_STABILITY = 30.0` (`decay.rs`)
- ~~USER_MODEL_DOMAINS hardcoded to 6~~ — Fixed: expanded to 10 domains with `other` catch-all field (`types.rs`, `repos/mod.rs`, `context_source.rs`)
- ~~Notes migrations run twice~~ — Fixed: removed duplicate call in `AgentLoopBuilder::build()` (`builder.rs`)
- ~~`threshold_confidence` computed but unused~~ — Fixed: scales `MAX_THRESHOLD_STEP` for faster convergence (`adaptive.rs`)
- ~~IntentPipeline is dead code~~ — Fixed: deleted `pipeline.rs`, moved `PipelineConfig` to `types.rs`, removed e2e tests
- ~~No test for delegation at max depth~~ — Already resolved: `test_delegation_tool_not_injected_at_max_depth` exists in `runtime.rs`
- ~~Cognitive provider created twice~~ — Fixed: stored `Option<DynProvider>` in `AppCore`, reflection handler uses it directly (`state.rs`, `cognitive.rs`)
- ~~AccumulatedEntry buffer not persisted across restarts~~ — Fixed: added `accumulated_observations` table, repo, and migration; `BackgroundConsolidationService` loads on startup, persists on add, deletes on promotion (`background.rs`, `repos/accumulated_observation.rs`)
- ~~Dev server dispatch must be manually updated~~ — Fixed: co-located `dispatch_dev()` functions in each command module with `DEV_COMMANDS` const arrays; chained dispatch in `dev_server.rs`; parity test ensures all Tauri commands have dev server coverage (`commands/*.rs`, `dev_server.rs`)
