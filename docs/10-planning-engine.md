# Planning Engine

The planning engine decomposes complex, multi-step user requests into structured plans that are generated, persisted, and executed step-by-step with automatic retry and backtracking support.

---

## Section 1: Narrative Overview

### Plan Data Model

A **Plan** is a structured execution graph consisting of an ordered list of **PlanStep** entries. Each plan is scoped to a session (`session_key`) for isolation, optionally linked to a goal (`goal_id`), and follows a strict status lifecycle enforced by `PlanStatus::validate_transition()`.

The core types live in the `plan` crate (`crates/plan/src/types.rs`):

- **Plan** -- the top-level container with title, description, steps, and execution metadata.
- **PlanStep** -- a single actionable unit within a plan, carrying a description, reasoning, expected tools, attempt tracking, and result.
- **PlanStatus** -- the plan lifecycle state machine (see below).
- **StepStatus** -- per-step execution state: `Pending`, `Executing`, `Completed`, `Failed`, `Skipped`.
- **BacktrackEntry** -- a record of a step failure that triggered replanning, stored in `plan.backtrack_history`.
- **PlanVisibility** -- controls whether a plan is shown to the user or auto-cleaned.

Plans are serialized to JSON for persistence. The `plan::conversions` module (`crates/plan/src/conversions.rs`) handles domain-to-row and row-to-domain transformations between `Plan`/`PlanStep` and `PlanRow`/`PlanStepRow`.

### Plan Status State Machine

Plans follow this state machine, enforced by `PlanStatus::validate_transition()` at `crates/plan/src/types.rs:91`:

```
Draft --> Approved --> Executing --> Completed
  |          |            |
  v          v            v
Abandoned  Abandoned    Failed
                          |
                          v
                       Abandoned
```

**Valid transitions:**

| From | To |
|------|----|
| Draft | Approved, Abandoned |
| Approved | Executing, Abandoned |
| Executing | Completed, Failed, Abandoned |
| Completed | (terminal -- no transitions) |
| Failed | (terminal -- no transitions) |
| Abandoned | (terminal -- no transitions) |

No-op transitions (same state to same state) are always allowed. Any transition from a terminal state to a different state returns `PlanError::InvalidState`.

### Plan Visibility

Defined at `crates/plan/src/types.rs:16`, `PlanVisibility` controls auto-cleanup behavior:

- **Transparent** (default) -- always visible in the dashboard and API. No auto-cleanup.
- **OnFailure** -- hidden from the user unless the plan fails. Successfully completed `on_failure` plans are auto-deleted after 7 days (168 hours).
- **Silent** -- never shown to the user. Auto-deleted 24 hours after reaching any terminal state (completed, failed, or abandoned).

The `PlanCleanupService` (`crates/agent/src/intent_pipeline/visibility.rs`) runs hourly as a background Tokio task. It calls `PlanRepo::delete_stale_plans()` with the configured age thresholds:

- `SILENT_AGE_HOURS = 24` -- silent plans older than 24h in terminal state.
- `ON_FAILURE_AGE_HOURS = 168` -- on_failure plans that completed successfully older than 7 days.

### PlanTool -- User Interface

The `PlanTool` (`crates/tools/src/plan_tool.rs:138`) implements the `Tool` trait and exposes plan management to the user through natural language in chat. It delegates all persistence to an injected `Arc<dyn PlanHandler>`.

Supported actions:

| Action | Parameters | Behavior |
|--------|-----------|----------|
| `create` | `title`, `description`, `session_key?`, `goal_id?` | Previews LLM-generated steps, asks for user approval via interaction channel, then creates and saves the plan with auto-generated steps. |
| `show` | `plan_id` | Returns plan details: title, status, description, step count, progress, timestamps. |
| `approve` | `plan_id` | Transitions a Draft plan to Approved. Only Draft plans can be approved. |
| `abandon` | `plan_id` | Transitions any non-terminal plan to Abandoned. |
| `status` | `session_key?` | Returns the most recent active plan (Draft/Approved/Executing) for the session. |
| `execute` | `plan_id` | Transitions an Approved plan to Executing and returns the plan for the agent loop to drive. |

The `create` action uses a two-phase approval flow:
1. Calls `handler.preview_steps()` to generate step descriptions without persisting.
2. Presents the preview to the user via `ask_plan_approval()`, which uses the interaction channel (TTY) or falls back to conversational approval (non-TTY).
3. On approval, calls `handler.create_plan()` followed by `handler.generate_steps()` to persist the plan and its LLM-generated steps.

