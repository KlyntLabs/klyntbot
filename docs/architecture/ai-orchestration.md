# AI Orchestration

## Agent Runtime — 12-Step Message Processing Pipeline

The `AgentRuntime::process_message()` method is the central orchestration point. Every user message, regardless of channel, flows through this pipeline.

```
Step 0a: AutoTuner shadow classification
Step 0b: Generate query embedding
Step 1:  Select orchestrator skill (keyword + semantic blended)
Step 2:  Write active profile (shared with SkillContextSource)
Step 2a: Activate non-orchestrator skills (max 3)
Step 2b: Squad mode diversion (if multi-persona)
Step 3:  Filter MCP tools by skill profile
Step 3c: Spawn memory prefetch (concurrent with Step 4)
Step 4:  Intent analysis (4-layer cascade)
Step 4b: Orchestration override (uses analysis.needs_orchestration)
Step 4c: Publish DomainEvent::SkillRouted (for Mirror layer)
Step 5:  Confidence evaluation
Step 6:  Await memory prefetch
Step 7:  Context assembly (system prompt + memory + history + tools)
Step 8:  Filter tool definitions by profile allowlist
Step 9:  Build ExecutionParams + Execute (Direct or Reactive engine)
Step 10: Record cost, strategy, and interaction (parallel tokio::join!)
Step 11: AutoTuner ground truth
```

> **Note:** `SkillRouted` is published mid-pipeline (Step 4c) so the Mirror
> self-reflection layer receives routing data before execution begins. Steps
> 10's three recording operations (cost, strategy, interaction) run as
> concurrent futures in a single `tokio::join!()` block.

### Detailed Step Flow

```
                              User Message
                                   |
                     +-------------+-------------+
                     |                           |
              Step 0a: Shadow              Step 0b: Embed
              classification               query vector
                     |                           |
                     +-------------+-------------+
                                   |
                            Step 1: SkillRouter
                            select_orchestrator_blended()
                                   |
                         +---------+---------+
                         |                   |
                   Domain skill         "general" fallback
                   (task-mgmt,              |
                    finance, etc.)          |
                         |                   |
                         +---------+---------+
                                   |
                     +-------------+-------------+
                     |                           |
              Step 3c: Spawn            Step 4: Analyze
              memory prefetch           (heuristic → LLM)
              (concurrent)                       |
                     |                    IntentAnalysis {
                     |                      mode: Direct|Reactive
                     |                      confidence: f32
                     |                      signals: ComplexitySignals
                     |                    }
                     |                           |
                     |                  Step 4b: Orchestration
                     |                  override (if general +
                     |                  needs_orchestration →
                     |                  restrict to ask_user
                     |                  + delegate)
                     |                           |
                     |                  Step 4c: Publish
                     |                  DomainEvent::SkillRouted
                     |                  (for Mirror layer)
                     |                           |
                     +-------------+-------------+
                                   |
                            Step 6: Await prefetch
                                   |
                            Step 7: Context Assembly
                            (system prompt, memory,
                             history, tool schemas)
                                   |
                     +-------------+-------------+
                     |                           |
               Direct Mode               Reactive Mode
               (1 LLM call,              (ReAct loop,
                no tools)                 max_iterations)
                     |                           |
                     v                           v
              Step 9: Execute            Step 9: Execute
              DirectEngine               ReactiveEngine
                     |                           |
                     +-------------+-------------+
                                   |
                            Steps 10-11:
                            Record cost + strategy +
                            interaction (parallel join),
                            AutoTuner ground truth
                                   |
                                   v
                            RuntimeResult {
                              content: String,
                              mode_used: String,
                              classification: IntentAnalysis,
                              agent_name: String,
                            }
```

## Squad Mode (Multi-Persona Execution)

When `session.squad_id` is set (Step 2b), the pipeline diverts to the unified debate engine instead of the normal Direct/Reactive flow.

**What is a squad:** A named group of `InsightPersonas`, each with a `role`, `perspective`, `tone`, `questioning_style`, `cognitive_bias`, and `analysis_frameworks`. 4 built-in squads:

| Squad | Personas |
|-------|----------|
| General Analysis | Skeptic, Connector, Student |
| Research | Devil's Advocate, Empiricist, Synthesizer |
| Finance | Risk Analyst, Optimizer, Long-term Planner |
| Strategy | Contrarian, Systems Thinker, Pragmatist |

