# Activity Columns Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace App Activity and Point Events columns with Tasks (due/created/completed), Transactions, and Note Activity columns, turning the dashboard into a holistic "what you actually did" view.

**Architecture:** Extend `timeline_query` with 3 new direct-query pipelines (Todo, Finance, Notes) that bypass the domain events log and query source tables directly. Remove overlapping domain event entries to avoid duplicates. Frontend gets 5 columns: Focus, Time Entries, Tasks, Transactions, Notes.

**Tech Stack:** Rust (sqlx, chrono, serde) for backend pipelines; TypeScript/React for frontend columns; Tailwind v4 CSS tokens for theming.

---

### Task 1: Add `TimelineSource::Todo` and `TimelineEntryType::TaskDue`

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs:1059-1083`
- Modify: `desktop-ui/src/lib/types.ts:1086-1100`

**Step 1: Add Rust enum variants**

In `crates/desktop-shared/src/commands.rs`, add `Todo` to `TimelineSource` (after `Task`):

```rust
pub enum TimelineSource {
    Productivity,
    Focus,
    Task,
    Todo,       // ← NEW: tasks due/created/completed
    Note,
    Finance,
    System,
}
```

Add `TaskDue` to `TimelineEntryType` (after `TaskUpdated`):

```rust
pub enum TimelineEntryType {
    AppUsage,
    FocusSession,
    TaskTimeEntry,
    TaskCreated,
    TaskCompleted,
    TaskUpdated,
    TaskDue,          // ← NEW
    NoteCreated,
    NoteUpdated,
    TransactionRecorded,
    ExpenseRecorded,
    IncomeRecorded,
    SystemEvent,
}
```

**Step 2: Add TypeScript types**

In `desktop-ui/src/lib/types.ts:1086`, add `"todo"` to `TimelineSource`:

```typescript
export type TimelineSource = "productivity" | "focus" | "task" | "todo" | "note" | "finance" | "system";
```

Add `"taskDue"` to `TimelineEntryType`:

```typescript
export type TimelineEntryType =
  | "appUsage"
  | "focusSession"
  | "taskTimeEntry"
  | "taskCreated"
  | "taskCompleted"
  | "taskUpdated"
  | "taskDue"
  | "noteCreated"
  | "noteUpdated"
  | "transactionRecorded"
  | "expenseRecorded"
  | "incomeRecorded"
  | "systemEvent";
```

**Step 3: Build to verify**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo build -p desktop-shared 2>&1 | tail -5`
Expected: build succeeds

**Step 4: Commit**

```bash
git add crates/desktop-shared/src/commands.rs desktop-ui/src/lib/types.ts
git commit -m "feat(timeline): add Todo source and TaskDue entry type"
```

---

### Task 2: Add `tasks_for_timeline` repo method

**Files:**
- Modify: `crates/storage/src/repos/action_repo.rs` (after `time_entries_in_range` at ~L720)

**Step 1: Add the repo method**

This queries tasks where `due_date` falls in range, OR `created_at` falls in range, OR `completed_at` falls in range. Returns `ActionRow` directly — normalization happens in the timeline handler.

```rust
/// Fetch tasks relevant to a date range for the timeline:
/// due on the date, created during the range, or completed during the range.
pub async fn tasks_for_timeline(
    &self,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<ActionRow>, StorageError> {
    let end_bound = format!("{end_date}T23:59:59Z");
    let rows = sqlx::query_as::<_, ActionRow>(
        r#"
        SELECT * FROM actions
        WHERE is_template = 0 AND (
            (due_date >= ?1 AND due_date < ?2)
            OR (created_at >= ?1 AND created_at < ?2)
            OR (completed_at >= ?1 AND completed_at < ?2)
        )
        ORDER BY COALESCE(due_date, created_at) ASC
        "#,
    )
    .bind(start_date)
    .bind(&end_bound)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

**Step 2: Build to verify**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo build -p storage 2>&1 | tail -5`
Expected: build succeeds

**Step 3: Commit**

```bash
git add crates/storage/src/repos/action_repo.rs
git commit -m "feat(storage): add tasks_for_timeline query"
```

---

### Task 3: Add `notes_in_date_range` repo method

**Files:**
- Modify: `crates/feature-notes/src/repo.rs` (after `list_notes` at ~L76)

**Step 1: Add the repo method**

NoteRow timestamps are `String` (ISO format), so we use string comparison like the existing time_entries_in_range pattern.

