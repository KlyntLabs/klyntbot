# Task2 Real Data Integration

**Date:** 2026-03-14
**Status:** Approved
**Scope:** Replace all mock data in `desktop-ui/src/features/tasks2/` with real backend data via existing Tauri IPC commands, plus a small backend addition (`createdAt`/`updatedAt` fields on `TaskResponse`).

## Context

The `tasks2` feature is a UI upgrade of the existing `tasks` feature, built with a Linear-inspired interface (tabbed navigation, kanban board, grid/list views, detail panel). It currently runs entirely on hardcoded mock data. The existing task system has a complete backend with Tauri commands, SQLite storage, and real-time event updates. This spec defines how to wire task2 to the real data.

## Architecture

```
Real Data (Tauri IPC)
  ├─ useQuery("task_list")              → task list/board views
  ├─ useQuery("task_get", {id})         → detail view
  ├─ useQuery("task_list_children")     → sub-issues
  ├─ useQuery("area_list")             → tab initialization
  ├─ useQuery("project_list")          → project resolution
  ├─ useQuery("timeline_query")        → activity tab (client-side filtered by entityId)
  ├─ useMutation("task_update")        → status/priority/field changes
  ├─ useMutation("task_create")        → create issue modal
  ├─ useMutation("task_delete")        → context menu delete
  ├─ useMutation("task_toggle_complete") → mark done
  └─ useEvent("entity:updated")        → auto-refresh
        ↓
Presentation Mappers (tasks2/lib/mappers.ts)
  ├─ resolveStatus(task)    → Status with icon
  ├─ resolvePriority(task)  → Priority with icon
  ├─ tagsToLabels(tags)     → LabelInterface[] with colors
  ├─ shortId(id)            → "T-a3f2" display identifier
  ├─ priorityToNumber(id)   → numeric value for TaskUpdateParams
  └─ taskToIssue(task, projectMap) → Issue shape for components
        ↓
Components (unchanged rendering logic)
```

## 1. Presentation Mappers

New file: `tasks2/lib/mappers.ts`

### resolveStatus(task: Task, statusLabels?: StatusLabel[]): Status

Maps the real `Task.status` + `Task.statusLabel` to a `Status` object with an SVG icon.

**Strategy (option C — match known names, fallback to generic):**
1. If `task.statusLabel` exists, check its `name` (case-insensitive) against known status names: "Backlog", "Todo"/"To Do", "In Progress", "Technical Review"/"In Review", "Paused"/"On Hold", "Completed"/"Done". Match → use the corresponding SVG icon from `status-icons.tsx`.
2. If no name match, use `statusLabel.color` to render a generic colored circle icon (same SVG structure, dynamic color).
3. If no `statusLabel` at all, map `task.status` string: `"open"` → Todo, `"completed"` → Completed, `"in_progress"` → InProgress. Fallback → Backlog icon.

### resolvePriority(task: Task): Priority

Maps `task.priority` (stored as `string | null` representing a number) to a `Priority` object with icon.

| `task.priority` | Priority ID | Icon |
|---|---|---|
| `null` or `"0"` | `"no-priority"` | NoPriorityIcon |
| `"1"` | `"urgent"` | UrgentPriorityIcon |
| `"2"` | `"high"` | HighPriorityIcon |
| `"3"` | `"medium"` | MediumPriorityIcon |
| `"4"` | `"low"` | LowPriorityIcon |

### priorityToNumber(priorityId: string): number | null

Reverse mapping for mutations. When a component selects a `Priority` object by ID (e.g., `"urgent"`), this converts it back to the numeric value needed by `TaskUpdateParams.priority`:

| Priority ID | Numeric value |
|---|---|
| `"no-priority"` | `null` |
| `"urgent"` | `1` |
| `"high"` | `2` |
| `"medium"` | `3` |
| `"low"` | `4` |

Used in `StatusSelector`, `PrioritySelector`, and `SidebarProperties` when dispatching `task_update` mutations.

### tagsToLabels(tags: string[]): LabelInterface[]

Maps tag strings to `LabelInterface[]` for visual badge rendering. Uses a fixed color palette and deterministic hash of the tag name to pick a color. Ensures the same tag always gets the same color across views.

Color palette: `["purple", "red", "green", "blue", "yellow", "orange", "pink", "gray", "indigo", "teal", "cyan"]`

### shortId(id: string): string

Returns `"T-"` + first 4 hex characters of the task UUID. Pure display function, no backend changes. Example: `"a3f2b1c4-..."` → `"T-a3f2"`.

### taskToIssue(task: Task, projectMap: Map<string, DisplayProject>): Issue

