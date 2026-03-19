# Project Detail Page + OKR System — Design Spec

> Date: 2026-03-19
> Status: Approved
> Scope: Iteration 1 — 4 tabs (Overview, Tasks, OKR, Notes)

## 1. Problem Statement

Klyntbot has full CRUD backends for Projects (7 commands) and OKRs (8 commands), but the frontend treats projects as mere task filters and has zero OKR UI. Users cannot create, view, or manage objectives or key results. Projects are "task folders" instead of intelligent containers.

**Goal:** Transform the Project Detail Page into the "second brain center" — an intelligent, interconnected container for tasks, notes, insights, memories, and goals. OKR becomes the goal layer sitting on top of the project. When a user opens a project, they should feel: "this is the brain of Project X."

## 2. Architecture Decision

**Approach A: Dedicated Route with Lazy Tabs** (selected over Expand Tasks Page and Slide-Over Panel).

Rationale:
- Makes the Project Detail Page THE center of a project — not a panel bolted onto Tasks
- Deep-linkable tabs via URL segments
- Future tabs (Finance, Productivity, Timeline) slot in naturally
- Full-page real estate for complex content (kanban, note editor, graphs)
- Global Tasks page remains as-is for cross-project task management

## 3. Routing

```
/project/:id          → Overview (default tab)
/project/:id/tasks    → Tasks kanban
/project/:id/okr      → OKR tree
/project/:id/notes    → Notes split view
```

Route is added to the existing HashRouter in `desktop-ui/src/App.tsx`. Each tab segment maps to a lazy-loaded component.

## 4. Page Layout

```
ProjectDetailPage (full height, flex column)
├─ ProjectHeader (sticky top)
│   ├─ Back arrow (navigates to previous page or /tasks)
│   ├─ Project color dot + name (inline editable)
│   ├─ Area badge (clickable → filters by area)
│   ├─ Health Score ring (clickable → jumps to OKR tab)
│   ├─ "Ask AI about this project" button → opens sidebar chat with full project context
│   └─ Actions context menu: Edit | Archive | Delete
├─ GlassTabBar (sticky below header)
│   ├─ Tabs: Overview | Tasks (count) | OKR (progress%) | Notes (count)
│   ├─ Live indicator dots: green (tasks due today), yellow (at-risk KR), purple (notes without insights)
│   └─ Draggable reorder (order persisted to project.settings.tabOrder)
├─ TabContent (flex-1, scrollable)
│   └─ Lazy-loaded tab component based on URL segment
└─ QuickAddFAB (rendered at ProjectDetailPage level, OUTSIDE TabContent to avoid overflow clipping)
    ├─ Default action: "+ Add Task" (scoped to project)
    └─ Dropdown: New Task | New Note | New Objective
```

**"Ask AI about this project" button:**
- Opens sidebar chat (`SidebarChat` component)
- Session key: `project-chat-{projectId}` (stable per project, preserves conversation history)
- Context payload: `{ projectId, projectName, activeTab, objectiveSummary, recentInsight, activeWorkContext }`
- Assembled from `ProjectContext` data at call time

**Tab order persistence:**
- Stored in `project.settings.tabOrder` as `string[]` (e.g., `["overview", "tasks", "okr", "notes"]`)
- On reorder: read current `project.settings` → merge `tabOrder` key → call `project_update({ id, settings: mergedSettings })`
- Read-merge-write pattern prevents clobbering other settings keys

## 5. Data Fetching

### Page-level (always loaded)

| Hook | Command | Purpose |
|------|---------|---------|
| `useProject(id)` | `project_get` | Project metadata, task counts, objective IDs, settings |
| `useProjectObjectives(id)` | `objective_list` with `{ projectId: id }` (note: command lives in `tasks.rs`, not `objectives.rs`) | OKR data for Health Score + tab badge |

These are provided via `ProjectContext` to all child components.

### Tab-specific (lazy, only when tab is active)

