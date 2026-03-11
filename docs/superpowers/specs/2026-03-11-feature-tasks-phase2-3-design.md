# feature-tasks Phase 2-3: Agentic Intelligence & Proactive Ecosystem

**Goal:** Add AI-powered task decomposition, autonomous execution, intelligent planning, proactive suggestions, estimation forecasting, and deep cognitive memory integration to the agentic task system built in Phase 1.

**Architecture:** Phase 2 adds three LLM-powered handler traits (DecompositionHandler, TaskExecutionHandler, DayPlanningHandler) implemented in the agent crate (L5) using dependency inversion — traits defined in feature-tasks (L4), implementations in agent (L5). Phase 3 adds two more handlers (ProactiveHandler + SuggestionApplier, ForecastHandler) and wires the entire task lifecycle into the cognitive memory pipeline for continuous learning.

**Prerequisite:** Phase 1 must be complete — all tables, types, config fields, and handler trait stubs must exist.

**Design decisions locked in brainstorming:**
- Task execution model: **Hybrid** (C) — agentic tasks get subagents, manual tasks get planning/monitoring
- Cost budgeting: **Tiered by complexity** (C) — auto-assign budget from complexity_score, user can override via ExecutionConfig
- Suggestion surfacing: **Inbox + inline** (D) — global suggestions in inbox, task-specific inline
- Decomposition approval: **Confidence-gated** (C) — auto-apply above threshold (0.75), review below
- Calendar integration: **Calendar-optional** (B) — graceful degradation if MCP unavailable

---

## Phase 2: Agentic Intelligence

### 2.1 DecompositionHandler

**Purpose:** AI breaks complex tasks into an executable subtask tree, leveraging cognitive memory, user energy patterns, and calendar context for intelligent decomposition. Confidence-gated auto-apply ensures simple decompositions are instant while ambiguous ones get human review.

**Flow:**

1. User (or agent) triggers decomposition on a task
2. Handler builds `DecompositionContext` — pulls cognitive facts from memory, user energy profile, calendar blocks, existing subtasks
3. LLM generates a `DecompositionTree` using structured output with `temp_id` references for inter-subtask dependencies
4. **Post-LLM validation & repair**: check for circular deps, duplicate titles, unrealistic estimates, criteria coverage. Auto-repair where safe; otherwise reduce confidence
5. **Confidence gate** (from `TasksConfig.decomposition_auto_apply_threshold`, default 0.75):
   - At or above threshold → auto-create subtasks in `tasks` table, link via `parent_id`, wire dependencies
   - Below threshold → store as pending `DecompositionPlan` in `task_decompositions` for user review
6. Log activity (`ActivityType::Decomposed`), emit `DomainEvent::TaskDecomposed`
7. If parent `acceptance_criteria` exists, auto-parse into verifiable steps and distribute across subtasks — validation ensures every criterion maps to at least one subtask

**Trait:**

```rust
#[async_trait]
pub trait DecompositionHandler: Send + Sync {
    async fn decompose(
        &self,
        task: &Task,
        context: &DecompositionContext,
    ) -> Result<DecompositionResult>;
}
```

**Types:**

```rust
pub struct DecompositionContext {
    pub max_depth: u32,                                  // default 2
    pub max_subtasks_per_level: u32,                     // default 10
    pub existing_subtasks: Vec<Task>,
    pub project_context: Option<String>,
    pub cognitive_facts: Vec<SemanticFact>,
    pub user_energy_profile: Option<EnergyProfile>,
    pub calendar_context: Option<Vec<CalendarBlock>>,
}

pub struct DecompositionResult {
    pub tree: DecompositionTree,
    pub confidence: f64,
    pub reasoning: String,
    pub validation_warnings: Vec<ValidationWarning>,
}

pub struct DecompositionTree {
    pub subtasks: Vec<PlannedSubtask>,
    pub total_estimated_mins: Option<i64>,
}

pub struct PlannedSubtask {
    pub temp_id: String,                    // stable ID within decomposition (e.g., "sub-1")
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub energy_level: Option<EnergyLevel>,
    pub priority: Option<i32>,
    pub task_type: Option<TaskType>,
    pub dependencies: Vec<String>,          // references to sibling temp_ids
    pub children: Vec<PlannedSubtask>,
}

pub struct ValidationWarning {
    pub kind: ValidationWarningKind,
    pub message: String,
    pub auto_repaired: bool,
}

pub enum ValidationWarningKind {
    CircularDependency, DuplicateTitle, UnrealisticEstimate,
    MissingCriteriaCoverage, ExcessiveDepth, ExcessiveSubtaskCount,
}
```

