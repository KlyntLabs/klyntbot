# Klyntbot AI System — Comprehensive Architecture Analysis

> Generated: 2026-03-09 | Codebase: 26 Rust crates, ~115K LoC (Rust) + ~26K LoC (TypeScript)

---

## Table of Contents

1. [System Architecture Overview](#1-system-architecture-overview)
2. [Memory System Deep Dive](#2-memory-system-deep-dive)
3. [Agent Runtime & LLM Integration](#3-agent-runtime--llm-integration)
4. [All Integrated Subsystems & Services](#4-all-integrated-subsystems--services)
5. [Data Flow & Component Interactions](#5-data-flow--component-interactions)
6. [Technical Analysis: Strengths & Weaknesses](#6-technical-analysis-strengths--weaknesses)
7. [Comparison with Other AI Architectures](#7-comparison-with-other-ai-architectures)
8. [Scoring Framework & Evaluation](#8-scoring-framework--evaluation)
9. [Recommendations for Improvement](#9-recommendations-for-improvement)

---

## 1. System Architecture Overview

### What It Is

Klyntbot is a **personal AI agent** — a single Rust binary that connects 6+ chat platforms (Telegram, Discord, Slack, Email, Desktop, Web) to LLMs, with persistent memory, task/project management, finance tracking, productivity coaching, and a desktop Tauri app. All state lives in SQLite + LanceDB (vector store).

### Layered Architecture (9 Layers)

```
┌─────────────────────────────────────────────────────────┐
│  L8: klyntbot (re-export facade binary)                 │
├─────────────────────────────────────────────────────────┤
│  L7: app-core, desktop-shared, desktop (Tauri)          │
├─────────────────────────────────────────────────────────┤
│  L6: mcp (Model Context Protocol server/client)         │
├─────────────────────────────────────────────────────────┤
│  L5: channels, agent, cognitive                         │
│      Platform integrations, agent runtime, memory       │
├─────────────────────────────────────────────────────────┤
│  L4: tools, feature-todo, feature-finance,              │
│      feature-notes, feature-productivity,               │
│      feature-coaching, plugin-runtime                   │
├─────────────────────────────────────────────────────────┤
│  L3: providers, session, scheduling, context_engine     │
│      LLM clients, sessions, cron, token budgets         │
├─────────────────────────────────────────────────────────┤
│  L2: storage, domain                                    │
│      SQLite + LanceDB, OKR+PARA domain types            │
├─────────────────────────────────────────────────────────┤
│  L1: config, bus, tools-core, tools-core-macros         │
│      Configuration, event bus, tool derive system        │
├─────────────────────────────────────────────────────────┤
│  L0: common                                             │
│      KlyntbotError, MessageRole, ChatId, SessionKey     │
└─────────────────────────────────────────────────────────┘
```

**Dependencies flow strictly upward** — no circular references. Each layer only depends on layers below it. This is enforced by the Cargo workspace structure.

### Key Architectural Patterns

| Pattern | Where Used | Purpose |
|---------|-----------|---------|
| **Dependency Inversion** | `ExtractionHandler`, `ConsolidationHandler`, `ReflectionHandler`, `SpawnHandler`, `CronHandler` | Traits defined in lower layers (L3–L5), implemented in `agent` (L5). Avoids circular deps. |
| **Event-Driven Architecture** | `DomainEventBus`, `LearningEventBus` | Feature crates emit events; cognitive layer subscribes for asynchronous processing. |
| **Feature Packages** | `feature-*` crates | Self-contained domains with tools + migrations + config + health checks. |
| **Derive-Based Tools** | `#[derive(Tool)]`, `#[derive(ToolParams)]` | Declarative tool registration via proc macros. |
| **App-Core + Thin Adapters** | `app-core` → `desktop`, `dev-server` | Business logic in one place; platform adapters delegate to it. |
| **Re-Export Facade** | `klyntbot` root crate | Single import point: `use klyntbot::AgentLoop`, etc. |

---

## 2. Memory System Deep Dive

The memory system is the most sophisticated component. It implements a **multi-tier cognitive architecture** inspired by human memory research, Mem0, and spaced repetition (FSRS).

### 2.1 Memory Types (Three Tiers)

```
┌──────────────────────────────────────────────────────────┐
│                  SEMANTIC MEMORY                          │
│  Structured facts: Subject-Predicate-Object triples      │
│  "user.peak_hours = 10am-12pm"                           │
│  Bi-temporal: valid_from/valid_until + recorded_at        │
│  FSRS decay: stability, retrievability scoring            │
│  Stored: SQLite (semantic_facts) + LanceDB (384-dim)     │
├──────────────────────────────────────────────────────────┤
│                  EPISODIC MEMORY                          │
│  Event-based: "Focus session: 45min, quality 92%"        │
│  Time-stamped with importance scoring                     │
│  Stored: SQLite (episodic_memories)                       │
│  Compacted: deleted after 90 days if access_count < 2    │
├──────────────────────────────────────────────────────────┤
│                  PROCEDURAL MEMORY                        │
│  Learned rules: "User is more productive after exercise"  │
│  Confidence + signal_count based validation               │
│  Stored: SQLite (procedural_rules)                        │
│  Generated by weekly reflection cycle                     │
└──────────────────────────────────────────────────────────┘
```

Additionally, there is a **Conversation Recall** subsystem:
- Full conversation messages embedded as 384-dim vectors
- Stored in LanceDB `conv_embeddings` table
- Time-decayed search (half-life: 138 days → ~0.995/day decay)

### 2.2 Storage Architecture

**SQLite** (`{data_dir}/data.db`):
- `semantic_facts` — SPO triples with bi-temporal markers, FSRS stability, access tracking
- `episodic_memories` — event snapshots with importance and stability
- `procedural_rules` — learned behavioral rules per domain
- `accumulated_observations` — buffered low-salience events (persisted across restarts)
- `user_profile` — explicit user-stated facts (legacy L2 system)
- `behavioral_patterns` — observed interaction patterns (legacy L2 system)
- `agent_adaptations` — per-agent preferences learned from satisfaction signals

**LanceDB** (`{data_dir}/lance/`):
- `todo_embeddings` — task search vectors (384-dim, cosine distance)
- `conv_embeddings` — conversation message vectors (384-dim)
- `cognitive_fact_embeddings` — semantic fact vectors (384-dim, domain-filtered)

All vectors use **384-dimensional** embeddings via FastEmbed (`paraphrase-multilingual-MiniLM-L12-v2`).

### 2.3 Memory Lifecycle

```
  User Messages (multiple)
       │
       ▼
  DomainEventBus ──publish──▶ BackgroundConsolidationService
       │                              │
       │                    collect_batch (3s window, max 10)
       │                              │
       │                    classify_batch (salience)
       │                    ┌─────────┼──────────┐
       │                    ▼         │          ▼
       │                 Extract    Discard   Accumulate
       │                    │                    │
       │                    │              Buffer events
       │                    │              ≥5 + ≥3 days
       │                    │                    │
       │                    │◄── DLQ retries ◄───┤ Promote
       │                    │                    │
       │                    ▼                    │
       │           Batch ExtractionHandler       │
       │              (1 LLM call)               │
       │              ┌────┴────┐                │
       │              │         │                │
       │           Success   Fallback            │
       │              │      (heuristic)         │
       │              │         │                │
       │              │    Dead-Letter Queue     │
       │              │    (retry w/ backoff)    │
       │              │         │                │
       │              ▼         ▼                │
       │         ExtractedFact[]                 │
       │              │                          │
       │         prefetch_existing (join_all)    │
       │              │                          │
       │              ▼                          │
       │     Batch ConsolidationHandler ◄────────┘
       │        (1 LLM call)
       │        ┌─────┼─────┬─────┐
       │        ▼     ▼     ▼     ▼
       │       ADD  UPDATE DELETE NOOP
       │        │     │     │
       │        ▼     ▼     ▼
       │  execute_memory_ops (SQLite + LanceDB)
       │
       ▼
  Context Assembly (retrieval on next request)
```

### 2.4 FSRS Decay & Relevance Scoring

The system uses **FSRS (Free Spaced Repetition Scheduler)** for memory decay:

```
Retrievability R = exp(ln(0.9) × elapsed_days / stability)
```

- At `stability = S` days, recall probability is exactly 90%
- Stability increases on successful retrieval: `new = current + ln(1 + current).max(0.1)`
- Capped at `MAX_STABILITY = 30.0` to prevent runaway inflation

**Composite Relevance Score** (5 weighted factors):
```
score = semantic_similarity × 0.30
      + retrievability      × 0.20
      + importance          × 0.15
      + access_frequency    × 0.10
      + situational_boost   × 0.25
```

### 2.5 Salience Filtering

Events are classified into three tiers:

| Verdict | Trigger Examples | Processing |
|---------|-----------------|------------|
| **Extract** | User stated facts, corrections, chat turns, budget alerts, coaching feedback | Micro-batched (3s window) → batch LLM extraction → consolidation |
| **Accumulate** | Productivity scores, task completions, focus sessions, normal transactions | Buffered; promoted after ≥5 events across ≥3 days |
| **Discard** | (None currently — all events are either Extract or Accumulate) | Dropped |

### 2.6 Consolidation (Mem0-Style, Batch)

Fact candidates are processed in batches via `decide_batch`:

1. Concurrent `prefetch_existing` lookups via `join_all` for `(subject, predicate)` matches
2. Candidates with no similar facts → **ADD** directly (skip LLM)
3. Candidates with existing matches → single batch LLM call to decide:
   - **ADD** — new, non-conflicting fact
   - **UPDATE** — supersedes old fact (bi-temporal: `superseded_at`, `superseded_by`)
   - **DELETE** — old fact no longer valid
   - **NOOP** — duplicate, no action needed
4. `execute_memory_ops` applies all decisions to SQLite + LanceDB

### 2.7 Weekly Reflection Cycle

A scheduled LLM-powered reflection:
1. Loads episodic memories from the past 7 days
2. Loads the full UserModel (all semantic facts)
3. Loads active procedural rules
4. Calls an LLM to synthesize insights
5. Consolidates new/updated facts (with confidence threshold ≥ 0.7)
6. Creates/updates procedural rules
7. Stores the reflection itself as an episodic memory (stability = 5.0)

### 2.8 Memory Compaction

Daily background job:
- Archives superseded facts older than 90 days
- Deletes episodic memories older than 90 days with `access_count < 2`
- Enforces size budget: max 10,000 active facts; archives low-stability facts if exceeded

### 2.9 Context Injection (Two-Tier)

When assembling the LLM prompt, the `CognitiveContextSource` injects memory:

**Static Tier:** Top facts by `confidence × stability` across all domains (identity baseline). Limited to 10 per domain. Always included.

**Dynamic Tier:** Vector-searched facts relevant to the current user message. Uses the full FSRS retrieval pipeline with cosine similarity from LanceDB. Only included when a query is present.

The assembled prompt contains:
```
# User Understanding

## Identity
- user: name = Jayden

## Energy & Rhythms
- user: peak_hours = 10am-12pm

## Learned Patterns
### productivity
- User is more productive after morning exercise (confidence: 80%, signals: 7)

## Relevant Personal Context (for this conversation)
- user: editor = neovim (relevance: 0.82)
```

### 2.10 UserSituation (Derived World Model)

The cognitive layer computes a real-time `UserSituation` from multi-domain signals:

| Signal | Range | Source |
|--------|-------|--------|
| `energy_level` | 0.0–1.0 | Hours active, break timing, peak hour match |
| `focus_state` | 0.0–1.0 | Focus session active + quality, or context switches |
| `deadline_pressure` | 0.0–1.0 | Overdue tasks + due-within-24h tasks |
| `distraction_risk` | 0.0–1.0 | Recent distractions, context switch frequency |
| `coaching_receptivity` | 0.0–1.0 | Base intensity - dismissal penalty |
| `task_avoidance_detected` | bool | ≥3 deferrals + <40% productive ratio |

This feeds the coaching engine's decision about when and how to intervene.

---

## 3. Agent Runtime & LLM Integration

### 3.1 The 10-Step Agent Pipeline

The `AgentRuntime::process_message` implements a precise 10-step pipeline:

```
  User Message (from any channel)
       │
  ┌────┴────────────────────────────────────────────┐
  │ Step 1: Agent Matching                          │
  │   AgentManager::match_agent() scores triggers   │
  │   Weighted by trigger word count, falls back    │
  │   to "general" orchestrator                     │
  ├─────────────────────────────────────────────────┤
  │ Step 2: Active Profile Set                      │
  │   Write AgentProfile into Arc<RwLock<>>         │
  ├─────────────────────────────────────────────────┤
  │ Step 3: MCP Tool Filtering                      │
  │   Filter MCP tools by profile.mcp_tools         │
  ├─────────────────────────────────────────────────┤
  │ Step 4: Intent Classification                   │
  │   Two-stage: heuristics → LLM fallback          │
  │   (see Section 3.3)                             │
  ├─────────────────────────────────────────────────┤
  │ Step 5: Confidence Gate                         │
  │   Low confidence → downgrade to DirectResponse  │
  ├─────────────────────────────────────────────────┤
  │ Step 6: Context Assembly                        │
  │   ContextEngine.assemble() with budget mgmt     │
  ├─────────────────────────────────────────────────┤
  │ Step 7: Tool Filtering + Delegation Injection   │
  │   Restrict tools to profile.tools               │
  │   Inject delegate() if can_delegate_to set      │
  │   Add planning prompt if complexity ≥ 4         │
  ├─────────────────────────────────────────────────┤
  │ Step 8: Execution                               │
  │   ExecutionRouter → Direct or Reactive engine   │
  ├─────────────────────────────────────────────────┤
  │ Step 9: Response Validation                     │
  │   Strip <confidence> blocks, truncate long      │
  │   responses, detect system prompt leakage       │
  ├─────────────────────────────────────────────────┤
  │ Step 10: Cost + Strategy Recording              │
  │   UsageRepo, StrategyRepo, InteractionRecorder  │
  └─────────────────────────────────────────────────┘
       │
       ▼
  Response → Channel + DomainEventBus (async memory)
```

### 3.2 Five Agent Profiles

| Agent | Description | Tools | MCP Access | Triggers |
|-------|------------|-------|------------|----------|
| **general** | Orchestrator, greetings, delegation | ask_user, memory, web_search, web_fetch, grep, glob, read_file, spawn, learning | All (`*`) | Default |
| **task** | Task/project/notes management (OKR+PARA) | task, area, project, okr, notes, calendar | google-calendar | todo, task, project, plan, review, notes |
| **finance** | Budget/transaction management | finance, ask_user, memory | None | spend, budget, transaction, account |
| **automation** | Cron scheduling | cron, ask_user, memory | None | automate, remind, schedule |
| **communication** | Cross-platform messaging | ask_user, memory | None | send, message, email |

Each agent has an `AGENT.md` (YAML frontmatter) + `skills/` folder compiled via `include_str!`.

### 3.3 Two-Stage Intent Analysis

**Stage 1 — Heuristics** (zero-cost, in `analyze_heuristic`):

| Priority | Pattern | Result | Confidence |
|----------|---------|--------|------------|
| 1 | Greetings ("hi", "hello", < 20 chars) | Direct | 0.95 |
| 2 | Multi-agent triggers (2+ domains) | Fall through to LLM | — |
| 3 | Very short (< 20 chars, ≤ 4 words) | Direct | 0.85 |
| 4 | Task management keywords | Reactive | 0.90 |
| 5 | Direct questions ("what is", "explain") | Direct | 0.90 |
| 6 | Complex workflow ("create a plan") | Reactive (high budget) | 0.85 |
| 7 | Structural analysis via ComplexitySignals | Reactive | varies |

**ComplexitySignals** scores 0–7: `estimated_tool_calls ≥ 3` (+2), `sequential_deps` (+2), `failure_risk ≥ Medium` (+1), `requires_state_tracking` (+1), `requires_retries` (+1).

**Iteration budget formula:** `min(max(estimated_tool_calls × 3, 10) + 5, 30)` — floor 15, ceiling 30.

**Stage 2 — LLM Classifier** (only when heuristics inconclusive):
Sends message + tool names to a lightweight model, returns JSON with mode, complexity signals, needs_orchestration, confidence, and reasoning. Receives 30-day historical strategy performance context from `StrategyRepo`.

### 3.4 Execution Strategies

| Strategy | When Used | Behavior |
|----------|-----------|----------|
| `DirectResponse` | Simple questions, greetings | Single LLM call, no tools. If LLM returns tool calls anyway → **escalate** to Reactive |
| `ToolAssisted { max_iterations }` | Tool-needed requests | ReAct loop up to N iterations |
| `AutonomousTask { max_iterations }` | Complex multi-step tasks | Full autonomous execution |
| `Clarification { reason }` | Ambiguous requests | Ask user for more info (no memory retrieval) |

### 3.4 Context Engine

The `ContextEngine` orchestrates:
1. **Budget Allocation** — Partitions the context window across priorities:
   - SystemIdentity → ToolDefinitions → RetrievedMemory → RecentHistory → CompressedHistory
2. **History Compression** — Two modes:
   - Extractive: truncate old messages
   - Abstractive: LLM-summarized segments (via `SummaryProvider`)
3. **Memory Retrieval** — Embedding-based conversation recall + cognitive fact retrieval
4. **Caching** — LRU cache (8 entries) keyed by SHA-256 of request inputs

Budget allocation uses 85% of the context window for input, reserving 15% for the response.

### 3.6 LLM Providers

The `providers` crate abstracts LLM API calls via the `LlmProvider` trait. **11 provider specs** are registered:

| Provider | API Type | Notable Features |
|----------|----------|------------------|
| OpenRouter | Gateway | Auto-detected by `sk-or-` API key prefix |
| AiHubMix | Gateway | Auto-detected by API base URL |
| **Anthropic** | Native | Prompt caching, extended thinking, 200K context |
| **OpenAI** | OpenAI-compat | GPT-4, GPT-4o, GPT-4o-mini |
| DeepSeek | OpenAI-compat | `deepseek/` model prefix |
| Gemini | OpenAI-compat | `gemini/` model prefix |
| Zhipu AI | OpenAI-compat | GLM models |
| DashScope | OpenAI-compat | Qwen models |
| Moonshot | OpenAI-compat | Kimi models |
| MiniMax | OpenAI-compat | |
| vLLM/local | OpenAI-compat | Local inference |
| Groq | OpenAI-compat | Fast inference |

**`ProviderManager`** wraps providers with:
- **Circuit breaker**: opens after 5 consecutive failures, auto-resets after 60s
- **Retry with exponential backoff**: rate-limit errors retry 3× (500ms → 1s → 2s)
- **Failover**: primary failure → automatic fallback to secondary provider
- **Dedicated classifier provider**: cheap/fast model for intent classification

Configuration via `config.json` with `Secret<String>` for API keys.

### 3.6 Session Management

Sessions are keyed by `SessionKey` (channel + chat ID). The `session` crate handles:
- Conversation history persistence (SQLite)
- Session context metadata
- Cross-session continuity

### 3.8 Cost Tracking

The `CostTracker` monitors per-request token usage with a static pricing table:

| Model | Input ($/MTok) | Output ($/MTok) |
|-------|----------------|-----------------|
| claude-opus-4 | $15.00 | $75.00 |
| claude-sonnet-4 | $3.00 | $15.00 |
| gpt-4o | $2.50 | $10.00 |
| gpt-4o-mini | $0.15 | $0.60 |
| deepseek-chat | $0.27 | $1.10 |

Cache read/write tokens are separately priced. `UsageReport` aggregates: total requests, tokens, cost, by-model breakdown, and by-day cost series.

### 3.9 Feedback & Learning Loop

The system has a multi-signal learning loop:

1. **Reaction-based feedback**: Emoji reactions map to satisfaction scores (👍❤️🎉→1.0, 👎😕→0.0), persisted to the most recent `StrategyRecord`
2. **Strategy records**: After each request, records predicted vs. actual strategy, latency, usage, and satisfaction. Feeds the LLM classifier's historical context (30-day summaries)
3. **InteractionRecorder**: Logs `(agent, tools[], channel, latency)` per interaction, feeds `BehavioralPatternRepo`
4. **OutcomeRecorder**: Per-tool success/failure, latency, error category — feeds `AgentAdaptationRepo`
5. **ConfidenceEvaluator**: Holds an `AtomicU32` threshold updated by `LearningService` via `LearningEventBus::ThresholdChanged`

### 3.10 ReAct Loop Details

The Reactive engine has several defensive mechanisms:

- **Fabricated tool response detection**: Heuristic checks for LLMs that return fake tool results as text (fake IDs, task-creation phrases). Triggers a force-retry prompt.
- **Tool deduplication**: Hash-based (`name + JSON args hash`) — duplicate calls within a request are blocked with synthetic "already called" results
- **Parallel tool execution**: `tokio::join_all` with semaphore (`MAX_CONCURRENT_TOOLS = 10`), per-tool timeout (600s for interactive tools like `ask_user`)
- **Planning injection**: For complexity ≥ 4, iteration 1 generates an execution plan; subsequent iterations execute it
- **Synthesis at max iterations**: If the loop doesn't converge, a final synthesis prompt is injected with **no tools** (forces text response)

---

## 4. All Integrated Subsystems & Services

### 4.1 Tool Framework

The tool system uses a two-level abstraction with proc-macro code generation:

- **`ToolExecute<Params>`** — typed interface tool authors implement
- **`Tool`** — untyped runtime interface (`execute(args: Value, ctx: &RoutingContext)`)
- **`#[derive(Tool)]`** macro bridges them: deserializes `Value` → typed `Params` → dispatches to `ToolExecute`
- **`#[derive(ToolParams)]`** generates JSON Schema from Rust types + doc comments
- **`#[tool_actions]`** + `#[derive(ActionParams)]` for multi-action tools (e.g., finance tool with 40+ actions)

All tools (built-in Rust, WASM plugins, MCP remote) register as `DynTool = Arc<dyn Tool>` — the registry sees no difference. The `ToolRegistry` uses a `prepare()`/`execute()` split to prevent deadlocks when the `DelegationTool` re-enters the registry during execution.

Permission levels: `ReadOnly` → `Standard` → `Elevated` → `Admin`, gated per-channel.

### 4.2 Feature Packages

| Feature | Crate | Tools | Actions | Capabilities |
|---------|-------|-------|---------|-------------|
| **Todo/Tasks** | `feature-todo` | task, area, project, okr | ~30 | OKR+PARA framework, RRULE recurrence, task CRUD, dependencies, time entries |
| **Finance** | `feature-finance` | finance | 40+ | Accounts, transactions, budgets, portfolios, investments, goals, liabilities, multi-currency, exchange rates, net worth |
| **Notes** | `feature-notes` | notes | ~15 | Notebooks, notes, tagging, linking, entity mentions, versioning (max 50 versions, 5min cooldown) |
| **Productivity** | `feature-productivity` | productivity | ~20 | Focus sessions, activity tracking, distraction interception, auto-focus detection, daily aggregation, productivity insights, nudge service, project detection |
| **Coaching** | `feature-coaching` | *(none — pure reactive)* | — | Signal accumulation, pattern detection, intervention routing, feedback tracking. **Not a FeaturePackage** — observes DomainEventBus without providing tools. `consecutive_coaching_ignores` atomic suppresses delivery after 2 dismissals. |

### 4.3 Built-in Tools (non-feature)

| Tool | Permission | Notable Capability |
|------|-----------|-------------------|
| `web_search` | Standard | Brave Search API, configurable max results |
| `web_fetch` | Standard | HTML→text conversion, 50K char truncation |
| `ask_user` | Standard | Three paths: CLI (mpsc), platform-native (Telegram keyboards), fallback text |
| `memory` | Standard | RRF (Reciprocal Rank Fusion) hybrid search — merges keyword SQL + LanceDB semantic results |
| `delegate` | Standard | Cross-agent delegation, max depth 2, 120s timeout |
| `spawn` | Admin | Background agent tasks (general/research/analyst profiles) |
| `cron` | Standard | Add/list/remove scheduled jobs |
| `okr` | Standard | 11 actions in dotted namespaces (objective.create, kr.update_metric, etc.) |
| `browser` | Elevated | 13 actions, three trust levels (Full/Autonomous/Strict), write-guarded actions require `ask_user` confirmation |
| File tools | ReadOnly/Elevated | read_file, list_dir, write_file, edit_file with allowed-directory enforcement |

### 4.4 Chat Channels

| Channel | Protocol | Implementation | Notable |
|---------|----------|---------------|---------|
| **Telegram** | Bot API | Raw HTTP long polling (no teloxide) | Voice transcription via Groq Whisper, inline keyboards for `ask_user`, typing indicator manager |
| **Discord** | Gateway WebSocket | Raw tokio-tungstenite (no serenity) | Manual heartbeat, sequence tracking, session resume |
| **Slack** | Socket Mode | WebSocket-based | No public HTTP endpoint needed |
| **Email** | IMAP/SMTP | Behind `email` feature flag | Optional deps gated by Cargo feature |
| **Desktop** | Tauri IPC | Commands + events | Entity update emission, streaming via event channels |
| **Web** | HTTP/Axum | Dev server on :3456 | Debug builds only, delegates to same `AppCore` methods |

All channels implement the `Channel` trait with auto-reconnect via `reconnect_loop()` (5s retry delay). Allowlists support compound IDs (`"123456|username"` split on `|`).

### 4.5 External Integrations

**MCP (Model Context Protocol)** — dual-mode:
- **Client** (`McpManager`): connects to external MCP servers (Stdio subprocess or HTTP transport). Parallel startup via `JoinSet`. Tool names: `mcp_{server}_{tool}` (64-char limit with hash suffix). Per-server `allow_tools`/`deny_tools` filtering. Hot-reload via `reconnect_server()`/`disconnect_all()`.
- **Server** (`KlyntbotServerHandler`): intentionally narrow API surface — currently only `get_status` exposed to external agents.

**WASM Plugins** (`plugin-runtime`):
- Loaded from `~/.klyntbot/plugins/*/klyntbot.plugin.json` + `plugin.wasm`
- Extism sandbox with configurable memory limit
- Permissions: `Network`, `Storage`, `Agent` → maps to tool permission levels
- Each plugin shares one `Arc<Mutex<extism::Plugin>>` (serialized execution)
- Implements `FeaturePackage` with migrations, health checks, default config

### 4.6 Desktop Application (Tauri 2)

Three-layer separation:
1. **`app-core`** — pure Rust business logic, no transport dependency. Contains `AppCore` state with all repos, agent loop, bus, persona manager, config, optional productivity/cognitive subsystems. `HandlerResult<T> = Result<(T, Vec<EntityUpdate>), ApiError>` is the canonical mutation return type.
2. **`desktop/src/app_core.rs`** — Tauri event wiring. `wire_event_channels()` connects MPSC receivers to `app.emit()` for: auto-focus events, dashboard ticks, nudges, coaching interventions (also shows tray popup), domain events (salience-filtered), pipeline events.
3. **`desktop/src/commands/*.rs`** — thin Tauri command adapters. Query commands passthrough; mutation commands call `emit_updates(&app, &updates)` after core handler returns.

**Windows:** Launcher (centered, dismiss-on-blur), Tray (positioned below tray icon), Main (hide-on-close, macOS `ActivationPolicy::Accessory`).

**Shortcuts:** `Alt+Space` → toggle launcher, `Alt+Shift+Space` → toggle tray.

**Frontend:** React + TypeScript + Tailwind v4 + Vite. Biome 2.0 for lint/format.

### 4.5 Domain Model (OKR + PARA)

```
OKR Framework:
  Objectives → Key Results → Actions (Tasks)

PARA Framework:
  Projects (active with deadlines)
  Areas (ongoing responsibilities)
  Resources (reference material)
  Archives (completed/inactive)
```

### 4.6 Scheduling

The `scheduling` crate provides:
- Cron-based job scheduling
- Persistent cron definitions (SQLite)
- Injection via `CronHandler` trait (dependency inversion)

### 4.7 Message Bus

Two event buses:
1. **MessageBus** — Channel ↔ Agent communication (inbound/outbound messages)
2. **DomainEventBus** — Cross-feature domain events (tokio broadcast, 16+ event types)
3. **LearningEventBus** — Learning system signals

---

## 5. Data Flow & Component Interactions

### 5.1 Request Flow (Channel → Agent → Response)

```
Channel (Telegram/Discord/etc.)
    │
    ├─ InboundMessage ──▶ MessageBus
    │                         │
    │                         ▼
    │                    AgentRuntime
    │                         │
    │                    ┌────┴────┐
    │                    │         │
    │               IntentAnalyzer │
    │                    │         │
    │               Agent Profile  │
    │               Selection      │
    │                    │         │
    │               ContextEngine  │
    │               ┌────┤         │
    │               │    │         │
    │          MemoryRetriever     │
    │          CognitiveContext    │
    │          HistoryCompression  │
    │               │    │         │
    │               └────┤         │
    │                    │         │
    │              LLM Provider    │
    │              (API Call)      │
    │                    │         │
    │              Tool Execution  │
    │              (if reactive)   │
    │                    │         │
    │              CostTracker     │
    │                    │         │
    │                    ▼         │
    │              OutboundMessage │
    │                    │         │
    ├─ OutboundMessage ◀─┘         │
    │                              │
    ▼                              ▼
Channel (sends response)    DomainEventBus
                                   │
                        BackgroundConsolidation
                        (async memory processing)
```

### 5.2 Memory Processing Flow (Async, Micro-Batch)

```
Feature Crate ──emit──▶ DomainEventBus
                              │
                    BackgroundConsolidationService
                              │
                     collect_batch (3s / max 10)
                              │
                     classify_batch (salience)
                    ┌─────┬───┴───┬──────┐
                    │     │       │      │
                 Extract  Accumulate  Discard
                    │     │              │
                    │     Buffer         (drop)
                    │     ≥5 + ≥3d → Promote
                    │     │
                    │◄────┘ + DLQ retries (self-healing)
                    │
                    ▼
             Batch ExtractionHandler (1 LLM call)
                    │
               ┌────┴────┐
            Success    Fallback → Dead-Letter Queue
               │       (heuristic)   (linear backoff,
               │                      max 3 retries)
               ▼
        ExtractedFact[] → to_semantic_fact()
                    │
             prefetch_existing (concurrent join_all)
                    │
                    ▼
        Batch ConsolidationHandler (1 LLM call)
             (no-existing → direct ADD, skip LLM)
                    │
               ┌────┼────┬────┐
               ADD UPDATE DEL  NOOP
               │    │    │
               ▼    ▼    ▼
         execute_memory_ops ──▶ SQLite + LanceDB
```

---

## 6. Technical Analysis: Strengths & Weaknesses

### 6.1 Strengths

**Architecture:**
- **Strict layering** — 9-layer hierarchy with enforced dependency direction prevents spaghetti dependencies
- **Dependency inversion** via traits — clean separation between definition and implementation (e.g., `ExtractionHandler` defined in L3, implemented in L5)
- **Feature packages** — domain isolation with self-contained migrations, making it easy to add new domains
- **Event-driven cognitive processing** — non-blocking memory updates via `DomainEventBus` + background tasks

**Memory System:**
- **Three-tier memory** (semantic/episodic/procedural) mirrors neuroscience models and provides a rich foundation for personalization
- **FSRS decay** is mathematically grounded — based on peer-reviewed spaced repetition research
- **Bi-temporal semantic facts** (valid_from/valid_until + recorded_at/superseded_at) enable time-travel queries and fact versioning
- **Mem0-style batch consolidation** with LLM-driven ADD/UPDATE/DELETE/NOOP prevents memory bloat and handles contradictions. Micro-batch pipeline (3s window) reduces N+N LLM calls to 1+1 per batch
- **Two-tier context injection** (static identity + dynamic query-relevant) balances always-on personalization with query-specific relevance
- **Compaction system** with configurable retention policies prevents unbounded growth
- **Accumulated observation promotion** (≥5 events across ≥3 days) filters noise while surfacing genuine patterns
- **Dead-letter queue** with self-healing retry ensures no observations are lost on LLM failure (linear backoff, max 3 retries, piggyback drain on healthy batches)

**Engineering:**
- **456 Rust source files, ~115K LoC** — substantial but well-organized
- **Zero-clippy-warning policy** enforced
- **Comprehensive test suite** — every module has inline tests with mocks
- **Ephemeral SQLite for tests** — no external DB setup needed
- **Proc-macro tool system** — declarative, type-safe tool definitions

### 6.2 Weaknesses

**Memory System:**
- **No vector index until enough rows** — IVF-PQ indexing requires a minimum row count, meaning early queries do brute-force scans (acceptable for personal use, not scalable)
- **No per-user FSRS calibration** — FSRS parameters are configurable globally but not automatically tuned per-user based on retrieval outcomes
- **Text-only embeddings** — no support for image/audio memory
- **Conversation recall is separate from cognitive facts** — two parallel vector search paths that don't share a unified relevance model *(design spec written, implementation planned — see `docs/superpowers/specs/2026-03-11-unified-memory-retrieval-design.md`)*
- **Legacy L2 learning tables coexist with L5 cognitive system** — `user_profile` and `agent_adaptations` are zombie tables (never written in production, only read for transparency events). `behavioral_patterns` is actively computed by `PatternAnalyzer` but overlaps with L5 procedural rules. `interaction_log` is actively used and has no L5 equivalent. Dual authority creates confusion about source of truth for user understanding.

**Architecture:**
- **Single-binary monolith** — while well-layered, all 26 crates compile into one binary. No microservice boundaries for independent scaling.
- **SQLite single-writer limitation** — concurrent writes are serialized; fine for personal use but a scalability ceiling
- **No distributed event processing** — `tokio::broadcast` is in-process only
- **LLM-dependent consolidation** — every memory write requires an LLM call when similar facts exist, adding latency and cost (mitigated: candidates with no existing matches now bypass LLM with direct ADD)

**Observability:**
- Existing `tracing` + `PipelineEvent` SSE stream is sufficient for a personal local app. No need for Prometheus/OpenTelemetry.

**Evaluation & Intelligence:**
- **No explicit intelligence scoring** — there is no mechanism to evaluate the AI's intelligence level
- **No explicit user-understanding scoring** — `UserModel.active_fact_count()` and `non_empty_domain_count()` exist but aren't used as quality metrics
- **Confidence calibration is placeholder** — `confidence_bits` in `CognitiveContextSource` exists but isn't dynamically updated based on outcomes
- **No response quality feedback loop** — coaching has `FeedbackResponse` (Helpful/Dismissed/StopSuggesting) but no general response quality signal
- **No hallucination detection** — facts extracted from conversation are trusted without verification

---

## 7. Comparison with Other AI Architectures

### 7.1 vs. RAG-Based Systems (e.g., LangChain + Pinecone)

| Dimension | Klyntbot | Typical RAG System |
|-----------|----------|--------------------|
| **Memory model** | Three-tier (semantic/episodic/procedural) + FSRS decay | Flat document chunks + vector similarity |
| **Memory evolution** | LLM-driven consolidation, supersession, compaction | Overwrite/append only; no conflict resolution |
| **Personalization** | Structured UserModel injected into every prompt | Usually no persistent user model |
| **Temporal awareness** | Bi-temporal facts, time-decayed recall | Typically timestamp metadata only |
| **Retrieval scoring** | 5-factor composite (similarity + decay + importance + frequency + situation) | Single-factor (cosine similarity) |
| **Cost** | Batched LLM calls (1+1 per micro-batch window instead of N+N per event) | Fewer LLM calls (embed + retrieve only) |

**Verdict:** Klyntbot's memory system is significantly more sophisticated than standard RAG. The tradeoff is higher LLM cost per memory operation.

### 7.2 vs. Agent-Based Architectures (e.g., AutoGPT, CrewAI)

| Dimension | Klyntbot | AutoGPT / CrewAI |
|-----------|----------|-------------------|
| **Agent specialization** | 5 built-in profiles with skill libraries | Dynamic agent creation |
| **Execution model** | ReAct loop with intent-based routing | ReAct / Plan-Execute |
| **Memory persistence** | Full FSRS-scored persistent memory | Typically session-only or flat file |
| **Multi-agent coordination** | Delegation model (general → specialist) | Parallel agent execution |
| **Tool system** | Compile-time type-safe (Rust proc macros) | Runtime dynamic (Python) |
| **Production readiness** | Single compiled binary, multi-platform channels | Usually demo-grade |

**Verdict:** Klyntbot is more production-ready and has deeper memory, but less flexible agent spawning than research-oriented frameworks.

### 7.3 vs. Memory-Augmented AI Systems (e.g., Mem0, MemGPT)

| Dimension | Klyntbot | Mem0 | MemGPT |
|-----------|----------|------|--------|
| **Memory types** | Semantic + Episodic + Procedural | Key-value facts | Hierarchical (core/archival/recall) |
| **Consolidation** | Batch LLM-driven ADD/UPDATE/DELETE/NOOP (Mem0-inspired), smart ADD bypass | Graph-based | Edit-based |
| **Decay model** | FSRS (spaced repetition) | None | Context-based eviction |
| **Observation filtering** | Salience-based (Extract/Accumulate/Discard) | All messages processed | All messages processed |
| **User model** | Structured `UserModel` with domain-organized facts | Flat memory store | Persona blocks |
| **Situation awareness** | Computed `UserSituation` (energy, focus, pressure) | None | None |
| **Weekly reflection** | LLM-powered cross-domain pattern synthesis | None | None |

**Verdict:** Klyntbot's cognitive system is the most complete of the three, combining Mem0's consolidation approach with FSRS decay, multi-tier salience filtering, and periodic reflection. The `UserSituation` computation for coaching-aware memory retrieval has no equivalent in other systems.

---

## 8. Scoring Framework & Evaluation

### Rating Scale

| Score | Meaning |
|-------|---------|
| 9–10 | Industry-leading, exceptional |
| 7–8 | Strong, above average |
| 5–6 | Adequate, room for improvement |
| 3–4 | Below average, significant gaps |
| 1–2 | Minimal, needs fundamental rethinking |

### Scores

| Dimension | Score | Rationale |
|-----------|-------|-----------|
| **Architecture Design** | **8/10** | Strict 9-layer hierarchy with dependency inversion is excellent. Single-binary monolith limits scalability but is appropriate for a personal AI. Feature packages are well-isolated. Loses points for SQLite single-writer limitation and lack of distributed event processing. |
| **Memory System Quality** | **9/10** | Best-in-class for a personal AI agent. Three-tier cognitive model, FSRS decay, Mem0-style batch consolidation, bi-temporal facts, micro-batch pipeline (1+1 LLM calls per window), dead-letter queue with self-healing retry, salience filtering, weekly reflection. Loses a point for separate conversation recall paths. |
| **Scalability** | **5/10** | Designed for single-user personal AI — SQLite, in-process event bus, single binary. Would need fundamental changes for multi-user. LanceDB vector search is brute-force until enough rows for indexing. Adequate for intended use case, but limited beyond it. |
| **Observability** | **N/A** | Not scored — `tracing` + `PipelineEvent` SSE is sufficient for a single-user local app. Structured metrics export is unnecessary overhead. |
| **Intelligence & Reasoning** | **7/10** | Multi-agent routing, intent analysis, ReAct execution with tool calling, abstractive history compression, and context-window-aware budget allocation. No chain-of-thought or multi-step planning beyond ReAct. No self-reflection on response quality. |
| **User Understanding & Personalization** | **8/10** | Structured UserModel with 10 domains, dynamic + static context injection, procedural rules from reflection, situation-aware coaching. Confidence calibration exists but isn't dynamically tuned. No explicit measurement of how well the system understands the user. |
| **Maintainability** | **8/10** | Clean crate boundaries, zero-clippy-warning policy, comprehensive tests, derive macros for tools, conventional commit format. 26 crates is a lot to navigate but well-organized. Legacy L2 learning tables alongside L5 cognitive system adds some confusion. |

### Overall Score: **7.0 / 10**

The system excels at memory architecture and personalization (where it arguably leads the field for personal AI agents) but has gaps in observability, scalability, and self-evaluation capabilities.

---

## 9. Recommendations for Improvement

### 9.1 High Priority

#### 1. Add Intelligence & Understanding Metrics

**Problem:** No way to measure how well the AI understands the user or how intelligent its responses are.

**Solution:**
- Implement a **User Understanding Score** computed from: fact coverage across domains, average confidence, recall success rate (FSRS-tracked), and coaching intervention effectiveness
- Add **Response Quality Signals**: implicit (user corrections, topic re-asks, session length) and explicit (thumbs up/down in desktop UI)
- Track **Confidence Calibration**: compare the AI's confidence predictions with actual outcomes

#### ~~2. Implement Structured Observability~~ — Skipped

Not needed for a single-user local app. Existing `tracing` logs, `/api/cognitive/stream` SSE, and `PipelineEvent` broadcast provide sufficient observability.

### 9.2 Medium Priority

#### ~~3. Batch LLM Operations~~ ✅ Implemented

~~**Problem:** `consolidate_batch` makes individual LLM calls per fact. Extraction and consolidation are sequential.~~

**Implemented:** Micro-batch pipeline collects events in 3s windows (max 10), processes via single batch LLM calls for both extraction and consolidation (1+1 per window instead of N+N). Smart ADD bypass skips LLM for candidates with no existing matches. Concurrent `prefetch_existing` via `join_all`. See `crates/cognitive/src/background.rs`.

#### ~~4. Add Dead-Letter Queue for Failed Memory Operations~~ ✅ Implemented

~~**Problem:** If LLM extraction or consolidation fails, the observation is lost.~~

**Implemented:** `failed_observations` table with linear backoff (`(retry_count + 1) * 5 minutes`, max 3 retries). Self-healing drain piggybacks on next successful LLM batch — pulls up to 5 eligible items per healthy cycle. See `crates/cognitive/src/repos/failed_observation.rs` and migration `007_failed_observations.sql`.

#### 5. Add Per-User FSRS Calibration

**Problem:** FSRS parameters (`maxStability`, relevance weights) are configurable via `CognitiveConfig` but not automatically tuned per-user.

**Solution:**
- Track retrieval success/failure outcomes
- Adjust stability growth rate and relevance weights per user based on actual recall patterns
- Implement Bayesian parameter optimization over time

### 9.3 Lower Priority / Future Vision

#### 6. Multi-Model Memory Pipeline

Use smaller/cheaper models for extraction and consolidation, reserving expensive models for user-facing responses.

#### 7. Graph-Based Fact Relations

Add edges between semantic facts to represent relationships (e.g., "peak_hours CAUSES higher_productivity"). This would enable reasoning chains.

#### 8. Proactive Memory Verification

Periodically verify high-importance facts with the user ("I believe your peak hours are 10am-12pm — is this still accurate?"). Prevents stale facts.

#### 9. Memory Export & Portability

Allow users to export their complete memory state (UserModel + episodic + procedural rules) as a portable format, enabling migration between AI systems.