### PlanHandler -- Plan CRUD

The `PlanHandler` trait is defined in `tools` (Layer 3) at `crates/tools/src/plan_tool.rs:37` for dependency inversion. It is implemented by `PlanHandlerImpl` in `agent` (Layer 5) at `crates/agent/src/plan_handler.rs:20`.

`PlanHandlerImpl` wraps a `PlanRepo` and an optional `DynProvider` (LLM provider). Key behaviors:

- **create_plan** -- creates a new plan in `Draft` status with empty steps, persists via `conversions::save_plan()`.
- **approve_plan** -- validates the plan is in `Draft` state (strict check, not using `validate_transition`), then transitions to `Approved`.
- **execute_plan** -- validates `Approved -> Executing` via `validate_transition`, transitions and persists.
- **generate_steps** -- calls `generate_plan_steps()` from the step generator, converts drafts to `PlanStep` records, and saves them. Only generates if the plan has no existing steps. Silently returns `Ok(())` on any failure.
- **preview_steps** -- generates step descriptions via LLM without persisting. Returns `Vec<String>` of descriptions for user review.
- **get_step_context** -- builds the context window (current step + next 3) for LLM prompting.

### PlanExecutor -- Step-by-Step Execution

The plan executor (`crates/agent/src/plan_executor.rs`) contains the core execution logic for individual plan steps:

**`run_step()`** (line 49) -- executes a single plan step using multi-cycle LLM-tool execution:
1. Builds a prompt from the plan context and step details (description, reasoning, expected tools).
2. Runs up to `MAX_CYCLES_PER_STEP` (5) cycles through the `ExecutionCore`.
3. Each cycle may produce tool calls (executed and looped back) or a final text response.
4. Tool failures immediately return a failed `StepExecutionResult`.
5. The first tool call's name is captured in `tool_name` for outcome recording.
6. Returns `StepExecutionResult` with success/failure status, accumulated output, and optional confidence assessment.

**`build_step_context()`** (line 190) -- builds a sliding context window for the LLM:
- Includes results from the last 3 completed steps (truncated to 500 characters each).
- Shows the current step marked `>>> CURRENT`.
- Shows the next 3 upcoming steps marked `NEXT 1/2/3`.
- Includes the plan title, goal, and progress indicator.

**`regenerate_from()`** (line 263) -- handles backtracking by asking the LLM to generate replacement steps from a failure point:
1. Summarizes completed steps so the LLM does not regenerate them.
2. Includes the failed step description and failure reason.
3. Parses the LLM response as a JSON array of step drafts.
4. If parsing fails, inserts a single fallback step: `"Retry: <failed_step_description>"`.
5. The caller is responsible for truncating `plan.steps` at the failure index and extending with the new steps.

### PlanStepGenerator -- LLM-Driven Decomposition

The step generator (`crates/agent/src/plan_step_generator.rs`) handles LLM-based task decomposition:

**`generate_plan_steps()`** (line 33) -- prompts the LLM to decompose a task description into 3-8 actionable steps. The prompt instructs the LLM to specify exact tool actions and parameters (not vague descriptions). Returns `Vec<PlanStepDraft>`, capped at 8 entries. Uses temperature 0.0 and max_tokens 1024.

**`parse_step_drafts()`** (line 115) -- parses raw LLM output (potentially wrapped in markdown fences) into `PlanStepDraft` values. Uses `common::utils::extract_json_array()` to handle markdown-fenced JSON. Silently returns empty on parse failure.

**`drafts_to_plan_steps()`** (line 92) -- converts drafts to full `PlanStep` records with UUIDs, sequential indices starting at `start_index`, `Pending` status, and `DEFAULT_MAX_STEP_ATTEMPTS` (3) for `max_attempts`.

### Backtracking

When a step exceeds its `max_attempts` (default 3 retries per step), the execution loop initiates backtracking:

1. A `BacktrackEntry` is recorded in `plan.backtrack_history` with the step index, attempt number, failure reason, and timestamp.
2. `regenerate_from()` prompts the LLM for replacement steps from the failure point forward.
3. The plan's steps are truncated at the failure index and replaced with the regenerated steps.
4. If the LLM returns invalid JSON, a single fallback "Retry" step is inserted.
5. After `MAX_BACKTRACK_ATTEMPTS` (3) full backtrack events across the entire plan, the plan is marked `Failed`.

Per-step retries (`attempt_count`) are separate from the backtrack limit. A step can be retried up to `max_attempts` times before triggering a backtrack event.

