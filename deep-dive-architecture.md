# Klyntbot Deep Dive: Architecture & AI System

## What Is This Project?

Klyntbot is a **personal AI agent** — a single Rust binary that connects 6+ chat platforms (Telegram, Discord, Slack, Email, desktop app, CLI) to LLMs, with built-in task/project management, financial tracking, coaching, and persistent memory. Think of it as a private AI assistant that lives across all your communication channels and actually *remembers* you.

All state lives in SQLite + LanceDB (vector embeddings). No cloud database required.

---

## The 10,000-Foot View

```
┌──────────────────────────────────────────────────────────┐
│                      CHANNELS (L5)                        │
│   Telegram  Discord  Slack  Email  Desktop  CLI           │
└─────────────────────────┬────────────────────────────────┘
                          │ InboundMessage
                          ▼
┌──────────────────────────────────────────────────────────┐
│                    MESSAGE BUS (L1)                        │
│   mpsc channels: inbound_tx/rx, outbound_tx/rx            │
└─────────────────────────┬────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────┐
│                   AGENT RUNTIME (L5)                      │
│                                                           │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐  │
│  │AgentManager  │→│IntentAnalyzer│→│ ContextEngine    │  │
│  │(profile      │  │(heuristic +  │  │(budget, memory,  │  │
│  │ matching)    │  │ LLM classify)│  │ context sources) │  │
│  └─────────────┘  └──────────────┘  └────────┬────────┘  │
│                                               │           │
│  ┌──────────────────────────────────────────┐ │           │
│  │          EXECUTION ROUTER                 │◄┘           │
│  │  ┌──────────┐     ┌───────────────────┐  │            │
│  │  │ Direct   │     │   Reactive (ReAct) │  │            │
│  │  │ Engine   │     │   Loop Engine      │  │            │
│  │  │ (1 call) │     │   (N iterations)   │  │            │
│  │  └──────────┘     └────────┬──────────┘  │            │
│  └────────────────────────────┼──────────────┘            │
│                               │                           │
│  ┌────────────────────────────▼──────────────────────┐    │
│  │              EXECUTION CORE                        │    │
│  │  LLM Call → Parse Tool Calls → Execute Tools       │    │
│  │  (parallel, semaphore-bounded, timeout-protected)  │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────┬────────────────────────────────┘
                          │ OutboundMessage
                          ▼
                     Back to Channel
```

---

## Layer Architecture (26 Crates, 9 Layers)

```
L8: klyntbot              ← Re-export facade
L7: app-core, desktop-shared, desktop  ← Tauri desktop app
L6: mcp                   ← Model Context Protocol
L5: channels, agent, cognitive  ← Platform integrations, AI brain, memory
L4: tools, feature-*      ← 20+ tools, feature packages
L3: providers, session, scheduling, context_engine  ← LLMs, persistence, cron
L2: storage, domain       ← SQLite repos, OKR+PARA types
L1: config, bus, tools-core, macros  ← Config, message bus, Tool traits
L0: common                ← Shared types (errors, enums)
```

Dependencies flow strictly upward. No circular deps — dependency inversion via `Arc<dyn Trait>`.

---

## Part 1: The AI Brain

### 1.1 Message Processing Pipeline

When a user sends "create a task to review the Q4 budget by Friday", here's exactly what happens:

**Step 1 — Agent Matching** (`AgentManager`)

The AgentManager scans the message against trigger keywords from 5 built-in agent profiles:

| Agent | Triggers | Purpose |
|-------|----------|---------|
| `general` | Fallback (no triggers) | Default orchestrator |
| `task` | "create a task", "todo", "weekly review" | Task/project CRUD |
| `finance` | "budget", "expense", "transaction" | Financial tracking |
| `automation` | "schedule", "automate", "workflow" | Cron jobs & workflows |
| `communication` | "email", "message", "reply" | Messaging |

Each profile defines: allowed tools, MCP server access, max iterations, delegation targets, always-loaded skills, and a full system prompt.

Our example message matches `task` (keyword "task") and `finance` (keyword "budget"). The manager selects the best match.

**Step 2 — Intent Classification** (`IntentAnalyzer`)

Two-stage hybrid classifier:

