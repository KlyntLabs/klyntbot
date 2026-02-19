# Klyntbot Architecture Overview

> Auto-generated from codebase analysis on 2026-02-19. 105K lines of Rust across 17 crates.

## What Is Klyntbot?

A Rust AI agent framework — a single binary that connects to 6+ chat platforms, calls LLMs, executes tools, manages tasks/projects, syncs with Apple Calendar, and maintains persistent memory. All persistent state is stored in PostgreSQL (with pgvector for embeddings).

## Workspace Layout (17 crates, 7 dependency layers)

```
Layer 0: common              — Error types, MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus,        — Config schema (camelCase JSON), async message bus (tokio::mpsc)
         heartbeat
Layer 1.5: storage           — PostgreSQL pool, auto-migrations, row structs, repository pattern
Layer 2: providers, session, — LLM HTTP client, session persistence, cron service,
         scheduling,           CalDAV sync engine, goal/plan state machines
         calendar, goal, plan
Layer 3: context_engine,     — Token budget allocator, tool trait + 15+ implementations,
         tools, channels       6 chat platform integrations
Layer 4: agent               — Agent loop, pipeline, orchestrator, execution engines,
                               context builder, memory, skills, subagents, learning
Layer 5: cli, dashboard      — CLI (4 commands), web dashboard (GraphQL + WebSocket)
Root:    klyntbot            — Re-export facade + binary entry point
```

Dependencies flow **strictly upward**. No circular dependencies — enforced by Cargo.

## Crate Size Distribution

| Crate | Files | % | Purpose |
|-------|-------|---|---------|
| agent | 56 | 20% | Core orchestration |
| cli | 44 | 16% | CLI & REPL |
| storage | 41 | 15% | PostgreSQL persistence |
| tools | 29 | 11% | Tool implementations |
| dashboard | 20 | 7% | Web UI (GraphQL) |
| common | 15 | 5% | Foundation types |
| calendar | 12 | 4% | CalDAV sync |
| channels | 9 | 3% | Platform integrations |
| providers | 7 | 3% | LLM abstraction |
| Other 8 | 40 | 16% | Config, scheduling, etc. |
| **Total** | **273** | **100%** | |

## Message Journey (End-to-End)

```
User Input → Channel/CLI
    ↓
InboundMessage → MessageBus (tokio::mpsc)
    ↓
AgentLoop::run() consumes from bus
    ↓
SessionManager: load/create session, add user message
    ↓
ContextBuilder: build system prompt + history + context
    ↓
Pipeline: Orchestrator → ContextEngine → EngineDispatch → Validator → CostTracker
    ↓
LLM Call: provider.chat() with tool definitions
    ↓
Tool Execution: parallel via ToolRegistry (up to 20 iterations)
    ↓
Final Response → Session save → OutboundMessage → Channel delivery
```

## Key Architectural Patterns

### 1. Dependency Inversion (11 handler traits)
Traits defined in `tools` (Layer 3), implemented in `agent` (Layer 4). Injected as `Arc<dyn Trait>`:

| Trait | Purpose | Implementor |
|-------|---------|-------------|
| SpawnHandler | Subagent spawning | SubagentManager |
| CronHandler | Cron scheduling | CronHandlerAdapter |
| CalendarHandler | Calendar sync | CalendarSyncAdapter |
| EnrichmentHandler | Task auto-fill | EnrichmentEngine |
| EmbeddingHandler | Semantic embeddings | EmbeddingEngineImpl |
| GoalHandler | Strategic goals | GoalHandlerImpl |
| PlanHandler | Plan execution | PlanHandlerImpl |
| PlanCompletionHandler | Plan callbacks | PlanCompletionHandlerImpl |
| LearningHandler | Adaptive learning | LearningHandlerImpl |
| EnrichmentFeedbackHandler | Feedback loop | (in learning) |
| ConversationEmbeddingHandler | Conv. search | ConversationEmbeddingHandlerImpl |

### 2. Repository Pattern (12 repos)
All persistent state goes through `*Repo` structs in `storage`. Repos hold `PgPool` (Clone+Send+Sync).

### 3. Adaptive Pipeline
```
Orchestrator (intent classify) → ContextEngine (budget) → EngineDispatch (execute)
    → ResponseValidator → CostTracker
```

### 4. Streaming Architecture (CLI)
```
StreamingHandle {
    event_rx: AgentEvent stream (content chunks, tool events)
    interaction_rx: ask_user prompts (blocks on oneshot response)
    cancel_token: CancellationToken (Ctrl+C)
    handle: JoinHandle<Result<String>>
}
```

## Feature Surface

- **15+ Tools**: filesystem (4), shell, web (2), message, spawn, cron, todo (24 actions), project, plan, goal, calendar, memory, learning
- **6 Channels**: Telegram, Discord, Slack, WhatsApp, QQ, Email (feature-gated)
- **Semantic Search**: fastembed (384-dim), pgvector ANN, hybrid RRF
- **Plan Execution**: Multi-step with backtracking (3 retry limit)
- **Adaptive Learning**: Per-tool confidence thresholds, outcome tracking
- **Calendar Sync**: CalDAV (Apple, Google, generic), conflict detection
- **Enrichment**: Auto-infer priority, duration, due dates from keywords
- **11 Skills**: todo modes, daily-planning, weekly-report, github, cron, etc.

## Known Implementation Gaps

| Gap | Severity | Details |
|-----|----------|---------|
| JSONL→SQL migration incomplete | **High** | ReminderEngine, SchedulingService, PlanCompletionHandler still use old stores |
| Dashboard under-utilized | Medium | GraphQL exists but WebSocket subscriptions not wired to agent loop |
| Todo Phase 2 features stubbed | Medium | Attachments, time tracking, cascade completion not persisted |
| Plan parameter generation | Low | `execute_step()` passes `{}` as tool args |
| Feature flags missing | Low | fastembed, dashboard, learning always built |
| Agent constructor complexity | Low | 10+ params, needs StorageLayer wrapper |

## Test Infrastructure

- **2,722 unit tests** (inline `#[cfg(test)]`)
- **47 integration test files** (cross-crate)
- **13 dedicated storage repo test suites**
- **Zero clippy warnings** (enforced)
- **nextest** for parallel execution
