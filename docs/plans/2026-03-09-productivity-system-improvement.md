# Productivity System Improvement Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring the productivity tracking system from 68/100 to 100/100 by fixing coherence issues, merging duplicate pages, adding analytics/intelligence features, and integrating calendar data.

**Architecture:** Four phases: (1) Fix internal coherence (3 quality formulas → 1, 2 session tables → 1, bug fixes), (2) Merge /dashboard and /productivity into one unified page, (3) Add weekly assessment, trend analysis, richer scoring, and urge surfing, (4) Add Google Calendar integration and keyboard accessibility.

**Tech Stack:** Rust (SQLx, Tokio, Chrono), TypeScript/React (Tailwind v4, Tauri IPC), SQLite

**Excluded:** PDF/CSV export, AI session planner (out of scope per user request)

---

## Phase 1: Coherence & Bug Fixes

### Task 1.1: Unify Quality Scoring — Single Formula

Currently three competing formulas:
- `compute_productivity_score()` in `aggregator.rs:L333` — 4 components, daily level, 0-100
- `QualityScorer::score_session()` in `intelligence/quality_scorer.rs:L46` — 5 components, session level, 0-100
- `FocusManager::compute_quality()` in `focus.rs:L256` — 3 components, manual session, 0.0-1.0

**Strategy:** Keep `QualityScorer` as the single source of truth. Deprecate `compute_quality()` from `FocusManager`. Rewrite `compute_productivity_score()` to consume quality scores from `productivity_quality_scores` table instead of inventing its own formula. Wire `score_day()` into production.

**Files:**
- Modify: `crates/feature-productivity/src/intelligence/quality_scorer.rs`
- Modify: `crates/feature-productivity/src/focus.rs`
- Modify: `crates/feature-productivity/src/aggregator.rs`
- Modify: `crates/feature-productivity/src/dashboard_emitter.rs`
- Modify: `crates/feature-productivity/src/types.rs`
- Test: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Write test for unified daily score from session quality scores**

In `crates/feature-productivity/tests/integration_test.rs`, add:

```rust
#[tokio::test]
async fn test_daily_score_uses_quality_scores() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_feature_migrations(&pool, productivity_migrations()).await.unwrap();
    let repos = ProductivityRepos::new(pool.clone());

    // Insert a productivity session with a quality score
    let session_id = uuid::Uuid::new_v4().to_string();
    let today = chrono::Utc::now().date_naive();
    repos.intelligence_sessions.create(&ProductivitySession {
        id: session_id.clone(),
        session_type: "focus".into(),
        started_at: format!("{}T09:00:00Z", today),
        ended_at: Some(format!("{}T10:30:00Z", today)),
        duration_secs: Some(5400),
        dominant_category: Some("Development".into()),
        category_purity: Some(0.85),
        quality_score: None,
        source: "auto".into(),
        context_switches: Some(3),
        distraction_count: Some(1),
        ..Default::default()
    }).await.unwrap();

    // Score the session
    let scorer = QualityScorer::new(repos.clone());
    let score = scorer.score_session(&session_id).await.unwrap().unwrap();
    assert!(score.overall_score > 0.0);

    // Score the day
    let day_score = scorer.score_day(today).await.unwrap();
    assert!(day_score.is_some());
    let day_score = day_score.unwrap();
    assert!((day_score.overall_score - score.overall_score).abs() < 0.01);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(daily_score_uses_quality_scores)' --no-capture`