```rust
/// Fetch notes created or updated within a date range (for timeline display).
pub async fn notes_in_date_range(
    &self,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<NoteRow>, StorageError> {
    let end_bound = format!("{end_date}T23:59:59Z");
    let rows = sqlx::query_as::<_, NoteRow>(
        r#"
        SELECT * FROM notes
        WHERE archived = 0
          AND (created_at >= ?1 AND created_at < ?2
               OR updated_at >= ?1 AND updated_at < ?2)
        ORDER BY COALESCE(updated_at, created_at) ASC
        "#,
    )
    .bind(start_date)
    .bind(&end_bound)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows)
}
```

**Step 2: Build to verify**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo build -p feature-notes 2>&1 | tail -5`
Expected: build succeeds

**Step 3: Commit**

```bash
git add crates/feature-notes/src/repo.rs
git commit -m "feat(notes): add notes_in_date_range query for timeline"
```

---

### Task 4: Add new pipelines to `timeline_query` handler

**Files:**
- Modify: `crates/app-core/src/handlers/timeline.rs`

**Step 1: Add `use` for new types**

The handler already imports from `desktop_shared::commands`. Add `storage::ActionRow` and `feature_notes::models::NoteRow` if not already in scope. Also need `chrono::NaiveDate` for finance filter.

At the top of the file, update imports:

```rust
use desktop_shared::commands::{
    SourceBreakdown, TimelineEntry, TimelineEntryType, TimelineQuery, TimelineResponse,
    TimelineSource, TimelineSummary, TopAppSummary,
};
use desktop_shared::errors::ApiError;
use std::collections::HashMap;

use crate::AppCore;
```

**Step 2: Add Pipeline 4 — Todo (tasks due/created/completed)**

After Pipeline 2 (task time entries) at ~L51, add:

```rust
// 4. Tasks due/created/completed in range (direct query)
if want(sources, TimelineSource::Todo) {
    if let Ok(tasks) = self.repos.actions.tasks_for_timeline(start, end).await {
        entries.extend(tasks.into_iter().flat_map(|t| normalize_task(t, start, end)));
    }
}
```

**Step 3: Add Pipeline 5 — Finance (direct transaction query)**

```rust
// 5. Financial transactions (direct query)
if want(sources, TimelineSource::Finance) {
    let filter = storage::FinanceTransactionFilter {
        date_from: chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").ok(),
        date_to: chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d").ok(),
        limit: Some(100),
        ..Default::default()
    };
    if let Ok(txs) = self.repos.finance.transactions.list(&filter).await {
        entries.extend(txs.into_iter().map(normalize_transaction));
    }
}
```

**Step 4: Add Pipeline 6 — Notes (direct query)**

```rust
// 6. Note activity (direct query)
if want(sources, TimelineSource::Note) {
    if let Ok(notes) = self.note_repo.notes_in_date_range(start, end).await {
        entries.extend(notes.into_iter().map(|n| normalize_note_activity(n, start)));
    }
}
```

**Step 5: Update domain events pipeline to skip duplicates**

In the existing domain events section (~L53-66), after collecting `domain_entries`, filter out types that are now handled by direct pipelines:

```rust
// 3. Domain event log (point-in-time events — may produce Task/Note/Finance/System)
if include_point {
    if let Some(ref repo) = self.event_log_repo {
        if let Ok(events) = repo.query_domain_events_range(start, end).await {
            let mut domain_entries: Vec<_> =
                events.into_iter().filter_map(normalize_domain_event).collect();
            // Apply source filter to domain events since they span multiple sources
            if let Some(src_list) = sources {
                domain_entries.retain(|e| src_list.contains(&e.source));
            }
            // Remove entries now handled by direct pipelines to avoid duplicates
            domain_entries.retain(|e| !matches!(
                e.entry_type,
                TimelineEntryType::TaskCreated
                | TimelineEntryType::TaskCompleted
                | TimelineEntryType::NoteCreated
                | TimelineEntryType::NoteUpdated
                | TimelineEntryType::TransactionRecorded
            ));
            entries.extend(domain_entries);
        }
    }
}
```

**Step 6: Add normalizer functions**

Add these after the existing `normalize_domain_event` function:

```rust
/// Normalize a task row into 1+ timeline entries.
/// A task may produce multiple entries: one for due date, one for created, one for completed.
fn normalize_task(t: storage::ActionRow, start: &str, end: &str) -> Vec<TimelineEntry> {
    let mut out = Vec::new();
    let start_bound = format!("{start}T00:00:00Z");
    let end_bound = format!("{end}T23:59:59Z");
    let task_route = format!("/task/{}", t.id);

    // Task due on this date
    if let Some(ref due) = t.due_date {
        let due_str = due.to_rfc3339();
        if due_str >= start_bound && due_str <= end_bound {
            out.push(TimelineEntry {
                id: format!("{}-due", t.id),
                source: TimelineSource::Todo,
                entry_type: TimelineEntryType::TaskDue,
                title: t.title.clone(),
                description: t.description.clone(),
                started_at: due_str,
                ended_at: None,
                duration_secs: t.estimated_minutes.map(|m| m as i64 * 60),
                entity_id: Some(t.id.clone()),
                entity_route: Some(task_route.clone()),
                color: "var(--timeline-todo)".into(),
                metadata: Some(serde_json::json!({
                    "status": t.status,
                    "priority": t.priority,
                })),
            });
        }
    }

    // Task created in range
    let created_str = t.created_at.to_rfc3339();
    if created_str >= start_bound && created_str <= end_bound {
        out.push(TimelineEntry {
            id: format!("{}-created", t.id),
            source: TimelineSource::Todo,
            entry_type: TimelineEntryType::TaskCreated,
            title: format!("Created: {}", t.title),
            description: None,
            started_at: created_str,
            ended_at: None,
            duration_secs: None,
            entity_id: Some(t.id.clone()),
            entity_route: Some(task_route.clone()),
            color: "var(--timeline-todo)".into(),
            metadata: None,
        });
    }

    // Task completed in range
    if let Some(ref completed) = t.completed_at {
        let comp_str = completed.to_rfc3339();
        if comp_str >= start_bound && comp_str <= end_bound {
            out.push(TimelineEntry {
                id: format!("{}-completed", t.id),
                source: TimelineSource::Todo,
                entry_type: TimelineEntryType::TaskCompleted,
                title: format!("Completed: {}", t.title),
                description: None,
                started_at: comp_str,
                ended_at: None,
                duration_secs: None,
                entity_id: Some(t.id.clone()),
                entity_route: Some(task_route),
                color: "var(--timeline-todo)".into(),
                metadata: None,
            });
        }
    }

    out
}

