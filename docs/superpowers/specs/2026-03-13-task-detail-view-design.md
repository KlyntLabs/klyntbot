# Task Detail View — Design Spec

## Overview

Two-column task detail view rendered inline within the tasks2 tab navigation system. Replaces the "Issue detail view coming soon" placeholder in `TabContent.tsx`. UI-first build with full mock data; backend wiring deferred.

## Scope

- Build all UI components with mock data
- All files in `desktop-ui/src/features/tasks2/components/detail/`
- Import and reuse the notes TipTap editor directly from `features/notes`
- Follow existing tasks2 patterns (scoped CSS, shadcn-style UI primitives, Zustand stores)

## Component Architecture

```
TabContent.tsx (existing, "issue" case)
└── IssueDetailView.tsx
    ├── IssueDetailBreadcrumb.tsx
    ├── IssueDetailTitle.tsx
    ├── IssueDetailTabs.tsx
    │   ├── IssueContentTab.tsx
    │   │   ├── EditorCore (from features/notes)
    │   │   ├── AcceptanceCriteria.tsx
    │   │   └── SubIssuesList.tsx
    │   └── IssueActivityTab.tsx
    └── IssueDetailSidebar.tsx
        ├── SidebarProperties.tsx
        ├── SidebarWorkState.tsx
        ├── SidebarTime.tsx
        └── SidebarAiInsights.tsx
```

## Layout

Two-column layout. Left column is flex-1 (main content). Right column is fixed 260px sidebar.

```
┌─────────────────────────────────────────────────────────────┐
│ Breadcrumb: Area / Project / TASK-ID                        │
├───────────────────────────────────┬─────────────────────────┤
│                                   │                         │
│  Title (inline editable h1)       │  PROPERTIES             │
│                                   │                         │
│  ┌─────────┬──────────────┐       ├─────────────────────────┤
│  │ Content │ Activity Log │       │  WORK STATE (live)      │
│  └─────────┴──────────────┘       ├─────────────────────────┤
│                                   │  TIME                   │
│  [Editor / Activity Feed]         ├─────────────────────────┤
│                                   │  AI INSIGHTS            │
│                                   │                         │
└───────────────────────────────────┴─────────────────────────┘
```

Below 900px container width: sidebar collapses behind a toggle button in top-right corner. Use CSS `@container` query on the detail view wrapper (since the content area width depends on the app sidebar, not viewport). `useState` for manual toggle override.

## Components

### IssueDetailView.tsx

Root container. Receives `issueId` from nav stack. Calls `useIssueDetail(issueId)` for all mock data. Manages sidebar collapse state. Renders the two-column flex layout.

### IssueDetailBreadcrumb.tsx

Renders `Area / Project / TASK-ID` as clickable segments. Each segment calls `navigateToStackIndex(index)` from the tab store (operates on the active tab, no `tabId` param). Uses `text-muted` for separators, `text-secondary` for segments, `text-primary` for current (last) segment.

### IssueDetailTitle.tsx

Large inline-editable heading (`text-2xl font-semibold`, matching global h1). Not part of TipTap. Renders as an auto-resizing `<textarea rows={1}>` with `resize: none` and transparent background. Saves on blur or Enter (Enter prevents newline). Receives `title` and `onUpdate` callback.

### IssueDetailTabs.tsx

Two tabs: **Content** and **Activity Log**. Tab bar uses existing tasks2 styling patterns. Active tab has bottom border accent. Manages active tab state internally.

### IssueContentTab.tsx

Three sections stacked vertically:

1. **TipTap Editor** — Imports `EditorCore` from `features/notes/components/editor/EditorCore.tsx` and `useNoteEditor` hook. Passes mock description content. Full extension set (wiki-links, entity mentions, slash commands, code blocks, math). **Side-effect note:** `EditorCore` imports Tauri APIs and calls `note_save_attachment` on image paste, and `useEntityResolution` fires IPC queries on mount. Pass no-op callbacks for `onNavigateNote`/`onNavigateEntity`. Accept that image paste routes to notes storage for now (out of scope to fix).

