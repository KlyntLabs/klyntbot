# Unified Activity Dashboard — Design Document

## Overview

Replace the current root route (`/`) with a time-block calendar dashboard that visualizes all system activity (tasks, notes, finance, app tracking, focus sessions) in one unified view. Tasks move to `/tasks`. The existing `/productivity` view remains for detailed app-tracking analytics.

## Requirements

- **Route**: `/` → dashboard, `/tasks` → tasks (breaking change, pre-production)
- **Visualization**: Time-block calendar with Day/Week/Month/Year views
- **Duration events**: Colored blocks (app tracking, focus sessions, task time entries)
- **Point-in-time events**: Thin fixed-height cards (note edits, finance transactions)
- **Summary panel**: Context-sensitive — aggregate stats by default, entity details on block click
- **Architecture**: Query-time aggregation across existing tables (no new storage tables)

## Data Model

### TimelineEntry (desktop-shared)

```rust
pub struct TimelineEntry {
    pub id: String,
    pub source: TimelineSource,
    pub entry_type: TimelineEntryType,
    pub title: String,
    pub description: Option<String>,
    pub started_at: String,             // ISO 8601
    pub ended_at: Option<String>,       // None = point-in-time
    pub duration_secs: Option<i64>,
    pub entity_id: Option<String>,
    pub entity_route: Option<String>,   // Frontend route e.g. "/tasks/abc123"
    pub color: String,                  // CSS token name
    pub metadata: Option<serde_json::Value>,
}

pub enum TimelineSource {
    Productivity, Focus, Task, Note, Finance, System,
}

pub enum TimelineEntryType {
    // Duration-based (blocks)
    AppUsage, FocusSession, TaskTimeEntry,
    // Point-in-time (thin cards)
    TaskCreated, TaskCompleted, TaskUpdated,
    NoteCreated, NoteUpdated,
    TransactionRecorded, ExpenseRecorded, IncomeRecorded,
    SystemEvent,
}
```

### TimelineQuery / TimelineResponse

```rust
pub struct TimelineQuery {
    pub start_date: String,
    pub end_date: String,
    pub sources: Option<Vec<TimelineSource>>,
    pub include_point_events: bool,
}

pub struct TimelineSummary {
    pub total_tracked_secs: i64,
    pub focus_secs: i64,
    pub tasks_completed: i64,
    pub tasks_created: i64,
    pub notes_touched: i64,
    pub transactions_count: i64,
    pub top_apps: Vec<TopAppEntry>,
    pub source_breakdown: Vec<SourceBreakdown>,
}

pub struct TimelineResponse {
    pub entries: Vec<TimelineEntry>,
    pub summary: TimelineSummary,
}
```

## Backend Architecture

### Query Strategy

Four parallel queries via `tokio::try_join!`, merged and sorted in Rust:

1. `activity_events` — app tracking blocks (has duration)
2. `focus_sessions` — focus session blocks (has duration)
3. `action_time_entries` — task work blocks (has duration)
4. `domain_event_log` — point-in-time events (task/note/finance)

Each source normalized into `Vec<TimelineEntry>` via dedicated `normalize_*` functions.

### Handler

New `app-core/src/handlers/timeline.rs` with `AppCore::timeline_query()`.

### Tauri Command + Dev Server

New `desktop/src/commands/timeline.rs` with `timeline_query` command. Dev server dispatch added.

### New Repo Methods

| Repo | Method | Purpose |
|------|--------|---------|
| `ActionRepo` | `time_entries_in_range(start, end)` | Query task time entries by date |
| `EventLogRepo` | `query_range(start, end)` | Query domain events by date |

### Critical Fix: Domain Event Emissions

`AppCore` mutation handlers must emit `DomainEvent`s to the bus (currently only agent tool paths do):

- `task_create` → `DomainEvent::TaskCreated`
- `task_toggle_complete` → `DomainEvent::TaskCompleted`
- `note_create` → `DomainEvent::NoteCreated` (new variant)
- `note_update` → `DomainEvent::NoteUpdated` (new variant)
- `finance_transaction_create` → `DomainEvent::TransactionRecorded`

New domain event variants to add:

```rust
NoteCreated { note_id: String, title: String },
NoteUpdated { note_id: String, title: String },
```

## UI Architecture

### Route Structure

