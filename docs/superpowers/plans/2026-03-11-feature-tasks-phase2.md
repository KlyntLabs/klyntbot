# Feature-Tasks Phase 2: Agentic Intelligence Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement three LLM-powered handler traits (DecompositionHandler, TaskExecutionHandler, DayPlanningHandler) in the agent crate (L5), with prompt templates, validation logic, repo extensions, and builder wiring.

**Architecture:** Traits are already defined in `crates/feature-tasks/src/handlers/` (L4). This plan implements them in `crates/agent/src/` (L5) using dependency inversion. Each handler wraps a `DynProvider` for LLM calls, a `TaskRepo` for persistence, and an optional `DomainEventBus` for events. The `TaskTool` gets new `with_*_handler()` methods, and `AgentLoopBuilder` constructs and injects them.

**Tech Stack:** Rust, async-trait, serde_json (structured LLM output), tokio (async), sqlx (SQLite), `providers::DynProvider` (LLM), `bus::DomainEventBus`

**Spec:** `docs/superpowers/specs/2026-03-11-feature-tasks-phase2-3-design.md` (sections 2.1–2.3)

---

## File Structure

### Files to create:
| File | Responsibility |
|------|---------------|
| `crates/agent/src/handlers/decomposition.rs` | `DecompositionHandler` impl — LLM call, validation, repair, confidence gating |
| `crates/agent/src/handlers/execution.rs` | `TaskExecutionHandler` impl — approval gate, execution lifecycle, retry |
| `crates/agent/src/handlers/planning.rs` | `DayPlanningHandler` impl — LLM day planning, energy matching, replanning |
| `crates/agent/src/handlers/mod.rs` | Re-export module for Phase 2 handlers |
| `crates/agent/src/templates/decomposition_prompt.md` | Prompt template for decomposition |
| `crates/agent/src/templates/execution_prompt.md` | Prompt template for task execution |
| `crates/agent/src/templates/day_plan_prompt.md` | Prompt template for day planning |

### Files to modify:
| File | Changes |
|------|---------|
| `crates/storage/src/repos/task_repo.rs` | Add `get_execution`, decomposition CRUD, `expire_suggestions` |
| `crates/feature-tasks/src/tool/mod.rs` | Add `with_decomposition_handler`, `with_execution_handler`, `with_planning_handler` fields + builder methods |
| `crates/feature-tasks/src/tool/actions/plan.rs` | Wire `DayPlanningHandler` into `handle_plan_day` (LLM path + scoring fallback) |
| `crates/agent/src/agent_loop/builder.rs` | Construct and inject Phase 2 handlers |
| `crates/agent/src/lib.rs` or `crates/agent/src/mod.rs` | Declare `handlers` module |

---

## Chunk 1: Storage Extensions & Decomposition Validation

### Task 1: Add missing TaskRepo methods

**Files:**
- Modify: `crates/storage/src/repos/task_repo.rs`

- [ ] **Step 1: Write test for `get_execution`**

Add to the existing test module in `task_repo.rs`:

```rust
#[tokio::test]
async fn test_get_execution() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_all_migrations(pool.inner()).await;
    let repo = TaskRepo::new(pool.inner().clone());

    // Create a task first
    let task = test_task("test-exec-get");
    repo.add(&task).await.unwrap();

    let exec_row = TaskExecutionRow {
        id: "exec-1".to_string(),
        task_id: task.id.clone(),
        status: "pending".to_string(),
        agent_profile: Some("task".to_string()),
        started_at: None,
        completed_at: None,
        duration_secs: None,
        tokens_used: None,
        cost_usd: None,
        input_context: None,
        output_summary: None,
        error_message: None,
        artifacts: None,
        metrics: None,
        retry_count: 0,
        created_at: Utc::now(),
    };
    repo.create_execution(&exec_row).await.unwrap();

    let fetched = repo.get_execution("exec-1").await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().status, "pending");

    let missing = repo.get_execution("nonexistent").await.unwrap();
    assert!(missing.is_none());
}
```

- [ ] **Step 2: Implement `get_execution`**

Add to `TaskRepo` impl block (near line ~1193, after `list_executions`):

```rust
/// Get a single execution by ID.
pub async fn get_execution(&self, id: &str) -> Result<Option<TaskExecutionRow>, StorageError> {
    let row = sqlx::query_as::<_, TaskExecutionRow>(
        "SELECT * FROM task_executions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 3: Run test**

```bash
cargo nextest run -p storage -E 'test(test_get_execution)'
```

- [ ] **Step 4: Write test for decomposition CRUD**

```rust
#[tokio::test]
async fn test_decomposition_crud() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_all_migrations(pool.inner()).await;
    let repo = TaskRepo::new(pool.inner().clone());

    let task = test_task("test-decomp");
    repo.add(&task).await.unwrap();

    let row = TaskDecompositionRow {
        id: "decomp-1".to_string(),
        task_id: task.id.clone(),
        plan: r#"{"subtasks":[]}"#.to_string(),
        confidence: 0.85,
        status: "pending".to_string(),
        reasoning: Some("test".to_string()),
        created_at: Utc::now(),
        applied_at: None,
    };
    let created = repo.create_decomposition(&row).await.unwrap();
    assert_eq!(created.id, "decomp-1");

    let fetched = repo.get_decomposition("decomp-1").await.unwrap();
    assert!(fetched.is_some());

    let pending = repo.list_pending_decompositions(&task.id).await.unwrap();
    assert_eq!(pending.len(), 1);

    repo.apply_decomposition("decomp-1").await.unwrap();
    let applied = repo.get_decomposition("decomp-1").await.unwrap().unwrap();
    assert_eq!(applied.status, "applied");

    let pending_after = repo.list_pending_decompositions(&task.id).await.unwrap();
    assert!(pending_after.is_empty());
}
```

- [ ] **Step 5: Implement decomposition CRUD**

```rust
/// Create a decomposition plan record.
pub async fn create_decomposition(
    &self,
    row: &TaskDecompositionRow,
) -> Result<TaskDecompositionRow, StorageError> {
    let inserted = sqlx::query_as::<_, TaskDecompositionRow>(
        r#"
        INSERT INTO task_decompositions (id, task_id, plan, confidence, status, reasoning)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        RETURNING *
        "#,
    )
    .bind(&row.id)
    .bind(&row.task_id)
    .bind(&row.plan)
    .bind(row.confidence)
    .bind(&row.status)
    .bind(&row.reasoning)
    .fetch_one(&self.pool)
    .await?;
    Ok(inserted)
}

