# Agent Loop Redesign: Adaptive Orchestrator Architecture

**Date:** 2026-02-18
**Status:** Approved
**Priority:** Chat-first (Telegram, Discord, WhatsApp, Slack), CLI secondary
**Current Grade:** 52/100
**Target Grade:** 87/100

## Executive Summary

Replace the monolithic ReAct agent loop with a **layered Adaptive Orchestrator** that classifies incoming requests and routes them to the optimal execution strategy. Add production-grade context engineering, multi-axis learning, native provider support, and chat-first optimizations.

### Core Principle

The best agents in 2025-2026 are not one big loop — they're an orchestrator that selects the right execution strategy per request, with disciplined context engineering throughout.

---

## Current Architecture Analysis

### What Works

- Solid ReAct loop foundation with parallel tool execution (`join_all`)
- Iteration limits (20 chat / 50 plan)
- Streaming support with SSE accumulation
- Confidence-gated tool execution (novel approach)
- Dependency inversion pattern for cross-layer tools
- Session persistence (JSONL)
- Multi-channel support (6+ platforms)
- Learning system foundation (outcome recording, adaptive thresholds)

### Critical Gaps (Why 52/100)

1. **No Token Budget Management** — All context injected unconditionally. No counting, no prioritization. System prompt grows unbounded. A large MEMORY.md silently overflows the context window.

2. **Plan Execution Is Stateless** — `PlanExecutor.execute_step()` builds a minimal 2-message context with NO memory, NO goals, NO history, NO skills. The plan-executing LLM is blind to everything the user cares about.

3. **Conversation Embeddings Stored But Never Used** — Full embedding pipeline exists (fastembed, 384-dim vectors, cosine similarity search) but nothing auto-injects relevant past conversations into context. The LLM must explicitly call a tool.

4. **Learning Only Adjusts One Float** — The entire learning system only changes the confidence threshold. It doesn't adapt tool selection, prompt strategy, skill priorities, or behavior patterns.

5. **No Reflection or Self-Correction** — The agent commits to tool calls without evaluating whether the results make sense. No post-tool-call evaluation, no structured reasoning.

### Additional Issues

- No tool timeouts — a hanging tool blocks the iteration indefinitely
- Write lock acquired for read-only tool definition cache
- No rate limit retry or provider failover
- All providers via OpenAI-compat — Anthropic native features inaccessible (extended thinking, prompt caching)
- Hard-coded 50-message history with no summarization
- No cost tracking or token usage monitoring
- Plan steps pass `{}` as tool arguments
- JSONL race conditions with no file locking
- No typing indicators or progress feedback for chat users
- No interrupt handling for follow-up messages during execution

---

## New Architecture: Adaptive Orchestrator

### Pipeline Overview

```
INBOUND MESSAGE
  │
  ▼
Layer 1: CONTEXT ASSEMBLER
  Token Budget Manager + Memory Retriever + History Compressor
  │
  ▼
Layer 2: ORCHESTRATOR
  Heuristic pre-filter → LLM classifier → Confidence gate
  → DirectResponse | ToolAssisted | AutonomousTask | Clarification
  │
  ▼
Layer 3: EXECUTION ENGINES
  DirectResponse | ReAct+ (with reasoning & reflection) | PlanExecute
  (shared ExecutionCore for LLM calls + tool dispatch)
  │
  ▼
Layer 4: OUTPUT PIPELINE
  Response Validator → Cost Tracker → Learning Recorder → Session Persistence → Async Embedding
```

### Crate Structure Changes

```
Layer 2: context_engine  — NEW: Token budget, memory retrieval, history compression
Layer 5: orchestrator    — NEW: Intent classification, strategy selection (module in agent)
Layer 5: agent           — REFACTORED: Execution engines (Direct, ReAct+, PlanExecute)
                           Replaces monolithic agent_loop.rs
```

---

## Layer 1: Context Assembler

### Token Budget Manager

Model-aware token budgeting with priority-based waterfall:

