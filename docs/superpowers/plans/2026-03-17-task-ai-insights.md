# Task Detail AI Insights Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire backend AI suggestion, decomposition, forecast, and complexity data to the task detail frontend UI.

**Architecture:** Full-stack per feature. Each feature adds response types in `desktop-shared`, Tauri commands in `desktop`, app-core handlers, then frontend hooks and UI. Backend handler traits (ProactiveHandler, DecompositionHandler, ForecastHandler, SuggestionApplier) are defined in `feature-tasks`, implemented in `agent` crate (as `LlmProactiveHandler`, `LlmDecompositionHandler`, `LlmForecastHandler`, `TaskSuggestionApplier`). Currently these are only constructed in `crates/app-core/src/init/cron.rs` and captured in cron closures — they are NOT on `AppCore`. **Task 0 adds them as fields on AppCore** so Tauri commands can call them on-demand.

**Tech Stack:** Rust (Tauri 2, SQLite via sqlx), TypeScript (React, Zustand), Tailwind CSS v4

**Spec:** `docs/superpowers/specs/2026-03-17-task-ai-insights-design.md`

---

## Chunk 0: Handler Trait Injection into AppCore

### Task 0: Add handler trait fields to AppCore

Currently `ProactiveHandler`, `DecompositionHandler`, `ForecastHandler`, and `SuggestionApplier` are only constructed in `crates/app-core/src/init/cron.rs` (lines 50-67) and captured in cron closures. They are NOT accessible from `AppCore`. We need to add them as fields so Tauri commands can call them on-demand.

**Files:**
- Modify: `crates/app-core/src/state.rs` (lines 34-97, AppCore struct)
- Modify: `crates/app-core/src/init/cron.rs` (lines 50-67, handler construction)
- Modify: wherever `AppCore` is constructed (likely `crates/desktop/src/main.rs` or `crates/app-core/src/init/`)

- [ ] **Step 1: Add handler trait fields to AppCore struct**

In `crates/app-core/src/state.rs`, add to the `AppCore` struct (after line 42):

```rust
pub proactive_handler: Option<Arc<dyn feature_tasks::handlers::ProactiveHandler>>,
pub suggestion_applier: Option<Arc<dyn feature_tasks::handlers::SuggestionApplier>>,
pub decomposition_handler: Option<Arc<dyn feature_tasks::handlers::DecompositionHandler>>,
pub forecast_handler: Option<Arc<dyn feature_tasks::handlers::ForecastHandler>>,
```

Add necessary imports at the top of the file.

- [ ] **Step 2: Initialize fields as None in AppCore constructor**

Find where `AppCore` is constructed and set all four fields to `None`. Then in the init code (likely near cron.rs), after the handlers are built, clone them into `AppCore`:

```rust
app_core.proactive_handler = Some(proactive_handler.clone());
app_core.suggestion_applier = suggestion_applier.clone();
app_core.decomposition_handler = Some(Arc::new(
    agent::handlers::LlmDecompositionHandler::new(
        provider.clone(),
        config.agents.defaults.model.clone(),
        repos.tasks.clone(),
        domain_event_bus.clone(),
    ),
));
app_core.forecast_handler = Some(Arc::new(
    agent::handlers::LlmForecastHandler::new(
        provider.clone(),
        config.agents.defaults.model.clone(),
        repos.tasks.clone(),
    ),
));
```

**Note:** Read the actual `LlmDecompositionHandler::new()` and `LlmForecastHandler::new()` constructors in `crates/agent/src/handlers/` to match the exact parameters. They follow the same pattern as `LlmProactiveHandler::new()`.

- [ ] **Step 3: Add getter helpers on AppCore**