Expected: FAIL (test doesn't exist yet, or missing imports)

**Step 3: Fix `QualityScorer::score_session` — remove hardcoded 0.5 for task_completion**

In `crates/feature-productivity/src/intelligence/quality_scorer.rs`, modify `score_session()` to compute `task_completion` from the session's `okr_alignment` field instead of hardcoding 0.5. If `okr_alignment` is `None`, default to `0.5` (preserving existing behavior for unlinked sessions, but allowing real data to flow through when available):

```rust
// Replace the hardcoded task_completion = 0.5 line with:
let task_completion = session.okr_alignment.unwrap_or(0.5);
```

**Step 4: Wire `score_day()` into `DailyAggregator`**

In `crates/feature-productivity/src/aggregator.rs`, after computing the daily summary, call `QualityScorer::score_day()` and use its result for `productivity_score`:

```rust
// After the existing compute_productivity_score call, replace with:
// 1. Try to get the intelligence-layer daily quality score
let day_quality = self.quality_scorer.score_day(date).await?;
let productivity_score = if let Some(q) = &day_quality {
    // Use the unified quality score when intelligence data exists
    q.overall_score
} else {
    // Fallback to the legacy formula when no intelligence sessions exist
    compute_productivity_score(&summary)
};
summary.productivity_score = Some(productivity_score);
```

This requires `DailyAggregator` to hold a `QualityScorer` reference. Add it to the struct:

In `aggregator.rs`, add `quality_scorer: QualityScorer` field to `DailyAggregator` and update the constructor.

**Step 5: Remove `compute_quality()` usage from `FocusManager::end_session`**

In `crates/feature-productivity/src/focus.rs:L93`, after ending a session, instead of computing quality inline with the old 3-component formula, create a `ProductivitySession` record in the intelligence table and let `QualityScorer` score it:

```rust
// In end_session(), replace:
//   let quality = compute_quality(&session, on_task_ratio, default_target);
//   session.quality_score = Some(quality);
// With:
// Score will be computed by the intelligence layer's QualityScorer
// Don't set quality_score here — it will be set when scored
```

**Step 6: Fix `DashboardEmitter` live score**

In `crates/feature-productivity/src/dashboard_emitter.rs:L72-L97`, the `ScoreWindow::score()` method builds a stub `DailySummary` with `avg_session_quality: None` and `context_switches: 0`. Update it to query the latest quality scores for today:

```rust
// In score(), after building the stub summary:
// Query today's average session quality from productivity_quality_scores
let today = chrono::Utc::now().date_naive();
let avg_quality = self.repos.quality_scores
    .average_for_date(today).await.unwrap_or(None);
summary.avg_session_quality = avg_quality;
// Also get context switches from buckets
let switches = self.repos.buckets
    .total_switches_for_date(&today.to_string()).await.unwrap_or(0);
summary.context_switches = switches as i64;
```

**Step 7: Run all productivity tests**

Run: `cargo nextest run -p feature-productivity --no-capture`
Expected: All pass

**Step 8: Commit**

```bash
git add crates/feature-productivity/
git commit -m "feat(productivity): unify quality scoring into single QualityScorer system"
```

---

### Task 1.2: Merge Session Tables — focus_sessions → productivity_sessions

Two parallel session tables exist:
- `focus_sessions` — used by FocusManager, DailyAggregator, BatchWriter, all handlers, timeline
- `productivity_sessions` — used by IntelligenceLayer FSM, QualityScorer

**Strategy:** Add missing fields to `productivity_sessions` (action_id, project_id, target_mins, etc.), migrate all data, update all repos and consumers to use one `SessionRepo` backed by `productivity_sessions`. Drop `focus_sessions` references.

**Files:**
- Create: `crates/feature-productivity/migrations/006_merge_sessions.sql`
- Modify: `crates/feature-productivity/src/repos/focus_session.rs` → rewrite to query `productivity_sessions`
- Modify: `crates/feature-productivity/src/repos/intelligence_session.rs` → merge into focus_session.rs
- Modify: `crates/feature-productivity/src/repos/mod.rs`
- Modify: `crates/feature-productivity/src/focus.rs`
- Modify: `crates/feature-productivity/src/batch_writer.rs`
- Modify: `crates/feature-productivity/src/aggregator.rs`
- Modify: `crates/feature-productivity/src/intelligence/session_aggregator.rs`
- Modify: `crates/feature-productivity/src/types.rs`
- Modify: `crates/app-core/src/handlers/productivity.rs`
- Modify: `crates/app-core/src/handlers/timeline.rs`
- Test: `crates/feature-productivity/tests/repos_test.rs`

**Step 1: Write migration SQL**

Create `crates/feature-productivity/migrations/006_merge_sessions.sql`:

```sql
-- Add missing fields from focus_sessions to productivity_sessions
ALTER TABLE productivity_sessions ADD COLUMN action_id TEXT;
ALTER TABLE productivity_sessions ADD COLUMN project_id TEXT;
ALTER TABLE productivity_sessions ADD COLUMN target_mins INTEGER;
ALTER TABLE productivity_sessions ADD COLUMN actual_mins INTEGER;
ALTER TABLE productivity_sessions ADD COLUMN interruptions INTEGER DEFAULT 0;
ALTER TABLE productivity_sessions ADD COLUMN distraction_events TEXT;
ALTER TABLE productivity_sessions ADD COLUMN completed INTEGER DEFAULT 0;

-- Migrate any focus_sessions not yet in productivity_sessions
INSERT OR IGNORE INTO productivity_sessions (
    id, session_type, started_at, ended_at, duration_secs,
    quality_score, source, action_id, project_id, target_mins,
    actual_mins, interruptions, distraction_events, completed,
    context_switches, distraction_count, created_at, updated_at
)
SELECT
    id,
    CASE WHEN session_type = 'pomodoro' THEN 'focus' ELSE session_type END,
    started_at,
    ended_at,
    COALESCE(actual_mins * 60, 0),
    quality_score,
    CASE WHEN source = 'auto_detected' THEN 'auto' ELSE COALESCE(source, 'manual') END,
    action_id,
    project_id,
    target_mins,
    actual_mins,
    interruptions,
    distraction_events,
    completed,
    0, -- context_switches
    0, -- distraction_count
    datetime('now'),
    datetime('now')
FROM focus_sessions
WHERE id NOT IN (SELECT id FROM productivity_sessions);

-- Update activity_events FK to point at productivity_sessions
-- SQLite can't ALTER FK, so we update the reference conceptually
-- The existing focus_session_id column values are valid UUIDs that now exist
-- in productivity_sessions too (from the INSERT above)

-- Create index for the active session query (replaces focus_sessions index)
CREATE INDEX IF NOT EXISTS idx_productivity_sessions_active
    ON productivity_sessions(ended_at) WHERE ended_at IS NULL;

-- Create index for action_id lookups
CREATE INDEX IF NOT EXISTS idx_productivity_sessions_action
    ON productivity_sessions(action_id) WHERE action_id IS NOT NULL;
```

**Step 2: Register migration in feature package**

In `crates/feature-productivity/src/lib.rs`, add migration 006 to the `migrations()` function:

```rust
FeatureMigration {
    feature_name: "productivity",
    version: 6,
    description: "Merge focus_sessions into productivity_sessions",
    sql: include_str!("../migrations/006_merge_sessions.sql"),
},
```

**Step 3: Write test for merged session CRUD**

In `crates/feature-productivity/tests/repos_test.rs`, add a test that creates a session via the unified repo and verifies it can be queried back with all fields (both focus-origin and intelligence-origin fields):

```rust
#[tokio::test]
async fn test_merged_session_crud() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_feature_migrations(&pool, productivity_migrations()).await.unwrap();
    let repos = ProductivityRepos::new(pool.clone());

    // Create a manual focus session (user-driven)
    let session = repos.sessions.create_focus_session(
        Some("action-123"),
        Some("project-456"),
        "focus",
        Some(25), // target_mins
        "manual",
    ).await.unwrap();

    assert_eq!(session.session_type, "focus");
    assert_eq!(session.action_id.as_deref(), Some("action-123"));
    assert_eq!(session.target_mins, Some(25));
    assert!(session.ended_at.is_none()); // active

    // Get active session
    let active = repos.sessions.get_active().await.unwrap();
    assert!(active.is_some());
    assert_eq!(active.unwrap().id, session.id);

    // End the session
    let ended = repos.sessions.end_session(&session.id, Some("good session")).await.unwrap();
    assert!(ended.is_some());
    let ended = ended.unwrap();
    assert!(ended.ended_at.is_some());
    assert!(ended.duration_secs.unwrap_or(0) > 0);
}
```

**Step 4: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(merged_session_crud)' --no-capture`
Expected: FAIL (method `create_focus_session` doesn't exist yet)

**Step 5: Rewrite `FocusSessionRepo` to use `productivity_sessions`**

Rewrite `crates/feature-productivity/src/repos/focus_session.rs` to query `productivity_sessions` instead of `focus_sessions`. Keep the same public API surface but change all SQL to target the merged table. Add the new `create_focus_session` method that populates both the legacy focus fields (action_id, target_mins, etc.) and the intelligence fields (context_switches, etc.).

Key methods to update:
- `create()` → INSERT into `productivity_sessions`
- `get_active()` → SELECT from `productivity_sessions WHERE ended_at IS NULL`
- `update()` → UPDATE `productivity_sessions`
- `list_range()` → SELECT from `productivity_sessions`
- `get()` → SELECT from `productivity_sessions`

**Step 6: Merge `IntelligenceSessionRepo` into the rewritten `FocusSessionRepo`**

Move all methods from `intelligence_session.rs` into `focus_session.rs`. They already target `productivity_sessions`, so just merge the impl blocks. Delete `intelligence_session.rs`.

Update `repos/mod.rs`:
- Remove the `intelligence_sessions` field
- Keep `sessions` field pointing to the rewritten repo
- Update all consumers that used `repos.intelligence_sessions` to use `repos.sessions`

**Step 7: Update `ProductivitySession` type to include focus fields**

In `crates/feature-productivity/src/types.rs`, add the missing focus-origin fields to `ProductivitySession`:

```rust
pub struct ProductivitySession {
    // ... existing fields ...
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub target_mins: Option<i64>,
    pub actual_mins: Option<i64>,
    pub interruptions: Option<i64>,
    pub distraction_events: Option<String>, // JSON
    pub completed: Option<bool>,
}
```

**Step 8: Update SessionAggregator to use unified repo**

In `crates/feature-productivity/src/intelligence/session_aggregator.rs`, change `self.repos.intelligence_sessions` calls to `self.repos.sessions`.

**Step 9: Update FocusManager to use unified repo**

In `crates/feature-productivity/src/focus.rs`, update all `self.repos.sessions` calls to use the new methods that write to `productivity_sessions`.

**Step 10: Update BatchWriter**

In `crates/feature-productivity/src/batch_writer.rs:L62`, `repos.sessions.get_active()` already calls the repo — since we rewrote it to query `productivity_sessions`, this works automatically. Verify the `focus_session_id` FK stamping still works (the column name stays the same, values are the same UUIDs).

**Step 11: Update DailyAggregator**

In `crates/feature-productivity/src/aggregator.rs:L76`, `repos.sessions.list_range()` now returns from the merged table. Update the aggregation logic to handle both manual and auto-detected sessions. Use `duration_secs` as the canonical duration (not `actual_mins * 60`).

**Step 12: Update handlers**

In `crates/app-core/src/handlers/productivity.rs`, update `session_to_response()` converter to map from `ProductivitySession` → `FocusSessionResponse`. The response type keeps the same shape for frontend compatibility.

In `crates/app-core/src/handlers/timeline.rs:L41`, update `normalize_focus_session()` to accept `ProductivitySession`.

**Step 13: Run all tests**

Run: `cargo nextest run -p feature-productivity --no-capture`
Run: `cargo nextest run -p app-core --no-capture`
Expected: All pass

**Step 14: Commit**

```bash
git add crates/feature-productivity/ crates/app-core/
git commit -m "feat(productivity): merge focus_sessions into productivity_sessions table"
```

---

### Task 1.3: Fix productivity_weekly to Live-Override Today

**Files:**
- Modify: `crates/app-core/src/handlers/productivity.rs:L227-L240`
- Test: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Write test**

```rust
#[tokio::test]
async fn test_weekly_includes_live_today() {
    // Setup pool with a stored summary for yesterday and nothing for today
    // Call productivity_weekly
    // Assert today's entry exists (live-computed) and yesterday's exists (from DB)
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(weekly_includes_live_today)' --no-capture`

**Step 3: Add live-override logic to `productivity_weekly`**

In `crates/app-core/src/handlers/productivity.rs`, update the `productivity_weekly` handler to match the `productivity_summary_range` pattern:

```rust
pub async fn productivity_weekly(&self) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    let aggregator = self.aggregator()?;
    let repos = self.productivity_repos()?;
    let today = chrono::Utc::now().date_naive();
    let week_start = today - chrono::Duration::days(6);

    let mut summaries = repos.summaries
        .list_range(&week_start.to_string(), &today.to_string())
        .await
        .map_err(map_prod_err)?;

    // Live-override today's summary
    let live_today = aggregator.compute_today().await.map_err(map_prod_err)?;
    if let Some(idx) = summaries.iter().position(|s| s.date == today.to_string()) {
        summaries[idx] = live_today;
    } else {
        summaries.push(live_today);
    }

    Ok(summaries.into_iter().map(summary_to_response).collect())
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p app-core --no-capture`
Expected: Pass

**Step 5: Commit**

```bash
git add crates/app-core/src/handlers/productivity.rs
git commit -m "fix(productivity): weekly endpoint now live-computes today's summary"
```

---

### Task 1.4: Fix add_time_entry Atomicity

**Files:**
- Modify: `crates/storage/src/repos/action_repo.rs:L606-L680`
- Test: `crates/storage/tests/` (or inline test)

**Step 1: Write test**

```rust
#[tokio::test]
async fn test_add_time_entry_atomic() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Create an action
    // Add a time entry
    // Verify total_tracked_secs matches the entry's duration
    // This test documents the atomicity requirement
}
```

**Step 2: Wrap in transaction**

In `crates/storage/src/repos/action_repo.rs:L606`, wrap both the INSERT and UPDATE in a transaction:

```rust
pub async fn add_time_entry(
    &self,
    action_id: &str,
    source: &str,
    started_at: &str,
    duration_secs: i64,
) -> Result<TimeEntryRow> {
    let mut tx = self.pool.begin().await?;
    let id = uuid::Uuid::new_v4().to_string();

    let entry = sqlx::query_as::<_, TimeEntryRow>(
        "INSERT INTO action_time_entries (id, action_id, source, started_at, duration_secs)
         VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *"
    )
    .bind(&id).bind(action_id).bind(source).bind(started_at).bind(duration_secs)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("UPDATE actions SET total_tracked_secs = total_tracked_secs + ?1 WHERE id = ?2")
        .bind(duration_secs).bind(action_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(entry)
}
```

Apply the same pattern to `close_time_entry` at L647.

**Step 3: Run tests**

Run: `cargo nextest run -p storage --no-capture`
Expected: Pass

**Step 4: Commit**

```bash
git add crates/storage/src/repos/action_repo.rs
git commit -m "fix(storage): wrap time entry insert+counter update in transaction"
```

---

### Task 1.5: Fix DistractionBanner Hardcoded Category Names

**Files:**
- Modify: `desktop-ui/src/components/productivity/DistractionBanner.tsx:L22-L32`

**Step 1: Fix the filter**

Replace the hardcoded string matching:

```typescript
// BEFORE (L27-L31):
// const distractingApps = topCategories
//   .filter(c => ["entertainment", "social media", "gaming"]
//     .includes(c.category.toLowerCase()))

// AFTER:
const distractingApps = topCategories
  .filter(c => c.categoryType === "distracting")
```

**Step 2: Run lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors

**Step 3: Commit**

```bash
git add desktop-ui/src/components/productivity/DistractionBanner.tsx
git commit -m "fix(ui): DistractionBanner uses categoryType instead of hardcoded names"
```

---

### Task 1.6: Fix WeekCalendarView Sidebar Bug

**Files:**
- Modify: `desktop-ui/src/components/dashboard/WeekCalendarView.tsx`

**Step 1: Add sidebar context gating**

The `WeekCalendarView` always renders `SummaryPanel` without checking `useSidebarOpen()`. Fix:

```typescript
import { useSidebarOpen } from "./layers";

// Inside the component:
const sidebarOpen = useSidebarOpen();

// In the JSX, wrap SummaryPanel:
{sidebarOpen && <SummaryPanel ... />}
```

**Step 2: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 3: Commit**

```bash
git add desktop-ui/src/components/dashboard/WeekCalendarView.tsx
git commit -m "fix(dashboard): WeekCalendarView respects sidebar toggle state"
```

---

### Task 1.7: Add Composite Index + Cache Cleanup

**Files:**
- Create: `crates/feature-productivity/migrations/007_indexes_and_cleanup.sql`
- Modify: `crates/feature-productivity/src/repos/categorization_cache.rs` (add `cleanup_expired`)
- Modify: `crates/feature-productivity/src/engine.rs` (schedule cache cleanup)

**Step 1: Write migration**

```sql
-- Composite index for the primary aggregation queries
CREATE INDEX IF NOT EXISTS idx_activity_events_started_idle
    ON activity_events(started_at, is_idle);

-- Note: categorization cache cleanup will be done in application code
```

**Step 2: Add `cleanup_expired()` method to categorization cache repo**

```rust
pub async fn cleanup_expired(&self) -> common::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM productivity_categorization_cache WHERE expires_at < datetime('now')"
    )
    .execute(&self.pool)
    .await?;
    Ok(result.rows_affected())
}
```

**Step 3: Schedule cleanup in `ProductivityEngine::start()`**

Add a periodic task (every 1 hour) that calls `repos.categorization_cache.cleanup_expired()`.

**Step 4: Register migration, run tests**

Run: `cargo nextest run -p feature-productivity --no-capture`

**Step 5: Commit**

```bash
git add crates/feature-productivity/
git commit -m "feat(productivity): add composite index and scheduled cache cleanup"
```

---

### Task 1.8: Deduplicate Day View Data Fetching

**Files:**
- Modify: `desktop-ui/src/components/dashboard/DayCalendarView.tsx`
- Modify: `desktop-ui/src/components/dashboard/ActivityTrack.tsx`

**Step 1: Pass timeline data from parent to ActivityTrack**

In `DayCalendarView.tsx`, it already fetches `timeline_query`. Pass the productivity-source entries down to `DayColumnsView` and then to `ActivityTrack` as a prop, instead of having `ActivityTrack` independently call `productivity_timeline`.

In `ActivityTrack.tsx`, accept an optional `timelineEntries` prop. When provided, derive the activity events from those entries instead of calling `productivity_timeline` separately. Only fall back to the independent fetch if the prop is not provided.

**Step 2: Remove duplicate 30s polling from ActivityTrack when data comes from parent**

**Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 4: Commit**

```bash
git add desktop-ui/src/components/dashboard/
git commit -m "fix(dashboard): deduplicate activity data fetching in day view"
```

---

### Task 1.9: Fix productivity_insights Handler — Return Stored Cards

**Files:**
- Modify: `crates/app-core/src/handlers/productivity.rs:L546-L558`

The `productivity_insights` handler calls `InsightEngine::generate_for_date()` which returns only newly-created cards. On repeat calls, all cards already exist so it returns `[]`. Fix: after generating, also query stored cards.

**Step 1: Fix the handler**

```rust
pub async fn productivity_insights(
    &self,
    date: Option<&str>,
) -> Result<Vec<InsightCardResponse>, ApiError> {
    let repos = self.productivity_repos()?;
    let date_str = date.unwrap_or(&chrono::Utc::now().date_naive().to_string());

    // Generate any missing insights (idempotent)
    let engine = InsightEngine::new(repos.clone());
    let _ = engine.generate_for_date(date_str).await.map_err(map_prod_err)?;

    // Always return ALL stored (non-dismissed) insights for the date
    let cards = repos.insights
        .list_for_date(date_str)
        .await
        .map_err(map_prod_err)?;

    Ok(cards.into_iter().map(insight_to_response).collect())
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p app-core --no-capture`

**Step 3: Commit**

```bash
git add crates/app-core/src/handlers/productivity.rs
git commit -m "fix(productivity): insights handler returns stored cards, not just new ones"
```

---

## Phase 2: Merge Dashboard & Productivity Into One Page

### Task 2.1: Extend SummaryPanel with Productivity Widgets

**Strategy:** Move key productivity widgets into the `SummaryPanel` sidebar. The layered calendar remains the main view. The sidebar becomes the productivity intelligence hub.

**Files:**
- Modify: `desktop-ui/src/components/dashboard/SummaryPanel.tsx`
- Move (import): productivity widgets into dashboard context

**Step 1: Add `ProductivityScoreRing` to SummaryPanel's DefaultSummary**

In `SummaryPanel.tsx`, import `ProductivityScoreRing` from `../productivity/ProductivityScoreRing` and render it at the top of the `DefaultSummary` section, replacing the simpler score display.

**Step 2: Add `InsightCardList` to SummaryPanel**

Import and render the `InsightCardList` component below the stats section in `DefaultSummary`. Pass the current date.

**Step 3: Add `GoalsProgress` to SummaryPanel**

Import and render `GoalsProgress` below insights. Use a compact variant (pass a `compact` prop if needed).

**Step 4: Add `AiSummaryCard` to SummaryPanel**

Import and render below goals.

**Step 5: Add `FocusStateIndicator` and `AutoFocusToast` to DashboardLayout**

These real-time overlays belong at the layout level, not inside the sidebar. Add them to `DashboardLayout.tsx` above the `{children}` render.

**Step 6: Run lint and visual check**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

**Step 7: Commit**

```bash
git add desktop-ui/src/components/dashboard/
git commit -m "feat(dashboard): integrate productivity widgets into SummaryPanel sidebar"
```

---

### Task 2.2: Add Activity Feed to Dashboard Day View

**Files:**
- Modify: `desktop-ui/src/components/dashboard/DayColumnsView.tsx`

**Step 1: Add `ActivityFeed` as a collapsible section below the calendar**

Import `ActivityFeed` from `../productivity/ActivityFeed`. Render it below the timeline grid as a collapsible panel (click to expand/collapse). Only show for day view, only for today.

**Step 2: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 3: Commit**

```bash
git add desktop-ui/src/components/dashboard/
git commit -m "feat(dashboard): add live activity feed to day view"
```

---

### Task 2.3: Add Categories Sub-Route to Dashboard

**Files:**
- Modify: `desktop-ui/src/App.tsx`

**Step 1: Add `/categories` route under dashboard layout**

Move the categories route from `/productivity/categories` to `/categories` wrapped in `DashboardLayout`. The `CategoriesPage` component stays the same — just re-routed.

```typescript
// In App.tsx routes, add:
<Route path="/categories" element={
  <DashboardLayout>
    <CategoriesPage />
  </DashboardLayout>
} />
```

**Step 2: Add Categories to DashboardLayout view switcher**

In `DashboardLayout.tsx`, add a "Categories" button to the view switcher pills (or as a gear icon in the toolbar) that navigates to `/categories`.

**Step 3: Commit**

```bash
git add desktop-ui/src/App.tsx desktop-ui/src/components/dashboard/DashboardLayout.tsx
git commit -m "feat(dashboard): add categories management route to dashboard"
```

---

### Task 2.4: Remove Separate Productivity Page

**Files:**
- Modify: `desktop-ui/src/App.tsx` — remove `/productivity/*` routes
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx` — remove Productivity nav item
- Delete (or deprecate): `desktop-ui/src/components/productivity/ProductivityLayout.tsx`
- Delete (or deprecate): `desktop-ui/src/components/productivity/DayView.tsx`

**Step 1: Update routes in App.tsx**

Remove all `/productivity/*` route definitions. Add redirects from old paths:

```typescript
<Route path="/productivity/*" element={<Navigate to="/" replace />} />
```

**Step 2: Remove Productivity from sidebar nav**

In `Sidebar.tsx:L23-L31`, remove the "Productivity" nav entry. The "Dashboard" entry now covers everything.

**Step 3: Clean up unused components**

Delete `ProductivityLayout.tsx` and `DayView.tsx` since their functionality is now in the dashboard. Keep all the individual widget components (`ProductivityScoreRing`, `ActivityFeed`, `InsightCardList`, etc.) as they're imported by the dashboard.

**Step 4: Run lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: Clean build, no unused import warnings

**Step 5: Commit**

```bash
git add desktop-ui/
git commit -m "feat(dashboard): remove separate productivity page, unify into dashboard"
```

---

### Task 2.5: Move Remaining Productivity Widgets to Dashboard

**Files:**
- Modify: `desktop-ui/src/components/dashboard/SummaryPanel.tsx`
- Modify: `desktop-ui/src/components/dashboard/DayColumnsView.tsx`

**Step 1: Add remaining widgets to appropriate locations**

Widgets that were on `/productivity` but not yet in the dashboard:
- `FocusSessionsList` → Add to SummaryPanel below score ring (shows today's completed sessions)
- `TopApps` → Already partially shown in SummaryPanel, enhance with full card
- `WorkHoursCard` → Add to SummaryPanel stats section
- `TimeEntrySection` → Add as a collapsible section in SummaryPanel or below calendar
- `LearnedRulesCard` → Add to Categories page, not the main dashboard
- `BreakdownDonuts` → Integrate into SummaryPanel's category breakdown
- `ProjectsCard` → Add to SummaryPanel

**Step 2: Make SummaryPanel scrollable**

With more widgets, ensure the sidebar is independently scrollable: `overflow-y-auto`.

**Step 3: Run lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

**Step 4: Commit**

```bash
git add desktop-ui/src/components/dashboard/
git commit -m "feat(dashboard): complete widget integration from productivity page"
```

---

## Phase 3: Analytics & Intelligence

### Task 3.1: Weekly Productivity Assessment

**Files:**
- Create: `crates/feature-productivity/src/weekly_assessment.rs`
- Modify: `crates/feature-productivity/src/lib.rs` (add module)
- Modify: `crates/feature-productivity/src/types.rs` (add types)
- Modify: `crates/app-core/src/handlers/productivity.rs` (add handler)
- Modify: `crates/desktop-shared/src/commands.rs` (add response type)
- Create: `desktop-ui/src/components/dashboard/WeeklyAssessment.tsx`
- Test: `crates/feature-productivity/tests/integration_test.rs`

**Step 1: Define types**

In `crates/feature-productivity/src/types.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAssessment {
    pub week_start: String,
    pub week_end: String,
    pub total_work_hours: f64,
    pub avg_daily_work_hours: f64,
    pub total_focus_hours: f64,
    pub total_meeting_hours: f64,
    pub total_break_hours: f64,
    pub avg_productivity_score: f64,
    pub score_trend: f64, // delta vs previous 4-week average
    pub focus_time_trend: f64,
    pub context_switches_avg: f64,
    pub context_switches_trend: f64,
    pub top_distractors: Vec<AppUsage>,
    pub most_productive_day: Option<String>,
    pub least_productive_day: Option<String>,
    pub consistency_score: f64, // 0-100, how consistent daily hours are
    pub daily_summaries: Vec<DailySummary>,
    pub ai_assessment: Option<String>,
}
```

**Step 2: Write failing test**

```rust
#[tokio::test]
async fn test_weekly_assessment_with_trend() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_feature_migrations(&pool, productivity_migrations()).await.unwrap();
    let repos = ProductivityRepos::new(pool.clone());

    // Insert 28 days of summaries (4 weeks of history)
    let today = chrono::Utc::now().date_naive();
    for i in 0..28 {
        let date = today - chrono::Duration::days(i);
        let summary = DailySummary {
            date: date.to_string(),
            productive_secs: 14400 + (i as i64 * 100), // slight trend
            total_active_secs: 28800,
            productivity_score: Some(65.0 + i as f64),
            context_switches: 20 - i as i64,
            ..Default::default()
        };
        repos.summaries.upsert(&summary).await.unwrap();
    }

    let engine = WeeklyAssessmentEngine::new(repos);
    let assessment = engine.compute(today).await.unwrap();

    assert!(assessment.avg_productivity_score > 0.0);
    assert!(assessment.score_trend != 0.0); // should show improvement
    assert_eq!(assessment.daily_summaries.len(), 7);
}
```

**Step 3: Run test to verify it fails**

Run: `cargo nextest run -p feature-productivity -E 'test(weekly_assessment_with_trend)' --no-capture`

**Step 4: Implement `WeeklyAssessmentEngine`**

Create `crates/feature-productivity/src/weekly_assessment.rs`:

```rust
pub struct WeeklyAssessmentEngine {
    repos: ProductivityRepos,
}

impl WeeklyAssessmentEngine {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self { repos }
    }

    pub async fn compute(&self, reference_date: NaiveDate) -> common::Result<WeeklyAssessment> {
        let week_end = reference_date;
        let week_start = week_end - chrono::Duration::days(6);
        let baseline_start = week_end - chrono::Duration::days(34); // 4 weeks + this week

        // Fetch this week's summaries
        let this_week = self.repos.summaries
            .list_range(&week_start.to_string(), &week_end.to_string())
            .await?;

        // Fetch 4-week baseline
        let baseline = self.repos.summaries
            .list_range(&baseline_start.to_string(), &(week_start - chrono::Duration::days(1)).to_string())
            .await?;

        // Compute metrics...
        let avg_score = this_week.iter()
            .filter_map(|s| s.productivity_score)
            .sum::<f64>() / this_week.len().max(1) as f64;

        let baseline_avg_score = baseline.iter()
            .filter_map(|s| s.productivity_score)
            .sum::<f64>() / baseline.len().max(1) as f64;

        let score_trend = avg_score - baseline_avg_score;

        // ... compute all other fields similarly ...

        Ok(WeeklyAssessment {
            week_start: week_start.to_string(),
            week_end: week_end.to_string(),
            avg_productivity_score: avg_score,
            score_trend,
            daily_summaries: this_week,
            // ... fill all fields ...
            ..Default::default()
        })
    }
}
```

**Step 5: Run test**

Run: `cargo nextest run -p feature-productivity -E 'test(weekly_assessment_with_trend)' --no-capture`
Expected: Pass

**Step 6: Add handler and IPC command**

In `crates/app-core/src/handlers/productivity.rs`, add:

```rust
pub async fn productivity_weekly_assessment(&self) -> Result<WeeklyAssessmentResponse, ApiError> {
    let repos = self.productivity_repos()?;
    let today = chrono::Utc::now().date_naive();
    let engine = WeeklyAssessmentEngine::new(repos.clone());
    let assessment = engine.compute(today).await.map_err(map_prod_err)?;
    Ok(assessment_to_response(assessment))
}
```

Add `WeeklyAssessmentResponse` to `desktop-shared/src/commands.rs`.

**Step 7: Create frontend component**

Create `desktop-ui/src/components/dashboard/WeeklyAssessment.tsx`:
- Show in SummaryPanel when viewing week view
- Display trend arrows (up/down) for score, focus time, context switches
- Show 4-week comparison bars
- Highlight most/least productive days

**Step 8: Run lint and tests**

Run: `cargo nextest run --workspace --no-capture`
Run: `cd desktop-ui && bun run lint:fix && bun run build`

**Step 9: Commit**

```bash
git add crates/feature-productivity/ crates/app-core/ crates/desktop-shared/ desktop-ui/
git commit -m "feat(productivity): add weekly productivity assessment with trend analysis"
```

---

### Task 3.2: Trend Analysis — Rolling Averages in SummaryPanel

**Files:**
- Modify: `crates/feature-productivity/src/repos/daily_summary.rs` (add rolling average query)
- Modify: `crates/app-core/src/handlers/productivity.rs` (add trend data to summary response)
- Modify: `crates/desktop-shared/src/commands.rs` (extend response type)
- Modify: `desktop-ui/src/components/dashboard/SummaryPanel.tsx` (show trend arrows)

**Step 1: Add `rolling_averages` query to `DailySummaryRepo`**

```rust
pub async fn rolling_averages(&self, before_date: &str, days: i64) -> common::Result<RollingAverages> {
    let row = sqlx::query_as::<_, RollingAverages>(
        "SELECT
            AVG(productivity_score) as avg_score,
            AVG(total_active_secs) as avg_active_secs,
            AVG(productive_secs) as avg_productive_secs,
            AVG(context_switches) as avg_context_switches,
            AVG(total_focus_secs) as avg_focus_secs
         FROM daily_summaries
         WHERE date < ?1 AND date >= date(?1, '-' || ?2 || ' days')"
    )
    .bind(before_date)
    .bind(days)
    .fetch_optional(&self.pool)
    .await?;
    Ok(row.unwrap_or_default())
}
```

**Step 2: Extend `ProductivitySummaryResponse` with trend fields**

In `desktop-shared/src/commands.rs`, add:

```rust
pub struct ProductivitySummaryResponse {
    // ... existing fields ...
    pub score_trend: Option<f64>,       // delta vs 4-week avg
    pub focus_time_trend: Option<f64>,  // delta vs 4-week avg
    pub active_time_trend: Option<f64>, // delta vs 4-week avg
}
```

**Step 3: Compute trends in handler**

In the `productivity_today` handler, after computing the summary, also compute rolling averages and attach trend deltas.

**Step 4: Show trend arrows in SummaryPanel**

In `SummaryPanel.tsx`, next to each stat (score, focus time, active time), show a small arrow icon with color:
- Green up arrow if trend > 0
- Red down arrow if trend < 0
- Gray dash if trend is 0 or no baseline data

**Step 5: Run lint, build, tests**

**Step 6: Commit**

```bash
git add crates/feature-productivity/ crates/app-core/ crates/desktop-shared/ desktop-ui/
git commit -m "feat(productivity): add rolling average trends with delta arrows in dashboard"
```

---

### Task 3.3: Richer Productivity Score — Expand to 8 Components

**Files:**
- Modify: `crates/feature-productivity/src/intelligence/quality_scorer.rs`
- Modify: `crates/feature-productivity/src/types.rs`
- Modify: `desktop-ui/src/components/productivity/ProductivityScoreRing.tsx`

**Step 1: Expand `ScoreWeights` to 8 components**

In `types.rs`, update `ScoreWeights`:

```rust
pub struct ScoreWeights {
    pub focus_depth: f64,           // 20% (was 30%)
    pub okr_alignment: f64,         // 10% (was 25%)
    pub distraction_inv: f64,       // 15% (was 20%)
    pub task_completion: f64,       // 10% (was 15%)
    pub continuity: f64,            // 10% (was 10%)
    pub deep_work_ratio: f64,       // 15% (new)
    pub avg_session_length: f64,    // 10% (new)
    pub meeting_focus_ratio: f64,   // 10% (new)
}
```

**Step 2: Implement new components in `QualityScorer`**

- `deep_work_ratio`: sessions > 90 min / total sessions
- `avg_session_length`: normalized (target: 45 min = 1.0, linear scale)
- `meeting_focus_ratio`: 1.0 - (meeting_secs / total_active_secs), rewarding less meeting time

**Step 3: Update `ProductivityScoreRing` to show 8 sub-bars**

Group into 4 visual categories for the ring:
- Focus (focus_depth + deep_work_ratio + avg_session_length)
- Quality (continuity + task_completion)
- Low distraction (distraction_inv)
- Alignment (okr_alignment + meeting_focus_ratio)

**Step 4: Run tests and lint**

**Step 5: Commit**

```bash
git add crates/feature-productivity/ desktop-ui/
git commit -m "feat(productivity): expand productivity score to 8 weighted components"
```

---

### Task 3.4: Urge Surfing for Distraction Overlay

**Files:**
- Modify: `desktop-ui/src/components/productivity/DistractionBanner.tsx`
- Modify: `desktop-ui/src/lib/types.ts` (if needed)

**Step 1: Add 10-second forced delay before dismiss button activates**

In `DistractionBanner.tsx`:

```typescript
const [urgeSurfingCountdown, setUrgeSurfingCountdown] = useState(10);

useEffect(() => {
  if (urgeSurfingCountdown <= 0) return;
  const timer = setTimeout(() => setUrgeSurfingCountdown(c => c - 1), 1000);
  return () => clearTimeout(timer);
}, [urgeSurfingCountdown]);

// In the JSX, disable the dismiss button during countdown:
<button
  disabled={urgeSurfingCountdown > 0}
  onClick={handleDismiss}
  className={urgeSurfingCountdown > 0 ? "opacity-50 cursor-not-allowed" : ""}
>
  {urgeSurfingCountdown > 0
    ? `Pause and reflect... (${urgeSurfingCountdown}s)`
    : "Thanks for the reminder!"}
</button>
```

**Step 2: Add breathing animation during countdown**

Add a subtle pulsing animation to the banner during the 10-second wait to encourage mindfulness:

```css
/* In theme.css or inline */
@keyframes breathe {
  0%, 100% { opacity: 0.7; }
  50% { opacity: 1; }
}
```

**Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "feat(productivity): add urge surfing with 10s forced pause on distraction overlay"
```

---

## Phase 4: Calendar Integration & Accessibility

### Task 4.1: Calendar Events Table + Meeting Sessions

**Files:**
- Create: `crates/feature-productivity/migrations/008_calendar_events.sql`
- Create: `crates/feature-productivity/src/repos/calendar_event.rs`
- Modify: `crates/feature-productivity/src/repos/mod.rs`
- Modify: `crates/feature-productivity/src/types.rs`
- Modify: `crates/feature-productivity/src/lib.rs`
- Test: `crates/feature-productivity/tests/repos_test.rs`

**Step 1: Write migration**

Create `crates/feature-productivity/migrations/008_calendar_events.sql`:

```sql
CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY,
    calendar_id TEXT NOT NULL DEFAULT 'primary',
    title TEXT NOT NULL,
    description TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    location TEXT,
    attendees_count INTEGER DEFAULT 0,
    is_recurring INTEGER DEFAULT 0,
    recurrence_id TEXT,
    source TEXT NOT NULL DEFAULT 'google', -- 'google', 'outlook', 'manual'
    external_uid TEXT, -- Google Calendar event UID
    session_id TEXT REFERENCES productivity_sessions(id),
    color TEXT,
    synced_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(external_uid, calendar_id)
);

CREATE INDEX idx_calendar_events_time ON calendar_events(started_at, ended_at);
CREATE INDEX idx_calendar_events_date ON calendar_events(date(started_at));
CREATE INDEX idx_calendar_events_session ON calendar_events(session_id) WHERE session_id IS NOT NULL;
```

**Step 2: Define CalendarEvent type in Rust**

In `types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    pub description: Option<String>,
    pub started_at: String,
    pub ended_at: String,
    pub location: Option<String>,
    pub attendees_count: i64,
    pub is_recurring: bool,
    pub recurrence_id: Option<String>,
    pub source: String,
    pub external_uid: Option<String>,
    pub session_id: Option<String>,
    pub color: Option<String>,
    pub synced_at: String,
    pub created_at: String,
    pub updated_at: String,
}
```

**Step 3: Write repo test**

```rust
#[tokio::test]
async fn test_calendar_event_crud() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    run_feature_migrations(&pool, productivity_migrations()).await.unwrap();
    let repos = ProductivityRepos::new(pool.clone());

    let event = CalendarEvent {
        id: uuid::Uuid::new_v4().to_string(),
        calendar_id: "primary".into(),
        title: "Team Standup".into(),
        started_at: "2026-03-09T09:00:00Z".into(),
        ended_at: "2026-03-09T09:30:00Z".into(),
        source: "google".into(),
        ..Default::default()
    };
    repos.calendar_events.upsert(&event).await.unwrap();

    let events = repos.calendar_events
        .list_range("2026-03-09T00:00:00Z", "2026-03-09T23:59:59Z")
        .await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].title, "Team Standup");
}
```

**Step 4: Run test to verify it fails, then implement repo**

Create `crates/feature-productivity/src/repos/calendar_event.rs` with:
- `upsert` (ON CONFLICT(external_uid, calendar_id) DO UPDATE)
- `list_range` (by started_at)
- `list_for_date` (by date)
- `delete_by_external_uid`
- `get_by_session_id`

**Step 5: Run test, verify pass**

**Step 6: Commit**

```bash
git add crates/feature-productivity/
git commit -m "feat(productivity): add calendar_events table and repo"
```

---

### Task 4.2: Calendar Sync Handler + Meeting Session Creation

**Files:**
- Create: `crates/app-core/src/handlers/calendar.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Modify: `crates/desktop-shared/src/commands.rs`
- Modify: `crates/desktop/src/commands/productivity.rs`

**Step 1: Add sync handler**

Create `crates/app-core/src/handlers/calendar.rs`:

```rust
impl AppCore {
    /// Sync calendar events from external source (called by frontend after MCP fetch)
    pub async fn calendar_sync_events(
        &self,
        events: Vec<CalendarEventInput>,
    ) -> Result<Vec<CalendarEventResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let mut results = Vec::new();

        for input in events {
            let event = CalendarEvent {
                id: uuid::Uuid::new_v4().to_string(),
                calendar_id: input.calendar_id.unwrap_or("primary".into()),
                title: input.title,
                description: input.description,
                started_at: input.started_at.clone(),
                ended_at: input.ended_at.clone(),
                location: input.location,
                attendees_count: input.attendees_count.unwrap_or(0),
                source: input.source.unwrap_or("google".into()),
                external_uid: input.external_uid,
                ..Default::default()
            };

            repos.calendar_events.upsert(&event).await.map_err(map_prod_err)?;

            // Auto-create a meeting session for this calendar event
            let duration_secs = /* compute from started_at/ended_at */;
            let session = repos.sessions.create_meeting_session(
                &event.started_at,
                &event.ended_at,
                duration_secs,
                &event.id,
            ).await.map_err(map_prod_err)?;

            // Link session to calendar event
            repos.calendar_events.link_session(&event.id, &session.id)
                .await.map_err(map_prod_err)?;

            results.push(calendar_event_to_response(event, Some(session)));
        }

        Ok(results)
    }

    /// Query calendar events for a date range
    pub async fn calendar_events(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<CalendarEventResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let events = repos.calendar_events
            .list_range(start_date, end_date)
            .await.map_err(map_prod_err)?;
        Ok(events.into_iter().map(|e| calendar_event_to_response(e, None)).collect())
    }
}
```

**Step 2: Add IPC types**

In `desktop-shared/src/commands.rs`, add `CalendarEventInput`, `CalendarEventResponse`.

**Step 3: Add Tauri commands**

In `desktop/src/commands/productivity.rs`, add `calendar_sync_events` and `calendar_events` commands.

**Step 4: Run tests**

Run: `cargo nextest run -p app-core --no-capture`

**Step 5: Commit**

```bash
git add crates/app-core/ crates/desktop-shared/ crates/desktop/
git commit -m "feat(productivity): add calendar sync handler with auto meeting session creation"
```

---

### Task 4.3: Calendar Layer in Dashboard

**Files:**
- Modify: `desktop-ui/src/components/dashboard/layers.ts`
- Modify: `desktop-ui/src/lib/types.ts`
- Modify: `desktop-ui/src/components/dashboard/DayColumnsView.tsx`
- Modify: `desktop-ui/src/components/dashboard/DayCalendarView.tsx`
- Modify: `desktop-ui/src/styles/theme.css`
- Modify: `crates/app-core/src/handlers/timeline.rs`

**Step 1: Add `TimelineSource::Calendar` in types**

In `desktop-ui/src/lib/types.ts`, add `"calendar"` to `TimelineSource` union type. Add `"calendarEvent"` to `TimelineEntryType`.

**Step 2: Add Calendar layer config**

In `desktop-ui/src/components/dashboard/layers.ts`:

```typescript
{
  key: "calendar" as LayerKey,
  label: "Calendar",
  icon: Calendar,
  color: "var(--timeline-calendar)",
  sources: ["calendar" as TimelineSource],
  defaultEnabled: true,
}
```

Add `"calendar"` to the `LayerKey` type.

**Step 3: Add timeline color variable**

In `desktop-ui/src/styles/theme.css`, add:

```css
--timeline-calendar: oklch(0.65 0.15 280);
```

Register in `@theme inline`:

```css
--color-timeline-calendar: var(--timeline-calendar);
```

**Step 4: Handle Calendar source in backend timeline handler**

In `crates/app-core/src/handlers/timeline.rs`, add a `Calendar` branch to the fan-out:

```rust
if sources.contains(&"calendar") {
    let calendar_events = repos.calendar_events
        .list_range(&query.start_date, &query.end_date).await?;
    for event in calendar_events {
        entries.push(TimelineEntry {
            id: event.id,
            entry_type: "calendarEvent".into(),
            source: "calendar".into(),
            title: event.title,
            started_at: event.started_at,
            ended_at: Some(event.ended_at),
            color: "var(--timeline-calendar)".into(),
            ..Default::default()
        });
    }
}
```

**Step 5: Render Calendar column in DayColumnsView**

Add a "Calendar" column alongside Activity, Time Entries, Tasks, etc. Render calendar event blocks with the purple calendar color, showing event title and time.

**Step 6: Run lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

**Step 7: Commit**

```bash
git add crates/app-core/ desktop-ui/
git commit -m "feat(dashboard): add calendar layer with meeting events in timeline"
```

---

### Task 4.4: Calendar Sync UI

**Files:**
- Create: `desktop-ui/src/components/dashboard/CalendarSync.tsx`
- Modify: `desktop-ui/src/components/dashboard/DashboardLayout.tsx`

**Step 1: Create CalendarSync component**

A settings/toolbar button that:
1. Checks if Google Calendar MCP is configured (call `mcp_status` or similar)
2. If configured, shows a "Sync Calendar" button
3. On click, invokes the agent to fetch today's calendar events via MCP
4. Passes the results to `calendar_sync_events` Tauri command
5. Shows sync status (last synced time)

```typescript
// CalendarSync.tsx
export function CalendarSync() {
  const { mutate, loading } = useMutation("calendar_sync_events");
  const [lastSynced, setLastSynced] = useState<string | null>(null);

  const handleSync = async () => {
    // This calls the backend which fetches from Google Calendar MCP
    await mutate({ source: "google" });
    setLastSynced(new Date().toLocaleTimeString());
  };

  return (
    <button onClick={handleSync} disabled={loading} title="Sync calendar">
      <Calendar className="w-4 h-4" />
      {loading && <span className="text-[10px]">syncing...</span>}
    </button>
  );
}
```

**Step 2: Add to DashboardLayout toolbar**

In `DashboardLayout.tsx`, add `<CalendarSync />` to the toolbar next to the layers button.

**Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 4: Commit**

```bash
git add desktop-ui/
git commit -m "feat(dashboard): add calendar sync button to toolbar"
```

---

### Task 4.5: Keyboard Accessibility

**Files:**
- Modify: `desktop-ui/src/components/dashboard/DayColumnsView.tsx`
- Modify: `desktop-ui/src/components/dashboard/WeekCalendarView.tsx`
- Modify: `desktop-ui/src/components/dashboard/MonthCalendarView.tsx`
- Modify: `desktop-ui/src/components/dashboard/ActivityTrack.tsx`

**Step 1: Add keyboard zoom in DayColumnsView**

Replace the `biome-ignore lint/a11y/noStaticElementInteractions` suppression with proper keyboard handlers:

```typescript
// On the hour gutter:
<div
  role="slider"
  aria-label="Zoom level"
  aria-valuemin={30}
  aria-valuemax={200}
  aria-valuenow={hourHeight}
  tabIndex={0}
  onKeyDown={(e) => {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setHourHeight(h => Math.min(200, h + 10));
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setHourHeight(h => Math.max(30, h - 10));
    }
  }}