### PlanCompletionHandler -- Post-Execution Callbacks

The `PlanCompletionHandler` trait (`crates/tools/src/plan_tool.rs:17`) is called by the agent loop after a plan finishes. It follows the dependency inversion pattern: trait in `tools` (Layer 3), implementation in `agent` (Layer 5).

`PlanCompletionHandlerImpl` (`crates/agent/src/plan_completion_handler.rs:18`) updates linked goal metrics when a plan completes:
- On success: increments `plans_completed` counter and updates `last_plan_at` via `GoalRepo::increment_completed()`.
- On failure: increments `plans_failed` counter via `GoalRepo::increment_failed()`.
- If no `goal_id` is linked, does nothing.

### Integration with the Intent Pipeline (PlannedEngine)

The `PlannedEngine` (`crates/agent/src/intent_pipeline/engines/planned.rs:28`) is the intent pipeline's execution engine for complex, multi-tool tasks. It implements the `ExecutionEngine` trait and wraps the plan executor.

**Standard execution flow** (`execute()` / `execute_fresh()`):
1. Extracts the user's message from conversation history.
2. Calls `generate_plan_steps()` to decompose the task via LLM.
3. If no steps are generated, falls back to `ReactiveEngine`.
4. Builds a `Plan` in `Approved` state, persists it, transitions to `Executing`.
5. Calls `run_plan_steps()` to execute each step sequentially with retry and backtracking.
6. Calls `synthesize_response()` to generate a human-readable summary of the raw step outputs via LLM.
7. Returns `EngineResult::Complete` with the synthesized content.

**Escalation takeover** (`execute_with_prior_work()`):
When the router escalates from Reactive to Planned, prior work (completed tool calls) is carried forward as pre-filled completed steps. The LLM generates only the remaining steps, and execution resumes from where the reactive engine left off.

**Visibility handling:**
- `execute_with_visibility()` allows the intent classifier to override the default visibility for user-requested plans (e.g., `Transparent` for explicit "make a plan" requests vs. `OnFailure` for auto-generated plans).

### Storage: PlanRepo

The `PlanRepo` (`crates/storage/src/repos/plan.rs`) provides persistence for plans and plan steps in SQLite.

**Visibility filtering in `list()`:**
- `visibility: None` (default) -- excludes `silent` plans.
- `visibility: Some("all")` -- returns all plans regardless of visibility.
- `visibility: Some(value)` -- filters to only that visibility level.

**Stale plan cleanup:**
`delete_stale_plans()` removes plans based on visibility and terminal status age, used by the `PlanCleanupService`.

### Goal System

The goal system provides strategic goal management that sits above the planning layer. Goals represent high-level objectives that span multiple projects and are tracked through linked plans.

#### Goal Data Model

A **Goal** (`crates/goal/src/types.rs`) is a strategic objective with plan-completion metrics:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique goal identifier |
| `title` | `String` | Goal title |
| `description` | `String` | Full goal description |
| `status` | `GoalStatus` | Current lifecycle state |
| `priority` | `u8` | Priority 1-5 (matches todo priority scale) |
| `target_date` | `Option<DateTime<Utc>>` | Optional deadline |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last modification timestamp |
| `plans_completed` | `i32` | Number of plans that completed successfully |
| `plans_failed` | `i32` | Number of plans that failed |
| `avg_duration_ms` | `Option<i64>` | Rolling average duration of completed plans in milliseconds |
| `last_plan_at` | `Option<DateTime<Utc>>` | Timestamp of the most recent plan completion/failure |
| `linked_project_ids` | `Vec<Uuid>` | Projects contributing to this goal |

#### GoalStatus State Machine

`GoalStatus` (`crates/goal/src/types.rs:34`) follows this state machine, enforced by `GoalStatus::validate_transition()`:

```
Active --> Paused
  |   \       |
  |    v      v
  |  Achieved   Abandoned
  |
  +---> Abandoned
```

**Valid transitions:**

| From | To |
|------|----|
| Active | Paused, Achieved, Abandoned |
| Paused | Active, Abandoned |
| Achieved | (terminal -- no transitions) |
| Abandoned | (terminal -- no transitions) |

No-op transitions (same state to same state) are always allowed. Note that `Paused` cannot transition directly to `Achieved` -- it must go through `Active` first.

#### GoalProgress

`GoalProgress` (`crates/goal/src/types.rs:125`) is an aggregated progress snapshot computed from plan-completion statistics:

| Field | Type | Description |
|-------|------|-------------|
| `goal_id` | `Uuid` | The goal being measured |
| `completion_percentage` | `f64` | Overall completion (0.0-100.0), calculated as `plans_completed / (plans_completed + plans_failed) * 100` |
| `summary` | `String` | Human-readable summary, e.g. "45% completion rate (9 of 20 plans succeeded)" |

#### GoalTool -- User Interface

The `GoalTool` (`crates/tools/src/goal_tool.rs`) implements the `Tool` trait and exposes goal management through natural language in chat. It delegates all operations to an injected `Arc<dyn GoalHandler>`.

Supported actions:

| Action | Parameters | Behavior |
|--------|-----------|----------|
| `create` | `title`, `description?`, `priority?` | Creates a new Active goal. Priority defaults to 3. |
| `list` | `status?` | Lists all goals, optionally filtered by status. |
| `show` | `goal_id` | Returns goal details: title, status, priority, description, plan stats, timestamps. |
| `update` | `goal_id`, `title?`, `description?`, `priority?`, `status?` | Updates goal fields. Validates status transitions. |
| `delete` | `goal_id` | Deletes a goal by ID. |
| `progress` | `goal_id` | Returns completion percentage and summary from plan statistics. |
| `decompose` | `goal_id` | Decomposes the goal into a Draft plan via LLM. Creates and persists the plan linked to this goal. |
| `status` | `goal_id` | Shows progress of all plans linked to this goal -- plan statuses and step completion. |
| `metrics` | `goal_id` | Returns plan-completion metrics: completion rate, average duration, last activity, active plan count. |

#### GoalHandler Trait and Implementation

The `GoalHandler` trait (`crates/tools/src/goal_tool.rs:18`) follows the dependency inversion pattern: defined in `tools` (Layer 3), implemented by `GoalHandlerImpl` in `agent` (Layer 5) at `crates/agent/src/goal_handler.rs`.

`GoalHandlerImpl` wraps a `GoalRepo`, an optional `PlanRepo`, and an optional `DynProvider` (LLM provider). Key behaviors:

- **create_goal** -- persists a new goal and links any associated project IDs via `GoalRepo`.
- **update_goal** -- validates the status transition against the existing goal state before persisting.
- **delete_goal** -- deletes the goal; returns an error if not found.
- **calculate_progress** -- loads the goal and computes `GoalProgress` from plan-completion counters using `conversions::compute_progress()`.
- **decompose_goal** -- calls `generate_plan_steps()` to decompose the goal via LLM, creates a `Draft` plan with `goal_id` set and `session_key` formatted as `"goal:{goal_id}"`, and persists it. Returns a graceful message (not an error) when provider or plan repo is not configured.
- **goal_progress** -- lists all plans linked to a goal via `PlanRepo::list()`, then for each plan fetches step completion statistics (completed vs total steps).
- **goal_metrics** -- returns completion rate, average duration (formatted in hours/minutes/ms), last activity timestamp, and count of active (Executing/Approved) plans.

#### How Goals Link to Plans

Goals and plans are connected through the `Plan.goal_id` field:

1. **Goal decomposition** (`decompose` action) generates a Draft plan with `goal_id` set, linking it to the source goal.
2. **Plan completion** triggers `PlanCompletionHandler`, which updates goal metrics (`plans_completed` or `plans_failed` counters, `last_plan_at` timestamp) via `GoalRepo::increment_completed()` or `GoalRepo::increment_failed()`.
3. **Goal progress** queries all plans matching `goal_id` to produce a cross-plan status view.
4. **Goal metrics** aggregates plan outcomes into completion rates and duration averages.

#### GoalError Variants

`GoalError` (`crates/goal/src/error.rs:7`) converts to `common::KlyntbotError::Goal` via `From`.

| Variant | Description |
|---------|-------------|
| `NotFound(String)` | Goal not found by ID |
| `InvalidState(String)` | Invalid state transition attempted |
| `StoreFailed(String)` | Storage operation failed |
| `ValidationFailed(String)` | Validation failed (e.g., priority out of range, unknown status string) |

---

## Section 2: API Reference

### Plan