```
Total context window: detected per-model from ProviderSpec
Reserved for LLM response: 15%
Available for input: 85%

Priority 0 — System Identity:      fixed (~500 tokens)
Priority 1 — Active Task Context:  up to 15%
Priority 2 — Tool Definitions:     up to 15%
Priority 3 — Recent History:       up to 25% (dynamic N messages)
Priority 4 — Retrieved Memory:     up to 10% (embedding-based RAG)
Priority 5 — Compressed History:   up to 10% (summarized older turns)
Priority 6 — Bootstrap Persona:    up to 5%
Priority 7 — Skills & Enrichment:  up to 5%
Overflow Budget: remaining tokens for additional context
```

**Key decisions:**
- Token counting via `tiktoken-rs` for OpenAI, Anthropic native `/count_tokens` endpoint, character estimation (4 chars ~ 1 token) for others
- Per-message token count cached — only count once
- Priority 0-2 always included (agent always knows identity, current task, available tools)
- Execution strategy influences budget allocation (DirectResponse = minimal, AutonomousTask = maximum)

### Memory Retriever (Embedding-Based RAG)

```
User message → embed(message) via fastembed
  → parallel search:
      ├─ ConversationEmbeddingStore.search(embedding, top_k=5, threshold=0.5)
      ├─ MemoryStore.search_semantic(embedding, top_k=3)  [chunks of MEMORY.md]
      └─ TodoStore.search_semantic(embedding, top_k=3)
  → deduplicate + rank by relevance
  → fit within Priority 4 budget
```

**Changes from current:**
- `MEMORY.md` chunked into paragraphs, each embedded separately (currently dumped verbatim)
- Conversation embeddings (currently stored but unused) auto-injected when relevant
- Todo search supplements static "Active Tasks" section

### History Compressor

```
Session history (all N messages)
  ├─ Recent window (last K messages): VERBATIM
  │   K = dynamic based on budget, minimum 4 (2 user + 2 assistant)
  ├─ Middle window (K+1 to K+M): SUMMARIZED
  │   Chunked into groups of 5-10, each summarized once and cached
  │   Format: "Earlier in this conversation: {bullet points}"
  └─ Old window (beyond K+M): DROPPED
      Key facts extracted into session metadata
```

**Summarization:** Incremental (chunk-based, cached), LLM-powered with extractive fallback.

### Tool Result Management

```
Tool result < 500 tokens:   include verbatim
500-2000 tokens:             truncate + store full in scratchpad
> 2000 tokens:               LLM summarize + store full in scratchpad + embed
```

### ContextEngine Interface

```rust
pub struct ContextEngine {
    token_counter: Arc<dyn TokenCounter>,
    memory_retriever: Arc<MemoryRetriever>,
    history_compressor: Arc<HistoryCompressor>,
    budget_config: BudgetConfig,
}

pub struct AssembledContext {
    pub messages: Vec<Message>,
    pub token_count: usize,
    pub budget_report: BudgetReport,
    pub metadata: ContextMetadata,
}

pub struct ContextRequest {
    pub message: InboundMessage,
    pub session: Arc<Session>,
    pub execution_strategy: ExecutionStrategy,
    pub active_plan: Option<PlanContext>,
    pub scratchpad: Option<Scratchpad>,
}

impl ContextEngine {
    pub async fn assemble(&self, request: ContextRequest) -> Result<AssembledContext>;
    pub async fn pre_warm(&self, session_key: &SessionKey, profile: &UserProfile);
}
```

### Production Safeguards

- Context overflow: gracefully degrade by dropping lowest-priority sections
- Stale cache: detect bootstrap file changes via modification timestamp
- Budget observability: every `AssembledContext` includes `BudgetReport`
- Deterministic assembly: same inputs → same output

---

## Layer 2: Orchestrator

### Intent Classification (3-step)

**Step 1: Heuristic Pre-filter (zero LLM cost)**
- Message < 20 chars + greeting pattern → DirectResponse
- Skill trigger matched → ToolAssisted
- Explicit plan command → AutonomousTask
- Contains file paths / code / "fix" / "build" → hint for LLM classifier

**Step 2: LLM Classification (single cheap call)**
- Uses `classifier_model` (e.g., claude-haiku) separate from main model
- Input: last 3 messages + tool names only (~300 tokens)
- Output: `{ strategy, reasoning, estimated_steps, tools_likely_needed, confidence }`

**Step 3: Confidence Gate**
- >= 0.8 → use classified strategy
- 0.5-0.8 → use with fallback enabled (can escalate mid-execution)
- < 0.5 → default to ToolAssisted (safe middle ground)

