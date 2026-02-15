# Sprint 3: Bidirectional Calendar Sync — Architecture Blueprint

**Author:** System Architect
**Date:** 2026-02-15
**Status:** APPROVED (2026-02-15, product-architect decisions incorporated)

---

## 1. Data Model Changes

### 1.1 Add `status` field to `CalendarEvent`

**File:** `crates/calendar/src/types.rs`

Add to `CalendarEvent`:
```rust
pub struct CalendarEvent {
    // ... existing fields ...

    /// Raw iCalendar STATUS string (e.g., "CONFIRMED", "TENTATIVE", "CANCELLED").
    /// Stored as Option<String> to handle non-standard values from CalDAV servers.
    /// None means no STATUS property was present (treat as CONFIRMED).
    #[serde(default)]
    pub status: Option<String>,
}
```

**Rationale (APPROVED):** Using `Option<String>` instead of an enum avoids needing to handle unknown variant values from CalDAV servers that might return non-standard STATUS values. The reconciliation logic matches on string values directly (`"CANCELLED"`, `"CONFIRMED"`, etc.). `None` means the iCal property was absent, which per RFC 5545 defaults to CONFIRMED.

### 1.2 Add `last_modified` field to `CalendarEvent`

```rust
pub struct CalendarEvent {
    // ... existing fields ...

    /// Last modification timestamp (for conflict detection)
    #[serde(default)]
    pub last_modified: Option<DateTime<Utc>>,
}
```

**Rationale:** `last_modified` (from iCal `LAST-MODIFIED` property) enables the reconciliation engine to determine which side has newer changes without relying solely on etag comparison.

---

## 2. Parser Changes

**File:** `crates/calendar/src/caldav/parser.rs`

Extend `parse_vevent()` to handle `STATUS` and `LAST-MODIFIED` properties:

```rust
// Inside the match on field_name:
"STATUS" => {
    status = Some(value.to_uppercase());
}
"LAST-MODIFIED" => {
    last_modified = Some(parse_datetime_with_params(value, params)?);
}
```

Add these variables at the top of the function:
```rust
let mut status: Option<String> = None;
let mut last_modified: Option<DateTime<Utc>> = None;
```

Include in the returned `CalendarEvent`:
```rust
Ok(CalendarEvent {
    uid,
    summary,
    description,
    start,
    end,
    source: EventSource::CalDAV,
    etag: None,
    status,           // None if absent, raw string otherwise
    last_modified,
})
```

Extend `generate_vevent()` to emit `STATUS` when present:
```rust
// After DTEND line:
if let Some(ref status) = event.status {
    lines.push(format!("STATUS:{}", status));
}
```

---

## 3. CalendarHandler Trait Extension

**File:** `crates/tools/src/calendar_tool.rs`

Add a `get_event` method to the existing `CalendarHandler` trait:

```rust
#[async_trait]
pub trait CalendarHandler: Send + Sync {
    // ... existing methods ...

    /// Fetch a single event by UID across all providers.
    /// Returns None if the event doesn't exist on any provider.
    async fn get_event(&self, uid: &str) -> Result<Option<Value>>;
}
```

**Why `Value` instead of `CalendarEvent`?** The trait lives in Layer 3 (tools) which depends on `calendar` (Layer 2), so we *could* use `CalendarEvent` directly. However, the existing trait methods all return `Value` for consistency with the JSON-based tool interface. **Decision: Use `CalendarEvent` directly** — it's cleaner for the reconciliation engine which needs typed access, and the trait already imports from the `calendar` crate transitively.

**Revised signature:**
```rust
async fn get_event(&self, uid: &str) -> Result<Option<calendar::CalendarEvent>>;
```

**Implementation in `CalendarSyncAdapter`:** Iterate all providers, call `get_events(None)` and filter by UID. This is acceptable for single lookups; batch operations should use the existing `get_events()` directly.

**Alternative considered:** Adding `get_event(uid)` to `CalendarProvider` trait — rejected because CalDAV REPORT queries by UID require provider-specific XML templates, adding complexity for minimal gain. The reconciliation engine operates on batch-fetched data anyway.

---

## 4. Reconciliation Engine Design

**File:** `crates/agent/src/calendar_reconcile.rs` (new)

### 4.1 Core Types

