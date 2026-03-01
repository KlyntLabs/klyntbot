# Planning Engine

## Purpose

The planning engine decomposes complex tasks into multi-step plans, persists them to SQLite, and executes each step sequentially with retry and backtracking support. It operates at two levels: the `PlanHandler` trait provides CRUD operations for managing plans through chat (via PlanTool), while the `PlanExecutor` drives autonomous step-by-step execution with context windowing and failure recovery. Plans can also be created and executed automatically by the `PlannedEngine` in the intent pipeline when the classifier determines a task is too complex for reactive execution.

Relevant source files:
- `crates/domain/src/plan.rs` -- domain types, status machines, persistence conversions
- `crates/agent/src/plan_handler.rs` -- PlanHandler trait implementation (create, approve, execute)
- `crates/agent/src/plan_executor.rs` -- step execution, context windowing, backtracking
- `crates/agent/src/plan_step_generator.rs` -- LLM-driven step decomposition

## Key Types

### Plan

The top-level domain struct in `domain/src/plan.rs`:

- `id` (Uuid) -- unique identifier
- `session_key` (String) -- scopes the plan to a channel:chat_id pair
- `goal_id` (Option\<Uuid\>) -- optional link to a goal for outcome tracking
- `title` / `description` (String) -- human-readable summary and full task description
- `status` (PlanStatus) -- current lifecycle state
- `steps` (Vec\<PlanStep\>) -- ordered list of executable steps
- `current_step_index` (usize) -- index of the next step to execute
- `iteration_limit` (usize) -- maximum total iterations allowed (default 50)
- `backtrack_history` (Vec\<BacktrackEntry\>) -- log of step failures and backtrack events
- `visibility` (PlanVisibility) -- controls UI visibility and auto-cleanup behavior
- `task_id` (Option\<String\>) -- optional link to a todo task
- `created_at` / `updated_at` / `completed_at` -- timestamps

### PlanStatus

The plan lifecycle is a strict state machine enforced by `PlanStatus::validate_transition()`:

```
Draft -> Approved -> Executing -> Completed
                  \               \
                   -> Abandoned    -> Failed
                                   \
                                    -> Abandoned
```

Valid transitions:
- **Draft** can move to Approved or Abandoned.
- **Approved** can move to Executing or Abandoned.
- **Executing** can move to Completed, Failed, or Abandoned.
- **Completed, Failed, Abandoned** are terminal -- no further transitions allowed.

Same-state transitions (no-ops) are permitted. Any invalid transition returns a `PlanError::InvalidState`.

### PlanVisibility

Controls whether auto-generated plans appear in the UI and when they are cleaned up:

- **Silent** -- never shown to the user. Auto-deleted 24 hours after reaching a terminal state. Used for plans the agent creates internally during escalation.
- **OnFailure** -- hidden unless a step fails, then surfaced for user review. Successfully completed plans are auto-deleted after 7 days.
- **Transparent** (default) -- always visible in the dashboard and API responses. Never auto-deleted. Used for plans the user explicitly creates.

### PlanStep

A single step within a plan:

- `id` (Uuid), `index` (usize) -- identity and ordering
- `description` (String) -- what the step does (should specify exact tool actions and parameters)
- `reasoning` (String) -- why this step is needed
- `expected_tools` (Vec\<String\>) -- hint list of tools the step should use
- `status` (StepStatus: Pending, Executing, Completed, Failed, Skipped)
- `attempt_count` / `max_attempts` (u8) -- retry tracking, default max is 3 (`DEFAULT_MAX_STEP_ATTEMPTS`)
- `result` (Option\<String\>) -- captured output after execution
- `started_at` / `completed_at` -- timestamps

### BacktrackEntry

Recorded when a step fails and the engine backtracks:

- `step_index` -- which step failed
- `attempt` -- which attempt number triggered the backtrack
- `failure_reason` -- why the step failed
- `timestamp` -- when it happened

### PlanStepDraft

A lightweight intermediate struct from `plan_step_generator.rs`, used between LLM generation and database persistence:

- `description`, `reasoning`, `expected_tools`