### Four Execution Strategies

```rust
pub enum ExecutionStrategy {
    DirectResponse {
        context_budget: BudgetPreset::Minimal,  // ~30% of window
    },
    ToolAssisted {
        max_iterations: u32,         // default: 10
        tools_hint: Vec<String>,
        reflection_mode: ReflectionMode::OnFailure,
        context_budget: BudgetPreset::Standard,  // ~70% of window
    },
    AutonomousTask {
        max_iterations: u32,         // default: 50
        planning_model: Option<ModelOverride>,
        reflection_mode: ReflectionMode::AtCheckpoints,
        context_budget: BudgetPreset::Maximum,  // ~85% of window
        progress_tracking: bool,
    },
    Clarification {
        ambiguity_reason: String,
        suggested_questions: Vec<String>,
    },
}
```

### Strategy Escalation

Engines can escalate mid-execution:
- DirectResponse → ToolAssisted (LLM wanted tools)
- ToolAssisted → AutonomousTask (hit iteration threshold)
- AutonomousTask → ToolAssisted (remaining steps trivial)
- Any → Clarification (needs user input)

Maximum 2 escalations per request. After that, finish with current strategy.

### Tool Filtering

Orchestrator provides tool hints to reduce context:
- Primary tools: full JSON schema in context
- Secondary tools: name + one-line description only
- Excluded tools: omitted entirely

### Cost Control

- Heuristic catches 30-40% of requests at zero LLM cost
- Classification prompt ~300 tokens input
- Classification timeout: 2s → fallback to ToolAssisted
- Per-channel overrides (Telegram: max_iterations=10, CLI: max_iterations=50)

### Orchestrator Interface

```rust
pub struct Orchestrator {
    classifier_provider: DynProvider,
    heuristic_rules: Vec<HeuristicRule>,
    strategy_overrides: StrategyOverrides,
    escalation_policy: EscalationPolicy,
    metrics: OrchestratorMetrics,
}

impl Orchestrator {
    pub async fn classify(
        &self,
        message: &InboundMessage,
        session_summary: &SessionSummary,
        available_tools: &[ToolSummary],
    ) -> Result<ClassificationResult>;

    pub async fn handle_escalation(
        &self,
        signal: EscalationSignal,
        current_strategy: &ExecutionStrategy,
        accumulated_context: &[Message],
    ) -> Result<ExecutionStrategy>;
}
```

---

## Layer 3: Execution Engines

### Shared ExecutionCore

All engines share this foundation:

```rust
pub struct ExecutionCore {
    provider: DynProvider,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    context_engine: Arc<ContextEngine>,
    confidence_evaluator: Arc<ConfidenceEvaluator>,
    outcome_recorder: Arc<OutcomeRecorder>,
    reasoning_store: Arc<ReasoningStore>,
}

pub enum CycleOutcome {
    ToolsExecuted { tool_results: Vec<ToolResult>, reasoning_trace: Option<ReasoningTrace>, token_usage: TokenUsage },
    FinalResponse { content: String, reasoning_trace: Option<ReasoningTrace> },
    EscalationRequested(EscalationSignal),
    ClarificationNeeded { question: String, context: String },
}

impl ExecutionCore {
    pub async fn run_cycle(
        &self, messages: &mut Vec<Message>, tool_filter: &ToolFilter, params: &ExecutionParams,
    ) -> Result<CycleOutcome>;
}
```

**Shared core handles:** LLM calls (streaming/standard), tool call parsing, parallel tool execution with per-tool timeout (default 30s), confidence evaluation, error-to-string passthrough, token usage tracking, outcome recording, **read lock** for tool definitions.

### Engine 1: DirectResponse

Single LLM call, no tools, minimal context (~500 input tokens). If LLM returns tool calls, escalates to ToolAssisted.

### Engine 2: ReAct+ (Enhanced)

Improved ReAct loop with three additions:

**Reasoning Scratchpad:**
- LLM outputs `<reasoning>` blocks (parsed and stripped before user response)
- Captures: thought, planned actions, actual action, reflection per cycle
- Persists in session metadata for debugging

**Reflection Checkpoints:**
```rust
pub enum ReflectionMode {
    Always,     // reflect after every tool execution
    OnFailure,  // reflect only when a tool fails
    Disabled,   // fastest, for simple tool calls
}
```