```rust
use calendar::CalendarEvent;
use chrono::{DateTime, Utc};
use tools::todo_types::{Todo, TodoStatus};

/// Result of reconciling a single calendar event against its linked todo
#[derive(Debug, Clone)]
pub enum ReconcileAction {
    /// Update the todo's due_date from the event's new time
    UpdateDueDate {
        todo_id: String,
        old_due: DateTime<Utc>,
        new_due: DateTime<Utc>,
    },
    /// Mark the todo as Done (APPROVED: go directly to Done, no intermediate Doing)
    /// Uses TodoPatch { status: Some(TodoStatus::Done) } — store handles completed_at
    CompleteTodo {
        todo_id: String,
    },
    /// Clear the calendar link (event was cancelled)
    ClearCalendarLink {
        todo_id: String,
        event_uid: String,
    },
    /// No action needed (no changes detected)
    NoChange {
        todo_id: String,
    },
}

/// Summary of a reconciliation run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub due_dates_updated: u32,
    pub todos_completed: u32,
    pub links_cleared: u32,
    pub errors: Vec<String>,
    pub timestamp: DateTime<Utc>,
}
```

### 4.2 Core Function Signatures

```rust
/// Determine what action to take for a single event-todo pair.
/// Pure function — no side effects, fully testable.
pub fn determine_action(event: &CalendarEvent, todo: &Todo) -> ReconcileAction {
    // 1. If event.status == Some("CANCELLED") → ClearCalendarLink
    // 2. If event.status == Some("COMPLETED") → CompleteTodo (→ Done directly)
    // 3. If event.start != todo.due_date → UpdateDueDate
    // 4. Otherwise → NoChange
}

/// Execute reconciliation across all linked todos.
/// Fetches events from providers, matches with todos, applies actions.
pub async fn reconcile(
    todo_store: &Arc<RwLock<TodoStore>>,
    providers: &[(String, &dyn CalendarProvider)],
    dispatcher: &NotificationDispatcher,
) -> Result<ReconcileReport> {
    // 1. Batch-fetch all events from all providers
    // 2. Build a HashMap<uid, CalendarEvent> for O(1) lookup
    // 3. Iterate todos with calendar_event_uid
    // 4. For each, call determine_action() and apply
    // 5. Send notifications for significant changes
    // 6. Return report
}
```

### 4.3 Design Rationale

- **Pure `determine_action`**: Separating the decision logic from execution makes it trivially testable without mocks. The reconciliation engine just applies the actions.
- **Batch fetch over individual gets**: One `get_events(None)` call per provider instead of N `get_event(uid)` calls. This is critical for performance — CalDAV REPORT queries are expensive HTTP round-trips.
- **HashMap lookup**: After batch fetching, build `HashMap<String, CalendarEvent>` from UIDs for O(1) matching against todos.

---

## 5. Background Service Pattern

**Decision: Cron callback, NOT a standalone background service.**

**Rationale:**
1. The `ReminderEngine` pattern (standalone `tokio::spawn` loop) is appropriate for high-frequency checks (every 60s) that need fine-grained control over timing.
2. Calendar reconciliation is a **low-frequency, heavyweight operation** (every 5-15 minutes) that aligns with the existing cron infrastructure already registered in `serve.rs`.
3. The calendar sync cron job (`__klyntbot_calendar_sync`) already exists — reconciliation should run **as part of the sync cycle**, not as a separate timer.
4. Adding another standalone service increases shutdown coordination complexity unnecessarily.

**Implementation:** Extend the existing `__klyntbot_calendar_sync` cron callback in `serve.rs` to call reconciliation after the pull sync completes. Alternatively, call reconciliation directly from `CalendarSyncAdapter::sync_calendar_internal()` after processing remote changes.

**Preferred approach:** Call from `sync_calendar_internal()` — this ensures reconciliation always runs after sync regardless of how sync is triggered (cron, manual CLI, agent tool call).

```rust
// In CalendarSyncAdapter::sync_calendar_internal():
async fn sync_calendar_internal(&self) -> Result<Value> {
    // ... existing sync logic ...

    // After processing all providers, reconcile:
    if self.bidirectional_sync {
        let report = calendar_reconcile::reconcile(
            &self.todo_store,
            &provider_refs,
            &self.dispatcher,
        ).await?;
        // Include report in sync result JSON
    }

    Ok(json!({ /* ... */ }))
}
```