**Validation & repair rules:**

| Check | Detection | Repair | Confidence penalty |
|-------|-----------|--------|--------------------|
| Circular dependencies | Topological sort on temp_id graph | Remove back-edge dependency | -0.15 per cycle |
| Duplicate titles | Case-insensitive comparison | Append disambiguating suffix | -0.05 per duplicate |
| Unrealistic total estimate (sum) | Sum > 3× parent estimate | Redistribute proportionally, flag | -0.10 |
| Unrealistic single subtask | Single subtask > 480 mins | Cap at 240 mins, flag | -0.05 |
| Missing criteria coverage | Parent criteria not mapped to any subtask | Flag uncovered (cannot auto-repair) | -0.10 per uncovered |
| Excessive depth | Branch > max_depth | Flatten: promote children up | -0.05 |
| Excessive count | Level > max_subtasks_per_level | Truncate to limit | -0.10 |

The LLM must output a `confidence: f64` (0.0–1.0) in its structured response. Post-validation penalties are then applied to this value. Confidence floor: 0.3. Below that, always requires review regardless of threshold.

**LLM prompt:** Loaded via `include_str!("../templates/decomposition_prompt.md")` at `crates/agent/src/templates/decomposition_prompt.md`.

**Implementation:** `crates/agent/src/handlers/decomposition.rs` (L5)

---

### 2.2 TaskExecutionHandler

**Purpose:** Spawn a subagent to autonomously work on agentic/hybrid tasks. Reads acceptance criteria, uses tools, streams real-time progress, tracks costs, and automatically persists artifacts. Hybrid tasks require explicit user approval before execution begins.

**Flow:**

1. Task must have `task_type` = `agentic` or `hybrid` and `execution_state` = `idle` or `failed`
2. Compute cost budget from `complexity_score` tier (or `ExecutionConfig.max_cost_usd` override)
3. **Approval gate** (hybrid tasks or `ExecutionConfig.require_approval == true`):
   - Create `TaskSuggestion` (type: `Execute`, status: `pending`), set `execution_state` → `awaiting_approval`. Note: `SuggestionType::Execute` must be added to the Phase 1 enum (11th variant).
   - Return `ExecuteResult::AwaitingApproval { suggestion_id }`
4. Create `TaskExecution` row (status: `pending`)
5. Build `ContextSnapshot`, update task: `execution_state` → `running`
6. Spawn subagent via `SpawnHandler` with budget ceiling and `CancellationToken`
7. Subagent emits `DomainEvent::TaskExecutionProgress` at `progress_interval_secs` intervals
8. On completion: parse artifacts → create `task_attachments` rows (source: `agent`), update execution, emit events
9. On failure: check `RetryPolicy` — agentic defaults to `auto_retry=true`, hybrid to `false`

**Trait:**

```rust
#[async_trait]
pub trait TaskExecutionHandler: Send + Sync {
    async fn execute(&self, task: &Task, config: &ExecutionConfig) -> Result<ExecuteResult>;
    async fn get_execution(&self, execution_id: &str) -> Result<TaskExecution>;
    async fn cancel_execution(&self, execution_id: &str) -> Result<()>;
    async fn retry_execution(&self, execution_id: &str) -> Result<ExecuteResult>;
}

pub enum ExecuteResult {
    Started { execution_id: String },
    AwaitingApproval { suggestion_id: String },
}
```

**Cost budgets by complexity tier:**

| Complexity score | Budget |
|-----------------|--------|
| 0–2 (low) | $0.25 |
| 3–4 (medium) | $1.00 |
| 5–6 (high) | $3.00 |
| 7+ (deep) | $5.00 |

Overridable via `ExecutionConfig.max_cost_usd`. At 80%: budget warning. At 100%: graceful stop with status `BudgetExceeded`.

**Retry defaults by task type:**

| Task type | auto_retry | max_retries |
|-----------|-----------|-------------|
| agentic | true | 2 |
| hybrid | false | 2 |
| manual | false | 0 |

