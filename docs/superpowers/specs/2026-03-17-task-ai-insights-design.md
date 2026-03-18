# Task Detail AI Insights — Design Spec

**Date:** 2026-03-17
**Scope:** Wire backend AI and tracking data to the task detail UI
**Approach:** Full-stack per feature (backend → frontend, one feature at a time)

## Overview

The task detail sidebar has UI scaffolding for AI insights (suggestions, memory, "Why This Task Now") that is currently stubbed with empty data. The backend has a rich AI layer — suggestions with confidence scoring, task decomposition, estimation forecasting, and complexity scoring — none of which is exposed to the frontend.

This spec covers wiring four features end-to-end:

1. **AI Suggestions** — on-demand suggestion generation with apply/dismiss
2. **Task Decomposition** — AI-generated subtask plans reviewed in a modal
3. **Complexity Score & Estimation Forecast** — display in sidebar properties and time sections
4. **"Why This Task Now"** — real reasons derived from task data

## Feature 1: AI Suggestions (On-Demand)

### Flow

User clicks "Get Suggestions" in SidebarAiInsights → Tauri command calls `ProactiveHandler::evaluate_task()` with `UserRequested` trigger → candidates persisted to `task_suggestions` table → returned as `Vec<SuggestionResponse>` → displayed in existing `SuggestionCard` components with apply/dismiss actions.

### Backend

**New Tauri commands** (`crates/desktop/src/commands/tasks.rs`):

- `task_get_suggestions(task_id: String) → Vec<SuggestionResponse>` — generates and returns suggestions
- `task_apply_suggestion(suggestion_id: String) → TaskResponse` — applies the suggestion action, returns updated task
- `task_dismiss_suggestion(suggestion_id: String) → ()` — marks suggestion as dismissed

**New response type** (`crates/desktop-shared/src/commands/tasks.rs`):

```rust
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionResponse {
    pub id: String,
    pub suggestion_type: String,
    pub title: String,
    pub description: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub created_at: String,
}
```

**App-core handlers** (`crates/app-core/src/handlers/tasks/`):

- `task_get_suggestions`: loads task, calls `ProactiveHandler::evaluate_task()` with `UserRequested` trigger, persists candidates via `TaskRepo::create_suggestion()`, returns all pending suggestions for the task. Note: no existing app-core handler calls `evaluate_task()` — this is new plumbing from app-core to the agent-implemented handler, not just wiring existing code
- `task_apply_suggestion`: loads pending suggestion via `TaskRepo::get_pending_suggestion()`, delegates to `SuggestionApplier::apply()` (which returns a summary string), then re-fetches the task to build the updated `TaskResponse`. Returns task with entity updates
- `task_dismiss_suggestion`: calls `TaskRepo::resolve_suggestion(id, "dismissed")`

### Frontend

- **useIssueDetail**: replace `suggestions: []` stub with state managed by a "Get Suggestions" button. Call `ipc<SuggestionResponse[]>("task_get_suggestions", { taskId })` on click. Wire `applySuggestion` and `dismissSuggestion` callbacks to corresponding Tauri commands + refetch.
- **SidebarAiInsights**: add a "Get Suggestions" button that triggers the fetch. Existing `SuggestionsList` and `SuggestionCard` components already handle display, apply, and dismiss — no new UI components needed.
- **Mapper**: map `SuggestionResponse` → existing `Suggestion` interface. Note: `SuggestionResponse.description` is `Option<String>` while `Suggestion.description` is `string` — map `null` to `""`. Extra fields (`suggestion_type`, `created_at`) are dropped by the mapper. Optionally extend the `Suggestion` interface to include `suggestionType` for icon/color mapping per type.

### Existing backend infrastructure used

- `TaskSuggestion` type with `SuggestionType`, `SuggestionStatus`, `SuggestionAction` enums
- `ProactiveHandler` trait (dependency-injected, implemented in agent crate)
- `SuggestionApplier` trait for executing suggestion actions
- `TaskRepo` suggestion methods: `create_suggestion`, `resolve_suggestion`, `get_pending_suggestion`, `list_pending_suggestions`

## Feature 2: Task Decomposition (Modal)

### Flow

User clicks "Break Down" button near sub-issues → Tauri command calls `DecompositionHandler::decompose()` → if confidence >= 0.90 threshold, subtasks auto-created and `auto_applied: true` returned → otherwise, plan returned for review → shown in `DecompositionModal` → user accepts or rejects → on accept, subtasks created and sub-issues list refreshes.

### Backend

**New Tauri commands** (`crates/desktop/src/commands/tasks.rs`):