Convenience mapper that composes all the above into the `Issue` shape that existing components render. This avoids touching every component during the initial integration — components can gradually migrate to using `Task` directly.

```typescript
{
  id: task.id,
  identifier: shortId(task.id),
  title: task.title,
  description: task.description ?? "",
  status: resolveStatus(task),
  assignee: null,  // single-user app
  priority: resolvePriority(task),
  labels: tagsToLabels(task.tags),
  createdAt: task.createdAt ?? "",  // requires backend addition (see Section 7)
  cycleId: "",
  project: task.projectId ? projectMap.get(task.projectId) : undefined,
  subissues: task.subtaskCount > 0 ? Array(task.subtaskCount).fill("") : undefined,
  // ^ only used for .length check in UI, not individual entries
  rank: "",
  dueDate: task.dueDate ?? undefined,
}
```

### projectToDisplayProject(project: RealProject): DisplayProject

Maps the real `Project` type (`{ id, name, color, areaId, ... }`) to the display type used by task2 components (`{ id, name, icon }`). Assigns a default `Folder` icon from lucide-react and preserves name/id. Components that show the project badge render a colored dot using `project.color` alongside the name.

```typescript
interface DisplayProject {
  id: string;
  name: string;
  color: string;
  icon: React.FC<LucideProps>;  // default: Folder
}
```

## 2. Data Fetching Hook — `useTasks`

New file: `tasks2/hooks/useTasks.ts`

Replaces `issues-store.ts`. Follows the same pattern as `TasksPage.tsx`.

```typescript
interface UseTasksResult {
  issues: Issue[];
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
  updateTask: (params: TaskUpdateParams) => Promise<void>;
  createTask: (params: TaskCreateParams) => Promise<void>;
  deleteTask: (id: string) => Promise<void>;
  toggleComplete: (id: string) => Promise<void>;
  // Mapped lookups
  projectMap: Map<string, DisplayProject>;
  areaMap: Map<string, Area>;
  areas: Area[];
}
```

**Internals:**
- `useQuery("task_list")` for tasks
- `useQuery("project_list")` for project resolution
- `useQuery("area_list")` for area resolution
- `useMutation` for create/update/delete/toggle
- `useEvent("entity:updated")` for auto-refresh (same pattern as TasksPage: refetch tasks on `"task"` or `"area"` events, refetch projects on `"project"` events)
- `useMemo` to run `taskToIssue()` over the task list
- Filtering logic (`filterIssues`) moves here as a `useMemo` over the mapped issues + filter store state

**Parameter naming:** Tauri commands use `#[serde(rename_all = "camelCase")]`, so frontend calls must use camelCase keys: `{ areaId: "..." }`, `{ projectId: "..." }`, `{ parentId: "..." }`.

## 3. Detail Hook — `useIssueDetail` Rewrite

Replace mock data with real queries:

```typescript
function useIssueDetail(issueId: string) {
  // Core task data
  const { data: rawTask } = useQuery("task_get", { id: issueId });
  const task = rawTask ? mapToDetailTask(rawTask, projectMap, areaMap) : null;

  // Sub-issues
  const { data: rawChildren } = useQuery("task_list_children", { parentId: issueId });
  const subIssues = rawChildren?.map(mapToSubIssue) ?? [];

  // Activity from timeline (client-side filtered — see note below)
  const { data: timeline } = useQuery("timeline_query", {
    startDate: "2020-01-01T00:00:00Z",
    endDate: new Date().toISOString(),
    sources: ["task"],
  });
  // Filter client-side by entityId since timeline_query doesn't support entity filtering
  const activity = useMemo(
    () => (timeline?.entries ?? [])
      .filter((e) => e.entityId === issueId)
      .map(timelineToActivity),
    [timeline, issueId]
  );

  // Mutations
  const updateMutation = useMutation("task_update", "params");
  const startFocus = useMutation("task_start_focus");
  const endFocus = useMutation("task_end_focus");

  // Suggestions: empty (no backend yet)
  const suggestions: MockSuggestion[] = [];

  // Memory: null (no backend bridge yet)
  const taskMemory = null;

  // Focus session: derive from task.focusedAt
  const focusSession = task?.focusedAt ? deriveFocusSession(task) : null;

  return { task, taskState, activity, suggestions, focusSession, subIssues, taskMemory, updateTask, dismissSuggestion, applySuggestion };
}
```

**Note on timeline filtering:** The `timeline_query` Tauri command (`TimelineQuery` struct) does not support `entityId` filtering. We fetch task-source entries and filter client-side by `entry.entityId === issueId`. This is acceptable for now since task timeline entries are lightweight. A future optimization can add `entity_id: Option<String>` to `TimelineQuery` for server-side filtering.