fn normalize_transaction(tx: storage::FinanceTransactionRow) -> TimelineEntry {
    let is_expense = tx.tx_type == "expense";
    let entry_type = if is_expense {
        TimelineEntryType::ExpenseRecorded
    } else {
        TimelineEntryType::IncomeRecorded
    };
    // Use created_at for precise timeline positioning (tx_date is day-level)
    let amount_display = format!("{}{:.2}",
        if is_expense { "-$" } else { "+$" },
        tx.amount.unsigned_abs() as f64 / 100.0
    );
    let title = match &tx.category {
        Some(cat) => format!("{} {}", amount_display, cat),
        None => amount_display.clone(),
    };

    TimelineEntry {
        id: tx.id.clone(),
        source: TimelineSource::Finance,
        entry_type,
        title,
        description: tx.notes.clone(),
        started_at: tx.created_at.to_rfc3339(),
        ended_at: None,
        duration_secs: None,
        entity_id: Some(tx.id),
        entity_route: Some("/finance/transactions".into()),
        color: if is_expense {
            "var(--timeline-finance-expense)".into()
        } else {
            "var(--timeline-finance-income)".into()
        },
        metadata: Some(serde_json::json!({
            "amount": tx.amount,
            "txType": tx.tx_type,
            "category": tx.category,
            "counterparty": tx.counterparty,
        })),
    }
}

fn normalize_note_activity(note: feature_notes::models::NoteRow, start: &str) -> TimelineEntry {
    let start_bound = format!("{start}T00:00:00Z");
    // If created_at is in range, it's a "created" event; otherwise "updated"
    let is_created = note.created_at >= start_bound && note.created_at == note.updated_at;
    let (entry_type, prefix) = if is_created {
        (TimelineEntryType::NoteCreated, "Created")
    } else {
        (TimelineEntryType::NoteUpdated, "Edited")
    };

    TimelineEntry {
        id: format!("{}-{}", note.id, if is_created { "created" } else { "updated" }),
        source: TimelineSource::Note,
        entry_type,
        title: format!("{}: {}", prefix, note.title),
        description: None,
        started_at: if is_created {
            note.created_at.clone()
        } else {
            note.updated_at.clone()
        },
        ended_at: None,
        duration_secs: None,
        entity_id: Some(note.id),
        entity_route: Some("/notes".into()),
        color: "var(--timeline-note)".into(),
        metadata: None,
    }
}
```

**Step 7: Update `compute_summary` to count `TaskDue`**

In the match arm at ~L274, add `TaskDue`:

```rust
TimelineEntryType::TaskDue => {} // counted via source_breakdown, no special counter
```

(Or add a `tasks_due` counter to `TimelineSummary` if desired — skip for now to keep changes minimal.)

**Step 8: Build and verify**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo build -p app-core 2>&1 | tail -10`
Expected: build succeeds