```rust
impl AppCore {
    pub fn proactive_handler(&self) -> Result<&dyn feature_tasks::handlers::ProactiveHandler, ApiError> {
        self.proactive_handler
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "ProactiveHandler not initialized"))
    }

    pub fn suggestion_applier(&self) -> Result<&dyn feature_tasks::handlers::SuggestionApplier, ApiError> {
        self.suggestion_applier
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "SuggestionApplier not initialized"))
    }

    pub fn decomposition_handler(&self) -> Result<&dyn feature_tasks::handlers::DecompositionHandler, ApiError> {
        self.decomposition_handler
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "DecompositionHandler not initialized"))
    }

    pub fn forecast_handler(&self) -> Result<&dyn feature_tasks::handlers::ForecastHandler, ApiError> {
        self.forecast_handler
            .as_deref()
            .ok_or_else(|| ApiError::new("INTERNAL", "ForecastHandler not initialized"))
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p app-core`
Expected: success (handlers are None initially, tests won't call them)

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/
git commit -m "feat(app-core): add handler trait fields to AppCore for on-demand AI features"
```

---

## Chunk 1: AI Suggestions (Feature 1)

### Task 1: Add SuggestionResponse type to desktop-shared

**Files:**
- Modify: `crates/desktop-shared/src/commands/tasks.rs` (after line 36, end of TaskResponse)

- [ ] **Step 1: Add SuggestionResponse struct**

Add after the `TaskResponse` struct (line 36):

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
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

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands/tasks.rs
git commit -m "feat(desktop-shared): add SuggestionResponse type"
```

### Task 2: Add reject_decomposition to storage (needed early for clean builds)

This is listed in Feature 2 spec but we add it now so the storage crate stays buildable as we add app-core handlers that reference it.

**Files:**
- Modify: `crates/storage/src/repos/task_repo/decompositions.rs` (after apply_decomposition at line 68)

- [ ] **Step 1: Add reject_decomposition method**

Add after `apply_decomposition` (line 68):

```rust
pub async fn reject_decomposition(&self, id: &str) -> Result<bool, StorageError> {
    let result = sqlx::query(
        "UPDATE task_decompositions SET status = 'rejected', applied_at = datetime('now') WHERE id = ?1 AND status = 'pending'",
    )
    .bind(id)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p storage`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/storage/src/repos/task_repo/decompositions.rs
git commit -m "feat(storage): add reject_decomposition method"
```

### Task 3: Add suggestion app-core handlers

**Files:**
- Create: `crates/app-core/src/handlers/tasks/suggestions.rs`
- Modify: `crates/app-core/src/handlers/tasks/mod.rs` (add module declaration)

- [ ] **Step 1: Create suggestions.rs with three handler methods**

Read `crates/app-core/src/handlers/tasks/mod.rs` to see the module list and `impl AppCore` pattern. Read `crates/app-core/src/state.rs` lines 38-97 to understand how AppCore accesses repos and agent. Then create the file:

```rust
use crate::ApiError;
use desktop_shared::commands::tasks::{SuggestionResponse, TaskResponse};
use feature_tasks::types::suggestion::{SuggestionTrigger, TaskSuggestion};

use crate::handlers::tasks::converters::row_to_task_response;
use crate::{AppCore, EntityUpdate, HandlerResult};

fn map_suggestion(s: &TaskSuggestion) -> SuggestionResponse {
    SuggestionResponse {
        id: s.id.clone(),
        suggestion_type: format!("{:?}", s.suggestion_type).to_lowercase(),
        title: s.title.clone(),
        description: s.description.clone(),
        confidence: s.confidence,
        status: format!("{:?}", s.status).to_lowercase(),
        created_at: s.created_at.to_rfc3339(),
    }
}

impl AppCore {
    pub async fn task_get_suggestions(
        &self,
        task_id: String,
    ) -> Result<Vec<SuggestionResponse>, ApiError> {
        // Load task
        let task_row = self
            .repos
            .tasks
            .get(&task_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("Task {task_id} not found")))?;

        // Convert row to domain Task for the handler
        let task = feature_tasks::types::Task::from(task_row);

        // Call ProactiveHandler::evaluate_task via injected handler
        let handler = self.proactive_handler()?;
        let candidates = handler
            .evaluate_task(&task, &SuggestionTrigger::UserRequested)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        // Persist candidates
        for candidate in &candidates {
            let suggestion = TaskSuggestion::from_candidate(candidate, Some(task_id.clone()));
            self.repos
                .tasks
                .create_suggestion(&suggestion.into())
                .await
                .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;
        }

        // Return all pending suggestions for this task
        let rows = self
            .repos
            .tasks
            .list_pending_suggestions(Some(&task_id))
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        let suggestions: Vec<SuggestionResponse> = rows
            .iter()
            .map(|row| SuggestionResponse {
                id: row.id.clone(),
                suggestion_type: row.suggestion_type.clone(),
                title: row.title.clone(),
                description: row.description.clone(),
                confidence: row.confidence,
                status: row.status.clone(),
                created_at: row.created_at.to_rfc3339(),
            })
            .collect();

        Ok(suggestions)
    }

    pub async fn task_apply_suggestion(
        &self,
        suggestion_id: String,
    ) -> HandlerResult<TaskResponse> {
        // Load the pending suggestion
        let suggestion = self
            .repos
            .tasks
            .get_pending_suggestion(&suggestion_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("Suggestion {suggestion_id} not found")))?;

        let task_id = suggestion
            .task_id
            .clone()
            .ok_or_else(|| ApiError::new("INTERNAL", "Suggestion has no task_id"))?;

        // Parse action and apply via SuggestionApplier
        if let Some(action_json) = &suggestion.action_payload {
            let action: feature_tasks::types::suggestion::SuggestionAction =
                serde_json::from_str(action_json)
                    .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

            let applier = self.suggestion_applier()?;
            applier
                .apply(&suggestion_id, Some(&task_id), &action)
                .await
                .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;
        }

        // Mark suggestion as applied
        self.repos
            .tasks
            .resolve_suggestion(&suggestion_id, "applied")
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        // Re-fetch task for updated response
        let (task_response, updates) = self.task_get_with_updates(task_id.clone()).await?;

        Ok((
            task_response.ok_or_else(|| ApiError::new("NOT_FOUND", format!("Task {task_id} not found")))?,
            updates,
        ))
    }

    pub async fn task_dismiss_suggestion(
        &self,
        suggestion_id: String,
    ) -> Result<(), ApiError> {
        self.repos
            .tasks
            .resolve_suggestion(&suggestion_id, "dismissed")
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;
        Ok(())
    }
}
```

**Important:** Verify how `TaskRow` converts to `Task` — check if `Task::from(TaskRow)` exists or if a different conversion is needed. Adapt the code accordingly.

- [ ] **Step 2: Add module to mod.rs**

Add `mod suggestions;` to `crates/app-core/src/handlers/tasks/mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app-core`
Expected: success (may need to adjust method names based on actual AgentLoop API)

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/tasks/suggestions.rs crates/app-core/src/handlers/tasks/mod.rs
git commit -m "feat(app-core): add suggestion handler methods"
```