**`mapToDetailTask`**: Extends `taskToIssue` mapping with detail-specific fields: `energyLevel`, `taskType`, `estimatedMinutes`, `actualMinutes`, `totalTrackedSecs`, `focusedAt`, `acceptanceCriteria`, `completed`, `createdAt`, `updatedAt`, `area`, `tags`. The `createdAt`/`updatedAt` fields come from the backend addition (see Section 8).

**`timelineToActivity`**: Maps `TimelineEntry` → `MockActivityEntry`:
- `entry.entryType` → `action` (e.g., `"taskCreated"` → `"created task"`, `"taskUpdated"` → `"updated task"`)
- `entry.title` → `detail`
- Actor: `"System"` for system events, `"You"` for user events, `"Klyntbot"` for agent events (derive from entry metadata or source)

**`deriveFocusSession`**: Creates a basic `MockFocusSession` from `task.focusedAt` and `task.totalTrackedSecs`. Quality score and flow state are derived or use sensible defaults until a richer focus backend is available.

## 4. Tab Store Update

Modify `tab-store.ts`:
- Remove `import { areas } from "../mock-data/areas"`
- Remove `const defaultTabs = buildDefaultTabs(areas)` initialization
- Add `initFromAreas(areas: Area[])` action that builds default tabs from real area data
- Start with a single "My Issues" tab as default before areas load
- `Tasks2Page.tsx` calls `initFromAreas(areas)` when `useQuery("area_list")` resolves

The `NavEntry` type, `Tab` type, and all navigation logic (navStack, reorder, close) stay unchanged — they are pure UI state.

## 5. Component Changes

### No changes needed (receive mapped `Issue` via props, no mock imports):
- `IssueBoard.tsx` — no change
- `IssueLine.tsx` — no change
- `IssueGrid.tsx` — no change
- `GroupIssues.tsx` — no change
- `TabBar.tsx` — no change
- `TabContent.tsx` — no change
- `HeaderNav.tsx` / `HeaderOptions.tsx` — no change
- `Filter.tsx` — no change
- `LabelBadge.tsx` — no change
- `IssueDetailBreadcrumb.tsx` — no change (only imports from tab-store, no mock data)
- `TabContextMenu.tsx` — no change (pure UI state)
- `Tasks2Layout.tsx` — no change (pure layout wrapper)
- `portal-context.tsx` — no change (pure context provider)

### Data source changes:
- **`AllIssues.tsx`**: Replace `useIssuesStore` with `useTasks()` hook
- **`AreaView.tsx`**: Replace `useIssuesStore().filterByProject(areaProjectIds)` with `useTasks()` filtered by area. Note: mock `MockArea` has `projectIds: string[]` but real `Area` does not — filter by `task.areaId` directly instead of going through projects.
- **`ProjectView.tsx`**: Replace `useIssuesStore().filterByProject(projectId)` with `useTasks()` filtered by `task.projectId`
- **`SearchIssues.tsx`**: Use client-side filter on `useTasks().issues` (filter by title, description, identifier)
- **`Tasks2Page.tsx`**: Add `useTasks()`, pass data down, call `initFromAreas()`
- **`AddTabMenu.tsx`**: Replace `import { areas }` and `import { projects }` from mock data with props or context receiving real `areas` and `projects` from `useTasks()` hook. Receive these as props from `Tasks2Page`.

### Mutation wiring:
- **`StatusSelector.tsx`**: Call `useMutation("task_update", { id, statusLabelId })` instead of `useIssuesStore().updateIssueStatus()`. The status list comes from `useEffectiveLabels()` (same hook as existing TasksPage) instead of the mock `status` array.
- **`PrioritySelector.tsx`**: Call `useMutation("task_update", { id, priority: priorityToNumber(selectedId) })` instead of store. The priority list comes from `priority-icons.tsx` (renamed from mock-data).
- **`CreateIssueModal.tsx`**: Wire to `useTasks().createTask()`
- **`IssueContextMenu.tsx`**: Wire delete to `useTasks().deleteTask()`

### Detail view:
- **`IssueDetailView.tsx`**: Consume rewritten `useIssueDetail()` — same return shape
- **`IssueActivityTab.tsx`**: Render real timeline entries (same `MockActivityEntry` interface)
- **`SidebarAiInsights.tsx`**: Render empty state when `suggestions.length === 0` with "AI insights will appear here as you work on this task" message
- **`SidebarWorkState.tsx`**: Wire focus start/end to real mutations
- **`SidebarProperties.tsx`**: Replace `import { priorities }` from mock data with import from relocated `priority-icons.tsx`. Replace `import { status as allStatus }` from mock data with status list from `useEffectiveLabels()` hook (passed as prop or via context). Replace `MockDetailTask` type import with the new mapped detail type. Wire `onUpdate` to dispatch `task_update` mutations with proper type conversions (e.g., `priorityToNumber()` for priority changes).
- **`SidebarTime.tsx`**: Replace `MockDetailTask` and `TaskState` type imports from mock data with the new mapped detail types from `mappers.ts`.
- **`IssueContentTab.tsx`**: Render real `description` and `acceptanceCriteria`