- `task_decompose(task_id: String) → DecompositionResponse` — generates decomposition plan
- `task_apply_decomposition(decomposition_id: String) → Vec<TaskResponse>` — creates subtasks from plan
- `task_reject_decomposition(decomposition_id: String) → ()` — marks plan as rejected

**New response types** (`crates/desktop-shared/src/commands/tasks.rs`):

```rust
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionResponse {
    pub id: String,
    pub confidence: f64,
    pub reasoning: String,
    pub subtasks: Vec<PlannedSubtaskResponse>,
    pub total_estimated_mins: Option<i32>,
    pub warnings: Vec<String>,
    pub auto_applied: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlannedSubtaskResponse {
    pub temp_id: String,
    pub title: String,
    pub description: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub energy_level: Option<String>,
    pub priority: Option<i16>,
    pub children: Vec<PlannedSubtaskResponse>,
}
```

**Note:** `priority` is `i16` matching the backend `PlannedSubtask` type. The frontend maps this to priority labels using existing `priority-icons.tsx` utilities. Fields `acceptance_criteria`, `task_type`, and `dependencies` from backend `PlannedSubtask` are intentionally omitted — they are not needed for the review UI.

**App-core handlers** (`crates/app-core/src/handlers/tasks/`):

- `task_decompose`: loads task, builds `DecompositionContext` (existing subtask titles, project context), calls `DecompositionHandler::decompose()`. If confidence >= 0.90 auto-apply threshold: creates subtasks immediately via `TaskRepo::add()`, marks decomposition as applied, returns `auto_applied: true`. Otherwise: stores as pending decomposition via `TaskRepo::create_decomposition()`, returns plan for review.
- `task_apply_decomposition`: loads pending decomposition, creates subtasks from plan tree, marks decomposition as applied, returns created tasks with entity updates.
- `task_reject_decomposition`: no existing `reject_decomposition` method exists in `TaskRepo`. Add a new method `reject_decomposition(id)` to `crates/storage/src/repos/task_repo/decompositions.rs` that updates the decomposition's status to "rejected" and sets a `resolved_at` timestamp. Pattern matches the existing `apply_decomposition` method.

### Frontend

- **DecompositionModal** — new component in `desktop-ui/src/features/tasks/components/detail/`:
  - Confidence badge + AI reasoning text at top
  - Subtask tree rendered as indented list: title, description (truncated), estimate, energy level per item
  - Nested children shown with indentation
  - Warnings section if any (yellow banner)
  - Total estimated time summary
  - "Apply" and "Cancel" buttons at bottom
  - Uses existing `dialog` UI primitive from tasks components
- **Trigger**: "Break Down" button placed in `IssueContentTab`, near the sub-issues section
- **Auto-apply handling**: if response has `auto_applied: true`, show a success toast ("AI created N subtasks") and refetch sub-issues instead of opening the modal
- **useIssueDetail**: add `decompose()` callback that calls `task_decompose`, handles auto-apply vs modal, and refetches sub-issues on apply

### Existing backend infrastructure used

- `DecompositionHandler` trait (dependency-injected)
- `DecompositionContext`, `DecompositionResult`, `DecompositionTree`, `PlannedSubtask` types
- `TaskRepo` decomposition methods: `create_decomposition`, `get_decomposition`, `list_pending_decompositions`, `apply_decomposition` (+ new `reject_decomposition` to be added)
- `TaskDecomposed` domain event

## Feature 3: Complexity Score & Estimation Forecast

### Complexity Score

**No new backend work.** `complexity_score: Option<i32>` is already in `TaskResponse`.

**Frontend** — requires two changes:
1. **mappers.ts**: add `complexityScore: number | null` to the `DetailTask` interface, and map it in `taskToDetailTask` from `TaskResponse.complexity_score`.
2. **SidebarProperties**: add a row displaying complexity as a color-coded badge (0-100 scale):
- 0-30: green ("Low")
- 31-60: yellow ("Medium")
- 61-80: orange ("High")
- 81-100: red ("Very High")

Only shown when `complexity_score` is not null. Placed after the energy level row.

### Estimation Forecast

**New Tauri command** (`crates/desktop/src/commands/tasks.rs`):

- `task_forecast(task_id: String) → TaskForecastResponse` — returns forecast for a task

**New response types** (`crates/desktop-shared/src/commands/tasks.rs`):

```rust
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskForecastResponse {
    pub estimated_minutes: i32,
    pub confidence_low: i32,
    pub confidence_high: i32,
    pub methodology: String,
    pub sample_size: u32,
    pub data_quality: String,
    pub risks: Vec<ForecastRiskResponse>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForecastRiskResponse {
    pub kind: String,
    pub description: String,
    pub impact_minutes: Option<i32>,
}
```

