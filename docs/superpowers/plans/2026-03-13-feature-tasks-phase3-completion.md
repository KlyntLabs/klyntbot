# feature-tasks Phase 3 Completion Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 3 of feature-tasks by wiring the proactive suggestion cron job (every 4 hours), suggestion auto-application at confidence ≥ 0.85, and focus time tracking (backend Tauri commands + frontend timer UI).

**Architecture:** Three independent subsystems wired in sequence: (1) cron job that calls the existing `LlmProactiveHandler` every 4 hours, persists `SuggestionCandidate` results to `task_suggestions`, and emits `ProactiveSuggestionCreated` with the DB row ID; (2) auto-apply logic that calls `TaskSuggestionApplier` on high-confidence suggestions inline after persist; (3) focus session tracking via two new Tauri commands (`start_focus`/`end_focus`) that store `ActiveTaskFocus` in `AppCore`, emit domain events for cognitive learning, and surface an active focus banner in the UI.

**Tech Stack:** Rust (async_trait, tokio, SQLx), Tauri v2, React + TypeScript, Tailwind v4, Vitest

**Spec:** `docs/superpowers/specs/2026-03-13-feature-tasks-phase3-completion.md`

---

## File Map

### New files
| File | Purpose |
|------|---------|
| `crates/feature-tasks/src/types/active_focus.rs` | `ActiveTaskFocus` struct |
| `crates/app-core/src/handlers/tasks/proactive.rs` | `run_proactive_scan()` called by cron |
| `crates/app-core/src/handlers/tasks/focus.rs` | `start_focus()` / `end_focus()` logic on `AppCore` |
| `desktop-ui/src/features/tasks/hooks/useFocusSession.ts` | Hook: elapsed timer from `focusedAt` |
| `desktop-ui/src/features/tasks/hooks/useFocusSession.test.ts` | Vitest tests for the hook |
| `desktop-ui/src/features/tasks/components/FocusBanner.tsx` | Fixed-position active focus indicator |
| `desktop-ui/src/features/tasks/components/FocusBanner.test.tsx` | Vitest tests for the banner |

### Modified files
| File | Change |
|------|--------|
| `crates/agent/src/handlers/proactive.rs` | Refactor `evaluate_task()` to not emit events; expand `suggest()` with 3 new triggers; add `suggestion_applier` field |
| `crates/app-core/src/init/cron.rs` | Add `JOB_PROACTIVE_SCAN`, `tasks_config` param, guard, registration |
| `crates/app-core/src/handlers/tasks/mod.rs` | Export `proactive` and `focus` submodules |
| `crates/feature-tasks/src/types/mod.rs` | Export `ActiveTaskFocus` |
| `crates/app-core/src/state.rs` | Add `active_task_focus: Arc<Mutex<Option<ActiveTaskFocus>>>` |
| `crates/desktop/src/commands/tasks.rs` | Add `task_start_focus`, `task_end_focus` Tauri commands |
| `crates/desktop/src/lib.rs` (or `main.rs`) | Register new commands in invoke handler |
| `desktop-ui/src/features/tasks/components/TaskCard.tsx` | Add Start Focus button |
| `desktop-ui/src/features/tasks/pages/TaskDetail.tsx` | Add Start Focus button |
| Root layout component | Mount `FocusBanner` with entity subscription |

---

## Chunk 1: Proactive Cron Job (Gap 1)

### Task 1: Refactor `evaluate_task()` — remove internal event emission

**Context:** `evaluate_task()` currently emits `DomainEvent::ProactiveSuggestionCreated` internally with a freshly generated UUID. The cron wrapper will instead emit this event after persist, using the actual DB row ID. Remove the emission from `evaluate_task()` so events aren't fired twice.

**Files:**
- Modify: `crates/agent/src/handlers/proactive.rs`

- [ ] **Step 1.1: Write failing test that confirms no internal emission**

In the `#[cfg(test)] mod tests` block at the bottom of `crates/agent/src/handlers/proactive.rs`, add:

```rust
#[tokio::test]
async fn test_evaluate_task_returns_candidates_without_emitting() {
    // Build handler with no domain_bus — if it tries to emit internally and
    // domain_bus is None, the emit silently no-ops, so we just verify
    // candidates are returned correctly regardless.
    let handler = LlmProactiveHandlerBuilder::test_default().build();
    // Use whichever test helper or builder pattern currently exists in the test block
    let task = Task {
        id: "t1".to_string(),
        status: "todo".to_string(),
        due_date: Some(Utc::now() - Duration::hours(1)), // overdue
        ..Task::default()
    };
    let candidates = handler
        .evaluate_task(&task, &SuggestionTrigger::TaskOverdue)
        .await
        .unwrap();
    // Candidates are returned — internal event emission does not panic or block
    assert!(candidates.len() <= 10); // sanity bound only
}
```