/// Get a decomposition by ID.
pub async fn get_decomposition(&self, id: &str) -> Result<Option<TaskDecompositionRow>, StorageError> {
    let row = sqlx::query_as::<_, TaskDecompositionRow>(
        "SELECT * FROM task_decompositions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&self.pool)
    .await?;
    Ok(row)
}

/// List pending decompositions for a task.
pub async fn list_pending_decompositions(
    &self,
    task_id: &str,
) -> Result<Vec<TaskDecompositionRow>, StorageError> {
    let rows = sqlx::query_as::<_, TaskDecompositionRow>(
        "SELECT * FROM task_decompositions WHERE task_id = ?1 AND status = 'pending' ORDER BY created_at DESC",
    )
    .bind(task_id)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}

/// Mark a decomposition as applied.
pub async fn apply_decomposition(&self, id: &str) -> Result<bool, StorageError> {
    let result = sqlx::query(
        "UPDATE task_decompositions SET status = 'applied', applied_at = datetime('now') WHERE id = ?1",
    )
    .bind(id)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p storage -E 'test(test_decomposition_crud)'
```

- [ ] **Step 7: Write test for `expire_suggestions_for_task`**

```rust
#[tokio::test]
async fn test_expire_suggestions() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_all_migrations(pool.inner()).await;
    let repo = TaskRepo::new(pool.inner().clone());

    let task = test_task("test-expire-sugg");
    repo.add(&task).await.unwrap();

    let sugg = TaskSuggestionRow {
        id: "sugg-1".to_string(),
        task_id: Some(task.id.clone()),
        suggestion_type: "Decompose".to_string(),
        title: "Break down".to_string(),
        description: None,
        confidence: 0.8,
        action_payload: None,
        status: "pending".to_string(),
        trigger: None,
        created_at: Utc::now(),
        resolved_at: None,
    };
    repo.create_suggestion(&sugg).await.unwrap();

    let expired = repo.expire_suggestions_for_task(&task.id).await.unwrap();
    assert_eq!(expired, 1);

    let pending = repo.list_pending_suggestions(Some(&task.id)).await.unwrap();
    assert!(pending.is_empty());
}
```

- [ ] **Step 8: Implement `expire_suggestions_for_task`**

```rust
/// Expire all pending suggestions for a task.
pub async fn expire_suggestions_for_task(&self, task_id: &str) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "UPDATE task_suggestions SET status = 'expired', resolved_at = datetime('now') WHERE task_id = ?1 AND status = 'pending'",
    )
    .bind(task_id)
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected())
}
```

- [ ] **Step 9: Run all new tests**

```bash
cargo nextest run -p storage -E 'test(test_get_execution) | test(test_decomposition_crud) | test(test_expire_suggestions)'
```

- [ ] **Step 10: Commit**

```bash
git add crates/storage/src/repos/task_repo.rs
git commit -m "feat(storage): add get_execution, decomposition CRUD, expire_suggestions"
```

---

### Task 2: Add handler fields to TaskTool

**Files:**
- Modify: `crates/feature-tasks/src/tool/mod.rs`

- [ ] **Step 1: Add handler imports and fields**

In the imports section (line ~12), add:

```rust
use crate::handlers::{DecompositionHandler, TaskExecutionHandler, DayPlanningHandler};
```

In `TaskTool` struct (after `config` field, line ~37):

```rust
    /// Optional decomposition handler (LLM-powered subtask generation).
    pub(crate) decomposition_handler: Option<Arc<dyn DecompositionHandler>>,
    /// Optional execution handler (agentic task execution).
    pub(crate) execution_handler: Option<Arc<dyn TaskExecutionHandler>>,
    /// Optional day planning handler (LLM-powered daily planning).
    pub(crate) planning_handler: Option<Arc<dyn DayPlanningHandler>>,
```

In `TaskTool::new()` (add to the `Self { ... }` block):

```rust
            decomposition_handler: None,
            execution_handler: None,
            planning_handler: None,
```

- [ ] **Step 2: Add builder methods**

After `with_config()` method:

```rust
    /// Attach a decomposition handler for AI-powered subtask generation.
    pub fn with_decomposition_handler(mut self, handler: Arc<dyn DecompositionHandler>) -> Self {
        self.decomposition_handler = Some(handler);
        self
    }

    /// Attach an execution handler for agentic task execution.
    pub fn with_execution_handler(mut self, handler: Arc<dyn TaskExecutionHandler>) -> Self {
        self.execution_handler = Some(handler);
        self
    }

    /// Attach a day planning handler for LLM-powered daily planning.
    pub fn with_planning_handler(mut self, handler: Arc<dyn DayPlanningHandler>) -> Self {
        self.planning_handler = Some(handler);
        self
    }
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p feature-tasks
```

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/tool/mod.rs
git commit -m "feat(tasks): add decomposition/execution/planning handler fields to TaskTool"
```

---

## Chunk 2: DecompositionHandler Implementation

### Task 3: Create decomposition prompt template

**Files:**
- Create: `crates/agent/src/templates/decomposition_prompt.md`

- [ ] **Step 1: Write the prompt template**

```markdown
You are a task decomposition assistant. Break the given task into actionable subtasks.

## Task
Title: {{title}}
Description: {{description}}
Acceptance Criteria: {{acceptance_criteria}}
Estimated Minutes: {{estimated_minutes}}
Energy Level: {{energy_level}}
Priority: {{priority}}

## Context
Project: {{project_context}}
Existing Subtasks: {{existing_subtasks}}
{{#cognitive_facts}}
Relevant Knowledge:
{{cognitive_facts}}
{{/cognitive_facts}}

## Constraints
- Maximum depth: {{max_depth}} levels
- Maximum subtasks per level: {{max_subtasks_per_level}}
- Each subtask should be independently completable
- Assign energy levels based on cognitive demand
- Estimate minutes realistically (max 240 per subtask)
- Use temp_id format "sub-N" for inter-subtask dependency references

## Output
Return ONLY valid JSON in this exact format:
{
  "confidence": 0.85,
  "reasoning": "Brief explanation of decomposition strategy",
  "subtasks": [
    {
      "temp_id": "sub-1",
      "title": "Subtask title",
      "description": "Optional description",
      "acceptance_criteria": "Optional criteria",
      "estimated_minutes": 30,
      "energy_level": "medium",
      "priority": 2,
      "task_type": "manual",
      "dependencies": [],
      "children": []
    }
  ],
  "total_estimated_mins": 120
}

Valid energy_level values: low, medium, high, deep
Valid task_type values: manual, agentic, hybrid
Dependencies reference sibling temp_ids (e.g., ["sub-1"]).
```

- [ ] **Step 2: Commit**

```bash
mkdir -p crates/agent/src/templates
git add crates/agent/src/templates/decomposition_prompt.md
git commit -m "feat(agent): add decomposition prompt template"
```

---

### Task 4: Implement DecompositionHandler

**Files:**
- Create: `crates/agent/src/handlers/decomposition.rs`
- Create: `crates/agent/src/handlers/mod.rs`

- [ ] **Step 1: Create the handlers module file**

`crates/agent/src/handlers/mod.rs`:

```rust
//! Phase 2 handler implementations (L5).

mod decomposition;
mod execution;
mod planning;

pub use decomposition::LlmDecompositionHandler;
pub use execution::LlmTaskExecutionHandler;
pub use planning::LlmDayPlanningHandler;
```

Note: `execution` and `planning` modules will be created in later tasks. For now, comment them out or create empty files.

- [ ] **Step 2: Write the validation module inline and handler struct**

`crates/agent/src/handlers/decomposition.rs`:

```rust
//! LLM-backed DecompositionHandler implementation.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use bus::DomainEvent;
use common::Result;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use storage::TaskRepo;
use tracing::{debug, warn};

use feature_tasks::handlers::DecompositionHandler;
use feature_tasks::types::{
    DecompositionContext, DecompositionResult, DecompositionTree,
    PlannedSubtask, Task, ValidationWarning, ValidationWarningKind,
};

const DECOMPOSITION_PROMPT: &str = include_str!("../templates/decomposition_prompt.md");

/// LLM-powered decomposition handler.
pub struct LlmDecompositionHandler {
    provider: DynProvider,
    model: String,
    repo: TaskRepo,
    domain_bus: Option<Arc<bus::DomainEventBus>>,
}

impl LlmDecompositionHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        repo: TaskRepo,
        domain_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self { provider, model, repo, domain_bus }
    }
}

/// LLM response structure for decomposition.
#[derive(serde::Deserialize)]
struct LlmDecompositionResponse {
    confidence: f64,
    reasoning: String,
    subtasks: Vec<LlmPlannedSubtask>,
    total_estimated_mins: Option<i32>,
}

#[derive(serde::Deserialize)]
struct LlmPlannedSubtask {
    temp_id: String,
    title: String,
    description: Option<String>,
    acceptance_criteria: Option<String>,
    estimated_minutes: Option<i32>,
    energy_level: Option<String>,
    priority: Option<i16>,
    task_type: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    children: Vec<LlmPlannedSubtask>,
}

#[async_trait]
impl DecompositionHandler for LlmDecompositionHandler {
    async fn decompose(
        &self,
        task: &Task,
        context: &DecompositionContext,
    ) -> Result<DecompositionResult> {
        let prompt = build_prompt(task, context);

        let params = ChatParams::new(&self.model)
            .with_temperature(0.2)
            .with_max_tokens(4096)
            .with_response_format(ResponseFormat::JsonObject);

        let messages = vec![
            Message::system("You are a task decomposition assistant. Return only valid JSON."),
            Message::user(prompt),
        ];

        let response = self.provider.chat(&messages, None, &params).await?;
        let content = response.content.unwrap_or_default();
        let json_str = common::utils::strip_llm_fences(&content);

        let parsed: LlmDecompositionResponse = serde_json::from_str(json_str)
            .map_err(|e| common::KlyntbotError::Internal(format!("Decomposition JSON parse failed: {e}")))?;

        let tree = convert_tree(&parsed);
        let mut warnings = Vec::new();
        let mut confidence = parsed.confidence.clamp(0.0, 1.0);

        // Validation & repair
        validate_and_repair(&mut tree_to_flat(&tree), context, &mut warnings, &mut confidence);

        // Floor confidence at 0.3
        confidence = confidence.max(0.3);

        debug!(
            task_id = %task.id,
            confidence,
            subtask_count = tree.subtasks.len(),
            "Decomposition complete"
        );

        Ok(DecompositionResult {
            tree,
            confidence,
            reasoning: parsed.reasoning,
            validation_warnings: warnings,
        })
    }
}

fn build_prompt(task: &Task, ctx: &DecompositionContext) -> String {
    let mut prompt = DECOMPOSITION_PROMPT.to_string();
    prompt = prompt.replace("{{title}}", &task.title);
    prompt = prompt.replace("{{description}}", task.description.as_deref().unwrap_or("(none)"));
    prompt = prompt.replace("{{acceptance_criteria}}", task.acceptance_criteria.as_deref().unwrap_or("(none)"));
    prompt = prompt.replace(
        "{{estimated_minutes}}",
        &task.estimated_minutes.map(|m| m.to_string()).unwrap_or_else(|| "(none)".to_string()),
    );
    prompt = prompt.replace(
        "{{energy_level}}",
        task.energy_level.as_ref().map(|e| e.to_string()).as_deref().unwrap_or("(none)"),
    );
    prompt = prompt.replace(
        "{{priority}}",
        &task.priority.map(|p| format!("P{p}")).unwrap_or_else(|| "(none)".to_string()),
    );
    prompt = prompt.replace("{{project_context}}", ctx.project_context.as_deref().unwrap_or("(none)"));
    prompt = prompt.replace(
        "{{existing_subtasks}}",
        &if ctx.existing_subtasks.is_empty() {
            "(none)".to_string()
        } else {
            ctx.existing_subtasks.join(", ")
        },
    );
    prompt = prompt.replace(
        "{{cognitive_facts}}",
        &if ctx.cognitive_facts.is_empty() {
            String::new()
        } else {
            ctx.cognitive_facts.join("\n")
        },
    );
    prompt = prompt.replace("{{max_depth}}", &ctx.max_depth.to_string());
    prompt = prompt.replace("{{max_subtasks_per_level}}", &ctx.max_subtasks_per_level.to_string());
    prompt
}

fn convert_subtask(llm: &LlmPlannedSubtask) -> PlannedSubtask {
    PlannedSubtask {
        temp_id: llm.temp_id.clone(),
        title: llm.title.clone(),
        description: llm.description.clone(),
        acceptance_criteria: llm.acceptance_criteria.clone(),
        estimated_minutes: llm.estimated_minutes,
        energy_level: llm.energy_level.as_deref().and_then(|s| s.parse().ok()),
        priority: llm.priority,
        task_type: llm.task_type.as_deref().and_then(|s| s.parse().ok()),
        dependencies: llm.dependencies.clone(),
        children: llm.children.iter().map(convert_subtask).collect(),
    }
}

fn convert_tree(llm: &LlmDecompositionResponse) -> DecompositionTree {
    DecompositionTree {
        subtasks: llm.subtasks.iter().map(convert_subtask).collect(),
        total_estimated_mins: llm.total_estimated_mins,
    }
}

/// Flatten tree into list of (temp_id, dependencies) for validation.
fn tree_to_flat(tree: &DecompositionTree) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    fn walk(subtask: &PlannedSubtask, out: &mut Vec<(String, Vec<String>)>) {
        out.push((subtask.temp_id.clone(), subtask.dependencies.clone()));
        for child in &subtask.children {
            walk(child, out);
        }
    }
    for s in &tree.subtasks {
        walk(s, &mut out);
    }
    out
}

fn validate_and_repair(
    flat: &mut Vec<(String, Vec<String>)>,
    ctx: &DecompositionContext,
    warnings: &mut Vec<ValidationWarning>,
    confidence: &mut f64,
) {
    let valid_ids: HashSet<String> = flat.iter().map(|(id, _)| id.clone()).collect();

    // Check circular dependencies via simple cycle detection
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for (id, deps) in flat.iter() {
        graph.insert(id.as_str(), deps.iter().map(|d| d.as_str()).collect());
    }

    // Detect back-edges (simplified: check if any dep references a node that depends on us)
    for (id, deps) in flat.iter_mut() {
        deps.retain(|dep| {
            if !valid_ids.contains(dep) {
                return false; // Invalid reference, remove silently
            }
            // Check if dep→...→id forms a cycle (simplified: direct check)
            if let Some(dep_deps) = graph.get(dep.as_str()) {
                if dep_deps.contains(&id.as_str()) {
                    warnings.push(ValidationWarning {
                        kind: ValidationWarningKind::CircularDependency,
                        message: format!("Circular dependency between {id} and {dep}"),
                        subtask_temp_id: Some(id.clone()),
                    });
                    *confidence -= 0.15;
                    return false; // Remove back-edge
                }
            }
            true
        });
    }

    // Check duplicate titles
    let mut title_counts: HashMap<String, u32> = HashMap::new();
    for (id, _) in flat.iter() {
        *title_counts.entry(id.clone()).or_default() += 1;
    }
    for (id, count) in &title_counts {
        if *count > 1 {
            warnings.push(ValidationWarning {
                kind: ValidationWarningKind::DuplicateTitle,
                message: format!("Duplicate temp_id: {id}"),
                subtask_temp_id: Some(id.clone()),
            });
            *confidence -= 0.05;
        }
    }

    // Check excessive count
    if flat.len() > ctx.max_subtasks_per_level as usize * 2 {
        warnings.push(ValidationWarning {
            kind: ValidationWarningKind::TooManySubtasks,
            message: format!("Too many subtasks: {} (limit ~{})", flat.len(), ctx.max_subtasks_per_level),
            subtask_temp_id: None,
        });
        *confidence -= 0.10;
    }
}
```

- [ ] **Step 3: Declare the handlers module in the agent crate**

Check `crates/agent/src/lib.rs` or `crates/agent/src/mod.rs` and add:

```rust
pub mod handlers;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo build -p agent
```

Fix any compilation issues. The key dependency is that `feature_tasks` types (DecompositionContext, etc.) must be importable. Check exact type paths and adjust imports.

- [ ] **Step 5: Write tests**

Add to `crates/agent/src/handlers/decomposition.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use feature_tasks::types::{DecompositionContext, EnergyProfile};

    #[test]
    fn test_build_prompt_includes_task_title() {
        let task = Task::default_instance();
        let ctx = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: vec![],
            project_context: None,
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };
        let prompt = build_prompt(&task, &ctx);
        assert!(prompt.contains(&task.title));
    }

    #[test]
    fn test_validate_detects_circular_deps() {
        let mut flat = vec![
            ("sub-1".to_string(), vec!["sub-2".to_string()]),
            ("sub-2".to_string(), vec!["sub-1".to_string()]),
        ];
        let ctx = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: vec![],
            project_context: None,
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };
        let mut warnings = vec![];
        let mut confidence = 0.9;
        validate_and_repair(&mut flat, &ctx, &mut warnings, &mut confidence);

        assert!(!warnings.is_empty());
        assert!(confidence < 0.9);
        assert!(warnings.iter().any(|w| matches!(w.kind, ValidationWarningKind::CircularDependency)));
    }

    #[test]
    fn test_convert_tree_from_llm_response() {
        let llm = LlmDecompositionResponse {
            confidence: 0.85,
            reasoning: "test".to_string(),
            subtasks: vec![LlmPlannedSubtask {
                temp_id: "sub-1".to_string(),
                title: "First subtask".to_string(),
                description: None,
                acceptance_criteria: None,
                estimated_minutes: Some(30),
                energy_level: Some("medium".to_string()),
                priority: Some(2),
                task_type: Some("manual".to_string()),
                dependencies: vec![],
                children: vec![],
            }],
            total_estimated_mins: Some(30),
        };
        let tree = convert_tree(&llm);
        assert_eq!(tree.subtasks.len(), 1);
        assert_eq!(tree.subtasks[0].title, "First subtask");
        assert_eq!(tree.subtasks[0].estimated_minutes, Some(30));
    }
}
```

