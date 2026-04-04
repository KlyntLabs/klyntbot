# Budget-Bounded Execution Model — Design Spec

## Vision

Transform Klyntbot from a time-bounded chatbot into a budget-bounded second brain that scales cognition on demand. Inspired by Claude Code's architecture (no wall-clock timeout, token budget, streaming-first) but upgraded significantly for a personal AI agent: user-controlled depth modes, adaptive depth suggestions via Mirror, progressive cognitive enrichment, and a live budget HUD.

## Problem Statement

The current agent pipeline uses an 11-step process with an LLM-based intent classifier and a single wall-clock timeout (300s default). This causes:

1. **3-5s tax per message** — IntentAnalyzer makes an LLM call before execution even starts
2. **Arbitrary timeout kills complex work** — FIRE analysis, deep research, 15-task decomposition all die at 300s regardless of progress
3. **Total failure on timeout** — no partial results, all work lost
4. **No user control** — users can't choose how deeply the agent thinks
5. **Provider-dependent failures** — slow providers (DeepSeek at 3-5s/call) hit the timeout faster despite doing the same work
6. **Classification outside timeout** — production bug: the classifier LLM call runs before the timeout wrapper, so a hung classifier blocks indefinitely

## Architecture

### Core Principle: Budget, Not Clock

Replace the single 300s wall-clock timeout with a **token/turn budget** that the user can see, control, and extend. The system defaults to generous, silent budgets for daily use. Depth modes (Normal/Deep Think/Ultra) let users scale cognition on demand. The budget is a creative tool, not a constraint.

### New Pipeline (6 phases, 0 classifier LLM calls)

```
Phase 1: Route        (0ms)   — SkillRouter heuristic only (Aho-Corasick + keyword)
Phase 2: Prepare      (<5ms)  — context assembly + tool filtering + depth resolution
Phase 3: Execute loop (budget) — streaming LLM + parallel tools + budget checks
Phase 4: Enrich       (async) — Mirror/Coaching/Notes/FSRS (depth-dependent)
Phase 5: Record       (async) — usage, strategy, autotuner ground truth
Phase 6: Adapt        (async) — depth history for future suggestions
```

### Old Pipeline (removed/simplified)

```
Step 0a: AutoTuner shadow         → Moved to Phase 5 (async)
Step 0b: Query embedding          → Removed (not needed without LLM classifier)
Step 1:  SkillRouter              → Phase 1 (unchanged)
Step 2:  Set profile              → Phase 2 (unchanged)
Step 2a: Activate per-msg skills  → Phase 2 (unchanged)
Step 3:  Filter tools             → Phase 2 (simplified)
Step 4:  IntentAnalyzer (LLM!)    → REMOVED ENTIRELY
Step 5:  Confidence check         → REMOVED (no classifier = no confidence)
Step 6:  Assemble context         → Phase 2
Step 7:  Filter tools by profile  → Phase 2
Step 7c: Chain-of-thought plan    → Removed (LLM plans inline)
Step 8:  Execute (wall-clock)     → Phase 3 (budget-bounded)
Step 9:  Validate                 → End of Phase 3 loop
Step 10: Record usage             → Phase 5 (async, non-blocking)
Step 11: AutoTuner ground truth   → Phase 5 (async)
```

## Depth Modes

Three user-selectable depth modes, presented as pills under the chat input:

### Normal (default)

- Budget: skill-default tokens + turns (e.g., task-management: 40K/12 turns)
- Subtle depth status line at top of chat: `Normal • 12 turns • $0.004  [Deep Think?]`
  - Tiny gray text, always dismissible, consistent across all modes
  - One-tap `[Deep Think?]` escalates the current session without re-typing
  - When Mirror has an adaptive suggestion, the link pulses briefly
  - Provides visual continuity so Deep/Ultra HUD doesn't feel like a sudden mode switch
- No Mirror/Coaching injection
- No auto-save
- Feels: instant, frictionless — like your own quick thoughts

### Deep Think

- Budget: skill-default × 1.5
- Live Budget HUD visible
- Mirror context injected into system prompt
- Coaching evaluation post-response
- Feels: the agent is thinking *with* you

### Ultra

- Budget: skill-default × 3 (or unlimited, bounded only by monthly budget)
- Live Budget HUD visible + pinnable results
- Mirror context + mid-loop memory injections
- Coaching mid-loop + post-response
- Auto-save results to NoteTree
- Auto-create FSRS atoms if learning-related
- Progressive enrichment visible in HUD stream
- Feels: full cognitive partner — you watch the brain grow

### Depth Resolution (with adaptive layer)

```
1. User explicitly chose a depth → honor it
2. Check DepthHistory for this skill/topic:
   - If user usually uses Deep/Ultra → suggest it (highlight pill)
   - One-tap to accept, no auto-escalation
3. Mirror can suggest mid-conversation:
   - "This looks like your FIRE deep-dive pattern — switch to Deep Think?"
   - Appears as a dismissible chip for 5 seconds
4. Default: Normal, no suggestion
```

### Skill-Aware Default Budgets