**Consequence:** `CalendarSyncAdapter` needs access to `NotificationDispatcher` (currently it doesn't have one). This requires adding it as a constructor parameter.

---

## 6. Notification Integration

### 6.1 Wiring

`CalendarSyncAdapter` must gain a reference to `NotificationDispatcher`:

```rust
pub struct CalendarSyncAdapter {
    // ... existing fields ...
    dispatcher: Option<Arc<NotificationDispatcher>>,
}
```

`Option<Arc<...>>` to maintain backward compatibility — tests that don't care about notifications pass `None`. **(APPROVED: Use full multi-target NotificationDispatcher, respecting user's configured `todo.notifications.targets`.)**

Update `CalendarSyncAdapter::new()` to accept an optional dispatcher:

```rust
pub async fn new(
    todo_store: Arc<RwLock<TodoStore>>,
    config: &CalendarConfig,
    timezone: String,
    dispatcher: Option<Arc<NotificationDispatcher>>,
) -> Result<Self> { /* ... */ }
```

### 6.2 Notification Messages

| Action | Title | Body |
|--------|-------|------|
| `UpdateDueDate` | "📅 Calendar Update" | "{todo.title} rescheduled: {old_due} → {new_due}" |
| `CompleteTodo` | "✅ Task Completed via Calendar" | "{todo.title} marked done" |
| `ClearCalendarLink` | "❌ Calendar Event Cancelled" | "{todo.title} unlinked from calendar" |

### 6.3 Notification Batching

For reconciliation runs that affect multiple todos, batch notifications into a single summary rather than N individual notifications:

```
"📅 Calendar Reconciliation: 2 rescheduled, 1 completed, 1 cancelled"
```

---

## 7. Config Changes

**File:** `crates/config/src/schema/core.rs`

Add `bidirectional_sync` to `CalendarConfig`:

```rust
pub struct CalendarConfig {
    // ... existing fields ...

    /// Enable bidirectional sync: calendar changes update linked todos.
    /// When true, time changes update due_date, completions mark todo done,
    /// cancellations clear the calendar link.
    #[serde(default = "default_true")]
    pub bidirectional_sync: bool,
}
```

**Default: `true` (APPROVED)** — Users who configured calendar sync already expect bidirectional behavior. Opt-in creates a confusing "I set up calendar but changes don't sync back" experience.

Update `CalendarConfig::default()`:
```rust
impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            // ... existing ...
            bidirectional_sync: true,
        }
    }
}
```

Config JSON path: `calendar.bidirectionalSync`
Environment override: `KLYNTBOT_CALENDAR__BIDIRECTIONAL_SYNC=true`

---

## 8. CLI Changes

**File:** `crates/cli/src/calendar.rs` (extend existing)

Add `reconcile` subcommand:

```rust
/// Calendar subcommands
#[derive(Subcommand)]
pub enum CalendarCommands {
    // ... existing: Sync, Status, Events ...

    /// Reconcile calendar events with linked todos
    Reconcile {
        /// Dry-run mode: show what would change without applying
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Show detailed report
        #[arg(long, short, default_value_t = false)]
        verbose: bool,
    },
}
```

**Handler:**
```rust
CalendarCommands::Reconcile { dry_run, verbose } => {
    // 1. Load config, create CalendarSyncAdapter
    // 2. Call reconcile() (or determine_action in dry-run mode)
    // 3. Print report table
}
```

**Output format:**
```
Calendar Reconciliation Report
──────────────────────────────
  Due dates updated: 2
  Todos completed:   1
  Links cleared:     1
  Errors:            0

Details:
  ✓ "Team standup" rescheduled: Feb 15 9:00 → Feb 16 10:00
  ✓ "Sprint review" marked done
  ✓ "Cancelled 1:1" calendar link cleared
```

Also add `reconcile` to `CalendarTool` actions:

```rust
// In CalendarTool::parameters():
"action": {
    "enum": ["sync", "list_events", "create_event", "status", "reconcile"],
}
```

With corresponding handler that calls the reconciliation engine.

---

## 9. Error Handling

### 9.1 Partial Failure Strategy

Reconciliation uses **continue-on-error** semantics:

```rust
pub async fn reconcile(...) -> Result<ReconcileReport> {
    let mut report = ReconcileReport::default();

    for todo in linked_todos {
        match apply_action(&action, store).await {
            Ok(_) => { /* increment report counters */ }
            Err(e) => {
                report.errors.push(format!("{}: {}", todo.id, e));
                tracing::warn!("Reconcile failed for todo {}: {}", todo.id, e);
                // Continue processing remaining todos
            }
        }
    }

    Ok(report) // Always returns Ok with error details in report
}
```

**Rationale:** A single failed todo update (e.g., JSONL write error) shouldn't prevent other todos from being reconciled. Errors are collected and surfaced in the report.