### Task 4: Add suggestion Tauri commands

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs` (add commands + update DEV_COMMANDS)

- [ ] **Step 1: Add three Tauri commands**

Add after the existing commands (before DEV_COMMANDS at line 132):

```rust
#[tauri::command]
pub async fn task_get_suggestions(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Vec<SuggestionResponse>, ApiError> {
    state.task_get_suggestions(task_id).await
}

#[tauri::command]
pub async fn task_apply_suggestion(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    suggestion_id: String,
) -> Result<TaskResponse, ApiError> {
    let (response, updates) = state.task_apply_suggestion(suggestion_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_dismiss_suggestion(
    state: State<'_, Arc<AppCore>>,
    suggestion_id: String,
) -> Result<(), ApiError> {
    state.task_dismiss_suggestion(suggestion_id).await
}
```

Add `SuggestionResponse` to the imports from `desktop_shared`.

- [ ] **Step 2: Update DEV_COMMANDS**

Add `"task_get_suggestions"`, `"task_apply_suggestion"`, `"task_dismiss_suggestion"` to the DEV_COMMANDS array.

- [ ] **Step 3: Update dispatch_dev function**

Add match arms in `dispatch_dev` (around line 149) using the `dev_helpers` pattern:

```rust
"task_get_suggestions" => dev::val(
    core.task_get_suggestions(try_field!(dev::get_str(body, "taskId")).into())
        .await
),
"task_apply_suggestion" => dev::val_rh(
    core.task_apply_suggestion(try_field!(dev::get_str(body, "suggestionId")).into())
        .await
),
"task_dismiss_suggestion" => dev::val(
    core.task_dismiss_suggestion(try_field!(dev::get_str(body, "suggestionId")).into())
        .await
),
```

- [ ] **Step 4: Register commands in Tauri builder**

In `crates/desktop/src/main.rs` (around line 317, inside `tauri::generate_handler![]`), add:

```rust
commands::tasks::task_get_suggestions,
commands::tasks::task_apply_suggestion,
commands::tasks::task_dismiss_suggestion,
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p desktop`
Expected: success

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): add suggestion Tauri commands"
```

### Task 5: Wire suggestions in frontend hook

**Files:**
- Modify: `desktop-ui/src/features/tasks/hooks/useIssueDetail.ts` (lines 167-172, stubs)
- Modify: `desktop-ui/src/features/tasks/lib/mappers.ts` (Suggestion interface)

- [ ] **Step 1: Extend Suggestion interface in mappers.ts**

Find the `Suggestion` interface and add `suggestionType`:

```typescript
export interface Suggestion {
  id: string;
  suggestionType: string;  // NEW
  title: string;
  description: string;
  confidence: number;
  status: SuggestionStatus;
}
```

- [ ] **Step 2: Replace suggestion stubs in useIssueDetail.ts**

Replace the stubs at lines 167-172 with real state and callbacks:

```typescript
const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
const [suggestionsLoading, setSuggestionsLoading] = useState(false);

const fetchSuggestions = useCallback(async () => {
  setSuggestionsLoading(true);
  try {
    const results = await ipc<Array<{
      id: string;
      suggestionType: string;
      title: string;
      description: string | null;
      confidence: number;
      status: string;
      createdAt: string;
    }>>("task_get_suggestions", { taskId: task.id });
    setSuggestions(
      results.map((s) => ({
        id: s.id,
        suggestionType: s.suggestionType,
        title: s.title,
        description: s.description ?? "",
        confidence: s.confidence,
        status: s.status as SuggestionStatus,
      })),
    );
  } finally {
    setSuggestionsLoading(false);
  }
}, [task.id]);

const applySuggestion = useCallback(
  async (id: string) => {
    await ipc("task_apply_suggestion", { suggestionId: id });
    await fetchSuggestions();
    refetch();
  },
  [fetchSuggestions, refetch],
);

const dismissSuggestion = useCallback(
  async (id: string) => {
    await ipc("task_dismiss_suggestion", { suggestionId: id });
    setSuggestions((prev) => prev.filter((s) => s.id !== id));
  },
  [],
);
```

- [ ] **Step 3: Add fetchSuggestions and suggestionsLoading to the return object**

Update the return object (line ~182) to include `fetchSuggestions` and `suggestionsLoading`.

- [ ] **Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks/hooks/useIssueDetail.ts desktop-ui/src/features/tasks/lib/mappers.ts
git commit -m "feat(tasks-ui): wire suggestion data fetching in useIssueDetail"
```

### Task 6: Add "Get Suggestions" button to SidebarAiInsights

**Files:**
- Modify: `desktop-ui/src/features/tasks/components/detail/SidebarAiInsights.tsx`

- [ ] **Step 1: Update SidebarAiInsightsProps interface**

Add `onFetchSuggestions` and `suggestionsLoading` to the props interface (line 6):

```typescript
interface SidebarAiInsightsProps {
  taskState: TaskState;
  suggestions: Suggestion[];
  taskMemory: TaskMemory | null;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
  onFetchSuggestions: () => void;   // NEW
  suggestionsLoading: boolean;       // NEW
}
```

- [ ] **Step 2: Add "Get Suggestions" button**

In the main component body, add a button before the conditional rendering (around line 23). When `taskState !== "completed"` and there are no pending suggestions, show:

```tsx
{taskState !== "completed" && suggestions.filter((s) => s.status === "pending").length === 0 && (
  <button
    type="button"
    onClick={onFetchSuggestions}
    disabled={suggestionsLoading}
    className="w-full flex items-center justify-center gap-1.5 text-xs px-3 py-1.5 rounded-md border border-purple-500/30 text-purple-300 hover:bg-purple-500/10 transition-colors disabled:opacity-50"
  >
    <Sparkles className="size-3" />
    {suggestionsLoading ? "Analyzing..." : "Get Suggestions"}
  </button>
)}
```

- [ ] **Step 3: Pass new props from IssueDetailSidebar**

Find `IssueDetailSidebar.tsx` and pass the new `onFetchSuggestions` and `suggestionsLoading` props through from the `detail` object.

- [ ] **Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks/components/detail/SidebarAiInsights.tsx desktop-ui/src/features/tasks/components/detail/IssueDetailSidebar.tsx
git commit -m "feat(tasks-ui): add Get Suggestions button to AI insights sidebar"
```

---

## Chunk 2: Task Decomposition (Feature 2)

### Task 7: Add decomposition response types to desktop-shared

**Files:**
- Modify: `crates/desktop-shared/src/commands/tasks.rs`

- [ ] **Step 1: Add DecompositionResponse and PlannedSubtaskResponse structs**

Add after `SuggestionResponse`:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands/tasks.rs
git commit -m "feat(desktop-shared): add DecompositionResponse types"
```

### Task 8: Add decomposition app-core handlers

**Files:**
- Create: `crates/app-core/src/handlers/tasks/decomposition.rs`
- Modify: `crates/app-core/src/handlers/tasks/mod.rs`

- [ ] **Step 1: Create decomposition.rs**

Read `crates/feature-tasks/src/types/suggestion.rs` to understand `DecompositionContext`, `DecompositionResult`, `PlannedSubtask` types. Read `crates/feature-tasks/src/handlers/decomposition.rs` for the `DecompositionHandler` trait. Then create:

```rust
use crate::ApiError;
use desktop_shared::commands::tasks::{
    DecompositionResponse, PlannedSubtaskResponse, TaskResponse,
};

use crate::{AppCore, EntityUpdate, HandlerResult};

fn map_subtask(s: &feature_tasks::types::suggestion::PlannedSubtask) -> PlannedSubtaskResponse {
    PlannedSubtaskResponse {
        temp_id: s.temp_id.clone(),
        title: s.title.clone(),
        description: s.description.clone(),
        estimated_minutes: s.estimated_minutes,
        energy_level: s.energy_level.as_ref().map(|e| format!("{e:?}").to_lowercase()),
        priority: s.priority,
        children: s.children.iter().map(map_subtask).collect(),
    }
}

impl AppCore {
    pub async fn task_decompose(
        &self,
        task_id: String,
    ) -> HandlerResult<DecompositionResponse> {
        let task_row = self
            .repos
            .tasks
            .get(&task_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("Task {task_id} not found")))?;

        let task = feature_tasks::types::Task::from(task_row);

        // Build decomposition context
        let existing_children = self
            .repos
            .tasks
            .list_children(&task_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;
        let existing_titles: Vec<String> = existing_children.iter().map(|c| c.title.clone()).collect();

        let context = feature_tasks::types::suggestion::DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 7,
            existing_subtasks: existing_titles,
            project_context: None,
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };

        // Call DecompositionHandler via injected handler
        let handler = self.decomposition_handler()?;
        let result = handler
            .decompose(&task, &context)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        let auto_apply_threshold = 0.90;
        let mut updates = vec![];

        if result.confidence >= auto_apply_threshold {
            // Auto-apply: create subtasks
            let created_ids = self
                .create_subtasks_from_plan(&task_id, &result.tree.subtasks)
                .await?;
            updates.push(EntityUpdate {
                kind: crate::EntityKind::Task,
                id: task_id.clone(),
            });
            for id in &created_ids {
                updates.push(EntityUpdate {
                    kind: crate::EntityKind::Task,
                    id: id.clone(),
                });
            }

            Ok((
                DecompositionResponse {
                    id: String::new(), // auto-applied, no stored plan
                    confidence: result.confidence,
                    reasoning: result.reasoning,
                    subtasks: result.tree.subtasks.iter().map(map_subtask).collect(),
                    total_estimated_mins: result.tree.total_estimated_mins,
                    warnings: result.validation_warnings.iter().map(|w| w.message.clone()).collect(),
                    auto_applied: true,
                },
                updates,
            ))
        } else {
            // Store as pending for review
            let decomp_id = common::new_id();
            let plan_json = serde_json::to_string(&result.tree)
                .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

            self.repos
                .tasks
                .create_decomposition(&storage::repos::task_repo::TaskDecompositionRow {
                    id: decomp_id.clone(),
                    task_id: task_id.clone(),
                    plan: plan_json,
                    confidence: result.confidence,
                    status: "pending".into(),
                    reasoning: Some(result.reasoning.clone()),
                    created_at: chrono::Utc::now(),
                    applied_at: None,
                })
                .await
                .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

            Ok((
                DecompositionResponse {
                    id: decomp_id,
                    confidence: result.confidence,
                    reasoning: result.reasoning,
                    subtasks: result.tree.subtasks.iter().map(map_subtask).collect(),
                    total_estimated_mins: result.tree.total_estimated_mins,
                    warnings: result.validation_warnings.iter().map(|w| w.message.clone()).collect(),
                    auto_applied: false,
                },
                vec![],
            ))
        }
    }

    pub async fn task_apply_decomposition(
        &self,
        decomposition_id: String,
    ) -> HandlerResult<Vec<TaskResponse>> {
        let decomp = self
            .repos
            .tasks
            .get_decomposition(&decomposition_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Decomposition not found"))?;

        let tree: feature_tasks::types::suggestion::DecompositionTree =
            serde_json::from_str(&decomp.plan)
                .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        let created_ids = self
            .create_subtasks_from_plan(&decomp.task_id, &tree.subtasks)
            .await?;

        self.repos
            .tasks
            .apply_decomposition(&decomposition_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        let mut responses = vec![];
        let mut updates = vec![];
        for id in created_ids {
            if let Some(row) = self.repos.tasks.get(&id).await.map_err(|e| ApiError::new("INTERNAL", e.to_string()))? {
                responses.push(crate::handlers::tasks::converters::row_to_task_response(&row, &self.repos).await?);
            }
            updates.push(EntityUpdate {
                kind: crate::EntityKind::Task,
                id,
            });
        }

        Ok((responses, updates))
    }

    pub async fn task_reject_decomposition(
        &self,
        decomposition_id: String,
    ) -> Result<(), ApiError> {
        self.repos
            .tasks
            .reject_decomposition(&decomposition_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;
        Ok(())
    }

    /// Helper: create subtasks from a planned subtask tree
    async fn create_subtasks_from_plan(
        &self,
        parent_id: &str,
        subtasks: &[feature_tasks::types::suggestion::PlannedSubtask],
    ) -> Result<Vec<String>, ApiError> {
        let mut created_ids = vec![];
        for planned in subtasks {
            let params = desktop_shared::commands::tasks::TaskCreateParams {
                title: planned.title.clone(),
                area_id: None,
                project_id: None,
                priority: planned.priority,
                due_date: None,
                tags: None,
                parent_id: Some(parent_id.into()),
                status_label_id: None,
                group_id: None,
                task_type: None,
                acceptance_criteria: None,
                energy_level: planned.energy_level.as_ref().map(|e| format!("{e:?}").to_lowercase()),
                estimated_minutes: planned.estimated_minutes,
            };
            let (task, _updates) = self.task_create(params).await?;
            created_ids.push(task.id.clone());

            // Recurse for children
            if !planned.children.is_empty() {
                let child_ids = self
                    .create_subtasks_from_plan(&task.id, &planned.children)
                    .await?;
                created_ids.extend(child_ids);
            }
        }
        Ok(created_ids)
    }
}
```

**Important:** Verify `TaskDecompositionRow` field names in storage and `row_to_task_response` import path. Adapt as needed.

- [ ] **Step 2: Add module to mod.rs**

Add `mod decomposition;` to `crates/app-core/src/handlers/tasks/mod.rs`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app-core`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/handlers/tasks/decomposition.rs crates/app-core/src/handlers/tasks/mod.rs
git commit -m "feat(app-core): add decomposition handler methods"
```

### Task 9: Add decomposition Tauri commands

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs`

- [ ] **Step 1: Add three Tauri commands**

```rust
#[tauri::command]
pub async fn task_decompose(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<DecompositionResponse, ApiError> {
    let (response, updates) = state.task_decompose(task_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_apply_decomposition(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    decomposition_id: String,
) -> Result<Vec<TaskResponse>, ApiError> {
    let (response, updates) = state.task_apply_decomposition(decomposition_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_reject_decomposition(
    state: State<'_, Arc<AppCore>>,
    decomposition_id: String,
) -> Result<(), ApiError> {
    state.task_reject_decomposition(decomposition_id).await
}
```

Add `DecompositionResponse` to imports from `desktop_shared`.

- [ ] **Step 2: Update DEV_COMMANDS and dispatch_dev**

Add `"task_decompose"`, `"task_apply_decomposition"`, `"task_reject_decomposition"` to DEV_COMMANDS. Add match arms in `dispatch_dev`:

```rust
"task_decompose" => dev::val_rh(
    core.task_decompose(try_field!(dev::get_str(body, "taskId")).into())
        .await
),
"task_apply_decomposition" => dev::val_rh(
    core.task_apply_decomposition(try_field!(dev::get_str(body, "decompositionId")).into())
        .await
),
"task_reject_decomposition" => dev::val(
    core.task_reject_decomposition(try_field!(dev::get_str(body, "decompositionId")).into())
        .await
),
```

- [ ] **Step 3: Register in Tauri builder**

In `crates/desktop/src/main.rs` (inside `tauri::generate_handler![]`), add:

```rust
commands::tasks::task_decompose,
commands::tasks::task_apply_decomposition,
commands::tasks::task_reject_decomposition,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p desktop`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): add decomposition Tauri commands"
```

### Task 10: Build DecompositionModal component

**Files:**
- Create: `desktop-ui/src/features/tasks/components/detail/DecompositionModal.tsx`

- [ ] **Step 1: Create the modal component**

Read the existing `dialog` UI primitive at `desktop-ui/src/features/tasks/components/ui/dialog.tsx` to understand the dialog pattern. Then create:

```tsx
import { Bot, ChevronRight, AlertTriangle, Clock, Zap } from "lucide-react";
import type { ReactNode } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";

interface PlannedSubtask {
  tempId: string;
  title: string;
  description: string | null;
  estimatedMinutes: number | null;
  energyLevel: string | null;
  priority: number | null;
  children: PlannedSubtask[];
}

interface DecompositionResult {
  id: string;
  confidence: number;
  reasoning: string;
  subtasks: PlannedSubtask[];
  totalEstimatedMins: number | null;
  warnings: string[];
  autoApplied: boolean;
}

interface DecompositionModalProps {
  result: DecompositionResult | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onApply: (id: string) => void;
  onReject: (id: string) => void;
  applying: boolean;
}

export function DecompositionModal({
  result,
  open,
  onOpenChange,
  onApply,
  onReject,
  applying,
}: DecompositionModalProps) {
  if (!result) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bot className="size-4 text-purple-400" />
            Task Breakdown
          </DialogTitle>
          <DialogDescription className="flex items-center gap-2">
            <span
              className={`text-xs px-1.5 py-0.5 rounded ${
                result.confidence >= 0.8
                  ? "bg-green-500/20 text-green-300"
                  : result.confidence >= 0.6
                    ? "bg-yellow-500/20 text-yellow-300"
                    : "bg-red-500/20 text-red-300"
              }`}
            >
              {Math.round(result.confidence * 100)}% confidence
            </span>
          </DialogDescription>
        </DialogHeader>

        {/* AI Reasoning */}
        <div className="text-xs text-[hsl(var(--muted-foreground))] bg-[hsl(var(--accent))]/50 rounded-md p-3">
          {result.reasoning}
        </div>

        {/* Warnings */}
        {result.warnings.length > 0 && (
          <div className="rounded-md border border-yellow-500/30 bg-yellow-500/10 p-3 space-y-1">
            {result.warnings.map((w) => (
              <div key={w} className="flex items-start gap-2 text-xs text-yellow-300">
                <AlertTriangle className="size-3 shrink-0 mt-0.5" />
                {w}
              </div>
            ))}
          </div>
        )}

        {/* Subtask tree */}
        <div className="space-y-1">
          <div className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider mb-2">
            Proposed Subtasks
          </div>
          {result.subtasks.map((s) => (
            <SubtaskItem key={s.tempId} subtask={s} depth={0} />
          ))}
        </div>

        {/* Total estimate */}
        {result.totalEstimatedMins != null && (
          <div className="flex items-center gap-1.5 text-xs text-[hsl(var(--muted-foreground))] pt-2 border-t border-[hsl(var(--border))]/50">
            <Clock className="size-3" />
            Total estimate: {result.totalEstimatedMins}m
          </div>
        )}

        <DialogFooter>
          <button
            type="button"
            onClick={() => onReject(result.id)}
            className="text-xs px-3 py-1.5 rounded text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))] transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onApply(result.id)}
            disabled={applying}
            className="text-xs px-3 py-1.5 rounded bg-purple-500/20 text-purple-300 hover:bg-purple-500/30 transition-colors disabled:opacity-50"
          >
            {applying ? "Creating..." : `Create ${countSubtasks(result.subtasks)} subtasks`}
          </button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function SubtaskItem({ subtask, depth }: { subtask: PlannedSubtask; depth: number }) {
  return (
    <>
      <div
        className="flex items-start gap-2 rounded-md p-2 hover:bg-[hsl(var(--accent))]/50"
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
      >
        {depth > 0 && <ChevronRight className="size-3 text-[hsl(var(--muted-foreground))]/50 shrink-0 mt-0.5" />}
        <div className="flex-1 min-w-0">
          <div className="text-sm text-[hsl(var(--foreground))]">{subtask.title}</div>
          {subtask.description && (
            <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5 line-clamp-2">
              {subtask.description}
            </p>
          )}
          <div className="flex items-center gap-2 mt-1">
            {subtask.estimatedMinutes != null && (
              <span className="text-[10px] text-[hsl(var(--muted-foreground))] flex items-center gap-0.5">
                <Clock className="size-2.5" />
                {subtask.estimatedMinutes}m
              </span>
            )}
            {subtask.energyLevel && (
              <span className="text-[10px] text-[hsl(var(--muted-foreground))] flex items-center gap-0.5">
                <Zap className="size-2.5" />
                {subtask.energyLevel}
              </span>
            )}
          </div>
        </div>
      </div>
      {subtask.children.map((c) => (
        <SubtaskItem key={c.tempId} subtask={c} depth={depth + 1} />
      ))}
    </>
  );
}

function countSubtasks(subtasks: PlannedSubtask[]): number {
  return subtasks.reduce((acc, s) => acc + 1 + countSubtasks(s.children), 0);
}
```

- [ ] **Step 2: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks/components/detail/DecompositionModal.tsx
git commit -m "feat(tasks-ui): add DecompositionModal component"
```

### Task 11: Wire decomposition in hook and content tab

**Files:**
- Modify: `desktop-ui/src/features/tasks/hooks/useIssueDetail.ts`
- Modify: `desktop-ui/src/features/tasks/components/detail/IssueContentTab.tsx`

- [ ] **Step 1: Add decomposition state and callbacks to useIssueDetail**

Add imports and state:

```typescript
const [decompositionResult, setDecompositionResult] = useState<DecompositionResult | null>(null);
const [decompositionOpen, setDecompositionOpen] = useState(false);
const [decompositionApplying, setDecompositionApplying] = useState(false);

const decompose = useCallback(async () => {
  const result = await ipc<DecompositionResult>("task_decompose", { taskId: task.id });
  if (result.autoApplied) {
    // Auto-applied, just refetch sub-issues
    refetch();
  } else {
    setDecompositionResult(result);
    setDecompositionOpen(true);
  }
}, [task.id, refetch]);

const applyDecomposition = useCallback(async (id: string) => {
  setDecompositionApplying(true);
  try {
    await ipc("task_apply_decomposition", { decompositionId: id });
    setDecompositionOpen(false);
    setDecompositionResult(null);
    refetch();
  } finally {
    setDecompositionApplying(false);
  }
}, [refetch]);

const rejectDecomposition = useCallback(async (id: string) => {
  await ipc("task_reject_decomposition", { decompositionId: id });
  setDecompositionOpen(false);
  setDecompositionResult(null);
}, []);
```

Add `DecompositionResult` type definition (matching the modal's interface) to mappers.ts or inline.

Add these to the return object: `decompose`, `decompositionResult`, `decompositionOpen`, `setDecompositionOpen`, `applyDecomposition`, `rejectDecomposition`, `decompositionApplying`.

- [ ] **Step 2: Add "Break Down" button and modal to IssueContentTab**

In `IssueContentTab.tsx`, near the sub-issues section (around line 77), add:

```tsx
import { Bot } from "lucide-react";
import { DecompositionModal } from "./DecompositionModal";

// In the sub-issues section header area:
<div className="flex items-center justify-between">
  <h3>Sub-issues ({completedCount}/{issues.length} done)</h3>
  <button
    type="button"
    onClick={detail.decompose}
    className="flex items-center gap-1 text-xs text-purple-300 hover:text-purple-200 transition-colors"
  >
    <Bot className="size-3" />
    Break Down
  </button>
</div>

// After the sub-issues list:
<DecompositionModal
  result={detail.decompositionResult}
  open={detail.decompositionOpen}
  onOpenChange={detail.setDecompositionOpen}
  onApply={detail.applyDecomposition}
  onReject={detail.rejectDecomposition}
  applying={detail.decompositionApplying}
/>
```

- [ ] **Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks/hooks/useIssueDetail.ts desktop-ui/src/features/tasks/components/detail/IssueContentTab.tsx desktop-ui/src/features/tasks/lib/mappers.ts
git commit -m "feat(tasks-ui): wire decomposition with Break Down button and modal"
```

---

## Chunk 3: Complexity Score & Estimation Forecast (Feature 3)

### Task 12: Add complexity score to frontend

**Files:**
- Modify: `desktop-ui/src/features/tasks/lib/mappers.ts` (DetailTask interface + mapper)
- Modify: `desktop-ui/src/features/tasks/components/detail/SidebarProperties.tsx`

- [ ] **Step 1: Add complexityScore to DetailTask interface**

In `mappers.ts`, find the `DetailTask` interface (line 29) and add after `acceptanceCriteria`:

```typescript
complexityScore: number | null;
```

- [ ] **Step 2: Map it in taskToDetailTask**

In the `taskToDetailTask` function (line ~280), add:

```typescript
complexityScore: task.complexityScore ?? null,
```

Verify `Task` type in `desktop-ui/src/shared/types/tasks.ts` has `complexityScore` field. If not, add it there too.

- [ ] **Step 3: Add complexity row to SidebarProperties**

In `SidebarProperties.tsx`, after the energy level row (around line 212), add:

```tsx
{detail.task.complexityScore != null && (
  <PropertyRow label="Complexity">
    <ComplexityBadge score={detail.task.complexityScore} />
  </PropertyRow>
)}
```

Add the `ComplexityBadge` component:

```tsx
function ComplexityBadge({ score }: { score: number }) {
  const { label, color } =
    score <= 30
      ? { label: "Low", color: "text-green-400 bg-green-500/20" }
      : score <= 60
        ? { label: "Medium", color: "text-yellow-400 bg-yellow-500/20" }
        : score <= 80
          ? { label: "High", color: "text-orange-400 bg-orange-500/20" }
          : { label: "Very High", color: "text-red-400 bg-red-500/20" };

  return (
    <span className={`text-xs px-1.5 py-0.5 rounded ${color}`}>
      {label} ({score})
    </span>
  );
}
```

- [ ] **Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks/lib/mappers.ts desktop-ui/src/features/tasks/components/detail/SidebarProperties.tsx desktop-ui/src/shared/types/tasks.ts
git commit -m "feat(tasks-ui): display complexity score badge in sidebar properties"
```

### Task 13: Add forecast response types and Tauri command

**Files:**
- Modify: `crates/desktop-shared/src/commands/tasks.rs`
- Create: `crates/app-core/src/handlers/tasks/forecast.rs`
- Modify: `crates/app-core/src/handlers/tasks/mod.rs`
- Modify: `crates/desktop/src/commands/tasks.rs`

- [ ] **Step 1: Add TaskForecastResponse types to desktop-shared**

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ForecastRiskResponse {
    pub kind: String,
    pub description: String,
    pub impact_minutes: Option<i32>,
}
```

- [ ] **Step 2: Create forecast.rs app-core handler**

Read `crates/feature-tasks/src/handlers/forecast.rs` for the `ForecastHandler` trait, and `crates/feature-tasks/src/types/planning.rs` for `TaskForecast`, `ForecastMethodology`, `DataQuality`, `RiskKind` types. Then create:

```rust
use crate::ApiError;
use desktop_shared::commands::tasks::{ForecastRiskResponse, TaskForecastResponse};

use crate::AppCore;

impl AppCore {
    pub async fn task_forecast(
        &self,
        task_id: String,
    ) -> Result<TaskForecastResponse, ApiError> {
        let task_row = self
            .repos
            .tasks
            .get(&task_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("Task {task_id} not found")))?;

        if task_row.estimated_minutes.is_none() {
            return Err(ApiError::new("VALIDATION", "Task has no estimate"));
        }

        let task = feature_tasks::types::Task::from(task_row);

        let context = feature_tasks::types::planning::ForecastContext {
            min_sample_size: 5,
            lookback_days: 90,
            include_subtasks: false,
        };

        let handler = self.forecast_handler()?;
        let forecast = handler
            .forecast_task(&task, &context)
            .await
            .map_err(|e| ApiError::new("INTERNAL", e.to_string()))?;

        Ok(TaskForecastResponse {
            estimated_minutes: forecast.estimated_minutes,
            confidence_low: forecast.confidence_low,
            confidence_high: forecast.confidence_high,
            methodology: forecast.methodology.name,
            sample_size: forecast.methodology.sample_size,
            data_quality: format!("{:?}", forecast.data_quality).to_lowercase(),
            risks: forecast
                .risks
                .iter()
                .map(|r| ForecastRiskResponse {
                    kind: format!("{:?}", r.kind).to_lowercase(),
                    description: r.description.clone(),
                    impact_minutes: r.impact_minutes,
                })
                .collect(),
        })
    }
}
```

Add `mod forecast;` to `mod.rs`.

- [ ] **Step 3: Add task_forecast Tauri command**

```rust
#[tauri::command]
pub async fn task_forecast(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<TaskForecastResponse, ApiError> {
    state.task_forecast(task_id).await
}
```

Add `"task_forecast"` to DEV_COMMANDS. Add dispatch_dev match arm:

```rust
"task_forecast" => dev::val(
    core.task_forecast(try_field!(dev::get_str(body, "taskId")).into())
        .await
),
```

In `crates/desktop/src/main.rs` (inside `tauri::generate_handler![]`), add:

```rust
commands::tasks::task_forecast,
```

- [ ] **Step 4: Verify full Rust build**

Run: `cargo build -p desktop`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/src/commands/tasks.rs crates/app-core/src/handlers/tasks/forecast.rs crates/app-core/src/handlers/tasks/mod.rs crates/desktop/src/commands/tasks.rs
git commit -m "feat: add task forecast backend command"
```

### Task 14: Wire forecast in SidebarTime

**Files:**
- Modify: `desktop-ui/src/features/tasks/components/detail/SidebarTime.tsx`
- Modify: `desktop-ui/src/features/tasks/hooks/useIssueDetail.ts`

- [ ] **Step 1: Add forecast state to useIssueDetail**

```typescript
interface TaskForecast {
  estimatedMinutes: number;
  confidenceLow: number;
  confidenceHigh: number;
  methodology: string;
  sampleSize: number;
  dataQuality: string;
  risks: Array<{ kind: string; description: string; impactMinutes: number | null }>;
}

const [forecast, setForecast] = useState<TaskForecast | null>(null);
const [forecastLoading, setForecastLoading] = useState(false);

const fetchForecast = useCallback(async () => {
  setForecastLoading(true);
  try {
    const result = await ipc<TaskForecast>("task_forecast", { taskId: task.id });
    setForecast(result);
  } catch {
    // Task may have no estimate
    setForecast(null);
  } finally {
    setForecastLoading(false);
  }
}, [task.id]);
```

Add `forecast`, `forecastLoading`, `fetchForecast` to the return object.

- [ ] **Step 2: Replace hardcoded forecast in SidebarTime**

In `SidebarTime.tsx`, remove the hardcoded `forecastSecs` calculation (line 14) and replace the forecast display section (lines 71-74) with an on-demand forecast:

```tsx
{detail.task.estimatedMinutes != null && (
  <div className="space-y-1.5">
    {detail.forecast ? (
      <>
        <div className="flex items-center gap-2 text-xs">
          <span className="text-[hsl(var(--muted-foreground))]">AI Forecast:</span>
          <span className="text-[hsl(var(--foreground))]">
            {detail.forecast.confidenceLow}m — {detail.forecast.estimatedMinutes}m — {detail.forecast.confidenceHigh}m
          </span>
        </div>
        <div className="text-[10px] text-[hsl(var(--muted-foreground))]">
          Based on {detail.forecast.sampleSize} similar tasks ({detail.forecast.dataQuality} quality)
        </div>
        {detail.forecast.risks.map((r) => (
          <div key={r.kind} className="flex items-start gap-1 text-[10px] text-yellow-400/80">
            <span className="shrink-0">!</span>
            {r.description}
          </div>
        ))}
      </>
    ) : (
      <button
        type="button"
        onClick={detail.fetchForecast}
        disabled={detail.forecastLoading}
        className="text-xs text-purple-300 hover:text-purple-200 transition-colors disabled:opacity-50"
      >
        {detail.forecastLoading ? "Forecasting..." : "Get AI Forecast"}
      </button>
    )}
  </div>
)}
```

- [ ] **Step 3: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks/components/detail/SidebarTime.tsx desktop-ui/src/features/tasks/hooks/useIssueDetail.ts
git commit -m "feat(tasks-ui): wire on-demand forecast in SidebarTime"
```

---

## Chunk 4: "Why This Task Now" (Feature 4)

### Task 15: Replace hardcoded WhyThisTaskNow with real data

**Files:**
- Modify: `desktop-ui/src/features/tasks/components/detail/SidebarAiInsights.tsx` (lines 126-152)

- [ ] **Step 1: Create computeReasons utility**

Replace the hardcoded `WhyThisTaskNow` component (lines 126-152) with:

```tsx
interface TaskReason {
  icon: typeof Zap;
  text: string;
  weight: number;
}

function computeReasons(task: DetailTask): TaskReason[] {
  const reasons: TaskReason[] = [];
  const now = new Date();

  // Priority
  if (task.priority?.id === "urgent") {
    reasons.push({ icon: Zap, text: "P1 — highest priority", weight: 100 });
  } else if (task.priority?.id === "high") {
    reasons.push({ icon: Zap, text: "P2 — high priority", weight: 80 });
  }

  // Due date
  if (task.dueDate) {
    const due = new Date(task.dueDate);
    const diffDays = Math.floor((due.getTime() - now.getTime()) / (1000 * 60 * 60 * 24));
    if (diffDays < 0) {
      reasons.push({ icon: Zap, text: `Overdue by ${Math.abs(diffDays)} day${Math.abs(diffDays) !== 1 ? "s" : ""}`, weight: 95 });
    } else if (diffDays === 0) {
      reasons.push({ icon: Zap, text: "Due today", weight: 90 });
    } else if (diffDays <= 3) {
      reasons.push({ icon: Zap, text: `Due in ${diffDays} day${diffDays !== 1 ? "s" : ""}`, weight: 70 });
    }
  }

  // Focus momentum
  if (task.focusedAt) {
    reasons.push({ icon: Zap, text: "You're already in flow", weight: 85 });
  }

  // Energy match
  if (task.energyLevel) {
    const hour = now.getHours();
    const currentEnergy = hour >= 6 && hour < 12 ? "high" : hour >= 12 && hour < 17 ? "medium" : "low";
    if (task.energyLevel === currentEnergy) {
      reasons.push({ icon: Zap, text: "Matches your current energy window", weight: 60 });
    }
  }

  // Quick win
  if (task.complexityScore != null && task.complexityScore <= 30) {
    reasons.push({ icon: Zap, text: "Quick win — low complexity", weight: 50 });
  }

  return reasons.sort((a, b) => b.weight - a.weight).slice(0, 3);
}

function WhyThisTaskNow({ task }: { task: DetailTask }) {
  const reasons = computeReasons(task);

  if (reasons.length === 0) return null;

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-[hsl(var(--foreground))]">
        <Bot className="size-3.5 text-purple-400" />
        Why This Task Now?
      </div>
      <div className="space-y-1.5">
        {reasons.map((r) => (
          <div
            key={r.text}
            className="flex items-center gap-2 text-xs text-[hsl(var(--muted-foreground))]"
          >
            <r.icon className="size-3 text-purple-400/60 shrink-0" />
            {r.text}
          </div>
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Update the SidebarAiInsights conditional to pass task**

Change the `WhyThisTaskNow` rendering (around line 29) from:

```tsx
<WhyThisTaskNow />
```

to:

```tsx
<WhyThisTaskNow task={detail.task} />
```

This requires passing `detail.task` (as `DetailTask`) through the props. Update the `SidebarAiInsightsProps` interface to accept `task: DetailTask` and update callers.

- [ ] **Step 3: Add DetailTask import**

Import `DetailTask` from the mappers module.

- [ ] **Step 4: Verify frontend builds**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 5: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/tasks/components/detail/SidebarAiInsights.tsx
git commit -m "feat(tasks-ui): replace hardcoded WhyThisTaskNow with real data-driven reasons"
```

### Task 16: Final verification

- [ ] **Step 1: Full Rust build**

Run: `cargo build --workspace`
Expected: success

- [ ] **Step 2: Rust clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Rust format check**

Run: `cargo fmt --all --check`
Expected: success

- [ ] **Step 4: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: success

- [ ] **Step 5: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: no errors

- [ ] **Step 6: Run Rust tests**

Run: `cargo nextest run --workspace`
Expected: all pass (existing tests should not break)

- [ ] **Step 7: Run frontend tests**

Run: `cd desktop-ui && bun run test`
Expected: all pass

- [ ] **Step 8: Final commit if any lint fixes**

```bash
git add -A
git commit -m "chore: lint and format fixes"
```
