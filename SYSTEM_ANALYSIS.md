# Klyntbot / Nanobot — Comprehensive System Analysis

> Generated: 2026-03-08 | Scope: Full codebase analysis across 27 crates

---

## Table of Contents

1. [System Architecture](#1-system-architecture)
2. [Memory System Deep Dive](#2-memory-system-deep-dive)
3. [Intelligence & Agent Runtime](#3-intelligence--agent-runtime)
4. [Integrated Subsystems & Services](#4-integrated-subsystems--services)
5. [Data Flow & Component Interactions](#5-data-flow--component-interactions)
6. [Technical Analysis](#6-technical-analysis)
7. [Comparison with Common AI Architectures](#7-comparison-with-common-ai-architectures)
8. [Scoring & Evaluation](#8-scoring--evaluation)
9. [Recommendations](#9-recommendations)

---

## 1. System Architecture

### 1.1 Overview

Klyntbot is a **Rust-based personal AI agent** — a single binary connecting 6+ chat platforms to multiple LLM providers with task/project management, persistent memory, and a native desktop application. All state lives in SQLite (relational) + LanceDB (vectors).

### 1.2 Workspace Structure (27 Crates, 9 Layers)

Dependencies flow **strictly upward** — no circular dependencies, no same-layer imports.

```
L8: klyntbot              — Re-export facade + binary entry point
L7: app-core, desktop-shared, desktop — Application core, Tauri desktop app
L6: cli, mcp              — CLI (serve/init/status/plugin), MCP server/client
L5: channels, agent, cognitive — Platform integrations, agent runtime, cognitive memory
L4: tools, feature-todo, feature-finance, feature-notes,
    feature-productivity, feature-coaching, plugin-runtime — Tools, features, WASM plugins
L3: providers, session, scheduling, context_engine — LLM clients, sessions, cron, token budgets
L2: storage, domain       — SqlitePool, migrations, repos, OKR+PARA domain types
L1: config, bus, tools-core, tools-core-macros — Config, message bus, Tool traits, derive macros
L0: common                — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey
```

### 1.3 Crate Responsibilities

| Crate | Layer | Role |
|-------|-------|------|
| `common` | L0 | Shared error types, enums (`MessageRole`, `ChannelName`, `ChatId`), `Result<T>` alias |
| `config` | L1 | `Config` struct with `#[serde(rename_all = "camelCase")]`, env override via `KLYNTBOT_*`, `Secret<String>` for API keys |
| `bus` | L1 | `MessageBus` with two `tokio::mpsc` channels (inbound + outbound, buffer=100) |
| `tools-core` | L1 | `Tool` trait, `FeaturePackage` trait, `PermissionLevel`, `RoutingContext` |
| `tools-core-macros` | L1 | `#[derive(Tool)]`, `#[derive(ToolParams)]`, `#[tool_actions]` proc macros |
| `storage` | L2 | `StoragePool` (wraps `SqlitePool`), 24 repository structs, `VectorStore` (LanceDB), migrations |
| `domain` | L2 | Pure domain types: `Area`, `Project`, `Objective`, `KeyResult` (OKR+PARA), no DB deps |
| `providers` | L3 | `ProviderManager` with circuit breaker (5-failure/60s), 14 LLM providers, retry with exponential backoff |
| `session` | L3 | `SessionManager` with `DashMap` for per-session locking, LRU eviction at 1000 sessions, SQL persistence |
| `scheduling` | L3 | `CronService` with in-memory store + SQL persistence, three schedule types (At/Every/Cron) |
| `context_engine` | L3 | `ContextEngine` with 8-priority waterfall token allocation, SHA-256 caching |
| `tools` | L4 | 20+ native tools (filesystem, web search, calculator, etc.) |
| `feature-todo` | L4 | `TaskTool` with 26 actions, urgency/priority/age scoring, recursive subtasks |
| `feature-finance` | L4 | `FinanceTool` with 40+ actions across 8 sub-modules, live market data |
| `feature-notes` | L4 | Data layer complete (repo, models, migrations), tool not yet implemented |
| `feature-productivity` | L4 | Activity tracking, auto-focus detection, 0-100 productivity scoring |
| `feature-coaching` | L4 | Proactive coaching pipeline: signals → patterns → LLM reasoning → interventions |
| `plugin-runtime` | L4 | Extism-based WASM plugin host with permission model (network/storage/agent) |
| `channels` | L5 | 6 platform integrations: Telegram, Discord, Slack, WhatsApp, Email, QQ |
| `agent` | L5 | `AgentRuntime`, `AgentLoop`, intent pipeline, ReAct engine, cost tracking, learning |
| `cognitive` | L5 | Three-tier cognitive memory (semantic/episodic/procedural), FSRS decay, consolidation |
| `cli` | L6 | Clap-based CLI: `serve`, `init`, `status`, `plugin` subcommands |
| `mcp` | L6 | MCP client (connects to external MCP servers) + MCP server (exposes klyntbot as MCP) |
| `app-core` | L7 | `AppCore` struct: transport-agnostic business logic, shared handlers |
| `desktop-shared` | L7 | 30+ typed IPC event constants + payload structs for Tauri frontend |
| `desktop` | L7 | Tauri 2 desktop app: 100+ commands, system tray, global shortcuts |
| `klyntbot` | L8 | Re-export facade + `main()` binary entry point |

### 1.4 Boot Sequence

```
main() → CLI parse → handle_serve(port) OR Tauri main()
  → config::load_with_env_overrides()
  → StoragePool::connect(&data_dir)          // WAL mode + FK pragma + migrations
  → providers::create_provider(&config)       // Primary + fallback with circuit breaker
  → MessageBus::new(100)                      // Inbound + outbound channels
  → CronService::new + start()               // Load persisted jobs, start timer loop
  → AgentLoop::builder().build().await        // Wire tools, context, cognitive, MCP
  → ChannelManager::new(config, bus)          // Initialize all enabled channels
  → tokio::spawn(agent_loop.run_with_rx(rx))  // Start message processing
  → tokio::spawn(channel_manager.start_all()) // Connect all channels
  → CancellationToken::cancelled()            // Wait for shutdown signal
```

### 1.5 App-Core + Thin Adapter Pattern

`AppCore` is transport-agnostic — it holds all shared state and business logic:

- **Desktop (Tauri):** `desktop/src/app_core.rs` calls `AppCore::init()`, then wires `EventChannels` receivers into Tauri's event system. Each of 100+ Tauri commands is a thin delegate to `AppCore` methods.
- **CLI serve mode:** Calls `AppCore::init()` and spawns the agent loop.
- **Dev HTTP server:** Delegates identically but calls `rh()` (discard entity updates) since there's no Tauri event handle.

---

## 2. Memory System Deep Dive

### 2.1 Architecture Overview

The system has **two parallel memory architectures**:

1. **Cognitive Memory System** (newer, structured) — `crates/cognitive/`
2. **Conversation Memory System** (older, vector-based) — `crates/storage/src/vector_store.rs`

### 2.2 Cognitive Memory: Three-Tier Model

Inspired by cognitive science, the system separates memory into three types:

#### Semantic Facts (What the system knows)
- **Structure:** Subject-Predicate-Object (SPO) triples
- **Storage:** `semantic_facts` table in SQLite
- **Fields:** `id`, `subject`, `predicate`, `object`, `source` (user_stated/inferred/observed), `confidence` (0.0-1.0), `importance` (0.0-1.0), `domain` (one of 6), `valid_from`/`valid_until` (bi-temporal), `recorded_at`/`superseded_at`, `superseded_by`, `stability` (FSRS), `access_count`
- **Bi-temporal:** Each fact tracks both when it was true in the world AND when it was known to the system
- **Example:** `("user", "prefers", "dark mode")` with `source=user_stated`, `confidence=0.95`

#### Episodic Memories (What happened)
- **Structure:** Timestamped events with importance scoring
- **Storage:** `episodic_memories` table in SQLite
- **Fields:** `id`, `session_key`, `timestamp`, `event_type`, `summary`, `participants`, `emotional_valence` (-1.0 to 1.0), `importance` (0.0-1.0), `context_tags`, `stability` (FSRS), `access_count`

#### Procedural Rules (How to behave)
- **Structure:** Learned behavioral rules with confidence and reinforcement
- **Storage:** `procedural_rules` table in SQLite
- **Fields:** `id`, `rule_text`, `trigger_pattern`, `action_pattern`, `source`, `confidence`, `signal_count`, `is_active`
- **Reinforcement:** `signal_count` increments on each confirmation

### 2.3 Memory Lifecycle

```
                     ┌─────────────────┐
                     │   Domain Event   │
                     └────────┬────────┘
                              │
                     ┌────────▼────────┐
                     │ Salience Filter  │
                     │ (Extract/Accum/  │
                     │  Discard)        │
                     └───┬─────────┬───┘
                         │         │
              ┌──────────▼──┐  ┌──▼──────────┐
              │   Extract   │  │  Accumulate  │
              │ (immediate) │  │  (buffered)  │
              └──────┬──────┘  └──────┬───────┘
                     │                │
                     │     ≥5 occurrences
                     │     ≥3 distinct days
                     │                │
              ┌──────▼────────────────▼──────┐
              │   LLM Extraction Handler     │
              │   (extract facts from text)  │
              └──────────────┬───────────────┘
                             │
              ┌──────────────▼───────────────┐
              │   Consolidation (Mem0-style)  │
              │   Find existing (subj, pred)  │
              │   → ADD / UPDATE / DELETE /   │
              │     NOOP                      │
              └──────────────┬───────────────┘
                             │
              ┌──────────────▼───────────────┐
              │   SemanticFactRepo.upsert()   │
              │   (SQLite)                    │
              └──────────────────────────────┘
```

### 2.4 Memory Retrieval & Scoring (FSRS)

The system uses **Free Spaced Repetition Scheduler (FSRS)** for memory relevance:

```
Retrievability = exp(ln(0.9) × elapsed_days / stability)
```

When a memory is accessed, stability increases:
```
S_new = S + ln(1 + S)    // Diminishing returns
```

**5-Factor Relevance Formula:**
```
score = 0.30 × semantic_similarity    // (currently hardcoded 0.5 — gap!)
      + 0.20 × retrievability         // FSRS decay
      + 0.15 × importance             // User-stated or inferred
      + 0.10 × access_frequency       // Normalized 0-1
      + 0.25 × situational_boost      // Context match (largest non-semantic weight)
```

### 2.5 Memory Maintenance

| Process | Frequency | Action |
|---------|-----------|--------|
| **Compaction** | Daily | Archive superseded facts >90 days; delete episodic memories >90 days with <2 accesses; enforce 10,000 active fact cap |
| **Weekly Reflection** | Weekly | LLM summarizes last 7 days of episodic memories + user model → new facts + procedural rules |
| **Stability Update** | Per-retrieval | Each accessed fact gets `S_new = S + ln(1 + S)` |
| **Accumulator Promotion** | Continuous | Buffered observations promoted after ≥5 occurrences across ≥3 days |

### 2.6 Conversation Memory (Vector-Based)

Separate from cognitive memory, this system embeds conversations for retrieval:

- **Model:** `paraphrase-multilingual-MiniLM-L12-v2` (384 dimensions, ~420MB)
- **Storage:** LanceDB with three tables: `todo_embeddings`, `conv_embeddings`, `memory_note_embeddings`
- **Retrieval:** ANN nearest-neighbor with cosine similarity and configurable threshold
- **Injection:** Retrieved memories added to LLM context at `Priority::RetrievedMemory`

### 2.7 Context Assembly (Token Budget Management)

The `ContextEngine` allocates tokens using an 8-priority waterfall:

```
Priority 1: Identity (agent name, core personality)
Priority 2: Bootstrap (system instructions)
Priority 3: Learning (behavioral adaptations)
Priority 4: Area (current domain context)
Priority 5: Todo (active tasks)
Priority 6: Agent (matched agent profile + skills)
Priority 7: Persona (user model from cognitive memory)
Priority 8: Page (retrieved conversation memory)
```

Each priority gets allocated tokens from the remaining budget. Context is cached via SHA-256 hash of inputs with 60-second TTL.

### 2.8 User Model

The `CognitiveContextSource` builds a `UserModel` from semantic facts across 6 domains:
- preferences, background, goals, relationships, routines, personality

This is formatted as Markdown and injected into the system prompt at priority 60 with a 60-second cache.

---

## 3. Intelligence & Agent Runtime

### 3.1 Agent Runtime Pipeline

```
InboundMessage
  │
  ▼
AgentLoop::process_message
  │
  ├─ SessionManager::get_or_create (DashMap + SQLite)
  │
  ▼
AgentRuntime::process_message (10-step pipeline)
  │
  ├─ Step 1:  AgentManager::match_agent         // keyword trigger scoring
  ├─ Step 2:  Write active_profile               // RwLock<Option<Arc<AgentProfile>>>
  ├─ Step 3:  Filter MCP tools by profile        // allowlist enforcement
  ├─ Step 4:  IntentAnalyzer::analyze            // heuristics → LLM classifier
  ├─ Step 5:  ConfidenceEvaluator check          // clarify if below threshold
  ├─ Step 6:  ContextEngine::assemble            // token budget allocation
  ├─ Step 7:  Filter tools + inject delegation   // profile-based tool access
  ├─ Step 8:  ExecutionRouter::execute           // Direct or Reactive mode
  ├─ Step 9:  ResponseValidator::validate        // empty/overlong detection
  └─ Step 10: CostTracker + StrategyRepo + InteractionRecorder
```

### 3.2 Two-Stage Intent Classification

**Stage 1: Heuristics (zero-cost)**
- Greeting patterns → Direct mode
- Task management keywords → Reactive with task tools
- Question patterns → Direct with maybe-tools
- Complexity scoring (0-7 based on tool_calls, deps, risk, state, retries)

**Stage 2: LLM Classifier (fallback)**
- Lightweight LLM JSON call for ambiguous messages
- Returns `IntentAnalysis` with strategy, complexity, and tool list
- Cached with 60s TTL for strategy context

### 3.3 Execution Modes

**Direct Mode:** Single LLM call, no tools. Used for simple queries, greetings, and factual questions.

**Reactive Mode (ReAct Loop):**
```
for iteration in 1..=max_iterations:
    LLM call with tools →
    ├─ FinalResponse      → return (done)
    ├─ FabricatedResponse → inject force-tool prompt, retry (max 2)
    ├─ ToolsExecuted      → parallel execution → observe → continue
    └─ EmptyResponse      → continue

    // If max iterations reached:
    → Synthesize (one final LLM call with no tools)
```

**Iteration Budget Formula:**
```
max_iterations = min(max(estimated_tool_calls × 3, 10) + 5, 30)
```

### 3.4 Multi-Agent System

Five built-in agent profiles loaded via `include_str!` from `agents/` directory:

| Agent | Triggers | Capabilities |
|-------|----------|-------------|
| **general** | Default fallback, orchestrator | All tools, delegation to others |
| **task** | Task/project/planning keywords | Task tools + Google Calendar MCP |
| **finance** | Money/budget/investment keywords | Finance tools (40+ actions) |
| **automation** | Cron/schedule/automate keywords | Scheduling tools |
| **communication** | Email/message keywords | Channel-specific tools |

**Delegation:** General agent can delegate to specialized agents via `delegate(agent, query)`. Max delegation depth = 2 to prevent infinite chains.

### 3.5 Adaptive Learning System

```
Tool Execution
    │
    ▼
OutcomeRecorder (success/fail, duration, confidence band)
    │
    ▼
LearningAnalyzer (hourly: aggregate stats, bucket by confidence range)
    │
    ▼
AdaptiveThresholds (adjust ≤±0.05/cycle, min 50 outcomes cold-start)
    │
    ▼
ConfidenceEvaluator (updated threshold → Proceed/Clarify/Skip)
```

### 3.6 LLM Provider Architecture

`ProviderManager` supports 14 providers with resilience:

| Feature | Implementation |
|---------|---------------|
| **Providers** | Anthropic, OpenAI, DeepSeek, Gemini, Zhipu, DashScope, Moonshot, MiniMax, vLLM, Groq, OpenRouter, AiHubMix, Ollama, xAI |
| **Circuit Breaker** | Opens after 5 consecutive failures, 60s reset |
| **Retry** | 3 attempts with exponential backoff (500ms → 1s → 2s) on rate limits |
| **Failover** | Primary → fallback provider routing |
| **Streaming** | Token-by-token streaming with `chat_stream()` |
| **Cost Tracking** | Per-model pricing table for 15+ models |

---

## 4. Integrated Subsystems & Services

### 4.1 Chat Platform Channels

| Channel | Protocol | Unique Features |
|---------|----------|----------------|
| **Telegram** | HTTP long-polling | Voice transcription (Groq), photo/document download, inline keyboards, typing indicators (4s auto-resend), reaction capture |
| **Discord** | Raw WebSocket (no serenity) | Custom `WsHandler` via `WebSocketManager`, Gateway event handling |
| **Slack** | Socket Mode WebSocket | Block Kit buttons, bot mention stripping, thread-aware replies, ACK envelopes |
| **WhatsApp** | WebSocket bridge to Node.js Baileys | Requires external `ws://localhost:3001` bridge |
| **Email** | IMAP polling + SMTP (lettre) | Consent-gated (`consent_granted` config flag), subject line parsing |
| **QQ** | WebSocket bridge | Similar architecture to WhatsApp |

**Shared patterns:**
- All implement the `Channel` trait (name/start/stop/send/is_allowed)
- WebSocket-based channels use shared `reconnect_loop` (5s delay)
- `ChannelFormatter` adapts Markdown → platform-specific format (HTML for Telegram, mrkdwn for Slack, plaintext for others)

### 4.2 Tool System

**Native tools (20+):** Filesystem, web search, calculator, memory notes, and more.

**Tool derive system:**
```rust
#[derive(Tool)]
#[tool(name = "web_search", description = "...", params = "WebSearchParams")]
struct WebSearchTool { /* deps */ }

// Multi-action tools:
#[tool_actions(name = "task", description = "...")]
impl TaskTool {
    #[action(name = "add", description = "...")]
    async fn add(&self, ctx: &RoutingContext, params: AddParams) -> Result<String> { ... }
}
```

### 4.3 Feature Packages

Each `feature-*` crate implements `FeaturePackage`:

| Feature | Actions | Highlights |
|---------|---------|------------|
| **feature-todo** | 26 | Recursive subtasks, dependency cycle detection (recursive CTE), semantic/hybrid search, recurrence templates, focus slots |
| **feature-finance** | 40+ | 8 sub-modules (accounts, transactions, budgets, investments, goals, reports, settings, health), live market data via `PriceService` |
| **feature-notes** | 0 (WIP) | Data layer complete (repo, models, migrations), tool not yet implemented |
| **feature-productivity** | ~10 | Activity tracking, auto-focus detection, distraction monitoring, 0-100 productivity score formula |
| **feature-coaching** | N/A | Proactive: signals → patterns → LLM reasoning → interventions → feedback tracking |

### 4.4 MCP (Model Context Protocol)

**Client:** `McpManager` connects to external MCP servers in parallel. Tool names sanitized to `mcp_{server}_{tool}`. Per-agent access control via `mcp_tools` allowlist. Supports stdio and HTTP transports with OAuth.

**Server:** `KlyntbotServerHandler` exposes klyntbot as an MCP server (currently only `get_status` tool).

### 4.5 WASM Plugin System

- Runtime: Extism 1.x
- Manifest: `klyntbot.plugin.json` per plugin
- Permissions: `network`, `storage`, `agent` (elevated if network/agent declared)
- Host functions: SQLite pool access, message bus sender
- Memory limit: Configurable via `sandbox_memory_mb`

### 4.6 Desktop Application (Tauri 2)

- 100+ Tauri commands across all domains
- `TauriEmitter` for streaming agent responses → `agent:content_chunk`, `agent:tool_start`, `agent:done`
- 30+ typed event constants in `desktop-shared`
- `TransparencyData` per message: usage, cost, timing, tool calls, memory accesses, skills, classification
- System tray integration, global keyboard shortcuts

### 4.7 Scheduling (CronService)

- In-memory store + SQL persistence (write-through cache)
- Three schedule types: `At` (one-shot), `Every` (interval), `Cron` (expression + timezone)
- `CancellationToken` for graceful shutdown
- `Notify` wake for immediate job processing on changes

---

## 5. Data Flow & Component Interactions

### 5.1 Message Processing Flow

```
┌─────────────┐     ┌──────────┐     ┌──────────────┐     ┌───────────┐
│   Channel   │────▶│ MessageBus│────▶│  AgentLoop   │────▶│ MessageBus│
│ (Telegram,  │     │ (inbound)│     │ (process_msg)│     │ (outbound)│
│  Discord..) │     └──────────┘     └──────┬───────┘     └─────┬─────┘
└─────────────┘                             │                   │
                                    ┌───────▼───────┐   ┌──────▼──────┐
                                    │ AgentRuntime   │   │  Channel    │
                                    │ (10-step pipe) │   │  Manager    │
                                    └───────┬───────┘   │  (dispatch) │
                                            │           └─────────────┘
                          ┌─────────────────┼─────────────────┐
                          │                 │                 │
                   ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐
                   │   Session   │  │   Context    │  │    LLM      │
                   │   Manager   │  │   Engine     │  │  Provider   │
                   └─────────────┘  └──────┬───────┘  └─────────────┘
                                           │
                          ┌────────────────┼────────────────┐
                          │                │                │
                   ┌──────▼──────┐  ┌──────▼──────┐  ┌─────▼───────┐
                   │  Cognitive  │  │   Vector     │  │  User Model │
                   │  Memory     │  │   Store      │  │  (6 domains)│
                   └─────────────┘  └─────────────┘  └─────────────┘
```

### 5.2 Cognitive Pipeline Flow

```
┌──────────────┐     ┌───────────┐     ┌─────────────┐     ┌──────────────┐
│ Domain Event │────▶│ Salience  │────▶│  Extraction  │────▶│Consolidation │
│ (bus)        │     │ Filter    │     │ (LLM-backed) │     │ (Mem0-style) │
└──────────────┘     └───────────┘     └─────────────┘     └──────┬───────┘
                                                                  │
    ┌─────────────────────────────────────────────────────────────┘
    │
    ▼
┌──────────────┐     ┌───────────┐     ┌─────────────┐
│ SemanticFact │────▶│   FSRS    │────▶│  User Model  │
│ Repo (SQL)   │     │  Scoring  │     │  (6 domains) │
└──────────────┘     └───────────┘     └──────┬───────┘
                                              │
                                       ┌──────▼───────┐
                                       │ System Prompt │
                                       │ (Priority 60) │
                                       └──────────────┘
```

### 5.3 Learning Feedback Loop

```
┌──────────────┐     ┌───────────────┐     ┌─────────────────┐
│ Tool Outcome │────▶│ OutcomeRecorder│────▶│  learning_       │
│ (success/    │     │ (privacy-safe) │     │  outcomes (SQL)  │
│  failure)    │     └───────────────┘     └────────┬────────┘
└──────────────┘                                    │
                                             hourly │
                                          ┌─────────▼────────┐
                                          │ LearningAnalyzer  │
                                          │ (bucket by conf.) │
                                          └─────────┬────────┘
                                                    │
                                          ┌─────────▼────────┐
                                          │ AdaptiveThresholds│
                                          │ (±0.05 max/cycle) │
                                          └─────────┬────────┘
                                                    │
                                          ┌─────────▼────────┐
                                          │ ConfidenceEval    │
                                          │ (AtomicU32 f32)   │
                                          └──────────────────┘
```

---

## 6. Technical Analysis

### 6.1 Strengths

1. **Layered architecture with strict dependency direction.** The 9-layer design prevents circular dependencies and enables independent crate testing. Each layer has a clear responsibility boundary.

2. **Cognitive memory model grounded in science.** The semantic/episodic/procedural split mirrors cognitive science. FSRS-based decay is more principled than simple TTL or LRU eviction. Bi-temporal tracking (world-time vs system-time) is enterprise-grade.

3. **Adaptive learning system.** The confidence → outcome → threshold adjustment loop is self-improving. Privacy-safe (no content stored in outcomes). Lock-free hot path via `AtomicU32`.

4. **Multi-provider resilience.** Circuit breaker, exponential backoff, primary→fallback failover. Support for 14 LLM providers means no vendor lock-in.

5. **Transport-agnostic core.** `AppCore` + thin adapter pattern means the same business logic runs in CLI, desktop, and dev server modes without duplication.

6. **Derive-based tool framework.** Tools are declarative and type-safe. Multi-action tools collapse complex APIs into single tools with action dispatch.

7. **Salience filtering.** Not every event triggers expensive LLM calls. The extract/accumulate/discard classification with promotion thresholds (≥5 occurrences, ≥3 days) is an elegant cost control.

8. **Feature package isolation.** Each feature owns its migrations, tools, config, and health check. Adding a new feature doesn't touch core code.

9. **Real-time cognitive pipeline.** Background consolidation service processes domain events in real-time, building user understanding incrementally rather than in batch.

10. **Rich desktop transparency.** `TransparencyData` captures full pipeline metadata (usage, cost, timing, tool calls, memory accesses, classification, delegation) per message.

### 6.2 Weaknesses

1. **Semantic similarity is hardcoded to 0.5.** `cognitive/retrieval.rs:L57` uses a placeholder value for the largest weight factor (0.30) in the relevance formula. Vector search exists but is not connected to cognitive retrieval. This significantly degrades memory relevance ranking.

2. **Two parallel memory systems.** `MemoryStore` (older vector-based) and `cognitive` (newer structured) coexist without cross-referencing. This creates confusion about which system is authoritative and doubles maintenance.

3. **CLI serve mode divergence.** `handle_serve` in `cli/src/serve.rs` doesn't use `AppCore::init()` — it manually wires everything, missing cognitive pipeline, coaching, and productivity features. ~400 lines of duplicated init code.

4. **No external observability.** Zero telemetry, no Prometheus metrics, no distributed tracing. Only local `tracing` crate logs. No dashboards beyond the desktop UI.

5. ~~**Unbounded analytics tables.**~~ Fixed — `Repos::cleanup_analytics()` runs daily via background task, enforcing retention: `strategy_records` (90d), `learning_outcomes` (30d), `interaction_log` (60d), `tool_usage` (90d), `enrichment_feedback` (90d).

6. **Coaching feedback is in-memory only.** `FeedbackTracker` loses all data on restart. Cannot analyze coaching effectiveness historically.

7. **Vector store fragility.** LanceDB upsert is delete-then-insert (not atomic). A crash between delete and add loses the vector. Predicate interpolation uses manual `'` escaping, not parameterized queries.

8. ~~**Conversation decay not implemented.**~~ Fixed — `ConversationEmbeddingStore.search_similar()` now applies `score × decay_factor^days_old` using the `created_at` timestamp from LanceDB results.

9. **`coaching_strategies` table orphaned.** Defined in migration SQL but has no Rust repo — never read or written.

10. ~~**Stale documentation.**~~ Fixed — CLAUDE.md no longer references 80% escalation. Code synthesizes at 100% of max_iterations. `escalation_count` field remains unused (always `0`) in `StrategyRecordRow`.

### 6.3 Design Decisions (Notable)

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| SQLite for all relational data | Single-file deployment, no external DB | No concurrent writes, VACUUM needed |
| LanceDB for vectors | Embedded vector store, no server | No managed ANN indexing, fragile upserts |
| 384-dim embeddings (MiniLM) | Fast inference, multilingual | Lower recall vs 768/1536-dim models |
| FSRS for memory decay | Proven SRS algorithm, principled | Complex to tune without user feedback |
| Salience filtering | Cost control for LLM extraction | May miss important one-off events |
| Single binary | Easy deployment | Monolithic, large compile times |
| Tauri 2 for desktop | Native perf, web UI flexibility | Platform-specific bugs, smaller ecosystem |
| WASM for plugins | Sandboxed, language-agnostic | Cold-start overhead, memory limits |
| Message bus (mpsc) | Decoupled channels ↔ agent | No back-pressure, no persistence |

### 6.4 Scalability Considerations

| Concern | Current State | At Scale |
|---------|--------------|----------|
| SQLite concurrency | WAL mode, single-writer | Bottleneck at >100 concurrent sessions |
| LanceDB indexing | IVF-PQ indexes auto-created at 256+ rows | Good for personal use; may need tuning at >100K vectors |
| Session memory | LRU eviction at 1000 | Good for personal use, not multi-tenant |
| MessageBus buffer | 100 messages | Queue overflow drops messages |
| Cognitive facts | 10K cap with compaction | Adequate for personal use |
| Token counting | tiktoken-rs (accurate) | `CharTokenCounter` fallback loses accuracy |
| Tool execution | Parallel with per-tool timeout | No global concurrency limit |

### 6.5 Reliability Considerations

| Concern | Status |
|---------|--------|
| Graceful shutdown | `CancellationToken` propagated to all tasks |
| Provider failover | Circuit breaker + fallback provider |
| Session persistence | SQL-backed with DashMap cache |
| Memory durability | SQL + WAL mode (crash-safe) |
| Vector durability | LanceDB (non-atomic upsert — gap) |
| Error propagation | `warn!` on metrics failures, never blocks response |
| Cron persistence | Write-through cache to SQL |

---

## 7. Comparison with Common AI Architectures

### 7.1 vs RAG-Based Systems (LangChain, LlamaIndex)

| Dimension | Klyntbot | Typical RAG |
|-----------|----------|-------------|
| **Memory model** | Three-tier cognitive (semantic/episodic/procedural) + vector store | Single vector store with chunked documents |
| **Memory lifecycle** | FSRS decay, consolidation, compaction, weekly reflection | Static embeddings, manual re-indexing |
| **Retrieval** | 5-factor scoring (semantic + retrievability + importance + frequency + situational) | Cosine similarity only |
| **User modeling** | Structured 6-domain user model injected into every prompt | None (stateless per query) |
| **Learning** | Adaptive confidence thresholds from tool outcomes | None |
| **Multi-provider** | 14 providers with circuit breaker and failover | Usually single provider |
| **Tool use** | Type-safe derive macros, 60+ tools, WASM plugins | String-based tool definitions |
| **Deployment** | Single binary with embedded SQLite | Python process + external vector DB |

**Assessment:** Klyntbot is significantly more sophisticated than typical RAG systems. The cognitive memory model and adaptive learning are distinguishing features. However, the semantic similarity gap (hardcoded 0.5) means the retrieval is currently weaker than a well-configured RAG system.

### 7.2 vs Agent-Based Architectures (AutoGPT, CrewAI, LangGraph)

| Dimension | Klyntbot | Agent Frameworks |
|-----------|----------|-----------------|
| **Agent routing** | Trigger-based profile matching + intent classification | Graph-based or task decomposition |
| **Multi-agent** | 5 specialized agents with delegation (max depth 2) | Arbitrary agent graphs, role assignment |
| **Planning** | No explicit planner — ReAct loop with iteration budget | Some have dedicated planning agents |
| **Reflection** | Weekly LLM reflection on episodic memories | Per-step reflection in some frameworks |
| **State** | Persistent sessions, cognitive memory, domain data | Usually ephemeral or simple checkpointing |
| **Tool framework** | Compile-time type-safe with derive macros | Runtime string-based definitions |
| **Deployment** | Compiled Rust binary, no runtime dependencies | Python processes, container orchestration |
| **Latency** | Native speed, no GC, async I/O | Python overhead, GC pauses |

**Assessment:** Klyntbot has stronger persistence and type safety than most agent frameworks, but lacks explicit planning capabilities. The ReAct loop without structured chain-of-thought is simpler than graph-based approaches. The multi-agent system is functional but less flexible than frameworks allowing arbitrary agent topologies.

### 7.3 vs Memory-Augmented AI Systems (MemGPT/Letta, Mem0)

| Dimension | Klyntbot | MemGPT/Letta | Mem0 |
|-----------|----------|-------------|------|
| **Memory tiers** | 3 (semantic, episodic, procedural) | 3 (core, recall, archival) | 1 (vector + graph) |
| **Memory decay** | FSRS spaced repetition | LRU archival | Recency weighting |
| **Consolidation** | LLM-based with Mem0-style ADD/UPDATE/DELETE | LLM self-edit of core memory | LLM extraction + dedup |
| **User model** | 6-domain structured model | Self-managed persona block | Graph-based relationships |
| **Context management** | 8-priority token budget waterfall | Virtual context manager | Simple truncation |
| **Retrieval** | 5-factor relevance formula | Recency + embedding search | Hybrid (vector + graph) |
| **Proactive behavior** | Coaching pipeline (signals → patterns → interventions) | None | None |
| **Self-improvement** | Adaptive confidence thresholds | Self-editing memory | None |

**Assessment:** Klyntbot's memory system is architecturally comparable to MemGPT/Letta and more structured than Mem0. The FSRS decay model is more principled than simple recency. The coaching pipeline (proactive behavior) is unique among these systems. However, the semantic similarity gap and lack of graph-based relationships are weaknesses.

---

## 8. Scoring & Evaluation

### Rating Scale: 1-10

| Dimension | Score | Justification |
|-----------|-------|---------------|
| **Architecture Design** | **8.5/10** | Excellent layered design with strict dependency flow. App-core + thin adapter pattern is clean. Feature package isolation is well-executed. Minor deductions for CLI serve divergence and some duplicated code. |
| **Memory System Quality** | **7.0/10** | Three-tier model is architecturally strong. FSRS decay and consolidation are principled. However, semantic similarity is hardcoded (major gap), two parallel memory systems create confusion, and coaching feedback isn't persisted. |
| **Scalability** | **5.5/10** | Designed as a personal AI agent — single-user SQLite is appropriate for that scope. Would require significant rework for multi-tenant: needs PostgreSQL, distributed vector store, message queue, and horizontal scaling. Adequate for its intended purpose. |
| **Observability** | **3.5/10** | Rich transparency data in desktop UI per message. Cost tracking, strategy recording, and learning analytics exist. But: no external metrics, no dashboards, no alerting, no distributed tracing, no health endpoints. Logging-only observability. |
| **Intelligence & Reasoning** | **7.0/10** | Two-stage intent classification is efficient. ReAct loop with fabricated-response detection and duplicate-call prevention is robust. Multi-agent delegation works. However, no structured chain-of-thought, no planning agent, no explicit reasoning steps. Direct mode silently fails on tool-requiring queries. |
| **User Understanding & Personalization** | **7.5/10** | 6-domain user model, procedural rules, emoji-reaction satisfaction, behavioral pattern detection. The system genuinely learns about users over time. Deductions for hardcoded semantic similarity reducing retrieval quality and for satisfaction rarely being collected outside chat. |
| **Maintainability** | **8.0/10** | Strong Rust type system prevents many bugs. Derive macros reduce boilerplate. Zero-clippy-warning policy. Good test infrastructure (ephemeral SQLite). Conventional commits. Minor deductions for 27-crate complexity and some undocumented code paths. |
| **Channel Coverage** | **8.0/10** | 6 platforms with unified `Channel` trait. Rich Telegram support. Shared reconnect and formatter patterns. Deductions for WhatsApp/QQ requiring external bridges, email being consent-gated, and no web chat channel. |
| **Tool Ecosystem** | **8.5/10** | 60+ tools across 5 feature packages. Type-safe derive macros. Multi-action dispatch. WASM plugin system. MCP client/server. Deduction only for feature-notes having no tools yet. |
| **Security** | **7.5/10** | `Secret<String>` for API keys. WASM sandbox with permissions. Allowlist-based channel access. HTML escaping in formatters. Per-agent tool access control. Deductions for manual SQL predicate escaping in vector store and no input sanitization framework. |

### Overall Score: **7.1/10**

### Radar Summary

```
Architecture    ████████░░  8.5
Memory          ███████░░░  7.0
Scalability     █████░░░░░  5.5
Observability   ███░░░░░░░  3.5
Intelligence    ███████░░░  7.0
Personalization ███████░░░  7.5
Maintainability ████████░░  8.0
Channels        ████████░░  8.0
Tools           ████████░░  8.5
Security        ███████░░░  7.5
```

---

## 9. Recommendations

### 9.1 Critical (Highest Impact)

#### R1: Connect vector search to cognitive retrieval
**Current:** `cognitive/retrieval.rs:L57` hardcodes `semantic_similarity = 0.5`
**Fix:** Embed semantic facts (SPO triples) into LanceDB on creation. When retrieving, embed the query and compute actual cosine similarity. This would make the 0.30 weight factor in the relevance formula functional.
**Impact:** Memory retrieval relevance improves dramatically. The entire FSRS scoring system becomes meaningful.

#### R2: Unify the two memory systems
**Current:** `MemoryStore` (vector-based diary) and `cognitive` (structured three-tier) coexist independently.
**Fix:** Migrate `MemoryStore` entries into `semantic_facts` with `source=user_stated`. Remove the parallel system. Use the cognitive retrieval pipeline for all memory operations.
**Impact:** Eliminates confusion, reduces maintenance, and strengthens the cognitive model.

#### R3: Migrate CLI serve to AppCore::init() SOLVED
**Current:** `cli/src/serve.rs` manually wires everything, missing cognitive/coaching/productivity features.
**Fix:** Replace the manual setup with a single `AppCore::init(None)` call, discarding `EventChannels` (or logging them).
**Impact:** Eliminates ~400 lines of duplicated code and ensures CLI mode has feature parity with desktop.

### 9.2 High Priority

#### R4: Add external observability
**Options:**
- Prometheus metrics endpoint (request latency, tool success rates, memory stats, LLM costs)
- OpenTelemetry integration for distributed tracing
- Health check HTTP endpoint
**Impact:** Enables monitoring, alerting, and performance analysis without the desktop UI.

#### R5: Persist coaching feedback to SQL — SOLVED (via R11)
**Current:** ~~`FeedbackTracker` is in-memory only — all coaching effectiveness data lost on restart.~~
**Fix:** `FeedbackTracker` now has `with_repo()`, `persist()`, and `load_from_db()` methods backed by the `coaching_strategies` table via `CoachingStrategyRepo`.
**Impact:** Coaching effectiveness data survives restarts.

#### ~~R6: Add retention policies for analytics tables~~ — SOLVED
**Fix:** Added `delete_older_than()` to `StrategyRepo` (90d), `OutcomeRepo` (30d), `InteractionLogRepo` (60d). `Repos::cleanup_analytics()` coordinates all tables including `tool_usage` (90d) and `enrichment_feedback` (90d). Runs daily via background task in `AppCore::init()`.

#### ~~R7: Implement conversation memory decay~~ — SOLVED
**Fix:** `ConversationEmbeddingStore.search_similar()` now applies `score × decay_factor^days_old` using `created_at` from LanceDB. `search_conv_embeddings` updated to return timestamps.

### 9.3 Medium Priority

#### ~~R8: Add structured chain-of-thought~~ — SOLVED
**Fix:** ReactiveEngine now injects a planning prompt for tasks with complexity score >= 5. The LLM generates a structured plan (parsed into `ExecutionPlan` steps) on iteration 1, then executes against it. Plan progress is tracked in the `Scratchpad`, emitted via `AgentEvent::PlanGenerated`/`PlanStepCompleted`, and included in synthesis prompts when max iterations are reached. Advisory only — the LLM is free to deviate.

#### ~~R9: Handle Direct mode tool-call overflow~~ — SOLVED
**Fix:** `DirectEngine` now returns `EngineResult::Escalate` instead of empty. `ExecutionRouter` catches escalation, clones original messages, and transparently retries with `ReactiveEngine` + actual tools. Usage from the failed Direct attempt is accumulated.

#### ~~R10: Create explicit ANN indexes in LanceDB~~ — SOLVED
**Fix:** Added `VectorStore::ensure_indexes(min_rows)` — creates IVF-PQ (cosine) indexes on tables with 256+ rows, called in background on boot via `AppCore::init()`.

### 9.4 Low Priority (Quality of Life)

| # | Recommendation |
|---|---------------|
| ~~R11~~ | ~~Remove orphaned `coaching_strategies` table or wire it to the coaching pipeline~~ — Done: wired `CoachingStrategyRepo` to `FeedbackTracker` with `persist()`/`load_from_db()` |
| ~~R12~~ | ~~Fix `learning_handler.rs` to pass through `suggested_threshold` from `AnalysisResult`~~ — Done: reads from `last_analysis.suggested_threshold` |
| ~~R13~~ | ~~Make vector store upsert atomic (begin transaction → delete → insert → commit)~~ — Done: reordered to insert-first-then-delete-old for crash safety |
| R14 | Add web chat channel for browser-based interaction without desktop app |
| ~~R15~~ | ~~Use dedicated cheaper model for intent classification (wire `classifier_provider` separately)~~ — Done: added `classifier_provider()` to `LlmProvider` trait, wired in `IntentAnalyzer` |
| ~~R16~~ | ~~Update CLAUDE.md to remove stale "80% escalation" documentation~~ — Done |
| ~~R17~~ | ~~Implement `feature-notes` tools (data layer is complete)~~ — Done: `NotesTool` with 10 actions |

---

## Appendix: Storage Schema Summary

### Core Tables (35 in 001_initial.sql)
`sessions`, `session_messages`, `areas`, `projects`, `actions`, `objectives`, `key_results`, `finance_accounts`, `finance_transactions`, `finance_budgets`, `finance_investments`, `finance_goals`, `finance_price_cache`, `cron_jobs`, `usage_records`, `agent_tasks`, `memory_notes`, `tags`, `action_tags`, `action_attachments`, `action_time_entries`, `action_dependencies`, `action_recurrence_templates`, `kr_actions`, `interaction_log`, `tool_usage`, `learning_outcomes`, `learning_state`, `behavioral_patterns`, `strategy_records`, `decision_log`, `enrichment_feedback`, `session_contexts`, `agent_task_runs`

### Cognitive Tables (5)
`semantic_facts`, `episodic_memories`, `procedural_rules`, `semantic_facts_archive`, `coaching_strategies`

### Feature Migration Tables
`_feature_migrations` (tracking), plus any tables added by feature crate migrations

### Vector Tables (LanceDB, 3)
`todo_embeddings`, `conv_embeddings`, `memory_note_embeddings`