| Hook | Command | Purpose |
|------|---------|---------|
| `useProjectTasks(id)` | `task_list` with `{ projectId: id }` | Tasks tab kanban |
| `useProjectNotes(notebookIds)` | Multiple `note_list({ notebookId })` calls (one per project notebook), merged client-side. `notebookIds` read from `project.settings.notebookIds`. | Notes tab |
| `useProjectActivity()` | No IPC call — reads from `ProjectContext` (tasks, objectives from page-level hooks) and aggregates `updatedAt` timestamps client-side | Overview timeline |

### Refetch strategy

All mutations use `useMutation` which emits `entity:updated` events. Page-level `useEvent("entity:updated")` triggers selective refetch based on `entityKind` (project, task, objective, key_result, note).

## 6. Tab Designs

### 6.1 Overview Tab — Project Brain Dashboard

**Layout:** 3-row grid of cards.

**Row 1 — Stats Cards:**

| Card | Data Source | Interaction |
|------|-----------|-------------|
| Health Score | Computed client-side (formula below) | Click → OKR tab, highlights lowest KR. Gradient ring (green→yellow→red). |
| Task Progress | `project_get` → `taskCount`/`completedCount` + filtered task_list for due today/overdue | Click → Tasks tab |
| OKR Summary | Top 3 objectives with inline progress bars from `useProjectObjectives` | Click objective → OKR tab with that objective expanded |

**Row 2 — Intelligence Cards:**

| Card | Data Source | Interaction |
|------|-----------|-------------|
| Active Work Context | `get_dashboard_intelligence` command → extracts current context summary from `DashboardIntelligenceResponse` | Click → Work Contexts page. Shows dominant app, duration, productivity %. Empty state: "No active session" |
| Latest Insight | `note_insight_cache_get` called per project note (from `useProjectNotes`), pick the most recent non-null result. N+1 mitigated by only checking the 5 most recently updated notes. | "View Insight" navigates to Notes tab. "Create Task" opens pre-filled task modal. Badge: "Generated X mins ago". "Ask AI to expand" opens sidebar chat. |
| Coaching Signal | Listens for `coaching:intervention` Tauri event via `useEvent("coaching:intervention")`. Displays the latest intervention payload. Feedback actions dispatch `coaching_submit_feedback` command. | "Helpful" / "Dismiss" feedback. "Why this matters" line from intervention payload. Empty state: "No active coaching — Deep work mode detected" (positive framing). |

**Row 3 — Recent Activity Timeline:**

Smart-grouped by time period (Today, This Week). Each item shows:
- Timestamp + color-coded dot (green=task, purple=note, yellow=KR, blue=focus)
- Description text
- Quick action button (e.g., completed task → "View linked KR")

Reads from `ProjectContext` (tasks, objectives) — no additional IPC calls. Aggregates `updatedAt` fields and sorts chronologically.

**Health Score Formula:**

```
healthScore = (
  okrProgress * 0.60 +           // weighted avg of KR progress values
  taskVelocity * 0.20 +          // completed / total tasks in last 7 days
  insightFreshness * 0.10 +      // 1.0 if newest insight < 7 days, linear decay to 0
  focusQuality * 0.10            // avg productivity % from dashboard intelligence
)
```

Note: `cognitivePatternMatch` factor deferred to iteration 2 (requires project-scoped SemanticFact queries not currently available). Formula uses 4 factors for iteration 1.

Color thresholds: green (>70%), yellow (40-70%), red (<40%).

The Health Score ring is clickable → navigates to OKR tab and highlights the KR pulling the score down.

AI Confidence badge on objectives is clickable → tooltip with breakdown: "60% from KR velocity, 20% from task completion rate, 10% from insight freshness, 10% from focus quality".

### 6.2 Tasks Tab — Project-Scoped Kanban

Adapts the existing `ProjectView` kanban component from the Tasks page.

**Layout:**
- Toolbar: Board/List toggle + Group by (Status default) + Filter + "+ Task" button
- Kanban columns based on project's status workflow

**Key additions over existing kanban:**