- [ ] **Step 6: Run tests**

```bash
cargo nextest run -p agent -E 'test(decomposition)'
```

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/handlers/ crates/agent/src/lib.rs
git commit -m "feat(agent): implement LlmDecompositionHandler with validation"
```

---

## Chunk 3: TaskExecutionHandler Implementation

### Task 5: Create execution prompt template

**Files:**
- Create: `crates/agent/src/templates/execution_prompt.md`

- [ ] **Step 1: Write the prompt template**

```markdown
You are an autonomous task execution agent. Complete the assigned task using available tools.

## Task
Title: {{title}}
Description: {{description}}
Acceptance Criteria: {{acceptance_criteria}}

## Context
{{context_snapshot}}

## Instructions
1. Analyze the task requirements and acceptance criteria
2. Use available tools to complete the task
3. Track your progress and report at intervals
4. Stop if you exceed the budget ceiling
5. Produce a summary of what was accomplished

## Budget
Maximum cost: ${{max_cost_usd}}
Maximum iterations: {{max_iterations}}

Report progress every {{progress_interval_secs}} seconds.
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent/src/templates/execution_prompt.md
git commit -m "feat(agent): add execution prompt template"
```

---

### Task 6: Implement TaskExecutionHandler

**Files:**
- Create: `crates/agent/src/handlers/execution.rs`
- Modify: `crates/agent/src/handlers/mod.rs` (uncomment `mod execution`)

- [ ] **Step 1: Write the handler implementation**

`crates/agent/src/handlers/execution.rs`:

```rust
//! LLM-backed TaskExecutionHandler implementation.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use chrono::Utc;
use common::Result;
use storage::TaskRepo;
use tracing::{debug, info, warn};
use uuid::Uuid;

use feature_tasks::handlers::TaskExecutionHandler;
use feature_tasks::types::{
    ExecuteResult, ExecutionConfig, Task, TaskExecution, TaskType,
};

/// Cost budget tiers by complexity score.
fn budget_for_complexity(score: Option<i32>) -> f64 {
    match score.unwrap_or(0) {
        0..=2 => 0.25,
        3..=4 => 1.00,
        5..=6 => 3.00,
        _ => 5.00,
    }
}

/// LLM-powered task execution handler.
pub struct LlmTaskExecutionHandler {
    repo: TaskRepo,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl LlmTaskExecutionHandler {
    pub fn new(
        repo: TaskRepo,
        domain_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        Self { repo, domain_bus }
    }

    fn emit(&self, event: DomainEvent) {
        if let Some(bus) = &self.domain_bus {
            let _ = bus.publish(event);
        }
    }
}

#[async_trait]
impl TaskExecutionHandler for LlmTaskExecutionHandler {
    async fn execute(&self, task: &Task, config: &ExecutionConfig) -> Result<ExecuteResult> {
        // Validate task type
        if task.task_type == TaskType::Manual {
            return Err(common::KlyntbotError::Validation(
                "Cannot execute manual tasks. Set task_type to 'agentic' or 'hybrid'.".into(),
            ));
        }

        // Check execution state
        let allowed_states = ["idle", "failed"];
        if !allowed_states.contains(&task.execution_state.as_str()) {
            return Err(common::KlyntbotError::Validation(
                format!("Task execution_state must be 'idle' or 'failed', got '{}'", task.execution_state),
            ));
        }

        // Approval gate for hybrid tasks or explicit require_approval
        if task.task_type == TaskType::Hybrid || config.require_approval {
            let suggestion_id = Uuid::new_v4().to_string();
            let sugg_row = storage::TaskSuggestionRow {
                id: suggestion_id.clone(),
                task_id: Some(task.id.clone()),
                suggestion_type: "Execute".to_string(),
                title: format!("Approve execution of: {}", task.title),
                description: Some("This task requires approval before agent execution.".into()),
                confidence: 1.0,
                action_payload: Some(serde_json::to_string(config).unwrap_or_default()),
                status: "pending".to_string(),
                trigger: Some("execution_request".to_string()),
                created_at: Utc::now(),
                resolved_at: None,
            };
            self.repo.create_suggestion(&sugg_row).await?;

            // Update execution state
            let mut patch = storage::TaskPatch::new(&task.id);
            patch.execution_state = Some("awaiting_approval".to_string());
            self.repo.update(&patch).await?;

            info!(task_id = %task.id, "Execution awaiting approval");
            return Ok(ExecuteResult::AwaitingApproval { suggestion_id });
        }

        // Compute budget
        let max_cost = config.max_cost_usd.unwrap_or_else(|| budget_for_complexity(task.complexity_score));

        // Create execution record
        let execution_id = Uuid::new_v4().to_string();
        let exec_row = storage::TaskExecutionRow {
            id: execution_id.clone(),
            task_id: task.id.clone(),
            status: "pending".to_string(),
            agent_profile: config.agent_profile.clone(),
            started_at: Some(Utc::now()),
            completed_at: None,
            duration_secs: None,
            tokens_used: None,
            cost_usd: Some(0.0),
            input_context: task.context_snapshot.as_ref().map(|cs| serde_json::to_string(cs).unwrap_or_default()),
            output_summary: None,
            error_message: None,
            artifacts: None,
            metrics: None,
            retry_count: 0,
            created_at: Utc::now(),
        };
        self.repo.create_execution(&exec_row).await?;

        // Update task state to running
        let mut patch = storage::TaskPatch::new(&task.id);
        patch.execution_state = Some("running".to_string());
        patch.spawned_execution_id = Some(Some(execution_id.clone()));
        self.repo.update(&patch).await?;

        // Log activity
        self.repo.log_activity(
            &task.id, "execution_started", None, None, None, "agent",
            Some(&format!("Execution {} started (budget: ${:.2})", execution_id, max_cost)),
        ).await?;

        // Emit domain event
        self.emit(DomainEvent::TaskExecutionStarted {
            task_id: task.id.clone(),
            execution_id: execution_id.clone(),
            agent_profile: config.agent_profile.clone().unwrap_or_else(|| "task".to_string()),
        });

        debug!(task_id = %task.id, execution_id = %execution_id, "Execution started");

        // NOTE: Actual subagent spawning will be integrated when SpawnHandler
        // is wired into this handler. For now, we create the execution record
        // and update state — the spawn integration is a follow-up task.

        Ok(ExecuteResult::Started { execution_id })
    }

    async fn get_execution(&self, execution_id: &str) -> Result<TaskExecution> {
        let row = self.repo.get_execution(execution_id).await?
            .ok_or_else(|| common::KlyntbotError::NotFound(
                format!("Execution {execution_id} not found"),
            ))?;
        Ok(TaskExecution::from(row))
    }

    async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        let row = self.repo.get_execution(execution_id).await?
            .ok_or_else(|| common::KlyntbotError::NotFound(
                format!("Execution {execution_id} not found"),
            ))?;

        if row.status != "running" && row.status != "pending" {
            return Err(common::KlyntbotError::Validation(
                format!("Cannot cancel execution in '{}' state", row.status),
            ));
        }

        self.repo.update_execution(execution_id, "cancelled", None, None, None).await?;

        // Reset task execution state
        let mut patch = storage::TaskPatch::new(&row.task_id);
        patch.execution_state = Some("idle".to_string());
        patch.spawned_execution_id = Some(None);
        self.repo.update(&patch).await?;