| Skill | Normal Tokens | Normal Turns | Deep Tokens | Deep Turns |
|-------|-------------|------------|-----------|----------|
| general | 60K | 15 | 90K | 25 |
| task-management | 40K | 12 | 60K | 18 |
| finance-management | 80K | 20 | 120K | 30 |
| communication | 40K | 10 | 60K | 15 |
| automation | 50K | 15 | 75K | 22 |

Ultra: 3× Normal tokens, turns unlimited (bounded by monthly budget only).

## ExecutionBudget

```rust
pub struct ExecutionBudget {
    max_tokens: u64,
    max_turns: u32,
    tokens_used: u64,
    turns_used: u32,
    wrap_up_pct: f32,       // 0.85 — inject "wrap up" instruction at this point
    reserved_synthesis: u64, // 2000 tokens — always reserved for final response
    depth: DepthMode,
}
```

### Budget lifecycle:

1. **Created** from `DepthMode + SkillMatch` at Phase 2
2. **Checked** before every LLM call in Phase 3
3. **Deducted** after every LLM response (real token usage from provider)
4. **Wrap-up injected** at 85% usage — system prompt addition: "Please provide your final response with the results you have so far."
5. **Exhausted** at 95% — force synthesis with accumulated results. NO total failure.
6. **Extendable** — user can tap "Extend +20 turns" in HUD mid-conversation

### Budget vs wall-clock:

- Token/turn budget: the real constraint. Correlates with work done.
- Wall-clock: 600s safety net only. Catches deadlocks/infinite loops. Should never fire in normal operation. If it does, it's a bug report.

## Execute Loop (Phase 3)

The core loop, replacing DirectEngine + ReactiveEngine + pipeline timeout:

```rust
loop {
    // 1. Budget gate
    if budget.should_wrap_up() {
        messages.push(system("Provide your final response with available results."));
    }
    if budget.exhausted() {
        return synthesize_partial(accumulated_results);
    }

    // 2. LLM call (streaming to user in real-time)
    let response = provider.chat_stream(messages, &tools, &params).await?;
    budget.deduct(&response.usage);
    emit(AgentEvent::ContentChunk { ... });

    // 3. Model decided it's done (no tool calls)
    if response.tool_calls.is_empty() {
        break;
    }

    // 4. Execute tools in parallel (existing infrastructure)
    let results = execute_tools_parallel(&response.tool_calls, tool_timeout).await;
    messages.extend(tool_results_to_messages(&results));

    // 5. Live context refresh (memory promotions, Mirror discoveries)
    if let Some(updates) = context_queue.drain() {
        messages.extend(updates);
    }

    // 6. Mid-loop compression (existing MidLoopCompressor)
    compressor.compress_if_needed(&mut messages, budget.remaining_tokens());

    turn += 1;
    emit(AgentEvent::TurnComplete { turn, budget_pct: budget.remaining_pct() });
}
```

### What determines loop termination:

| Condition | Behavior |
|-----------|----------|
| Model returns text only (no tool_use) | Clean exit — model is done |
| Token budget at 85% | Inject wrap-up instruction, continue |
| Token budget at 95% | Force synthesis, return partial results |
| Turn limit reached | Force synthesis, return partial results |
| User cancels (AbortController) | Return partial results immediately |
| Tool hangs (30s per-tool timeout) | Tool returns error, loop continues |
| Provider error (HTTP 429/500) | Retry with backoff (existing withRetry) |
| Safety wall-clock (600s) | Emergency stop — indicates a bug |

### Key differences from old model:

- **No pre-execution classification** — LLM self-selects via tool_use blocks
- **No Direct vs Reactive split** — unified loop handles both cases
- **No wall-clock as primary constraint** — budget is the constraint
- **Partial results always returned** — never total failure
- **User sees progress** — streaming + HUD + enrichment stream

## Enrichment Phase (Phase 4)

Post-response async work that runs without blocking the user:

| Enrichment | Normal | Deep Think | Ultra |
|-----------|--------|------------|-------|
| Mirror reflection | Skip | tokio::spawn | tokio::spawn + emit progress |
| Coaching evaluation | Skip | tokio::spawn | tokio::spawn + emit progress |
| NoteTree auto-save | Skip | Skip | tokio::spawn + emit progress |
| FSRS atom creation | Skip | Skip | tokio::spawn + emit progress |
| InsightReview generation | Skip | Skip | tokio::spawn + emit progress |

In Ultra mode, enrichment progress is streamed to the HUD:
```
"Mirror spotted a 3-month pattern in FIRE notes…"
"Creating 5 knowledge atoms for spaced repetition…"  
"Building InsightReview — tap to pin any perspective"
```

User can interact mid-enrichment: "Pin this", "Save as note now", "Skip".

## New AgentEvent Variants