**App-core handler**: `task_forecast` loads the task, calls `ForecastHandler::forecast_task()` with default context (lookback 90 days, min sample size from config), maps result to response. The backend `ForecastMethodology` struct (with `name`, `sample_size`, `lookback_days`, `adjustments`) is flattened: `methodology` gets `methodology.name`, `sample_size` gets `methodology.sample_size`. `DataQuality` and `RiskKind` enums are serialized to lowercase strings (e.g., `"high"`, `"historical_underestimation"`). Returns error if task has no estimate.

**Frontend** — `SidebarTime`: replace the existing hardcoded forecast placeholder (`Math.round(estimatedSecs * 1.1)`) with an on-demand "Forecast" section:
- Triggered by a "Forecast" link/button (not auto-loaded)
- Range display: `confidence_low — estimated — confidence_high` (e.g., "25m — 40m — 55m")
- Data quality line: "Based on N similar tasks" with quality indicator
- Risk bullets if any (e.g., "Historical underestimation pattern")
- Only available when task has an estimate

### Existing backend infrastructure used

- `ForecastHandler` trait with `forecast_task()` method
- `TaskForecast`, `ForecastMethodology`, `ForecastRisk`, `DataQuality` types
- `forecast.rs` pure computation functions (similarity scoring, deviation correction)
- `TaskRepo` estimation methods: `estimation_stats`, `list_estimation_history`

## Feature 4: "Why This Task Now" (Real Data)

### Backend

**No new Tauri commands.** All data is already in `TaskResponse`.

### Frontend

Replace the hardcoded `WhyThisTaskNow` component in `SidebarAiInsights` with a function that computes real reasons from `DetailTask` fields.

**Reason computation logic** — build array of `{ icon, text, weight }`, filter to applicable, sort by weight descending, show top 3:

| Condition | Text | Weight |
|-----------|------|--------|
| `priority === "urgent"` | "P1 — highest priority" | 100 |
| `priority === "high"` | "P2 — high priority" | 80 |
| due date is past | "Overdue by N days" | 95 |
| due date is today | "Due today" | 90 |
| due date within 3 days | "Due in N days" | 70 |
| `focusedAt` is set | "You're already in flow" | 85 |
| `energyLevel` matches time of day | "Matches your current energy window" | 60 |
| `complexityScore` <= 30 | "Quick win — low complexity" | 50 |

**Time-of-day energy heuristic** (using `new Date().getHours()` in local time):
- 6am-12pm: high energy
- 12pm-5pm: medium energy
- 5pm-10pm: low energy

Show nothing if no reasons apply (empty state hidden). Only shown for non-completed tasks without pending suggestions (preserving the existing conditional logic in `SidebarAiInsights`).

## Files Changed Summary

### New files
- `desktop-ui/src/features/tasks/components/detail/DecompositionModal.tsx`

### Modified files

**Rust:**
- `crates/desktop-shared/src/commands/tasks.rs` — add `SuggestionResponse`, `DecompositionResponse`, `PlannedSubtaskResponse`, `TaskForecastResponse`, `ForecastRiskResponse`
- `crates/desktop/src/commands/tasks.rs` — add 7 new Tauri commands + update `DEV_COMMANDS`
- `crates/app-core/src/handlers/tasks/` — add suggestion, decomposition, forecast handler methods (new file or extend existing)
- `crates/storage/src/repos/task_repo/decompositions.rs` — add `reject_decomposition` method

**TypeScript:**
- `desktop-ui/src/features/tasks/hooks/useIssueDetail.ts` — replace stubs with real data fetching
- `desktop-ui/src/features/tasks/components/detail/SidebarAiInsights.tsx` — wire suggestions, replace hardcoded WhyThisTaskNow
- `desktop-ui/src/features/tasks/components/detail/SidebarProperties.tsx` — add complexity score row
- `desktop-ui/src/features/tasks/components/detail/SidebarTime.tsx` — add forecast section
- `desktop-ui/src/features/tasks/components/detail/IssueContentTab.tsx` — add "Break Down" button
- `desktop-ui/src/features/tasks/lib/mappers.ts` — add `complexityScore` to `DetailTask` interface, add response type mappings for suggestions/decomposition/forecast

## Build Order

1. **AI Suggestions** — backend commands + wire existing UI
2. **Task Decomposition** — backend commands + new modal component
3. **Complexity Score & Forecast** — backend forecast command + sidebar additions
4. **"Why This Task Now"** — frontend-only, no backend changes

## Out of Scope

- Task dependencies/blockers UI
- Attachments
- Individual time entries list
- Agentic execution monitoring
- Task memory / cognitive integration
- Auto-generating suggestions on task create/update (proactive mode)
- Estimation accuracy history UI