Defined at `crates/plan/src/types.rs:49`.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique plan identifier |
| `session_key` | `String` | Session scope for isolation (format: `"{channel}:{chat_id}"`) |
| `goal_id` | `Option<Uuid>` | Optional linked goal for metrics tracking |
| `title` | `String` | Human-readable plan title |
| `description` | `String` | Full task description |
| `status` | `PlanStatus` | Current lifecycle state |
| `steps` | `Vec<PlanStep>` | Ordered list of execution steps |
| `current_step_index` | `usize` | Index of the next step to execute |
| `iteration_limit` | `usize` | Maximum total iterations allowed (default: 50) |
| `backtrack_history` | `Vec<BacktrackEntry>` | Record of all backtracking events |
| `visibility` | `PlanVisibility` | Controls UI visibility and auto-cleanup (default: `Transparent`) |
| `task_id` | `Option<String>` | Optional linked task ID |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last modification timestamp |
| `completed_at` | `Option<DateTime<Utc>>` | Timestamp when plan reached terminal state |

### PlanStep

Defined at `crates/plan/src/types.rs:148`.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique step identifier |
| `index` | `usize` | Position in the plan (0-indexed) |
| `description` | `String` | What this step does (should specify exact tool actions) |
| `reasoning` | `String` | Why this step is needed |
| `expected_tools` | `Vec<String>` | Suggested tool names for the LLM |
| `status` | `StepStatus` | Current step state |
| `attempt_count` | `u8` | Number of execution attempts so far |
| `max_attempts` | `u8` | Maximum retries before backtracking (default: `DEFAULT_MAX_STEP_ATTEMPTS` = 3) |
| `result` | `Option<String>` | Captured output from execution |
| `started_at` | `Option<DateTime<Utc>>` | When execution began |
| `completed_at` | `Option<DateTime<Utc>>` | When execution finished |

### PlanStatus

Defined at `crates/plan/src/types.rs:71`. Implements `FromStr`, `Display`, `Default` (defaults to `Draft`).

| Variant | Description |
|---------|-------------|
| `Draft` | Initial state. Steps may be empty or being generated. |
| `Approved` | User has approved the plan. Ready for execution. |
| `Executing` | Plan is actively being executed step-by-step. |
| `Completed` | All steps finished successfully. Terminal state. |
| `Failed` | Execution failed after exhausting backtrack attempts. Terminal state. |
| `Abandoned` | User or system abandoned the plan. Terminal state. |

Valid transitions enforced by `validate_transition()` at line 91:

```
Draft      -> Approved | Abandoned
Approved   -> Executing | Abandoned
Executing  -> Completed | Failed | Abandoned
Completed  -> (none)
Failed     -> (none)
Abandoned  -> (none)
```

### PlanVisibility

Defined at `crates/plan/src/types.rs:16`. Implements `FromStr`, `Display`, `Default` (defaults to `Transparent`).

| Variant | Description | Auto-cleanup |
|---------|-------------|-------------|
| `Silent` | Never shown to user | 24h after terminal state |
| `OnFailure` | Hidden unless plan fails | 7 days after successful completion |
| `Transparent` | Always visible | None |

### StepStatus

Defined at `crates/plan/src/types.rs:164`. Implements `FromStr`, `Display`, `Default` (defaults to `Pending`).

| Variant | Description |
|---------|-------------|
| `Pending` | Not yet started |
| `Executing` | Currently running |
| `Completed` | Finished successfully |
| `Failed` | Execution failed |
| `Skipped` | Step was skipped (e.g., during backtracking) |

### BacktrackEntry

Defined at `crates/plan/src/types.rs:201`.

| Field | Type | Description |
|-------|------|-------------|
| `step_index` | `usize` | Index of the step that failed |
| `attempt` | `u8` | Which attempt number triggered the backtrack |
| `failure_reason` | `String` | Description of why the step failed |
| `timestamp` | `DateTime<Utc>` | When the backtrack occurred |

### PlanError

Defined at `crates/plan/src/error.rs:7`. Converts to `common::KlyntbotError::Plan` via `From`.

| Variant | Description |
|---------|-------------|
| `NotFound(String)` | Plan not found by ID |
| `GenerationFailed(String)` | Step generation via LLM failed |
| `InvalidState(String)` | Invalid state transition attempted |
| `ExecutionStalled { step_index, reason }` | Execution stuck at a step |
| `BacktrackLimitReached(usize)` | Max backtrack attempts exceeded |
| `StoreFailed(String)` | Storage operation failed |

### PlanTool

Defined at `crates/tools/src/plan_tool.rs:138`. Tool name: `"plan"`.