Reflection checks tool results against expectations. Can trigger retry with insight injection, abort with clarification, or continue.

**Escalation Threshold:**
After N cycles (configurable), auto-escalate to AutonomousTask if still running.

### Engine 3: PlanExecute (Redesigned)

Five critical fixes over current implementation:

**Fix 1 — Real Parameter Generation:**
Planner generates tool parameters (not `{}`). Planner receives full tool schemas. Executor validates parameters against JSON Schema before calling. On validation failure, targeted LLM call fixes just the parameters.

**Fix 2 — Rich Step Context:**
Each step receives full context: plan overview, completed step summaries, current step details, upcoming steps, user context (memory, preferences, session info), accumulated results from prior steps, reasoning scratchpad.

**Fix 3 — Step Dependencies & Parallel Execution (LLMCompiler pattern):**
Steps declare `depends_on: Vec<usize>`. Independent steps run in parallel via topological sort into parallel groups. Example: steps [0,1] in parallel, then [2], then [3].

**Fix 4 — Checkpoint Reflection:**
Every N steps (default 2), pause to verify completed steps against `verification_criteria`. Can: retry with adjusted params, insert corrective step, replan from failure point, skip unnecessary step.

**Fix 5 — Progress File:**
For 5+ step plans, write progress to `~/.klyntbot/data/progress/<plan-id>.md`. Survives session boundaries. New session reads progress file to resume from last completed step.

### PlanStep Schema (Extended)

```rust
pub struct PlanStep {
    pub description: String,
    pub tools_needed: Vec<String>,
    pub parameters: HashMap<String, Value>,    // pre-generated by planner
    pub verification_criteria: String,          // how to check success
    pub depends_on: Vec<usize>,                // for parallelism
    pub estimated_tokens: u32,                 // budget estimation
    pub status: StepStatus,
    pub timestamps: StepTimestamps,
    pub attempt_count: u32,
}
```

### Engine Output (Shared)

```rust
pub struct EngineOutput {
    pub content: String,
    pub strategy_used: ExecutionStrategy,
    pub cycles: u32,
    pub total_tokens: TokenUsage,
    pub reasoning_traces: Vec<ReasoningTrace>,
    pub escalation: Option<EscalationSignal>,
    pub learning_data: LearningData,
    pub plan_id: Option<String>,
}
```

---

## Layer 4: Output Pipeline

### Stage 1: Response Validator

**Safety checks (deterministic, non-LLM):**
- Content length within channel limits (Telegram 4096, Discord 2000, etc.)
- No leaked internal tokens (`<confidence>`, `<reasoning>`, `<scratchpad>`)
- No raw JSON tool schemas in response
- UTF-8 encoding valid for target channel

**Quality checks (advisory, non-blocking):**
- Response addresses user's question (embedding similarity > 0.3)
- Response not empty or generic filler
- If tools were called, results are referenced

### Stage 2: Cost & Usage Tracker

Per-request recording:
- Request ID, session key, strategy, classification source
- LLM calls count, input/output tokens, tool calls with latency
- Total latency, estimated cost (USD)
- Context budget report

Stored in `~/.klyntbot/data/usage.jsonl`. Queryable via `klyntbot usage report`.

### Stage 3: Expanded Learning Recorder

Records beyond current tool_name/success/confidence:
- Strategy effectiveness (predicted vs actual, escalation count)
- Reasoning quality (predicted steps vs actual, predicted tools vs actual)
- Context effectiveness (memories retrieved vs used, history included)
- Reflection outcomes (triggered, caused retry, caught error)

### Stage 4: Session Persistence (Upgraded)

- Atomic writes via temp file + rename (no corruption)
- File locking via `flock()` for cross-process safety
- Auto-compaction when session > 1000 entries
- Metadata on each entry: request_id, strategy, cycles, tools_called, reasoning_summary, token_usage

### Stage 5: Async Embedding (Improved)

- Embed user messages, assistant responses (existing)
- Embed tool results > 200 tokens (new — enables future retrieval)
- Embed reasoning summaries (new — "how did I solve similar problems?")
- Batch embedding for efficiency
- Retry with backoff on failure (2 retries, then silent fail)

---

## Upgraded Provider Layer