**Cancellation:** `cancel_execution()` propagates `CancellationToken` via `SpawnHandler::cancel()` → subagent finishes current tool call, synthesizes partial summary, exits.

**Artifact handling:** Subagent output parsed for artifacts → stored in both `task_executions.artifacts` (JSON) AND `task_attachments` rows with `source = 'agent'`.

**LLM prompt:** Loaded via `include_str!("../templates/execution_prompt.md")` at `crates/agent/src/templates/execution_prompt.md`.

**Implementation:** `crates/agent/src/handlers/execution.rs` (L5)

---

### 2.3 DayPlanningHandler

**Purpose:** AI-powered daily planning combining pre-computed scores, energy matching, calendar awareness, and cognitive memory. Produces time-slotted `DayPlan`. Supports mid-day replanning with locked slots. Falls back to scoring-based ranking when LLM handler unavailable.

**Flow:**

1. Tool action assembles `PlanningContext` with pre-scored `Vec<ScoredTask>` (handler does NOT re-score)
2. If `DayPlanningHandler` injected → LLM-powered planning; if not → Phase 1 scoring fallback
3. LLM generates time-slotted plan with energy-appropriate assignments
4. Post-LLM validation: time overflow, dependency violations, energy coherence, dedup
5. If `TasksConfig.auto_apply_day_plan == true`: auto-focus first slot's task
6. Return `DayPlan` with `slots` + `locked_slots` (for replanning)

**Trait:**

```rust
#[async_trait]
pub trait DayPlanningHandler: Send + Sync {
    async fn plan_day(&self, context: &PlanningContext) -> Result<DayPlan>;
    async fn replan(&self, context: &PlanningContext, current_plan: &DayPlan, reason: &str) -> Result<DayPlan>;
}
```

**Energy matching (time-of-day defaults, overridden by EnergyProfile from cognitive memory):**

| Time of day | Default energy | Best task types |
|-------------|---------------|-----------------|
| Early morning (start → start+2h) | high | Creative, complex |
| Mid-morning (start+2h → lunch_start) | deep | Research, focused |
| Post-lunch (lunch_start → lunch_start+2h) | low | Admin, routine |
| Afternoon (lunch_start+2h → end) | medium | Standard work, reviews |

`lunch_start` derived from `WorkingHours` — defaults to 12:00. Phase 1's `WorkingHours` struct must include a `lunch_start: NaiveTime` field (default `12:00`).

**Replanning:** Completed/active slots → `locked_slots` (non-editable). Pending/skipped → return to candidate pool. Re-run LLM with disruption reason.

**Fallback:** If handler not injected or fails → Phase 1 scoring-based top-N list (no time-slotting).

**LLM prompt:** Loaded via `include_str!("../templates/day_plan_prompt.md")` at `crates/agent/src/templates/day_plan_prompt.md`.

**Implementation:** `crates/agent/src/handlers/planning.rs` (L5)

---

## Phase 3: Proactive Ecosystem

### 3.1 ProactiveHandler + SuggestionApplier

**Purpose:** Generate actionable suggestions without being asked. Three trigger sources: event-driven (DomainEventBus subscriber), periodic scan (cron), on-demand. Separate `SuggestionApplier` trait executes accepted actions. Suggestions auto-expire when task state changes.

**Traits:**

```rust
#[async_trait]
pub trait ProactiveHandler: Send + Sync {
    async fn suggest(&self, scope: &SuggestionScope) -> Result<Vec<SuggestionCandidate>>;
    async fn evaluate_task(&self, task: &Task, trigger: &SuggestionTrigger) -> Result<Vec<SuggestionCandidate>>;
}

#[async_trait]
pub trait SuggestionApplier: Send + Sync {
    async fn apply(&self, suggestion_id: &str, task_id: Option<&str>, action: &SuggestionAction) -> Result<String>;
}
```

**Suggestion lifecycle:**

```
Created (pending) → Accepted → Applied (SuggestionApplier.apply() executes action AND updates status to Applied)
                  → Dismissed (user disagrees)
                  → Expired (task state changed significantly)
                  → Auto-applied (confidence ≥ 0.83: SuggestionApplier.apply() executes AND sets status to AutoApplied)
```