| Action | Required Params | Optional Params | Returns |
|--------|----------------|-----------------|---------|
| `create` | `title` | `description`, `session_key`, `goal_id` | Plan ID and status, or preview for conversational approval |
| `show` | `plan_id` | | Plan details (title, ID, status, description, steps, progress, timestamps) |
| `approve` | `plan_id` | | Confirmation with plan title and new status |
| `abandon` | `plan_id` | | Confirmation with plan ID |
| `status` | | `session_key` | Active plan summary or "No active plan" |
| `execute` | `plan_id` | | Confirmation with plan title and Executing status |

### PlanHandler Trait

Defined at `crates/tools/src/plan_tool.rs:37`. Implemented by `PlanHandlerImpl` at `crates/agent/src/plan_handler.rs:20`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `create_plan` | `(title, description, session_key, goal_id?) -> Result<Plan>` | Create a new Draft plan |
| `get_plan` | `(id) -> Result<Option<Plan>>` | Load a plan by ID with steps |
| `get_active_plan` | `(session_key) -> Result<Option<Plan>>` | Get most recent non-terminal plan for session |
| `approve_plan` | `(id) -> Result<Plan>` | Transition Draft to Approved |
| `abandon_plan` | `(id) -> Result<()>` | Transition any non-terminal plan to Abandoned |
| `get_step_context` | `(id) -> Result<String>` | Build context window for current step |
| `execute_plan` | `(id) -> Result<Plan>` | Transition Approved to Executing |
| `generate_steps` | `(plan_id) -> Result<()>` | Auto-generate steps via LLM and save |
| `preview_steps` | `(description) -> Result<Vec<String>>` | Generate step descriptions without persisting |

### PlanExecutor

Defined at `crates/agent/src/plan_executor.rs`.

**Constants:**

| Name | Value | Description |
|------|-------|-------------|
| `MAX_BACKTRACK_ATTEMPTS` | `3` | Maximum full backtrack events before plan fails (line 22) |
| `MAX_CYCLES_PER_STEP` | `5` | Maximum LLM cycles per step to prevent infinite loops (line 25) |

**Functions:**

| Function | Signature | Description |
|----------|-----------|-------------|
| `run_step` | `(core, step, plan_context, routing_ctx, confidence_evaluator?) -> Result<StepExecutionResult>` | Execute a single step with multi-cycle LLM-tool loops (line 49) |
| `build_step_context` | `(plan, current_index) -> String` | Build sliding context window: last 3 completed + current + next 3 (line 190) |
| `regenerate_from` | `(plan, failure_index, failure_reason, provider) -> Result<Vec<PlanStep>>` | LLM-driven step regeneration from failure point (line 263) |

### StepExecutionResult

Defined at `crates/agent/src/plan_executor.rs:30`.

| Field | Type | Description |
|-------|------|-------------|
| `success` | `bool` | Whether the step completed successfully |
| `output` | `String` | Captured output from tool execution or LLM response |
| `failure_reason` | `Option<String>` | Reason for failure if `success` is false |
| `confidence` | `Option<ConfidenceAssessment>` | Confidence assessment from the LLM, if available |
| `tool_name` | `Option<String>` | Name of the first tool executed (None for text-only responses) |

### PlanStepGenerator

Defined at `crates/agent/src/plan_step_generator.rs`.

| Item | Signature | Description |
|------|-----------|-------------|
| `PlanStepDraft` | struct: `{ description, reasoning, expected_tools }` | Lightweight draft before DB persistence (line 18) |
| `generate_plan_steps` | `(provider, model, description, context, available_tools) -> Result<Vec<PlanStepDraft>>` | LLM decomposition into 3-8 steps (line 33) |
| `parse_step_drafts` | `(content) -> Vec<PlanStepDraft>` | Parse JSON array from raw LLM output, capped at 8 (line 115) |
| `drafts_to_plan_steps` | `(drafts, start_index) -> Vec<PlanStep>` | Convert drafts to full PlanStep records with UUIDs (line 92) |

### PlanCompletionHandler Trait

Defined at `crates/tools/src/plan_tool.rs:17`. Implemented by `PlanCompletionHandlerImpl` at `crates/agent/src/plan_completion_handler.rs:18`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `on_plan_completed` | `(plan_id, goal_id?, success, summary) -> Result<()>` | Called after plan execution finishes. Updates goal metrics if linked. |

### PlanCleanupService