2. **Acceptance Criteria** — Collapsible section (`ChevronDown` toggle). Renders acceptance criteria text. When collapsed, shows one-line preview. Editable in future pass.

3. **Sub-Issues List** — List of child tasks with status indicator (colored dot), priority badge, and title. Each row clickable to navigate to that sub-issue via `navigateInPlace()`. Shows count in header: "Sub-issues (3/5 done)".

### IssueActivityTab.tsx

Chronological feed of task events. Each entry has:
- Avatar (user icon or purple/indigo gradient circle for AI)
- Actor name + timestamp (right-aligned, `text-muted`, use `formatRelativeTime()` from `shared/lib/dates.ts` for recent events, `formatTime()` for older)
- Action description

Entry types from mock data:
- Created, status changed, priority changed, description updated
- AI suggestion applied, AI decomposition, focus session completed
- Comment (future)

Uses `ActorType` to distinguish: `User` gets standard avatar, `Agent`/`System` gets purple gradient avatar.

### IssueDetailSidebar.tsx

260px fixed-width right column. Vertical stack of sections separated by `glass-divider`. Reads `taskState` to control section visibility per the state table.

#### Task State Derivation

```typescript
type TaskState = "new" | "focused" | "has-history" | "completed";

// MockDetailTask must include `completed: boolean` (mirrors shared/types/tasks.ts Task type,
// NOT the tasks2-local Issue type which uses status.id instead).
function deriveTaskState(task: MockDetailTask): TaskState {
  if (task.completed) return "completed";
  if (task.focused_at) return "focused";
  if (task.total_tracked_secs > 0) return "has-history";
  return "new";
}
```

#### State Table

| State       | Properties | Work State      | Time                     | AI Insights             |
|-------------|-----------|-----------------|--------------------------|-------------------------|
| new         | Full      | Hidden          | Estimate only (hidden if no estimate set) | "Why This Task Now?"    |
| focused     | Compact (Status, Priority, Energy, Due, Estimate only) | Live | Est + Tracked + Forecast | Top suggestion + memory |
| has-history | Full      | Hidden          | Est + Tracked + Forecast | Top suggestion + memory |
| completed   | Full      | Session summary | Final analysis           | "What AI Learned"       |

### SidebarProperties.tsx

Linear-style property rows. Each row: label (left, `text-muted`, 80px) + value (right, clickable). Click opens inline editor (dropdown or picker) using `glass-panel` for the popup.

Fields:
- **Status** — Colored dot + label. Dropdown with status options from mock data.
- **Priority** — Priority icon + "P1"/"P2"/etc. Dropdown.
- **Energy** — Energy level badge (Low/Medium/High/Deep). Dropdown.
- **Type** — Manual/Agentic/Hybrid. Dropdown.
- **Due date** — Formatted date or "No due date". Date picker popup.
- **Area** — Area name. Dropdown with areas from `tasks2/mock-data/areas.ts`.
- **Project** — Project name or "No project". Dropdown with projects from `tasks2/mock-data/projects.ts`.
- **Tags** — Tag pills. Click to add/remove.
- **Estimate** — "Xh Ym" or "No estimate". Inline input.

**Compact mode** (when focused): Hides Area, Project, Tags, Type. Shows only Status, Priority, Energy, Due, Estimate.

Property dropdowns render via portal (`PortalContainerProvider` already exists in tasks2) to avoid clipping.

### SidebarWorkState.tsx

Only visible when `taskState === "focused"`.

- **Timer** — `HH:MM:SS` display, `font-variant-numeric: tabular-nums`. Computes elapsed from `Date.now() - new Date(focused_at).getTime()`. Uses `setInterval` (1s) with cleanup on unmount via `useEffect` return.
- **Focus mode** — Badge: "Deep Work" / "Focus" / "Pomodoro". Static from mock.
- **Quality score** — `0.00–1.00` numeric display. Color-coded: green (>0.7), amber (0.4–0.7), red (<0.4).
- **Distraction count** — Number with label.
- **Flow state** — "Active" / "Building" / "Lost" badge. Color matches quality thresholds.
- **Quality sparkline** — Mini bar chart (5-minute buckets). Simple div-based bars, heights proportional to quality values. Uses CSS grid, 12–15 bars max.
- **Controls** — Pause and Stop buttons. Mock handlers (update local state).