### Interaction Modes

Message content determines the interaction mode:

| Mode | Detection | Behavior |
|------|-----------|----------|
| **Debate** | "everyone", "squad", "team", "all of you" | Full multi-round debate engine |
| **DirectAddress** | `@PersonaName`, `"PersonaName,"` at start | Single persona response with blackboard context |
| **LeadResponse** | Fallback (configurable per squad) | Squad lead persona responds alone |

Fallback mode is configured per squad via `default_interaction_mode` (Lead / Debate / Smart). Smart mode learns from interaction patterns over the last 30 days.

### Unified Debate Engine

`run_debate(config, context, squad, provider, ...)` is a pure-function interface used by both chat and notes callers:

```
Step 1: Budget pre-check
  → Estimate cost, reduce rounds or request approval if over budget
     |
Step 2: Load learned weights (FSRS-5 accuracy blend)
  → blended_weight = (1 - blend) × relevance_score + blend × accuracy_score
     |
Step 3: Build persona system prompts
  → skill_prompt (from orchestrator_skill) + persona block + context
     |
Step 4: Debate loop (max_rounds, phased)
  → Opening: parallel fan-out (sorted by blended_weight DESC)
  → Discussion: sequential (weakest first, reads blackboard)
  → Final: parallel fan-out
  → Per-round judge evaluation → consensus score → early exit if threshold met
     |
Step 5: Synthesis
  → OutputMode::Synthesized (chat) or StructuredPerPersona (notes)
     |
Step 6: Post-synthesis consensus judge
  → Rate each persona's alignment with final output → FSRS-5 rating
     |
Step 7: Blackboard cleanup
  → Delete debate-scoped entries (captured in DebateResult.rounds)
```

**Two callers:**
- **Chat** (`streaming.rs`): `Synthesized` output, 6 max rounds, persists as session messages, records token usage
- **Notes** (`insight.rs`): `StructuredPerPersona` output, 3 max rounds, stores as `InsightContent.perspectives` with debate transcript

### FSRS-5 Active Feedback Loop

After every debate, a consensus judge rates each persona's alignment (0.0–1.0) with the final synthesis. Mapped to FSRS-5 ratings (4=Easy, 3=Good, 2=Hard, 1=Again). Over ~20 debates, accuracy learning ramps to full influence on persona ordering.

- `relevance_score` is the base weight (short-term, thumbs up/down adjusts ±0.1)
- `accuracy_score` is FSRS-5 stability (long-term, consensus-based)
- User overrides (pins, manual reorder, "Reset learning") always win

### Blackboard Lifecycle

- **Debate working memory** (ephemeral): scoped to `"debate:{squad_id}:{uuid}"`, deleted after debate
- **Thread context** (persistent): `"session:{session_key}:{squad_id}"`, persists for session lifetime
- **Safety-net cron** (weekly): deletes entries older than 30 days

## Execution Engines

### Direct Engine

Single LLM call without tool access. Used for greetings, simple Q&A, conversational responses.

```
DirectEngine::execute()
     |
     v
provider.chat(messages, tools=None, params)
     |
     +--FinalResponse--> EngineResult::Complete { content, iterations=1 }
     |
     +--ToolsExecuted--> EngineResult::Escalate { usage }
     |                   (re-run with ReactiveEngine)
     |
     +--EmptyResponse--> EngineResult::Empty
```

### Reactive Engine (ReAct Loop)

Iterative reasoning-and-acting loop with tool execution.