| Feature | Description |
|---------|-------------|
| KR link badge | Small target icon + KR name text on task cards. Tooltip shows full KR title + progress. Click → OKR tab. |
| "Link to KR" context menu | Opens picker showing project objectives/KRs. At-risk and low-confidence KRs shown first. |
| Task completion cascade | Completing task → `entity:updated` → KR metric recalculates (if tracking_mode = task_count) → objective progress updates → Health Score ring refreshes |
| Pre-scoped creation | "+ Task" button pre-fills `projectId`. Optional KR link field in creation modal. |

### 6.3 OKR Tab — Objective & Key Result Tree

**Layout:**
- Header: "Objectives" label + overall progress badge (also shown in GlassTabBar as tab badge) + Filter (All/On Track/At Risk/Achieved) + "+ New Objective" button
- Tree: collapsible Objective cards containing KR rows

**Objective Card (collapsible):**
- Progress ring (md size) with percentage
- Title + KR count + due date
- Status badge: On Track (green) / At Risk (yellow) / Achieved (green, filled). Computed client-side: "At Risk" if progress < expected_progress_for_elapsed_time, "Achieved" if progress >= 1.0.
- AI Confidence badge: computed client-side from `(kr_velocity_score * 0.7 + task_completion_rate * 0.3)`. Clickable → breakdown tooltip. Shows coaching link text if confidence < 50%.
- Context menu: Edit, Delete, Change Status
- "Ask AI: Suggest next KR" button → calls agent with full project context → returns 2-3 KR suggestions

Note: `ObjectiveResponse` has `id`, `title`, `status`, `progress`, `projectId`, `keyResults`. The `description`, `priority`, `dueDate` fields exist on `ObjectiveCreateParams`/`ObjectiveUpdateParams` but are not on the response type. The Edit modal must re-fetch or use the create/update params. If displaying due date in the card is needed, the `ObjectiveResponse` type should be extended (minor backend change — add `description`, `priority`, `due_date` to the response converter).

**Key Result Row (nested under objective):**
- Mini progress ring
- Title + current/target metric display (e.g., "245ms / 200ms")
- Linked task count: fetched lazily when KR row is expanded. Calls `task_list({ projectId })` and filters client-side for tasks linked to this KR (via `entity_link` or a tag convention). Initially shows "N tasks" badge from a count stored alongside the KR or fetched on expand.
- "+ Link Task" button → task picker scoped to project
- Context menu: Edit, Delete, "Create Task from Gap"

**Task-KR linking strategy:**
Tasks are linked to KRs via the task's metadata. When "Link to KR" is used, `task_update({ id, metadata: { keyResultId: "kr-id" } })` stores the link. `useProjectTasks` provides the full task list; filtering by `task.metadata.keyResultId === kr.id` gives linked tasks per KR. No new backend schema needed.

**Inline metric editing:**
- Click current value on KR → inline number input → save → calls `key_result_update_metric` → auto-recalculates objective progress via backend

**Linked Tasks (expandable under KR):**
- Shows task checkboxes + title + status
- Completing a task here triggers the cascade: KR metric → Objective → Health Score

**Empty state:**
- Dashed border card: "+ New Objective — or ask AI: 'Suggest objectives for this project'"

**CRUD mapping:**

| Action | Backend Command |
|--------|----------------|
| Create objective | `objective_create({ title, projectId, description?, priority?, dueDate? })` |
| Edit objective | `objective_update({ id, title?, description?, status?, priority?, dueDate? })` |
| Delete objective | `objective_delete({ id })` |
| Create KR | `key_result_create({ objectiveId, title, targetValue?, unit?, trackingMode? })` |
| Edit KR | `key_result_update({ id, title?, description?, status?, dueDate? })` |
| Update KR metric | `key_result_update_metric({ id, currentValue })` |
| Delete KR | `key_result_delete({ id })` |

### 6.4 Notes Tab — Knowledge-to-Action Bridge

