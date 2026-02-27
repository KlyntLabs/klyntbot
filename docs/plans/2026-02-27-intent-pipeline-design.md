# Intent Pipeline — Intelligent Auto-Planning Design

**Date:** 2026-02-27
**Status:** Approved
**Scope:** Full rewrite of the routing/execution pipeline — replaces Orchestrator, EngineDispatch, and all execution engines with a unified IntentPipeline that automatically decides when to use plans based on message complexity.

---

## Problem

The current system requires users to explicitly invoke the `plan` tool to use multi-step planning. The orchestrator classifies messages into 4 strategies but the complexity analysis is shallow (keyword matching), mid-execution escalation discards prior work, plans are always visible in the UI, and the explicit plan execution path (`PlanTool "execute"` and dashboard status transition) is broken — `run_plan_execution()` has zero call sites at runtime. The only working plan execution is through `PlanGenerateEngine` (auto-triggered by `AutonomousTask` classification).

## Goals

1. **Auto-escalation**: The system automatically decides Direct vs Reactive vs Planned based on structured complexity analysis — users never need to say "create a plan"
2. **Latent planning**: Plans can be invisible execution graphs that only surface on failure
3. **Mid-execution escalation with context preservation**: ReactiveEngine can hand off to PlannedEngine mid-stream without losing completed work
4. **Task → Plan bridge**: Complex tasks auto-generate execution plans on creation, focus, or explicit "execute" command
5. **Single working execution path**: Unify the broken `plan_runner.rs` and working `plan_generate.rs` into one `PlannedEngine`

## Non-Goals

- Changing the plan data model beyond adding `visibility` and `task_id` columns
- Modifying the plan step execution logic (`run_step`, `build_step_context`, `regenerate_from`)
- Changing the `ExecutionCore::run_cycle()` primitive
- Multi-user or team features

---

## Core Types

```rust
// crates/agent/src/intent_pipeline/types.rs

/// The three execution modes
pub enum ExecutionMode {
    /// Single LLM call, no tools. Greetings, factual Q&A.
    Direct,
    /// ReAct loop with tools. Single-shot tasks, searches, CRUD.
    Reactive { max_iterations: u32 },
    /// Multi-step plan with structured execution.
    Planned { visibility: PlanVisibility, max_steps: u8 },
}

/// Controls whether auto-generated plans appear in the UI
pub enum PlanVisibility {
    /// Never shown. Auto-cleanup after 24h.
    Silent,
    /// Hidden until step failure, then surfaced for review.
    OnFailure,
    /// Always visible (user-created plans, current behavior).
    Transparent,
}

/// Structured complexity analysis output
pub struct ComplexitySignals {
    pub estimated_tool_calls: u8,
    pub has_sequential_deps: bool,
    pub failure_risk: FailureRisk,
    pub requires_state_tracking: bool,
    pub requires_retries: bool,
}

pub enum FailureRisk { Low, Medium, High }

impl ComplexitySignals {
    pub fn complexity_score(&self) -> u8 {
        let mut score: u8 = 0;
        if self.estimated_tool_calls >= 3 { score += 2; }
        else if self.estimated_tool_calls >= 2 { score += 1; }
        if self.has_sequential_deps { score += 2; }
        if self.failure_risk >= FailureRisk::Medium { score += 1; }
        if self.requires_state_tracking { score += 1; }
        if self.requires_retries { score += 1; }
        score
    }
}

/// Result of the full intent analysis
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
}

pub enum AnalysisSource {
    Heuristic,
    LlmClassifier,
    MidExecutionEscalation,
}
```

### Scoring Rule

| Complexity Score | Execution Mode |
|---|---|
| 0 | `Direct` |
| 1 | `Reactive { max_iterations: 5 }` |
| 2 | `Reactive { max_iterations: 10 }` |
| 3+ | `Planned { visibility: config.default, max_steps: 8 }` |

Threshold is configurable via `orchestrator.planComplexityThreshold` (default: 3).

---

## Intent Analyzer (Replaces Orchestrator)

Two-stage classification:

```
User message
    ↓
Stage 1: Heuristics (0ms, deterministic)
    │ confidence >= 0.85 → return early
    │ confidence < 0.85 → Stage 2
    ↓
Stage 2: LLM Classifier (1-2s, cheap model)
    │ Returns ComplexitySignals + mode
    │ confidence < 0.5 → fallback to Reactive
    ↓
IntentAnalysis { mode, signals, confidence }
```