>
```

**Step 2: Add aria-labels to focus indicator bars in ActivityTrack**

```typescript
<button
  aria-label={`Focus session: ${session.title || "Untitled"}, ${formatDuration(session.duration)}`}
  // ... existing props
>
```

**Step 3: Add keyboard navigation to calendar views**

In `MonthCalendarView`, add arrow key navigation between day cells:
- Left/Right: previous/next day
- Up/Down: previous/next week
- Enter: navigate to day view

In `WeekCalendarView`, add Left/Right for day navigation.

**Step 4: Add accessible zoom reset description**

```typescript
<button aria-label="Reset zoom to default level" onClick={resetZoom}>
  Reset
</button>
```

**Step 5: Run lint**

Run: `cd desktop-ui && bun run lint:fix`

**Step 6: Commit**

```bash
git add desktop-ui/
git commit -m "feat(dashboard): add keyboard accessibility for zoom, navigation, and ARIA labels"
```

---

## Summary

| Phase | Tasks | Estimated Commits |
|-------|-------|-------------------|
| Phase 1: Coherence & Bugs | 9 tasks | 9 commits |
| Phase 2: Merge Pages | 5 tasks | 5 commits |
| Phase 3: Analytics | 4 tasks | 4 commits |
| Phase 4: Calendar + A11y | 5 tasks | 5 commits |
| **Total** | **23 tasks** | **23 commits** |

### Dependency Graph

```
Phase 1 (all independent, can be parallelized):
  1.1 Unify Scoring ─┐
  1.2 Merge Sessions ─┤─► Phase 2 (depends on 1.1, 1.2)
  1.3 Fix Weekly ─────┘
  1.4 Fix Atomicity ──── independent
  1.5 Fix Distraction ── independent
  1.6 Fix Sidebar ────── independent
  1.7 Index + Cache ──── independent
  1.8 Dedup Fetch ────── independent
  1.9 Fix Insights ───── independent

Phase 2 (sequential):
  2.1 → 2.2 → 2.3 → 2.4 → 2.5

Phase 3 (mostly independent, but 3.3 depends on 1.1):
  3.1 Weekly Assessment ── depends on Phase 1
  3.2 Trend Analysis ───── depends on Phase 1
  3.3 Richer Score ──────── depends on 1.1
  3.4 Urge Surfing ──────── depends on 1.5

Phase 4 (sequential):
  4.1 → 4.2 → 4.3 → 4.4 (calendar chain)
  4.5 Accessibility ─────── independent
```