**Layout:** Split view
- Left sidebar (220px): Search bar + Notebook tree + Recent notes list + Mini graph preview
- Right content: Note header + TipTap editor body + Action bar + Insight preview

**Left sidebar:**
- Search: calls `note_search_hybrid` scoped to project notebooks
- Notebook tree: shows notebooks linked to this project (from `project.settings.notebookIds`). Click notebook → filters notes.
- Recent notes: sorted by `updatedAt`, shows title + timestamp + backlink count
- Mini graph: Cytoscape-powered preview of note relationships. Click → expands to full graph view.

**Right content:**
- Note header: title + notebook name + updated timestamp + backlink count badge
- Note body: TipTap rich text editor (reuses existing note editor component). Wikilinks (`[[Note Name]]`) rendered as clickable links navigating within the tab.
- Action bar (below note body, above border):
  - **Generate Insight** (primary/brand color) → calls `note_insight_review` (SSE-based). This command initiates a streaming insight review session. The UI shows a loading state ("Generating insight...") while SSE events stream in, then renders the final result as an `InsightPreview` card.
  - **Link to KR** (secondary) → opens KR picker → stores link in note metadata via `note_update`
  - **Create Task** (secondary) → task modal pre-scoped to project with optional KR link
  - **Flashcards** (secondary) → calls `flashcard_generate`

**Insight preview panel (below action bar, when insight exists):**
- Insight data loaded via `note_insight_cache_get({ noteId })` — returns cached insight if one exists, null otherwise
- Badge: "AI Insight · Generated X mins ago"
- "Refresh" button → calls `note_insight_review` again (SSE streaming)
- AI-generated summary text
- "Create suggested task" button → pre-fills task title + description from insight content + auto-links to nearest matching KR (matched by keyword overlap between insight text and KR titles)

**Note-Project scoping strategy:**
- Notebook-based: When a project is created, the frontend automatically calls `notebook_create({ name: projectName })` and stores the notebook ID in `project.settings.notebookIds: string[]` via `project_update`.
- Notes in those notebooks belong to the project.
- Users can link additional notebooks via a "Link Notebook" button in the Notes tab sidebar (adds notebook ID to the array).
- `useProjectNotes` calls `note_list({ notebookId })` for each ID in the array, merges results client-side, deduplicates, and sorts by `updatedAt`.

**Minor backend addition needed:** The `project_create` handler does NOT auto-create a notebook. The frontend handles this: after `project_create` succeeds, it calls `notebook_create` + `project_update` to store the notebook ID. This is a 2-step frontend flow, not a backend change.

## 7. New Shared Components

| Component | Props | Purpose |
|-----------|-------|---------|
| `GlassTabBar` | `tabs: { id, label, badge?, icon?, indicatorColor? }[]`, `activeTab: string`, `onTabChange`, `onReorder` | Reusable glass-style tab bar with drag reorder (via dnd-kit), badges, indicator dots. |
| `ProgressRing` | `progress: number`, `size: "sm" \| "md" \| "lg"`, `color?: string`, `gradient?: boolean` | SVG progress ring with gradient support. Extends the existing `ProgressRing` in `features/finance/` with size variants. Check compatibility — if the finance version is reusable, extend it; if not, create a new shared version and migrate finance to use it. |
| `ObjectiveCard` | `objective: Objective`, `onExpand`, `onEdit`, `onDelete` | Collapsible objective with KR tree, AI confidence, context menu. |
| `KeyResultRow` | `keyResult: KeyResult`, `onMetricEdit`, `onLinkTask`, `onDelete` | KR row with inline metric edit, linked task badges. |
| `QuickAddFAB` | `projectId: string`, `onAction: (type) => void` | Split button with dropdown. Positioned at `ProjectDetailPage` level (outside scrollable `TabContent`) to avoid overflow clipping. |
| `ActionBar` | `actions: { label, icon, variant, onClick }[]` | Row of action buttons. Used in Notes tab. |
| `InsightPreview` | `insight: InsightData`, `onCreateTask`, `onRefresh`, `loading?: boolean` | Inline AI insight card with create-task one-click action. `loading` state shown during SSE streaming. |
| `ProjectHealthRing` | `score: number`, `onClick`, `breakdown: BreakdownItem[]` | Project Health Score gradient ring with clickable tooltip showing factor breakdown. Named `ProjectHealthRing` to avoid collision with existing `HealthScoreRing` in finance feature. |