### SidebarTime.tsx

Visible when `taskState !== "new"`, OR when `taskState === "new"` AND `estimated_minutes` is set. Hidden when `taskState === "new"` and no estimate exists.

- **Three-value display** (when not "new"): Estimated / Tracked / AI Forecast. Each as label + value pair.
- **Progress bar** — Horizontal bar. Fill color: green (<80% of estimate), amber (80–100%), red (>100%). Width = `min(tracked / estimated * 100, 100)%`.
- **Status text** — e.g., "52% · ahead of schedule", "105% · over estimate". Derived from ratio.

When `taskState === "new"`: shows only estimate field (editable).
When `taskState === "completed"`: shows final analysis text — actual vs estimated with deviation.

### SidebarAiInsights.tsx

Always visible, content varies by state.

**When suggestions exist:**
- Top suggestion card: title, description (2 lines max), confidence badge.
- Two buttons: "Apply" (primary small) and "Dismiss" (ghost small). Mock handlers.
- "See all (N more)" link expands remaining suggestions below.

**When `taskState === "new"` and no suggestions:**
- "Why This Task Now?" section with contextual reasons (mock data):
  - "High priority — P1"
  - "Due in 2 days"
  - "Matches your current energy window"

**When `taskState === "completed"`:**
- "What AI Learned" section with summary text.

**Task Memory section** (below suggestions):
- Last session summary (1–2 lines)
- Continuity note
- Related semantic facts (if any)
- All from mock data.

## Mock Data

Single file: `desktop-ui/src/features/tasks2/mock-data/issue-detail.ts`

Exports:
- `mockDetailTask` — Full task object with all fields populated. Must include `completed: boolean`, `total_tracked_secs: number`, `estimated_minutes: number | null`, `focused_at: string | null` (ISO timestamp). Shaped like `shared/types/tasks.ts` Task, not the tasks2-local `Issue` type.
- `mockActivityEntries` — 8–10 activity log entries (mix of user/agent/system)
- `mockSuggestions` — 3 AI suggestions with varying confidence
- `mockFocusSession` — Live focus session state (quality history as number[], distraction count, flow state, focus mode)
- `mockSubIssues` — 3–4 child tasks with varying completion states

## Hook

`useIssueDetail(issueId: string)` in `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`

Returns all mock data, derives `taskState`, provides mock update handlers:
```typescript
{
  task: MockDetailTask;
  taskState: TaskState;
  activity: MockActivityEntry[];
  suggestions: MockSuggestion[];
  focusSession: MockFocusSession | null;
  subIssues: MockSubIssue[];
  updateTask: (field: string, value: unknown) => void;
  dismissSuggestion: (id: string) => void;
  applySuggestion: (id: string) => void;
}
```

## Navigation Integration

In `TabContent.tsx`, the `"issue"` case currently shows a placeholder. Replace with:
```tsx
case "issue":
  return <IssueDetailView issueId={currentView.id} />;
```

Breadcrumb uses the nav stack from the tab store. Each segment maps to a stack entry. Clicking navigates back via `navigateToStackIndex()`.

## Styling Approach

- All theme tokens from `theme.css` — no hardcoded colors
- `glass-card` for sidebar section containers
- `glass-panel` for dropdown overlays (via portal)
- `glass-divider` between sidebar sections
- Scoped within `.tasks2-scope` (inherits existing tasks2 CSS variables)
- Editor styling from `src/styles/editor.css` (already global)

## Out of Scope

- Real backend data wiring (deferred — only `useIssueDetail` hook changes later)
- TipTap editor customization (using notes editor as-is)
- Keyboard shortcuts
- Drag-and-drop for sub-issues reordering
- Inline property editing beyond dropdowns (complex pickers like recurrence, dependencies)
- Mobile/tablet responsive below 900px (just collapse sidebar)
