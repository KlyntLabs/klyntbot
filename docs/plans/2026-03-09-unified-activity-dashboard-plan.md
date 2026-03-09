# Unified Activity Dashboard — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a time-block calendar dashboard at `/` that visualizes all system activity (tasks, notes, finance, app tracking, focus sessions) with Day/Week/Month/Year views and a context-sensitive summary panel.

**Architecture:** Query-time aggregation across 4 existing tables (`activity_events`, `focus_sessions`, `action_time_entries`, `domain_event_log`) normalized into a unified `TimelineEntry` type. No new storage tables. Backend handler in `app-core`, Tauri command in `desktop`, React components in `desktop-ui`.

**Tech Stack:** Rust (sqlx, tokio, serde), TypeScript/React (react-router, Tailwind v4 CSS tokens, lucide-react icons), Tauri 2 IPC.

**Design Doc:** `docs/plans/2026-03-09-unified-activity-dashboard-design.md`

---

## Task 1: Add Note Domain Events to Bus

**Files:**
- Modify: `crates/bus/src/domain_events.rs:14-95` (DomainEvent enum)

**Step 1: Add NoteCreated and NoteUpdated variants**

In `crates/bus/src/domain_events.rs`, add after the `// -- Finance --` block (after line 72):

```rust
    // -- Notes --
    NoteCreated {
        note_id: String,
        title: String,
    },
    NoteUpdated {
        note_id: String,
        title: String,
    },
```

**Step 2: Verify it compiles**

Run: `cargo build -p bus`
Expected: SUCCESS

**Step 3: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add NoteCreated and NoteUpdated domain events"
```

---

## Task 2: Emit Domain Events from AppCore Mutation Handlers

**Files:**
- Modify: `crates/app-core/src/handlers/tasks.rs:231-329` (task_create), `crates/app-core/src/handlers/tasks.rs:385-430` (task_toggle_complete)
- Modify: `crates/app-core/src/handlers/notes.rs:237-275` (note_create), `crates/app-core/src/handlers/notes.rs:277-309` (note_update)
- Modify: `crates/app-core/src/handlers/finance.rs:251-312` (finance_transaction_create)

All handlers follow the same pattern: after the mutation succeeds and before returning, publish to the domain event bus if available. Use `if let Ok(bus) = self.domain_event_bus()` so it's a no-op when the bus is disabled.

**Step 1: Add domain event emission to `task_create`**

In `crates/app-core/src/handlers/tasks.rs`, inside `task_create`, after `let mut task = action_to_task(...)` (around line 327) and before the final `Ok((task, updates))`:

```rust
        // Emit domain event for timeline tracking
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::TaskCreated {
                task_id: id.clone(),
                project: params.project_id.clone(),
                estimate_mins: None,
            });
        }
```

Add `use bus;` at the top of the file if not already present.

**Step 2: Add domain event emission to `task_toggle_complete`**

In `task_toggle_complete`, after the `ActionPatch` is applied and before returning, only when completing (not uncompleting):

```rust
        // Emit domain event when completing
        if new_status == "done" {
            if let Ok(bus) = self.domain_event_bus() {
                bus.publish(bus::DomainEvent::TaskCompleted {
                    task_id: id.clone(),
                    actual_duration_mins: None,
                    estimated_duration_mins: row.estimated_minutes.map(|m| m as i64),
                });
            }
        }
```

**Step 3: Add domain event emission to `note_create`**

In `crates/app-core/src/handlers/notes.rs`, inside `note_create`, before the final `Ok((response, updates))`:

```rust
        // Emit domain event for timeline tracking
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::NoteCreated {
                note_id: id.clone(),
                title: row.title.clone(),
            });
        }
```

**Step 4: Add domain event emission to `note_update`**

In `note_update`, before the final `Ok((response, updates))`:

```rust
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::NoteUpdated {
                note_id: params.id.clone(),
                title: updated.title.clone(),
            });
        }
```

**Step 5: Add domain event emission to `finance_transaction_create`**

In `crates/app-core/src/handlers/finance.rs`, inside `finance_transaction_create`, before the final `Ok((row, ...))`:

```rust
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::TransactionRecorded {
                category: row.category.clone().unwrap_or_default(),
                amount: row.amount as f64 / 100.0,
                is_over_budget: false,
            });
        }
```

**Step 6: Verify it compiles**

Run: `cargo build -p app-core`
Expected: SUCCESS

**Step 7: Commit**

```bash
git add crates/app-core/src/handlers/tasks.rs crates/app-core/src/handlers/notes.rs crates/app-core/src/handlers/finance.rs
git commit -m "feat(app-core): emit domain events from all mutation handlers"
```

---

## Task 3: Add `query_range` Method to EventLogRepo

**Files:**
- Modify: `crates/cognitive/src/repos/event_log.rs:34-146` (EventLogRepo impl)

**Step 1: Write the test**

At the bottom of the existing `mod tests` block in `event_log.rs`:

```rust
    #[tokio::test]
    async fn query_range_filters_by_date() {
        let pool = setup().await;
        let repo = EventLogRepo::new(pool);

        repo.insert_domain_event(
            "evt-a", "TaskCreated", "tasks", "extract",
            r#"{"task_id":"t1"}"#, "2026-03-08T10:00:00Z",
        ).await.unwrap();

        repo.insert_domain_event(
            "evt-b", "NoteCreated", "notes", "extract",
            r#"{"note_id":"n1"}"#, "2026-03-09T14:30:00Z",
        ).await.unwrap();

        repo.insert_domain_event(
            "evt-c", "TaskCompleted", "tasks", "extract",
            r#"{"task_id":"t2"}"#, "2026-03-10T08:00:00Z",
        ).await.unwrap();

        // Query only March 9
        let results = repo.query_domain_events_range("2026-03-09", "2026-03-09").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "evt-b");

        // Query March 8-9
        let results = repo.query_domain_events_range("2026-03-08", "2026-03-09").await.unwrap();
        assert_eq!(results.len(), 2);
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(query_range_filters_by_date)'`
Expected: FAIL — method `query_domain_events_range` not found