        self.repo.log_activity(
            &row.task_id, "execution_cancelled", None, None, None, "user",
            Some(&format!("Execution {execution_id} cancelled")),
        ).await?;

        Ok(())
    }

    async fn retry_execution(&self, execution_id: &str) -> Result<ExecuteResult> {
        let row = self.repo.get_execution(execution_id).await?
            .ok_or_else(|| common::KlyntbotError::NotFound(
                format!("Execution {execution_id} not found"),
            ))?;

        if row.status != "failed" {
            return Err(common::KlyntbotError::Validation(
                format!("Can only retry failed executions, got '{}'", row.status),
            ));
        }

        // Get the task to re-execute
        let task_row = self.repo.get_or_err(&row.task_id).await?;
        let task = feature_tasks::types::Task::from(task_row);

        let config = ExecutionConfig {
            agent_profile: row.agent_profile.clone(),
            max_cost_usd: row.cost_usd,
            require_approval: false,
            ..Default::default()
        };

        self.execute(&task, &config).await
    }
}
```

- [ ] **Step 2: Ensure `TaskPatch` supports `spawned_execution_id` and `execution_state`**

Check `crates/storage/src/rows/task.rs` for `TaskPatch` struct. If `spawned_execution_id` or `execution_state` fields don't exist in `TaskPatch`, add them. The `TaskPatch` should have optional fields that map to the `tasks` table columns.

Grep for `TaskPatch`:

```bash
grep -n "spawned_execution_id\|execution_state" crates/storage/src/repos/task_repo.rs
```

If missing, add them to the `TaskPatch` struct and the `update()` query builder.

- [ ] **Step 3: Add `TaskExecution::from(TaskExecutionRow)` conversion**

Check if this `From` impl exists in `crates/feature-tasks/src/types.rs`. If not, add:

```rust
impl From<storage::TaskExecutionRow> for TaskExecution {
    fn from(row: storage::TaskExecutionRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            status: row.status.parse().unwrap_or(ExecutionStatus::Pending),
            agent_profile: row.agent_profile,
            started_at: row.started_at,
            completed_at: row.completed_at,
            duration_secs: row.duration_secs,
            tokens_used: row.tokens_used,
            cost_usd: row.cost_usd,
            input_context: row.input_context,
            output_summary: row.output_summary,
            error_message: row.error_message,
            artifacts: row.artifacts
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            metrics: row.metrics
                .and_then(|s| serde_json::from_str(&s).ok()),
            retry_count: row.retry_count,
            created_at: row.created_at,
        }
    }
}
```

- [ ] **Step 4: Update handlers/mod.rs**

Uncomment `mod execution;` and `pub use execution::LlmTaskExecutionHandler;`

- [ ] **Step 5: Verify compilation**

```bash
cargo build -p agent
```

- [ ] **Step 6: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_for_complexity() {
        assert_eq!(budget_for_complexity(Some(0)), 0.25);
        assert_eq!(budget_for_complexity(Some(2)), 0.25);
        assert_eq!(budget_for_complexity(Some(3)), 1.00);
        assert_eq!(budget_for_complexity(Some(5)), 3.00);
        assert_eq!(budget_for_complexity(Some(7)), 5.00);
        assert_eq!(budget_for_complexity(None), 0.25);
    }
}
```

- [ ] **Step 7: Run tests**

```bash
cargo nextest run -p agent -E 'test(budget_for_complexity)'
```

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/handlers/execution.rs crates/agent/src/handlers/mod.rs crates/agent/src/templates/execution_prompt.md
git commit -m "feat(agent): implement LlmTaskExecutionHandler with approval gate and budget tiers"
```

---

## Chunk 4: DayPlanningHandler Implementation

### Task 7: Create day planning prompt template

**Files:**
- Create: `crates/agent/src/templates/day_plan_prompt.md`

- [ ] **Step 1: Write the prompt template**

```markdown
You are a daily planning assistant. Create an optimal time-slotted work plan.

## Available Tasks (pre-scored, highest priority first)
{{scored_tasks}}

## Working Hours
Start: {{work_start}}
End: {{work_end}}
Lunch: {{lunch_start}} (30 min break)
Available minutes: {{available_mins}}

## Energy Profile
Peak hours: {{peak_hours}}
Low energy hours: {{low_hours}}
Avg focus duration: {{avg_focus_mins}} min

## Calendar Blocks (busy times)
{{calendar_blocks}}

## Locked Slots (already scheduled, do not change)
{{locked_slots}}

## Instructions
- Match task energy levels to time-of-day energy
- High/deep energy tasks → peak hours
- Low energy tasks → post-lunch
- Respect calendar blocks (no overlaps)
- Keep locked slots unchanged
- Each slot: task_id, title, estimated_minutes, start_time, energy_level
- If tasks exceed available time, defer the lowest-priority ones
- Total planned minutes should not exceed available_mins