```
ReactiveEngine::execute()
     |
     v
Initialize:
  - Scratchpad (reasoning trace + loop detector + plan)
  - MidLoopCompressor (70% context threshold)
  - LiveContextRefresher (ContextUpdateQueue drainer)
  - Optional planning_prompt (complexity >= 4)
     |
     v
FOR iteration = 1..max_iterations:
  |
  +-- Check CancellationToken
  |
  +-- Emit AgentEvent::IterationStart
  |
  +-- ExecutionCore::run_cycle()
  |       |
  |       +-- provider.chat_stream(messages, tools, params)
  |       |   (or .chat() if no event_tx)
  |       |
  |       +-- Handle CycleOutcome:
  |       |     FinalResponse   --> break loop, return content
  |       |     ToolsExecuted   --> process results, continue
  |       |     Fabricated      --> retry with force prompt (max 2)
  |       |     EmptyResponse   --> continue
  |       |
  |       +-- Tool execution (parallel, Semaphore=10):
  |             - Registry lookup + param validation
  |             - Execute with timeout (custom or 30s)
  |             - Sanitize result (100KB max, strip control chars)
  |             - Dedup detection (hash-based)
  |             - DomainEvent::ToolCallExecuted published
  |
  +-- MidLoopCompressor::compress_if_needed()
  |     (if >70% context: compress old Tool messages to 150-char summaries)
  |
  +-- LiveContextRefresher::inject_pending()
  |     (drain ContextUpdateQueue, inject as Message::ContextUpdate)
  |
  +-- Loop detection (hash-based):
  |     3 repeats → Warning (inject suggestion)
  |     5 repeats → HardStop (break loop)
  |
  +-- Failure reflection (inject prompt on tool failures)
  |
  +-- Plan tracking (mark steps complete semantically)
     |
     v
Max iterations reached:
  - Inject synthesis prompt (with plan progress)
  - One final LLM call with tools=[] (force text response)
  - Return EngineResult::Complete
```

### Execution Core — Tool Call Handling

```
CycleOutcome::ToolsExecuted
     |
     v
For each tool_call in response.tool_calls:
     |
     +-- Duplicate check (hash of name + args)
     |     Yes --> "Skipped: duplicate" result
     |     No  --> Continue
     |
     +-- Registry lookup: tool_registry.prepare(name, args)
     |     Not found --> error result
     |
     +-- Parallel execution via Semaphore(10)
     |     |
     |     +-- Timeout: custom_timeout() or 30s default
     |     |   (ask_user special: 600s)
     |     |
     |     +-- tool.execute(args, &routing_ctx)
     |     |
     |     +-- Result sanitization:
     |           - Truncate to 100KB
     |           - Strip control chars (preserve \n, \t, \r)
     |
     +-- All results "Skipped:"? --> inject dup warning, break
     |
     +-- Any failures? --> inject reflection prompt
     |
     +-- Append Message::Tool for each result
```

### Fabrication Detection

Some LLMs (notably DeepSeek, Kimi) "fake" tool calls by returning structured text instead of calling tools. Detection heuristics:

1. Fake hex IDs (`id: abc123`)
2. Context-specific phrases (`"task created"`, `"search results:"`)
3. Multiple field-like patterns (`Priority:`, `Due Date:`)
4. Numbered lists resembling search results

On detection: `CycleOutcome::FabricatedResponse` → retry with force prompt: `"You MUST call the appropriate tool..."` (max 2 retries).

## Mid-Loop Context Management

### Compression (70% Threshold)

```
Before compression:
  [System][User][Asst][Tool(5KB)][User][Asst][Tool(8KB)][User][Asst][Tool(2KB)]
                  ^--- old, compressible ---^            ^-- recent (protected) --^

After compression:
  [System][User][Asst][Tool(150c)][User][Asst][Tool(150c)][User][Asst][Tool(2KB)]
```

**Rules:**
- Triggered when total tokens > 70% of context window
- Only `Message::Tool` messages are compressed
- Only messages > 50 tokens are compressed
- Last 8 messages are always preserved verbatim
- All `System` messages at start are always preserved
- Compressed format: `"{first 150 chars}... [compressed {name} result, originally {N} chars]"`

### Live Context Refresh

```
BackgroundConsolidationService
     |
     +-- Memory promoted --> ContextUpdateQueue::push(MemoryPromoted)
     |
     v
ReactiveEngine (at each iteration boundary)
     |
     +-- LiveContextRefresher::inject_pending()
           |
           +-- Drain queue
           +-- Sort by priority (High first)
           +-- Calculate remaining capacity
           +-- Budget: Standard=80% of remaining, High=90%
           +-- Inject as Message::ContextUpdate
           +-- Emit AgentEvent::ContextReassembled
```

This allows newly promoted memories to influence in-flight agent reasoning without waiting for the next message.

## Cost Tracking

### Pricing Model

Hardcoded pricing table (per million tokens):