### Stage 1: Enhanced Heuristics

Analyze **structural complexity**, not just keywords:

- `count_tool_indicators(msg)`: count action verbs → estimated_tool_calls
- `detect_sequential_language(msg)`: "first...then", "after that", "once X is done" → has_sequential_deps
- `assess_failure_keywords(msg)`: "deploy", "send email", "book", "purchase" → failure_risk
- `detect_state_requirements(msg)`: "using the result", "based on that" → requires_state_tracking
- `detect_retry_indicators(msg)`: "if it fails", "make sure", "verify" → requires_retries

Returns `Some(IntentAnalysis)` when confidence >= threshold, `None` when ambiguous.

Preserved from current system:
- Greeting detection (short messages, exact words) → `Direct`
- Task management patterns ("create a task", "add a todo") → `Reactive { 5 }` (prevents over-escalation)

### Stage 2: LLM Classifier

Structured output prompt:

```
Analyze this user request and estimate its execution complexity.

Return ONLY valid JSON:
{
  "mode": "direct" | "reactive" | "planned",
  "estimated_tool_calls": <number 0-10>,
  "has_sequential_deps": <boolean>,
  "failure_risk": "low" | "medium" | "high",
  "requires_state_tracking": <boolean>,
  "requires_retries": <boolean>,
  "confidence": <0.0-1.0>,
  "reasoning": "<brief explanation>"
}

User message: "{message}"
Available tools: {tool_names}
{historical_context}
```

Historical context: 30-day strategy accuracy summaries injected to bias future classifications (preserved from current system).

---

## Execution Engine Trait

Unified interface replacing DirectEngine, ReactPlusEngine, PlanGenerateEngine:

```rust
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn execute(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
        params: ExecutionParams,
        ctx: RoutingContext,
    ) -> EngineResult;

    fn mode(&self) -> &str;
}

pub enum EngineResult {
    Complete {
        content: String,
        usage: TokenUsage,
        iterations: u32,
        traces: Vec<ReasoningTrace>,
        tool_name: Option<String>,
    },
    Escalate {
        reason: String,
        carried_context: EscalationContext,
        usage: TokenUsage,
    },
}

pub struct EscalationContext {
    pub messages: Vec<Message>,
    pub completed_work: Vec<CompletedStep>,
    pub original_message: String,
}

pub struct CompletedStep {
    pub description: String,
    pub tool_name: String,
    pub result: String,
}
```

### Three Engine Implementations

#### DirectEngine
Single LLM call, no tools. Returns `Escalate` if LLM wants tools.

#### ReactiveEngine
ReAct loop with tools. After each tool cycle, checks escalation conditions:
- `tool_chain_length > 2`
- Sequential dependency detected in tool results
- `iteration >= ceil(max_iterations * 0.8)`

If any condition true → returns `Escalate { carried_context }` with all completed work preserved.

Preserved features: duplicate tool call detection, fabrication detection, reflection on failure.

#### PlannedEngine
Unified plan generation + execution. Supports two modes:

**Fresh execution** (from IntentAnalyzer classification):
1. Generate 3-8 plan steps via LLM
2. Save plan with configured visibility
3. Execute steps sequentially via `plan_executor::run_step()`
4. Full retry + backtracking support

**Escalation takeover** (from ReactiveEngine escalation):
1. Receive `EscalationContext` with completed work
2. Generate plan steps via LLM, including context of completed work
3. Pre-fill completed steps from `carried_context.completed_work`
4. Continue execution from first pending step
5. No re-execution of prior work

---

## Mid-Execution Escalation

```
ReactiveEngine running (max 10 iterations)
    │
    │ After each tool cycle, check escalation conditions
    │
    ├─ Condition met → EngineResult::Escalate {
    │      carried_context: EscalationContext {
    │          messages: [all accumulated messages],
    │          completed_work: [
    │              CompletedStep { "Searched flights", "web_search", "5 results..." },
    │              CompletedStep { "Fetched pricing", "web_fetch", "$450..." },
    │          ],
    │          original_message: "book the cheapest flight to Tokyo",
    │      }
    │  }
    │
    ↓
ExecutionRouter receives Escalate
    │
    ↓
PlannedEngine::execute() with escalation context
    │
    ├─ LLM generates plan steps with awareness of completed work
    │  Step 1: "Search flights" → Completed (pre-filled from context)
    │  Step 2: "Fetch pricing"  → Completed (pre-filled from context)
    │  Step 3: "Compare options" → Pending ← execution starts HERE
    │  Step 4: "Book cheapest"   → Pending
    │
    ↓
Continues from step 3 without redoing prior work
```