Converted to full `PlanStep` records via `drafts_to_plan_steps()`, which assigns UUIDs, sequential indices, Pending status, and the default max attempts.

### PlanError

Plan-specific error variants: `NotFound`, `GenerationFailed`, `InvalidState`, `ExecutionStalled`, `BacktrackLimitReached`, `StoreFailed`. Converts into `KlyntbotError::Plan` via a `From` impl.

## How It Works

### Plan Creation and Step Generation

Plans are created through two paths:

**User-initiated (via PlanTool):** The `PlanHandlerImpl` in `plan_handler.rs` implements the `PlanHandler` trait. `create_plan()` builds a Plan in Draft status with no steps and persists it. `generate_steps()` then calls the LLM to decompose the plan's description into 3-8 steps (only if the plan has no steps yet). `approve_plan()` transitions Draft to Approved. `execute_plan()` transitions Approved to Executing.

**Auto-initiated (via PlannedEngine):** When the intent pipeline classifies a message as Planned, the `PlannedEngine` generates steps, builds a Plan directly in Approved status, transitions to Executing, and runs all steps in one pass. No user approval step is needed because the pipeline already decided the task warrants planning.

### Step Generation

The `generate_plan_steps()` function in `plan_step_generator.rs` prompts the LLM with the task description, optional prior conversation context, and a list of available tool names. The prompt instructs the LLM to produce 3-8 concrete steps as a JSON array with `description`, `reasoning`, and `expectedTools` fields. Each step description should specify the exact tool action and parameters (e.g., "Call todo with action=update, id=X, due_date=Y").

The response is parsed via `parse_step_drafts()`, which uses `extract_json_array()` to handle markdown fences and preamble text. Results are capped at 8 steps. On parse failure, an empty list is returned so callers can degrade gracefully.

### Step Execution

The `run_step()` function in `plan_executor.rs` executes a single plan step using multi-cycle LLM-tool loops. It runs up to `MAX_CYCLES_PER_STEP` (5) iterations through the `ExecutionCore`:

1. Builds a prompt from the plan context window plus the step's description, reasoning, and expected tools.
2. Sets a system prompt instructing the LLM to execute autonomously (no user interaction), use write/update actions (not just read/list), and reference IDs from previous step results.
3. Calls `ExecutionCore::run_cycle()` in a loop. If the LLM returns tool calls, the tools are executed and results are accumulated. If a tool fails, execution stops with a failure result. If the LLM returns a final text response, the step succeeds with the accumulated output.
4. After max cycles, returns success with whatever output was accumulated.

Returns a `StepExecutionResult` with success flag, output text, optional failure reason, optional confidence assessment, and the name of the first tool called.

### Context Windowing

The `build_step_context()` function constructs a focused context window for each step:

- Plan title and goal description
- Progress indicator (e.g., "step 3/7")
- Results from the last 3 completed steps (truncated to 500 characters each) so the LLM has concrete values (IDs, data) to use as arguments
- Current step marked with `>>> CURRENT`
- Next 3 upcoming steps marked with `NEXT 1`, `NEXT 2`, `NEXT 3`

This sliding window keeps the LLM prompt manageable while providing enough context for proper tool argument construction.

### Backtracking

When a step exceeds its `max_attempts` (default 3 retries per step), the orchestration loop (in `PlannedEngine::run_plan_steps()`) triggers backtracking:

1. A `BacktrackEntry` is recorded in the plan's `backtrack_history` with the step index, attempt number, failure reason, and timestamp.
2. The step is marked Failed. The plan is saved.
3. `regenerate_from()` is called with the plan context and failure details. It prompts the LLM to generate replacement steps from the failure point forward, including a summary of completed steps so the LLM knows what work has already been done.
4. The plan's step list is truncated at the failure index and extended with the new steps.
5. Execution continues from the first new step.

If the LLM returns invalid JSON during regeneration, a single fallback step is created: "Retry: \<original step description\>" with the failure reason as reasoning.

After `MAX_BACKTRACK_ATTEMPTS` (3) full backtrack events, the plan is marked Failed and execution stops. Per-step retries (`attempt_count`) are tracked separately from full backtrack events.