*Stage A: Heuristic (zero-cost).* Pattern matching catches obvious cases:
- Greetings → Direct mode, 0.95 confidence
- "create/add/delete" verbs → Reactive mode
- Short non-action messages → Direct mode
- Complex workflow keywords → Reactive with high budget

*Stage B: LLM Classifier (if heuristic returns None).* A lightweight LLM call with structured JSON output. Returns:

```rust
ComplexitySignals {
    estimated_tool_calls: 2,        // 0-10
    has_sequential_deps: false,
    failure_risk: Low,
    requires_state_tracking: false,
    requires_retries: false,
}
```

The complexity score (0-7) determines iteration budget: `min(max(tool_calls * 3, 10) + 5, 30)`.

Our example: heuristic detects "create" → Reactive mode, ~2 estimated tool calls.

**Step 3 — Context Assembly** (`ContextEngine`)

The context engine builds the complete prompt within a token budget:

```
Total Budget (e.g., 128K tokens)
    ├── System Prompt (agent instructions + skills)
    ├── Memories (semantic search from cognitive system)
    ├── Conversation History (compressed if needed)
    └── Tool Descriptions (JSON schemas)
```

Budget allocation shifts by execution strategy:
- **DirectResponse**: ~70% history, minimal tools
- **ToolAssisted**: ~40% history, ~30% tools
- **AutonomousTask**: ~20% history, ~40% tools

Context sources are priority-ordered and pluggable:

| Priority | Source | What It Injects |
|----------|--------|-----------------|
| 35 | AgentContextSource | Agent instructions + always-loaded skills |
| 30 | IdentitySource | "User's name is Klynt, prefers..." |
| 25 | ProductivitySource | "Currently in deep work session..." |
| 20 | ProjectSource | "Active project: Q4 Planning" |
| 15 | TodoSource | "3 high-priority tasks overdue" |
| 10 | AnnotationSource | User-pinned notes |
| 5 | AreaSource | Domain-specific context |

Results are SHA-256 cached (LRU, capacity 8) for repeated queries.

**Step 4 — Execution**

Two engines, selected by intent classification:

*Direct Engine* — Single LLM call. If the LLM unexpectedly returns tool calls, it escalates to Reactive (misclassification recovery).

*Reactive Engine* — ReAct loop:
```
for iteration in 1..max_iterations:
    response = LLM(messages + tool_results)
    if response.has_tool_calls:
        results = execute_tools_parallel(tool_calls)  // up to 10 concurrent
        messages.append(results)
    else:
        return response.text  // Done
```

For complexity ≥ 4, a chain-of-thought planning prompt is injected before iteration 1.

**Step 5 — Tool Execution** (`ExecutionCore`)

Tools run in parallel with backpressure (semaphore, max 10 concurrent). Each tool has a 30s default timeout (configurable). Includes fabrication detection for models that hallucinate tool results.

**Step 6 — Validation & Recording**

`ResponseValidator` checks output quality. `CostTracker` logs token usage. Strategy success/failure is recorded for future classification improvements.

### 1.2 The Tool System

Tools are defined with derive macros — zero boilerplate:

```rust
#[derive(Tool, ToolParams)]
pub struct CreateTaskTool {
    #[param(required)]
    pub title: String,
    #[param]
    pub due_date: Option<String>,
}

#[async_trait]
impl ToolExecute for CreateTaskTool {
    type Params = <Self as Tool>::Params;
    async fn execute(&self, params: Self::Params, ctx: &RoutingContext) -> Result<String> {
        // Business logic here
    }
}
```

The `#[derive(Tool)]` macro generates: JSON Schema for LLM consumption, parameter parsing from JSON, the `Tool` trait implementation (name, description, schema).

Multi-action tools use an enum pattern:
```rust
#[tool_actions]
pub enum TaskActions {
    #[action(doc = "Create a new task")]
    Create(CreateParams),
    #[action(doc = "Update an existing task")]
    Update(UpdateParams),
    // ... 25 actions total for TaskTool
}
```

Every tool receives a `RoutingContext` with: channel info, chat ID, interaction sender (for ask_user), delegation depth, and entity event channels.

### 1.3 The Cognitive Memory System

This is what makes Klyntbot *remember*. Three memory types:

**Semantic Facts** (bi-temporal, FSRS decay):
```rust
SemanticFact {
    domain: "identity",           // identity, work, finance, learning, etc.
    subject: "user",
    predicate: "prefers",
    object: "morning meetings",
    confidence: 0.85,
    valid_from: "2025-01-15",     // When fact became true
    valid_until: None,            // Still true
    stability: 0.92,             // FSRS retention score
}
```

**Episodic Memories** (events with importance scoring):
```rust
EpisodicMemory {
    domain: "work",
    content: "User completed Q3 budget review in 2 hours",
    importance: 0.7,
    occurred_at: "2025-10-01T14:00:00Z",
}
```

**Procedural Rules** (learned from reflection):
```rust
ProceduralRule {
    rule_text: "When user mentions 'budget', also check for overdue finance tasks",
    confidence: 0.8,
    signal_count: 5,  // How many times this pattern was observed
}
```

**Consolidation Pipeline:**
```
Domain Events (task completed, tool executed, user stated fact...)
    ↓
ExtractionHandler → Parse into observations
    ↓
SalienceVerdict: Extract | Accumulate | Discard
    ↓
ConsolidationHandler → Dedupe, merge, supersede facts
    ↓
ReflectionHandler → Learn procedural rules from patterns
```

Memory retrieval uses embedding-based semantic search via LanceDB. Facts are injected into the system prompt via `CognitiveContextSource`.

### 1.4 LLM Provider Abstraction

```rust
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages, tools, params) -> Result<LlmResponse>;
    async fn chat_stream(&self, messages, tools, params) -> Result<Box<dyn LlmStream>>;
}
```

Two implementations:
- `AnthropicNativeProvider` — Direct Anthropic API with extended thinking support
- `OpenAiCompatProvider` — OpenAI-compatible APIs (OpenRouter, DeepSeek, Gemini, etc.)

Provider resolution: explicit config → model name detection (claude-* → Anthropic) → gateway detection → first non-empty API key.

`ProviderManager` wraps providers with failover + circuit breaker.

### 1.5 MCP Integration

The Model Context Protocol lets external tools plug in:

```
Config → McpManager → connects to MCP servers at startup
    ↓
Tool discovery: each server's tools → wrapped as McpTool
    ↓
Naming: mcp_{server}_{tool} (e.g., mcp_linear_list_issues)
    ↓
Access control: per-agent mcp_tools field
    ["*"] = all servers, [] = none, ["linear"] = specific
```

### 1.6 Session Management

Conversations persist in SQLite via `SessionManager`:
- DashMap for lock-free concurrent per-session access
- Per-session tokio::sync::Mutex for mutations
- LRU eviction: when 1000+ entries, keep 500 most recent
- Session key format: `channel:chat_id`
- Full message history with tool calls and metadata

---

## Part 2: The Domain Model

### PARA + OKR Framework

```
Area (e.g., "Work", "Personal")
 └── Project (e.g., "Q4 Planning")
      └── Objective (e.g., "Close all Q4 deals")
           └── Key Result (metric or action-based)
                └── Action/Task (the actual work item)
```

**Actions** (tasks) are the richest type — they support:
- Subtasks (parent_id hierarchy)
- Focus sessions with time tracking
- RRULE-based recurrence (iCalendar standard)
- Dependencies (blocked_by / blocks)
- Attachments and time entries
- Custom status workflows
- Calendar event linking

### Feature Packages

Self-contained crates implementing `FeaturePackage`:

| Feature | Tools | What It Does |
|---------|-------|-------------|
| `feature-todo` | TaskTool (25 actions) | Task CRUD, focus, time tracking, recurrence |
| `feature-finance` | FinanceTool (40+ actions) | Accounts, transactions, budgets, investments, FIRE planning |
| `feature-notes` | NotesTool | Notebooks, notes, tags, linking, versioning |
| `feature-productivity` | ProductivityTool | Focus sessions, activity tracking, daily aggregation, nudges |
| `feature-coaching` | — | Pattern detection → LLM reasoning → interventions |

Each package brings its own: tools, database migrations, config section, and health checks.

### Event-Driven Architecture

The `DomainEventBus` (tokio broadcast) decouples features:

```
TaskCompleted event →
    ├── Cognitive system (learns patterns)
    ├── Coaching (detects behavioral signals)
    ├── Productivity (updates daily aggregate)
    └── Any subscriber
```

30+ event types covering: productivity, tasks, finance, notes, chat, tools, coaching, and behavioral patterns.

