# domain

## Purpose

The `domain` crate defines Klyntbot's core strategic types -- goals and plans -- that sit above individual task management. It lives at **Layer 2** of the workspace architecture and provides the data structures, state machines, error types, and storage conversion helpers for the planning engine. This crate has no dependency on the agent or tools layers, making it available to any higher-level crate.

## Key Types

### Goal types

**`Goal`** -- A strategic objective that spans multiple projects and plans. Fields:
- `id: Uuid`, `title`, `description`
- `status: GoalStatus`, `priority: u8` (1-5, matching the todo priority scale)
- `target_date: Option<DateTime<Utc>>`
- Plan statistics: `plans_completed`, `plans_failed`, `avg_duration_ms`, `last_plan_at`
- `linked_project_ids: Vec<Uuid>` -- projects contributing to this goal
- `created_at`, `updated_at`

**`GoalStatus`** -- Four-state enum: `Active` (default), `Paused`, `Achieved`, `Abandoned`.

**`GoalProgress`** -- An aggregated snapshot with `goal_id`, `completion_percentage` (0.0-100.0), and a human-readable `summary`. Computed from plan completion statistics via `compute_progress`.

**`GoalError`** -- Goal-specific error variants: `NotFound`, `InvalidState`, `StoreFailed`, `ValidationFailed`. Converts into `KlyntbotError::Goal` via a `From` impl.

### Plan types

**`Plan`** -- A structured multi-step execution plan. Fields:
- `id: Uuid`, `session_key` (ties plan to a chat session)
- `goal_id: Option<Uuid>` (optional link to a parent goal)
- `title`, `description`
- `status: PlanStatus`
- `steps: Vec<PlanStep>`, `current_step_index: usize`
- `iteration_limit: usize` -- maximum iterations before the plan is failed
- `backtrack_history: Vec<BacktrackEntry>` -- records of retry/backtrack events
- `visibility: PlanVisibility`
- `task_id: Option<String>` -- optional link to a todo task
- `created_at`, `updated_at`, `completed_at`

**`PlanStep`** -- A single step within a plan:
- `id: Uuid`, `index: usize`
- `description`, `reasoning` (why this step is needed)
- `expected_tools: Vec<String>` -- which tools the step anticipates using
- `status: StepStatus`
- `attempt_count: u8`, `max_attempts: u8` (default 3 via `DEFAULT_MAX_STEP_ATTEMPTS`)
- `result: Option<String>` -- output from step execution
- `started_at`, `completed_at`

**`PlanVisibility`** -- Controls whether auto-generated plans appear in the UI:
- `Transparent` (default): always visible in dashboard and API responses
- `OnFailure`: hidden until a step fails, then surfaced for review. Auto-cleaned after 7 days.
- `Silent`: never shown to the user. Auto-cleaned 24 hours after reaching terminal state.

**`BacktrackEntry`** -- Records a retry event: `step_index`, `attempt` number, `failure_reason`, `timestamp`.

**`PlanError`** -- Plan-specific error variants: `NotFound`, `GenerationFailed`, `InvalidState`, `ExecutionStalled` (with step index and reason), `BacktrackLimitReached`, `StoreFailed`. Converts into `KlyntbotError::Plan` via a `From` impl.

### Status enums

**`StepStatus`** -- Five-state enum: `Pending` (default), `Executing`, `Completed`, `Failed`, `Skipped`.

## How It Works

### GoalStatus state machine

```
Active --> Paused
Active --> Achieved
Active --> Abandoned
Paused --> Active
Paused --> Abandoned
Achieved --> (terminal)
Abandoned --> (terminal)
```

`GoalStatus::validate_transition(from, to)` enforces these rules. No-op transitions (same state) are always allowed. Achieved and Abandoned are final states -- no further transitions are permitted. Paused goals cannot transition directly to Achieved; they must return to Active first.

### PlanStatus state machine

```
Draft --> Approved --> Executing --> Completed
  \          \            \
   v          v            v
 Abandoned  Abandoned   Abandoned / Failed
```

`PlanStatus::validate_transition(from, to)` enforces these rules:
- **Draft** can move to Approved or Abandoned
- **Approved** can move to Executing or Abandoned
- **Executing** can move to Completed, Failed, or Abandoned
- **Completed**, **Failed**, and **Abandoned** are terminal -- no transitions out

No-op transitions (same state) are always allowed. Skipping states (e.g., Draft directly to Executing) is rejected. This is the same state machine described in the CLAUDE.md planning engine section.

### Storage conversions

The crate provides free functions (not trait impls) for converting between domain and storage types:

**Goals:**
- `goal_to_row(goal) -> GoalRow` -- serializes status as string, priority as i16
- `row_to_goal(row, linked_project_ids) -> Goal` -- parses status from string, linked projects are loaded separately from the join table
- `load_goal(repo, id) -> Option<Goal>` -- loads the goal row and its project links, assembling the full domain type
- `save_goal(repo, goal)` -- upserts the goal row, then syncs project links (clears existing, re-inserts current)
- `compute_progress(goal) -> GoalProgress` -- calculates completion rate from `plans_completed / (plans_completed + plans_failed)`

**Plans:**
- `plan_to_row(plan) -> PlanRow` -- serializes status, visibility, and backtrack_history (as JSON)
- `step_to_row(step, plan_id) -> PlanStepRow` -- serializes step status, casts attempt_count and max_attempts to i16
- `row_to_plan(row, step_rows) -> Plan` -- parses status, visibility, and backtrack_history from their serialized forms. Steps are sorted by index.
- `load_plan(repo, id) -> Option<Plan>` -- loads the plan row and its step rows, assembling the full domain type
- `save_plan(repo, plan)` -- upserts the plan row, then upserts each step row
- `get_active_plan(repo, session_key) -> Option<Plan>` -- finds the most recent Draft, Approved, or Executing plan for a session

### Priority and validation

Goals validate that priority is in the 1-5 range via `goal.validate_priority()`. Invalid priorities produce a `GoalError::ValidationFailed`.

## Connections

**Depends on:**
- `common` (KlyntbotError, Result alias)
- `storage` (GoalRepo, GoalRow, GoalProjectLinkRow, PlanRepo, PlanRow, PlanStepRow, StorageError)
- `chrono`, `serde`, `serde_json`, `uuid`, `thiserror`

**Depended on by:**
- `agent` (uses Plan and Goal types for plan execution, goal tracking, and the intent pipeline's PlannedEngine)
- `tools` (PlanTool and GoalTool reference these domain types)
- `klyntbot` (re-exports via facade)
- Integration tests