```rust
pub enum AgentEvent {
    // Existing events (unchanged)
    PipelineStarted { ... },
    ContentChunk { data: String },
    ToolStart { name: String, input: Value },
    ToolEnd { name: String, success: bool, result: Option<String>, duration_ms: u64 },
    IterationStart { iteration: usize },
    Error { message: String },
    UsageReport { ... },

    // NEW: Budget HUD (Deep/Ultra only)
    BudgetUpdate { tokens_remaining_pct: f32, turns_used: u32, cost_usd: f64 },
    BudgetExtended { additional_turns: u32 },

    // NEW: Depth suggestion (adaptive layer)
    DepthSuggestion { recommended: DepthMode, reason: String },

    // NEW: Enrichment progress (Ultra visible, Deep optional)
    EnrichmentStarted { phase: String },
    EnrichmentComplete { phase: String, summary: String },

    // NEW: Turn tracking
    TurnComplete { turn: u32, budget_remaining_pct: f32 },
}
```

## Configuration Changes

### Remove

- `OrchestratorConfig.llm_classifier_timeout` — no classifier
- `OrchestratorConfig.llm_classifier_model` — no classifier
- `AgentDefaults.pipeline_timeout_secs` — replaced by budget

### Add

```rust
pub struct ExecutionConfig {
    /// Safety wall-clock timeout (deadlock catcher). Default: 600s.
    pub safety_timeout_secs: u64,

    /// Default depth mode. Default: Normal.
    pub default_depth: DepthMode,

    /// Per-skill budget overrides.
    pub skill_budgets: HashMap<String, SkillBudget>,

    /// Enable adaptive depth suggestions. Default: true.
    pub adaptive_depth: bool,
}

pub struct SkillBudget {
    pub normal_tokens: u64,
    pub normal_turns: u32,
    pub deep_multiplier: f32,   // 1.5
    pub ultra_multiplier: f32,  // 3.0
}
```

### Keep (unchanged)

- `AgentDefaults.monthly_budget_usd` — monthly cost ceiling
- `AgentDefaults.max_concurrent_subagents` — unchanged
- `AgentDefaults.model`, `temperature`, `max_tokens` — unchanged

## Impact on Existing Systems

### IntentAnalyzer

- **Keep:** Layer 1 (Aho-Corasick heuristics) for complexity signal generation
- **Keep:** Layer 2 (embedding fallback) for skill routing boost
- **Remove:** Layer 3 (LLM classifier call)
- **Remove:** Layer 4 (cognitive boost)
- **New role:** Lightweight signal generator, not mode selector. Runs in <1ms.

### ExecutionRouter

- **Simplify:** Remove Direct/Reactive mode switching
- **New role:** Just wraps the unified Execute Loop
- **Keep:** Escalation detection (if first response has tool_calls but was started without tools)

### DirectEngine / ReactiveEngine

- **Merge:** Into a single `ExecuteLoop` that handles both cases
- **DirectEngine behavior:** Loop iteration 1, model returns text only → exit
- **ReactiveEngine behavior:** Loop iterates until model is done or budget exhausted

### CostTracker

- **Keep:** Monthly budget tracking
- **Add:** Per-request budget enforcement (checked before each LLM call)
- **Add:** Real-time cost emission via AgentEvent::BudgetUpdate

### Simulator

- **Fix:** The 60s timeout wrapper becomes the 600s safety net
- **Fix:** No more classification timeout issues
- **Add:** ExecutionBudget with configurable depth for scenarios
- **Result:** All Tier 5-6 metrics will produce real data

## Migration Path

Since we haven't released production yet, this is a clean breaking change:

1. **Create ExecutionBudget struct** in `crates/agent/src/execution/`
2. **Create unified ExecuteLoop** merging Direct+Reactive engines
3. **Simplify process_message()** from 11 steps to 6 phases
4. **Remove IntentAnalyzer Layer 3+4** (keep Layer 1-2 as signal generator)
5. **Add DepthMode to message processing API** (new parameter)
6. **Add new AgentEvent variants** for HUD
7. **Update config schema** (remove pipeline_timeout, add ExecutionConfig)
8. **Update simulator** to use ExecutionBudget
9. **Update desktop UI** to render depth pills + HUD (separate PR)

## UX Polish Details

### Budget Extend Button

Context-aware copy instead of generic "+20 turns":
- Finance context: "Think deeper on FIRE (+20 turns)"
- Research context: "Continue research (+20 turns)"
- Task context: "Keep planning (+20 turns)"

Uses the active skill name to generate the label.

### Ultra First-Use Confirmation

Soft confirmation the first time a user selects Ultra for a new skill:
- "Ultra will use up to 3× budget and auto-save to notes. Continue?"
- One-time per skill (persisted in user preferences)
- Skippable via settings toggle

### Monthly Budget in HUD

When monthly budget hits 80%, the depth status line integrates the warning:
- `Normal • 12 turns • Monthly budget at 82% [Deep Think?]`
- In Deep/Ultra HUD: amber indicator replaces the cost display
- "Still want to go Ultra?" prompt appears on depth escalation
- Keeps everything in one mental model — no separate toasts or modals

## Non-Goals

- **Multi-model routing** (using different models for different depths) — future enhancement, not in this spec
- **Inter-agent budget sharing** (parent + subagent shared pool) — future, keep independent tracking
- **Automatic depth escalation without user consent** — always suggest, never auto-escalate
- **Billing/payment integration** — monthly_budget_usd is a local config, not connected to payments