**Step 3: Implement `query_domain_events_range`**

In `EventLogRepo` impl block, add:

```rust
    /// Fetch domain events within a date range (inclusive), oldest first.
    /// Dates are ISO date strings like "2026-03-09".
    pub async fn query_domain_events_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DomainEventRow>, sqlx::Error> {
        sqlx::query_as::<_, DomainEventRow>(
            r#"
            SELECT id, event_type, domain, salience, payload, timestamp
            FROM domain_event_log
            WHERE date(timestamp) >= date(?1)
              AND date(timestamp) <= date(?2)
            ORDER BY timestamp ASC
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
    }
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(query_range_filters_by_date)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/cognitive/src/repos/event_log.rs
git commit -m "feat(cognitive): add query_domain_events_range to EventLogRepo"
```

---

## Task 4: Add `time_entries_in_range` Method to ActionRepo

**Files:**
- Modify: `crates/storage/src/repos/action_repo.rs` (ActionRepo impl)

**Step 1: Write the test**

Add a test in the existing `mod tests` block in `action_repo.rs`. First, check how the existing time entry tests are structured — look for the `add_time_entry` test and follow the same setup pattern. The test needs to insert time entries and query by date range.

```rust
    #[tokio::test]
    async fn time_entries_in_range_filters_by_date() {
        let pool = test_pool().await;
        let repo = ActionRepo::new(pool.inner().clone());

        // Create a task first
        let mut row = test_action_row("te-range-1");
        repo.add(&row).await.unwrap();

        // Add time entries on different dates
        repo.add_time_entry(&TimeEntryRow {
            id: "te-1".into(),
            action_id: "te-range-1".into(),
            started_at: "2026-03-08T10:00:00Z".into(),
            ended_at: Some("2026-03-08T11:00:00Z".into()),
            duration_secs: Some(3600),
            note: None,
        }).await.unwrap();

        repo.add_time_entry(&TimeEntryRow {
            id: "te-2".into(),
            action_id: "te-range-1".into(),
            started_at: "2026-03-09T14:00:00Z".into(),
            ended_at: Some("2026-03-09T15:30:00Z".into()),
            duration_secs: Some(5400),
            note: None,
        }).await.unwrap();

        repo.add_time_entry(&TimeEntryRow {
            id: "te-3".into(),
            action_id: "te-range-1".into(),
            started_at: "2026-03-10T09:00:00Z".into(),
            ended_at: Some("2026-03-10T09:45:00Z".into()),
            duration_secs: Some(2700),
            note: None,
        }).await.unwrap();

        let results = repo.time_entries_in_range("2026-03-09", "2026-03-09").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "te-2");
    }
```

Note: Check the exact `TimeEntryRow` struct fields and `test_action_row` helper before writing. Adapt as needed.

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(time_entries_in_range)'`
Expected: FAIL — method not found

**Step 3: Implement `time_entries_in_range`**

In `ActionRepo` impl, add:

```rust
    /// Fetch time entries within a date range (inclusive), oldest first.
    /// Returns entries with their parent action's title for display.
    pub async fn time_entries_in_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TimeEntryWithTask>, sqlx::Error> {
        sqlx::query_as::<_, TimeEntryWithTask>(
            r#"
            SELECT te.id, te.action_id, a.title as action_title,
                   te.started_at, te.ended_at, te.duration_secs, te.note
            FROM action_time_entries te
            JOIN actions a ON a.id = te.action_id
            WHERE date(te.started_at) >= date(?1)
              AND date(te.started_at) <= date(?2)
            ORDER BY te.started_at ASC
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
    }
```