**Event-driven triggers:**

| Event | Trigger | Likely suggestions |
|-------|---------|-------------------|
| TaskCompleted with deviation > 30% | EstimationDeviation | AdjustEstimation |
| TaskExecutionFailed retry ≥ 2 | ExecutionFailed | Decompose, Abandon |
| TaskFocusEnded duration < est/3 | FocusAbandonedEarly | Decompose, AdjustEnergy |
| Task overdue (cron) | TaskOverdue | Reschedule, Reprioritize |
| Task in "doing" > stale_task_days (cron) | TaskStale | Decompose, Abandon |
| In-progress > wip_limit (cron) | WipLimitExceeded | Reprioritize, WorkflowInsight |
| Blocked chain stale > `stale_task_days` (cron) | BlockedChainStale | Unblock, RemoveBlocker |

**Auto-expiration:** DomainEventBus subscriber watches for TaskCompleted/TaskUpdated(status→someday)/task deleted/archived → marks all pending suggestions for that task as `expired`.

**Per-scope config:** `project_overrides` and `area_overrides` in TasksConfig allow per-project/area WIP limits and stale thresholds.

**Deduplication:** Before persisting, check for existing pending suggestion with same `(task_id, suggestion_type)`. Update if higher confidence, skip if lower.

**LLM prompt:** Loaded via `include_str!("../templates/proactive_suggestions.md")` at `crates/agent/src/templates/proactive_suggestions.md`.

**Implementation:** `crates/agent/src/handlers/proactive.rs` (ProactiveHandler) and `crates/agent/src/handlers/suggestion_applier.rs` (SuggestionApplier) (L5)

---

### 3.2 ForecastHandler

**Purpose:** Predict task completion likelihood and estimate accuracy by learning from `task_estimation_history`. Task-level forecasts adjust estimates based on historical deviation. Project-level forecasts compute velocity and projected completion. Accuracy reports help users calibrate.

**Trait:**

```rust
#[async_trait]
pub trait ForecastHandler: Send + Sync {
    async fn forecast_task(&self, task: &Task, context: &ForecastContext) -> Result<TaskForecast>;
    async fn forecast_project(&self, project_id: &str, context: &ForecastContext) -> Result<ProjectForecast>;
    async fn accuracy_stats(&self, scope: &AccuracyScope) -> Result<AccuracyReport>;
}
```

**Similarity matching (for finding comparable completed tasks):**

| Criterion | Weight | Match logic |
|-----------|--------|-------------|
| Tags overlap | 0.35 | Jaccard similarity |
| Energy level | 0.20 | Exact=1.0, adjacent=0.5, else 0.0. Order: low < medium < high < deep; adjacent = differs by exactly one step |
| Complexity score | 0.20 | max(0.0, 1.0 - (abs(a-b) / 10.0)). Clamped to [0.0, 1.0] |
| Same project | 0.15 | Exact=1.0, else 0.0 |
| Recency | 0.10 | e^(-days_ago/30) |

Threshold: similarity ≥ 0.3. Relaxed to 0.1 if sample below `min_sample_size`. Only tasks completed within `forecast_lookback_days` (default 90) are considered as candidates.

**Deviation correction:**

```
adjusted_estimate = original × (1.0 + mean_deviation)
optimistic = original × (1.0 + mean_deviation - std_deviation)
pessimistic = original × (1.0 + mean_deviation + std_deviation)
```

**Data quality tiers:**

| Sample size | Quality | Confidence |
|------------|---------|------------|
| 20+ | Strong | High |
| 10–19 | Moderate | Decent |
| 5–9 | Weak | Usable |
| <5 | Insufficient | Speculative |

**Velocity (project forecast):**

```
velocity = completed_mins_last_4_weeks / 4
projected_weeks = adjusted_remaining_mins / velocity
```

If velocity is zero (no completions in last 4 weeks), return `ProjectForecast` with `projected_completion: None` and a `ForecastRisk` noting insufficient data. Do not attempt to compute `projected_weeks`.

**Architecture split:**
- Pure computation (similarity, deviation, velocity, accuracy stats): `crates/feature-tasks/src/forecast.rs` (L4, no LLM)
- LLM-enhanced (risk narratives, mitigation suggestions): `crates/agent/src/handlers/forecast.rs` (L5)