### Import path fix:
- **`lib/status-utils.tsx`**: Update import from `../mock-data/status` to `./status-icons` after the rename.

### Project type adaptation:
- Task2's `Project` mock type has `icon: React.FC<LucideProps>`. The real `Project` type has `color: string` but no icon.
- Solution: `projectToDisplayProject()` mapper (see Section 1) creates a `DisplayProject` with a default `Folder` icon. `ProjectBadge.tsx` renders a colored dot using `project.color` alongside the name.

## 6. File Changes Summary

### Create:
- `tasks2/lib/mappers.ts` — all presentation mappers + `DisplayProject` type + `priorityToNumber()`
- `tasks2/hooks/useTasks.ts` — data fetching hook

### Rewrite:
- `tasks2/hooks/useIssueDetail.ts` — real data queries
- `tasks2/store/tab-store.ts` — real area initialization

### Rename (UI assets, not mock data):
- `mock-data/status.tsx` → `lib/status-icons.tsx`
- `mock-data/priorities.tsx` → `lib/priority-icons.tsx`

### Modify (wire mutations + fix imports):
- `tasks2/components/AllIssues.tsx`
- `tasks2/components/AreaView.tsx`
- `tasks2/components/ProjectView.tsx`
- `tasks2/components/SearchIssues.tsx`
- `tasks2/components/AddTabMenu.tsx` — replace mock area/project imports with props
- `tasks2/components/StatusSelector.tsx`
- `tasks2/components/PrioritySelector.tsx`
- `tasks2/components/CreateIssueModal.tsx`
- `tasks2/components/IssueContextMenu.tsx`
- `tasks2/components/detail/IssueDetailView.tsx`
- `tasks2/components/detail/IssueActivityTab.tsx`
- `tasks2/components/detail/SidebarAiInsights.tsx`
- `tasks2/components/detail/SidebarWorkState.tsx`
- `tasks2/components/detail/SidebarProperties.tsx` — replace mock status/priority imports
- `tasks2/components/detail/SidebarTime.tsx` — replace mock type imports
- `tasks2/lib/status-utils.tsx` — update import path after rename
- `tasks2/pages/Tasks2Page.tsx`

### Delete:
- `mock-data/issues.ts`
- `mock-data/issue-detail.ts`
- `mock-data/areas.ts`
- `mock-data/projects.ts`
- `mock-data/users.ts`
- `mock-data/labels.ts`
- `store/issues-store.ts`

### Keep unchanged:
- `lib/utils.ts`
- `store/filter-store.ts` (pure UI state)
- `store/view-store.ts` (pure UI state)
- `store/create-issue-store.ts` (pure UI state)
- `store/search-store.ts` (pure UI state)
- All UI primitive components (`components/ui/`)
- `components/IssueDetailBreadcrumb.tsx` (no mock imports)
- `components/TabContextMenu.tsx` (no mock imports)
- `components/Tasks2Layout.tsx` (no mock imports)
- `components/portal-context.tsx` (no mock imports)

## 7. Backend Change — Add `createdAt` / `updatedAt` to TaskResponse

The `TaskResponse` struct in `crates/desktop-shared/src/commands/tasks.rs` currently omits `created_at` and `updated_at`, but `TaskRow` in storage has both fields. The detail view needs these for display.

**Change:** Add two fields to `TaskResponse`:
```rust
pub created_at: Option<String>,  // ISO 8601
pub updated_at: Option<String>,  // ISO 8601
```

**Change:** Update the converter in `crates/app-core/src/handlers/tasks/converters.rs` to populate these from `TaskRow.created_at` and `TaskRow.updated_at`.

**Change:** Add `createdAt` and `updatedAt` (both `string | undefined`) to the frontend `Task` interface in `desktop-ui/src/shared/types/tasks.ts`.

This is a small, additive change — no migration needed, no breaking changes.

## 8. Not In Scope

- Backend for AI suggestions — future feature, stubbed with empty state
- Backend for task memory bridging — future feature, stubbed with empty state
- Server-side `entityId` filtering on `timeline_query` — future optimization, client-side filtering used for now
- `assignee` / multi-user support — single-user app
- Sprint/cycle management — no concept in current system
- Backend `sequence_number` for identifiers — future upgrade from short hash (`T-a3f2`)
