# Coaching Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a dedicated `/coaching` page with tabbed layout (Overview, Patterns, History) that surfaces behavioral patterns, coaching health, intervention history, and retroactive feedback.

**Architecture:** Backend adds a `coaching_intervention_log` SQLite table + repo for persistent intervention history. Frontend adds a new `features/coaching/` feature with three tab pages following the `FinanceLayout` tabbed pattern. All existing coaching commands are reused; one new command (`coaching_intervention_log`) and two handler modifications enable the History tab.

**Tech Stack:** Rust (SQLite via sqlx), React 19, Tauri 2, Tailwind v4, lucide-react icons

**Spec:** `docs/superpowers/specs/2026-03-21-coaching-dashboard-design.md`

---

## File Map

### New Files

| File | Responsibility |
|------|---------------|
| `crates/storage/src/repos/coaching_intervention_log.rs` | Repo: insert, update_feedback, list_recent |
| `desktop-ui/src/features/coaching/index.ts` | Barrel exports for all pages + layout |
| `desktop-ui/src/features/coaching/components/CoachingLayout.tsx` | Tabbed layout (mirrors FinanceLayout) |
| `desktop-ui/src/features/coaching/components/CoachingHealthCard.tsx` | Receptivity gauge + rate limits |
| `desktop-ui/src/features/coaching/components/PatternCard.tsx` | Single pattern display card |
| `desktop-ui/src/features/coaching/components/InterventionRow.tsx` | Intervention row + retroactive feedback |
| `desktop-ui/src/features/coaching/pages/OverviewPage.tsx` | Overview tab: health + previews |
| `desktop-ui/src/features/coaching/pages/PatternsPage.tsx` | Patterns grid |
| `desktop-ui/src/features/coaching/pages/HistoryPage.tsx` | Intervention history + feedback |

### Modified Files

| File | Change |
|------|--------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Append `coaching_intervention_log` table |
| `crates/cognitive/src/repos/mod.rs` | Bump migration version 9 → 10 |
| `crates/storage/src/repos/mod.rs` | Add `pub mod coaching_intervention_log;` |
| `crates/storage/src/lib.rs` | Re-export `CoachingInterventionLogRepo` |
| `crates/desktop-shared/src/cognitive_commands.rs` | Add `InterventionLogResponse` struct |
| `crates/app-core/src/state.rs` | Add `coaching_intervention_log_repo` field |
| `crates/app-core/src/init/coaching.rs` | Instantiate repo, pass to `CoachingResult` |
| `crates/app-core/src/handlers/coaching.rs` | Add `coaching_intervention_log` handler; modify `coaching_submit_feedback` + `coaching_report_ignored` |
| `crates/feature-coaching/src/service.rs` | Persist intervention to DB after `record_delivery` (both main + debrief paths) |
| `crates/desktop/src/commands/cognitive.rs` | Add command + DEV_COMMANDS + dispatch_dev |
| `desktop-ui/src/shared/types/common.ts` | Add `"Coaching"` to `SidebarItem` union |
| `desktop-ui/src/app/layouts/Sidebar.tsx` | Add coaching sidebar entry |
| `desktop-ui/src/app/layouts/AppShell.tsx` | Add `/coaching` to `activeSidebarItem` |
| `desktop-ui/src/app/router.tsx` | Add 3 coaching routes |
| `desktop-ui/src/features/projects/components/overview/CoachingCard.tsx` | Make clickable → navigate to `/coaching` |

---

