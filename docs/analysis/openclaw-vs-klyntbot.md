# openclaw vs klyntbot: Comparative Analysis

> Generated: 2026-02-23, Updated: 2026-02-24
> Original scan: 5 parallel agents, 100+ files, ~470k tokens
> Update: Reflects completed SQLite+LanceDB migration, WASM plugin system, browser automation, learning loop + hierarchical sub-agents

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [At a Glance](#2-at-a-glance)
3. [Project Vision & Positioning](#3-project-vision--positioning)
4. [Architecture Overview](#4-architecture-overview)
5. [Language, Runtime & Tech Stack](#5-language-runtime--tech-stack)
6. [Agent Loop & Orchestration](#6-agent-loop--orchestration)
7. [Channel Integrations](#7-channel-integrations)
8. [Tools System](#8-tools-system)
9. [Memory & Storage](#9-memory--storage)
10. [LLM Provider Support](#10-llm-provider-support)
11. [Configuration System](#11-configuration-system)
12. [Plugin & Extension System](#12-plugin--extension-system)
13. [Skills System](#13-skills-system)
14. [Scheduling (Cron)](#14-scheduling-cron)
15. [Browser Automation](#15-browser-automation)
16. [Media Understanding](#16-media-understanding)
17. [Canvas / Generative UI](#17-canvas--generative-ui)
18. [Planning Engine](#18-planning-engine)
19. [Security Model](#19-security-model)
20. [Deployment & Distribution](#20-deployment--distribution)
21. [Testing Architecture](#21-testing-architecture)
22. [Remaining Gap Analysis](#22-remaining-gap-analysis)
23. [Summary Scorecard](#23-summary-scorecard)

---

## 1. Executive Summary

**openclaw** and **klyntbot** are both multi-channel AI agent frameworks with strikingly similar goals — a single agent serving multiple chat platforms, persistent memory, scheduling, and extensibility — but they diverge in architectural and engineering decisions.

| Dimension | openclaw | klyntbot |
|-----------|----------|----------|
| **Language** | TypeScript (Node 22+, ESM) | Rust (Edition 2021, Tokio) |
| **Distribution** | npm package + Docker + mobile apps | Single stripped binary (zero-infra) |
| **Storage** | SQLite + JSONL flat files | SQLite + LanceDB (zero-infra) |
| **Embeddings** | 5 cloud providers + local llama-cpp | fastembed local only (384d) |
| **Plugin System** | npm SDK: 24 lifecycle hooks, 40+ plugins | WASM sandbox (Extism): multi-lang, registry, CLI |
| **Browser automation** | Playwright/CDP + Docker sandboxes | agent-browser CLI + trust-level guards |
| **Planning engine** | Not implemented | Full 6-state lifecycle + backtracking |
| **Channels** | 8 core + extensions (Matrix, Teams, etc.) | 6 (Telegram, Discord, Slack, WhatsApp, QQ, Email) |
| **Mobile apps** | iOS (SwiftUI), Android (Jetpack), macOS native | None |
| **Canvas / Generative UI** | Yes (HTTP/WebSocket canvas host) | None |

**Bottom line**: Both frameworks now have plugin systems, browser automation, and zero-infrastructure storage. openclaw leads in ecosystem breadth (mobile, canvas, channel count, npm community). klyntbot leads in architectural depth (planning, orchestration with active learning loop, hierarchical sub-agents, domain models, WASM sandboxing, Rust safety).

---

## 2. At a Glance

```
┌──────────────────────────────────────────────────────────────────┐
│                      FEATURE MATRIX                              │
├─────────────────────────────┬──────────────┬────────────────────┤
│ Feature                     │  openclaw    │     klyntbot       │
├─────────────────────────────┼──────────────┼────────────────────┤
│ Multi-channel agent         │ ✅ 8+ channels│ ✅ 6 channels      │
│ Persistent memory           │ ✅ SQLite+FTS5│ ✅ SQLite+LanceDB  │
│ Semantic (vector) search    │ ✅ sqlite-vec │ ✅ LanceDB ANN     │
│ Hybrid search (RRF)         │ ✅           │ ✅                  │
│ LLM provider abstraction    │ ✅ 6+         │ ✅ 10+             │
│ Tool calling                │ ✅           │ ✅                  │
│ Cron / scheduling           │ ✅ croner     │ ✅ tokio tasks      │
│ Session persistence         │ ✅ JSONL      │ ✅ SQLite            │
│ Plugin / extension SDK      │ ✅ Rich SDK   │ ✅ WASM (Extism)    │
│ Browser automation          │ ✅ Playwright │ ✅ agent-browser    │
│ Media understanding         │ ✅ multi-prov │ ❌                  │
│ Canvas / Generative UI      │ ✅           │ ❌                  │
│ Multi-step planning engine  │ ❌           │ ✅ Full lifecycle   │
│ Goal tracking               │ ❌           │ ✅ GoalRepo         │
│ Task enrichment (AI)        │ ❌           │ ✅                  │
│ Calendar (CalDAV)           │ ❌           │ ✅ Apple/Google Cal │
│ Finance tools               │ ❌           │ ✅ (optional pack)  │
│ Adaptive orchestration      │ ❌           │ ✅ heuristic+LLM    │
│                             │              │   + learning loop   │
│ Hierarchical sub-agents     │ Partial      │ ✅ 4 profiles       │
│ User satisfaction feedback  │ ❌           │ ✅ reaction→score   │
│ Mobile apps                 │ ✅ iOS/Android│ ❌                  │
│ Docker sandbox              │ ✅           │ ❌                  │
│ Daemon management (OS)      │ ✅ launchd/   │ ❌ (manual/serve)  │
│                             │   systemd    │                    │
│ Pairing / allowlist         │ ✅           │ Partial            │
│ ACP protocol (IDE integr.)  │ ✅           │ ❌                  │
│ Single binary deploy        │ ❌           │ ✅                  │
│ Zero-deps deploy            │ ❌ (Node+PG) │ ✅ (single binary)  │
│ Test coverage enforcement   │ ✅ 70% thresh │ ✅ nextest+clippy  │
└─────────────────────────────┴──────────────┴────────────────────┘
```

---

## 3. Project Vision & Positioning

### openclaw

> *"OpenClaw is the AI that actually does things. It runs on your devices, in your channels, with your rules."*

- **Local-first personal agent**: Designed to run on the user's own hardware (Mac app, Docker, CLI)
- **Privacy through self-hosting**: No cloud infrastructure required for core operation
- **Multi-platform ubiquity**: iOS, Android, macOS native apps + web UI
- **Terminal-first setup** with no hidden security decisions
- **Ecosystem play**: Plugin SDK, npm extensions, community-contributed channels
- **Target user**: Power users and developers who want a ChatGPT-grade assistant that respects their privacy and runs across all their communication platforms

### klyntbot

- **Framework-first**: Rust AI agent framework for developers building autonomous agents
- **Operator-grade reliability**: Single binary, minimal attack surface, zero-downtime restarts
- **Domain-intelligent**: Deep task/project/goal/planning integration, not just a chat relay
- **Backend-native**: No mobile app — integrates with existing infrastructure (Telegram, Discord, etc. as front-ends)
- **Autonomous operation**: Multi-step planning, adaptive strategy routing, cron-driven autonomous turns
- **Target user**: Developers who want a programmable autonomous agent with persistent state and intelligent scheduling

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Primary metaphor** | Personal assistant across all your apps | Autonomous agent with persistent intelligence |
| **Setup complexity** | Medium (daemon, config wizard, channel auth) | Low (init wizard, zero infrastructure) |
| **Extensibility model** | Community plugins (npm ecosystem) | Source-code modifications + skill files |
| **End-user vs developer** | Both (mobile app for end users) | Developer-first |
| **Operator model** | Self-hosted on your devices | Self-hosted on server/laptop |

---

## 4. Architecture Overview

### openclaw Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│          OpenClaw — TypeScript Monorepo (pnpm workspaces)       │
├─────────────────────────────────────────────────────────────────┤
│  Entry: src/entry.ts → src/index.ts → src/cli/run-main.ts       │
│                                                                  │
│  ┌──────────────┐   ┌──────────────┐   ┌─────────────────────┐ │
│  │   Gateway    │   │   Channels   │   │   ACP Protocol      │ │
│  │  (Express +  │◄──│  (Telegram,  │   │  (stdio NDJSON,     │ │
│  │  WebSocket)  │   │  Discord,    │   │  IDE integration)   │ │
│  └──────┬───────┘   │  WhatsApp,   │   └─────────────────────┘ │
│         │           │  iMessage,   │                            │
│  ┌──────▼───────┐   │  Slack, etc) │   ┌─────────────────────┐ │
│  │  Agent Loop  │   └──────────────┘   │   Daemon Mgmt       │ │
│  │  (@marioz/pi │                      │  (launchd/systemd/  │ │
│  │  based)      │   ┌──────────────┐   │   schtasks)         │ │
│  └──────┬───────┘   │  Cron Service│   └─────────────────────┘ │
│         │           │  (croner)    │                            │
│  ┌──────▼───────┐   └──────────────┘   ┌─────────────────────┐ │
│  │  Tool        │                      │   Plugin System     │ │
│  │  Dispatcher  │   ┌──────────────┐   │  (24 hooks, npm)    │ │
│  └──────┬───────┘   │   Memory     │   └─────────────────────┘ │
│         │           │  (SQLite +   │                            │
│  ┌──────▼───────┐   │   FTS5 +     │   ┌─────────────────────┐ │
│  │  Browser     │   │  sqlite-vec) │   │   Canvas Host       │ │
│  │  (Playwright)│   └──────────────┘   │  (Gen UI)           │ │
│  └──────────────┘                      └─────────────────────┘ │
│                                                                  │
│  Storage: SQLite (memory) + JSONL (sessions) + FS (skills)      │
│  Runtime: Node 22+, pnpm, optional Bun execution                │
└─────────────────────────────────────────────────────────────────┘
```

**Key characteristics**:
- **Multi-process optional**: Can run agent in separate container, IDE connects via ACP
- **Event-driven core**: WebSocket gateway, event hooks, streams
- **Plugin-first extensibility**: Every component can be replaced/extended via npm plugins
- **Horizontal integration surface**: 8 chat channels + browser + canvas + mobile

### klyntbot Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│            klyntbot — Rust Workspace (Cargo, 15+ crates)        │
├─────────────────────────────────────────────────────────────────┤
│  Layer 0: common     ← KlyntbotError, MessageRole, ChatId       │
│  Layer 1: config     ← Config schema (camelCase JSON + serde)   │
│  Layer 1: bus        ← tokio::mpsc message bus                  │
│  Layer 1.5: storage  ← SqlitePool, LanceDB VectorStore, 24 repos│
│                                                                  │
│  Layer 2: providers  ← LlmProvider trait, 10+ implementations   │
│  Layer 2: session    ← SessionManager, SessionRepo              │
│  Layer 2: context_engine ← Budget alloc, compression, assembly  │
│  Layer 2: scheduling ← Cron types, CronRepo                     │
│  Layer 2: calendar   ← CalDAV sync engine                       │
│  Layer 2.5: plugin-runtime ← Extism WASM sandbox, PluginManager │
│                                                                  │
│  Layer 3: tools      ← Tool trait, 13+ implementations          │
│                        Handler traits (SpawnHandler, etc.)       │
│                        BrowserTool (agent-browser subprocess)    │
│                                                                  │
│  Layer 4: channels   ← Channel trait, 6 implementations         │
│  Layer 4: heartbeat  ← Scheduled agent turns                    │
│                                                                  │
│  Layer 5: agent      ← AgentLoop, Orchestrator, ContextEngine   │
│                        EngineDispatch, PlanExecutor,            │
│                        MemoryStore, SkillManager, SubagentMgr   │
│                        SubagentProfile (4 profiles, tool gates) │
│                        Strategy persistence (pipeline Step 6)   │
│                                                                  │
│  Layer 6: cli        ← Clap CLI (5 commands), init wizard       │
│  Layer 7: klyntbot   ← Re-export facade + binary entry point    │
│                                                                  │
│  Storage: SQLite (relational) + LanceDB (vector embeddings)    │
│  Binary: Single stripped executable (~50MB release)             │
└─────────────────────────────────────────────────────────────────┘
```

**Key characteristics**:
- **Strict dependency layers**: No circular deps, enforced by Cargo
- **Single process**: All components in one binary, communication via `Arc<MessageBus>`
- **Repository pattern**: All state via typed repos backed by `SqlitePool` + LanceDB
- **Dependency inversion**: Handler traits prevent circular deps between tool ↔ agent layers

### Architectural Philosophy Comparison

| Dimension | openclaw | klyntbot |
|-----------|----------|----------|
| **Modularity model** | Plugin/npm packages | Cargo workspace crates |
| **Extension boundary** | npm plugin API (public) | Source code (internal) |
| **Communication** | WebSocket, event emitters, streams | `Arc<MessageBus>` (tokio mpsc) |
| **State sharing** | In-process + SQLite | SqlitePool + LanceDB (Arc-backed) |
| **Process model** | Single Node.js process (multi-process optional via ACP) | Single Rust process |
| **Startup time** | ~2-5s (Node boot + channel init) | ~200ms (Rust binary) |
| **Memory footprint** | ~150-300MB (Node.js baseline) | ~50-150MB (Rust, no GC) |

---

## 5. Language, Runtime & Tech Stack

### openclaw

```
Language:     TypeScript (strict, no implicit any)
Runtime:      Node.js 22+ (ESM modules)
Package mgr:  pnpm (workspaces)
Build tool:   tsdown (TypeScript compiler alternative)
Test runner:  Vitest + V8 coverage (70% threshold)
Key deps:
  - @agentclientprotocol/sdk 0.14.1  (ACP protocol)
  - @mariozechner/pi-*         0.54.1 (agent core from PI project)
  - @sinclair/typebox          0.34.48 (JSON schema generation)
  - express                    5.2.1  (HTTP gateway)
  - ws                         8.19.0 (WebSocket)
  - croner                     10.0.1 (cron scheduling)
  - playwright-core                   (browser automation)
  - better-sqlite3                    (memory storage)
  - node:sqlite (native)              (embedding cache)
  - pino                              (structured logging)
  - zod                               (config validation)
```

### klyntbot

```
Language:     Rust (Edition 2021)
Runtime:      Native binary (compiled, no runtime dependency)
Build tool:   Cargo (workspace)
Test runner:  cargo-nextest (parallel) + cargo test (doctests)
Linting:      cargo clippy (zero warnings enforced)
Key deps:
  - tokio                 1.49    (async runtime)
  - sqlx                  0.8     (SQLite + auto-migrations)
  - lancedb              0.26     (vector storage)
  - serde / serde_json            (serialization)
  - reqwest               0.13    (HTTP client)
  - tokio-tungstenite             (WebSocket)
  - fastembed             5       (local embedding inference)
  - extism                1       (WASM plugin runtime)
  - dashmap                       (concurrent hash map)
  - async-trait                   (trait objects)
  - clap                          (CLI)
  - chrono                        (date/time)
```

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Type safety** | TypeScript (good, erasure at runtime) | Rust (excellent, compile-time guarantees) |
| **Memory safety** | GC + V8 engine | Ownership system (no GC, no null, no data races) |
| **Concurrency model** | Event loop + Worker threads | Tokio async tasks (green threads) |
| **Binary size** | N/A (Node.js runtime required) | ~50MB stripped release |
| **Cold start** | ~2-5s | ~200ms |
| **Runtime deps** | Node.js 22+ (installed separately) | None (statically linked) |
| **Ecosystem** | npm (2M+ packages) | crates.io (150k+ crates) |
| **Async model** | Promise + async/await + EventEmitter | Future + async/await + tokio mpsc |
| **Error handling** | throw/catch + Result types | `Result<T, E>` everywhere (no exceptions) |
| **JSON schema** | TypeBox (runtime schema gen) | `serde_json::Value` + manual |

**Assessment**: Rust gives klyntbot hard guarantees on memory safety, thread safety, and binary size. TypeScript gives openclaw access to the npm ecosystem, faster iteration, and easier plugin contribution. Both are appropriate choices for their respective goals.

---

## 6. Agent Loop & Orchestration

### openclaw Agent Loop

openclaw's agent loop is built on the `@mariozechner/pi-*` (PI project) library — an external dependency forming the agent core:

```
Inbound (channel/ACP/cron)
  ↓
Gateway WebSocket → GatewayClient
  ↓
Session resolution + auth check
  ↓
Agent Core (PI-based)
  ├─ Context assembly (history + tools + system prompt)
  ├─ LLM call (streaming SSE)
  ├─ Tool call extraction
  ├─ Tool execution loop
  └─ Response delivery
  ↓
Outbound → Channel → User
```

**Characteristics**:
- **No explicit strategy routing**: Single agent loop handles all message types
- **Session-scoped context**: JSONL-backed conversation history
- **Compression**: Native LLM compaction (no external summarizer by default)
- **Subagent support**: Via spawning isolated sessions; `countActiveDescendantRuns()` prevents premature exit
- **Tool call loop**: Continues until no more tool calls or max iterations

### klyntbot Agent Loop

klyntbot's `AgentLoop` is custom-built with a sophisticated two-stage orchestration layer:

```
InboundMessage (channel → MessageBus)
  ↓
AgentLoop::process_message()
  ↓
AgentPipeline::process_message()
  ├─ 1. Orchestrator::classify()
  │    ├─ Heuristic pre-filter (keyword matching, ~70% cases, zero LLM cost)
  │    └─ LLM classifier fallback (30% ambiguous cases, confidence gate at 0.5)
  │    Returns: DirectResponse | ToolAssisted | AutonomousTask | Clarification
  │
  ├─ 2. ContextEngine::assemble()
  │    ├─ BudgetAllocator (strategy-aware token splits)
  │    ├─ HistoryCompressor (extractive / abstractive / sliding)
  │    ├─ MemoryRetriever (embedding-based relevance filtering)
  │    └─ Context sources: Bootstrap, Goals, Memory, Skills, Todos, Confidence
  │
  ├─ 3. EngineDispatch::execute()
  │    ├─ DirectEngine — single LLM call
  │    ├─ ReactPlusEngine — multi-cycle ReAct (up to 10 iters, reflection, escalation)
  │    ├─ PlanGenerateEngine — structured plan decomposition
  │    └─ ExecutionCore — plan step execution with backtracking
  │
  ├─ 4. ResponseValidator::validate()
  ├─ 5. CostTracker::record()
  └─ 6. StrategyPersistence::record()
  │    └─ Writes StrategyRecordRow (predicted/actual strategy, escalation,
  │       iterations_used, response_time_ms, chat_id)
  ↓
OutboundMessage → MessageBus → Channels
  ↑ (feedback loop)
  └─ User reactions (👍/👎) → satisfaction score → backfilled to strategy record
```

**Characteristics**:
- **Adaptive routing**: Strategy selection based on message heuristics + LLM confidence
- **Budget-aware context**: Token allocation varies by execution strategy
- **Multiple execution engines**: Different loop behaviours for different task types
- **Active learning loop**: Pipeline Step 6 persists every strategy outcome (`StrategyRecordRow` with predicted/actual strategy, escalation count, iterations used, response time, chat_id). The orchestrator reads these records to calibrate future classification decisions. User reactions (emoji on channel messages) backfill a satisfaction score to the most recent strategy record via `set_satisfaction_for_chat()`.
- **Hierarchical sub-agents**: `SubagentProfile` enum (General, Research, Code, Analyst) with profile-based tool registration and iteration limits. LLM selects profile via `SpawnTool` parameter. Each profile gets a role-specific system prompt and restricted tool set (e.g., Analyst gets read-only filesystem, no shell/web).
- **Planning integration**: Plan execution steps have their own cycle with backtracking. `PlanCompletionHandler` increments typed goal columns (`plans_completed`, `plans_failed`, `avg_duration_ms`) on plan completion.

### Comparison

| Dimension | openclaw | klyntbot |
|-----------|----------|----------|
| **Orchestration** | Single unified loop (PI library) | Two-stage adaptive routing |
| **Strategy routing** | None (same path for all messages) | Heuristic + LLM classifier with confidence gating |
| **Context assembly** | History + tools (PI-managed) | 6-source assembly with budget allocation |
| **Execution engines** | 1 (single agent loop) | 4 (Direct, ReAct+, PlanGenerate, ExecutionCore) |
| **Token budget management** | Implicit (PI library handles) | Explicit per-strategy budget splits |
| **History compression** | Native LLM compaction | 3 modes: extractive, abstractive, sliding |
| **Adaptive learning** | None | Active loop: strategy persistence → classifier feedback → satisfaction backfill |
| **Subagent spawning** | Yes (isolated sessions, descendant tracking) | Yes (SpawnHandler, 4 profiles: General/Research/Code/Analyst) |
| **User satisfaction** | None | Emoji reactions → satisfaction score on strategy records |

**klyntbot advantage**: Sophisticated orchestration, adaptive routing, and multiple execution engines.
**openclaw advantage**: Less opinionated loop — easier to extend without modifying the core.

---

## 7. Channel Integrations

### openclaw Channels

**8 core channels** + extension ecosystem (Matrix, Teams, Zalo, BlueBubbles, etc.):

```typescript
// Channel plugin interface
type ChannelPlugin = {
  id: ChannelId;
  meta: ChannelMeta;
  capabilities: ChannelCapabilities;  // polls, reactions, edit, unsend, threads, media, etc.
  config: ChannelConfigAdapter;
  setup?: ChannelSetupAdapter;
  pairing?: ChannelPairingAdapter;    // Approval-based allowlist
  security?: ChannelSecurityAdapter;  // DM policy enforcement
  groups?: ChannelGroupAdapter;
  outbound?: ChannelOutboundAdapter;  // send modes: direct/gateway/hybrid
  gateway?: ChannelGatewayAdapter;    // start/stop/processMessage lifecycle
  streaming?: ChannelStreamingAdapter;
  // ... 12 adapter types total
};
```

| Channel | Approach | Notes |
|---------|----------|-------|
| **Telegram** | Bot API | Best supported |
| **WhatsApp** | Web client (QR link) | Separate phone + eSIM recommended |
| **Discord** | Bot API + @buape/carbon | Slash commands, embeds, reactions |
| **Signal** | signal-cli linked device | CLI-based |
| **Slack** | Socket Mode | Full Bolt-style |
| **iMessage** | imsg CLI (JSON-RPC 2.0) | macOS only, stdio pipe |
| **Google Chat** | HTTP webhook | Chat API |
| **IRC** | Server + Nick | DM + channel routing |
| **LINE** | Bot SDK + webhook | Flex messages, postbacks |

**Message flow**:
- All channels normalise to `MsgContext` → `FinalizedMsgContext`
- `ReplyDispatcher` handles rate limiting, typing indicators, batching
- Channel-specific markdown rendering (WhatsApp: bold/monospace, LINE: Flex messages, Discord: embeds)
- Pairing system for sender approval (8-char code, per-channel JSON allowlist)

### klyntbot Channels

**6 channels**, defined by the `Channel` trait:

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;
    async fn send_typing(&self, chat_id: &str) -> Result<()>;
}
```

| Channel | Approach | Notes |
|---------|----------|-------|
| **Telegram** | Bot API (polling) | Markdown + HTML |
| **Discord** | WebSocket (tokio-tungstenite) | Embeds, file attachments |
| **Slack** | Bolt-style | Threading, reactions |
| **WhatsApp** | Twilio API | Template messages, rate limits |
| **QQ** | OneBot v11 protocol | Group + private |
| **Email** | IMAP/SMTP (lettre) | Feature-gated, optional |

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Channel count** | 8 core + extensions | 6 |
| **Channel interface** | 12-adapter plugin (rich capability matrix) | 5-method trait (simpler) |
| **iMessage** | ✅ (imsg CLI, macOS) | ❌ |
| **Signal** | ✅ | ❌ |
| **IRC** | ✅ | ❌ |
| **LINE** | ✅ (Flex messages) | ❌ |
| **Google Chat** | ✅ | ❌ |
| **Email** | ❌ (not a channel) | ✅ (feature-gated) |
| **QQ** | ❌ | ✅ |
| **Sender approval / pairing** | ✅ Full system (8-char codes, allowlists, DM policy) | Basic `is_allowed()` check |
| **Node pairing (devices)** | ✅ (iOS, Android, macOS paired) | ❌ |
| **Channel capability matrix** | ✅ (polls, reactions, edit, unsend, threads...) | ❌ (uniform trait) |
| **Markdown rendering** | Per-channel (WhatsApp/LINE Flex/Discord embeds) | Basic per-channel formatting |
| **Rate limiting** | Built-in (ReplyDispatcher) | Basic (channel-level) |

**openclaw advantage**: More channels, richer capability model, pairing system, platform-specific rich formatting.
**klyntbot advantage**: Email as first-class channel; QQ (Chinese market); simpler trait easier to implement new channels.

---

## 8. Tools System

### openclaw Tools

openclaw tools are **channel-attached** or **agent-registered** via the plugin SDK:

```typescript
type AnyAgentTool = {
  name: string;
  description: string;
  parameters: TSchema;    // TypeBox JSON schema
  execute: (params: unknown, ctx: ToolContext) => Promise<ToolResult>;
};

// Registration via plugin API
api.registerTool(tool, { scope?: "session" | "global" });

// Or channel-specific tools
channel.agentTools?: ChannelAgentTool[];
```

**Built-in tools** (from bundled plugins/skills):
- bash/shell execution
- file read/write
- web fetch + search
- message send (cross-channel)
- memory read/write
- cron management
- browser control (Playwright-based)
- canvas rendering
- media analysis
- link extraction
- subagent spawning

Tools are **schema-validated** via TypeBox before execution.

### klyntbot Tools

klyntbot tools implement the `Tool` trait (Layer 3):

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;            // JSON schema
    async fn execute(&self, action: &str, params: Value, ctx: &RoutingContext) -> Result<ToolResponse>;
}
```

**13+ core tools**:

| Tool | Actions | Notes |
|------|---------|-------|
| `FilesystemTool` | read_file, write_file, list_dir, delete_file | Path expansion, parent dir creation |
| `ShellTool` | run_command, kill_process | Timeout enforcement |
| `WebTool` | fetch_page, query_api | HTML→text conversion |
| `MessageTool` | send | Cross-channel via bus |
| `SpawnTool` | spawn_subagent | Profile-based (general/research/code/analyst) |
| `GoalTool` | create, show, list, update, metrics | Typed plan completion tracking |
| `CronTool` | schedule, list_jobs, cancel_job | Delegates to CronHandler |
| `AskUserTool` | ask | Blocks on interaction_rx |
| `TodoTool` | add, get, update, delete, search, search-semantic, search-hybrid | Full CRUD + RRF |
| `ProjectTool` | create, list, update, delete | Project management |
| `PlanTool` | create, approve, execute, status | Plan lifecycle |
| `CalendarTool` | sync_now, list_events, get_status | Delegates to CalendarHandler |
| `EnrichmentTool` | enrich | AI-powered task field inference |
| `BrowserTool` | navigate, snapshot, click, type, fill, scroll, screenshot, eval, fill_form, login_flow, submit_and_confirm | agent-browser subprocess, trust-level write guards |

**Dependency inversion**: `SpawnHandler`, `CronHandler`, `CalendarHandler`, `EnrichmentHandler`, `PlanHandler`, `PlanCompletionHandler`, `GoalHandler` — all defined in `tools` (Layer 3), implemented in `agent` (Layer 5), injected at construction as `Arc<dyn Trait>`. The `SpawnHandler` now carries a `profile: String` parameter that gets converted to `SubagentProfile` at the trait implementation boundary.

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Tool interface** | TypeBox-validated async function | Rust trait with `execute(&action, params, ctx)` |
| **Schema generation** | TypeBox (runtime, typed) | `serde_json::Value` (manual JSON schema) |
| **Tool count (built-in)** | ~15 (via bundled plugins) | 13+ core tools |
| **Tool discovery** | Plugin load time | `ToolRegistry::get_definitions()` |
| **Browser tool** | ✅ (Playwright, full CDP) | ✅ (agent-browser, trust-level guards) |
| **Media tool** | ✅ (multi-provider vision) | ❌ |
| **Calendar tool** | ❌ | ✅ (CalDAV + Apple/Google Cal) |
| **Task/todo tool** | ❌ (no dedicated tool) | ✅ (full CRUD + semantic search) |
| **Plan tool** | ❌ | ✅ (full lifecycle) |
| **Enrichment tool** | ❌ | ✅ (AI-inferred priority/duration/deadline) |
| **Ask user tool** | ✅ (interactive prompts) | ✅ (blocks on interaction_rx) |
| **Extensibility** | Plugin API (npm) | WASM plugins (Extism) + source code |
| **Dependency injection** | Plugin constructor injection | `Arc<dyn Trait>` at loop construction |

---

## 9. Memory & Storage

### openclaw Storage

**Three-tier storage**:

1. **SQLite (node:sqlite)** — Memory index:
   ```sql
   CREATE TABLE chunks (
     id TEXT, path TEXT, startLine INT, endLine INT,
     content TEXT, embedding BLOB,  -- float32 array
     model TEXT, dims INT
   );
   CREATE VIRTUAL TABLE chunks_fts USING fts5(content, content='chunks');
   ```
   - Full-text search via FTS5 (BM25 ranking)
   - Vector search via optional `sqlite-vec` extension
   - Embedding cache table for provider cost reduction

2. **JSONL files** — Session transcripts:
   - Location: `~/.openclaw/agents/{agentId}/sessions/{sessionKey}.jsonl`
   - Newline-delimited JSON for message history
   - Append-only, portable

3. **Filesystem** — Workspace memory:
   - Markdown files tracked by chokidar
   - Indexed on write via memory index manager
   - Source: `"memory"` (files) or `"sessions"` (transcripts)

**Embedding providers** (6 options):
- OpenAI: text-embedding-3-small/large, ada-002
- Gemini: text-embedding-004
- Voyage AI: voyage-3, voyage-large-2
- Mistral: mistral-embed
- Local: node-llama-cpp (embeddinggemma-300m-qat-q8_0)
- Auto: detect from available API keys

**Search modes**:
- FTS5 BM25 (keyword)
- sqlite-vec cosine similarity (vector)
- Hybrid: RRF(BM25, cosine) — same RRF formula as klyntbot

### klyntbot Storage

**SQLite + LanceDB** (zero infrastructure):

```
~/.klyntbot/
├── data.db              ← SQLite (all relational data, WAL mode)
├── lance/               ← LanceDB (vector embeddings)
│   ├── todo_embeddings.lance/
│   ├── conv_embeddings.lance/
│   └── memory_note_embeddings.lance/
└── config.json
```

**24 repository types** in `storage` crate:
- Core: `TodoRepo`, `ProjectRepo`, `SessionRepo`, `CronRepo`, `UsageRepo`
- Planning: `PlanRepo`, `GoalRepo`, `StrategyRepo`, `OutcomeRepo`, `DecisionLogRepo`
- Memory: `MemoryNoteRepo`, `LearningStateRepo`
- Calendar: `CalendarEventCacheRepo`, `CalendarSyncRepo`
- Finance: `FinanceAccountRepo`, `FinanceBudgetRepo`, `FinanceGoalRepo`
- Vector: `VectorStore` (LanceDB — todo, conversation, memory note embeddings)

**SqlitePool pattern**:
- `SqlitePool` is `Clone + Send + Sync` (Arc-backed by sqlx internally)
- All repos hold `SqlitePool`, clone freely — no `Arc<RwLock<>>` needed
- Auto-migrations on `StoragePool::connect(data_dir)` via `sqlx::migrate!()`
- WAL mode + foreign keys enabled at connection time

**Semantic search** (LanceDB ANN):
- 384-dimension vectors via fastembed (paraphrase-multilingual-MiniLM-L12-v2)
- Approximate nearest neighbor search with cosine similarity
- Hybrid search: keyword (SQL) + semantic (LanceDB) merged via RRF

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Session storage** | JSONL flat files | SQLite (sessions table) |
| **Memory/embedding storage** | SQLite (node:sqlite) | SQLite + LanceDB |
| **FTS search** | FTS5 BM25 | SQLite full-text search |
| **Vector search** | sqlite-vec (optional extension) | LanceDB ANN (cosine similarity) |
| **Hybrid search** | ✅ RRF (FTS5 + vector) | ✅ RRF (keyword + LanceDB) |
| **Embedding providers** | 5 cloud + 1 local | 1 local only (fastembed, 384d) |
| **No-cloud embedding** | ✅ (node-llama-cpp) | ✅ (fastembed, default) |
| **Task/project storage** | None (no task management) | Full schema: todos, projects, plans, goals |
| **Repository pattern** | MemoryIndexManager | 24 typed repos, `Repos` aggregate |
| **Migration strategy** | Auto on first access (schema versioning) | Auto via `sqlx::migrate!()` |
| **Portability** | ✅ (SQLite file, copyable) | ✅ (SQLite + LanceDB files, copyable) |
| **Infrastructure requirement** | None (SQLite bundled) | None (SQLite + LanceDB embedded) |
| **Data richness** | Memory + sessions | 24 domain models (todos, goals, plans, finance...) |

**Both use embedded storage** — no external database required. klyntbot has richer domain models (24 repos). openclaw has more embedding provider options (5 cloud + local).

---

## 10. LLM Provider Support

### openclaw Providers

**6 core providers** + plugin-registered custom providers:

| Provider | API Type | Models | Notes |
|----------|----------|--------|-------|
| OpenAI | openai-completions | gpt-4o, gpt-4-turbo, o1, o3 | Streaming, tool use |
| Anthropic | anthropic-messages | claude-opus-4, sonnet-4, haiku | Extended thinking, vision |
| Google | google-generative-ai | Gemini 2.0/1.5 | Streaming, tool use |
| GitHub Copilot | openai-completions | gpt-4o via enterprise | Enterprise only |
| Ollama | ollama | Any local model | Custom endpoint |
| AWS Bedrock | bedrock-converse-stream | Claude, Llama, Mixtral | AWS SDK auth |

```typescript
type ModelDefinition = {
  id: string;
  api?: "openai-completions" | "anthropic-messages" | "google-generative-ai" | "ollama" | "bedrock-converse-stream";
  reasoning?: boolean;
  input?: ("text" | "image")[];
  cost?: { input?: number; output?: number; cacheRead?: number; cacheWrite?: number };
  contextWindow?: number;
  maxTokens?: number;
};
```

**Model selection**: Config-based default → plugin hook `before_model_resolve` can override.

### klyntbot Providers

**10+ providers** via auto-detection registry:

| Provider | Detection | Notes |
|----------|-----------|-------|
| Anthropic (native) | `claude-*` model name OR `apiBase` contains `anthropic` | Native streaming SSE, extended thinking |
| Anthropic (OpenAI compat) | Same as above + `openai_compat = true` | OpenAI-compatible endpoint |
| OpenAI | `gpt-*`, `o1-*`, `o3-*` model name | Function calling, streaming |
| OpenRouter | `sk-or-*` API key prefix | Routes to 200+ models |
| DeepSeek | `deepseek-*` model OR `deepseek` in base URL | Chinese provider |
| Gemini | `gemini-*` model name | Google, multimodal |
| Groq | `groq` in base URL | Fast inference |
| vLLM | `vllm` in base URL | Self-hosted |
| Zhipu/GLM | `glm-*` model | Chinese market |
| Dashscope/Qwen | `qwen-*` model | Alibaba |
| Moonshot | `moonshot-*` model | Chinese |
| MiniMax | `minimax` in base URL | Chinese |
| AiHubMix | `aihubmix` in base URL | Hub |

**Provider auto-detection** (5-step resolution):
1. Explicit `config.agents.defaults.provider`
2. Model name keyword matching
3. API key prefix (`sk-or-*` → OpenRouter)
4. Base URL keyword matching
5. First provider with non-empty API key

```rust
pub struct ProviderCapabilities {
    pub extended_thinking: bool,
    pub structured_outputs: bool,
    pub prompt_caching: bool,
    pub native_token_counting: bool,
    pub vision: bool,
    pub streaming: bool,
    pub tool_choice_required: bool,
    pub parallel_tool_calls: bool,
}
```

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Built-in providers** | 6 | 10+ |
| **OpenAI** | ✅ | ✅ |
| **Anthropic** | ✅ | ✅ (native + compat) |
| **Google Gemini** | ✅ | ✅ |
| **Ollama (local)** | ✅ | ✅ |
| **AWS Bedrock** | ✅ | ❌ |
| **OpenRouter** | ❌ | ✅ |
| **DeepSeek** | ❌ | ✅ |
| **Groq** | ❌ | ✅ |
| **Zhipu/Qwen** | ❌ | ✅ |
| **Plugin-registered providers** | ✅ (via plugin SDK) | ❌ |
| **Provider auto-detection** | Config + plugin hooks | 5-step keyword/prefix matching |
| **Cost tracking** | Per-model cost fields | `Usage` struct (prompt/completion/cache) |
| **Streaming** | ✅ SSE | ✅ SSE + custom |
| **Extended thinking** | ✅ (Anthropic) | ✅ (`reasoning_content` field) |
| **Vision** | ✅ (model input: ["text","image"]) | ✅ (via `ProviderCapabilities.vision`) |
| **Prompt caching** | ✅ (cost field: cacheRead/Write) | ✅ (cache_read/write_tokens) |

**klyntbot advantage**: More built-in providers, especially Asian market (DeepSeek, Zhipu, Qwen, Moonshot), OpenRouter for 200+ model access.
**openclaw advantage**: AWS Bedrock, plugin-registered custom providers, GitHub Copilot enterprise.

---

## 11. Configuration System

### openclaw Configuration

- **Location**: `~/.openclaw/config.json` (standard) or `.openclaw/config.json` (workspace)
- **Validation**: Zod schema
- **Env override**: `OPENCLAW_*` with `__` nesting
- **Hot reload**: No (restart required)
- **Secret management**: Fields marked `.register(sensitive)`; stored in config, not auto-redacted in logs

```json
{
  "agents": {
    "defaults": {
      "workspace": "~/openclaw",
      "model": "anthropic/claude-opus-4-6",
      "contextTokens": 100000
    }
  },
  "models": {
    "providers": {
      "anthropic": { "apiKey": "sk-...", "models": [...] }
    }
  },
  "gateway": { "bind": "loopback", "port": 18789, "auth": { "token": "..." } },
  "channels": {
    "telegram": { "enabled": true, "botToken": "..." },
    "discord": { "token": "..." }
  },
  "memory": {
    "backend": "builtin",
    "query": { "maxResults": 10, "minScore": 0.5 }
  }
}
```

### klyntbot Configuration

- **Location**: `~/.klyntbot/config.json`
- **Validation**: Manual Rust `Deserialize` impls with defaults
- **Env override**: `KLYNTBOT_*` with `__` nesting
- **Secret management**: API keys wrapped in `Secret<String>` (redacted in Debug/Display)
- **Hot reload**: No (restart `klyntbot serve`)

```json
{
  "dataDir": "~/.klyntbot",
  "agents": {
    "defaults": { "model": "claude-3-5-sonnet-20241022", "temperature": 0.7 }
  },
  "providers": {
    "anthropic": { "apiKey": "sk-...", "native": true }
  },
  "channels": {
    "telegram": { "token": "...", "allowFrom": [] }
  },
  "todo": {
    "creationMode": "ask-first",
    "enrichment": { "enabled": true, "autoApplyThreshold": 0.70 },
    "search": { "enabled": true, "semanticThreshold": 0.5, "rrfK": 60 }
  },
  "packs": {
    "enabled": ["task-management", "productivity", "ai-intelligence", "developer-tools"]
  }
}
```

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Format** | JSON (camelCase) | JSON (camelCase) |
| **Validation** | Zod (runtime, descriptive errors) | Rust serde (compile-time, less descriptive) |
| **Secret redaction** | Manual (`.register(sensitive)`) | `Secret<String>` wrapper (automatic) |
| **Env overrides** | `OPENCLAW_*` with `__` | `KLYNTBOT_*` with `__` |
| **Schema richness** | Deep (memory, auth profiles, node pairing) | Deep (todo, packs, calendar, enrichment) |
| **Workspace-scoped config** | ✅ (`.openclaw/config.json` per project) | ❌ (global only) |
| **Hot reload** | ❌ | ❌ |
| **Multi-profile support** | ✅ (auth profiles for different contexts) | ❌ |

---

## 12. Plugin & Extension System

This is the **largest architectural gap** between the two projects.

### openclaw Plugin System

**Rich, npm-loadable plugin ecosystem**:

```
Plugin discovery (3 locations):
  1. extensions/          — 40+ bundled plugins
  2. ~/.openclaw/plugins/ — user-installed global
  3. .openclaw/plugins/   — workspace-local

Plugin manifest: openclaw.plugin.json
Plugin loader: jiti (handles TypeScript plugins directly)
```

**Plugin API** (`OpenClawPluginApi`):

```typescript
type OpenClawPluginApi = {
  // Registration
  registerTool(tool, opts?)          // Add LLM tool
  registerHook(events, handler, opts?) // Lifecycle events
  registerHttpHandler(handler)       // Custom HTTP routes
  registerHttpRoute(params)          // Named HTTP routes
  registerChannel(registration)      // Add new chat channel
  registerGatewayMethod(method, handler) // Add RPC method
  registerCli(registrar, opts?)      // Add CLI commands
  registerService(service)           // Background daemon
  registerProvider(provider)         // Add LLM provider
  registerCommand(command)           // Add slash command

  // Lifecycle hooks (24 total)
  on(hookName, handler, opts?)
};
```

**24 lifecycle hooks** — fine-grained interception at every stage:

```
before_model_resolve    before_prompt_build    before_agent_start
llm_input               llm_output             agent_end
before_compaction       after_compaction       before_reset
message_received        message_sending        message_sent
before_tool_call        after_tool_call        tool_result_persist
before_message_write    session_start          session_end
subagent_spawning       subagent_delivery_target  subagent_spawned
subagent_ended          gateway_start          gateway_stop
```

**Bundled extensions** (40+): msteams, matrix, zalo, bluebubbles, memory-lancedb, github, search providers, notification integrations, etc.

**Plugin config schema**: Zod-compatible schema per plugin, with UI hints for wizard display.

**Plugin capabilities**: Tools, hooks, channels, CLI commands, HTTP routes, gateway methods, LLM providers, services.

### klyntbot Extension System

**WASM plugin sandbox** (Extism) + feature packs:

1. **WASM Plugins** (runtime-loadable, multi-language):
   - `plugin-runtime` crate (Layer 2.5): Extism runtime, host ABI, `WasmPlugin` wrapper
   - `plugin-sdk` crate: `#[plugin_tool]` macro for Rust authors, typed host bindings
   - Multi-language: Rust, TypeScript (Javy), Python (py2wasm), Go (TinyGo)
   - Permission model: `network`, `storage`, `agent` — explicit per plugin
   - Distribution: registry (`plugins.klyntbot.io`), GitHub releases, local file
   - CLI: `klyntbot plugin install|list|remove|update|search|new|publish`
   - Sandboxed storage: each plugin gets its own table namespace (`plugin_{id}_*`)
   - Cron integration: plugin manifests declare scheduled jobs

2. **Feature packs** (built-in, config-selected):
   ```rust
   pub enum PackTier { Core, Recommended, Optional }
   pub struct Pack { id, tier, skills: Vec<String>, description }
   ```
   7 packs: task-management, productivity, ai-intelligence, developer-tools, finance, weather, skill-creator

3. **Skills** (SKILL.md files, filesystem-loaded):
   - Loaded from `skills/` at startup
   - Filtered by enabled packs
   - Injected into system prompt as capability descriptions

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **External plugin loading** | ✅ (npm, jiti, 3 discovery paths) | ✅ (WASM via Extism, 3 install paths) |
| **Plugin sandboxing** | ❌ (runs in Node process) | ✅ (WASM sandbox, explicit permissions) |
| **Multi-language plugins** | TypeScript only | ✅ (Rust, TypeScript, Python, Go) |
| **Lifecycle hooks** | ✅ 24 hooks | ❌ (tools + cron only) |
| **Plugin can add tools** | ✅ | ✅ (via manifest) |
| **Plugin can add channels** | ✅ | ❌ (source only) |
| **Plugin can add CLI commands** | ✅ | ❌ |
| **Plugin can add LLM providers** | ✅ | ❌ |
| **Plugin can add HTTP routes** | ✅ | ❌ |
| **Plugin config schema** | ✅ (Zod + UI hints) | ✅ (manifest `config_schema` + secrets) |
| **Plugin storage** | Shared (in-process) | ✅ (sandboxed per-plugin tables) |
| **Plugin CLI** | npm install | ✅ (install, list, remove, update, search, new, publish) |
| **Registry distribution** | ✅ (npm) | ✅ (plugins.klyntbot.io + GitHub) |
| **Bundled extensions** | 40+ | N/A (plugins are external) |
| **Feature packs** | ❌ | ✅ (7 packs, wizard selection) |

**openclaw advantage**: Broader plugin capabilities (channels, providers, hooks, HTTP routes). Larger existing npm ecosystem.
**klyntbot advantage**: WASM sandboxing (security), multi-language support, explicit permission model, per-plugin storage isolation.

---

## 13. Skills System

### openclaw Skills

54+ bundled `SKILL.md` files with structured metadata:

```markdown
---
name: model-usage
description: CodexBar CLI wrapper for cost breakdown
metadata:
  openclaw:
    emoji: "📊"
    os: ["darwin", "linux"]
    requires: { bins: ["codexbar"] }
    install:
      - { id: "npm", kind: "npm", package: "codexbar" }
---
# Content...
```

- Skills include **installation instructions** (brew, npm, pip)
- **OS compatibility** metadata
- **Required binaries** declaration
- Loaded by SkillManager at runtime; filtered by enabled state

### klyntbot Skills

10 built-in skills in `skills/{id}/SKILL.md`:

```
todo           — Task management quick reference
todo-yolo      — Rapid task creation with auto-enrichment
todo-party     — Interactive brainstorming task creation
daily-planning — Morning/evening planning prompts
summarize      — Document + conversation summarization
github         — GitHub issue/PR queries + actions
tmux           — tmux session management
skill-creator  — User-defined skill generation
finance        — Personal finance guidance
weekly-report  — Auto-generated weekly summaries
```

Skills are **filtered by feature packs** — only skills from enabled packs are available. Injected into system prompt via `SkillManager::get_skill_context()`.

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Built-in skills** | 54+ | 10 |
| **External skills (user)** | ✅ (workspace skills dir) | ✅ (workspace `~/.klyntbot/skills/`) |
| **Installation metadata** | ✅ (brew/npm/pip instructions) | ❌ |
| **OS compatibility** | ✅ (darwin/linux/win32) | ❌ |
| **Required binaries** | ✅ (declared per skill) | ❌ |
| **Pack-based filtering** | ❌ | ✅ |
| **System prompt injection** | ✅ | ✅ |
| **Skill vs Tool distinction** | ✅ (clearly separate) | ✅ (Skills = prompts, Tools = code) |

---

## 14. Scheduling (Cron)

### openclaw Cron

Built on **croner** library with rich delivery configuration:

```typescript
type CronJob = {
  id: string;
  schedule: string;          // "0 9 * * *" POSIX cron
  payload: {
    kind: "agentTurn" | "systemEvent" | "webhook";
    message?: string;
    model?: string;
    delivery?: {
      channel?: string;      // Which channel to post result to
      to?: string;           // Phone/user ID
      accountId?: string;    // Multi-account routing
      threadId?: string;     // Group chat thread ID
      bestEffort?: boolean;  // Skip if channel unavailable
    };
  };
  enabled: boolean;
  lastRun?: number;
  nextRun?: number;
};
```

**CronService** manages in-memory timers, persistent run logs, subagent followup tracking.

**Isolated agent turns**: Cron triggers isolated sessions (`src/cron/isolated-agent/`) with their own context, subagent tracking, and delivery channel.

**Heartbeat system**: Regular "read HEARTBEAT.md and act on it" turns at configurable intervals (default 30 minutes). Token HEARTBEAT_OK suppresses false-positive replies.

### klyntbot Cron

```rust
pub enum CronSchedule {
    At { at_ms: i64 },                                   // One-time
    Every { every_ms: u64 },                             // Fixed interval
    Cron { expr: String, tz: Option<String> },           // Cron expression
}

pub enum CronPayload {
    agent_turn { message: String },
    deliver { channel: String, to: String, message: String },
    custom { kind: String, data: Value },
}
```

Jobs stored in `CronRepo` (SQLite). Background Tokio task calculates next run times. Fires as `InboundMessage` to the agent loop. Managed via `CronTool` (natural language scheduling through chat) and `CronHandler` dependency injection.

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Cron expression** | ✅ POSIX croner | ✅ Standard cron |
| **One-time scheduled** | ❌ | ✅ `At { at_ms }` |
| **Fixed interval** | Via cron equivalent | ✅ `Every { every_ms }` |
| **Delivery routing** | ✅ (channel, to, threadId, accountId) | Basic (channel + to) |
| **Multi-account delivery** | ✅ | ❌ |
| **Subagent followup** | ✅ (wait for descendant summary) | ❌ |
| **Heartbeat system** | ✅ (HEARTBEAT.md, 30m default, OK token) | Partial (heartbeat crate) |
| **Persistence** | File-based run logs | SQLite (CronRepo) |
| **Natural language scheduling** | ❌ (manual config) | ✅ (CronTool via chat) |
| **Timezone support** | ✅ | ✅ (tz field on Cron schedule) |

---

## 15. Browser Automation

### openclaw Browser Automation

Full **Playwright CDP** integration:

```typescript
// Browser capabilities
- Full page navigation with URL guards
- Screenshot capture (PNG/JPEG, configurable quality)
- Role-based accessibility snapshots (ARIA refs)
- Click, double-click, keyboard, hover, drag
- Form filling, file upload
- Dialog interception (alert/confirm/prompt)
- Download file interception
- Storage manipulation (cookies, localStorage, sessionStorage)
- Network response interception
- Page state tracking (console, errors, network)
- Trace capture for debugging
- Chrome extension relay for additional isolation
```

**Sandbox architecture**:
- Playwright sessions in isolated Docker containers (`Dockerfile.sandbox-browser`)
- Chrome DevTools Protocol (CDP) over WebSocket
- Browser profile isolation per agent session
- SSRF policy enforcement on navigation
- noVNC web UI for visual debugging (port 6080)
- VNC access (port 5900)

**Docker sandbox tiers**:
1. `Dockerfile.sandbox` — minimal Linux base (bash, git, python3, ripgrep)
2. `Dockerfile.sandbox-browser` — Chromium + Xvfb + noVNC + websockify
3. `Dockerfile.sandbox-common` — polyglot (Node, Go, Rust, Bun, Homebrew)

### klyntbot Browser Automation

**`BrowserTool`** wraps the `agent-browser` CLI (subprocess pattern, same as `ExecTool`):

- **13 actions**: navigate, snapshot, click, type, fill, press, scroll, wait, get_text, screenshot, eval, fill_form, login_flow, submit_and_confirm
- **Semantic element refs**: `@e1`, `@e2` — 93% token reduction vs raw Playwright
- **Trust-level write guard**: `Strict` (confirm everything), `Autonomous` (confirm dangerous actions), `Full` (no guards)
- **Composite helpers**: `fill_form` (multi-field), `login_flow` (auth pages), `submit_and_confirm` (always guarded)
- **Feature-gated**: opt-in via `config.tools.browser.enabled`, init wizard handles binary detection

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Browser control** | ✅ Full Playwright/CDP | ✅ agent-browser CLI (semantic refs) |
| **Screenshots** | ✅ | ✅ |
| **Form filling** | ✅ | ✅ (fill_form composite helper) |
| **JavaScript execution** | ✅ | ✅ (eval action) |
| **Trust-level guards** | ❌ | ✅ (Strict/Autonomous/Full) |
| **Docker sandbox** | ✅ 3-tier | ❌ (runs with user permissions) |
| **Visual debugging (noVNC)** | ✅ | ❌ |
| **SSRF protection** | ✅ (URL guard + policy) | ❌ |
| **Token efficiency** | Standard DOM selectors | ✅ 93% reduction via semantic `@e` refs |

**openclaw advantage**: Docker sandbox isolation, SSRF protection, visual debugging.
**klyntbot advantage**: Trust-level write guards with user confirmation, token-efficient semantic element refs.

---

## 16. Media Understanding

### openclaw Media Understanding

Multi-provider vision, audio transcription, and video processing:

```typescript
// Supported media
Images:  PNG, JPEG, WebP → vision model (Claude/GPT-4V/Gemini)
Audio:   MP3, WAV, M4A, OGG → transcription (Deepgram + provider-native)
Video:   MP4, WebM, MOV → frame extraction + description
PDFs:    Via extracted images or custom CLI handlers

// Provider fallback chain
primary → fallback → error (with scope-based skip policies)

// Decision audit trail
type MediaDecision = {
  provider: string;
  model: string;
  reason: "capability" | "scope" | "error";
  timestamp: number;
};
```

**Auto-key-model binding**: If an embedding provider API key is configured, the matching vision model is automatically selected.

**Scope-based filtering**: Media understanding can be restricted per channel, chat type, or session.

### klyntbot Media Understanding

**Not implemented.** No vision model integration, no audio transcription.

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Image understanding** | ✅ (Claude/GPT-4V/Gemini) | ❌ |
| **Audio transcription** | ✅ (Deepgram + native) | ❌ |
| **Video processing** | ✅ (frame extraction) | ❌ |
| **PDF processing** | ✅ | ❌ |
| **Provider fallback** | ✅ | N/A |
| **MIME detection** | ✅ (auto) | ❌ |

---

## 17. Canvas / Generative UI

### openclaw Canvas

Agents can push **interactive HTML/component-based UI** to a canvas layer:

```
Canvas Host:
  - HTTP server (static assets + custom HTML/JS/CSS)
  - WebSocket for real-time agent→UI updates
  - A2UI (Agent-to-UI) framework integration
  - File watcher (chokidar) for live reload
  - Path traversal protection
  - Interactive test page
```

**Use cases**:
- Rich data visualisations beyond text
- Interactive forms / confirmation dialogs
- Real-time dashboards
- Document rendering (PDF, markdown → HTML)

### klyntbot Canvas

**Not implemented.** All output is text/markdown via chat channels.

---

## 18. Planning Engine

### openclaw Planning

**Not implemented.** openclaw has no multi-step plan management, step tracking, backtracking, or plan-to-goal linkage.

### klyntbot Planning Engine

Full 6-state lifecycle with LLM-driven step generation and backtracking:

```rust
// Plan lifecycle
Draft → Approved → Executing → Completed
              ↘                ↘
           Abandoned          Failed

// Validated state machine
impl PlanStatus {
    pub fn validate_transition(from: &PlanStatus, to: &PlanStatus) -> Result<()>
}
```

**Plan structure**:
- Multi-step plans with per-step `description`, `reasoning`, `expected_tools`
- Step status: Pending → Executing → Completed/Failed
- Per-step: `attempt_count`, `max_attempts` (3), `result`
- Backtrack history: `Vec<BacktrackEntry>`

**Execution loop** (per step):
1. Build context window: plan goal + current step + next 3 steps preview + previous 2 results
2. Up to 5 LLM-tool cycles per step
3. On step failure (>3 attempts): backtracking
   - LLM generates replacement steps from failure point
   - Fallback: "Retry: {step}" if LLM returns invalid JSON
   - After 3 full backtrack events: mark plan Failed

**Plan-to-goal linkage**: Plans can be linked to `GoalRepo` entries. Goal metrics (completion_rate, avg_duration) updated on plan completion.

**Natural language management**: PlanTool (`create`, `approve`, `execute`, `status`) — full lifecycle management via chat.

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Multi-step planning** | ❌ | ✅ Full lifecycle |
| **State machine** | ❌ | ✅ 6 states, validated transitions |
| **Step-level retry** | ❌ | ✅ (3 attempts per step) |
| **Backtracking** | ❌ | ✅ (LLM regenerates from failure point) |
| **Goal linkage** | ❌ | ✅ (GoalRepo + metrics) |
| **Plan persistence** | ❌ | ✅ (PlanRepo, SQLite) |
| **Natural language management** | ❌ | ✅ (PlanTool via chat) |

**klyntbot decisive advantage**: Planning engine is a major differentiator. No equivalent in openclaw.

---

## 19. Security Model

### openclaw Security

**TLS and transport security**:
- TLS fingerprint pinning (mTLS, `checkServerIdentity` override)
- Plaintext `ws://` blocked with hard error (CWE-319)
- CSRF protection (Origin header check)

**Auth and access control**:
- Auth token or OAuth2 password required for gateway
- Device auth token storage in `~/.openclaw/device-auth.json`
- Session create rate limiting (120 requests / 10 seconds)
- Prompt size limits (2MB max, CWE-400 DoS prevention)

**Sandbox isolation**:
- Browser runs in Docker container (non-root `sandbox` user)
- File path policy enforcement (inbound/outbound)
- Environment variable filtering in sandbox execution
- SSRF policy checks on all outbound connections
- AppArmor/SELinux supported at deployment layer

**Pairing system**:
- 8-char approval codes for new senders
- Per-channel allowlists
- DM policies: `"pairing"`, `"allow-all"`, `"blocked"`

### klyntbot Security

**Transport**:
- Channels use their respective platform auth (bot tokens, OAuth)
- No TLS or WebSocket server (intended to run behind proxy)

**Config secrets**:
- API keys wrapped in `Secret<String>` (auto-redacted in Debug/Display, access via `.expose()`)
- No plaintext secret logging

**Access control**:
- `Channel::is_allowed(sender_id)` per-channel allowlist
- Email: IMAP/SMTP credential isolation
- Browser trust-level guards (Strict/Autonomous/Full)

**Plugin sandboxing**:
- WASM sandbox via Extism/Wasmtime — plugins cannot access host memory
- Explicit permission model: `network`, `storage`, `agent`
- Per-plugin storage isolation (`plugin_{id}_*` table namespace)
- Permission display at install time before user confirmation

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **TLS pinning** | ✅ | ❌ (no TLS server) |
| **Auth layer** | ✅ (token/OAuth2 gateway) | ❌ (proxy required) |
| **Rate limiting** | ✅ (session creation) | ❌ |
| **Sandbox (Docker)** | ✅ (browser + code execution) | ✅ (WASM sandbox for plugins) |
| **SSRF protection** | ✅ | ❌ |
| **Secret redaction** | Manual | ✅ (Secret<String>) |
| **Sender allowlist** | ✅ (full pairing system) | Basic (`is_allowed()`) |
| **Prompt size limits** | ✅ (2MB) | ❌ |

**openclaw advantage**: Significantly more comprehensive security model, especially for multi-tenant / public-facing deployments.
**klyntbot**: Relies on deployment layer for security. Appropriate for single-operator self-hosted use.

---

## 20. Deployment & Distribution

### openclaw Deployment

**Multiple deployment targets**:

1. **CLI / npm package**:
   ```bash
   npm install -g openclaw
   openclaw gateway --port 18789
   ```

2. **Daemon (OS-native)**:
   - macOS: LaunchAgent (`~/Library/LaunchAgents/ai.openclaw.gateway.plist`)
   - Linux: systemd user service
   - Windows: Scheduled Tasks
   - Auto-restart on crash (`KeepAlive: true` on macOS)

3. **Docker**:
   ```yaml
   # docker-compose.yml
   services:
     openclaw-gateway:
       image: openclaw-gateway
       ports: ["18789:18789"]
       volumes:
         - ~/.openclaw:/root/.openclaw
   ```

4. **Mobile apps**:
   - iOS (SwiftUI) — native app, WebSocket to gateway
   - Android (Jetpack Compose) — native app
   - macOS (Cocoa) — native menubar app

**External dependencies**:
- Node.js 22+ (runtime)
- SQLite (bundled, no separate install)
- Optional: PostgreSQL-equivalent (only for custom integrations)

### klyntbot Deployment

**Single binary**:
```bash
# Build
cargo build --release  # ~50MB stripped binary

# Deploy
./klyntbot serve       # Start gateway + channels
./klyntbot chat        # Interactive REPL
./klyntbot status      # Health check
./klyntbot plugin      # Install/list/remove WASM plugins
```

**External dependencies**:
- None. SQLite + LanceDB are embedded. Data stored at `~/.klyntbot/`.

**No daemon management**: Must use external supervisor (systemd, launchd, Docker, pm2) for auto-restart.

**No mobile apps**.

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Single binary** | ❌ (Node.js required) | ✅ (~50MB, statically linked) |
| **Daemon management** | ✅ (launchd/systemd/schtasks) | ❌ (manual/external) |
| **Docker support** | ✅ (full Dockerfile + compose) | Partial (no official Dockerfile) |
| **Mobile apps** | ✅ iOS + Android + macOS | ❌ |
| **Cold start** | ~2-5s | ~200ms |
| **Runtime requirement** | Node.js 22+ | None (binary) |
| **Database requirement** | SQLite (bundled) | None (SQLite + LanceDB embedded) |
| **Memory footprint** | ~150-300MB | ~50-150MB |
| **ARM/edge support** | Partial (Node.js ARM) | ✅ (cross-compile via Cargo) |
| **Auto-update** | npm / macOS app | Manual binary replacement |

**openclaw advantage**: OS daemon management, mobile apps, npm ecosystem distribution, auto-restart.
**klyntbot advantage**: Single binary (no runtime), zero infrastructure (no DB server), lower memory, faster startup, true cross-compilation.

---

## 21. Testing Architecture

### openclaw Testing

```
Framework:    Vitest + V8 coverage
Threshold:    70% (lines, branches, functions — enforced in CI)
Pre-commit:   prek install (git hooks)
```

**Test tiers**:
1. **Unit**: `*.test.ts` colocated with source
2. **Integration**: Multi-module workflows, gateway HTTP API tests
3. **E2E**: `test:docker:live-models`, `test:docker:live-gateway`, `test:docker:onboard`
4. **Live API**: `LIVE=1 pnpm test:live` with real credentials

**Mocking strategy**:
- Browser: Mock `playwright-core` with fake CDP
- Gateway: In-memory `GatewayClient` mock
- LLM providers: Stub response fixtures
- Channels: Mock Discord/Telegram APIs

### klyntbot Testing

```
Framework:     cargo-nextest (parallel) + cargo test (doctests)
Linting:       cargo clippy --workspace --all-targets (0 warnings)
Formatting:    cargo fmt --all --check
DB tests:      DATABASE_URL=postgres://localhost/klyntbot_test
```

**Test tiers**:
1. **Unit**: `#[cfg(test)] mod tests` inline in each crate
2. **Integration**: `tests/` root directory, full agent→tool→storage→provider
3. **Pattern matching**: `cargo nextest run -E 'test(session_persistence)'`

**Mock provider**: `tests/mock_provider.rs` (shared mock `LlmProvider` across all integration tests)

**No database requirement**: All tests use ephemeral SQLite (`StoragePool::connect_in_memory()`).

### Comparison

| Aspect | openclaw | klyntbot |
|--------|----------|----------|
| **Coverage enforcement** | ✅ 70% threshold | No threshold (clippy only) |
| **Parallel test execution** | ✅ Vitest workers | ✅ cargo-nextest |
| **E2E with Docker** | ✅ | ❌ |
| **Live API tests** | ✅ | Limited |
| **Mock LLM provider** | ✅ | ✅ (mock_provider.rs) |
| **Zero-warning linting** | ✅ (TypeScript strict) | ✅ (clippy zero warnings) |
| **Doctest support** | ❌ (no doctests in TS) | ✅ (cargo test --doc) |
| **DB requirement for tests** | ❌ (SQLite in-memory) | ❌ (SQLite in-memory) |

**openclaw advantage**: Docker-based E2E tests, coverage threshold enforcement, no external DB for unit tests.
**klyntbot advantage**: Doctests, nextest filtering, compile-time correctness (fewer runtime surprises).

---

## 22. Remaining Gap Analysis

All four original klyntbot gaps have been closed: plugin system (WASM/Extism), browser automation (agent-browser), storage infrastructure (SQLite+LanceDB), and learning loop + sub-agents (strategy persistence, 4 profiles, satisfaction feedback).

### What klyntbot still needs

| Gap | Priority | Notes |
|-----|----------|-------|
| **Media understanding (vision/audio)** | High | No vision model integration, no audio transcription |
| **More channels (iMessage, Signal, LINE, IRC)** | High | 6 vs 8+ channels |
| **Pairing / sender approval system** | High | Simple allowlist only, no approval codes |
| **Generative UI (Canvas)** | Medium | Text-only output |
| **OS daemon management (launchd/systemd)** | Medium | Must use external supervisor |
| **Mobile apps (iOS/Android)** | Medium | No native clients |
| **Docker sandbox for browser** | Medium | Browser runs with user permissions (no SSRF protection) |
| **Plugin lifecycle hooks** | Low | WASM plugins can add tools + cron, but no 24-hook interception |
| **Plugin-added channels/providers** | Low | Plugins limited to tools; channels/providers require source |
| **Coverage threshold enforcement** | Low | CI config change |

### What openclaw still needs

| Gap | Priority | Notes |
|-----|----------|-------|
| **Multi-step planning engine** | High | No plan management or goal tracking |
| **Task/todo management** | High | No native todo/project system |
| **Adaptive orchestration** | Medium | Single agent loop, no strategy routing or learning feedback |
| **CalDAV calendar integration** | Medium | No calendar sync |
| **Finance domain tools** | Low | No built-in domain tooling |

---

## 23. Summary Scorecard

| Category | openclaw | klyntbot | Winner |
|----------|----------|----------|--------|
| **Language & Runtime** | TypeScript/Node | Rust/Binary | Tie |
| **Agent orchestration** | Single loop | Adaptive 4-engine + learning loop | klyntbot ✅✅ |
| **Planning engine** | ❌ | Full lifecycle | klyntbot ✅✅ |
| **Task/goal management** | ❌ | Rich (24 repos) | klyntbot ✅✅ |
| **Channel count** | 8+ | 6 | openclaw ✅ |
| **Channel richness** | Full capability matrix | Simple trait | openclaw ✅ |
| **Storage** | SQLite + JSONL | SQLite + LanceDB | Tie |
| **Embedding providers** | 6 (cloud+local) | 1 (local only) | openclaw ✅ |
| **LLM provider breadth** | 6 | 10+ | klyntbot ✅ |
| **Plugin system** | npm SDK (24 hooks) | WASM sandbox (multi-lang) | Tie (different strengths) |
| **Browser automation** | Playwright + Docker sandbox | agent-browser + trust guards | Tie (different strengths) |
| **Media understanding** | Multi-provider | ❌ | openclaw ✅✅ |
| **Generative UI** | Canvas host | ❌ | openclaw ✅ |
| **Calendar integration** | ❌ | CalDAV full | klyntbot ✅ |
| **Security model** | TLS, Docker, SSRF, rate limits | WASM sandbox, trust guards | openclaw ✅ |
| **Deployment** | Node + daemon mgmt + mobile | Single binary, zero infra | Tie |
| **Mobile apps** | iOS + Android + macOS | ❌ | openclaw ✅✅ |
| **Adaptive orchestration** | ❌ | Two-stage + active learning + satisfaction | klyntbot ✅✅ |
| **Hierarchical sub-agents** | Partial (isolated sessions) | 4 profiles + tool restriction | klyntbot ✅ |
| **Infrastructure requirement** | Node.js 22+ | None | klyntbot ✅ |
| **Startup / footprint** | ~3-5s / ~300MB | ~200ms / ~50MB | klyntbot ✅ |

**Overall**: The gap has narrowed significantly. openclaw leads in ecosystem breadth (mobile, canvas, media, channels). klyntbot leads in architectural depth (planning, active learning loop, hierarchical sub-agents, domain models) and operational simplicity (zero infrastructure, single binary, WASM sandboxing). The learning loop closes what was previously a write-only strategy system — klyntbot now persists every strategy outcome, feeds it back to the orchestrator, and incorporates user satisfaction signals from emoji reactions across Telegram, Discord, and Slack.