Defined at `crates/agent/src/intent_pipeline/visibility.rs:20`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(plan_repo, cancel_token) -> Self` | Create a new cleanup service |
| `spawn` | `(self)` | Spawn as a background Tokio task |

Runs every `CLEANUP_INTERVAL_SECS` (3600 = 1 hour). Calls `PlanRepo::delete_stale_plans(24, 168)`.

### PlanRepo

Defined at `crates/storage/src/repos/plan.rs:12`.

**Plan operations:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(pool: SqlitePool) -> Self` | Create repo from connection pool (line 17) |
| `create` | `(row) -> Result<PlanRow>` | Insert a new plan (line 22) |
| `upsert` | `(row) -> Result<PlanRow>` | Insert or update on ID conflict (line 51) |
| `get` | `(id: Uuid) -> Result<PlanRow>` | Get plan by ID; errors if not found (line 89) |
| `list` | `(status?, session_key?, goal_id?, visibility?) -> Result<Vec<PlanRow>>` | List plans with optional filters; excludes silent by default (line 103) |
| `update` | `(row) -> Result<PlanRow>` | Update mutable fields; auto-sets `updated_at` (line 143) |
| `delete` | `(id: Uuid) -> Result<bool>` | Delete plan by ID; cascades to steps (line 171) |
| `update_status` | `(id: Uuid, status: &str) -> Result<()>` | Update status with automatic `completed_at` for terminal states (line 180) |
| `delete_stale_plans` | `(silent_age_hours, on_failure_age_hours) -> Result<u64>` | Delete stale plans based on visibility rules (line 206) |

**Step operations:**

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_step` | `(step: &PlanStepRow) -> Result<PlanStepRow>` | Insert a new step (line 233) |
| `update_step` | `(step: &PlanStepRow) -> Result<PlanStepRow>` | Update step status, attempt_count, result, timestamps (line 259) |
| `upsert_step` | `(step: &PlanStepRow) -> Result<PlanStepRow>` | Insert or update step on ID conflict (line 279) |
| `get_active` | `(session_key: &str) -> Result<Option<PlanRow>>` | Get most recent active plan (draft/approved/executing) for session (line 311) |
| `get_steps` | `(plan_id: Uuid) -> Result<Vec<PlanStepRow>>` | Get all steps for a plan, ordered by step_index (line 328) |

### PlannedEngine

Defined at `crates/agent/src/intent_pipeline/engines/planned.rs:28`. Implements `ExecutionEngine`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(core, plan_repo, provider, model, default_visibility) -> Self` | Constructor (line 37) |
| `execute` | `(messages, tools, params, ctx, event_tx?) -> Result<EngineResult>` | Standard execution: generate plan, persist, execute steps (line 438) |
| `execute_with_prior_work` | `(escalation, tools, params, ctx, event_tx?) -> Result<EngineResult>` | Escalation takeover with pre-filled completed steps (line 57) |
| `execute_with_visibility` | `(messages, tools, params, ctx, event_tx?, visibility) -> Result<EngineResult>` | Execute with explicit visibility override (line 408) |
| `mode` | `() -> &str` | Returns `"planned"` (line 450) |

### Goal

Defined at `crates/goal/src/types.rs:11`.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique goal identifier |
| `title` | `String` | Goal title |
| `description` | `String` | Full goal description |
| `status` | `GoalStatus` | Current lifecycle state |
| `priority` | `u8` | Priority 1-5 (matches todo priority scale) |
| `target_date` | `Option<DateTime<Utc>>` | Optional deadline |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last modification timestamp |
| `plans_completed` | `i32` | Successful plan count |
| `plans_failed` | `i32` | Failed plan count |
| `avg_duration_ms` | `Option<i64>` | Rolling average plan duration in ms |
| `last_plan_at` | `Option<DateTime<Utc>>` | Most recent plan completion/failure |
| `linked_project_ids` | `Vec<Uuid>` | Projects contributing to this goal |

### GoalStatus

Defined at `crates/goal/src/types.rs:34`. Implements `FromStr`, `Display`, `Default` (defaults to `Active`).

| Variant | Description |
|---------|-------------|
| `Active` | Currently pursuing. Default state. |
| `Paused` | Temporarily suspended. |
| `Achieved` | Completed successfully. Terminal state. |
| `Abandoned` | No longer pursuing. Terminal state. |

Valid transitions enforced by `validate_transition()`:

```
Active     -> Paused | Achieved | Abandoned
Paused     -> Active | Abandoned
Achieved   -> (none)
Abandoned  -> (none)
```

### GoalProgress

Defined at `crates/goal/src/types.rs:125`.