## Output
Return ONLY valid JSON:
{
  "slots": [
    {
      "task_id": "abc-123",
      "title": "Task title",
      "estimated_minutes": 30,
      "start_time": "09:00",
      "energy_level": "high"
    }
  ],
  "deferred": [
    {
      "task_id": "def-456",
      "title": "Deferred task",
      "reason": "Insufficient time"
    }
  ],
  "total_work_mins": 240,
  "utilization": 0.85,
  "reasoning": "Brief explanation of planning decisions"
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent/src/templates/day_plan_prompt.md
git commit -m "feat(agent): add day planning prompt template"
```

---

### Task 8: Implement DayPlanningHandler

**Files:**
- Create: `crates/agent/src/handlers/planning.rs`

- [ ] **Step 1: Write the handler implementation**

`crates/agent/src/handlers/planning.rs`:

```rust
//! LLM-backed DayPlanningHandler implementation.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use chrono::Utc;
use common::Result;
use providers::{ChatParams, DynProvider, Message, ResponseFormat};
use tracing::{debug, warn};

use feature_tasks::handlers::DayPlanningHandler;
use feature_tasks::types::{
    DayPlan, DeferredTask, EnergyLevel, PlanSlot, PlanSlotStatus, PlanningContext,
};

const DAY_PLAN_PROMPT: &str = include_str!("../templates/day_plan_prompt.md");

/// LLM response structure for day planning.
#[derive(serde::Deserialize)]
struct LlmDayPlanResponse {
    slots: Vec<LlmPlanSlot>,
    #[serde(default)]
    deferred: Vec<LlmDeferredTask>,
    total_work_mins: Option<i32>,
    utilization: Option<f64>,
    reasoning: String,
}

#[derive(serde::Deserialize)]
struct LlmPlanSlot {
    task_id: String,
    title: String,
    estimated_minutes: i32,
    start_time: Option<String>,
    energy_level: Option<String>,
}

#[derive(serde::Deserialize)]
struct LlmDeferredTask {
    task_id: String,
    title: String,
    reason: String,
}

pub struct LlmDayPlanningHandler {
    provider: DynProvider,
    model: String,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl LlmDayPlanningHandler {
    pub fn new(
        provider: DynProvider,
        model: String,
        domain_bus: Option<Arc<DomainEventBus>>,
    ) -> Self {
        Self { provider, model, domain_bus }
    }

    fn emit(&self, event: DomainEvent) {
        if let Some(bus) = &self.domain_bus {
            let _ = bus.publish(event);
        }
    }
}

#[async_trait]
impl DayPlanningHandler for LlmDayPlanningHandler {
    async fn plan_day(&self, context: &PlanningContext) -> Result<DayPlan> {
        let prompt = build_planning_prompt(context, &[]);
        let plan = call_llm_for_plan(&self.provider, &self.model, &prompt).await?;

        self.emit(DomainEvent::DayPlanGenerated {
            task_count: plan.slots.len() as u32,
            total_estimated_mins: plan.total_work_mins,
        });

        debug!(slots = plan.slots.len(), "Day plan generated");
        Ok(plan)
    }

    async fn replan(
        &self,
        context: &PlanningContext,
        current_plan: &DayPlan,
        reason: &str,
    ) -> Result<DayPlan> {
        let prompt = build_planning_prompt(context, &current_plan.locked_slots);
        let prompt_with_reason = format!(
            "{}\n\n## Replanning Reason\n{}\n\nKeep locked slots unchanged. Re-optimize remaining time.",
            prompt, reason
        );
        let mut plan = call_llm_for_plan(&self.provider, &self.model, &prompt_with_reason).await?;

        // Preserve locked slots from the current plan
        plan.locked_slots = current_plan.locked_slots.clone();

        self.emit(DomainEvent::DayPlanGenerated {
            task_count: plan.slots.len() as u32,
            total_estimated_mins: plan.total_work_mins,
        });

        debug!(slots = plan.slots.len(), "Day plan re-generated (reason: {})", reason);
        Ok(plan)
    }
}

async fn call_llm_for_plan(
    provider: &DynProvider,
    model: &str,
    prompt: &str,
) -> Result<DayPlan> {
    let params = ChatParams::new(model)
        .with_temperature(0.3)
        .with_max_tokens(4096)
        .with_response_format(ResponseFormat::JsonObject);

    let messages = vec![
        Message::system("You are a daily planning assistant. Return only valid JSON."),
        Message::user(prompt.to_string()),
    ];

    let response = provider.chat(&messages, None, &params).await?;
    let content = response.content.unwrap_or_default();
    let json_str = common::utils::strip_llm_fences(&content);

    let parsed: LlmDayPlanResponse = serde_json::from_str(json_str)
        .map_err(|e| common::KlyntbotError::Internal(format!("Day plan JSON parse failed: {e}")))?;

    let available_mins = calculate_available_mins(&parsed);

    Ok(DayPlan {
        slots: parsed.slots.iter().map(|s| PlanSlot {
            task_id: s.task_id.clone(),
            title: s.title.clone(),
            estimated_minutes: s.estimated_minutes,
            energy_level: s.energy_level.as_deref().and_then(|e| e.parse().ok()),
            start_time: s.start_time.clone(),
            status: PlanSlotStatus::Pending,
        }).collect(),
        locked_slots: vec![],
        total_work_mins: parsed.total_work_mins.unwrap_or(0),
        available_mins,
        utilization: parsed.utilization.unwrap_or(0.0),
        reasoning: parsed.reasoning,
        deferred: parsed.deferred.iter().map(|d| DeferredTask {
            task_id: d.task_id.clone(),
            title: d.title.clone(),
            reason: d.reason.clone(),
        }).collect(),
        generated_at: Utc::now(),
    })
}

fn calculate_available_mins(plan: &LlmDayPlanResponse) -> i32 {
    plan.slots.iter().map(|s| s.estimated_minutes).sum::<i32>()
        + plan.deferred.len() as i32 * 30 // rough estimate for deferred
}

fn build_planning_prompt(ctx: &PlanningContext, locked: &[PlanSlot]) -> String {
    let mut prompt = DAY_PLAN_PROMPT.to_string();

    // Format scored tasks
    let tasks_str = ctx.tasks.iter().enumerate().map(|(i, t)| {
        format!(
            "{}. {} (ID: {}, est: {}min, energy: {}, P{})",
            i + 1,
            t.title,
            t.id,
            t.estimated_minutes.unwrap_or(30),
            t.energy_level.as_ref().map(|e| e.to_string()).unwrap_or_else(|| "medium".to_string()),
            t.priority.unwrap_or(3),
        )
    }).collect::<Vec<_>>().join("\n");

    prompt = prompt.replace("{{scored_tasks}}", &tasks_str);
    prompt = prompt.replace("{{work_start}}", &ctx.working_hours.start.format("%H:%M").to_string());
    prompt = prompt.replace("{{work_end}}", &ctx.working_hours.end.format("%H:%M").to_string());
    prompt = prompt.replace("{{lunch_start}}", &ctx.working_hours.lunch_start.format("%H:%M").to_string());

    // Calculate available minutes
    let work_mins = (ctx.working_hours.end - ctx.working_hours.start).num_minutes() - 30; // minus lunch
    prompt = prompt.replace("{{available_mins}}", &work_mins.to_string());

    // Energy profile
    if let Some(ref profile) = ctx.energy_profile {
        prompt = prompt.replace("{{peak_hours}}", &format!("{:?}", profile.peak_hours));
        prompt = prompt.replace("{{low_hours}}", &format!("{:?}", profile.low_energy_hours));
        prompt = prompt.replace(
            "{{avg_focus_mins}}",
            &profile.avg_focus_duration_mins.unwrap_or(45).to_string(),
        );
    } else {
        prompt = prompt.replace("{{peak_hours}}", "[9, 10, 11]");
        prompt = prompt.replace("{{low_hours}}", "[14, 15]");
        prompt = prompt.replace("{{avg_focus_mins}}", "45");
    }

    // Calendar blocks
    let cal_str = if ctx.calendar_blocks.is_empty() {
        "(none)".to_string()
    } else {
        ctx.calendar_blocks.iter().map(|b| {
            format!("- {} ({} - {}, {})",
                b.title,
                b.start.format("%H:%M"),
                b.end.format("%H:%M"),
                if b.is_busy { "busy" } else { "free" },
            )
        }).collect::<Vec<_>>().join("\n")
    };
    prompt = prompt.replace("{{calendar_blocks}}", &cal_str);

    // Locked slots
    let locked_str = if locked.is_empty() {
        "(none)".to_string()
    } else {
        locked.iter().map(|s| {
            format!("- {} at {} ({}min)",
                s.title,
                s.start_time.as_deref().unwrap_or("?"),
                s.estimated_minutes,
            )
        }).collect::<Vec<_>>().join("\n")
    };
    prompt = prompt.replace("{{locked_slots}}", &locked_str);

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use feature_tasks::types::{Task, WorkingHours};

    #[test]
    fn test_build_planning_prompt_includes_tasks() {
        let mut task = Task::default_instance();
        task.title = "Important task".to_string();
        task.estimated_minutes = Some(60);

        let ctx = PlanningContext {
            tasks: vec![task],
            working_hours: WorkingHours {
                start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
                lunch_start: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            },
            calendar_blocks: vec![],
            energy_profile: None,
            max_tasks: None,
            locked_task_ids: vec![],
            target_date: None,
        };

        let prompt = build_planning_prompt(&ctx, &[]);
        assert!(prompt.contains("Important task"));
        assert!(prompt.contains("60min"));
        assert!(prompt.contains("09:00"));
        assert!(prompt.contains("17:00"));
    }

    #[test]
    fn test_calculate_available_mins() {
        let plan = LlmDayPlanResponse {
            slots: vec![
                LlmPlanSlot {
                    task_id: "1".into(), title: "A".into(),
                    estimated_minutes: 60, start_time: None, energy_level: None,
                },
                LlmPlanSlot {
                    task_id: "2".into(), title: "B".into(),
                    estimated_minutes: 45, start_time: None, energy_level: None,
                },
            ],
            deferred: vec![],
            total_work_mins: Some(105),
            utilization: Some(0.5),
            reasoning: "test".into(),
        };
        assert_eq!(calculate_available_mins(&plan), 105);
    }
}
```

- [ ] **Step 2: Update handlers/mod.rs**

Uncomment `mod planning;` and `pub use planning::LlmDayPlanningHandler;`

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p agent
```

- [ ] **Step 4: Run tests**

```bash
cargo nextest run -p agent -E 'test(planning)'
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/handlers/planning.rs crates/agent/src/handlers/mod.rs crates/agent/src/templates/day_plan_prompt.md
git commit -m "feat(agent): implement LlmDayPlanningHandler with energy matching"
```

---

## Chunk 5: Wire plan_day Action & Builder Integration

### Task 9: Wire DayPlanningHandler into plan_day action

**Files:**
- Modify: `crates/feature-tasks/src/tool/actions/plan.rs`

- [ ] **Step 1: Update `handle_plan_day` to use injected handler when available**

The current `handle_plan_day` is the Phase 1 scoring-only fallback. Wrap it so when `self.planning_handler` is `Some`, it builds a `PlanningContext` and delegates:

Replace the body of `handle_plan_day` in `crates/feature-tasks/src/tool/actions/plan.rs`:

```rust
pub(crate) async fn handle_plan_day(&self, p: &ParamExtractor<'_>) -> Result<String> {
    let count = p.optional_u64("count")?.unwrap_or(3).min(10) as usize;
    let energy_preference = p.optional_str("energy_level")?;

    // If LLM planning handler is available, use it
    if let Some(ref handler) = self.planning_handler {
        return self.handle_plan_day_llm(handler.as_ref(), count).await;
    }

    // Fallback: scoring-only plan (Phase 1 behavior)
    self.handle_plan_day_scoring(count, energy_preference).await
}

/// LLM-powered daily planning path.
async fn handle_plan_day_llm(
    &self,
    handler: &dyn crate::handlers::DayPlanningHandler,
    count: usize,
) -> Result<String> {
    use crate::types::{PlanningContext, Task, WorkingHours};

    let rows = self.repo.list(&storage::TaskFilter::default()).await?;
    let tasks: Vec<Task> = rows.into_iter()
        .map(Task::from)
        .filter(|t| t.status == "todo" && !t.is_template && t.focused_at.is_none())
        .take(count * 3) // Give LLM more candidates to choose from
        .collect();

    let ctx = PlanningContext {
        tasks,
        working_hours: self.config.working_hours.clone(),
        calendar_blocks: vec![],
        energy_profile: None,
        max_tasks: Some(count as u32),
        locked_task_ids: vec![],
        target_date: None,
    };

    let plan = handler.plan_day(&ctx).await?;

    // Format output
    let mut output = format!("Daily plan ({} slots):\n\n", plan.slots.len());
    for (i, slot) in plan.slots.iter().enumerate() {
        let time = slot.start_time.as_deref().unwrap_or("--:--");
        let energy = slot.energy_level.as_ref()
            .map(|e| format!(" · energy: {e}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "{}. [{}] {} (ID: {}) · {}min{}\n",
            i + 1, time, slot.title, slot.task_id, slot.estimated_minutes, energy,
        ));
    }

    if !plan.deferred.is_empty() {
        output.push_str(&format!("\nDeferred ({}):\n", plan.deferred.len()));
        for d in &plan.deferred {
            output.push_str(&format!("  - {} ({})\n", d.title, d.reason));
        }
    }

    output.push_str(&format!(
        "\nTotal: {}min ({:.1}h) · Utilization: {:.0}%\n",
        plan.total_work_mins,
        plan.total_work_mins as f64 / 60.0,
        plan.utilization * 100.0,
    ));

    if !plan.reasoning.is_empty() {
        output.push_str(&format!("\n{}\n", plan.reasoning));
    }

    Ok(output)
}

/// Scoring-only daily plan (Phase 1 fallback).
async fn handle_plan_day_scoring(
    &self,
    count: usize,
    energy_preference: Option<&str>,
) -> Result<String> {
    // (Move the existing plan_day body here — the code currently in handle_plan_day)
    info!("Generating daily plan (top {} tasks, scoring fallback)", count);
    // ... existing scoring code ...
}
```

The key change: extract the existing `handle_plan_day` body into `handle_plan_day_scoring`, and add the `handle_plan_day_llm` path that delegates to the injected handler.

- [ ] **Step 2: Verify compilation**

```bash
cargo build -p feature-tasks
```

- [ ] **Step 3: Run existing plan_day tests (should still pass with scoring fallback)**

```bash
cargo nextest run -p feature-tasks -E 'test(plan_day)'
```

- [ ] **Step 4: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/plan.rs
git commit -m "feat(tasks): wire DayPlanningHandler into plan_day action with scoring fallback"
```

---

### Task 10: Wire handlers in AgentLoopBuilder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Add handler construction after existing task_tool setup**

In builder.rs, after the domain_bus wiring block (line ~644) and before `tool_registry.register(task_tool)` (line ~647), add:

```rust
            // ── Phase 2: Agentic Intelligence handlers ────────────────────
            // Decomposition handler
            let decomp_handler = Arc::new(
                crate::handlers::LlmDecompositionHandler::new(
                    provider.clone(),
                    config.agents.defaults.model.clone(),
                    task_repo.clone(),
                    self.domain_event_bus.clone(),
                )
            );
            task_tool = task_tool.with_decomposition_handler(
                Arc::clone(&decomp_handler) as Arc<dyn feature_tasks::DecompositionHandler>
            );

            // Execution handler
            let exec_handler = Arc::new(
                crate::handlers::LlmTaskExecutionHandler::new(
                    task_repo.clone(),
                    self.domain_event_bus.clone(),
                )
            );
            task_tool = task_tool.with_execution_handler(
                Arc::clone(&exec_handler) as Arc<dyn feature_tasks::TaskExecutionHandler>
            );

            // Day planning handler
            let plan_handler = Arc::new(
                crate::handlers::LlmDayPlanningHandler::new(
                    provider.clone(),
                    config.agents.defaults.model.clone(),
                    self.domain_event_bus.clone(),
                )
            );
            task_tool = task_tool.with_planning_handler(
                Arc::clone(&plan_handler) as Arc<dyn feature_tasks::DayPlanningHandler>
            );
```

Note: `task_repo` is created on line ~575 (`TaskRepo::new(pool_ref.clone())`). Make sure the variable is still in scope. If needed, clone it before this block.

- [ ] **Step 2: Add import for handlers module**

At the top of `builder.rs`, add the import or ensure `crate::handlers` is accessible.

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p agent
```

- [ ] **Step 4: Verify full workspace builds**

```bash
cargo build --workspace
```

- [ ] **Step 5: Run all tests**

```bash
cargo nextest run --workspace
```

- [ ] **Step 6: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features
```

Fix any warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire Phase 2 handlers into AgentLoopBuilder"
```

---

## Chunk 6: Add Decompose & Execute Tool Actions

### Task 11: Add `decompose` and `execute` tool actions

**Files:**
- Create: `crates/feature-tasks/src/tool/actions/decompose.rs`
- Create: `crates/feature-tasks/src/tool/actions/execute.rs`
- Modify: `crates/feature-tasks/src/tool/actions/mod.rs`
- Modify: `crates/feature-tasks/src/tool/mod.rs` (add actions to enum + dispatch)

- [ ] **Step 1: Create decompose action**

`crates/feature-tasks/src/tool/actions/decompose.rs`:

```rust
//! Decompose action: AI-powered subtask generation.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;
use crate::types::{DecompositionContext, Task};

impl TaskTool {
    pub(crate) async fn handle_decompose(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.decomposition_handler.as_ref().ok_or_else(|| {
            common::KlyntbotError::Internal("Decomposition handler not available".into())
        })?;

        let id = p.required_str("id")?;
        let task = self.get_full_task(id).await?
            .ok_or_else(|| common::KlyntbotError::NotFound(format!("Task {id} not found")))?;

        info!(task_id = %id, "Decomposing task");

        let existing = self.repo.get_children(&task.id).await?;
        let context = DecompositionContext {
            max_depth: 2,
            max_subtasks_per_level: 10,
            existing_subtasks: existing.iter().map(|r| r.title.clone()).collect(),
            project_context: task.project_id.clone(),
            cognitive_facts: vec![],
            user_energy_profile: None,
            calendar_context: vec![],
        };

        let result = handler.decompose(&task, &context).await?;

        // Confidence gate
        let threshold = self.config.decomposition_auto_apply_threshold;
        if result.confidence >= threshold {
            // Auto-create subtasks
            let mut created_ids = Vec::new();
            for subtask in &result.tree.subtasks {
                let sub_id = uuid::Uuid::new_v4().to_string();
                let mut row = storage::TaskRow::default();
                row.id = sub_id.clone();
                row.title = subtask.title.clone();
                row.description = subtask.description.clone();
                row.parent_id = Some(task.id.clone());
                row.area_id = task.area_id.clone();
                row.project_id = task.project_id.clone();
                row.acceptance_criteria = subtask.acceptance_criteria.clone();
                row.estimated_minutes = subtask.estimated_minutes;
                row.energy_level = subtask.energy_level.as_ref().map(|e| e.to_string());
                row.priority = subtask.priority;
                row.task_type = subtask.task_type.as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "manual".to_string());
                self.repo.add(&row).await?;
                created_ids.push(sub_id);
            }

            // Log activity
            self.repo.log_activity(
                &task.id, "decomposed", None, None, None, "agent",
                Some(&format!("Auto-decomposed into {} subtasks (confidence: {:.0}%)",
                    created_ids.len(), result.confidence * 100.0)),
            ).await?;

            // Emit domain event
            if let Some(ref bus) = self.domain_bus {
                let _ = bus.publish(bus::DomainEvent::TaskDecomposed {
                    source_task_id: task.id.clone(),
                    subtask_ids: created_ids.clone(),
                    total_estimated_mins: result.tree.total_estimated_mins,
                });
            }

            let mut output = format!(
                "Decomposed '{}' into {} subtasks (confidence: {:.0}%, auto-applied):\n\n",
                task.title, created_ids.len(), result.confidence * 100.0,
            );
            for (i, subtask) in result.tree.subtasks.iter().enumerate() {
                output.push_str(&format!("  {}. {}", i + 1, subtask.title));
                if let Some(est) = subtask.estimated_minutes {
                    output.push_str(&format!(" ({}min)", est));
                }
                output.push('\n');
            }
            Ok(output)
        } else {
            // Store as pending decomposition for review
            let decomp_row = storage::TaskDecompositionRow {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: task.id.clone(),
                plan: serde_json::to_string(&result.tree)?,
                confidence: result.confidence,
                status: "pending".to_string(),
                reasoning: Some(result.reasoning.clone()),
                created_at: chrono::Utc::now(),
                applied_at: None,
            };
            self.repo.create_decomposition(&decomp_row).await?;

            let mut output = format!(
                "Decomposition plan created for '{}' (confidence: {:.0}%, below {:.0}% threshold — needs review):\n\n",
                task.title, result.confidence * 100.0, threshold * 100.0,
            );
            for (i, subtask) in result.tree.subtasks.iter().enumerate() {
                output.push_str(&format!("  {}. {}", i + 1, subtask.title));
                if let Some(est) = subtask.estimated_minutes {
                    output.push_str(&format!(" ({}min)", est));
                }
                output.push('\n');
            }
            if !result.validation_warnings.is_empty() {
                output.push_str(&format!("\nWarnings ({}):\n", result.validation_warnings.len()));
                for w in &result.validation_warnings {
                    output.push_str(&format!("  - {}\n", w.message));
                }
            }
            Ok(output)
        }
    }
}
```

- [ ] **Step 2: Create execute action**

`crates/feature-tasks/src/tool/actions/execute.rs`:

```rust
//! Execute action: start agentic task execution.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;
use crate::types::ExecutionConfig;

impl TaskTool {
    pub(crate) async fn handle_execute(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.execution_handler.as_ref().ok_or_else(|| {
            common::KlyntbotError::Internal("Execution handler not available".into())
        })?;

        let id = p.required_str("id")?;
        let task = self.get_full_task(id).await?
            .ok_or_else(|| common::KlyntbotError::NotFound(format!("Task {id} not found")))?;

        let config = ExecutionConfig {
            agent_profile: p.optional_str("agent_profile")?.map(|s| s.to_string()),
            max_cost_usd: p.optional_f64("max_cost")?,
            max_iterations: p.optional_u64("max_iterations")?.map(|n| n as u32),
            timeout_secs: p.optional_u64("timeout_secs")?,
            require_approval: p.optional_bool("require_approval")?.unwrap_or(false),
            ..Default::default()
        };

        info!(task_id = %id, "Executing task");

        let result = handler.execute(&task, &config).await?;

        match result {
            crate::types::ExecuteResult::Started { execution_id } => {
                Ok(format!("Execution started for '{}' (execution: {})", task.title, execution_id))
            }
            crate::types::ExecuteResult::AwaitingApproval { suggestion_id } => {
                Ok(format!(
                    "Execution of '{}' requires approval. Suggestion created: {}",
                    task.title, suggestion_id,
                ))
            }
        }
    }

    pub(crate) async fn handle_cancel_execution(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.execution_handler.as_ref().ok_or_else(|| {
            common::KlyntbotError::Internal("Execution handler not available".into())
        })?;

        let execution_id = p.required_str("execution_id")?;
        handler.cancel_execution(execution_id).await?;
        Ok(format!("Execution {} cancelled", execution_id))
    }
}
```

- [ ] **Step 3: Add action modules to mod.rs**

In `crates/feature-tasks/src/tool/actions/mod.rs`, add:

```rust
mod decompose;
mod execute;
```

- [ ] **Step 4: Register new actions in TaskTool dispatch**

In `crates/feature-tasks/src/tool/mod.rs`, add to the `parameters()` JSON (the `"enum"` array):

```
"decompose", "execute", "cancel_execution"
```

Add to the `execute()` match block:

```rust
"decompose" => self.handle_decompose(&p).await,
"execute" => self.handle_execute(&p).await,
"cancel_execution" => self.handle_cancel_execution(&p).await,
```

Update the `description()` string to include the new actions.

Also add new parameter definitions to `parameters()`:

```json
"execution_id": { "type": "string", "description": "Execution ID (for cancel_execution)" },
"agent_profile": { "type": "string", "description": "Agent profile for execution" },
"max_cost": { "type": "number", "description": "Max cost in USD for execution" },
"max_iterations": { "type": "integer", "description": "Max LLM iterations for execution" },
"timeout_secs": { "type": "integer", "description": "Timeout in seconds for execution" },
"require_approval": { "type": "boolean", "description": "Require approval before execution" }
```

- [ ] **Step 5: Verify compilation**

```bash
cargo build -p feature-tasks
```

- [ ] **Step 6: Run all existing tests**

```bash
cargo nextest run -p feature-tasks
```

- [ ] **Step 7: Commit**

```bash
git add crates/feature-tasks/src/tool/actions/
git add crates/feature-tasks/src/tool/mod.rs
git commit -m "feat(tasks): add decompose, execute, cancel_execution tool actions"
```

---

### Task 12: Final verification

- [ ] **Step 1: Full workspace build**

```bash
cargo build --workspace
```

- [ ] **Step 2: Full test suite**

```bash
cargo nextest run --workspace
```

- [ ] **Step 3: Clippy (zero warnings)**

```bash
cargo clippy --workspace --all-targets --all-features
```

- [ ] **Step 4: Format check**

```bash
cargo fmt --all --check
```

- [ ] **Step 5: Fix any issues and commit**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting for Phase 2"
```

---

## Summary

| Task | Component | Files | Estimated Complexity |
|------|-----------|-------|---------------------|
| 1 | Storage extensions | task_repo.rs | Low |
| 2 | TaskTool handler fields | tool/mod.rs | Low |
| 3 | Decomposition prompt | templates/ | Low |
| 4 | DecompositionHandler impl | handlers/decomposition.rs | High |
| 5 | Execution prompt | templates/ | Low |
| 6 | TaskExecutionHandler impl | handlers/execution.rs | High |
| 7 | Day planning prompt | templates/ | Low |
| 8 | DayPlanningHandler impl | handlers/planning.rs | High |
| 9 | Wire plan_day action | actions/plan.rs | Medium |
| 10 | Builder wiring | builder.rs | Medium |
| 11 | Decompose/Execute actions | actions/ | Medium |
| 12 | Final verification | — | Low |

**Dependencies:** Tasks 1-2 must complete first (storage + fields). Tasks 3-8 are the core handlers (can be done in any order, but each prompt→handler pair should be sequential). Tasks 9-11 wire everything together. Task 12 is final.