**Step 9: Commit**

```bash
git add crates/app-core/src/handlers/timeline.rs
git commit -m "feat(timeline): add Todo, Finance, Notes direct-query pipelines"
```

---

### Task 5: Add CSS theme tokens for new columns

**Files:**
- Modify: `desktop-ui/src/styles/theme.css:47-61`

**Step 1: Add new timeline tokens**

After existing timeline variables, add:

```css
--timeline-todo: oklch(0.70 0.16 200);
--timeline-finance-expense: oklch(0.65 0.18 25);
--timeline-finance-income: oklch(0.72 0.16 155);
```

**Step 2: Register in `@theme inline`**

Find the `@theme inline` block and add the new variables so Tailwind can generate utilities for them.

**Step 3: Commit**

```bash
git add desktop-ui/src/styles/theme.css
git commit -m "feat(theme): add timeline-todo and finance expense/income color tokens"
```

---

### Task 6: Update layers configuration

**Files:**
- Modify: `desktop-ui/src/components/dashboard/layers.ts`

**Step 1: Update `LayerKey` type**

```typescript
export type LayerKey = "focus" | "timeEntries" | "tasks" | "transactions" | "notes";
```

**Step 2: Update `LAYERS` array**

```typescript
export const LAYERS: LayerConfig[] = [
  {
    key: "focus",
    label: "Focus Sessions",
    sources: ["focus"],
    defaultOn: true,
    color: "var(--timeline-focus)",
  },
  {
    key: "timeEntries",
    label: "Time Entries",
    sources: ["task"],
    defaultOn: true,
    color: "var(--timeline-task)",
  },
  {
    key: "tasks",
    label: "Tasks",
    sources: ["todo"],
    defaultOn: true,
    color: "var(--timeline-todo)",
  },
  {
    key: "transactions",
    label: "Transactions",
    sources: ["finance"],
    defaultOn: true,
    color: "var(--timeline-finance)",
  },
  {
    key: "notes",
    label: "Notes",
    sources: ["note"],
    defaultOn: true,
    color: "var(--timeline-note)",
  },
];
```

Note: removed `apps`, `events`, and `calendar` (comingSoon) layers entirely.

**Step 3: Commit**

```bash
git add desktop-ui/src/components/dashboard/layers.ts
git commit -m "feat(dashboard): update layers to focus/timeEntries/tasks/transactions/notes"
```

---

### Task 7: Update `DayColumnsView.tsx` columns and renderers

**Files:**
- Modify: `desktop-ui/src/components/dashboard/DayColumnsView.tsx`

**Step 1: Update column definitions**

Replace the `COLUMNS` array (lines 26-62) with:

```typescript
const COLUMNS: ColumnDef[] = [
  {
    key: "focus",
    label: "Focus",
    icon: "◉",
    color: "var(--timeline-focus)",
    flex: 0.7,
    filter: (e) => e.entryType === "focusSession",
  },
  {
    key: "timeEntries",
    label: "Time Entries",
    icon: "☰",
    color: "var(--timeline-task)",
    flex: 1.8,
    filter: (e) => e.entryType === "taskTimeEntry",
  },
  {
    key: "tasks",
    label: "Tasks",
    icon: "☑",
    color: "var(--timeline-todo)",
    flex: 1.8,
    filter: (e) =>
      e.entryType === "taskDue" ||
      e.entryType === "taskCreated" ||
      e.entryType === "taskCompleted",
  },
  {
    key: "transactions",
    label: "Transactions",
    icon: "$",
    color: "var(--timeline-finance)",
    flex: 1.2,
    filter: (e) =>
      e.entryType === "expenseRecorded" ||
      e.entryType === "incomeRecorded" ||
      e.entryType === "transactionRecorded",
  },
  {
    key: "notes",
    label: "Notes",
    icon: "✎",
    color: "var(--timeline-note)",
    flex: 1.2,
    filter: (e) =>
      e.entryType === "noteCreated" ||
      e.entryType === "noteUpdated",
  },
];
```

**Step 2: Update the `ColumnEntry` component**

In the `ColumnEntry` component (lines 204-313), replace the column-specific render branches with new ones for tasks, transactions, and notes.

Keep existing `focus` and `tasks` (now `timeEntries`) renderers. Add new renderers:

For `tasks` column (due/created/completed):
```typescript
if (column.key === "tasks") {
  const isDue = entry.entryType === "taskDue";
  const isCompleted = entry.entryType === "taskCompleted";
  const status = entry.metadata?.status as string | undefined;
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute left-1 right-1 rounded-md px-1.5 py-0.5 text-[11px] leading-tight overflow-hidden cursor-pointer transition-colors",
        isDue
          ? "border-l-2 border-l-[var(--timeline-todo)] bg-[var(--timeline-todo)]/15 hover:bg-[var(--timeline-todo)]/25"
          : "border-l border-border bg-white/[0.04] hover:bg-white/[0.08]",
        isCompleted && "opacity-60 line-through",
        selected && "ring-1 ring-brand",
      )}
      style={{ top, height: Math.max(height, 20) }}
      title={entry.title}
    >
      <span className="text-secondary truncate block">{entry.title}</span>
      {isDue && status && height > 28 && (
        <span className="text-muted text-[10px] truncate block capitalize">{status}</span>
      )}
    </button>
  );
}
```

For `transactions` column:
```typescript
if (column.key === "transactions") {
  const isExpense = entry.entryType === "expenseRecorded";
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute left-0.5 right-0.5 rounded-md px-1.5 py-0.5 text-[10px] leading-tight overflow-hidden cursor-pointer transition-colors",
        isExpense
          ? "border-l-2 border-l-[var(--timeline-finance-expense)] bg-[var(--timeline-finance-expense)]/15 hover:bg-[var(--timeline-finance-expense)]/25"
          : "border-l-2 border-l-[var(--timeline-finance-income)] bg-[var(--timeline-finance-income)]/15 hover:bg-[var(--timeline-finance-income)]/25",
        selected && "ring-1 ring-brand",
      )}
      style={{ top, height: Math.max(height, 18) }}
      title={entry.title}
    >
      <span className={cn("truncate block font-medium", isExpense ? "text-[var(--timeline-finance-expense)]" : "text-[var(--timeline-finance-income)]")}>
        {entry.title}
      </span>
    </button>
  );
}
```

For `notes` column:
```typescript
if (column.key === "notes") {
  const isCreated = entry.entryType === "noteCreated";
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "absolute left-1 right-1 flex items-center gap-1 text-[10px] cursor-pointer transition-colors",
        "text-muted hover:text-secondary",
        selected && "text-brand",
      )}
      style={{ top }}
      title={entry.title}
    >
      <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: isCreated ? "var(--timeline-note)" : "var(--timeline-note)" }} />
      <span className="truncate">{entry.title}</span>
    </button>
  );
}
```

**Step 3: Update the old `tasks` key references to `timeEntries`**

In the existing ColumnEntry, change `column.key === "tasks"` to `column.key === "timeEntries"` for the time entry renderer.

**Step 4: Build and verify**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot/desktop-ui && bun run build 2>&1 | tail -5`
Expected: build succeeds

**Step 5: Lint**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot/desktop-ui && bun run lint:fix`

**Step 6: Commit**

```bash
git add desktop-ui/src/components/dashboard/DayColumnsView.tsx
git commit -m "feat(dashboard): replace Apps/Events columns with Tasks/Transactions/Notes"
```

---

### Task 8: Update `SummaryPanel.tsx`

**Files:**
- Modify: `desktop-ui/src/components/dashboard/SummaryPanel.tsx`

**Step 1: Remove "Top Apps" section**

Delete the top apps section (~lines 81-93) since we no longer have an Apps column.

**Step 2: Add "Tasks Due" stat to the quick stats grid**

Add a stat for tasks created:

```tsx
<Stat
  icon={<ListTodo className="w-3.5 h-3.5" />}
  label="Created"
  value={String(summary.tasksCreated)}
/>
```

**Step 3: Update `SOURCE_ORDER`**

```typescript
const SOURCE_ORDER: Record<string, number> = {
  focus: 0,
  task: 1,
  todo: 2,
  note: 3,
  finance: 4,
};
```

**Step 4: Build, lint, commit**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot/desktop-ui && bun run build && bun run lint:fix`

```bash
git add desktop-ui/src/components/dashboard/SummaryPanel.tsx
git commit -m "feat(dashboard): update summary panel for new activity columns"
```

---

### Task 9: Visual verification and final cleanup

**Step 1: Run the full app**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo build --workspace 2>&1 | tail -5`
Expected: workspace builds with no errors

**Step 2: Run lints**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot && cargo clippy -p app-core -p desktop-shared -p storage -p feature-notes --all-targets 2>&1 | tail -10`
Expected: 0 warnings

**Step 3: Run frontend lint**

Run: `cd /Users/jayden/Projects/Klynt/nanobot/klyntbot/desktop-ui && bun run lint:fix`

**Step 4: Final commit if any cleanup needed**