**Budget:** Max 1 escalation per request (configurable via `orchestrator.maxEscalations`).

---

## Latent Planning (Plan Visibility)

### Schema Change

```sql
ALTER TABLE plans ADD COLUMN visibility TEXT NOT NULL DEFAULT 'transparent';
```

### Behavior Matrix

| Scenario | Silent | OnFailure | Transparent |
|---|---|---|---|
| Listed in `GET /api/plans` | No | No (until failure) | Yes |
| Shown in dashboard | No | Only after failure | Yes |
| Results returned to user | Final summary only | Summary; full plan on failure | Full step-by-step |
| Auto-cleanup | 24h after completion | 7d after completion | Never |
| Created by | PlannedEngine (auto) | PlannedEngine (auto) | User (PlanTool, dashboard) |

### API Change

`GET /api/plans` default behavior: excludes `silent` plans, includes `on_failure` only if they have failed steps. New query param `?visibility=all` returns everything.

### Auto-Cleanup

Background task (runs hourly):
- Delete completed `Silent` plans older than 24h
- Delete completed `OnFailure` plans with no failed steps older than 7d
- `Transparent` plans are never auto-deleted

---

## Task → Plan Auto Bridge

### Task Complexity Signals

```rust
pub struct TaskComplexitySignals {
    pub has_dependencies: bool,
    pub subtask_count: u16,
    pub dependency_depth: u8,
    pub estimated_minutes: Option<u32>,
    pub priority: u8,
}

impl TaskComplexitySignals {
    pub fn complexity_score(&self) -> u8 {
        let mut score: u8 = 0;
        if self.has_dependencies { score += 2; }
        if self.subtask_count >= 3 { score += 1; }
        if self.dependency_depth >= 2 { score += 1; }
        if self.estimated_minutes.unwrap_or(0) >= 60 { score += 1; }
        if self.priority <= 2 { score += 1; }
        score
    }

    pub fn is_plan_worthy(&self) -> bool {
        self.complexity_score() >= 3 // configurable
    }
}
```

### Three Trigger Points

**1. On Task Creation** (`TodoTool::handle_add`)
- If `config.todo.auto_plan_suggestion` AND `task.is_plan_worthy()`
- Agent appends suggestion text: "This looks complex. Should I generate an execution plan?"
- Suggestion only, no auto-execution

**2. On Task Focus** (`TodoTool::handle_focus`)
- If `config.todo.auto_plan_on_focus` AND `task.is_plan_worthy()`
- Auto-generates plan with configured visibility
- Plan linked to task via `plans.task_id`

**3. On "Execute Task"** (new `TodoTool` action: `execute`)
- Evaluates task complexity
- If plan-worthy: generates + executes plan via PlannedEngine
- If not: marks task as `doing`, returns guidance
- Plan linked to task via `plans.task_id`

### Plan Generation from Task Context

LLM prompt includes full task context: title, description, priority, estimated duration, subtasks (with statuses), dependencies (with statuses), and attachments.

### Schema Change

```sql
ALTER TABLE plans ADD COLUMN task_id TEXT REFERENCES todos(id) ON DELETE SET NULL;
```

Enables: "View plan" button on TaskDetail, auto-mark task `done` on plan completion, analytics on which tasks generate plans.

---

## Full Pipeline Architecture

```
User Message
    ↓
IntentAnalyzer
  Stage 1: Heuristics (0ms) → if confident, return
  Stage 2: LLM Classifier (1-2s) → structured ComplexitySignals
    ↓
IntentAnalysis { mode, signals, confidence }
    ↓
ContextAssembler (existing ContextEngine, minimal changes)
  Allocates token budget based on mode
    ↓
ExecutionRouter
  Direct  → DirectEngine
  Reactive → ReactiveEngine ──→ Escalate? → PlannedEngine (with context)
  Planned → PlannedEngine
    ↓
ResponseValidator (existing, unchanged)
    ↓
StrategyRecorder (existing, enhanced with complexity_signals)
```

---

## Module Structure