## 8. Empty States

| Tab | Empty State Message | Actions |
|-----|-------------------|---------|
| Overview | "Your project brain is empty. Start by adding tasks, notes, or objectives." | 3 quick-start buttons: Create Task, Create Note, Create Objective |
| Tasks | "No tasks yet. Create your first task or ask AI to suggest tasks for this project." | "+ Create Task" button + "Ask AI" link |
| OKR | "No objectives defined. Create one or ask AI: 'Suggest objectives for this project.'" | Dashed card with "+ New Objective" |
| Notes | "No notes linked to this project. Create a note or link an existing notebook." | "+ Create Note" button + "Link Notebook" button |

## 9. Future Tabs (Iteration 2-3)

The `GlassTabBar` accepts any `tabs` array. These tabs are NOT built in iteration 1 but their data contracts are defined:

| Tab | Data Source | Backend Ready? | Effort |
|-----|-----------|---------------|--------|
| Insights | `note_insight_cache_get` per project note | Yes | 1 day |
| Memories | `cognitive_fact_list` + `project_memories_list` | Yes | 1 day |
| Finance | `finance_*` filtered by project-linked accounts | Partial — needs project-account linking | 2 days |
| Productivity | `productivity_*` scoped to project work contexts | Yes | 1 day |
| Timeline | Aggregation of all entity timestamps | Client-side | 1 day |

## 10. Sidebar Navigation

The main sidebar gets a "Projects" section:
- Lists all projects as clickable items (project color dot + name + Health Score mini ring)
- Clicking navigates to `/project/:id` (Overview tab)
- "+" button creates a new project via modal (calls `project_create`, then `notebook_create` + `project_update` to link default notebook)
- The existing global Tasks page (`/tasks`) remains as-is for cross-project task management

## 11. Backend Commands Used

**Existing commands (no backend changes needed):**

**Project:** `project_get`, `project_list`, `project_create`, `project_update`, `project_delete`, `project_archive`
**OKR:** `objective_create`, `objective_get`, `objective_update`, `objective_delete`, `objective_list` (in tasks.rs), `key_result_create`, `key_result_update`, `key_result_update_metric`, `key_result_delete`
**Tasks:** `task_list`, `task_create`, `task_update`, `task_delete`, `task_toggle_complete`
**Notes:** `note_list` (takes single `notebookId`), `note_get`, `note_create`, `note_update`, `notebook_create`, `note_search_hybrid`, `note_insight_review` (SSE-based), `note_insight_cache_get`, `note_insight_regenerate_tab`, `flashcard_generate`
**Intelligence:** `get_dashboard_intelligence` (work context data), `coaching_submit_feedback` (feedback on coaching interventions)
**Events (Tauri, not commands):** `coaching:intervention` (listened via `useEvent`)

**Recommended minor backend enhancement (optional, not blocking):**
- Extend `ObjectiveResponse` to include `description`, `priority`, `due_date` fields (currently only on create/update params, not the response type). This enables displaying due dates and descriptions in the OKR tab cards without extra fetches.

## 12. Non-Goals (Iteration 1)

- No drag-and-drop tasks between KRs in the OKR tab
- No project templates or cloning
- No multi-project OKR dashboard (comes with OKR Dashboard page later)
- No finance or productivity integration (future tabs)
- No project-level chat history (the "Ask AI" button opens a new scoped session per project)
- No `cognitivePatternMatch` in Health Score (deferred to iteration 2 when project-scoped SemanticFact queries are available)
- No real-time task-to-KR drag linking (use context menu "Link to KR" instead)
