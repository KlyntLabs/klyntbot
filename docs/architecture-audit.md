# Klyntbot Architecture Audit

> **Date:** 2026-03-24
> **Scope:** Comprehensive analysis of the agentic AI architecture — 34 crates, 9 layers, 10 subsystems
> **Method:** Multi-round automated deep-dive exploration (10 specialized code explorers across 2 rounds)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [System Architecture Overview](#2-system-architecture-overview)
3. [Component Deep-Dives](#3-component-deep-dives)
4. [Component Interconnection Map](#4-component-interconnection-map)
5. [Maturity Assessment](#5-maturity-assessment)
6. [Failure Pattern Analysis](#6-failure-pattern-analysis)
7. [Benchmarking Against Modern Architectures](#7-benchmarking-against-modern-architectures)
8. [Architectural Gaps & Risks](#8-architectural-gaps--risks)
9. [Recommendations](#9-recommendations)

---

## 1. Executive Summary

Klyntbot is a **single-user, local-first personal AI agent** built in Rust. It connects 6+ chat platforms (Telegram, Discord, Slack, Email, Desktop, MCP) to LLMs with persistent memory, task/project management, and self-optimization. All state lives in SQLite + LanceDB with no external services required.

### Key Architectural Strengths
- **Mature ReAct implementation** with fabrication detection, duplicate suppression, and forced synthesis
- **Sophisticated memory system** with FSRS-inspired decay, bi-temporal facts, and background consolidation
- **Self-optimizing autotuner** with zero-cost shadow evaluation and phased parameter expansion
- **Clean dependency inversion** — 9-layer architecture with strictly upward dependencies
- **Multi-modal skill routing** — keyword + semantic scoring with graceful fallback

### Key Architectural Risks
- **Tokenizer mismatch** — tiktoken cl100k_base used for all providers (±15% error for Claude)
- **Single-threaded outbound dispatcher** — slow channel blocks all platform delivery
- **No retry on LRU session eviction failure** — potential data loss
- **Dead-letter queue without retry limits** — pathological inputs cause infinite retries
- **Classification prompt injection risk** — user input embedded in classifier prompt without sanitization

### Overall Maturity: **Advanced Prototype → Early Production**

The system has production-grade patterns in core areas (ReAct loop, memory retrieval, tool execution) but pre-production gaps in resilience, observability, and edge-case handling.

---

## 2. System Architecture Overview

### Layer Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│ L8: klyntbot (facade), klyntbot-server (MCP binary)                │
├─────────────────────────────────────────────────────────────────────┤
│ L7: app-core (shared handlers), desktop-shared, desktop (Tauri)    │
├─────────────────────────────────────────────────────────────────────┤
│ L6: mcp (MCP server/client)                                        │
├─────────────────────────────────────────────────────────────────────┤
│ L5: channels, agent, cognitive                                      │
│     ┌──────────┐ ┌──────────────────────────────────┐ ┌──────────┐ │
│     │Telegram  │ │ AgentRuntime                     │ │Episodic  │ │
│     │Discord   │ │  ├─ IntentAnalyzer (4-layer)     │ │Semantic  │ │
│     │Slack     │ │  ├─ SkillRouter                  │ │FSRS5     │ │
│     │Email     │ │  ├─ ExecutionRouter               │ │Reflection│ │
│     │          │ │  │   ├─ DirectEngine              │ │Salience  │ │
│     │          │ │  │   └─ ReactiveEngine (ReAct)    │ │Decay     │ │
│     │          │ │  ├─ ContextEngine                 │ │          │ │
│     │          │ │  └─ CostTracker                   │ │          │ │
│     └──────────┘ └──────────────────────────────────┘ └──────────┘ │
├─────────────────────────────────────────────────────────────────────┤
│ L4: tools, feature-*, plugin-runtime, autotuner, activity-log      │
│     20+ domain tools, WASM plugins, self-optimization               │
├─────────────────────────────────────────────────────────────────────┤
│ L3: providers, session, scheduling, context_engine, skill-system   │
│     LLM clients, session persistence, cron, token budgets          │
├─────────────────────────────────────────────────────────────────────┤
│ L2: storage (SqlitePool, migrations, *Repo, VectorStore/LanceDB)   │
├─────────────────────────────────────────────────────────────────────┤
│ L1: config, bus, tools-core, tools-core-macros, analytics          │
├─────────────────────────────────────────────────────────────────────┤
│ L0: common (KlyntbotError, MessageRole, types), platform-macos     │
└─────────────────────────────────────────────────────────────────────┘
```

### Data Flow (Happy Path)

```
User Message (Telegram/Discord/Slack/Email/Desktop/MCP)
  → Channel adapter normalizes → InboundMessage
  → MessageBus (mpsc) → AgentLoop
  → SessionManager (DashMap LRU + SQLite)
  → AgentRuntime pipeline:
      1. SkillRouter selects orchestrator (keyword 70% + semantic 30%)
      2. IntentAnalyzer classifies (heuristic → embedding → LLM → cognitive)
      3. ContextEngine assembles (budget waterfall → memory RAG → history compression)
      4. ExecutionRouter dispatches (Direct or Reactive)
      5. ReactiveEngine runs ReAct loop (LLM ↔ tools, max N iterations)
      6. ResponseValidator validates output
      7. CostTracker records usage
  → OutboundMessage → MessageBus → ChannelManager
  → Channel adapter formats → Platform API
  → Background: DomainEventBus → Cognitive consolidation pipeline
```

---

## 3. Component Deep-Dives

### 3.1 Agent Runtime & ReAct Loop

**Location:** `crates/agent/src/agent_runtime/`, `crates/agent/src/intent_pipeline/`, `crates/agent/src/execution/`

The agent runtime is the central orchestrator that sequences the entire message-processing pipeline. It implements a **classic ReAct (Reasoning + Acting) pattern** with several production-hardening additions.

#### ReAct Loop Mechanics

The `ReactiveEngine` iterates up to `max_iterations` (configurable per-skill, default 10, max 30):

1. **LLM call** — streaming or non-streaming depending on channel support
2. **Tool call detection** — if the LLM returns tool calls, execute them all in parallel (semaphore-bounded to 10)
3. **Result injection** — tool results appended as `Message::Tool` back into the conversation
4. **Cycle outcome classification:**
   - `FinalResponse` — text answer, loop terminates
   - `ToolsExecuted` — continue to next iteration
   - `FabricatedResponse` — LLM hallucinated a tool result instead of calling tools; retry with explicit instruction
   - `EmptyResponse` — no content, continue

**Termination conditions:**
- LLM produces a final text response (not on planning iteration 1)
- `max_iterations` exhausted → forced synthesis call with empty tool list
- `CancellationToken` fires (pre-checked each iteration)
- Fabrication retry limit exceeded (default 2)

**Production hardening:**
- **Duplicate tool call suppression** via argument hashing (`DefaultHasher` on `name|args`)
- **Fabrication detection** — 4 heuristics catch LLMs (DeepSeek, Kimi) that skip tool calls and hallucinate results
- **Forced synthesis at exhaustion** — synthesis prompt includes plan progress (done/total steps)
- **Per-tool timeouts** — 30s default, 600s for `ask_user`, per-tool custom override
- **Tool result sanitization** — 100KB truncation + control character stripping
- **Reflection prompts** — on tool failure, injects "What went wrong..." prompt to guide recovery

#### Intent Classification (4-Layer Cascade)

The `IntentAnalyzer` uses a layered approach to classify messages as Direct (simple response) or Reactive (needs tools):

| Layer | Method | Cost | Latency |
|-------|--------|------|---------|
| L1 | Aho-Corasick keyword patterns | Zero | ~0ms |
| L2 | Embedding cosine similarity vs precomputed centroids | Zero (cached) | ~5ms |
| L3 | LLM JSON classifier (cheaper model) | Low | ~500ms |
| L4 | Cognitive boost (user model facts) | Zero | ~1ms |

Layer 3 is skipped in shadow mode (autotuner). Fallback on any failure: Reactive mode, 15 iterations, 0.5 confidence.

#### Chain-of-Thought Planning

For complex tasks (`complexity_score >= 4`), a planning prompt is injected requesting:
1. Optimistic, skeptical, and practical angles
2. A numbered plan with `[tool: <name>]` markers
3. The plan is parsed into an `ExecutionPlan` and tracked via `Scratchpad`

**Observation:** Plan step matching is by tool name only — if the same tool is called for different plan steps, tracking becomes inaccurate.

#### Iteration Budget Formula

```
budget = min(max(tool_calls * 3, 10) + 5, 30)
```

Profile cap applies on top, but orchestration override bypasses the profile cap.

---

### 3.2 Skill System & Routing

**Location:** `crates/skill-system/`, `skills/`

The skill system implements a **multi-persona routing layer** that selects the appropriate orchestrator skill per-message and controls tool access.

#### 5 Built-in Orchestrator Skills

| Skill | Triggers | MCP Access | Max Iterations | Delegation |
|-------|----------|------------|----------------|------------|
| `general` | 46 phrases ("hi", "hey", catch-all) | `["*"]` (all) | 15 | All 4 specialists |
| `task-management` | 72 phrases (multi-word) | `["google-calendar"]` | 12 | — |
| `finance-management` | Finance keywords | `[]` (none) | 10 | — |
| `automation` | Schedule/cron keywords | — | 10 | — |
| `communication` | Email/message keywords | — | 10 | — |

#### Routing Algorithm

```
score = 0.7 × keyword_score + 0.3 × semantic_score
```

- **Keyword scoring:** stop-word filtered tokenization + trigger phrase `contains()` matching (each hit adds 0.3, capped at 1.0)
- **Semantic scoring:** cosine similarity between message embedding and precomputed skill description embeddings
- **Candidacy gate:** `keyword_score > 0 OR semantic_score >= 0.5`
- **Fallback:** `general` skill (always selected if no candidate passes)

#### Tool Access Control (Dual Layer)

1. **Native tools:** `allowed_tool_names()` — `None` = unrestricted, `Some([])` = deny all
2. **MCP tools:** `allows_mcp_server()` — server-name whitelist per skill

#### Key Design Decisions

- **Compile-time embedding:** All 5 SKILL.md files + 21 reference files compiled into the binary via `include_str!` — zero runtime I/O
- **Scope shadowing:** User skills override builtins; project skills override user skills (priority: BuiltIn < User < Project)
- **Protected from compaction:** `SkillContextSource` is marked `protected = true`, preventing context compaction from evicting skill instructions mid-conversation
- **Delegation depth limit:** `MAX_DELEGATION_DEPTH = 2` prevents cascading delegation chains

#### Finding: `activated_skills` Write Path Missing

The `activated_skills` RwLock is initialized empty and passed to `SkillContextSource`, but `SkillRouter::activate_skills()` results are never written to it in the main pipeline. Only `always_skills` get injected. This is a gap between design intent and implementation.

---

### 3.3 Context Engine & Retrieval

**Location:** `crates/context_engine/`, `crates/cognitive/src/services/`

The context engine orchestrates **token budget allocation** and **multi-source retrieval** to assemble the final message array sent to the LLM.

#### Token Budget Waterfall

Priority order (highest allocated first):

| Priority | Source | Purpose |
|----------|--------|---------|
| 0 | SystemIdentity | Core identity + persona |
| 1 | ActiveTask | Current focused task context |
| 2 | ToolDefinitions | Tool schemas for function calling |
| 3 | RecentHistory | Recent conversation messages |
| 4 | RetrievedMemory | RAG-retrieved facts and recalls |
| 5 | CompressedHistory | Summarized older history |
| 6 | BootstrapPersona | Workspace markdown files |
| 7 | Skills | Skill instructions |

15% of the context window is always reserved for response generation. Memory retrieval is allocated *before* history compression, ensuring memories always fit.

#### InsightForge (Multi-Source Retrieval)

The `InsightForge` decomposes queries into up to 5 sub-queries, fans out across all sources in parallel, and merges with **Reciprocal Rank Fusion (RRF, k=60)**:

```
Sources fanned out per sub-query:
  ├─ UnifiedMemoryService (cognitive facts + conversation recall)
  ├─ NoteSearcher
  ├─ TaskSearcher
  ├─ GraphSearcher
  ├─ FinanceSearcher
  └─ BookRAGSearcher
```

**Source diversity cap:** No single source can provide more than 60% of results (post-RRF enforcement).

**Circuit breaker:** Per-session, 3 failures → 300s cooldown → fallback to plain `MemoryRetriever`.

#### Query Rewriting (Async Race)

1. **Heuristic path** (synchronous): classifies query specificity (High/Medium/Low based on pronouns, domain keywords, action verbs), collects context signals (active skill/task, recent correction, active view, recent messages)
2. **LLM path** (async, 800ms cap): spawned via `tokio::oneshot`, races against InsightForge execution
3. If LLM finishes during InsightForge, supplementary retrieval is performed (capped at `limit/2`)

**Finding:** The LLM rewrite result is checked via `try_recv()` (non-blocking). If InsightForge is fast, the LLM result is always discarded — making LLM rewriting effectively best-effort.

#### Context Source Priority Stack

Context sources are collected and injected in priority order:

| Priority | Source | Content |
|----------|--------|---------|
| 100 | `IdentityContextSource` | Date/time/OS/channel, routing instructions |
| 95 | `PersonaContextSource` | Merged persona chain instructions |
| 90 | `BootstrapContextSource` | AGENTS.md, SOUL.md, USER.md, TOOLS.md, etc. |
| 60 | `CognitiveContextSource` | User model facts + procedural rules (60s cache) |
| 35 | `SkillContextSource` | Active skill SKILL.md body + sub-skills |

**Finding:** `BootstrapSource` reads all 7 workspace markdown files without size limits. A large `SOUL.md` would bloat every context window for the session lifetime.

---

### 3.4 Cognitive Memory System

**Location:** `crates/cognitive/`

The cognitive system implements a **biologically-inspired memory architecture** with episodic/semantic distinction, spaced repetition, salience-based filtering, and weekly reflection.

#### Memory Types

| Type | Storage | Purpose |
|------|---------|---------|
| `SemanticFact` | SQLite + LanceDB vectors | SPO triples with FSRS fields — the core knowledge graph |
| `EpisodicMemory` | SQLite | Event records with importance scores |
| `ProceduralRule` | SQLite | Learned behavioral rules (from reflection) |
| `UserModel` | Derived from SemanticFact | Domain-sectioned user understanding |
| `Annotation` | SQLite | Entity-linked metadata |

#### Memory Ingestion Pipeline (Background)

```
DomainEventBus → BackgroundConsolidationService
  → 3s batch window (max 10 events)
  → Salience filter: Extract | Accumulate | Discard
  → Extract path:
      LLM extraction → SPO triples (subject, predicate, object)
      → Classify type: fact | decision | milestone | pattern | insight
      → Prefetch existing facts for overlap detection
      → LLM consolidation: ADD | UPDATE | DELETE | NOOP
      → Execute: upsert to SQLite + embed to LanceDB
  → Accumulate path:
      Buffer in AccumulatedObservationRepo
      → Promote to extraction when count ≥ threshold AND seen on ≥ min_days
```

#### Memory Retrieval (6-Factor Relevance Scoring)

```
score = 0.30 × semantic_similarity
      + 0.20 × FSRS_retrievability
      + 0.15 × importance/confidence
      + 0.10 × access_frequency
      + 0.25 × situational_boost
      + 0.05 × temporal_recency
```

All 6 weights are overridable by the autotuner champion.

**Retrieval pipeline:**
1. Vector path: LanceDB cosine search → batch SQL load → FSRS-weighted scoring
2. Fallback path (< 3 vector results): all active facts scored with neutral 0.5 similarity
3. BM25 boost: FTS5 full-text search → RRF-style score addition
4. Stability update on access: `S += ln(1 + S)`

#### Two-Tier Prompt Injection

| Tier | Source | Trigger | Cache |
|------|--------|---------|-------|
| Static | `CognitiveContextSource` (priority 60) | Every message | 60s TTL |
| Dynamic | `UnifiedMemoryService` via `InsightForge` | Per-query retrieval | None |

Static tier: top facts by `confidence × stability` from UserModel, formatted as `"subject: predicate = object"`.
Dynamic tier: query-specific facts via vector search + RRF merging.

#### FSRS-Inspired Decay (Dual Formula)

Two intentionally different decay formulas:
- **Memory retrieval** (decay.rs): `R = exp(ln(0.9) × t / S)` — exponential
- **Flashcard scheduling** (fsrs5.rs): `R = (1 + t/(9S))^-1` — power-law (canonical FSRS-5)

Both give R=0.9 at t=S but diverge elsewhere.

#### Reflection Cycle (Weekly)

- Guard: ≥ 20 episodic memories required
- Input: week's episodic memories + current UserModel + procedural rules
- LLM output: fact updates, rule updates, summary
- Filter: only `source=="user_stated"` OR `confidence >= 0.7` are persisted
- Output: reflection episodic memory (importance=0.9, stability=5.0)

#### Compaction (Daily)

- Archive superseded facts > 90 days old
- Delete low-access episodic memories > 90 days (< 2 accesses)
- If total active > 10,000: archive facts with stability < 0.1

**Finding — Dead-letter queue without retry limit:** Failed LLM extractions are retried on the next batch cycle with no hard retry limit — pathological inputs could cause infinite retries.

---

### 3.5 Tool System & Feature Packages

**Location:** `crates/tools-core/`, `crates/tools-core-macros/`, `crates/tools/`, `crates/feature-*/`

#### Tool Lifecycle

```
#[derive(Tool)] / #[tool_actions]  →  Tool trait impl  →  ToolRegistry.register()
  → ToolRegistry.to_schema()  →  JSON Schema for LLM  →  LLM returns tool_call
  → ToolRegistry.prepare() (clone Arc, drop read lock)
  → InterceptorChain.check()
  → tool.execute(args, RoutingContext)  →  Result<String>
  → sanitize (100KB truncation, control chars stripped)
  → Message::Tool injected into conversation
```

#### Three Tool Definition Paths

1. **`#[derive(Tool)]`** — single-action tools via proc macro (simple path)
2. **`#[tool_actions]`** — multi-action tools with generated enum dispatch
3. **Manual `Tool` impl** — complex tools (Finance, Memory, Notes) using `ParamExtractor` directly

#### Feature Package Contract

```rust
trait FeaturePackage {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<DynTool>;           // Tool registration
    fn migrations(&self) -> Vec<FeatureMigration>;  // Schema management
    fn config_key(&self) -> Option<&str>;       // Config section
    fn default_config(&self) -> Option<Value>;  // Defaults
    fn health_check(&self) -> HealthStatus;     // Runtime health
}
```

**Finding — Bypass pattern:** `TasksFeature::tools()`, `FinanceFeature::tools()`, and `NotesFeature::tools()` return empty vectors. Their tools are wired directly in `AgentLoopBuilder` because they require injected dependencies (`Arc<dyn Trait>`) not available through `FeaturePackage::tools()`.

#### Permission System

`PermissionLevel`: `ReadOnly < Standard < Elevated < Admin`. No configured `ToolPermissions` = all tools allowed. MCP tools and WASM plugins with `network`/`agent` permissions are always `Elevated`.

#### WASM Plugin System

`PluginManager` loads Extism WASM plugins from `{data_dir}/plugins/`. Tool name = WASM function name. Execution: serialize args → `plugin.call(func_name, input)` → return string.

#### Read Lock Design

`ToolRegistry::prepare()` explicitly clones the `Arc<dyn Tool>` and releases the read lock before `execute()`. This prevents deadlocks when tools (e.g., delegation) need write access to the registry.

---

### 3.6 Channel & Platform Integrations

**Location:** `crates/channels/`, `crates/bus/`

#### Platform Matrix

| Platform | Transport | Normalization | Max Message | Streaming |
|----------|-----------|---------------|-------------|-----------|
| Telegram | HTTP long-poll (reqwest) | Markdown → HTML | 4096 chars | No |
| Discord | WebSocket (tokio-tungstenite) | Passthrough | 2000 chars | No |
| Slack | Socket Mode WebSocket | Markdown → mrkdwn | 8000 chars | No |
| Email | IMAP poll + SMTP (lettre) | HTML → plain text | 8000 chars | No |
| Desktop | Tauri IPC + SSE | Native | Unlimited | Yes |
| MCP | stdio JSON-RPC (rmcp) | Native | Unlimited | No |

**Finding — No streaming to external channels:** Real-time streaming (`StreamingHandle` with per-token events) only works for the Desktop UI. All chat platforms get the complete response after the full pipeline completes.

#### Message Bus Architecture

Two independent bus systems:

| Bus | Type | Purpose | Consumer |
|-----|------|---------|----------|
| `MessageBus` | `tokio::mpsc` (point-to-point) | Channel ↔ Agent transport | Single (AgentLoop / ChannelManager) |
| `DomainEventBus` | `tokio::broadcast` (fan-out) | Cross-feature semantic events | Multiple (cognitive, coaching, autotuner) |

#### Outbound Dispatcher

**Finding — Single-threaded dispatcher:** `ChannelManager` loops sequentially through outbound messages. A slow channel's `send()` blocks delivery to all other channels.

#### Platform-Specific Details

- **Telegram:** Voice messages transcribed via Groq; long-poll at 30s intervals; no teloxide (raw HTTP)
- **Discord:** No serenity (raw WebSocket); reconnect loop with 5s retry
- **Slack:** Socket Mode with envelope ACK
- **Email:** Consent gate (refuses to start without explicit opt-in); IMAP UID tracking (in-memory, lost on restart)

---

### 3.7 Session Management

**Location:** `crates/session/`

#### Architecture

- **Cache:** `DashMap<SessionKey, Arc<TokioMutex<Session>>>` — concurrent per-session locking
- **LRU eviction:** `IndexMap`-based ordering; evicted sessions saved to SQLite
- **Persistence:** `batch_add_messages` in 111-row chunks with `INSERT OR IGNORE`
- **Compaction:** At 1000 messages, insert marker + keep 500 most recent

**Finding — LRU eviction data-loss risk:** If saving an evicted session to SQLite fails, a warning is logged but there is no retry or dead-letter queue.

---

### 3.8 Autotuner (Self-Optimization)

**Location:** `crates/autotuner/`, `crates/agent/src/autotuner/`

The autotuner implements a **champion/challenger A/B testing framework** that continuously optimizes the agent's routing, retrieval, and query rewriting parameters.

#### 19 Tunable Parameters (3 Phases)

| Phase | Parameters | Activation |
|-------|------------|------------|
| Phase 1: Routing | Heuristic weights, confidence thresholds, embedding similarity thresholds | Immediate (shadow classification) |
| Phase 2: Memory Retrieval | `vector_top_k`, `min_similarity`, 6 relevance weights, `accumulate_promote_threshold`, `accumulate_min_days` | After 7-day champion stability |
| Phase 3: Query Rewriting | `confidence_threshold`, `max_signals`, `min_enrichment_length` | Immediate (shadow + live) |

#### Shadow Evaluation (Zero-Cost)

Shadow predictions run `IntentAnalyzer` in Layer 1-2 only (Aho-Corasick + embeddings) — no LLM calls. Ground truth is recorded after the live response completes. Shadow retrieval uses `UnifiedMemoryService::retrieve_with_overrides`.

#### Nightly Cycle

```
Cron "0 2 * * *" → collect 24h metrics per trial
  → ConstraintEvaluator (9 constraints, all-failures collected):
      ├─ Correction improvement ≥ 5%
      ├─ Token increase ≤ 8%
      ├─ Latency increase ≤ 15%
      ├─ Routing stability drop ≤ 10%
      ├─ Memory relevance drop ≤ 5%
      └─ Rewrite engagement drop ≤ 10%
  → Score: correction_improvement + 0.1 × diversity_bonus
  → Promote winner → update champion → propagate to live path
  → Check regression → auto-rollback after 3 consecutive days
  → LLM generates next experiment (3 variants, temperature=0.7)
```

#### Experiment Pace

Three modes: `conservative` (small adjustments), `balanced` (1 conservative + 1 moderate + 1 bold, default), `bold` (2 bold + 1 moderate). Injected into the LLM prompt to shape exploration strategy.

**Finding — `accumulate_promote_threshold` and `accumulate_min_days` are dead letters:** These Phase 2 params are read once at startup by `BackgroundConsolidationService`. The autotuner promotes new values, but they only take effect after a restart.

**Finding — Diversity bonus is negligible:** `0.1 × (distance / max_distance)` adds at most 0.5 percentage points to a 5% correction improvement. The winner is almost always the trial with the best raw `correction_rate`.

---

### 3.9 Squad & Persona System

**Location:** `crates/cognitive/src/repos/{squad,persona,blackboard}.rs`, `crates/agent/src/intent_pipeline/engines/debate.rs`

#### Two Separate Persona Systems

| System | Storage | Purpose |
|--------|---------|---------|
| `PersonaManager` + `.md` files | Filesystem (`~/.klyntbot/personas/`) | Shapes agent system prompt per session scope |
| `PersonaRepo` + builtin DB rows | SQLite (10 hardcoded personas) | Insight analysis and squad debate |

These share the name "persona" but are architecturally distinct with no shared code.

#### Squad Debate Architecture

4-phase room debate with an LLM judge:

1. **Opening:** All personas respond in parallel
2. **Discussion:** Sequential (each sees prior speakers via blackboard)
3. **Targeted:** Judge issues per-persona challenges
4. **Final:** Parallel closing statements → consensus

Convergence: `MAX_ROUNDS = 6`, `CONSENSUS_THRESHOLD = 85.0`. Judge returns `"stop"` or score ≥ 85 → early termination.

**Finding — Not cancellable:** The debate loop runs to completion with no `CancellationToken` threading. Long debates can't be interrupted.

**Finding — Blackboard leak:** `blackboard_entries` has no TTL or cleanup job. Sessions with UUID keys accumulate rows over time.

---

### 3.10 Storage & Resilience

**Location:** `crates/storage/`, `crates/common/src/error.rs`

#### Storage Architecture

- **Relational:** SQLite via `sqlx` with WAL mode, foreign keys, `busy_timeout=5000`
- **Vectors:** LanceDB (optional — `Option<VectorStore>` throughout; missing vector store degrades silently)
- **Sessions:** `DashMap` LRU cache → SQLite overflow
- **Feature migrations:** Per-feature `FeatureMigration` in transactions, tracked via `_feature_migrations` table

#### Dual Circuit Breaker Architecture

| Circuit Breaker | Scope | Persistence | Threshold | Cooldown |
|----------------|-------|-------------|-----------|----------|
| `ProviderManager` | Global (LLM providers) | SQLite (survives restart) | 5 failures | 60s |
| `InsightForge` | Per-session | In-memory `DashMap` | 3 failures | 300s |

#### Error Hierarchy

```
KlyntbotError (top-level, 15 variants)
  ├─ ToolError (tool execution failures)
  ├─ ProviderError (LLM call failures)
  ├─ ChannelError (platform communication)
  ├─ SessionError (session management)
  ├─ ConfigError (configuration issues)
  └─ StorageError (5 variants: NotFound, Conflict, Migration, Connection, Query)
```

#### Provider Failover

```
ProviderManager::chat()
  → check circuit_breaker → skip if open
  → retry_with_backoff([500ms, 1s, 2s])  (3 attempts)
      → on RateLimited: retry with delay
      → on other error: record_failure()
      → if failures ≥ 5: open circuit, persist
  → on exhaustion: try_fallback() (single attempt, no retry)
```

**Finding — No retry on fallback:** If the fallback provider is also rate-limited, the call fails immediately.

#### VectorStore Safety

`sanitize_predicate_value` and `validate_predicate` guard against LanceDB predicate injection (no parameterized queries). Values containing `;`, `\n`, `--`, or `/*` are rejected; single quotes are escaped.

---

### 3.11 Prompting Strategies

**Location:** `crates/agent/src/context_sources/`, `skills/`, `crates/cognitive/src/services/context_source.rs`

#### System Prompt Assembly

The final system prompt is assembled from priority-ordered context sources joined with `\n\n---\n\n`:

```
[Priority 100] Identity: date/time/OS/channel, routing instructions, ask_user, progressive disclosure
[Priority 95]  Persona: merged persona chain (Global → Area → Project/Feature)
[Priority 90]  Bootstrap: AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md, RESPONSE.md, HEARTBEAT.md
[Priority 60]  Cognitive: User model facts + procedural rules (cached 60s)
[Priority 35]  Skill: Active orchestrator SKILL.md body + always_skills + activated_skills
```

#### Specialized Prompt Templates

| Template | Trigger | Format |
|----------|---------|--------|
| Planning prompt | `complexity_score ≥ 4` | User message: multi-angle analysis → numbered `[tool: <name>]` plan |
| Scenario prompt | `has_hypothetical` | User message: 5-step reasoning with knowledge graph neighborhoods |
| Synthesis prompt | Max iterations exhausted | User message: plan progress summary → forced text response |
| Fabrication retry | LLM hallucinated tool result | User message: "You MUST call the appropriate tool" |
| Duplicate block | Same tool call repeated | User message: "This tool call was blocked as duplicate" |
| Reflection prompt | Tool failure | User message: "What went wrong..." |

#### Cognitive LLM Prompts (Background)

| Prompt | Purpose | Output Format |
|--------|---------|---------------|
| `EXTRACTION_SYSTEM_PROMPT` | Convert observations → SPO triples | JSON |
| `CONSOLIDATION_SYSTEM_PROMPT` | Decide: ADD / UPDATE / DELETE / NOOP | JSON |
| `REFLECTION_SYSTEM_PROMPT` | Weekly pattern synthesis | JSON |
| `CLASSIFICATION_PROMPT` | Intent classification | JSON |

#### Few-Shot Strategy

No static few-shot examples. The only in-context learning uses dynamically retrieved `StrategyRepo` records (past successful strategy decisions) appended to the classification prompt.

#### Confidence XML

The LLM is instructed to emit inline `<confidence score="0.85" clarity="high" reasoning="..." />` XML before tool calls. `ResponseValidator` strips these blocks via regex before user-facing output.

**Finding — Classification prompt injection risk:** `IntentClassifier` sends the classification prompt as `Message::user(prompt)` with no system message. User input is embedded via `replace("{message}", message)` without sanitization.

**Finding — System leak detector is keyword-only:** `ResponseValidator` checks 11 hardcoded lowercase patterns. Paraphrases and unicode lookalikes bypass it.

---

## 4. Component Interconnection Map

```
                                    ┌────────────────┐
                                    │  External LLMs  │
                                    │ (Claude, GPT,   │
                                    │  DeepSeek, etc) │
                                    └───────┬────────┘
                                            │
                    ┌───────────────────────┼───────────────────────┐
                    │                       │                       │
              ┌─────▼─────┐          ┌──────▼──────┐         ┌─────▼─────┐
              │ Provider   │          │  Provider   │         │ Provider  │
              │ Manager    │◄─circuit─┤  Factory    │         │ (Fallback)│
              │ (Primary)  │ breaker  │             │         │           │
              └─────┬──────┘          └─────────────┘         └───────────┘
                    │
              ┌─────▼──────────────────────────────────────────────────┐
              │                    AgentRuntime                         │
              │  ┌──────────┐  ┌──────────────┐  ┌─────────────────┐  │
              │  │  Skill   │  │   Intent     │  │  Execution      │  │
              │  │  Router  │──┤  Analyzer    │──┤  Router         │  │
              │  │          │  │  (4-layer)   │  │  Direct/React   │  │
              │  └──────────┘  └──────────────┘  └────────┬────────┘  │
              │                                           │           │
              │  ┌──────────────────┐  ┌─────────────────▼────────┐  │
              │  │  Context Engine  │  │    ReactiveEngine        │  │
              │  │  ├─ Budget       │  │    (ReAct Loop)          │  │
              │  │  ├─ InsightForge │  │    ├─ Tool execution     │  │
              │  │  ├─ QueryRewrite │  │    ├─ Fabrication detect │  │
              │  │  └─ Sources[]    │  │    ├─ Duplicate suppress │  │
              │  └────────┬─────────┘  │    └─ Synthesis          │  │
              │           │            └──────────────────────────┘  │
              └───────────┼────────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────────────────┐
          │               │                           │
    ┌─────▼──────┐  ┌─────▼──────┐           ┌───────▼───────┐
    │  Cognitive  │  │  Unified   │           │  Tool         │
    │  Context    │  │  Memory    │           │  Registry     │
    │  Source     │  │  Service   │           │  (20+ tools)  │
    │  (static)   │  │  (dynamic) │           │  + WASM       │
    └─────┬──────┘  └─────┬──────┘           │  + MCP        │
          │               │                   └───────┬───────┘
          │         ┌─────┼──────────┐               │
          │         │     │          │         ┌──────▼──────┐
          │    ┌────▼──┐ ┌▼────┐ ┌──▼───┐    │  Feature    │
          │    │Vector │ │BM25 │ │Recall│    │  Packages   │
          │    │Search │ │FTS5 │ │(conv)│    │  tasks/fin/ │
          │    │LanceDB│ │     │ │      │    │  notes/prod │
          │    └───────┘ └─────┘ └──────┘    └─────────────┘
          │
    ┌─────▼──────────────────────────────────────────────┐
    │                Background Pipeline                  │
    │  DomainEventBus → Salience → Extract → Consolidate │
    │                → Accumulate → Promote → Extract     │
    │  Weekly Reflection    Daily Compaction               │
    └─────────────────────────────────────────────────────┘
          │
    ┌─────▼──────┐     ┌────────────┐     ┌──────────────┐
    │  Autotuner │     │  Session   │     │  Scheduling  │
    │  Shadow ◄──┼─────┤  Manager   │     │  CronService │
    │  Nightly   │     │  DashMap   │     │  Reminders   │
    │  Champion  │     │  + SQLite  │     │              │
    └────────────┘     └────────────┘     └──────────────┘
```

### Cross-Cutting Connections

| From | To | Mechanism | Purpose |
|------|----|-----------|---------|
| Channels → Agent | `MessageBus` (mpsc) | Normalized `InboundMessage` delivery |
| Agent → Channels | `MessageBus` (mpsc) | `OutboundMessage` delivery |
| Agent → Cognitive | `DomainEventBus` (broadcast) | `ChatTurnCompleted` events trigger memory ingestion |
| Agent → Autotuner | `AutoTunerHook` trait | Shadow classification on every message |
| Autotuner → Memory | `memory_param_sink` (RwLock) | Champion params override retrieval weights |
| Autotuner → Routing | `TrialParams` overrides | Shadow classification with trial-specific params |
| Skills → Tools | `allowed_tool_names()` | Per-skill tool access filtering |
| Skills → MCP | `allows_mcp_server()` | Per-skill MCP server access |
| Context Engine → Memory | `MemoryRetriever` trait | Dynamic fact retrieval via InsightForge |
| Context Engine → Cognitive | `CognitiveContextSource` | Static user model injection |
| Tools → Storage | `StoragePool` / Repos | All domain data access |
| Session → Storage | `SessionRepo` | Overflow and persistence |
| Provider → Storage | `circuit_breaker_state` table | Circuit breaker persistence |

---

## 5. Maturity Assessment

### Maturity Scale

| Level | Description |
|-------|-------------|
| **1 - Prototype** | Functional but fragile; happy-path only |
| **2 - Alpha** | Core logic solid; error handling incomplete |
| **3 - Beta** | Production-ready patterns; gaps in edge cases |
| **4 - Production** | Battle-tested; comprehensive error handling |
| **5 - Mature** | Observability, graceful degradation, self-healing |

### Component Ratings

| Component | Maturity | Justification |
|-----------|----------|---------------|
| **ReAct Loop** | 4 | Fabrication detection, duplicate suppression, forced synthesis, per-tool timeouts, reflection prompts |
| **Intent Classification** | 3.5 | 4-layer cascade with graceful fallback, but classification prompt is injection-vulnerable |
| **Skill Routing** | 3 | Blended scoring works well, but `activated_skills` never written, trigger overlap unresolved |
| **Context Engine** | 3.5 | Sophisticated budget waterfall + multi-source RAG, but tokenizer mismatch (±15%) |
| **Memory System** | 3.5 | FSRS decay, bi-temporal facts, 6-factor scoring, background consolidation, but DLQ without retry limits |
| **Tool System** | 4 | Derive macros, permission levels, read-lock-drop pattern, 100KB sanitization, interceptor chain |
| **Channels** | 2.5 | All 4 platforms working, but no streaming, single-threaded outbound, email UID reset on restart |
| **Session Management** | 3 | DashMap LRU + SQLite, compaction at 1000, but data-loss risk on eviction failure |
| **Autotuner** | 3.5 | Zero-cost shadow eval, phased expansion, regression rollback, but dead-letter params, low diversity bonus |
| **Squad/Persona** | 2.5 | 4-phase debate works, but not cancellable, blackboard leaks, two confusingly-named persona systems |
| **Storage/Resilience** | 3 | Dual circuit breakers, WAL mode, feature migrations, but no fallback retry, predicate injection guard is basic |
| **Prompting** | 3 | Priority-ordered sources, specialized templates, confidence XML, but no prompt size limits, injection risk |
| **Config** | 3.5 | Diff-based save, env overrides, Secret newtype, but no schema versioning |

### Overall: **3.2 / 5 (Beta+)**

---

## 6. Failure Pattern Analysis

Analysis of common agentic AI failure patterns and whether Klyntbot is susceptible:

### 6.1 Infinite Loop / Runaway Execution

**Risk: LOW**

| Mitigation | Implementation |
|------------|----------------|
| Hard iteration cap | `max_iterations` (default 10, max 30), enforced at ReactiveEngine level |
| Forced synthesis | At exhaustion, synthesis prompt with empty tools forces termination |
| Duplicate suppression | `seen_tool_calls` HashSet prevents repeating identical tool calls |
| CancellationToken | Pre-checked at each iteration start |
| Per-tool timeout | 30s default, configurable per-tool |

**Residual risk:** Orchestration override bypasses profile cap. A malicious or confused LLM could call 30 unique tool variations before exhaustion.

### 6.2 Hallucinated Tool Results (Fabrication)

**Risk: LOW-MEDIUM**

| Mitigation | Implementation |
|------------|----------------|
| Fabrication detection | 4 heuristics (fake hex IDs, structured result patterns, multiple fields, numbered lists) |
| Retry with explicit instruction | "You MUST call the appropriate tool" injected |
| Retry limit | `max_fabrication_retries = 2` |

**Residual risk:** Detection is heuristic-based. Sophisticated fabrications that don't match the 4 patterns slip through. Models other than DeepSeek/Kimi may fabricate in novel ways.

### 6.3 Context Window Overflow

**Risk: LOW**

| Mitigation | Implementation |
|------------|----------------|
| Token budget waterfall | Priority-ordered allocation with 15% response reserve |
| History compression | LLM abstractive summary or extractive snippets for older messages |
| Session compaction | At 1000 messages, keep 500 + marker |
| Tool result truncation | 100KB hard cap |

**Residual risk:** Tokenizer mismatch (tiktoken cl100k_base for all providers) means budget calculations are ±15% off for Claude. The 15% reserve partially absorbs this.

### 6.4 Memory Pollution / Drift

**Risk: MEDIUM**

| Mitigation | Implementation |
|------------|----------------|
| Salience filter | Discard / Accumulate / Extract classification before ingestion |
| LLM consolidation | ADD/UPDATE/DELETE/NOOP decisions with existing fact context |
| Bi-temporal facts | Soft-delete (supersede) preserves history |
| Weekly reflection | User-stated or high-confidence only |
| Daily compaction | Archive old/low-stability facts |
| 90-day hard compaction | Remove superseded and low-access entries |

**Residual risk:** No human-in-the-loop confirmation for memory writes. Extraction hallucinations become permanent facts. The accumulator's promotion-without-verification path could crystallize noise. No mechanism to detect or correct factual contradictions beyond LLM consolidation.

### 6.5 Prompt Injection / Jailbreaking

**Risk: MEDIUM-HIGH**

| Mitigation | Implementation |
|------------|----------------|
| System leak detector | 11 hardcoded keyword patterns in ResponseValidator |
| Confidence XML | Optional self-assessment (stripped before output) |
| Tool permission levels | ReadOnly / Standard / Elevated / Admin |
| Skill-based tool filtering | Per-skill tool allowlists |

**Residual risk:**
- **Classification prompt injection:** User input is embedded via `replace("{message}", message)` in a `Message::User` (not system). Adversarial inputs could manipulate classification.
- **System leak detector is keyword-only:** Paraphrases, unicode lookalikes, and encoded outputs bypass it.
- **No input sanitization layer:** Raw user messages reach the LLM without any preprocessing.
- **Memory as injection vector:** Extracted facts from previous conversations are re-injected into future prompts without validation.

### 6.6 Cascading Failures / Thundering Herd

**Risk: LOW-MEDIUM**

| Mitigation | Implementation |
|------------|----------------|
| Dual circuit breakers | Provider (global, persisted) + InsightForge (per-session, in-memory) |
| Provider failover | Primary → retry with backoff → fallback provider |
| InsightForge degradation | Circuit open → plain MemoryRetriever fallback |
| Optional VectorStore | Missing vector store degrades silently |
| Noop provider | Missing API key → readable error, not crash |

**Residual risk:**
- No retry on fallback provider — single point of failure
- Single-threaded outbound dispatcher — slow platform blocks all delivery
- LRU session eviction failure → data loss with no recovery
- Circuit breaker persistence is best-effort (`tokio::spawn` write, warn on failure)

### 6.7 Skill/Intent Misrouting

**Risk: MEDIUM**

| Mitigation | Implementation |
|------------|----------------|
| Blended scoring | 70% keyword + 30% semantic with candidacy gate |
| General fallback | Default to general skill if no candidate passes |
| Orchestration override | Force general when IntentAnalyzer detects orchestration need |
| Autotuner shadow | Continuously evaluates routing accuracy |

**Residual risk:**
- Trigger phrase overlap between skills (e.g., "notes" matches both general and task-management)
- Semantic scoring depends on precomputed embeddings that don't update on skill reload
- No user feedback mechanism to correct misrouting in real-time

### 6.8 Stale / Contradictory Memory

**Risk: MEDIUM**

| Mitigation | Implementation |
|------------|----------------|
| Bi-temporal facts | `valid_from`/`valid_until` + `recorded_at`/`superseded_at` |
| Supersede pattern | Old facts soft-deleted, not overwritten |
| Weekly reflection | Synthesizes patterns from episodic memories |
| Compaction | 90-day archive for superseded facts |

**Residual risk:**
- No contradiction detection between facts (beyond LLM consolidation)
- Static tier cache (60s TTL) can serve stale facts
- Superseded facts remain in vector store until the next embedding cycle
- No temporal reasoning — facts lack "as of" context in prompts

### 6.9 Cost Runaway

**Risk: LOW**

| Mitigation | Implementation |
|------------|----------------|
| Iteration caps | Per-skill, profile-bounded, max 30 |
| CostTracker | Per-request usage recording with per-model pricing |
| Shadow mode is zero-cost | No LLM calls for autotuner shadow evaluation |
| Direct mode for simple queries | Confidence-based downgrade from Reactive to Direct |

**Residual risk:** No per-day or per-session cost budget. A long debate (6 rounds × N personas) can consume significant tokens with no ceiling.

---

## 7. Benchmarking Against Modern Architectures

### Comparison with Notable Agent Frameworks

| Capability | Klyntbot | LangGraph | CrewAI | AutoGPT | Claude Computer Use |
|------------|----------|-----------|--------|---------|---------------------|
| **ReAct loop** | Full (with planning, fabrication detection, synthesis) | Full (graph-based state machine) | Partial (sequential/parallel tasks) | Full (with self-criticism) | Implicit (model-driven) |
| **Memory persistence** | FSRS decay + vector + BM25 + bi-temporal | Vector store (pluggable) | Short-term only | File-based + vector | None (stateless) |
| **Self-optimization** | Autotuner with A/B testing, nightly cycle | None built-in | None built-in | None | None |
| **Multi-persona debate** | 4-phase with LLM judge | Custom via graph nodes | CrewAI agents | None | None |
| **Skill/intent routing** | Blended keyword + semantic scoring | Graph edges | Role-based | Goal decomposition | None |
| **Tool access control** | Per-skill dual-layer (native + MCP) | Manual configuration | Role-based | Self-managed | Hardcoded |
| **Context budget management** | Priority waterfall with compaction | Manual truncation | None | Token counting | Automatic (model-side) |
| **Multi-platform** | 6 platforms | API-only | API-only | CLI/Web | API-only |
| **Local-first / Privacy** | Yes (SQLite + local LanceDB) | No (cloud services) | No | Partial | No |
| **Streaming** | Desktop only | Full | None | Partial | Full |
| **Error recovery** | Dual circuit breakers, provider failover, fabrication retry | Custom | Basic retry | Basic retry | None |

### Key Differentiators

**Klyntbot advantages over the field:**
1. **Self-optimization loop** — No other framework has a built-in A/B testing autotuner with shadow evaluation
2. **FSRS-inspired memory decay** — Biologically-inspired forgetting curve; most frameworks use static vector stores
3. **Multi-platform with format adaptation** — 6 platforms with per-platform formatters; most are API-only
4. **Local-first architecture** — SQLite + LanceDB, no cloud dependency
5. **Bi-temporal fact management** — Business time + system time; unprecedented in consumer AI agents
6. **Background memory consolidation** — Async ingestion with salience filtering; most frameworks store everything

**Klyntbot gaps vs. the field:**
1. **No streaming to chat platforms** — LangGraph and Claude Computer Use support full streaming
2. **No graph-based workflow orchestration** — LangGraph's state machine model is more flexible for complex multi-step workflows
3. **No human-in-the-loop for memory** — Memory writes are fully automated; no confirmation UI
4. **No structured output enforcement** — Tools return plain strings; no typed output validation
5. **Limited observability** — No OpenTelemetry, no metrics dashboard (intentional non-goal per CLAUDE.md)
6. **No multi-turn tool planning** — Planning is single-turn text; no persistent plan state across conversations

### Architecture Pattern Comparison

| Pattern | Klyntbot | Industry Standard |
|---------|----------|-------------------|
| **Agent Loop** | ReAct with escalation | ReAct, Plan-and-Execute, Tree-of-Thought |
| **Memory** | SPO triples + FSRS + vector + BM25 | Vector store + metadata filtering |
| **Routing** | Skill-based with blended scoring | Router agent, classifier, or graph edges |
| **Tool Calling** | OpenAI function-calling format | OpenAI format (de facto standard) |
| **Context Management** | Priority waterfall budget | Manual truncation or automatic (model-side) |
| **State Persistence** | SQLite + LanceDB (local) | Redis/Postgres + Pinecone/Weaviate (cloud) |
| **Self-improvement** | Champion/challenger A/B testing | None (manual tuning) |
| **Error Recovery** | Circuit breaker + fallback + retry | Retry with backoff |

---

## 8. Architectural Gaps & Risks

### Critical (Should Address Before Production)

| # | Gap | Impact | Affected Component |
|---|-----|--------|--------------------|
| C1 | **Tokenizer mismatch** — tiktoken cl100k_base used for all providers, ±15% error for Claude | Context overflow or waste | Context Engine |
| C2 | **Classification prompt injection** — user input embedded in `Message::User` without sanitization | Intent manipulation, skill bypass | Intent Analyzer |
| C3 | **No retry on LRU session eviction failure** — data loss with only a warning | Lost conversation history | Session Manager |
| C4 | **Single-threaded outbound dispatcher** — one slow channel blocks all platforms | Delivery latency for all users | Channel Manager |
| C5 | **Dead-letter queue without retry limit** — pathological LLM inputs retried forever | Background CPU/token waste | Cognitive Pipeline |

### High (Significant Impact)

| # | Gap | Impact | Affected Component |
|---|-----|--------|--------------------|
| H1 | **`activated_skills` write path missing** — designed feature not connected | Per-message skill activation doesn't work | Skill System |
| H2 | **System leak detector is keyword-only** — trivial to bypass | Information leakage | Response Validator |
| H3 | **No human-in-the-loop for memory writes** — extraction hallucinations become facts | Memory pollution | Cognitive Pipeline |
| H4 | **Email IMAP UID tracking resets on restart** — re-processes old emails | Duplicate responses | Email Channel |
| H5 | **Blackboard entries have no cleanup** — accumulate indefinitely | Storage growth | Squad System |
| H6 | **Bootstrap workspace files have no size limit** — large SOUL.md bloats every request | Token waste | Context Engine |
| H7 | **Shadow log ground truth matched by chat_id, not message_id** — wrong matches possible | Autotuner accuracy degraded | Autotuner |

### Medium (Worth Addressing)

| # | Gap | Impact | Affected Component |
|---|-----|--------|--------------------|
| M1 | **No streaming to external channels** — full response only after pipeline completes | Poor UX for long responses | Channels |
| M2 | **Dual FSRS formulas** — exponential vs power-law in different modules | Maintainer confusion | Cognitive Memory |
| M3 | **`AgentContextSource` is dead code** — superseded by `SkillContextSource` | Code cleanliness | Agent Runtime |
| M4 | **Plan step matching by tool name only** — same tool for different steps confuses tracker | Inaccurate plan progress | ReAct Loop |
| M5 | **Two persona systems with same name** — confusing for contributors | Development friction | Architecture |
| M6 | **Feature migrations called from 40+ sites** — no centralized startup registry | No migration order guarantee | Storage |
| M7 | **Debate judge truncates to 500 chars** — cuts reasoning mid-sentence | Poor judge decisions | Squad System |
| M8 | **No cost budget per day/session** — debate + complex tasks can run up costs | Unexpected bills | Cost Tracking |
| M9 | **Subagents run without memory/persona** — lose user context | Suboptimal delegation results | Agent Runtime |
| M10 | **Query rewrite LLM result often discarded** — InsightForge usually finishes first | Wasted LLM calls | Context Engine |
| M11 | **`accumulate_promote_threshold` requires restart** — autotuner promotes without effect | Dead parameter | Autotuner |
| M12 | **Diversity bonus capped at 10%** — rarely breaks ties | Reduced exploration | Autotuner |
| M13 | **Circuit breaker persistence is best-effort** — spawned write can fail | State lost on crash | Provider Manager |

### Low (Nice to Have)

| # | Gap | Impact | Affected Component |
|---|-----|--------|--------------------|
| L1 | **JSON hash key ordering dependency** — duplicate detection may miss semantically identical calls | Rare false negatives | ReAct Loop |
| L2 | **Confidence XML is instruction-prompted, not enforced** — LLM may skip or malform it | Missing confidence data | Prompting |
| L3 | **Schema type coverage gap** — nested objects/enums require hand-written schemas | Developer friction | Tool Macros |
| L4 | **MCP tool naming collision** — two servers with same sanitized name could collide | Rare edge case | MCP |
| L5 | **Planning prompt re-embeds user message** — message appears twice in context | Minor token waste | Prompting |
| L6 | **Config has no schema version** — breaking default changes affect users silently | Upgrade friction | Config |

---

## 9. Recommendations

### Immediate (Critical Path)

1. **Fix tokenizer per-provider** (C1)
   Use provider-specific tokenizers: `tiktoken` for OpenAI, Claude's tokenizer for Anthropic, or switch to character-based estimation with provider-specific chars-per-token ratios. The 15% reserve is insufficient for production use.

2. **Sanitize classification prompt** (C2)
   Move user input to a separate `Message::User` after the classification instructions (which should be `Message::System`), or escape/delimit user content with clear markers. Consider using structured output format to reduce prompt injection surface.

3. **Add retry + DLQ for session eviction** (C3)
   Implement a bounded retry (3 attempts with backoff) before logging data loss. Alternatively, keep evicted sessions in a lightweight in-memory queue until confirmed persisted.

4. **Parallelize outbound dispatcher** (C4)
   Spawn a `tokio::spawn` per-channel for outbound delivery instead of sequential loop. Each channel can have its own rate limiter.

5. **Add DLQ retry limit** (C5)
   Cap retries at 3-5 attempts in `BackgroundConsolidationService`. After limit, log the failed observation and move on.

### Short-Term (High Impact)

6. **Wire `activated_skills`** (H1) — Connect `SkillRouter::activate_skills()` results to the `activated_skills` RwLock in the runtime pipeline.

7. **Enhance system leak detector** (H2) — Add semantic pattern matching or LLM-based detection for system prompt leakage.

8. **Add memory confirmation UI** (H3) — Surface extracted facts in the desktop UI with accept/reject/edit actions before they become permanent.

9. **Persist email UIDs** (H4) — Store processed IMAP UIDs in SQLite instead of in-memory HashSet.

10. **Add blackboard TTL cleanup** (H5) — Run cleanup job for blackboard entries older than 24h.

11. **Add workspace file size limits** (H6) — Cap each bootstrap source file at a token budget (e.g., 2000 tokens each).

### Medium-Term (Architecture Improvements)

12. **Streaming for chat platforms** (M1) — Implement progressive message editing (Telegram `editMessageText`, Discord message edit, Slack `chat.update`) to simulate streaming.

13. **Consolidate FSRS formulas** (M2) — Document the intentional divergence clearly, or unify on the canonical FSRS-5 power-law form.

14. **Remove dead `AgentContextSource`** (M3) — Clean up the unused code path.

15. **Message-level shadow log matching** (H7) — Add `message_id` to shadow log entries for accurate ground truth matching.

16. **Centralize feature migrations** (M6) — Create a startup migration registry instead of 40+ callsites.

17. **Cost budgets** (M8) — Add configurable per-day and per-session cost limits with warning thresholds.

---

## Appendix A: Crate Dependency Graph (Simplified)

```
L0: common ←───────────────────────────── (all crates)
    platform-macos

L1: config ← bus ← tools-core ← tools-core-macros ← analytics

L2: storage ← (config, common, tools-core for FeatureMigration)

L3: providers ← (config, common)
    session ← (storage, common)
    scheduling ← (storage, common)
    context_engine ← (providers, common)
    skill-system ← (common, context_engine)

L4: tools ← (tools-core, storage, common, bus)
    feature-tasks ← (tools-core, storage, common)
    feature-finance ← (tools-core, storage, common, analytics)
    feature-notes ← (tools-core, storage, common)
    feature-productivity ← (tools-core, storage, common)
    autotuner ← (storage, common, config)
    activity-log ← (storage, common)
    plugin-runtime ← (tools-core, common)

L5: channels ← (bus, common, config, providers)
    agent ← (tools, tools-core, all feature-*, mcp, plugin-runtime,
             context_engine, providers, session, cognitive, autotuner,
             skill-system, storage, bus, common)
    cognitive ← (context_engine, storage, common, bus)

L6: mcp ← (common, tools-core)

L7: app-core ← (agent, channels, bus, scheduling, cognitive, mcp, config, storage)
    desktop-shared ← (common)
    desktop ← (app-core, desktop-shared, config)

L8: klyntbot ← (re-exports all)
    klyntbot-server ← (app-core, mcp)
```

## Appendix B: Key Metrics

| Metric | Value |
|--------|-------|
| Total crates | 34 (+ 2 excluded) |
| Architecture layers | 9 |
| Built-in orchestrator skills | 5 |
| Built-in personas (DB) | 10 |
| Built-in squads | 4 |
| Tunable autotuner parameters | 19 |
| Domain tools | 20+ |
| Supported chat platforms | 6 |
| Context source priorities | 5 levels (100, 95, 90, 60, 35) |
| ReAct max iterations | 30 (hard cap) |
| Memory relevance factors | 6 |
| Circuit breakers | 2 (provider global + InsightForge per-session) |
| Provider retry attempts | 3 (with exponential backoff) |
| Session compaction threshold | 1000 messages (keep 500) |
| Tool result size limit | 100KB |
| Parallel tool execution cap | 10 (semaphore) |
| Debate max rounds | 6 |
| Debate consensus threshold | 85.0 |