| Model | Input | Output | Cache Read | Cache Write |
|-------|-------|--------|-----------|-------------|
| Claude Sonnet 4.x | $3.00 | $15.00 | $0.30 | $3.75 |
| Claude Opus 4.x | $15.00 | $75.00 | $1.50 | $18.75 |
| Claude Haiku 4.5 | $0.80 | $4.00 | $0.08 | $1.00 |
| GPT-4o | $2.50 | $10.00 | $1.25 | $0.00 |
| Gemini 2.0 Flash | $0.10 | $0.40 | $0.025 | $0.00 |

**Budget enforcement:** Monthly spend tracked via `UsageRepo.total_cost_current_month()`. Alert at 80% of `monthly_budget_usd`.

### Token Tracking

`Usage` struct tracks separately: `prompt_tokens`, `completion_tokens`, `total_tokens`, `cache_read_tokens`, `cache_write_tokens` (Anthropic prompt caching).

Recorded per-interaction to `usage_records` table with model, provider, strategy (skill name), and channel.

## Tool System

### Tool Trait

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;           // JSON Schema
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
    fn permission_level(&self) -> PermissionLevel;
    fn metadata(&self) -> ToolMetadata;
    fn custom_timeout(&self) -> Option<Duration>;
    fn to_schema(&self) -> Value;            // OpenAI function schema format
}
```

### Derive Macros

```rust
#[derive(Tool)]
#[tool(name = "tasks", description = "Manage tasks", params = "TaskParams")]
struct TaskTool { /* deps */ }

#[derive(ToolParams)]
struct TaskParams {
    /// The action to perform
    #[param(required)]
    action: String,
    /// Task title
    title: Option<String>,
}

// Multi-action tools:
#[tool_actions]
impl FinanceTool {
    #[action(name = "add_transaction")]
    async fn add_transaction(&self, params: AddTransactionParams, ctx: &RoutingContext) -> Result<String> { ... }

    #[action(name = "get_budget")]
    async fn get_budget(&self, params: GetBudgetParams, ctx: &RoutingContext) -> Result<String> { ... }
}
```

### Tool Registry

`HashMap<String, DynTool>` with:
- Usage count tracking (`Mutex<HashMap<String, u64>>`)
- Cached schema definitions (invalidated on register/unregister)
- `prepare(name, params)` separates lookup from execution (prevents deadlocks)
- `unregister_by_prefix(prefix)` for MCP server cleanup

### Complete Tool Inventory

| Category | Tool | Type | Key Actions |
|----------|------|------|-------------|
| **Domain** | `tasks` | Multi-action | create, list, update, delete, focus, forecast, batch, deps |
| | `project` | Multi-action | CRUD for projects |
| | `area` | Multi-action | PARA area management |
| | `okr` | Multi-action | Objectives & key results |
| | `notes` | Multi-action | Note CRUD with tags |
| | `memory` | Multi-action | store, search, list, delete memories |
| | `finance` | Multi-action | accounts, transactions, budgets, investments, reports |
| | `productivity` | Multi-action | Focus tracking, activity, intelligence |
| | `learning` | Multi-action | Flashcard/knowledge management |
| | `cron` | Multi-action | Scheduling operations |
| | `annotate` | Multi-action | Content annotation |
| | `mirror` | Multi-action | get_state, narratives, routing history, brain versions |
| **System** | `ask_user` | Single | Interactive user input (600s timeout) |
| | `web_search` | Single | Web search |
| | `web_fetch` | Single | Fetch URL content |
| | `browser` | Single | Browser automation |
| | `read_file` | Single | Read file from workspace |
| | `list_dir` | Single | List directory contents |
| | `grep` | Single | Search file content |
| | `glob` | Single | Glob file matching |
| | `message` | Single | Send outbound messages |
| | `skill_reference` | Single | Load full skill reference content |
| | `spawn` | Single | Spawn sub-agents |
| | `delegation` | Single | Route to sub-agents |
| **MCP** | `mcp_{server}_{tool}` | Dynamic | External tools from MCP servers |
| **Plugin** | (user-defined) | Dynamic | WASM plugin tools |

### Feature Packages

Feature crates implement `FeaturePackage`:

```rust
pub trait FeaturePackage {
    fn tools(&self) -> Vec<DynTool>;
    fn migration(&self) -> Option<FeatureMigration>;
    fn config(&self) -> Value;
    fn health_check(&self) -> HealthStatus;
}
```

**Exception:** Some tools (e.g., `TaskTool`) are wired directly in the agent builder, not via `FeaturePackage::tools()`.