### Native Provider Support

```
LlmProvider trait
  ├── AnthropicNativeProvider  — /v1/messages endpoint
  ├── OpenAiNativeProvider     — native OpenAI endpoint
  └── OpenAiCompatProvider     — all other 10+ providers (existing)
```

**Anthropic native unlocks:**
- Extended thinking (reasoning in `<thinking>` blocks)
- Prompt caching (`cache_control` headers — up to 90% cost savings on system prompts)
- Native token counting (`/v1/messages/count_tokens`)
- Citations API
- Beta headers for new features

**OpenAI native unlocks:**
- Structured outputs (guaranteed JSON schema conformance)
- `tool_choice: "required"` (force tool use)
- Parallel function calling controls
- Response format constraints

### Extended Thinking Usage

| Component | Extended Thinking | Rationale |
|---|---|---|
| Orchestrator classification | No | Fast and cheap |
| DirectResponse | No | Simple responses |
| ReAct+ tool calls | No | Each cycle fast |
| ReAct+ reflection | **Yes** | Deep analysis benefits |
| Plan generation | **Yes** | Highest-leverage thinking |
| Plan checkpoint reflection | **Yes** | Catching errors critical |
| Plan step execution | No | Steps execute quickly |
| Backtrack regeneration | **Yes** | Recovery needs care |

### Provider Manager (Failover + Rate Limiting)

```rust
pub struct ProviderManager {
    primary: DynProvider,
    fallback: Option<DynProvider>,
    classifier_provider: Option<DynProvider>,
    rate_limiter: RateLimiter,        // token bucket per provider
    circuit_breaker: CircuitBreaker,  // trip after N consecutive failures
}
```

**Retry strategy:** Exponential backoff (500ms, 1s, 2s) up to 3 attempts on rate limits. Circuit breaker opens after 5 consecutive failures, auto-resets after 60s. Fallback provider used when circuit open.

### LlmProvider Trait Extension

```rust
pub trait LlmProvider: Send + Sync {
    // Existing
    async fn chat(...) -> Result<LlmResponse>;
    async fn chat_stream(...) -> Result<BoxStream<LlmStreamChunk>>;
    fn name(&self) -> &str;
    fn default_model(&self) -> &str;

    // NEW
    async fn count_tokens(&self, messages: &[Message], tools: &[Value]) -> Result<usize>;
    fn capabilities(&self) -> ProviderCapabilities;
    fn context_window(&self) -> usize;
}

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

### Provider Config

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-...",
      "native": true,
      "cacheSystemPrompt": true,
      "extendedThinking": {
        "enabled": true,
        "budgetTokens": 10000,
        "useFor": ["planning", "reflection"]
      }
    },
    "openai": {
      "apiKey": "sk-...",
      "native": true,
      "structuredOutputs": true
    }
  },
  "providerManager": {
    "primary": "anthropic",
    "fallback": "openai",
    "classifierModel": "claude-haiku",
    "rateLimits": {
      "anthropic": { "requestsPerMinute": 50, "tokensPerMinute": 100000 },
      "openai": { "requestsPerMinute": 60, "tokensPerMinute": 150000 }
    },
    "circuitBreaker": {
      "failureThreshold": 5,
      "resetTimeout": 60
    }
  }
}
```

---

## Multi-Axis Learning System

### Six Axes of Adaptation

**Axis 1: Per-Tool Confidence Thresholds** (improved from global float)
- Tools with 95%+ success → lower threshold (act faster)
- Tools with <70% success → raise threshold (be cautious)

**Axis 2: Strategy Classification Accuracy**
- Track predicted strategy vs actual (did it escalate?)
- Feed misclassifications back to orchestrator heuristics
- Per-channel strategy preferences

**Axis 3: Context Relevance**
- Track which retrieved memories were actually used
- Adjust retrieval threshold per topic cluster
- Track which context sections the LLM references

**Axis 4: Tool Performance**
- Per-tool: avg latency, success rate, retry frequency
- Auto-adjust tool timeout based on p95 latency
- Track tool co-occurrence patterns

**Axis 5: Response Quality (Behavioral Signals)**
- "thanks" / new topic → positive (+0.8 / +0.5)
- "wrong" / rephrase same question → negative (-0.8 / -0.6)
- "more details" → partial (-0.2)
- Immediate follow-up (<5s) → correction (-0.4)
- No explicit feedback required — purely behavioral