```
/                   → DashboardRedirect → /day/{today}
/day/:date          → DashboardLayout > DayCalendarView
/week/:date         → DashboardLayout > WeekCalendarView
/month/:date        → DashboardLayout > MonthCalendarView
/year/:year         → DashboardLayout > YearHeatmapView
/tasks              → TasksPage (moved from /)
/productivity/*     → stays as-is
```

### Component Tree

```
DashboardLayout
├── TopBar
│   ├── ViewSwitcher (Day | Week | Month | Year)
│   ├── DateDisplay
│   └── DateNavigator (← Today →)
├── Content (flex)
│   ├── CalendarGrid (flex-1, scrollable)
│   │   ├── DayCalendarView — 24h vertical axis, single column
│   │   ├── WeekCalendarView — 24h vertical axis, 7 day columns
│   │   ├── MonthCalendarView — Calendar grid with mini activity bars
│   │   └── YearHeatmapView — 12 month heatmap grids
│   └── SummaryPanel (w-72, glass-card)
│       ├── DefaultSummary (aggregate stats)
│       └── EntryDetail (on block click — entity info + navigate button)
```

### View Behaviors

**Day View**: 24h vertical time axis, blocks positioned by `started_at`/`ended_at`, thin cards at fixed 15min height. Current time red line indicator. Overlapping blocks shown side-by-side.

**Week View**: 7 day columns with shared hourly axis. Blocks narrower. Point-in-time events shown as colored dots with hover tooltip.

**Month View**: Calendar grid. Each cell shows stacked mini color bars (proportional time per source) and count badges. Click cell → navigate to day view.

**Year View**: 12 mini-month heatmap grids. Cell intensity = tracked hours. Click month → navigate to month view. Summary shows yearly totals.

### Color Tokens

| Source | Token | Visual |
|--------|-------|--------|
| App (Productive) | `--timeline-app-productive` | Green-tinted |
| App (Distracting) | `--timeline-app-distracting` | Red-tinted |
| App (Neutral) | `--timeline-app-neutral` | Gray-tinted |
| Focus Session | `--timeline-focus` | Brand purple |
| Task | `--timeline-task` | Blue |
| Note | `--timeline-note` | Amber |
| Finance | `--timeline-finance` | Emerald |
| System | `--timeline-system` | Gray |

### Interactions

- Hover block → tooltip (title, time range, duration)
- Click block → summary panel shows EntryDetail
- Click outside / ESC → summary panel returns to DefaultSummary
- Double-click block → navigate to entity route
- `entity:updated` event → refetch timeline data

### Sidebar Changes

| Position | Icon | Route | Change |
|----------|------|-------|--------|
| 1 | `LayoutDashboard` | `/` | New — dashboard |
| 2 | `CheckSquare` | `/tasks` | Moved from `/` |
| 3+ | ... | ... | Unchanged |

## Backend Changes Summary

| Change | Location | Scope |
|--------|----------|-------|
| Timeline types | `desktop-shared` | New types |
| `timeline_query` handler | `app-core/handlers/timeline.rs` | New file |
| `timeline_query` command | `desktop/commands/timeline.rs` | New file |
| `time_entries_in_range()` | `storage/repos/action_repo.rs` | New method |
| `query_range()` | `cognitive/repos/event_log.rs` | New method |
| Note domain events | `bus/domain_events.rs` | Extend enum |
| Emit events from AppCore handlers | `app-core/handlers/*.rs` | Add publish calls |
| Register command + route | `desktop/main.rs`, `dev_server.rs` | Wire up |

## Frontend Changes Summary

| Change | Location | Scope |
|--------|----------|-------|
| Dashboard routes | `App.tsx` | Route restructure |
| `DashboardLayout` | `components/dashboard/DashboardLayout.tsx` | New |
| `DayCalendarView` | `components/dashboard/DayCalendarView.tsx` | New |
| `WeekCalendarView` | `components/dashboard/WeekCalendarView.tsx` | New |
| `MonthCalendarView` | `components/dashboard/MonthCalendarView.tsx` | New |
| `YearHeatmapView` | `components/dashboard/YearHeatmapView.tsx` | New |
| `SummaryPanel` + sub-components | `components/dashboard/SummaryPanel.tsx` | New |
| `TimeBlock` + `ThinCard` | `components/dashboard/TimeBlock.tsx` | New |
| Timeline color tokens | `styles/theme.css` | Extend |
| Sidebar icon/route update | `components/layout/Sidebar.tsx` | Modify |
| Move tasks route | `App.tsx` | Route change |