**Proactive trigger:** If forecast reveals deadline risk (severity ≥ 0.7), triggers `ProactiveHandler.evaluate_task()` for follow-up suggestions. The L5 ForecastHandler impl holds `Option<Arc<dyn ProactiveHandler>>` — injected at construction. If `None`, risk is returned in `ForecastRisk` without triggering proactive flow (caller can trigger separately).

---

### 3.3 Cognitive Integration

**Purpose:** Wire the task system into the cognitive memory pipeline for continuous learning about user work patterns, estimation habits, energy rhythms, and productivity dynamics.

**Three pillars:**

#### Pillar 1: Event → Observation mapping

Enhanced observation mapping with importance scoring in `BackgroundConsolidationService`:

| Event | Base importance | Boost conditions |
|-------|----------------|------------------|
| TaskCompleted (on time) | 0.4 | +0.3 if deviation > 50%, +0.1 if P1 |
| TaskCompleted (overdue) | 0.6 | +0.1 if P1 |
| TaskUpdated(status→someday) (1st) | 0.3 | — |
| TaskUpdated(status→someday) (3+) | 0.7 | Repeated pattern → ExtractNow |
| TaskExecutionFailed (1st) | 0.4 | — |
| TaskExecutionFailed (3+) | 0.8 | Note: overrides Phase 1's unconditional `ExtractNow` stub for this event |
| TaskFocusEnded | 0.3 | — |
| EstimationRecorded | 0.3 | +0.3 if deviation > 50% |

Importance formula: `min(base + modifiers, 1.0)`. Events at importance ≥ 0.7 → episodic memory.

#### Pillar 2: Observation → Fact extraction

Expected semantic facts the system learns over time:

| Subject | Predicate | Example | Source |
|---------|-----------|---------|--------|
| user | peak_focus_hours | "9:00-11:30" | TaskFocusStarted/Ended |
| user | estimation_bias | "+38% underestimation" | EstimationRecorded |
| user | estimation_bias_{tag} | "+55% for rust tasks" | EstimationRecorded by tag |
| user | preferred_energy_{period} | "deep in morning" | TaskFocusStarted by time |
| user | tasks_completed_per_week | "12.5 average" | TaskCompleted |
| user | agentic_success_rate | "78% (7/9)" | TaskExecution events |
| project:{id} | completion_pace | "3.2 tasks/week" | TaskCompleted by project |
| user | deferral_pattern | "defers 'planning' tasks to someday" | TaskUpdated(status→someday) |
| tag:{name} | typical_duration | "45 min median" | EstimationRecorded by tag |

Accumulation → promotion thresholds: 3+ observations within same pattern for facts, 5+ for procedural rules.

#### Pillar 3: Fact → Task feedback loop

| Consumer | Facts used | How |
|----------|-----------|-----|
| EnrichmentHandler | estimation_bias_{tag}, typical_duration | Auto-adjust suggested estimates |
| DecompositionHandler | preferred_task_size, estimation_bias | Size subtasks appropriately |
| DayPlanningHandler | peak_focus_hours, preferred_energy_* | Populate EnergyProfile, optimize slots |
| TaskExecutionHandler | agentic_success_rate | Adjust retry policy, flag risky executions |
| ProactiveHandler | deferral_pattern, estimation_bias | Trigger relevant suggestions |
| ForecastHandler | estimation_bias, project velocity | Correct predictions |

**cognitive_bridge.rs** (L4, in feature-tasks): Typed helpers for parsing cognitive facts into domain structures:

```rust
pub fn extract_energy_profile(facts: &[SemanticFact]) -> Option<EnergyProfile>
pub fn extract_estimation_bias(facts: &[SemanticFact], tags: &[String]) -> Option<f64>
pub fn extract_velocity(facts: &[SemanticFact], project_id: Option<&str>) -> Option<f64>
pub fn extract_deferral_patterns(facts: &[SemanticFact]) -> Vec<String>
pub fn extract_agentic_success_rate(facts: &[SemanticFact]) -> Option<f64>
```

---

## Cross-cutting concerns

### DomainEvent variants (all defined in Phase 1, consumed in Phase 2-3)

Phase 1 defines these variants. Phase 2-3 emits and handles them:

| Variant | Emitted by | Consumed by |
|---------|-----------|-------------|
| TaskDecomposed | DecompositionHandler | Cognitive, ProactiveHandler |
| TaskExecutionStarted | TaskExecutionHandler | Cognitive |
| TaskExecutionProgress | TaskExecutionHandler | Desktop UI (live monitor) |
| TaskExecutionCompleted | TaskExecutionHandler | Cognitive, ForecastHandler |
| TaskExecutionFailed | TaskExecutionHandler | Cognitive, ProactiveHandler |
| TaskBlocked | Dependency system | ProactiveHandler |
| TaskUnblocked | Dependency system | ProactiveHandler |
| DayPlanGenerated | DayPlanningHandler | Cognitive |
| ProactiveSuggestionCreated | ProactiveHandler | Desktop UI (badge) |
| TaskFocusStarted | Focus system | Cognitive |
| TaskFocusEnded | Focus system | Cognitive, ProactiveHandler |
| EstimationRecorded | Completion handler | Cognitive, ForecastHandler |

### Schema (all tables defined in Phase 1, used in Phase 2-3)

| Table | Phase 2-3 usage |
|-------|----------------|
| task_executions | TaskExecutionHandler CRUD |
| task_suggestions | ProactiveHandler persistence + auto-expiration |
| task_decompositions | DecompositionHandler pending plans |
| task_estimation_history | ForecastHandler data source |
| task_activity | All handlers log activities |

### Config fields (all defined in Phase 1's TasksConfig)

| Field | Default | Consumed by |
|-------|---------|-------------|
| decomposition_auto_apply_threshold | 0.75 | DecompositionHandler |
| suggestion_auto_apply_threshold | 0.83 | ProactiveHandler |
| working_hours | 9-18 | DayPlanningHandler |
| max_plan_tasks | 8 | DayPlanningHandler |
| auto_apply_day_plan | false | DayPlanningHandler |
| wip_limit | 5 | ProactiveHandler |
| stale_task_days | 5 | ProactiveHandler |
| project_overrides | {} | ProactiveHandler |
| area_overrides | {} | ProactiveHandler |
| forecast_min_sample_size | 5 | ForecastHandler |
| forecast_lookback_days | 90 | ForecastHandler |
| proactive_suggestions | true | ProactiveHandler (returns empty vec when disabled) |
| cognitive_integration | true | Cognitive bridge |

### Implementation file locations

| File | Layer | Component |
|------|-------|-----------|
| `crates/feature-tasks/src/handlers/decomposition.rs` | L4 | Trait definition |
| `crates/feature-tasks/src/handlers/execution.rs` | L4 | Trait definition |
| `crates/feature-tasks/src/handlers/planning.rs` | L4 | Trait definition |
| `crates/feature-tasks/src/handlers/proactive.rs` | L4 | Trait definition |
| `crates/feature-tasks/src/handlers/suggestion_applier.rs` | L4 | Trait definition |
| `crates/feature-tasks/src/handlers/forecast.rs` | L4 | Trait definition |
| `crates/feature-tasks/src/forecast.rs` | L4 | Pure computation |
| `crates/feature-tasks/src/cognitive_bridge.rs` | L4 | Fact extraction helpers |
| `crates/agent/src/handlers/decomposition.rs` | L5 | LLM implementation |
| `crates/agent/src/handlers/execution.rs` | L5 | SpawnHandler integration |
| `crates/agent/src/handlers/planning.rs` | L5 | LLM implementation |
| `crates/agent/src/handlers/proactive.rs` | L5 | LLM implementation |
| `crates/agent/src/handlers/suggestion_applier.rs` | L5 | Cross-handler delegation |
| `crates/agent/src/handlers/forecast.rs` | L5 | LLM-enhanced risk analysis |
| `crates/agent/src/templates/decomposition_prompt.md` | L5 | Prompt template |
| `crates/agent/src/templates/execution_prompt.md` | L5 | Prompt template |
| `crates/agent/src/templates/day_plan_prompt.md` | L5 | Prompt template |
| `crates/agent/src/templates/proactive_suggestions.md` | L5 | Prompt template |
| `crates/cognitive/src/background.rs` | L5 | Enhanced observation mapping |
| `crates/cognitive/src/salience.rs` | L5 | Enhanced salience classification |