```
crates/agent/src/intent_pipeline/
├── mod.rs              — IntentPipeline (replaces AgentPipeline)
├── types.rs            — ExecutionMode, ComplexitySignals, IntentAnalysis, PlanVisibility
├── analyzer.rs         — IntentAnalyzer (replaces Orchestrator)
├── heuristics.rs       — Enhanced complexity-aware heuristics
├── classifier.rs       — LLM classifier with structured ComplexitySignals output
├── router.rs           — ExecutionRouter (replaces EngineDispatch)
├── engines/
│   ├── mod.rs          — ExecutionEngine trait
│   ├── direct.rs       — DirectEngine
│   ├── reactive.rs     — ReactiveEngine (enhanced with escalation)
│   └── planned.rs      — PlannedEngine (unified generate + execute)
├── escalation.rs       — EscalationContext, escalation handling
└── visibility.rs       — PlanVisibility, auto-cleanup service
```

### Files Deleted

| File | Replaced By |
|---|---|
| `orchestrator/mod.rs` | `intent_pipeline/analyzer.rs` |
| `orchestrator/heuristics.rs` | `intent_pipeline/heuristics.rs` |
| `orchestrator/classifier.rs` | `intent_pipeline/classifier.rs` |
| `execution/dispatch.rs` | `intent_pipeline/router.rs` |
| `execution/direct.rs` | `intent_pipeline/engines/direct.rs` |
| `execution/react_plus.rs` | `intent_pipeline/engines/reactive.rs` |
| `execution/plan_generate.rs` | `intent_pipeline/engines/planned.rs` |
| `pipeline.rs` | `intent_pipeline/mod.rs` |
| `plan_runner.rs` | `intent_pipeline/engines/planned.rs` |

### Files Kept (Unchanged or Minimal Changes)

| File | Changes |
|---|---|
| `execution/core.rs` | Unchanged — low-level `run_cycle()` primitive |
| `execution/scratchpad.rs` | Unchanged — reasoning trace storage |
| `plan_executor.rs` | Unchanged — `run_step()`, `build_step_context()`, `regenerate_from()` |
| `plan_step_generator.rs` | Unchanged — `generate_plan_steps()` |
| `plan_handler.rs` | Unchanged — PlanHandler trait impl for PlanTool |
| `plan_completion_handler.rs` | Unchanged — goal metric updates |
| `context_sources/*` | Unchanged — system prompt assembly |
| `learning/*` | Unchanged — strategy recording, satisfaction tracking |

---

## Schema Changes Summary

```sql
-- Migration: NNN_intent_pipeline.sql

-- 1. Plan visibility
ALTER TABLE plans ADD COLUMN visibility TEXT NOT NULL DEFAULT 'transparent';

-- 2. Task → Plan link
ALTER TABLE plans ADD COLUMN task_id TEXT REFERENCES todos(id) ON DELETE SET NULL;

-- 3. Enhanced strategy recording
ALTER TABLE strategy_records ADD COLUMN complexity_signals TEXT NOT NULL DEFAULT '{}';
ALTER TABLE strategy_records ADD COLUMN execution_mode TEXT;

-- 4. Index for task-plan lookups
CREATE INDEX idx_plans_task_id ON plans(task_id);

-- 5. Index for visibility filtering
CREATE INDEX idx_plans_visibility ON plans(visibility);
```

---

## Configuration

```json
{
  "orchestrator": {
    "heuristicConfidenceThreshold": 0.85,
    "llmClassifierTimeout": 2000,
    "llmClassifierModel": null,
    "defaultPlanVisibility": "on_failure",
    "planComplexityThreshold": 3,
    "maxEscalations": 1
  },
  "todo": {
    "autoPlanSuggestion": true,
    "autoPlanOnFocus": false,
    "planComplexityThreshold": 3
  }
}
```

---

## Testing Strategy

1. **Unit tests for heuristics**: Message → ComplexitySignals → ExecutionMode mapping
2. **Unit tests for complexity scoring**: TaskComplexitySignals threshold checks
3. **Integration tests for escalation**: ReactiveEngine → Escalate → PlannedEngine with mock LLM
4. **Integration tests for plan visibility**: Silent/OnFailure/Transparent filtering in API
5. **Integration tests for Task→Plan bridge**: Focus trigger, execute trigger, suggestion on create
6. **End-to-end pipeline test**: Message → IntentAnalyzer → Router → Engine → Response

All tests use `StoragePool::connect_in_memory()` for ephemeral SQLite.