| Field | Type | Description |
|-------|------|-------------|
| `goal_id` | `Uuid` | The goal being measured |
| `completion_percentage` | `f64` | 0.0-100.0 based on plan success ratio |
| `summary` | `String` | Human-readable progress summary |

### GoalError

Defined at `crates/goal/src/error.rs:7`. Converts to `common::KlyntbotError::Goal` via `From`.

| Variant | Description |
|---------|-------------|
| `NotFound(String)` | Goal not found by ID |
| `InvalidState(String)` | Invalid state transition attempted |
| `StoreFailed(String)` | Storage operation failed |
| `ValidationFailed(String)` | Validation failed (priority out of range, unknown status) |

### GoalTool

Defined at `crates/tools/src/goal_tool.rs:37`. Tool name: `"goal"`.

| Action | Required Params | Optional Params | Returns |
|--------|----------------|-----------------|---------|
| `create` | `title` | `description`, `priority` | Goal ID and title |
| `list` | | `status` | List of goals with short IDs, priorities, statuses |
| `show` | `goal_id` | | Goal details (title, ID, status, priority, description, plan stats, timestamps) |
| `update` | `goal_id` | `title`, `description`, `priority`, `status` | Confirmation with goal ID |
| `delete` | `goal_id` | | Confirmation with goal ID |
| `progress` | `goal_id` | | Completion percentage and summary |
| `decompose` | `goal_id` | | Plan creation summary or graceful fallback message |
| `status` | `goal_id` | | Linked plan statuses with step completion |
| `metrics` | `goal_id` | | Completion rate, avg duration, last activity, active plan count |

### GoalHandler Trait

Defined at `crates/tools/src/goal_tool.rs:18`. Implemented by `GoalHandlerImpl` at `crates/agent/src/goal_handler.rs:21`.

| Method | Signature | Description |
|--------|-----------|-------------|
| `create_goal` | `(goal: Goal) -> Result<Uuid>` | Persist a new goal |
| `get_goal` | `(id: &Uuid) -> Result<Option<Goal>>` | Load a goal by ID with project links |
| `list_goals` | `(status: Option<GoalStatus>) -> Result<Vec<Goal>>` | List goals, optionally filtered by status |
| `update_goal` | `(goal: Goal) -> Result<()>` | Update goal fields with status transition validation |
| `delete_goal` | `(id: &Uuid) -> Result<()>` | Delete a goal by ID |
| `calculate_progress` | `(id: &Uuid) -> Result<GoalProgress>` | Compute progress from plan-completion counters |
| `decompose_goal` | `(goal_id: &Uuid) -> Result<String>` | Decompose goal into a Draft plan via LLM |
| `goal_progress` | `(goal_id: &Uuid) -> Result<String>` | Show linked plan statuses and step completion |
| `goal_metrics` | `(goal_id: &Uuid) -> Result<String>` | Return completion rate, avg duration, last activity, active plan count |

### Goal Conversion Functions

Defined at `crates/goal/src/conversions.rs`.

| Function | Signature | Description |
|----------|-----------|-------------|
| `goal_to_row` | `(goal: &Goal) -> GoalRow` | Domain to SQL row |
| `row_to_goal` | `(row: GoalRow, linked_project_ids: Vec<Uuid>) -> Goal` | SQL row + project links to domain |
| `compute_progress` | `(goal: &Goal) -> GoalProgress` | Compute progress from plan stats |
| `load_goal` | `(repo: &GoalRepo, id: &Uuid) -> Result<Option<Goal>>` | Load goal with project links from repo |
| `save_goal` | `(repo: &GoalRepo, goal: &Goal) -> Result<()>` | Upsert goal and sync project links |

### Conversion Functions

Defined at `crates/plan/src/conversions.rs`.

| Function | Signature | Description |
|----------|-----------|-------------|
| `plan_to_row` | `(plan: &Plan) -> Result<PlanRow>` | Domain to SQL row (line 9) |
| `step_to_row` | `(step: &PlanStep, plan_id: Uuid) -> PlanStepRow` | Step domain to SQL row (line 30) |
| `row_to_plan` | `(row: PlanRow, step_rows: Vec<PlanStepRow>) -> Plan` | SQL rows to domain (line 48) |
| `load_plan` | `(repo, id) -> Result<Option<Plan>>` | Load plan + steps from repo (line 87) |
| `save_plan` | `(repo, plan) -> Result<()>` | Upsert plan + all steps to repo (line 99) |
| `get_active_plan` | `(repo, session_key) -> Result<Option<Plan>>` | Get most recent active plan for session (line 113) |