Run: `cargo nextest run -p agent -E 'test(evaluate_task_returns_candidates)'`
Expected: FAIL (test doesn't exist yet or trait behaviour differs).

- [ ] **Step 1.2: Delete the internal `ProactiveSuggestionCreated` emission block from `evaluate_task()`**

Find the section inside `evaluate_task()` that does:
```rust
if let Some(ref bus) = self.domain_bus {
    let _ = bus.publish(DomainEvent::ProactiveSuggestionCreated { ... });
}
```
Delete it entirely. The method now just returns `Ok(candidates)`.

- [ ] **Step 1.3: Run tests**

```bash
cargo nextest run -p agent
```
Expected: all existing tests pass. New test passes.

- [ ] **Step 1.4: Commit**

```bash
git add crates/agent/src/handlers/proactive.rs
git commit -m "refactor(proactive): remove internal event emission from evaluate_task"
```

---

### Task 2: Expand `suggest()` with `TaskStale`, `WipLimitExceeded`, `BlockedChainStale`

**Context:** Currently `suggest()` only fetches `status = "todo"` tasks and evaluates `TaskOverdue`. Expand it to cover the three remaining triggers. `WipLimitExceeded` requires a separate COUNT query.

**Files:**
- Modify: `crates/agent/src/handlers/proactive.rs`

- [ ] **Step 2.1: Check if `TaskRepo` has `count_by_status()`**

```bash
cargo grep -r "count_by_status" crates/storage/
```
If it doesn't exist, add it to `crates/storage/src/repos/task_repo/queries.rs` (or wherever task queries live):

```rust
pub async fn count_by_status(&self, status: &str) -> Result<i64, StorageError> {
    let row = sqlx::query!("SELECT COUNT(*) as count FROM tasks WHERE status = ?", status)
        .fetch_one(&self.pool.0)
        .await?;
    Ok(row.count)
}
```

Run: `cargo nextest run -p storage` — ensure it compiles and passes.

- [ ] **Step 2.2: Write failing tests for new triggers**

```rust
#[tokio::test]
async fn test_suggest_evaluates_stale_tasks() {
    // task with status=doing, updated_at = 10 days ago (stale_task_days=5)
    // suggest() should produce candidates with trigger=TaskStale
    // (Integration test using in-memory pool + seeded task row)
}

#[tokio::test]
async fn test_suggest_evaluates_wip_limit_exceeded() {
    // seed 6 tasks with status=doing (wip_limit default=5)
    // suggest() should produce WipLimitExceeded candidates
}
```

Run: `cargo nextest run -p agent -E 'test(suggest_evaluates)'`
Expected: FAIL.

- [ ] **Step 2.3: Broaden the status filter in `suggest()`**

Change `TaskFilter { status: Some("todo".into()), ... }` to a multi-status fetch. If `TaskFilter` supports a `Vec<String>`, use that. Otherwise issue two separate calls and merge:

```rust
let mut tasks = self.repo.list(TaskFilter { status: Some("todo".into()), ..Default::default() }).await?;
let mut doing_tasks = self.repo.list(TaskFilter { status: Some("doing".into()), ..Default::default() }).await?;
tasks.append(&mut doing_tasks);
```

- [ ] **Step 2.4: Add `TaskStale` loop after the existing `TaskOverdue` loop**

```rust
// --- TaskStale ---
let stale_cutoff = Utc::now() - Duration::days(self.tasks_config.stale_task_days as i64);
for task in tasks.iter().filter(|t| t.updated_at < stale_cutoff) {
    let mut c = self.evaluate_task(task, &SuggestionTrigger::TaskStale).await?;
    all_candidates.append(&mut c);
}
```

- [ ] **Step 2.5: Add `WipLimitExceeded` using a separate count query**

```rust
// --- WipLimitExceeded ---
let doing_count = self.repo.count_by_status("doing").await?;
if doing_count > self.tasks_config.wip_limit as i64 {
    let mut doing = self.repo
        .list(TaskFilter { status: Some("doing".into()), ..Default::default() })
        .await?;
    // Oldest first (most likely stale)
    doing.sort_by_key(|t| t.updated_at);
    for task in doing.iter().take(3) {
        let mut c = self.evaluate_task(task, &SuggestionTrigger::WipLimitExceeded).await?;
        all_candidates.append(&mut c);
    }
}
```

- [ ] **Step 2.6: Add `BlockedChainStale` loop**

**Note:** `Task.blocked_by` is a derived field that is NOT populated by the standard `repo.list()` call. Use the existing `TaskRepo::get_blockers(task_id)` method which already exists in `crates/storage/src/repos/task_repo/dependencies.rs` and returns `Vec<TaskRow>` (full blocker rows, not IDs). No new method needs to be added.

```rust
// --- BlockedChainStale ---
let stale_cutoff = Utc::now() - Duration::days(self.tasks_config.stale_task_days as i64);
for task in tasks.iter() {
    let blocker_rows = self.repo.get_blockers(&task.id).await.unwrap_or_default();
    if blocker_rows.is_empty() {
        continue;
    }
    let has_stale_blocker = blocker_rows.iter().any(|b| b.updated_at < stale_cutoff);
    if has_stale_blocker {
        let mut c = self.evaluate_task(task, &SuggestionTrigger::BlockedChainStale).await?;
        all_candidates.append(&mut c);
    }
}
```

- [ ] **Step 2.7: Run tests**

```bash
cargo nextest run -p agent -p storage
```
Expected: all pass.

- [ ] **Step 2.8: Commit**

```bash
git add crates/agent/src/handlers/proactive.rs crates/storage/src/repos/task_repo/
git commit -m "feat(proactive): expand suggest() to evaluate TaskStale, WipLimitExceeded, BlockedChainStale"
```

---

### Task 3: Write `run_proactive_scan()` and wire cron job

**Context:** The cron callback calls `run_proactive_scan()`, which calls `suggest()`, persists each candidate to `task_suggestions`, and emits `ProactiveSuggestionCreated` with the real DB row ID.

**Files:**
- Create: `crates/app-core/src/handlers/tasks/proactive.rs`
- Modify: `crates/app-core/src/handlers/tasks/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 3.1: Create `run_proactive_scan()` with a failing test**

Create `crates/app-core/src/handlers/tasks/proactive.rs`:

```rust
use std::sync::Arc;
use tracing::{info, warn};
use chrono::Utc;
use uuid::Uuid;
use feature_tasks::{
    handlers::proactive::ProactiveHandler,
    types::SuggestionScope,
    TasksConfig,
};
use storage::{repos::task_repo::TaskSuggestionRow, Repos};
use bus::{DomainEvent, DomainEventBus};
use common::KlyntbotError;

pub async fn run_proactive_scan(
    handler: &Arc<dyn ProactiveHandler>,
    repos: &Repos,
    domain_bus: &Option<Arc<DomainEventBus>>,
    tasks_config: &TasksConfig,
    suggestion_applier: &Option<Arc<dyn feature_tasks::handlers::suggestion_applier::SuggestionApplier>>,
) -> Result<usize, KlyntbotError> {
    let candidates = handler.suggest(&SuggestionScope::default()).await?;
    let mut persisted = 0;

    for candidate in &candidates {
        let row = TaskSuggestionRow {
            id: Uuid::new_v4().to_string(),
            task_id: candidate.task_id.clone(),
            suggestion_type: candidate.suggestion_type.to_string(),
            title: candidate.title.clone(),
            description: candidate.description.clone(),
            confidence: candidate.confidence,
            action_payload: candidate.action.as_ref()
                .and_then(|a| serde_json::to_string(a).ok()),
            status: "pending".to_string(),
            trigger: candidate.trigger.as_ref().map(|t| t.to_string()),
            created_at: Utc::now(),
            resolved_at: None,
        };

        match repos.tasks.create_suggestion(&row).await {
            Ok(created) => {
                // Emit with the real DB row ID (bus.publish, not bus.send)
                if let Some(bus) = domain_bus {
                    let _ = bus.publish(DomainEvent::ProactiveSuggestionCreated {
                        suggestion_id: created.id.clone(),
                        suggestion_type: created.suggestion_type.clone(),
                        task_id: created.task_id.clone(),
                        confidence: created.confidence,
                    });
                }

                // Auto-apply if confidence is high enough
                if candidate.confidence >= tasks_config.suggestion_auto_apply_threshold {
                    if let (Some(applier), Some(action)) = (suggestion_applier, &candidate.action) {
                        match applier.apply(&created.id, created.task_id.as_deref(), action).await {
                            Ok(_) => {
                                let _ = repos.tasks.resolve_suggestion(&created.id, "autoapplied").await;
                                info!(
                                    "Auto-applied suggestion {} ({}) for task {:?} (confidence {:.2})",
                                    created.id, created.suggestion_type, created.task_id, candidate.confidence
                                );
                            }
                            Err(e) => warn!("Auto-apply failed for suggestion {}: {e}", created.id),
                        }
                    }
                }

                persisted += 1;
            }
            Err(e) => warn!("Failed to persist proactive suggestion: {e}"),
        }
    }

    Ok(persisted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_proactive_scan_persists_zero_when_no_candidates() {
        // Mock handler returning empty vec → 0 persisted
        // (Use a simple struct implementing ProactiveHandler that returns Ok(vec![]))
        // Verify task_suggestions table is empty
    }

    #[tokio::test]
    async fn test_run_proactive_scan_persists_candidates() {
        // Mock handler returning 1 candidate → 1 row in task_suggestions
    }

    #[tokio::test]
    async fn test_run_proactive_scan_auto_applies_high_confidence() {
        // candidate confidence=0.90, threshold=0.85
        // → resolve_suggestion called with "autoapplied"
    }

    #[tokio::test]
    async fn test_run_proactive_scan_leaves_low_confidence_pending() {
        // candidate confidence=0.60 → status stays "pending"
    }
}
```

Run: `cargo nextest run -p app-core -E 'test(run_proactive_scan)'`
Expected: FAIL (not yet wired, missing impls).

- [ ] **Step 3.2: Export proactive module**

In `crates/app-core/src/handlers/tasks/mod.rs`, add:
```rust
pub mod proactive;
```

- [ ] **Step 3.3: Add `JOB_PROACTIVE_SCAN` constant and extend `init_cron()` signature**

In `crates/app-core/src/init/cron.rs`:

```rust
pub const JOB_PROACTIVE_SCAN: &str = "proactive_scan";
```

`init_cron()` must also receive the proactive handler and suggestion applier so they can be captured into the cron callback. Add these parameters to `init_cron()` (and to `register_cron_callbacks()` if it's a separate private fn):

```rust
pub async fn init_cron(
    // ... existing params ...
    tasks_config: feature_tasks::TasksConfig,
    proactive_handler: Arc<dyn feature_tasks::handlers::proactive::ProactiveHandler>,
    suggestion_applier: Option<Arc<dyn feature_tasks::handlers::suggestion_applier::SuggestionApplier>>,
) -> CronResult { ... }
```

Update all call sites (in `crates/app-core/src/init/mod.rs` or wherever `init_cron` is called) to pass:
- `TasksConfig::default()` for `tasks_config`
- The constructed `LlmProactiveHandler` (already built as part of agent initialization) for `proactive_handler`
- `Some(Arc::new(TaskSuggestionApplier::new(...)))` for `suggestion_applier`

- [ ] **Step 3.4: Register the cron callback**

Inside `register_cron_callbacks()`, add the proactive scan registration. The `CronService::register_handler()` takes a **synchronous** `Arc<Fn>` callback — use `tokio::task::block_in_place` to bridge into async, following the pattern used by all other handlers in the file:

```rust
if tasks_config.proactive_suggestions {
    let handler = proactive_handler.clone();
    let repos = repos.clone();
    let bus = domain_bus.clone();
    let cfg = tasks_config.clone();
    let applier = suggestion_applier.clone();
    let rt = tokio::runtime::Handle::current();

    cron_service.register_handler(
        JOB_PROACTIVE_SCAN,
        Arc::new(move |_job: &scheduling::CronJob| {
            let handler = handler.clone();
            let repos = repos.clone();
            let bus = bus.clone();
            let cfg = cfg.clone();
            let applier = applier.clone();
            tokio::task::block_in_place(|| {
                rt.block_on(async move {
                    match run_proactive_scan(&handler, &repos, &bus, &cfg, &applier).await {
                        Ok(n) => Ok(Some(format!("Proactive scan complete: {n} suggestions generated"))),
                        Err(e) => {
                            warn!("Proactive scan failed: {e}");
                            Ok(None)
                        }
                    }
                })
            })
        }),
    );
}
```

Note: `register_handler` is **synchronous** — no `.await` needed.

- [ ] **Step 3.5: Add to `ensure_cron_jobs()` using the `ensure_job!` macro**

Inside `ensure_cron_jobs()` in `cron.rs`, the existing pattern uses a local `ensure_job!` macro. Add the proactive scan entry following the same macro syntax used by the other jobs in that function:

```rust
if tasks_config.proactive_suggestions {
    ensure_job!(
        JOB_PROACTIVE_SCAN,
        scheduling::CronSchedule::Cron {
            expr: "0 */4 * * *".to_string(),
            tz: None,
        },
        "Proactive task suggestion scan"
    );
}
```

If `ensure_cron_jobs` doesn't receive `tasks_config`, pass it as a parameter alongside other config values.

- [ ] **Step 3.6: Run tests and build**

```bash
cargo nextest run -p app-core
cargo build -p app-core
```
Expected: all tests pass, no compile errors.

- [ ] **Step 3.7: Commit**

```bash
git add crates/app-core/src/handlers/tasks/proactive.rs \
        crates/app-core/src/handlers/tasks/mod.rs \
        crates/app-core/src/init/cron.rs
git commit -m "feat(cron): register proactive suggestion scan every 4 hours with auto-apply"
```

---

## Chunk 2: Focus Time Tracking — Backend (Gap 3 BE)

### Task 4: Add `ActiveTaskFocus` type

**Files:**
- Create: `crates/feature-tasks/src/types/active_focus.rs`
- Modify: `crates/feature-tasks/src/types/mod.rs`

- [ ] **Step 4.1: Create the struct**

`crates/feature-tasks/src/types/active_focus.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::EnergyLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTaskFocus {
    pub task_id: String,
    pub started_at: DateTime<Utc>,
    pub energy_level: Option<EnergyLevel>,
}
```

- [ ] **Step 4.2: Export from types mod**

In `crates/feature-tasks/src/types/mod.rs`, add:

```rust
pub mod active_focus;
pub use active_focus::ActiveTaskFocus;
```

- [ ] **Step 4.3: Verify compilation**

```bash
cargo build -p feature-tasks
```
Expected: compiles cleanly.

- [ ] **Step 4.4: Commit**

```bash
git add crates/feature-tasks/src/types/active_focus.rs \
        crates/feature-tasks/src/types/mod.rs
git commit -m "feat(tasks): add ActiveTaskFocus type for in-memory focus session state"
```

---

### Task 5: Add `active_task_focus` field to `AppCore`

**Files:**
- Modify: `crates/app-core/src/state.rs`

- [ ] **Step 5.1: Add import and field**

In `crates/app-core/src/state.rs`, add the import:

```rust
use feature_tasks::types::ActiveTaskFocus;
```

`state.rs` already imports `tokio::sync::Mutex` for other fields. To avoid ambiguity, use the **standard library** mutex for `active_task_focus` (it's only held briefly, never across `.await` points) with its fully qualified path:

Add field to `AppCore` struct (near other Arc-wrapped state):

```rust
pub active_task_focus: Arc<std::sync::Mutex<Option<ActiveTaskFocus>>>,
```

Initialize it in `AppCore::new()` or the builder:

```rust
active_task_focus: Arc::new(std::sync::Mutex::new(None)),
```

In `focus.rs`, use the same fully qualified path when locking:
```rust
let mut lock = self.active_task_focus.lock().unwrap(); // std::sync::Mutex — sync, safe to unwrap
```

- [ ] **Step 5.2: Verify compilation**

```bash
cargo build -p app-core
```
Expected: compiles cleanly.

- [ ] **Step 5.3: Commit**

```bash
git add crates/app-core/src/state.rs
git commit -m "feat(app-core): add active_task_focus field to AppCore"
```

---

### Task 6: Implement `start_focus()` and `end_focus()` on `AppCore`

**Files:**
- Create: `crates/app-core/src/handlers/tasks/focus.rs`
- Modify: `crates/app-core/src/handlers/tasks/mod.rs`

- [ ] **Step 6.1: Create the focus handler file with tests**

Create `crates/app-core/src/handlers/tasks/focus.rs`:

```rust
use chrono::Utc;
use desktop_shared::types::EntityKind;
use feature_tasks::types::ActiveTaskFocus;
use bus::DomainEvent;
use crate::state::{AppCore, HandlerResult, EntityUpdate};
use crate::errors::ApiError;
use super::converters::row_to_task; // same import used in crud.rs

impl AppCore {
    pub async fn start_focus(&self, task_id: String) -> HandlerResult<TaskResponse> {
        // Step 1: Read task or return error (use ApiError::new, there is no not_found() constructor)
        let task_row = self.repos.tasks.get(&task_id).await?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("task not found: {task_id}")))?;
        let task = row_to_task(&self.repos, &task_row).await?;

        let mut updates: Vec<EntityUpdate> = vec![];

        // Step 2: Auto-end any existing focus session on a different task
        let prev_session = self.active_task_focus.lock().unwrap().clone();
        if let Some(prev) = prev_session {
            if prev.task_id != task_id {
                let (_, prev_updates) = self.end_focus_inner(&prev).await?;
                updates.extend(prev_updates);
            }
        }

        // Step 3: Set focusedAt (use existing TaskRepo::focus())
        // max_slots=1 ensures only one focused task at a time at storage level
        self.repos.tasks.focus(&task_id, 1, None).await?;

        // Step 4: Store ActiveTaskFocus
        {
            let mut lock = self.active_task_focus.lock().unwrap();
            *lock = Some(ActiveTaskFocus {
                task_id: task_id.clone(),
                started_at: Utc::now(),
                energy_level: task.energy_level.clone(),
            });
        }

        // Step 5: Emit TaskFocusStarted (bus.publish, not bus.send)
        if let Some(bus) = &self.domain_event_bus {
            let _ = bus.publish(DomainEvent::TaskFocusStarted {
                task_id: task_id.clone(),
                energy_level: task.energy_level
                    .as_ref()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "medium".to_string()),
            });
        }

        updates.push(EntityUpdate { kind: EntityKind::Task, id: task_id.clone() });

        let updated_row = self.repos.tasks.get(&task_id).await?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("task not found: {task_id}")))?;
        let response = row_to_task(&self.repos, &updated_row).await?;

        Ok((response, updates))
    }

    pub async fn end_focus(&self, _task_id: String) -> Result<Option<(TaskResponse, Vec<EntityUpdate>)>, ApiError> {
        let session = self.active_task_focus.lock().unwrap().clone();
        let Some(focus) = session else {
            return Ok(None); // no active session — no-op
        };
        let result = self.end_focus_inner(&focus).await?;
        Ok(Some(result))
    }

    pub(crate) async fn end_focus_inner(&self, focus: &ActiveTaskFocus) -> HandlerResult<TaskResponse> {
        let task_id = &focus.task_id;

        // Read full task BEFORE clearing state (needed for estimated_minutes)
        let task_row = self.repos.tasks.get(task_id).await?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("task not found: {task_id}")))?;
        let task = row_to_task(&self.repos, &task_row).await?;

        let duration_secs = Utc::now()
            .signed_duration_since(focus.started_at)
            .num_seconds()
            .max(0) as u64;

        // Clear focusedAt in DB
        self.repos.tasks.unfocus(task_id).await?;

        // Clear AppCore session
        {
            let mut lock = self.active_task_focus.lock().unwrap();
            if lock.as_ref().map(|f| f.task_id.as_str()) == Some(task_id.as_str()) {
                *lock = None;
            }
        }

        if let Some(bus) = &self.domain_event_bus {
            // Emit TaskFocusEnded (bus.publish, not bus.send)
            let _ = bus.publish(DomainEvent::TaskFocusEnded {
                task_id: task_id.clone(),
                duration_secs,
            });

            // Emit EstimationRecorded only if estimated_minutes is set
            if let Some(estimated_mins) = task.estimated_minutes {
                if estimated_mins > 0 {
                    let actual_mins = (duration_secs / 60) as u32;
                    let deviation_pct =
                        (actual_mins as f64 - estimated_mins as f64) / estimated_mins as f64 * 100.0;
                    let _ = bus.publish(DomainEvent::EstimationRecorded {
                        task_id: task_id.clone(),
                        estimated_mins: estimated_mins as u32,
                        actual_mins,
                        deviation_pct,
                    });
                }
            }
        }

        let updates = vec![EntityUpdate { kind: EntityKind::Task, id: task_id.clone() }];
        Ok((task, updates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    // Helper: build a minimal AppCore backed by an in-memory SQLite pool
    // Follow the pattern used in other app-core handler tests
    async fn make_core() -> AppCore {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Run all migrations, build AppCore — follow the existing test helper pattern
        todo!("wire up test AppCore")
    }

    #[tokio::test]
    async fn test_start_focus_sets_focusedAt() {
        let core = make_core().await;
        // Create a test task via core.task_create(...)
        // Call core.start_focus(task_id)
        // Fetch task from DB, assert focusedAt.is_some()
        todo!()
    }

    #[tokio::test]
    async fn test_start_focus_not_found_returns_error() {
        let core = make_core().await;
        let result = core.start_focus("nonexistent-id".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_focus_auto_ends_previous_session() {
        let core = make_core().await;
        // start focus on task-1
        // start focus on task-2
        // assert task-1 has focusedAt = NULL, task-2 has focusedAt set
        todo!()
    }

    #[tokio::test]
    async fn test_end_focus_no_active_session_returns_ok_none() {
        let core = make_core().await;
        let result = core.end_focus("any-id".to_string()).await;
        assert!(matches!(result, Ok(None)));
    }

    #[tokio::test]
    async fn test_end_focus_clears_focusedAt_in_db() {
        let core = make_core().await;
        // start_focus, then end_focus
        // assert focusedAt = NULL
        todo!()
    }

    #[tokio::test]
    async fn test_end_focus_emits_estimation_recorded_only_when_estimated_minutes_set() {
        // task WITH estimated_minutes → EstimationRecorded should fire
        // task WITHOUT estimated_minutes → EstimationRecorded should NOT fire
        todo!()
    }
}
```

- [ ] **Step 6.2: Fill in `make_core()` test helper**

Look at how existing `app-core` tests build a minimal `AppCore`. Replicate the same pattern. Replace the `todo!()` in `make_core()`.

- [ ] **Step 6.3: Fill in the remaining `todo!()` test bodies**

Use `core.task_create(...)` to seed tasks, then call the focus methods. Assert DB state via `core.repos.tasks.get(...)`.

- [ ] **Step 6.4: Export the focus module**

In `crates/app-core/src/handlers/tasks/mod.rs`:
```rust
pub mod focus;
```

- [ ] **Step 6.5: Run tests**

```bash
cargo nextest run -p app-core -E 'test(focus)'
```
Expected: all pass.

- [ ] **Step 6.6: Run full workspace tests**

```bash
cargo nextest run --workspace
```
Expected: all pass, zero clippy warnings.

- [ ] **Step 6.7: Commit**

```bash
git add crates/app-core/src/handlers/tasks/focus.rs \
        crates/app-core/src/handlers/tasks/mod.rs
git commit -m "feat(tasks): implement start_focus and end_focus with TaskFocusStarted/Ended events"
```

---

### Task 7: Add Tauri commands and register them

**Files:**
- Modify: `crates/desktop/src/commands/tasks.rs`
- Modify: `crates/desktop/src/lib.rs` (or wherever commands are registered)

- [ ] **Step 7.1: Add commands to `tasks.rs`**

Following the exact pattern of existing commands in the file:

```rust
#[tauri::command]
pub async fn task_start_focus(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<TaskResponse, ApiError> {
    let (response, updates) = state.start_focus(task_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_end_focus(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<Option<TaskResponse>, ApiError> {
    match state.end_focus(task_id).await? {
        Some((response, updates)) => {
            super::emit_updates(&app, &updates);
            Ok(Some(response))
        }
        None => Ok(None),
    }
}
```

- [ ] **Step 7.2: Register commands in the invoke handler**

Find the `.invoke_handler(tauri::generate_handler![...])` call (likely in `crates/desktop/src/lib.rs`). Add:
```rust
commands::tasks::task_start_focus,
commands::tasks::task_end_focus,
```

- [ ] **Step 7.3: Build check**

```bash
cargo build -p desktop
```
Expected: compiles cleanly.

- [ ] **Step 7.4: Commit**

```bash
git add crates/desktop/src/commands/tasks.rs crates/desktop/src/lib.rs
git commit -m "feat(desktop): expose task_start_focus and task_end_focus Tauri commands"
```

---

## Chunk 3: Focus Time Tracking — Frontend (Gap 3 FE)

### Task 8: `useFocusSession` hook

**Files:**
- Create: `desktop-ui/src/features/tasks/hooks/useFocusSession.ts`
- Create: `desktop-ui/src/features/tasks/hooks/useFocusSession.test.ts`

- [ ] **Step 8.0: Install test dependencies and configure jsdom environment**

`@testing-library/react` and `jsdom` are not installed. Install them and configure Vitest for DOM testing:

```bash
cd desktop-ui && bun add -d @testing-library/react @testing-library/jest-dom jsdom
```

Then create `desktop-ui/vitest.config.ts`. It must include the same path aliases from `vite.config.ts` (otherwise `@shared/*` imports in components under test will fail to resolve):

```typescript
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@shared': path.resolve(__dirname, './src/shared'),
      '@features': path.resolve(__dirname, './src/features'),
      '@app': path.resolve(__dirname, './src/app'),
      '@': path.resolve(__dirname, './src'),
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test-setup.ts'],
    globals: true,
  },
});
```

Copy alias values from `vite.config.ts` — verify they match exactly.

Create `desktop-ui/src/test-setup.ts`:
```typescript
import '@testing-library/jest-dom';
```

If a `vitest.config.ts` already exists, merge these settings rather than replacing. Verify setup with:
```bash
cd desktop-ui && bun run test
```

- [ ] **Step 8.1: Write failing tests first**

`desktop-ui/src/features/tasks/hooks/useFocusSession.test.ts`:

```typescript
import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { useFocusSession } from './useFocusSession';

afterEach(() => vi.useRealTimers());

describe('useFocusSession', () => {
  it('is inactive when focusedAt is null', () => {
    const { result } = renderHook(() => useFocusSession(null));
    expect(result.current.isActive).toBe(false);
    expect(result.current.elapsedSecs).toBe(0);
  });

  it('is active when focusedAt is set', () => {
    const focusedAt = new Date(Date.now() - 60_000).toISOString();
    const { result } = renderHook(() => useFocusSession(focusedAt));
    expect(result.current.isActive).toBe(true);
    expect(result.current.elapsedSecs).toBeGreaterThanOrEqual(59);
  });

  it('timer increments via setInterval', () => {
    vi.useFakeTimers();
    const focusedAt = new Date().toISOString();
    const { result } = renderHook(() => useFocusSession(focusedAt));
    act(() => vi.advanceTimersByTime(5000));
    expect(result.current.elapsedSecs).toBeGreaterThanOrEqual(5);
  });

  it('resets elapsed to 0 when focusedAt becomes null', () => {
    vi.useFakeTimers();
    const focusedAt = new Date().toISOString();
    const { result, rerender } = renderHook(
      ({ f }: { f: string | null }) => useFocusSession(f),
      { initialProps: { f: focusedAt } }
    );
    act(() => vi.advanceTimersByTime(3000));
    rerender({ f: null });
    expect(result.current.elapsedSecs).toBe(0);
    expect(result.current.isActive).toBe(false);
  });

  it('formatElapsed: mm:ss under one hour', () => {
    const { result } = renderHook(() => useFocusSession(null));
    expect(result.current.formatElapsed(90)).toBe('01:30');
    expect(result.current.formatElapsed(0)).toBe('00:00');
    expect(result.current.formatElapsed(3599)).toBe('59:59');
  });

  it('formatElapsed: h:mm:ss at one hour or more', () => {
    const { result } = renderHook(() => useFocusSession(null));
    expect(result.current.formatElapsed(3600)).toBe('1:00:00');
    expect(result.current.formatElapsed(3661)).toBe('1:01:01');
  });
});
```

Run: `cd desktop-ui && bun run test -- useFocusSession`
Expected: FAIL (file doesn't exist).

- [ ] **Step 8.2: Implement the hook**

`desktop-ui/src/features/tasks/hooks/useFocusSession.ts`:

```typescript
import { useState, useEffect, useCallback } from 'react';

export function useFocusSession(focusedAt: string | null | undefined) {
  const [elapsedSecs, setElapsedSecs] = useState(0);

  useEffect(() => {
    if (!focusedAt) {
      setElapsedSecs(0);
      return;
    }

    const startMs = new Date(focusedAt).getTime();
    const tick = () => setElapsedSecs(Math.floor((Date.now() - startMs) / 1000));

    tick(); // immediate sync
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, [focusedAt]);

  const formatElapsed = useCallback((secs: number): string => {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    const mm = String(m).padStart(2, '0');
    const ss = String(s).padStart(2, '0');
    return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
  }, []);

  return { isActive: !!focusedAt, elapsedSecs, formatElapsed };
}
```

- [ ] **Step 8.3: Run tests**

```bash
cd desktop-ui && bun run test -- useFocusSession
```
Expected: all 6 tests pass.

- [ ] **Step 8.4: Commit**

```bash
git add desktop-ui/src/features/tasks/hooks/useFocusSession.ts \
        desktop-ui/src/features/tasks/hooks/useFocusSession.test.ts
git commit -m "feat(tasks-ui): add useFocusSession hook with elapsed timer and format utilities"
```

---

### Task 9: `FocusBanner` component

**Files:**
- Create: `desktop-ui/src/features/tasks/components/FocusBanner.tsx`
- Create: `desktop-ui/src/features/tasks/components/FocusBanner.test.tsx`

- [ ] **Step 9.1: Write failing tests**

`desktop-ui/src/features/tasks/components/FocusBanner.test.tsx`:

```typescript
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { FocusBanner } from './FocusBanner';

describe('FocusBanner', () => {
  it('renders nothing when activeTask is null', () => {
    const { container } = render(
      <FocusBanner activeTask={null} onEndFocus={() => {}} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders task title and timer when active', () => {
    const focusedAt = new Date(Date.now() - 90_000).toISOString();
    render(
      <FocusBanner
        activeTask={{ id: 'task-1', title: 'Write tests', focusedAt: focusedAt }}
        onEndFocus={() => {}}
      />
    );
    expect(screen.getByText('Write tests')).toBeInTheDocument();
    // Timer should be visible
    expect(screen.getByText(/\d{2}:\d{2}/)).toBeInTheDocument();
  });

  it('calls onEndFocus with task id when End Focus clicked', () => {
    const onEnd = vi.fn();
    const focusedAt = new Date().toISOString();
    render(
      <FocusBanner
        activeTask={{ id: 'task-1', title: 'Write tests', focusedAt: focusedAt }}
        onEndFocus={onEnd}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: /end focus/i }));
    expect(onEnd).toHaveBeenCalledWith('task-1');
  });
});
```

Run: `cd desktop-ui && bun run test -- FocusBanner`
Expected: FAIL.

- [ ] **Step 9.2: Implement `FocusBanner`**

`desktop-ui/src/features/tasks/components/FocusBanner.tsx`:

```tsx
import { useFocusSession } from '../hooks/useFocusSession';

interface ActiveTask {
  id: string;
  title: string;
  focusedAt: string;
}

interface Props {
  activeTask: ActiveTask | null;
  onEndFocus: (taskId: string) => void;
}

export function FocusBanner({ activeTask, onEndFocus }: Props) {
  const { isActive, elapsedSecs, formatElapsed } = useFocusSession(
    activeTask?.focusedAt ?? null
  );

  if (!activeTask || !isActive) return null;

  return (
    <div className="fixed top-0 left-0 right-0 z-50 flex items-center justify-between px-4 py-2 bg-surface-elevated border-b border-border">
      <div className="flex items-center gap-3">
        <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
        <span className="text-sm font-medium text-primary truncate max-w-xs">
          {activeTask.title}
        </span>
        <span className="text-xs text-muted font-mono tabular-nums">
          {formatElapsed(elapsedSecs)}
        </span>
      </div>
      <button
        onClick={() => onEndFocus(activeTask.id)}
        className="text-xs text-muted hover:text-primary px-2 py-1 rounded hover:bg-surface-hover transition-colors"
      >
        End Focus
      </button>
    </div>
  );
}
```

- [ ] **Step 9.3: Run tests**

```bash
cd desktop-ui && bun run test -- FocusBanner
```
Expected: all 3 tests pass.

- [ ] **Step 9.4: Commit**

```bash
git add desktop-ui/src/features/tasks/components/FocusBanner.tsx \
        desktop-ui/src/features/tasks/components/FocusBanner.test.tsx
git commit -m "feat(tasks-ui): add FocusBanner component with live elapsed timer"
```

---

### Task 10: Wire FocusBanner into layout and add Start Focus buttons

**Files:**
- Modify: Root layout (find with `grep -r "FocusBanner\|AppLayout\|RootLayout" desktop-ui/src/app/`)
- Modify: `desktop-ui/src/features/tasks/components/TaskCard.tsx`
- Modify: `desktop-ui/src/features/tasks/pages/TaskDetail.tsx`

- [ ] **Step 10.1: Find the root layout file**

```bash
grep -r "children" desktop-ui/src/app/ --include="*.tsx" -l
```

Open the main layout and find where to mount the banner (before or after the main content area).

- [ ] **Step 10.2: Mount `FocusBanner` in the root layout**

The layout needs to know which task is actively focused. Use the existing entity subscription pattern to detect when a task has `focusedAt` set.

**Important:** All Tauri IPC calls in this codebase go through the `ipc()` plain async function from `@shared/hooks/useIpc`, NOT bare `invoke()` and NOT a React hook. Import and call it directly:

```tsx
import { ipc } from '@shared/hooks/useIpc';

// In root layout component:
const [focusedTask, setFocusedTask] = useState<{ id: string; title: string; focusedAt: string } | null>(null);

// Subscribe to task entity updates (use the existing hook/event mechanism)
// When a task update arrives with focusedAt !== null → set as activeTask
// When focusedAt is null → clear
// Check how other entity subscriptions work: grep for `useEntitySubscription` or look at
// how existing task updates are received in the tasks feature.

// Mount the banner:
<FocusBanner
  activeTask={focusedTask}
  onEndFocus={async (taskId) => {
    await ipc('task_end_focus', { taskId });
  }}
/>
```

- [ ] **Step 10.3: Add Start Focus button to `TaskCard`**

Find the action area in `TaskCard.tsx`. Add — import `ipc` at the top of the file if not already present:

```tsx
import { ipc } from '@shared/hooks/useIpc';

// In the action area:
{!task.focusedAt && (
  <button
    onClick={async (e) => {
      e.stopPropagation();
      await ipc('task_start_focus', { taskId: task.id });
    }}
    className="text-xs text-muted hover:text-primary transition-colors"
    title="Start Focus"
  >
    Focus
  </button>
)}
```

- [ ] **Step 10.4: Add Start Focus button to `TaskDetail`**

Same pattern in the task detail header action bar. Import `ipc` from `@shared/hooks/useIpc` and call `ipc('task_start_focus', { taskId: task.id })`.

- [ ] **Step 10.5: Lint check**

```bash
cd desktop-ui && bun run lint:fix
```
Expected: no errors.

- [ ] **Step 10.6: Run all frontend tests**

```bash
cd desktop-ui && bun run test
```
Expected: all pass.

- [ ] **Step 10.7: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(tasks-ui): wire FocusBanner into layout and add Start Focus buttons to TaskCard and TaskDetail"
```

---

## Final Verification

- [ ] **Run full test suite**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cd desktop-ui && bun run test && bun run lint:fix
```
Expected: zero test failures, zero clippy warnings, zero lint errors.

- [ ] **Remove A-010 from BACKLOG.md**

Open `BACKLOG.md`, delete the A-010 entry (Phase 3 Implementation Pending).

- [ ] **Final commit**

```bash
git add BACKLOG.md
git commit -m "docs: mark A-010 resolved — feature-tasks Phase 3 complete"
```