## Task 1: Add `coaching_intervention_log` Table

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs:L68` (version bump)

- [ ] **Step 1: Append table DDL to migration file**

Add to the end of `crates/cognitive/migrations/001_cognitive_tables.sql`:

```sql
-- Coaching intervention history (persistent log for dashboard + retroactive feedback)
CREATE TABLE IF NOT EXISTS coaching_intervention_log (
    id TEXT PRIMARY KEY,
    intervention_type TEXT NOT NULL,
    message TEXT NOT NULL,
    trigger_name TEXT NOT NULL,
    feedback TEXT,
    delivered_at TEXT NOT NULL,
    feedback_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_coaching_intervention_log_delivered
    ON coaching_intervention_log(delivered_at DESC);
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, change the `FeatureMigration` version:

```rust
// Before
version: 9,

// After
version: 10,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p cognitive`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(coaching): add coaching_intervention_log table (migration v10)"
```

---

## Task 2: Add `CoachingInterventionLogRepo`

**Files:**
- Create: `crates/storage/src/repos/coaching_intervention_log.rs`
- Modify: `crates/storage/src/repos/mod.rs`
- Modify: `crates/storage/src/lib.rs`

- [ ] **Step 1: Write the repo test**

Create `crates/storage/src/repos/coaching_intervention_log.rs`:

```rust
//! Repository for the `coaching_intervention_log` table (cognitive migration v10).

use sqlx::SqlitePool;

use crate::error::StorageError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InterventionLogRow {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub feedback: Option<String>,
    pub delivered_at: String,
    pub feedback_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CoachingInterventionLogRepo {
    pool: SqlitePool,
}

impl CoachingInterventionLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        id: &str,
        intervention_type: &str,
        message: &str,
        trigger_name: &str,
        delivered_at: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT OR IGNORE INTO coaching_intervention_log
                (id, intervention_type, message, trigger_name, delivered_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id)
        .bind(intervention_type)
        .bind(message)
        .bind(trigger_name)
        .bind(delivered_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_feedback(
        &self,
        id: &str,
        feedback: &str,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE coaching_intervention_log
             SET feedback = ?2, feedback_at = datetime('now')
             WHERE id = ?1",
        )
        .bind(id)
        .bind(feedback)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_recent(&self, limit: i64) -> Result<Vec<InterventionLogRow>, StorageError> {
        let rows = sqlx::query_as::<_, InterventionLogRow>(
            "SELECT id, intervention_type, message, trigger_name, feedback, delivered_at, feedback_at
             FROM coaching_intervention_log
             ORDER BY delivered_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> CoachingInterventionLogRepo {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS coaching_intervention_log (
                id TEXT PRIMARY KEY,
                intervention_type TEXT NOT NULL,
                message TEXT NOT NULL,
                trigger_name TEXT NOT NULL,
                feedback TEXT,
                delivered_at TEXT NOT NULL,
                feedback_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        CoachingInterventionLogRepo::new(pool)
    }

    #[tokio::test]
    async fn test_insert_and_list() {
        let repo = setup().await;
        repo.insert("int-1", "ChatMessage", "Take a break", "distraction_streak", "2026-03-21T10:00:00Z")
            .await
            .unwrap();

        let rows = repo.list_recent(10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "int-1");
        assert_eq!(rows[0].trigger_name, "distraction_streak");
        assert!(rows[0].feedback.is_none());
    }

    #[tokio::test]
    async fn test_update_feedback() {
        let repo = setup().await;
        repo.insert("int-2", "DashboardCard", "Focus now", "overdue_pressure", "2026-03-21T11:00:00Z")
            .await
            .unwrap();

        let updated = repo.update_feedback("int-2", "helpful").await.unwrap();
        assert!(updated);

        let rows = repo.list_recent(10).await.unwrap();
        assert_eq!(rows[0].feedback.as_deref(), Some("helpful"));
        assert!(rows[0].feedback_at.is_some());
    }

    #[tokio::test]
    async fn test_update_feedback_nonexistent() {
        let repo = setup().await;
        let updated = repo.update_feedback("no-such-id", "helpful").await.unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn test_insert_duplicate_is_ignored() {
        let repo = setup().await;
        repo.insert("dup-1", "ChatMessage", "msg1", "trigger1", "2026-03-21T12:00:00Z")
            .await
            .unwrap();
        repo.insert("dup-1", "ChatMessage", "msg2", "trigger2", "2026-03-21T13:00:00Z")
            .await
            .unwrap();

        let rows = repo.list_recent(10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].message, "msg1"); // first insert wins
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/storage/src/repos/mod.rs`, add:

```rust
pub mod coaching_intervention_log;
```

In `crates/storage/src/lib.rs`, add to the re-exports:

```rust
pub use repos::coaching_intervention_log::CoachingInterventionLogRepo;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p storage -E 'test(coaching_intervention)'`
Expected: 4 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/storage/src/repos/coaching_intervention_log.rs crates/storage/src/repos/mod.rs crates/storage/src/lib.rs
git commit -m "feat(storage): add CoachingInterventionLogRepo for persistent intervention history"
```

---

## Task 3: Add `InterventionLogResponse` DTO

**Files:**
- Modify: `crates/desktop-shared/src/cognitive_commands.rs`

- [ ] **Step 1: Add the response struct**

In `crates/desktop-shared/src/cognitive_commands.rs`, after `RouterStatusResponse`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterventionLogResponse {
    pub id: String,
    pub intervention_type: String,
    pub message: String,
    pub trigger_name: String,
    pub feedback: Option<String>,
    pub delivered_at: String,
    pub feedback_at: Option<String>,
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/cognitive_commands.rs
git commit -m "feat(desktop-shared): add InterventionLogResponse DTO for coaching history"
```

---

## Task 4: Wire Repo Into AppCore and Modify Handlers

**Files:**
- Modify: `crates/app-core/src/state.rs:L70` (add field)
- Modify: `crates/app-core/src/init/coaching.rs` (instantiate repo)
- Modify: `crates/app-core/src/handlers/coaching.rs` (add handler, modify submit + ignored)

- [ ] **Step 1: Add repo field to AppCore state**

In `crates/app-core/src/state.rs`, after the `feedback_tracker` field (line ~70), add:

```rust
    pub coaching_intervention_log_repo: Option<storage::CoachingInterventionLogRepo>,
```

And add a helper method near the other coaching accessor methods:

```rust
    pub fn coaching_log_repo(&self) -> Result<&storage::CoachingInterventionLogRepo, ApiError> {
        self.coaching_intervention_log_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }
```

- [ ] **Step 2: Add repo to CoachingResult and init**

In `crates/app-core/src/init/coaching.rs`, add to `CoachingResult`:

```rust
    pub coaching_intervention_log_repo: Option<storage::CoachingInterventionLogRepo>,
```

In the `init_coaching` function, inside the `if mode == common::AppMode::Desktop` block, after `let coaching_repo = ...` (line ~50), add:

```rust
        let intervention_log_repo = storage::CoachingInterventionLogRepo::new(storage_pool.inner().clone());
```

Add the field to `CoachingResult` struct and return it. In the desktop branch, return `Some(intervention_log_repo)`. In the `else` (server mode) block, return `None`.

Note: `CoachingInterventionLogRepo` is NOT added to the `Repos` aggregate — it follows the same pattern as `CoachingStrategyRepo`, which is also instantiated directly in `init_coaching.rs` (line ~50) rather than through `Repos`.

Then in the `AppCore` builder (wherever `CoachingResult` fields are spread into `AppCore`), assign `coaching_intervention_log_repo` from the result.

- [ ] **Step 3: Add `coaching_intervention_log` handler**

In `crates/app-core/src/handlers/coaching.rs`, add:

```rust
    pub async fn coaching_intervention_log(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<InterventionLogResponse>, ApiError> {
        let repo = self.coaching_log_repo()?;
        let rows = repo
            .list_recent(limit.unwrap_or(50))
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", &e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| InterventionLogResponse {
                id: r.id,
                intervention_type: r.intervention_type,
                message: r.message,
                trigger_name: r.trigger_name,
                feedback: r.feedback,
                delivered_at: r.delivered_at,
                feedback_at: r.feedback_at,
            })
            .collect())
    }
```

- [ ] **Step 4: Modify `coaching_submit_feedback` for DB persistence**

In `coaching_submit_feedback`, after the existing `record_explicit` block and before the router dismissal logic, add a DB write that works regardless of whether the intervention is still in-memory:

```rust
        // Persist feedback to intervention log (works for both live and retroactive feedback)
        if let Ok(repo) = self.coaching_log_repo() {
            let feedback_str = match feedback_response {
                bus::FeedbackResponse::Helpful => "helpful",
                bus::FeedbackResponse::Dismissed => "dismissed",
                bus::FeedbackResponse::StopSuggesting => "stop",
            };
            if let Err(e) = repo.update_feedback(&intervention_id, feedback_str).await {
                debug!("failed to persist coaching feedback: {e}");
            }
        }
```

- [ ] **Step 5: Modify `coaching_report_ignored` for DB persistence**

In `coaching_report_ignored`, after the atomic increment, add:

```rust
        // Persist "ignored" feedback to intervention log
        if let Ok(repo) = self.coaching_log_repo() {
            if let Err(e) = repo.update_feedback(&intervention_id, "ignored").await {
                debug!("failed to persist ignored feedback: {e}");
            }
        }
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build -p app-core`
Expected: compiles with no errors

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/state.rs crates/app-core/src/init/coaching.rs crates/app-core/src/handlers/coaching.rs
git commit -m "feat(app-core): wire coaching intervention log repo + handlers"
```

---

## Task 5: Persist Interventions in CoachingService

**Files:**
- Modify: `crates/feature-coaching/src/service.rs:L29-L39` (add repo param)
- Modify: `crates/feature-coaching/src/service.rs:L159-L170` (persist after delivery)
- Modify: `crates/app-core/src/init/coaching.rs` (pass repo to service)

- [ ] **Step 1: Add repo parameter to `CoachingService::start`**

In `crates/feature-coaching/src/service.rs`, add `storage` to `Cargo.toml` dependencies if not already present. Then add a new parameter to `CoachingService::start`:

```rust
    pub fn start(
        mut event_rx: broadcast::Receiver<DomainEvent>,
        accumulator: Arc<Mutex<SignalAccumulator>>,
        detector: Arc<Mutex<PatternDetector>>,
        router: Arc<Mutex<InterventionRouter>>,
        feedback: Arc<Mutex<FeedbackTracker>>,
        situation: Arc<Mutex<UserSituation>>,
        reasoner: Arc<dyn CoachingReasonerHandler>,
        intervention_tx: mpsc::Sender<DeliveredIntervention>,
        intervention_log: Option<storage::CoachingInterventionLogRepo>,  // NEW
        cancel: CancellationToken,
    ) -> Self {
```

- [ ] **Step 2: Create a helper to persist interventions**

Add a helper function in `service.rs` to avoid duplicating the persist logic across both delivery paths:

```rust
/// Persist a delivered intervention to the coaching_intervention_log table.
async fn persist_intervention(
    log_repo: &Option<storage::CoachingInterventionLogRepo>,
    intervention: &DeliveredIntervention,
) {
    if let Some(ref repo) = log_repo {
        let type_str = serde_json::to_value(&intervention.intervention_type)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| format!("{:?}", intervention.intervention_type));
        if let Err(e) = repo.insert(
            &intervention.id,
            &type_str,
            &intervention.message,
            &intervention.trigger_name,
            &intervention.delivered_at.to_rfc3339(),
        ).await {
            warn!("failed to persist intervention to log: {e}");
        }
    }
}
```

- [ ] **Step 3: Patch the main delivery path**

In the `RoutingResult::Delivered` arm (around line 159-170), after `record_delivery` and before `intervention_tx.send`, add:

```rust
                                    // Persist to intervention log
                                    persist_intervention(&intervention_log, &intervention).await;
```

- [ ] **Step 4: Patch the focus debrief delivery path**

In the focus debrief delivery path (around line 79-84, inside `if let Some(intervention) = debrief`), after `record_delivery` and before `intervention_tx.send`, add the same call:

```rust
                                    if let Some(intervention) = debrief {
                                        {
                                            let mut fb = feedback.lock().await;
                                            fb.record_delivery(&intervention);
                                        }
                                        persist_intervention(&intervention_log, &intervention).await;
                                        let _ = intervention_tx.send(intervention).await;
                                    }
```

- [ ] **Step 5: Update existing tests in service.rs**

The `#[cfg(test)] mod tests` in `service.rs` has two tests that call `CoachingService::start` with 9 positional args. Add `None` for the new `intervention_log` parameter (between `intervention_tx` and `cancel`):

```rust
// In both test_coaching_service_processes_distraction_events and test_coaching_service_stops_gracefully:
let service = CoachingService::start(
    bus.subscribe(),
    accumulator,
    detector,
    router,
    feedback,
    situation,
    reasoner,
    tx,
    None,  // NEW — no intervention log repo in tests
    cancel,
);
```

- [ ] **Step 6: Update the call site in init_coaching.rs**

In `crates/app-core/src/init/coaching.rs`, update the `CoachingService::start` call to pass the repo:

```rust
        let coaching_service = feature_coaching::CoachingService::start(
            domain_event_bus.subscribe(),
            signal_accumulator.clone(),
            pattern_detector.clone(),
            intervention_router.clone(),
            feedback_tracker.clone(),
            user_situation.clone(),
            coaching_reasoner,
            intervention_tx.clone(),
            Some(storage::CoachingInterventionLogRepo::new(storage_pool.inner().clone())),  // NEW
            coaching_cancel,
        );
```

- [ ] **Step 7: Verify it compiles and tests pass**

Run: `cargo build -p feature-coaching -p app-core && cargo nextest run -p feature-coaching`
Expected: compiles with no errors, existing tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/feature-coaching/src/service.rs crates/app-core/src/init/coaching.rs
git commit -m "feat(coaching): persist delivered interventions to coaching_intervention_log"
```

---

## Task 6: Add Tauri Command + DEV_COMMANDS

**Files:**
- Modify: `crates/desktop/src/commands/cognitive.rs`

- [ ] **Step 1: Add the Tauri command**

In `crates/desktop/src/commands/cognitive.rs`, after `coaching_report_ignored`, add:

```rust
#[tauri::command]
pub async fn coaching_intervention_log(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<desktop_shared::cognitive_commands::InterventionLogResponse>, ApiError> {
    state.coaching_intervention_log(limit).await
}
```

- [ ] **Step 2: Add to DEV_COMMANDS**

In the `DEV_COMMANDS` array (gated `#[cfg(test)]`), add:

```rust
    "coaching_intervention_log",
```

- [ ] **Step 3: Add to dispatch_dev**

In the `dispatch_dev` match (gated `#[cfg(debug_assertions)]`), add:

```rust
        "coaching_intervention_log" => dev::val(
            core.coaching_intervention_log(dev::get(body, "limit")).await,
        ),
```

- [ ] **Step 4: Register in generate_handler!**

In `crates/desktop/src/main.rs`, find the `generate_handler!` macro and add `coaching_intervention_log` to the list.

- [ ] **Step 5: Verify it compiles and DEV_COMMANDS test passes**

Run: `cargo build -p desktop && cargo nextest run -p klyntbot -E 'test(dev_server_covers)'`
Expected: compiles and test passes (confirms DEV_COMMANDS coverage)

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/commands/cognitive.rs crates/desktop/src/main.rs
git commit -m "feat(desktop): add coaching_intervention_log Tauri command + dev server"
```

---

## Task 7: Frontend — Sidebar + Routing Shell

**Files:**
- Modify: `desktop-ui/src/shared/types/common.ts`
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`
- Modify: `desktop-ui/src/app/layouts/AppShell.tsx`
- Create: `desktop-ui/src/features/coaching/components/CoachingLayout.tsx`
- Create: `desktop-ui/src/features/coaching/index.ts`
- Modify: `desktop-ui/src/app/router.tsx`

- [ ] **Step 1: Add `"Coaching"` to SidebarItem union**

In `desktop-ui/src/shared/types/common.ts`, add to the `SidebarItem` union:

```ts
  | "Coaching"
```

- [ ] **Step 2: Add sidebar entry**

In `desktop-ui/src/app/layouts/Sidebar.tsx`, import `Sparkles` from `lucide-react` and add to the `items` array after Finance:

```tsx
  { key: "Coaching",    icon: Sparkles,        path: "/coaching" },
```

- [ ] **Step 3: Add activeSidebarItem branch**

In `desktop-ui/src/app/layouts/AppShell.tsx`, in the `activeSidebarItem` memo, add before `return "Dashboard"`:

```tsx
  if (path.startsWith("/coaching")) return "Coaching";
```

- [ ] **Step 4: Create CoachingLayout**

Create `desktop-ui/src/features/coaching/components/CoachingLayout.tsx`:

```tsx
import { useLocation, useNavigate } from "react-router";

interface CoachingLayoutProps {
  children: React.ReactNode;
}

const subNav = [
  { label: "Overview", path: "/coaching" },
  { label: "Patterns", path: "/coaching/patterns" },
  { label: "History", path: "/coaching/history" },
];

export function CoachingLayout({ children }: CoachingLayoutProps) {
  const navigate = useNavigate();
  const location = useLocation();
  const currentPath = location.pathname;

  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5" role="tablist">
          {subNav.map((item) => {
            const isActive = currentPath === item.path;
            return (
              <button
                type="button"
                key={item.path}
                role="tab"
                aria-selected={isActive}
                onClick={() => navigate(item.path)}
                className={`flex-1 py-2 rounded-xl text-[13px] font-light transition-all duration-200 ${
                  isActive
                    ? "glass-button-active text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent"
                }`}
              >
                {item.label}
              </button>
            );
          })}
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-4">{children}</div>
    </div>
  );
}
```

- [ ] **Step 5: Create placeholder pages and barrel export**

Create `desktop-ui/src/features/coaching/pages/OverviewPage.tsx`:

```tsx
export function CoachingOverviewPage() {
  return <div className="text-muted-foreground text-sm">Coaching Overview — coming soon</div>;
}
```

Create `desktop-ui/src/features/coaching/pages/PatternsPage.tsx`:

```tsx
export function PatternsPage() {
  return <div className="text-muted-foreground text-sm">Patterns — coming soon</div>;
}
```

Create `desktop-ui/src/features/coaching/pages/HistoryPage.tsx`:

```tsx
export function HistoryPage() {
  return <div className="text-muted-foreground text-sm">History — coming soon</div>;
}
```

Create `desktop-ui/src/features/coaching/index.ts`:

```ts
export { CoachingLayout } from "./components/CoachingLayout";
export { CoachingOverviewPage } from "./pages/OverviewPage";
export { PatternsPage } from "./pages/PatternsPage";
export { HistoryPage } from "./pages/HistoryPage";
```

- [ ] **Step 6: Add routes**

In `desktop-ui/src/app/router.tsx`, add lazy imports at the top, after the finance imports (matching the existing `lazy(() => import(...).then(m => ...))` pattern):

```tsx
// ── Coaching Feature ────────────────────────────────────────
const CoachingLayout = lazy(() =>
  import("../features/coaching").then((m) => ({ default: m.CoachingLayout })),
);
const CoachingOverviewPage = lazy(() =>
  import("../features/coaching").then((m) => ({ default: m.CoachingOverviewPage })),
);
const CoachingPatternsPage = lazy(() =>
  import("../features/coaching").then((m) => ({ default: m.PatternsPage })),
);
const CoachingHistoryPage = lazy(() =>
  import("../features/coaching").then((m) => ({ default: m.HistoryPage })),
);
```

Then add the routes **inside** the `{ element: <AppShell />, children: [...] }` block (alongside the finance routes, around line 201):

```tsx
      { path: "/coaching", element: <CoachingLayout><CoachingOverviewPage /></CoachingLayout> },
      { path: "/coaching/patterns", element: <CoachingLayout><CoachingPatternsPage /></CoachingLayout> },
      { path: "/coaching/history", element: <CoachingLayout><CoachingHistoryPage /></CoachingLayout> },
```

**Important:** These must be nested inside `AppShell` `children`, not at the top level — otherwise the sidebar won't render.

- [ ] **Step 7: Verify the shell renders**

Run: `cd desktop-ui && bun run build`
Expected: builds with no errors

Run: `cd desktop-ui && bun run lint`
Expected: no lint errors

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/shared/types/common.ts desktop-ui/src/app/layouts/Sidebar.tsx desktop-ui/src/app/layouts/AppShell.tsx desktop-ui/src/features/coaching/ desktop-ui/src/app/router.tsx
git commit -m "feat(coaching): add coaching page shell with tabbed layout + sidebar entry"
```

---

## Task 8: Overview Page — Coaching Health Card

**Files:**
- Create: `desktop-ui/src/features/coaching/components/CoachingHealthCard.tsx`
- Modify: `desktop-ui/src/features/coaching/pages/OverviewPage.tsx`

- [ ] **Step 1: Create CoachingHealthCard**

Create `desktop-ui/src/features/coaching/components/CoachingHealthCard.tsx`:

```tsx
import { useQuery } from "@shared/hooks/useQuery";

// Types matching the Rust DTOs (camelCase from serde)
interface UserSituation {
  energyLevel: number;
  focusState: number;
  deadlinePressure: number;
  distractionRisk: number;
  coachingReceptivity: number;
  taskAvoidanceDetected: boolean;
  hoursActiveToday: number;
  minsSinceBreak: number;
  hourOfDay: number;
  recentContextSwitches: number;
}

interface RouterStatus {
  hourlyCount: number;
  hourlyLimit: number;
  dailyCount: number;
  dailyLimit: number;
}

interface StrategyFeedback {
  strategyType: string;
  domain: string;
  timesUsed: number;
  acceptanceRate: number;
  effectiveness: number;
  behavioralPositive: number;
  behavioralNegative: number;
}

export function CoachingHealthCard() {
  const { data: situation } = useQuery<UserSituation>("coaching_situation", undefined, undefined, 5000);
  const { data: router } = useQuery<RouterStatus>("coaching_router_status");
  const { data: strategies } = useQuery<StrategyFeedback[]>("coaching_feedback_stats", undefined, []);

  const receptivity = situation?.coachingReceptivity ?? 0;
  const pct = Math.round(receptivity * 100);

  const avgAcceptance = strategies && strategies.length > 0
    ? strategies.reduce((sum, s) => sum + s.acceptanceRate, 0) / strategies.length
    : 0;

  return (
    <div className="glass-card rounded-xl p-5">
      <h2 className="text-[13px] font-medium text-muted-foreground mb-4">Coaching Health</h2>

      <div className="grid grid-cols-3 gap-6">
        {/* Receptivity gauge */}
        <div className="flex flex-col items-center gap-2">
          <div className="relative w-16 h-16">
            <svg viewBox="0 0 36 36" className="w-full h-full -rotate-90">
              <circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor"
                className="text-accent" strokeWidth="3" />
              <circle cx="18" cy="18" r="15.5" fill="none" stroke="currentColor"
                className="text-primary" strokeWidth="3"
                strokeDasharray={`${pct} ${100 - pct}`}
                strokeLinecap="round" />
            </svg>
            <span className="absolute inset-0 flex items-center justify-center text-sm font-medium tabular-nums">
              {pct}
            </span>
          </div>
          <span className="text-[10px] text-muted-foreground">Receptivity</span>
        </div>

        {/* Rate limits */}
        <div className="flex flex-col gap-2">
          <div className="flex items-baseline justify-between">
            <span className="text-[10px] text-muted-foreground">Hourly</span>
            <span className="text-xs tabular-nums">
              {router?.hourlyCount ?? 0} / {router?.hourlyLimit ?? 3}
            </span>
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-[10px] text-muted-foreground">Daily</span>
            <span className="text-xs tabular-nums">
              {router?.dailyCount ?? 0} / {router?.dailyLimit ?? 5}
            </span>
          </div>
        </div>

        {/* Acceptance rate */}
        <div className="flex flex-col items-center gap-2">
          <span className="text-2xl font-light tabular-nums">{Math.round(avgAcceptance * 100)}%</span>
          <span className="text-[10px] text-muted-foreground">Acceptance</span>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Update OverviewPage with health card + previews**

Replace `desktop-ui/src/features/coaching/pages/OverviewPage.tsx`:

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { useNavigate } from "react-router";
import { CoachingHealthCard } from "../components/CoachingHealthCard";
import { formatTime } from "@shared/lib/dates";

interface DetectedPattern {
  name: string;
  confidence: number;
  signalCount: number;
  description: string;
  domain: string;
}

interface InterventionLog {
  id: string;
  interventionType: string;
  message: string;
  triggerName: string;
  feedback: string | null;
  deliveredAt: string;
  feedbackAt: string | null;
}

export function CoachingOverviewPage() {
  const navigate = useNavigate();
  const { data: patterns } = useQuery<DetectedPattern[]>("coaching_patterns", undefined, []);
  const { data: history } = useQuery<InterventionLog[]>("coaching_intervention_log", { limit: 5 }, []);

  return (
    <div className="flex flex-col gap-4 max-w-3xl">
      <CoachingHealthCard />

      {/* Recent Patterns preview */}
      <div className="glass-card rounded-xl p-5">
        <div className="flex items-baseline justify-between mb-3">
          <h2 className="text-[13px] font-medium text-muted-foreground">Recent Patterns</h2>
          {patterns && patterns.length > 0 && (
            <button type="button" onClick={() => navigate("/coaching/patterns")}
              className="text-[10px] text-primary hover:underline">View all</button>
          )}
        </div>
        {patterns && patterns.length > 0 ? (
          <div className="grid grid-cols-2 gap-3">
            {patterns.slice(0, 4).map((p) => (
              <div key={p.name} className="rounded-lg bg-accent/30 p-3">
                <p className="text-xs font-medium text-foreground">{p.name}</p>
                <p className="text-[10px] text-muted-foreground mt-1 line-clamp-2">{p.description}</p>
                <p className="text-[10px] text-dim mt-1 tabular-nums">{Math.round(p.confidence * 100)}% confidence</p>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-[11px] text-muted-foreground">
            No patterns detected yet. Patterns emerge as the coaching system observes your work habits over time.
          </p>
        )}
      </div>

      {/* Recent Interventions preview */}
      <div className="glass-card rounded-xl p-5">
        <div className="flex items-baseline justify-between mb-3">
          <h2 className="text-[13px] font-medium text-muted-foreground">Recent Interventions</h2>
          {history && history.length > 0 && (
            <button type="button" onClick={() => navigate("/coaching/history")}
              className="text-[10px] text-primary hover:underline">View all</button>
          )}
        </div>
        {history && history.length > 0 ? (
          <div className="flex flex-col gap-2">
            {history.map((h) => (
              <div key={h.id} className="flex items-center gap-3 py-1.5">
                <span className="text-[10px] text-dim tabular-nums w-14 shrink-0">
                  {formatTime(h.deliveredAt)}
                </span>
                <p className="text-[11px] text-foreground truncate flex-1">{h.message}</p>
                <FeedbackBadge feedback={h.feedback} />
              </div>
            ))}
          </div>
        ) : (
          <p className="text-[11px] text-muted-foreground">
            No coaching interventions yet. The system will start offering suggestions as it learns your patterns.
          </p>
        )}
      </div>
    </div>
  );
}

function FeedbackBadge({ feedback }: { feedback: string | null }) {
  if (!feedback) return null;
  const colors: Record<string, string> = {
    helpful: "text-success bg-success/10",
    dismissed: "text-warning bg-warning/10",
    stop: "text-destructive bg-destructive/10",
    ignored: "text-dim bg-accent/30",
  };
  return (
    <span className={`text-[9px] px-1.5 py-0.5 rounded-full ${colors[feedback] ?? "text-dim"}`}>
      {feedback}
    </span>
  );
}
```

- [ ] **Step 3: Export CoachingHealthCard from index**

In `desktop-ui/src/features/coaching/index.ts`, add:

```ts
export { CoachingHealthCard } from "./components/CoachingHealthCard";
```

- [ ] **Step 4: Verify it builds and lint passes**

Run: `cd desktop-ui && bun run build && bun run lint`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coaching/
git commit -m "feat(coaching): implement Overview page with health card + pattern/intervention previews"
```

---

## Task 9: Patterns Page

**Files:**
- Create: `desktop-ui/src/features/coaching/components/PatternCard.tsx`
- Modify: `desktop-ui/src/features/coaching/pages/PatternsPage.tsx`

- [ ] **Step 1: Create PatternCard**

Create `desktop-ui/src/features/coaching/components/PatternCard.tsx`:

```tsx
interface PatternCardProps {
  name: string;
  description: string;
  domain: string;
  confidence: number;
  signalCount: number;
}

export function PatternCard({ name, description, domain, confidence, signalCount }: PatternCardProps) {
  const pct = Math.round(confidence * 100);

  return (
    <div className="glass-card rounded-xl p-4">
      <div className="flex items-start justify-between mb-2">
        <h3 className="text-xs font-medium text-foreground">{name}</h3>
        <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-accent/30 text-dim shrink-0">
          {domain}
        </span>
      </div>
      <p className="text-[11px] text-muted-foreground leading-relaxed mb-3">{description}</p>
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1.5">
          <div className="h-1 flex-1 rounded-full bg-accent overflow-hidden w-16">
            <div className="h-full rounded-full bg-primary transition-all" style={{ width: `${pct}%` }} />
          </div>
          <span className="text-[10px] tabular-nums text-dim">{pct}%</span>
        </div>
        <span className="text-[10px] text-dim tabular-nums">{signalCount} signals</span>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Implement PatternsPage**

Replace `desktop-ui/src/features/coaching/pages/PatternsPage.tsx`:

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { PatternCard } from "../components/PatternCard";

interface DetectedPattern {
  name: string;
  confidence: number;
  signalCount: number;
  description: string;
  domain: string;
}

export function PatternsPage() {
  const { data: patterns, loading } = useQuery<DetectedPattern[]>("coaching_patterns", undefined, []);

  if (loading) {
    return <div className="text-[11px] text-muted-foreground">Loading patterns...</div>;
  }

  if (!patterns || patterns.length === 0) {
    return (
      <div className="flex items-center justify-center h-48">
        <p className="text-[11px] text-muted-foreground">
          No patterns detected yet. Patterns emerge as the coaching system observes your work habits over time.
        </p>
      </div>
    );
  }

  const sorted = [...patterns].sort((a, b) => b.confidence - a.confidence);

  return (
    <div className="grid grid-cols-2 gap-3 max-w-3xl">
      {sorted.map((p) => (
        <PatternCard key={p.name} {...p} />
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Export PatternCard**

In `desktop-ui/src/features/coaching/index.ts`, add:

```ts
export { PatternCard } from "./components/PatternCard";
```

- [ ] **Step 4: Verify it builds**

Run: `cd desktop-ui && bun run build && bun run lint`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coaching/
git commit -m "feat(coaching): implement Patterns page with pattern grid"
```

---

## Task 10: History Page with Retroactive Feedback

**Files:**
- Create: `desktop-ui/src/features/coaching/components/InterventionRow.tsx`
- Modify: `desktop-ui/src/features/coaching/pages/HistoryPage.tsx`

- [ ] **Step 1: Create InterventionRow**

Create `desktop-ui/src/features/coaching/components/InterventionRow.tsx`:

```tsx
import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { formatTime } from "@shared/lib/dates";
import { ThumbsUp, X } from "lucide-react";

interface InterventionRowProps {
  id: string;
  message: string;
  interventionType: string;
  triggerName: string;
  feedback: string | null;
  deliveredAt: string;
}

const feedbackColors: Record<string, string> = {
  helpful: "text-success bg-success/10",
  dismissed: "text-warning bg-warning/10",
  stop: "text-destructive bg-destructive/10",
  ignored: "text-dim bg-accent/30",
};

export function InterventionRow({
  id,
  message,
  interventionType,
  triggerName,
  feedback,
  deliveredAt,
}: InterventionRowProps) {
  const { mutate: submitFeedback } = useMutation("coaching_submit_feedback");

  const handleFeedback = async (response: string) => {
    await submitFeedback({ intervention_id: id, response });
    invalidateQueries("coaching_intervention_log");
    invalidateQueries("coaching_feedback_stats");
  };

  const canGiveFeedback = !feedback || feedback === "ignored";

  return (
    <div className="flex items-start gap-3 py-3 border-b border-border last:border-0">
      <span className="text-[10px] text-dim tabular-nums w-14 pt-0.5 shrink-0">
        {formatTime(deliveredAt)}
      </span>

      <div className="flex-1 min-w-0">
        <p className="text-[11px] text-foreground leading-relaxed">{message}</p>
        <div className="flex items-center gap-2 mt-1.5">
          <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-accent/30 text-dim">
            {interventionType}
          </span>
          <span className="text-[9px] text-dim">{triggerName}</span>
        </div>
      </div>

      <div className="flex items-center gap-2 shrink-0">
        {feedback && (
          <span className={`text-[9px] px-1.5 py-0.5 rounded-full ${feedbackColors[feedback] ?? "text-dim"}`}>
            {feedback}
          </span>
        )}
        {canGiveFeedback && (
          <>
            <button
              type="button"
              onClick={() => handleFeedback("helpful")}
              className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-success transition-colors"
              title="Mark as helpful"
            >
              <ThumbsUp className="w-3 h-3" />
            </button>
            <button
              type="button"
              onClick={() => handleFeedback("dismissed")}
              className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-destructive transition-colors"
              title="Dismiss"
            >
              <X className="w-3 h-3" />
            </button>
          </>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Implement HistoryPage**

Replace `desktop-ui/src/features/coaching/pages/HistoryPage.tsx`:

```tsx
import { useQuery } from "@shared/hooks/useQuery";
import { InterventionRow } from "../components/InterventionRow";

interface InterventionLog {
  id: string;
  interventionType: string;
  message: string;
  triggerName: string;
  feedback: string | null;
  deliveredAt: string;
  feedbackAt: string | null;
}

export function HistoryPage() {
  const { data: history, loading } = useQuery<InterventionLog[]>(
    "coaching_intervention_log",
    { limit: 100 },
    [],
  );

  if (loading) {
    return <div className="text-[11px] text-muted-foreground">Loading history...</div>;
  }

  if (!history || history.length === 0) {
    return (
      <div className="flex items-center justify-center h-48">
        <p className="text-[11px] text-muted-foreground">
          No coaching interventions yet. The system will start offering suggestions as it learns your patterns.
        </p>
      </div>
    );
  }

  return (
    <div className="max-w-3xl">
      <div className="glass-card rounded-xl p-5">
        {history.map((h) => (
          <InterventionRow key={h.id} {...h} />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Export InterventionRow**

In `desktop-ui/src/features/coaching/index.ts`, add:

```ts
export { InterventionRow } from "./components/InterventionRow";
```

- [ ] **Step 4: Verify it builds**

Run: `cd desktop-ui && bun run build && bun run lint`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/coaching/
git commit -m "feat(coaching): implement History page with retroactive feedback"
```

---

## Task 11: Make CoachingCard Clickable

**Files:**
- Modify: `desktop-ui/src/features/projects/components/overview/CoachingCard.tsx`

- [ ] **Step 1: Add navigation**

In `CoachingCard.tsx`, add `useNavigate` import and make the card clickable:

```tsx
import { useNavigate } from "react-router";
import { useCoachingNudge } from "@shared/hooks/useCoachingNudge";
import { Brain, ThumbsUp, X } from "lucide-react";

export function CoachingCard() {
  const navigate = useNavigate();
  const { nudge, handleFeedback } = useCoachingNudge({ autoCollapseMs: 60_000 });

  return (
    <div
      className="glass-card rounded-xl p-5 cursor-pointer hover:bg-accent/5 transition-colors"
      onClick={() => navigate("/coaching")}
      onKeyDown={(e) => e.key === "Enter" && navigate("/coaching")}
      role="button"
      tabIndex={0}
    >
      {/* ...existing content unchanged... */}
    </div>
  );
}
```

Keep all existing inner content (nudge display + feedback buttons) unchanged. The feedback buttons already call `handleFeedback` which uses `e.stopPropagation` implicitly via the mutation — but add explicit `stopPropagation` to prevent navigation when clicking feedback:

```tsx
            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); handleFeedback(nudge.id, "helpful"); }}
              ...
            >
```

Do the same for the dismiss button.

- [ ] **Step 2: Verify it builds**

Run: `cd desktop-ui && bun run build && bun run lint`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/projects/components/overview/CoachingCard.tsx
git commit -m "feat(coaching): make CoachingCard clickable to navigate to /coaching"
```

---

## Task 12: Final Verification

- [ ] **Step 1: Full Rust build + clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 2: Full Rust test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass (including new coaching_intervention_log repo tests)

- [ ] **Step 3: Frontend build + lint**

Run: `cd desktop-ui && bun run build && bun run lint`
Expected: no errors

- [ ] **Step 4: Update GAPS.md**

In `docs/architecture/GAPS.md`, mark the Coaching Dashboard gap as resolved:
- Remove item #1 from the Medium priority action plan
- Update the summary count from "4 Medium" to "3 Medium"
- Update `feature-coaching` row in Section 3 from "Partial UI" to "Full UI"

- [ ] **Step 5: Commit**

```bash
git add docs/architecture/GAPS.md
git commit -m "docs: mark coaching dashboard gap as resolved"
```