Also add the return type struct near other time entry types:

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct TimeEntryWithTask {
    pub id: String,
    pub action_id: String,
    pub action_title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p storage -E 'test(time_entries_in_range)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/storage/src/repos/action_repo.rs
git commit -m "feat(storage): add time_entries_in_range to ActionRepo"
```

---

## Task 5: Add Timeline Types to desktop-shared

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs` (add after line ~809)
- Modify: `crates/desktop-shared/src/types.rs:48-59` (add Dashboard to EntityKind)

**Step 1: Add timeline types to `commands.rs`**

At the end of the file (before any closing brace), add:

```rust
// ── Timeline / Dashboard ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineQuery {
    pub start_date: String,
    pub end_date: String,
    pub sources: Option<Vec<TimelineSource>>,
    pub include_point_events: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineResponse {
    pub entries: Vec<TimelineEntry>,
    pub summary: TimelineSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub id: String,
    pub source: TimelineSource,
    pub entry_type: TimelineEntryType,
    pub title: String,
    pub description: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: Option<i64>,
    pub entity_id: Option<String>,
    pub entity_route: Option<String>,
    pub color: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineSource {
    Productivity,
    Focus,
    Task,
    Note,
    Finance,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineEntryType {
    AppUsage,
    FocusSession,
    TaskTimeEntry,
    TaskCreated,
    TaskCompleted,
    TaskUpdated,
    NoteCreated,
    NoteUpdated,
    TransactionRecorded,
    ExpenseRecorded,
    IncomeRecorded,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSummary {
    pub total_tracked_secs: i64,
    pub focus_secs: i64,
    pub tasks_completed: i64,
    pub tasks_created: i64,
    pub notes_touched: i64,
    pub transactions_count: i64,
    pub top_apps: Vec<TopAppSummary>,
    pub source_breakdown: Vec<SourceBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopAppSummary {
    pub app_name: String,
    pub duration_secs: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakdown {
    pub source: TimelineSource,
    pub duration_secs: i64,
    pub count: i64,
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: SUCCESS

**Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands.rs
git commit -m "feat(desktop-shared): add timeline query/response types"
```

---

## Task 6: Implement Timeline Handler in app-core

**Files:**
- Create: `crates/app-core/src/handlers/timeline.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` (add `pub mod timeline;`)

This is the core handler. It runs 4 parallel queries and normalizes results. Check exact field names on `ActivityEventRow` and `FocusSessionRow` from the productivity repos before writing — they may differ from the design doc.

**Step 1: Check productivity repo types**

Before writing code, read these files to understand the exact types:
- `crates/feature-productivity/src/repos/activity_repo.rs` — `ActivityEventRow` fields
- `crates/feature-productivity/src/repos/focus_repo.rs` — `FocusSessionRow` fields

Adapt the normalization functions to match the actual field names.

**Step 2: Create timeline.rs handler**

Create `crates/app-core/src/handlers/timeline.rs`:

```rust
use desktop_shared::commands::{
    TimelineEntry, TimelineEntryType, TimelineQuery, TimelineResponse, TimelineSource,
    TimelineSummary, TopAppSummary, SourceBreakdown,
};
use desktop_shared::errors::ApiError;
use std::collections::HashMap;

use crate::AppCore;

impl AppCore {
    pub async fn timeline_query(&self, params: TimelineQuery) -> Result<TimelineResponse, ApiError> {
        let start = &params.start_date;
        let end = &params.end_date;
        let include_point = params.include_point_events.unwrap_or(true);

        // Run queries in parallel across available sources
        let mut entries = Vec::new();

        // 1. Activity events (app tracking — duration blocks)
        if self.productivity_repos.is_some() {
            if let Ok(repos) = self.productivity_repos() {
                if let Ok(app_events) = repos.activity.query_range(start, end).await {
                    entries.extend(app_events.into_iter().map(|e| normalize_app_event(e)));
                }

                // 2. Focus sessions (duration blocks)
                if let Ok(sessions) = repos.focus.query_range(start, end).await {
                    entries.extend(sessions.into_iter().map(|s| normalize_focus_session(s)));
                }
            }
        }

        // 3. Task time entries (duration blocks)
        if let Ok(time_entries) = self.repos.actions.time_entries_in_range(start, end).await {
            entries.extend(time_entries.into_iter().map(|te| normalize_time_entry(te)));
        }

        // 4. Domain event log (point-in-time events)
        if include_point {
            if let Some(ref repo) = self.event_log_repo {
                if let Ok(events) = repo.query_domain_events_range(start, end).await {
                    entries.extend(events.into_iter().filter_map(|e| normalize_domain_event(e)));
                }
            }
        }

        // Apply source filter if specified
        if let Some(ref sources) = params.sources {
            entries.retain(|e| sources.contains(&e.source));
        }

        entries.sort_by(|a, b| a.started_at.cmp(&b.started_at));

        let summary = compute_summary(&entries);
        Ok(TimelineResponse { entries, summary })
    }
}

// ── Normalization functions ────────────────────────────────────────
// NOTE: Adapt these to the actual field names from the productivity repo types.
// The field names below are placeholders — verify against the actual structs.

fn normalize_app_event(e: feature_productivity::repos::ActivityEventRow) -> TimelineEntry {
    let category_color = match e.category_id.as_deref() {
        Some(_) => "var(--timeline-app-productive)",
        None => "var(--timeline-app-neutral)",
    };
    TimelineEntry {
        id: e.id,
        source: TimelineSource::Productivity,
        entry_type: TimelineEntryType::AppUsage,
        title: e.app_name.clone(),
        description: e.window_title,
        started_at: e.started_at,
        ended_at: Some(e.ended_at),
        duration_secs: Some(e.duration_secs),
        entity_id: None,
        entity_route: Some("/productivity".into()),
        color: category_color.into(),
        metadata: None,
    }
}

fn normalize_focus_session(s: feature_productivity::repos::FocusSessionRow) -> TimelineEntry {
    TimelineEntry {
        id: s.id,
        source: TimelineSource::Focus,
        entry_type: TimelineEntryType::FocusSession,
        title: "Focus Session".into(),
        description: s.session_type.clone(),
        started_at: s.started_at,
        ended_at: s.ended_at.clone(),
        duration_secs: s.duration_secs,
        entity_id: None,
        entity_route: Some("/productivity".into()),
        color: "var(--timeline-focus)".into(),
        metadata: None,
    }
}

fn normalize_time_entry(te: storage::repos::TimeEntryWithTask) -> TimelineEntry {
    TimelineEntry {
        id: te.id,
        source: TimelineSource::Task,
        entry_type: TimelineEntryType::TaskTimeEntry,
        title: te.action_title,
        description: te.note,
        started_at: te.started_at,
        ended_at: te.ended_at,
        duration_secs: te.duration_secs,
        entity_id: Some(te.action_id.clone()),
        entity_route: Some(format!("/task/{}", te.action_id)),
        color: "var(--timeline-task)".into(),
        metadata: None,
    }
}

fn normalize_domain_event(e: cognitive::DomainEventRow) -> Option<TimelineEntry> {
    let payload: serde_json::Value = serde_json::from_str(&e.payload).ok()?;

    let (entry_type, source, title, entity_id, entity_route, color) = match e.event_type.as_str() {
        "TaskCreated" => (
            TimelineEntryType::TaskCreated,
            TimelineSource::Task,
            format!("Task created: {}", payload.get("task_id").and_then(|v| v.as_str()).unwrap_or("?")),
            payload.get("task_id").and_then(|v| v.as_str()).map(String::from),
            payload.get("task_id").and_then(|v| v.as_str()).map(|id| format!("/task/{id}")),
            "var(--timeline-task)",
        ),
        "TaskCompleted" => (
            TimelineEntryType::TaskCompleted,
            TimelineSource::Task,
            format!("Task completed"),
            payload.get("task_id").and_then(|v| v.as_str()).map(String::from),
            payload.get("task_id").and_then(|v| v.as_str()).map(|id| format!("/task/{id}")),
            "var(--timeline-task)",
        ),
        "NoteCreated" => (
            TimelineEntryType::NoteCreated,
            TimelineSource::Note,
            format!("Note: {}", payload.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled")),
            payload.get("note_id").and_then(|v| v.as_str()).map(String::from),
            Some("/notes".into()),
            "var(--timeline-note)",
        ),
        "NoteUpdated" => (
            TimelineEntryType::NoteUpdated,
            TimelineSource::Note,
            format!("Edited: {}", payload.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled")),
            payload.get("note_id").and_then(|v| v.as_str()).map(String::from),
            Some("/notes".into()),
            "var(--timeline-note)",
        ),
        "TransactionRecorded" => (
            TimelineEntryType::TransactionRecorded,
            TimelineSource::Finance,
            format!("Transaction: {}", payload.get("category").and_then(|v| v.as_str()).unwrap_or("Uncategorized")),
            None,
            Some("/finance/transactions".into()),
            "var(--timeline-finance)",
        ),
        // Skip events we don't want on the timeline
        "ChatTurnCompleted" | "UserStatedFact" | "UserCorrectedAI"
        | "CoachingFeedback" | "ProductivityScoreComputed" => return None,
        // Other events as System
        _ => (
            TimelineEntryType::SystemEvent,
            TimelineSource::System,
            e.event_type.replace(|c: char| c.is_uppercase(), |c| format!(" {c}")).trim().into(),
            None,
            None,
            "var(--timeline-system)",
        ),
    };

    Some(TimelineEntry {
        id: e.id,
        source,
        entry_type,
        title,
        description: None,
        started_at: e.timestamp,
        ended_at: None,
        duration_secs: None,
        entity_id,
        entity_route,
        color: color.into(),
        metadata: Some(payload),
    })
}

// ── Summary computation ────────────────────────────────────────────

fn compute_summary(entries: &[TimelineEntry]) -> TimelineSummary {
    let mut total_tracked_secs: i64 = 0;
    let mut focus_secs: i64 = 0;
    let mut tasks_completed: i64 = 0;
    let mut tasks_created: i64 = 0;
    let mut notes_touched: i64 = 0;
    let mut transactions_count: i64 = 0;
    let mut app_durations: HashMap<String, i64> = HashMap::new();
    let mut source_durations: HashMap<String, (i64, i64)> = HashMap::new(); // (secs, count)

    for entry in entries {
        let dur = entry.duration_secs.unwrap_or(0);
        if dur > 0 {
            total_tracked_secs += dur;
        }

        // Per-source tracking
        let source_key = format!("{:?}", entry.source);
        let (s_dur, s_count) = source_durations.entry(source_key).or_insert((0, 0));
        *s_dur += dur;
        *s_count += 1;

        match entry.entry_type {
            TimelineEntryType::FocusSession => focus_secs += dur,
            TimelineEntryType::TaskCompleted => tasks_completed += 1,
            TimelineEntryType::TaskCreated => tasks_created += 1,
            TimelineEntryType::NoteCreated | TimelineEntryType::NoteUpdated => notes_touched += 1,
            TimelineEntryType::TransactionRecorded
            | TimelineEntryType::ExpenseRecorded
            | TimelineEntryType::IncomeRecorded => transactions_count += 1,
            TimelineEntryType::AppUsage => {
                *app_durations.entry(entry.title.clone()).or_insert(0) += dur;
            }
            _ => {}
        }
    }

    // Top 5 apps by duration
    let mut app_list: Vec<_> = app_durations.into_iter().collect();
    app_list.sort_by(|a, b| b.1.cmp(&a.1));
    let total_app_secs: i64 = app_list.iter().map(|(_, d)| d).sum();
    let top_apps: Vec<TopAppSummary> = app_list
        .into_iter()
        .take(5)
        .map(|(name, dur)| TopAppSummary {
            app_name: name,
            duration_secs: dur,
            percentage: if total_app_secs > 0 { dur as f64 / total_app_secs as f64 * 100.0 } else { 0.0 },
        })
        .collect();

    let source_breakdown: Vec<SourceBreakdown> = source_durations
        .into_iter()
        .map(|(source_str, (dur, count))| {
            let source = match source_str.as_str() {
                "Productivity" => TimelineSource::Productivity,
                "Focus" => TimelineSource::Focus,
                "Task" => TimelineSource::Task,
                "Note" => TimelineSource::Note,
                "Finance" => TimelineSource::Finance,
                _ => TimelineSource::System,
            };
            SourceBreakdown { source, duration_secs: dur, count }
        })
        .collect();

    TimelineSummary {
        total_tracked_secs,
        focus_secs,
        tasks_completed,
        tasks_created,
        notes_touched,
        transactions_count,
        top_apps,
        source_breakdown,
    }
}
```

**Important:** Before writing this file, read the actual productivity repo types:
- `crates/feature-productivity/src/repos/activity_repo.rs` for `ActivityEventRow` exact field names
- `crates/feature-productivity/src/repos/focus_repo.rs` for `FocusSessionRow` exact field names
- Verify they have `query_range` methods or add them (similar to Task 3)

**Step 3: Register the module in mod.rs**

In `crates/app-core/src/handlers/mod.rs`, add: `pub mod timeline;`

**Step 4: Verify it compiles**

Run: `cargo build -p app-core`
Expected: SUCCESS (may need field name fixes)

**Step 5: Commit**

```bash
git add crates/app-core/src/handlers/timeline.rs crates/app-core/src/handlers/mod.rs
git commit -m "feat(app-core): add timeline_query handler with 4-source aggregation"
```

---

## Task 7: Add Tauri Command and Dev Server Route

**Files:**
- Create: `crates/desktop/src/commands/timeline.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (add `pub mod timeline;`)
- Modify: `crates/desktop/src/main.rs:144` (register command in `generate_handler!`)
- Modify: `crates/desktop/src/dev_server.rs:146` (add dispatch)

**Step 1: Create timeline Tauri command**

Create `crates/desktop/src/commands/timeline.rs`:

```rust
use desktop_shared::commands::{TimelineQuery, TimelineResponse};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn timeline_query(
    state: State<'_, Arc<AppCore>>,
    params: TimelineQuery,
) -> Result<TimelineResponse, ApiError> {
    state.timeline_query(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["timeline_query"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "timeline_query" => dev::val(core.timeline_query(try_field!(dev::parse_params(body))).await),
        _ => return None,
    })
}
```

Note: Check that the `try_field!` macro is imported — look at how other command files import it (e.g., `use super::dev_helpers::{self as dev, try_field};`).

**Step 2: Register in mod.rs**

Add to `crates/desktop/src/commands/mod.rs`: `pub mod timeline;`

**Step 3: Register in main.rs**

In `crates/desktop/src/main.rs`, add inside the `generate_handler!` macro:
```rust
            // Timeline / Dashboard
            commands::timeline::timeline_query,
```

**Step 4: Register in dev_server.rs**

In `crates/desktop/src/dev_server.rs`, add a new dispatch line after the existing ones (around line 178):
```rust
    if let Some(r) = commands::timeline::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
```

**Step 5: Verify it compiles**

Run: `cargo build -p desktop`
Expected: SUCCESS

**Step 6: Commit**

```bash
git add crates/desktop/src/commands/timeline.rs crates/desktop/src/commands/mod.rs crates/desktop/src/main.rs crates/desktop/src/dev_server.rs
git commit -m "feat(desktop): add timeline_query Tauri command and dev server route"
```

---

## Task 8: Add Timeline Color Tokens to Theme CSS

**Files:**
- Modify: `desktop-ui/src/styles/theme.css` (add tokens to `:root` and `@theme inline`)

**Step 1: Add CSS variables to `:root`**

In `desktop-ui/src/styles/theme.css`, add inside the `:root` block (after the existing semantic colors):

```css
  /* Timeline / Dashboard */
  --timeline-app-productive: oklch(0.72 0.15 145);
  --timeline-app-distracting: oklch(0.65 0.18 25);
  --timeline-app-neutral: oklch(0.55 0.02 250);
  --timeline-focus: oklch(0.65 0.18 290);
  --timeline-task: oklch(0.68 0.16 240);
  --timeline-note: oklch(0.75 0.14 80);
  --timeline-finance: oklch(0.72 0.16 160);
  --timeline-system: oklch(0.55 0.02 250);
```

**Step 2: Register in `@theme inline`**

Add inside the `@theme inline` block:

```css
    --color-timeline-app-productive: var(--timeline-app-productive);
    --color-timeline-app-distracting: var(--timeline-app-distracting);
    --color-timeline-app-neutral: var(--timeline-app-neutral);
    --color-timeline-focus: var(--timeline-focus);
    --color-timeline-task: var(--timeline-task);
    --color-timeline-note: var(--timeline-note);
    --color-timeline-finance: var(--timeline-finance);
    --color-timeline-system: var(--timeline-system);
```

**Step 3: Verify build**

Run: `cd desktop-ui && bun run build`
Expected: SUCCESS

**Step 4: Commit**

```bash
git add desktop-ui/src/styles/theme.css
git commit -m "feat(ui): add timeline color tokens to theme"
```

---

## Task 9: Add TypeScript Timeline Types

**Files:**
- Modify: `desktop-ui/src/lib/types.ts` (add timeline interfaces)

**Step 1: Add TypeScript types**

Add at the end of `desktop-ui/src/lib/types.ts`:

```typescript
// ── Timeline / Dashboard ──────────────────────────────────────────

export type TimelineSource =
  | "productivity"
  | "focus"
  | "task"
  | "note"
  | "finance"
  | "system";

export type TimelineEntryType =
  | "appUsage"
  | "focusSession"
  | "taskTimeEntry"
  | "taskCreated"
  | "taskCompleted"
  | "taskUpdated"
  | "noteCreated"
  | "noteUpdated"
  | "transactionRecorded"
  | "expenseRecorded"
  | "incomeRecorded"
  | "systemEvent";

export interface TimelineEntry {
  id: string;
  source: TimelineSource;
  entryType: TimelineEntryType;
  title: string;
  description?: string;
  startedAt: string;
  endedAt?: string;
  durationSecs?: number;
  entityId?: string;
  entityRoute?: string;
  color: string;
  metadata?: Record<string, unknown>;
}

export interface TopAppSummary {
  appName: string;
  durationSecs: number;
  percentage: number;
}

export interface SourceBreakdown {
  source: TimelineSource;
  durationSecs: number;
  count: number;
}

export interface TimelineSummary {
  totalTrackedSecs: number;
  focusSecs: number;
  tasksCompleted: number;
  tasksCreated: number;
  notesTouched: number;
  transactionsCount: number;
  topApps: TopAppSummary[];
  sourceBreakdown: SourceBreakdown[];
}

export interface TimelineResponse {
  entries: TimelineEntry[];
  summary: TimelineSummary;
}

export interface TimelineQuery {
  startDate: string;
  endDate: string;
  sources?: TimelineSource[];
  includePointEvents?: boolean;
}
```

**Step 2: Add "Dashboard" to SidebarItem type**

Find the `SidebarItem` type (around line 916) and add `"Dashboard"`:

```typescript
export type SidebarItem =
  | "Dashboard"
  | "Chat"
  | "Tasks"
  // ... rest unchanged
```

**Step 3: Commit**

```bash
git add desktop-ui/src/lib/types.ts
git commit -m "feat(ui): add timeline TypeScript types and Dashboard sidebar item"
```

---

## Task 10: Update Routing — Dashboard at `/`, Tasks at `/tasks`

**Files:**
- Modify: `desktop-ui/src/App.tsx:165-245` (router)
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx:22-29` (nav items)
- Modify: `desktop-ui/src/components/layout/AppShell.tsx:38-62` (active item derivation)

**Step 1: Update App.tsx routes**

In `desktop-ui/src/App.tsx`:

Add lazy imports at the top (near existing lazy imports):

```typescript
const DashboardDayPage = lazy(() =>
  import("./components/dashboard/DayCalendarView").then((m) => ({ default: m.DayCalendarView })),
);
const DashboardWeekPage = lazy(() =>
  import("./components/dashboard/WeekCalendarView").then((m) => ({ default: m.WeekCalendarView })),
);
const DashboardMonthPage = lazy(() =>
  import("./components/dashboard/MonthCalendarView").then((m) => ({ default: m.MonthCalendarView })),
);
const DashboardYearPage = lazy(() =>
  import("./components/dashboard/YearHeatmapView").then((m) => ({ default: m.YearHeatmapView })),
);
const DashboardLayout = lazy(() =>
  import("./components/dashboard/DashboardLayout").then((m) => ({ default: m.DashboardLayout })),
);
```

Replace the routes. Change `{ path: "/", element: <MainApp /> }` to a dashboard redirect, and add dashboard + tasks routes:

```typescript
      // Dashboard (home)
      { path: "/", element: <DashboardRedirect /> },
      { path: "/day/:date", element: <DashboardLayout><DashboardDayPage /></DashboardLayout> },
      { path: "/week/:date", element: <DashboardLayout><DashboardWeekPage /></DashboardLayout> },
      { path: "/month/:date", element: <DashboardLayout><DashboardMonthPage /></DashboardLayout> },
      { path: "/year/:year", element: <DashboardLayout><DashboardYearPage /></DashboardLayout> },
      // Tasks (moved from /)
      { path: "/tasks", element: <MainApp /> },
```

Add a `DashboardRedirect` component (either inline or in a shared file):

```typescript
function DashboardRedirect() {
  const today = new Date().toISOString().slice(0, 10);
  return <Navigate to={`/day/${today}`} replace />;
}
```

**Step 2: Update Sidebar**

In `desktop-ui/src/components/layout/Sidebar.tsx`, update the `items` array:

```typescript
import { LayoutDashboard } from "lucide-react"; // Add import

const items = [
  { key: "Chat", icon: MessageSquare, path: "/chat" },
  { key: "Dashboard", icon: LayoutDashboard, path: "/" },
  { key: "Tasks", icon: CheckSquare, path: "/tasks" },
  { key: "Notes", icon: FileText, path: "/notes" },
  { key: "Finance", icon: Wallet, path: "/finance" },
  { key: "Productivity", icon: Activity, path: "/productivity" },
  { key: "Debug", icon: Bug, path: "/debug", bottom: true },
  { key: "Settings", icon: Settings, path: "/settings", bottom: true },
];
```

**Step 3: Update AppShell active item derivation**

In `desktop-ui/src/components/layout/AppShell.tsx`, update the `activeSidebarItem` memo:

```typescript
  const activeSidebarItem = useMemo((): SidebarItem => {
    const path = location.pathname;
    if (path.startsWith("/chat")) return "Chat";
    if (path.startsWith("/tasks") || path.startsWith("/task/")) return "Tasks";
    if (path.startsWith("/notes")) return "Notes";
    if (path.startsWith("/finance")) return "Finance";
    if (path.startsWith("/productivity")) return "Productivity";
    if (path.startsWith("/settings")) return "Settings";
    if (path.startsWith("/debug")) return "Debug";
    // Dashboard routes: /day/*, /week/*, /month/*, /year/*, /
    return "Dashboard";
  }, [location.pathname]);
```

Also update the `viewContext` memo to handle the new routes:

```typescript
  const viewContext = useMemo(() => {
    const path = location.pathname;
    if (path.startsWith("/tasks")) return { entityKind: "tasks" };
    if (path.startsWith("/day/") || path.startsWith("/week/") || path.startsWith("/month/") || path.startsWith("/year/")) {
      return { entityKind: "dashboard" };
    }
    // ... rest unchanged
  }, [location.pathname, location.search]);
```

**Step 4: Verify dev server**

Run: `cd desktop-ui && bun run build`
Expected: SUCCESS (will fail until placeholder components exist — create them in next tasks)

**Step 5: Commit**

```bash
git add desktop-ui/src/App.tsx desktop-ui/src/components/layout/Sidebar.tsx desktop-ui/src/components/layout/AppShell.tsx
git commit -m "feat(ui): update routing — dashboard at /, tasks at /tasks"
```

---

## Task 11: Build DashboardLayout Component

**Files:**
- Create: `desktop-ui/src/components/dashboard/DashboardLayout.tsx`

This mirrors the existing `ProductivityLayout` pattern — a navigation shell with view switcher and date navigator.

**Step 1: Create the component**

```typescript
import { ChevronLeft, ChevronRight } from "lucide-react";
import type { ReactNode } from "react";
import { useNavigate, useParams, useLocation } from "react-router";
import { cn } from "../../lib/utils";

type ViewMode = "day" | "week" | "month" | "year";

function getViewMode(pathname: string): ViewMode {
  if (pathname.startsWith("/week/")) return "week";
  if (pathname.startsWith("/month/")) return "month";
  if (pathname.startsWith("/year/")) return "year";
  return "day";
}

function formatDateDisplay(mode: ViewMode, param: string): string {
  if (mode === "year") return param;
  const date = new Date(param + "T00:00:00");
  if (mode === "day") {
    return date.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric", year: "numeric" });
  }
  if (mode === "week") {
    const end = new Date(date);
    end.setDate(end.getDate() + 6);
    return `${date.toLocaleDateString("en-US", { month: "short", day: "numeric" })} – ${end.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}`;
  }
  // month
  return date.toLocaleDateString("en-US", { month: "long", year: "numeric" });
}

export function DashboardLayout({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const location = useLocation();
  const params = useParams<{ date?: string; year?: string }>();
  const mode = getViewMode(location.pathname);
  const dateParam = params.date || params.year || new Date().toISOString().slice(0, 10);

  const navigateToView = (view: ViewMode) => {
    const today = new Date().toISOString().slice(0, 10);
    switch (view) {
      case "day": navigate(`/day/${today}`); break;
      case "week": navigate(`/week/${today}`); break;
      case "month": navigate(`/month/${today}`); break;
      case "year": navigate(`/year/${new Date().getFullYear()}`); break;
    }
  };

  const navigatePrev = () => {
    const d = new Date(dateParam + "T00:00:00");
    switch (mode) {
      case "day": d.setDate(d.getDate() - 1); break;
      case "week": d.setDate(d.getDate() - 7); break;
      case "month": d.setMonth(d.getMonth() - 1); break;
      case "year": d.setFullYear(d.getFullYear() - 1); break;
    }
    const iso = mode === "year" ? String(d.getFullYear()) : d.toISOString().slice(0, 10);
    navigate(`/${mode}/${iso}`);
  };

  const navigateNext = () => {
    const d = new Date(dateParam + "T00:00:00");
    switch (mode) {
      case "day": d.setDate(d.getDate() + 1); break;
      case "week": d.setDate(d.getDate() + 7); break;
      case "month": d.setMonth(d.getMonth() + 1); break;
      case "year": d.setFullYear(d.getFullYear() + 1); break;
    }
    const iso = mode === "year" ? String(d.getFullYear()) : d.toISOString().slice(0, 10);
    navigate(`/${mode}/${iso}`);
  };

  const navigateToday = () => {
    const today = new Date().toISOString().slice(0, 10);
    const iso = mode === "year" ? String(new Date().getFullYear()) : today;
    navigate(`/${mode}/${iso}`);
  };

  const views: { key: ViewMode; label: string }[] = [
    { key: "day", label: "Day" },
    { key: "week", label: "Week" },
    { key: "month", label: "Month" },
    { key: "year", label: "Year" },
  ];

  return (
    <div className="flex-1 flex flex-col gap-2 min-w-0">
      {/* Top bar */}
      <div className="glass-card px-4 py-2 flex items-center justify-between">
        <div className="flex items-center gap-2">
          {views.map((v) => (
            <button
              key={v.key}
              type="button"
              onClick={() => navigateToView(v.key)}
              className={cn(
                "px-3 py-1 rounded-lg text-xs font-medium transition-all",
                mode === v.key
                  ? "glass-button-active text-brand"
                  : "text-muted hover:text-secondary hover:bg-white/[0.05]",
              )}
            >
              {v.label}
            </button>
          ))}
        </div>

        <span className="text-sm font-medium text-primary">
          {formatDateDisplay(mode, dateParam)}
        </span>

        <div className="flex items-center gap-1">
          <button type="button" onClick={navigatePrev} className="p-1 rounded hover:bg-white/[0.05] text-muted hover:text-secondary">
            <ChevronLeft className="w-4 h-4" />
          </button>
          <button type="button" onClick={navigateToday} className="px-2 py-1 rounded text-xs text-muted hover:text-secondary hover:bg-white/[0.05]">
            Today
          </button>
          <button type="button" onClick={navigateNext} className="p-1 rounded hover:bg-white/[0.05] text-muted hover:text-secondary">
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {children}
      </div>
    </div>
  );
}
```

**Step 2: Commit**

```bash
git add desktop-ui/src/components/dashboard/DashboardLayout.tsx
git commit -m "feat(ui): add DashboardLayout with view switcher and date navigation"
```

---

## Task 12: Build SummaryPanel Component

**Files:**
- Create: `desktop-ui/src/components/dashboard/SummaryPanel.tsx`

**Step 1: Create the component**

This is the context-sensitive right panel — shows aggregate stats by default, entity detail on block click. Build it with a `selectedEntry` prop.

```typescript
import { Clock, CheckCircle, FileText, DollarSign, ExternalLink, X } from "lucide-react";
import type { TimelineEntry, TimelineSummary } from "../../lib/types";

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

interface SummaryPanelProps {
  summary: TimelineSummary | null;
  selectedEntry: TimelineEntry | null;
  onClose: () => void;
}

export function SummaryPanel({ summary, selectedEntry, onClose }: SummaryPanelProps) {
  if (selectedEntry) {
    return <EntryDetail entry={selectedEntry} onClose={onClose} />;
  }
  if (!summary) return null;
  return <DefaultSummary summary={summary} />;
}

function DefaultSummary({ summary }: { summary: TimelineSummary }) {
  return (
    <div className="w-72 glass-card p-4 flex flex-col gap-4 overflow-y-auto">
      <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">Summary</h3>

      {/* Total tracked time */}
      <div>
        <div className="text-2xl font-bold text-primary">{formatDuration(summary.totalTrackedSecs)}</div>
        <div className="text-xs text-muted">tracked</div>
      </div>

      {/* Quick stats */}
      <div className="grid grid-cols-2 gap-2">
        <Stat icon={<Clock className="w-3.5 h-3.5" />} label="Focus" value={formatDuration(summary.focusSecs)} />
        <Stat icon={<CheckCircle className="w-3.5 h-3.5" />} label="Completed" value={String(summary.tasksCompleted)} />
        <Stat icon={<FileText className="w-3.5 h-3.5" />} label="Notes" value={String(summary.notesTouched)} />
        <Stat icon={<DollarSign className="w-3.5 h-3.5" />} label="Transactions" value={String(summary.transactionsCount)} />
      </div>

      {/* Top apps */}
      {summary.topApps.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Top Apps</h4>
          <div className="flex flex-col gap-1.5">
            {summary.topApps.map((app) => (
              <div key={app.appName} className="flex items-center justify-between text-xs">
                <span className="text-secondary truncate">{app.appName}</span>
                <span className="text-muted">{formatDuration(app.durationSecs)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Source breakdown */}
      {summary.sourceBreakdown.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted mb-2">Breakdown</h4>
          <div className="flex flex-col gap-1.5">
            {summary.sourceBreakdown.map((s) => (
              <div key={s.source} className="flex items-center justify-between text-xs">
                <span className="text-secondary capitalize">{s.source}</span>
                <span className="text-muted">{s.count} items</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function EntryDetail({ entry, onClose }: { entry: TimelineEntry; onClose: () => void }) {
  const navigate = (await import("react-router")).useNavigate();

  return (
    <div className="w-72 glass-card p-4 flex flex-col gap-3 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-muted uppercase tracking-wider">Details</h3>
        <button type="button" onClick={onClose} className="text-muted hover:text-secondary">
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: entry.color }} />
        <span className="text-sm font-medium text-primary">{entry.title}</span>
      </div>

      {entry.description && (
        <p className="text-xs text-muted">{entry.description}</p>
      )}

      <div className="text-xs text-muted space-y-1">
        <div>Started: {new Date(entry.startedAt).toLocaleTimeString()}</div>
        {entry.endedAt && <div>Ended: {new Date(entry.endedAt).toLocaleTimeString()}</div>}
        {entry.durationSecs && <div>Duration: {formatDuration(entry.durationSecs)}</div>}
        <div className="capitalize">Source: {entry.source}</div>
      </div>

      {entry.entityRoute && (
        <button
          type="button"
          onClick={() => navigate(entry.entityRoute!)}
          className="flex items-center gap-1.5 text-xs text-brand hover:underline mt-1"
        >
          <ExternalLink className="w-3.5 h-3.5" />
          Open {entry.source}
        </button>
      )}
    </div>
  );
}

function Stat({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="glass-card p-2 flex flex-col gap-0.5">
      <div className="flex items-center gap-1 text-muted">{icon}<span className="text-[10px]">{label}</span></div>
      <div className="text-sm font-semibold text-primary">{value}</div>
    </div>
  );
}
```

**Note:** The `EntryDetail` component uses a dynamic import for `useNavigate` which won't work — use a regular import at the top. This is a known pattern issue. Fix: import `useNavigate` at the top and pass it as a prop, or restructure so `EntryDetail` is a separate component file.

**Step 2: Commit**

```bash
git add desktop-ui/src/components/dashboard/SummaryPanel.tsx
git commit -m "feat(ui): add context-sensitive SummaryPanel component"
```

---

## Task 13: Build DayCalendarView Component

**Files:**
- Create: `desktop-ui/src/components/dashboard/DayCalendarView.tsx`

This is the primary view — a 24h vertical time axis with blocks and thin cards. This is the largest UI component.

**Step 1: Create the component**

The DayCalendarView should:
1. Fetch timeline data via `useQuery("timeline_query", { startDate, endDate })` where both are the same day
2. Render a scrollable 24h grid with hour markers on the left
3. Position duration blocks by their `startedAt`/`endedAt` times
4. Render thin cards (15min height) for point-in-time events
5. Show a red "current time" line for today
6. Handle click on blocks to select them (passed up to parent for SummaryPanel)
7. Handle overlap by stacking blocks side-by-side

Key calculations:
- `HOUR_HEIGHT = 60px` (each hour = 60px, total grid = 1440px)
- Block top = `(hour * 60 + minutes) * (HOUR_HEIGHT / 60)`
- Block height = `durationSecs / 60 * (HOUR_HEIGHT / 60)` or minimum 15px for thin cards

This is a complex component — implement iteratively. Start with the grid and time markers, then add block rendering, then interactions.

**Step 2: Commit**

```bash
git add desktop-ui/src/components/dashboard/DayCalendarView.tsx
git commit -m "feat(ui): add DayCalendarView with time blocks and thin cards"
```

---

## Task 14: Build WeekCalendarView Component

**Files:**
- Create: `desktop-ui/src/components/dashboard/WeekCalendarView.tsx`

Similar to DayCalendarView but with 7 columns. Query `startDate` = Monday, `endDate` = Sunday. Each column is a day. Blocks are narrower. Point-in-time events show as colored dots with tooltip.

**Step 1: Create the component**

Reuse the same hour grid logic from DayCalendarView. Add a header row with day names + dates.

**Step 2: Commit**

```bash
git add desktop-ui/src/components/dashboard/WeekCalendarView.tsx
git commit -m "feat(ui): add WeekCalendarView with 7-day columns"
```

---

## Task 15: Build MonthCalendarView Component

**Files:**
- Create: `desktop-ui/src/components/dashboard/MonthCalendarView.tsx`

Standard calendar grid. Each day cell shows stacked mini color bars + count badges. Click navigates to day view.

**Step 1: Create the component**

**Step 2: Commit**

```bash
git add desktop-ui/src/components/dashboard/MonthCalendarView.tsx
git commit -m "feat(ui): add MonthCalendarView with activity cells"
```

---

## Task 16: Build YearHeatmapView Component

**Files:**
- Create: `desktop-ui/src/components/dashboard/YearHeatmapView.tsx`

12 mini-month heatmap grids. Cell color intensity = total tracked hours that day.

**Step 1: Create the component**

**Step 2: Commit**

```bash
git add desktop-ui/src/components/dashboard/YearHeatmapView.tsx
git commit -m "feat(ui): add YearHeatmapView with activity heatmaps"
```

---

## Task 17: Integration Testing and Polish

**Step 1: Run full build**

```bash
cargo build --workspace
cd desktop-ui && bun run build
```

**Step 2: Run linters**

```bash
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
cd desktop-ui && bun run lint:fix
```

**Step 3: Run tests**

```bash
cargo nextest run --workspace
```

**Step 4: Manual testing**

Run: `cargo run -p dev-api` + `cd desktop-ui && bun run dev`

Verify:
- `/` redirects to `/day/{today}`
- DayCalendarView renders with time axis
- Blocks appear for app tracking data (if productivity is enabled)
- Point-in-time events appear as thin cards
- Summary panel shows aggregate stats
- Clicking a block shows entry detail
- Day/Week/Month/Year switching works
- Sidebar highlights Dashboard correctly
- `/tasks` shows the task table view
- `/productivity` still works as before

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: unified activity dashboard with time-block calendar"
```
