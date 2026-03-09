# Automations Page Design

**Date:** 2026-03-09
**Status:** Approved

## Overview

Add a dedicated "Automations" page to the desktop UI for managing cron jobs. The backend cron system is complete (`CronService` with full CRUD) but has zero frontend surface — no Tauri commands, no IPC types, no UI.

## Backend Changes

### CronOrigin Enum

New enum added to `CronJob`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CronOrigin {
    System,
    User,
    Ai,
    Plugin,
}
```

Set at creation points:
- `ensure_cron_jobs()` → `System`
- `CronTool` (LLM agent) → `Ai`
- Plugin registration → `Plugin`
- UI creation → `User`

### Migration

```sql
ALTER TABLE cron_jobs ADD COLUMN origin TEXT NOT NULL DEFAULT 'system';
```

### Tauri Commands (`commands/cron.rs`)

| Command | Params | Returns |
|---------|--------|---------|
| `list_cron_jobs` | `{ includeDisabled: bool }` | `Vec<CronJob>` |
| `enable_cron_job` | `{ id, enabled }` | `CronJob` |
| `run_cron_job` | `{ id }` | `bool` |
| `delete_cron_job` | `{ id }` | `bool` |
| `create_cron_job` | `{ name, schedule, message, ... }` | `CronJob` |
| `update_cron_job` | `{ id, name?, schedule?, ... }` | `CronJob` |
| `cron_status` | none | `{ enabled, jobs, nextWakeAtMs }` |

System jobs protected from delete/edit at `AppCore` handler level.

## Frontend Architecture

### Route & Navigation

- Sidebar icon: `Timer` or `Zap` from lucide-react
- Route: `/#/automations` → lazy-loaded `AutomationsPage`
- Single page view, no sub-routes

### Page Layout

```
┌─────────────────────────────────────────────────────┐
│  Header: "Automations"                [+ New Job]   │
│  [All] [System] [AI] [User] [Plugin]     🔍 search  │
│                                                     │
│  Name          Origin  Schedule    Next   Status    │
│  ───────────────────────────────────────────────    │
│  Daily Plan    System  9:00 AM     28m    ●OK       │
│  ▼ Budget Chk  System  Every 6h    2h     ●OK       │
│  ┌─────────────────────────────────────────────┐   │
│  │ Created: Mar 5   Last run: 2h ago   ●OK     │   │
│  │ Message: "Check budget thresholds"          │   │
│  │ Channel: telegram  Deliver: Yes             │   │
│  │ [Run Now]           (no edit/delete - system)│   │
│  └─────────────────────────────────────────────┘   │
│  Remind me..   AI      Daily 5PM   4h     ●OK       │
└─────────────────────────────────────────────────────┘
```

### Interactions

- **Click row** → expands inline to show full details
- **Toggle switch** inline for enable/disable
- **Context menu** (right-click) → Run Now, Edit, Delete (system jobs protected)
- **"+ New Job"** → inline create form expands at top of table
- **No SlidePanel** — avoids conflict with SidebarChat

### Components

```
AutomationsPage
├── AutomationsHeader          — title + "New Job" button
├── AutomationsFilters         — origin tabs + search input
├── AutomationCreateForm       — inline form at top (when creating)
├── AutomationsTable           — sortable table of jobs
│   ├── AutomationRow          — row with toggle, badges, relative times
│   └── AutomationExpandedRow  — inline details + edit form
├── JobScheduleBuilder         — guided schedule inputs
│   └── CronExpressionInput    — advanced mode raw cron
├── JobPayloadForm             — message, channel, deliver
└── AutomationsSkeleton        — loading state
```

### Data Flow

- `useQuery("list_cron_jobs", { includeDisabled: true })` → full list
- Client-side filtering/search (small dataset, <100 jobs)
- Mutations via `useMutation` → `refetch` on success
- `useEvent("entity:updated")` for live updates

### State (all useState, no global store)

- `originFilter: "all" | "system" | "ai" | "user" | "plugin"`
- `searchQuery: string` (debounced)
- `sortBy: "nextRun" | "name" | "lastRun"`
- `expandedJobId: string | null`
- `isCreating: boolean`

## Visual Design

### Origin Badges

- **System** — `bg-blue-500/20 text-blue-400`
- **AI** — `bg-purple-500/20 text-purple-400`
- **User** — `bg-emerald-500/20 text-emerald-400`
- **Plugin** — `bg-amber-500/20 text-amber-400`

### Status Indicators

- Enabled + OK → green dot
- Enabled + Error → red dot, hover tooltip with `lastError`
- Disabled → muted row `opacity-50`
- Never run → dash `—`

### Schedule Display (humanized)

| Schedule | Display |
|---|---|
| `{ kind: "every", everyMs: 1800000 }` | `Every 30 min` |
| `{ kind: "cron", expr: "0 9 * * *" }` | `Daily at 9:00 AM` |
| `{ kind: "cron", expr: "0 18 * * 0" }` | `Sundays at 6:00 PM` |
| `{ kind: "at", atMs: ... }` | `Mar 10, 3:00 PM` |

### Job Name Humanization

`__klyntbot_daily_planning` → `Daily Planning`, `todo_focus_check` → `Focus Check`.

### Create/Edit Form (inline)

```
┌──────────────────────────────────────────────────┐
│  Name: [____________]                            │
│                                                  │
│  Schedule Type: (●) Recurring  ( ) One-time      │
│                 ( ) Cron expression               │
│  Every [30] [minutes ▾]                          │
│                                                  │
│  Message: [__________________________________]   │
│  Channel: [optional ▾]   □ Deliver to user       │
│                                                  │
│                        [Cancel]  [Save]          │
└──────────────────────────────────────────────────┘
```

Schedule type radio switches between:
- **Recurring** → interval amount + unit dropdown
- **One-time** → date+time picker
- **Cron expression** → raw text input with syntax hint