### Plan Completion and Goal Tracking

The `PlanCompletionHandlerImpl` in `plan_executor.rs` implements the `PlanCompletionHandler` trait. When a plan finishes (success or failure) and has a `goal_id`, it updates the linked goal's completion counters (`plans_completed` or `plans_failed`) via atomic `GoalRepo` increments. This enables tracking how many plans a goal has required.

### PlanHandler Operations

The `PlanHandlerImpl` in `plan_handler.rs` provides the full CRUD interface used by PlanTool:

- `create_plan()` -- creates a Draft plan with empty steps
- `get_plan()` / `get_active_plan()` -- loads plans by ID or session key (active = Draft/Approved/Executing)
- `approve_plan()` -- transitions Draft to Approved (strict validation, rejects non-Draft plans)
- `execute_plan()` -- transitions Approved to Executing (validates via `PlanStatus::validate_transition`)
- `generate_steps()` -- calls the LLM to produce steps for a plan with no steps
- `preview_steps()` -- generates step descriptions without persisting (for user review)
- `abandon_plan()` -- transitions any non-terminal state to Abandoned
- `get_step_context()` -- returns the context window string for the current step

### Persistence

Plans and steps are stored in SQLite via `PlanRepo`. The `domain/src/plan.rs` module provides bidirectional conversion functions:

- `plan_to_row()` / `step_to_row()` -- domain types to SQL row structs
- `row_to_plan()` -- SQL rows back to domain types (parses status and visibility from strings, deserializes backtrack_history from JSON)
- `save_plan()` -- upserts the plan row and all step rows
- `load_plan()` -- loads a plan row and its steps, converts to the domain type
- `get_active_plan()` -- finds the most recent non-terminal plan for a session

## Connections

- **Intent Pipeline** (`agent::intent_pipeline`): The `PlannedEngine` is one of three execution engines. It is invoked when the classifier selects Planned mode, or when the Reactive engine escalates due to complexity.
- **ExecutionCore** (`agent::execution`): The shared LLM-call + tool-dispatch mechanism. `run_step()` uses it for multi-cycle step execution. The PlannedEngine holds an `Arc<ExecutionCore>`.
- **PlanTool** (`tools::plan_tool`, Layer 3): Defines the `PlanHandler` and `PlanCompletionHandler` traits. The agent crate provides `PlanHandlerImpl` and `PlanCompletionHandlerImpl` as the concrete implementations, injected via `Arc<dyn Trait>` to avoid circular dependencies.
- **StoragePool / PlanRepo** (`storage` crate, Layer 1.5): Persists plans and steps to SQLite. `PlanRepo` provides CRUD operations, step queries, active plan lookups, and stale plan deletion.
- **GoalRepo** (`storage` crate): Used by `PlanCompletionHandlerImpl` to increment goal completion/failure counters when plans finish.
- **PlanCleanupService** (`agent::intent_pipeline::visibility`): Background hourly task that deletes stale Silent and OnFailure plans based on age thresholds.

## Known Limitations

- **Single-cycle per step in PlanTool path**: When plans are created and executed through the PlanTool (user-initiated), execution is driven by `PlanHandlerImpl::execute_plan()` which only transitions status. The actual step execution in `plan_executor::run_step()` runs up to 5 LLM cycles per step, but each step gets one independent execution context -- there is no cross-step ReAct loop.
- **No real-time progress**: Plan progress is persisted between step executions but there is no streaming progress update to the user during execution. The user sees the final synthesized summary.
- **Iteration limit**: The `iteration_limit` field (default 50) is stored on the Plan struct but is not currently enforced in the orchestration loop. It exists for future use.
- **Backtracking is LLM-dependent**: The quality of regenerated steps depends entirely on the LLM's ability to produce valid JSON and meaningful replacement steps. Invalid JSON falls back to a simple retry step.
- **Synthesis may hallucinate**: The post-execution summary is generated by a separate LLM call. The prompt instructs the LLM to only report actions confirmed by raw tool outputs, but there is no programmatic verification.