**Axis 6: Per-User / Per-Channel Preferences**
- Response length preference (Brief / Standard / Detailed)
- Frequently used tools per user
- Topic clusters per user
- Active hours for cache pre-warming

### Learning Profiles

```rust
pub struct LearningProfile {
    pub global_confidence: PerToolThresholds,
    pub strategy_accuracy: StrategyAccuracyStats,
    pub tool_performance: HashMap<String, ToolStats>,
    pub channel_profiles: HashMap<ChannelName, ChannelProfile>,
    pub user_profiles: HashMap<SessionKey, UserProfile>,
}

pub struct UserProfile {
    pub preferred_response_length: ResponseLength,
    pub frequently_used_tools: Vec<String>,
    pub topic_clusters: Vec<TopicCluster>,
    pub last_active: DateTime<Utc>,
    pub total_interactions: u64,
    pub satisfaction_score: f32,
}
```

---

## Chat-First Adaptations

### Latency Optimization

| Phase | Current | New |
|---|---|---|
| Context build | Full rebuild ~200-500ms | Incremental + pre-warmed ~20-50ms |
| Classification | N/A (always full loop) | Heuristic 0ms / LLM ~150ms |
| LLM call | Wait for full response | Stream first token immediately |
| Tool execution | User sees nothing | Typing indicator + progress updates |
| Response delivery | Send all at once | Stream as generated (Discord, Slack) |

### Typing Indicators & Progress

```rust
pub struct ChatProgressReporter {
    channel: ChannelName,
    chat_id: ChatId,
    bus: Arc<MessageBus>,
}

impl ChatProgressReporter {
    pub async fn show_typing(&self);
    pub async fn send_progress(&self, status: &str);  // for channels supporting message editing
}
```

### Interrupt Handling

```rust
pub enum InterruptPolicy {
    Queue,           // buffer message, finish current work
    CancelAndSwitch, // cancel current, process new message
    Merge,           // append to current context
}
```

Detection:
- Short correction (<50 chars, <2s) → Merge
- Cancel intent ("stop", "cancel", "nevermind") → CancelAndSwitch
- Default → Queue

### Channel-Aware Response Formatting

```rust
pub struct ResponseFormatter {
    channel: ChannelName,
    max_length: usize,
}
```

- Telegram: 4096 chars, Markdown, split at paragraphs
- Discord: 2000 chars, Markdown, embeds for structured data
- WhatsApp: plain text, strip markdown, emoji bullets
- Slack: mrkdwn format conversion

### Context Pre-Warming

Channels that send "user is typing" events (Telegram, Discord) trigger pre-loading of session history, relevant memories, and tool definitions before the message arrives.

---

## New CLI Commands

```bash
# Usage & cost tracking
klyntbot usage report [--period day|week|month] [--channel telegram|discord|all]
klyntbot usage breakdown [--by strategy|tool|channel]

# Learning inspection
klyntbot learning status
klyntbot learning profile <session-key>
klyntbot learning reset [--axis confidence|strategy|all]

# Session debugging
klyntbot session inspect <key>
klyntbot session export <key> --format json

# Provider status
klyntbot provider status
klyntbot provider test [provider-name]
```

---

## Projected Score

| Dimension | Current | New | Delta |
|---|---|---|---|
| Core Loop Design | 7/10 | 9/10 | +2 |
| Context Engineering | 3/10 | 9/10 | +6 |
| Tool Execution | 6/10 | 9/10 | +3 |
| Decision Making | 4/10 | 9/10 | +5 |
| Planning & Autonomy | 4/10 | 8/10 | +4 |
| Memory & Learning | 5/10 | 9/10 | +4 |
| Error Handling & Recovery | 5/10 | 8/10 | +3 |
| Provider Integration | 5/10 | 9/10 | +4 |
| Observability | 6/10 | 9/10 | +3 |
| Production Readiness | 7/10 | 8/10 | +1 |
| **Total** | **52/100** | **87/100** | **+35** |

### Not In Scope (Phase 2+)

- Multi-agent orchestration (overkill for chat-first single-user)
- A/B testing of strategies
- Horizontal scaling / load balancing
- Real-time monitoring dashboard