---

## Part 3: The Desktop App

### Three-Layer Architecture

```
┌─────────────────────────────┐
│  desktop-ui (React + Vite)  │  ← UI components, hooks, routing
├─────────────────────────────┤
│  desktop (Tauri adapter)    │  ← Thin command wrappers + event forwarding
├─────────────────────────────┤
│  app-core (business logic)  │  ← Transport-agnostic handlers
└─────────────────────────────┘
```

**AppCore** is the key abstraction — it contains all business logic and returns `HandlerResult<T> = (data, Vec<EntityUpdate>)`. The Tauri adapter just forwards calls and emits UI events.

### IPC Pattern

```typescript
// Frontend: SWR-style caching
const { data: task } = useQuery<Task>("task_get", { id });

// Mutations trigger entity events
const update = useMutation<Task, Params>("task_update", "params");
await update.mutate({ id, title: "New title" });
// → Tauri command → AppCore → DB update → emit entity:updated
// → useEvent listener → useQuery refetch → React re-render
```

**Dual-mode IPC**: Tauri `invoke()` in production, HTTP POST to dev-api in browser dev mode.

### Event Flow

```
Feature Service (ProductivityEngine, CoachingService, etc.)
    ↓ tokio channel
Desktop adapter receives, emits Tauri event
    ↓ entity:updated, score:updated, coaching:intervention
Frontend useEvent listener
    ↓
Update local state → React re-render
```

---

## Part 4: The Storage Layer

**StoragePool** wraps sqlx::SqlitePool (Clone+Send+Sync). WAL mode enabled. Foreign keys enforced.

**Repos** aggregate provides a single access point to 20+ repository types:

```rust
let repos = Repos::from_pool(&pool);
let task = repos.actions.get("task-id").await?;
let projects = repos.projects.list(&filter).await?;
```

Feature migrations tracked separately in `_feature_migrations` table — each FeaturePackage brings its own schema.

Vectors stored in LanceDB (`~/.klyntbot/lancedb/`) for semantic search.

---

## Part 5: Channel Integrations

Unified `Channel` trait with platform-specific implementations:

| Channel | Connection | Special Features |
|---------|-----------|-----------------|
| Telegram | Bot API polling/webhook | Voice transcription (Groq), inline keyboards |
| Discord | Gateway WebSocket | Button components, thread replies, rich embeds |
| Slack | Socket Mode WebSocket | Block Kit, message threading |
| Email | IMAP receive, SMTP send | MIME parsing, attachments |

All channels auto-reconnect with 5-second delay. Allowlist checking per-platform.

---

## Part 6: Plugin System (WASM)

Extensibility via WebAssembly plugins:

```
~/.klyntbot/plugins/
  └── notion-connector/
      ├── manifest.json  (tools, cron jobs, migrations, permissions)
      └── plugin.wasm
```

Plugins declare permissions: `Network` (HTTP), `Storage` (DB), `Agent` (context access). Tools are wrapped as `Arc<dyn Tool>` and registered in the ToolRegistry.

---

## Key Architectural Patterns

| Pattern | Where | Why |
|---------|-------|-----|
| Dependency inversion | Handler traits in L1-L2, impls in L5 | Avoids circular deps |
| Derive-based tools | `#[derive(Tool)]` | Zero-boilerplate tool definitions |
| Two-stage classification | Heuristic → LLM fallback | Fast for easy cases, accurate for hard ones |
| Context source plugins | Priority-ordered, parallel-queried | Extensible system prompt |
| Entity-driven UI invalidation | `EntityUpdate` → Tauri events → refetch | Granular UI updates |
| FSRS memory decay | Spaced repetition on semantic facts | Natural forgetting curve |
| Semaphore-bounded parallelism | Tool execution | Backpressure without deadlocks |
| SHA-256 context caching | ContextEngine | Skip redundant assembly |
| Feature packages | `FeaturePackage` trait | Self-contained domain modules |
| Broadcast event bus | `DomainEventBus` | Loose coupling between features |

---

## Configuration

File: `~/.klyntbot/config.json` (camelCase JSON)

Environment override: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o`

API keys wrapped in `Secret<String>` — shows `[REDACTED]` in logs, accessed via `.expose()`.

All timestamps UTC (RFC3339). Config changes require app restart.