### 9.2 Provider Fetch Failures

If a provider fails during batch fetch, skip it and log the error — the existing `sync_single_provider` pattern already handles this with `provider_results` tracking per-provider status.

### 9.3 Conflict with Manual Todo Changes

If a todo's due_date was manually updated *after* the last sync but *before* reconciliation runs, the calendar event's time will overwrite the manual change. This is acceptable because:
1. `bidirectional_sync` defaults to `true` but can be disabled if unwanted
2. The conflict resolution setting (`server_wins`) explicitly defines this behavior
3. Conflicts are logged to `calendar_conflicts.jsonl` for auditability

---

## 10. Performance

### 10.1 Batch Fetch Strategy

**Always batch-fetch, never individual get_event calls.**

```rust
// Good: One HTTP request per provider
let (events, _) = provider.get_events(sync_token).await?;
let event_map: HashMap<String, CalendarEvent> = events
    .into_iter()
    .map(|e| (e.uid.clone(), e))
    .collect();

// Bad: N HTTP requests
for todo in linked_todos {
    let event = provider.get_event(&todo.calendar_event_uid).await?; // DON'T
}
```

### 10.2 Todo Filtering

Only process todos that actually have a `calendar_event_uid`:

```rust
let linked_todos: Vec<Todo> = todos
    .into_iter()
    .filter(|t| t.calendar_event_uid.is_some())
    .collect();
```

This avoids iterating the entire todo list for reconciliation checks.

### 10.3 Incremental Sync Token

The existing sync token mechanism means `get_events(Some(token))` returns only *changed* events since the last sync. Reconciliation only needs to process changed events, not the entire calendar.

### 10.4 Expected Scale

Klyntbot is a personal productivity tool. Expected volumes:
- Providers: 1-3
- Events per sync: 0-50 (incremental), 100-500 (full)
- Linked todos: 10-100

At this scale, O(n) iteration is fine. No need for indices or caching beyond the HashMap.

---

## File Change Summary

| File | Change Type | Description |
|------|------------|-------------|
| `crates/calendar/src/types.rs` | Modify | Add `status: Option<String>` and `last_modified: Option<DateTime<Utc>>` fields to `CalendarEvent` |
| `crates/calendar/src/caldav/parser.rs` | Modify | Parse `STATUS` and `LAST-MODIFIED` iCal properties |
| `crates/tools/src/calendar_tool.rs` | Modify | Add `get_event()` and `reconcile` to `CalendarHandler` trait + tool |
| `crates/agent/src/calendar_reconcile.rs` | **New** | Reconciliation engine: `determine_action()` + `reconcile()` |
| `crates/agent/src/calendar_sync_adapter.rs` | Modify | Add `dispatcher` field, call reconciliation after sync |
| `crates/config/src/schema/core.rs` | Modify | Add `bidirectional_sync: bool` to `CalendarConfig` |
| `crates/cli/src/calendar.rs` | Modify | Add `reconcile` subcommand |
| `crates/agent/src/lib.rs` | Modify | Re-export `calendar_reconcile` module |

## Implementation Order

1. **Data model** (`types.rs` + `parser.rs`) — foundation, no dependents
2. **CalendarHandler trait** (`calendar_tool.rs`) — `get_event()` + `reconcile` action
3. **Reconciliation engine** (`calendar_reconcile.rs`) — core logic
4. **Adapter integration** (`calendar_sync_adapter.rs`) — wire reconciliation into sync
5. **Config** (`core.rs`) — `bidirectional_sync` flag
6. **CLI** (`calendar.rs`) — `reconcile` command
7. **Tests** — unit tests for `determine_action()`, integration test for full cycle

---

## Resolved Decisions (Product Architect, 2026-02-15)

1. **`bidirectional_sync` default: `true`** — Users who configured calendar sync already expect bidirectional behavior. Opt-in creates confusion.

2. **CompleteTodo → `Done` directly.** No intermediate `Doing` step. Calendar completion is an explicit signal. Use `TodoPatch { status: Some(TodoStatus::Done) }` and let TodoStore handle `completed_at`.

3. **Full `NotificationDispatcher` (multi-target).** Respects user's configured `todo.notifications.targets`. No reason to limit to OS-native only.

4. **`status` field: `Option<String>` not enum.** Store the raw iCal STATUS string to handle non-standard values from CalDAV servers. Reconciliation matches on string values ("CANCELLED", "COMPLETED", "CONFIRMED").
