# Task2 Real Data Integration — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all mock data in the tasks2 feature with real backend data via existing Tauri IPC commands.

**Architecture:** A presentation mapper layer transforms flat `Task` data into rich `Issue` objects with status/priority icons. A `useTasks()` hook replaces the zustand issues-store with `useQuery`/`useMutation`/`useEvent`. Components receive the same `Issue` shape and need minimal changes.

**Tech Stack:** TypeScript, React, Zustand, Tauri IPC (`useQuery`/`useMutation`/`useEvent`), Rust (`TaskResponse`/`TaskRow`)

**Spec:** `docs/superpowers/specs/2026-03-14-task2-integration-design.md`

---

## Chunk 1: Backend + Foundation

### Task 1: Add `createdAt`/`updatedAt` to TaskResponse

**Files:**
- Modify: `crates/desktop-shared/src/commands/tasks.rs:7-34` (TaskResponse struct)
- Modify: `crates/app-core/src/handlers/tasks/converters.rs:16-48` (row_to_task_response fn)
- Modify: `desktop-ui/src/shared/types/tasks.ts:5-32` (Task interface)

- [ ] **Step 1: Add fields to Rust TaskResponse**

In `crates/desktop-shared/src/commands/tasks.rs`, add two fields at the end of `TaskResponse`:

```rust
pub struct TaskResponse {
    // ... existing fields ...
    pub focused_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
```

- [ ] **Step 2: Populate in converter**

In `crates/app-core/src/handlers/tasks/converters.rs`, add to `row_to_task_response()`:

```rust
TaskResponse {
    // ... existing fields ...
    focused_at: row.focused_at.map(|dt| dt.to_rfc3339()),
    created_at: Some(row.created_at.to_rfc3339()),
    updated_at: Some(row.updated_at.to_rfc3339()),
}
```

- [ ] **Step 3: Add to frontend Task type**

In `desktop-ui/src/shared/types/tasks.ts`, add to the `Task` interface after `focusedAt`:

```typescript
export interface Task {
  // ... existing fields ...
  focusedAt?: string;
  createdAt?: string;
  updatedAt?: string;
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: successful build

- [ ] **Step 5: Run existing tests**

Run: `cargo nextest run -p app-core -E 'test(task)' 2>&1 | tail -10`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/desktop-shared/src/commands/tasks.rs crates/app-core/src/handlers/tasks/converters.rs desktop-ui/src/shared/types/tasks.ts
git commit -m "feat(tasks): add createdAt/updatedAt to TaskResponse"
```

---

### Task 2: Rename mock-data icon files to lib/

**Files:**
- Rename: `desktop-ui/src/features/tasks2/mock-data/status.tsx` → `desktop-ui/src/features/tasks2/lib/status-icons.tsx`
- Rename: `desktop-ui/src/features/tasks2/mock-data/priorities.tsx` → `desktop-ui/src/features/tasks2/lib/priority-icons.tsx`
- Modify: `desktop-ui/src/features/tasks2/lib/status-utils.tsx:2` (update import)

- [ ] **Step 1: Move status icons**

```bash
cd desktop-ui/src/features/tasks2
mv mock-data/status.tsx lib/status-icons.tsx
```

- [ ] **Step 2: Move priority icons**

```bash
mv mock-data/priorities.tsx lib/priority-icons.tsx
```

- [ ] **Step 3: Update status-utils.tsx import**

In `desktop-ui/src/features/tasks2/lib/status-utils.tsx`, change line 2:

```typescript
// Before:
import { StatusIcon } from "../mock-data/status";

// After:
import { StatusIcon } from "./status-icons";
```

- [ ] **Step 4: Verify no import errors**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: build succeeds (other files still import mock-data paths which haven't been touched yet, but we haven't deleted those files yet so they still exist)

Note: This step might show TypeScript errors from files still importing `../mock-data/status` or `../mock-data/priorities` — that's expected. Those will be updated in subsequent tasks. As long as the renamed files themselves are valid, proceed.

- [ ] **Step 5: Commit**

```bash
git add -A desktop-ui/src/features/tasks2/lib/ desktop-ui/src/features/tasks2/mock-data/
git commit -m "refactor(tasks2): rename status/priority icons from mock-data to lib"
```

---

### Task 3: Create presentation mappers

**Files:**
- Create: `desktop-ui/src/features/tasks2/lib/mappers.ts`

- [ ] **Step 1: Write mappers file**

Create `desktop-ui/src/features/tasks2/lib/mappers.ts`:

```typescript
import type { StatusLabel, TimelineEntry } from "@shared/types/common";
import type { Area, Project, Task } from "@shared/types/tasks";
import { Folder, type LucideProps } from "lucide-react";
import type React from "react";
import type { LabelInterface } from "./priority-icons";
import {
  HighPriorityIcon,
  LowPriorityIcon,
  MediumPriorityIcon,
  NoPriorityIcon,
  UrgentPriorityIcon,
  priorities,
  type Priority,
} from "./priority-icons";
import {
  BacklogIcon,
  CompletedIcon,
  InProgressIcon,
  PausedIcon,
  TechnicalReviewIcon,
  ToDoIcon,
  type Status,
  status as allStatusDefs,
} from "./status-icons";

// ── Display types ─────────────────────────────────────────

export interface DisplayProject {
  id: string;
  name: string;
  color: string;
  icon: React.FC<LucideProps>;
}

export type { Status, Priority, LabelInterface };

export type TaskState = "new" | "focused" | "has-history" | "completed";
export type ActorType = "user" | "agent" | "system";
export type SuggestionStatus = "pending" | "applied" | "dismissed";

export interface DetailTask {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  priority: Priority;
  labels: LabelInterface[];
  project: DisplayProject | null;
  area: { id: string; name: string } | null;
  tags: string[];
  dueDate: string | null;
  energyLevel: string | null;
  taskType: string;
  estimatedMinutes: number | null;
  actualMinutes: number | null;
  totalTrackedSecs: number;
  focusedAt: string | null;
  acceptanceCriteria: string | null;
  completed: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ActivityEntry {
  id: string;
  actorType: ActorType;
  actorName: string;
  action: string;
  detail: string | null;
  createdAt: string;
}

export interface Suggestion {
  id: string;
  title: string;
  description: string;
  confidence: number;
  status: SuggestionStatus;
}

export interface SubIssue {
  id: string;
  identifier: string;
  title: string;
  status: Status;
  priority: Priority;
  completed: boolean;
}

export interface TaskMemory {
  lastSessionSummary: string;
  continuityNote: string;
  relatedFacts: string[];
}

export interface FocusSession {
  focusMode: string;
  qualityScore: number;
  distractionCount: number;
  flowState: string;
  qualityHistory: number[];
}

/** The Issue shape consumed by list/board components. */
export interface Issue {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  assignee: null;
  priority: Priority;
  labels: LabelInterface[];
  createdAt: string;
  cycleId: string;
  project?: DisplayProject;
  subissues?: string[];
  rank: string;
  dueDate?: string;
}

// ── Status resolution (option C) ──────────────────────────

const KNOWN_STATUS_MAP: Record<string, Status> = {};
for (const s of allStatusDefs) {
  KNOWN_STATUS_MAP[s.name.toLowerCase()] = s;
}
// Add aliases
const STATUS_ALIASES: Record<string, string> = {
  "to do": "todo",
  "on hold": "paused",
  "in review": "technical review",
  done: "completed",
};

export function resolveStatus(task: Task): Status {
  // 1. Try statusLabel name match
  if (task.statusLabel) {
    const name = task.statusLabel.name.toLowerCase();
    const aliased = STATUS_ALIASES[name] ?? name;
    const match = KNOWN_STATUS_MAP[aliased];
    if (match) return match;

    // 2. Fallback to colored circle
    const found = allStatusDefs.find(
      (s) => s.color.toLowerCase() === task.statusLabel!.color.toLowerCase(),
    );
    if (found) return { ...found, name: task.statusLabel.name };

    // Use the first status def as template, override color/name
    return {
      id: task.statusLabel.id,
      name: task.statusLabel.name,
      color: task.statusLabel.color,
      icon: BacklogIcon, // generic fallback
    };
  }

  // 3. Map from task.status string
  switch (task.status) {
    case "completed":
      return allStatusDefs.find((s) => s.id === "completed") ?? allStatusDefs[0];
    case "in_progress":
      return allStatusDefs.find((s) => s.id === "in-progress") ?? allStatusDefs[0];
    case "open":
      return allStatusDefs.find((s) => s.id === "to-do") ?? allStatusDefs[0];
    default:
      return allStatusDefs.find((s) => s.id === "backlog") ?? allStatusDefs[0];
  }
}

// ── Priority resolution ───────────────────────────────────

const PRIORITY_MAP: Record<string, Priority> = {
  "": priorities[0], // no-priority
  P0: priorities[0], // no-priority
  P1: priorities[1], // urgent
  P2: priorities[2], // high
  P3: priorities[3], // medium
  P4: priorities[4], // low
};

export function resolvePriority(task: Task): Priority {
  if (!task.priority) return priorities[0]; // no-priority
  return PRIORITY_MAP[task.priority] ?? priorities[0];
}

export function priorityToNumber(priorityId: string): number | null {
  switch (priorityId) {
    case "urgent":
      return 1;
    case "high":
      return 2;
    case "medium":
      return 3;
    case "low":
      return 4;
    default:
      return null; // no-priority
  }
}

// ── Tags → Labels ─────────────────────────────────────────

const TAG_COLORS = [
  "purple", "red", "green", "blue", "yellow",
  "orange", "pink", "gray", "indigo", "teal", "cyan",
];

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash * 31 + str.charCodeAt(i)) | 0;
  }
  return Math.abs(hash);
}

export function tagsToLabels(tags: string[]): LabelInterface[] {
  return tags.map((tag) => ({
    id: tag,
    name: tag,
    color: TAG_COLORS[hashString(tag) % TAG_COLORS.length],
  }));
}

// ── Short ID ──────────────────────────────────────────────

export function shortId(id: string): string {
  const hex = id.replace(/-/g, "").slice(0, 4);
  return `T-${hex}`;
}

// ── Project mapping ───────────────────────────────────────

export function projectToDisplayProject(project: Project): DisplayProject {
  return {
    id: project.id,
    name: project.name,
    color: project.color,
    icon: Folder,
  };
}

// ── Task → Issue ──────────────────────────────────────────

export function taskToIssue(
  task: Task,
  projectMap: Map<string, DisplayProject>,
): Issue {
  return {
    id: task.id,
    identifier: shortId(task.id),
    title: task.title,
    description: task.description ?? "",
    status: resolveStatus(task),
    assignee: null,
    priority: resolvePriority(task),
    labels: tagsToLabels(task.tags),
    createdAt: task.createdAt ?? "",
    cycleId: "",
    project: task.projectId ? projectMap.get(task.projectId) : undefined,
    subissues: task.subtaskCount > 0 ? Array(task.subtaskCount).fill("") : undefined,
    rank: "",
    dueDate: task.dueDate ?? undefined,
  };
}

// ── Task → DetailTask ─────────────────────────────────────

export function taskToDetailTask(
  task: Task,
  projectMap: Map<string, DisplayProject>,
  areaMap: Map<string, Area>,
): DetailTask {
  const area = areaMap.get(task.areaId);
  return {
    id: task.id,
    identifier: shortId(task.id),
    title: task.title,
    description: task.description ?? "",
    status: resolveStatus(task),
    priority: resolvePriority(task),
    labels: tagsToLabels(task.tags),
    project: task.projectId ? (projectMap.get(task.projectId) ?? null) : null,
    area: area ? { id: area.id, name: area.name } : null,
    tags: task.tags,
    dueDate: task.dueDate ?? null,
    energyLevel: task.energyLevel ?? null,
    taskType: task.taskType ?? "manual",
    estimatedMinutes: task.estimatedMinutes ?? null,
    actualMinutes: task.actualMinutes ?? null,
    totalTrackedSecs: task.totalTrackedSecs ?? 0,
    focusedAt: task.focusedAt ?? null,
    acceptanceCriteria: task.acceptanceCriteria ?? null,
    completed: task.completed,
    createdAt: task.createdAt ?? "",
    updatedAt: task.updatedAt ?? "",
  };
}

// ── Child Task → SubIssue ─────────────────────────────────

export function taskToSubIssue(task: Task): SubIssue {
  return {
    id: task.id,
    identifier: shortId(task.id),
    title: task.title,
    status: resolveStatus(task),
    priority: resolvePriority(task),
    completed: task.completed,
  };
}

// ── Derive task state ─────────────────────────────────────

export function deriveTaskState(task: DetailTask): TaskState {
  if (task.completed) return "completed";
  if (task.focusedAt) return "focused";
  if (task.totalTrackedSecs > 0) return "has-history";
  return "new";
}

// ── Timeline → Activity ───────────────────────────────────
// Uses TimelineEntry from @shared/types/common (imported at top of file)

const ENTRY_TYPE_ACTIONS: Record<string, string> = {
  taskCreated: "created task",
  taskCompleted: "completed task",
  taskUpdated: "updated task",
  taskDue: "task due",
  taskTimeEntry: "tracked time",
  focusSession: "focus session",
};

export function timelineToActivity(entry: TimelineEntry): ActivityEntry {
  return {
    id: entry.id,
    actorType: "system",
    actorName: "System",
    action: ENTRY_TYPE_ACTIONS[entry.entryType] ?? entry.entryType,
    detail: entry.description ?? entry.title,
    createdAt: entry.startedAt,
  };
}

// ── Derive focus session ──────────────────────────────────

export function deriveFocusSession(task: DetailTask): FocusSession | null {
  if (!task.focusedAt) return null;
  return {
    focusMode: "focus",
    qualityScore: 0.7,
    distractionCount: 0,
    flowState: "building",
    qualityHistory: [0.5, 0.6, 0.7],
  };
}

// ── Filter logic (moved from issues-store) ────────────────

interface FilterState {
  status: string[];
  assignee: string[];
  priority: string[];
  labels: string[];
  project: string[];
}

export function filterIssues(issues: Issue[], filters: FilterState): Issue[] {
  let result = issues;

  if (filters.status.length > 0) {
    result = result.filter((issue) => filters.status.includes(issue.status.id));
  }
  if (filters.priority.length > 0) {
    result = result.filter((issue) => filters.priority.includes(issue.priority.id));
  }
  if (filters.labels.length > 0) {
    result = result.filter((issue) =>
      issue.labels.some((l) => filters.labels.includes(l.id)),
    );
  }
  if (filters.project.length > 0) {
    result = result.filter(
      (issue) => issue.project && filters.project.includes(issue.project.id),
    );
  }

  return result;
}

export function searchIssues(issues: Issue[], query: string): Issue[] {
  const q = query.toLowerCase().trim();
  if (!q) return issues;
  return issues.filter(
    (issue) =>
      issue.title.toLowerCase().includes(q) ||
      issue.description.toLowerCase().includes(q) ||
      issue.identifier.toLowerCase().includes(q),
  );
}

// ── Group by status ───────────────────────────────────────

export function groupIssuesByStatus(issueList: Issue[]): Record<string, Issue[]> {
  return issueList.reduce(
    (acc, issue) => {
      const statusId = issue.status.id;
      if (!acc[statusId]) acc[statusId] = [];
      acc[statusId].push(issue);
      return acc;
    },
    {} as Record<string, Issue[]>,
  );
}

export function sortIssuesByPriority(issueList: Issue[]): Issue[] {
  const order: Record<string, number> = {
    urgent: 0, high: 1, medium: 2, low: 3, "no-priority": 4,
  };
  return [...issueList].sort(
    (a, b) => (order[a.priority.id] ?? 99) - (order[b.priority.id] ?? 99),
  );
}
```

- [ ] **Step 2: Fix LabelInterface import**

The `LabelInterface` type is currently defined in `mock-data/labels.ts`. Since we're keeping this type but deleting the mock data, we need to define it in `priority-icons.tsx` (which already uses it conceptually) or in mappers itself. Check if `priority-icons.tsx` already exports it — if not, add the interface to mappers.ts directly.

Actually, looking at the code: `LabelInterface` is defined in `mock-data/labels.ts` as `{ id: string; name: string; color: string }`. Since we're deleting that file, define it in `mappers.ts`:

```typescript
export interface LabelInterface {
  id: string;
  name: string;
  color: string;
}
```

Remove the import from `priority-icons` and keep it in mappers.

- [ ] **Step 3: Verify TypeScript compilation**

Run: `cd desktop-ui && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: the new file compiles. Other files may have errors from still importing mock data — that's expected.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/mappers.ts
git commit -m "feat(tasks2): add presentation mappers for real data integration"
```

---

### Task 4: Create useTasks hook

**Files:**
- Create: `desktop-ui/src/features/tasks2/hooks/useTasks.ts`

- [ ] **Step 1: Write the hook**

Create `desktop-ui/src/features/tasks2/hooks/useTasks.ts`:

```typescript
import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { ApiError } from "@shared/types/common";
import type { Area, Project, Task, TaskCreateParams, TaskUpdateParams } from "@shared/types/tasks";
import { useCallback, useMemo } from "react";
import {
  type DisplayProject,
  type Issue,
  filterIssues,
  projectToDisplayProject,
  searchIssues,
  taskToIssue,
} from "../lib/mappers";
import { useFilterStore } from "../store/filter-store";
import { useSearchStore } from "../store/search-store";

export interface UseTasksResult {
  issues: Issue[];
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
  updateTask: (params: TaskUpdateParams) => Promise<void>;
  createTask: (params: TaskCreateParams) => Promise<void>;
  deleteTask: (id: string) => Promise<void>;
  toggleComplete: (id: string) => Promise<void>;
  projectMap: Map<string, DisplayProject>;
  areaMap: Map<string, Area>;
  areas: Area[];
  projects: Project[];
}

export function useTasks(): UseTasksResult {
  const {
    data: tasks,
    loading,
    error,
    refetch: refetchTasks,
  } = useQuery<Task[]>("task_list", undefined, []);
  const { data: projects, refetch: refetchProjects } = useQuery<Project[]>(
    "project_list",
    undefined,
    [],
  );
  const { data: areas, refetch: refetchAreas } = useQuery<Area[]>(
    "area_list",
    undefined,
    [],
  );

  const updateMutation = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const createMutation = useMutation<Task, TaskCreateParams>("task_create", "params");
  const deleteMutation = useMutation<void, { id: string }>("task_delete");
  const toggleMutation = useMutation<Task, { id: string }>("task_toggle_complete");

  // Build lookup maps
  const projectMap = useMemo(() => {
    const map = new Map<string, DisplayProject>();
    for (const p of projects) {
      map.set(p.id, projectToDisplayProject(p));
    }
    return map;
  }, [projects]);

  const areaMap = useMemo(
    () => new Map(areas.map((a) => [a.id, a])),
    [areas],
  );

  // Map tasks → issues
  const allIssues = useMemo(
    () => tasks.map((t) => taskToIssue(t, projectMap)),
    [tasks, projectMap],
  );

  // Note: filtering is NOT done here — components apply their own filters
  // via filterIssues() and searchIssues() from mappers.ts.
  // This hook returns ALL mapped issues.

  // Auto-refresh on entity updates
  useEvent<{ entityKind: string; id: string }>("entity:updated", (payload) => {
    const kind = payload?.entityKind;
    if (!kind) {
      refetchTasks();
      refetchProjects();
      refetchAreas();
      return;
    }
    if (kind === "task" || kind === "area") refetchTasks();
    if (kind === "project") refetchProjects();
    if (kind === "area") refetchAreas();
  });

  // Mutation wrappers
  const updateTask = useCallback(
    async (params: TaskUpdateParams) => {
      await updateMutation.mutate(params);
      refetchTasks();
    },
    [updateMutation, refetchTasks],
  );

  const createTask = useCallback(
    async (params: TaskCreateParams) => {
      await createMutation.mutate(params);
      refetchTasks();
    },
    [createMutation, refetchTasks],
  );

  const deleteTask = useCallback(
    async (id: string) => {
      await deleteMutation.mutate({ id });
      refetchTasks();
    },
    [deleteMutation, refetchTasks],
  );

  const toggleComplete = useCallback(
    async (id: string) => {
      await toggleMutation.mutate({ id });
      refetchTasks();
    },
    [toggleMutation, refetchTasks],
  );

  return {
    issues: allIssues,
    loading,
    error,
    refetch: refetchTasks,
    updateTask,
    createTask,
    deleteTask,
    toggleComplete,
    projectMap,
    areaMap,
    areas,
    projects,
  };
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/hooks/useTasks.ts
git commit -m "feat(tasks2): add useTasks hook for real data fetching"
```

---

### Task 5: Rewrite useIssueDetail hook

**Files:**
- Rewrite: `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`

- [ ] **Step 1: Rewrite the hook**

Replace the entire contents of `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`:

```typescript
import { useQuery } from "@shared/hooks/useQuery";
import { useMutation } from "@shared/hooks/useMutation";
import type { Area, Project, Task, TaskUpdateParams } from "@shared/types/tasks";
import type { TimelineResponse } from "@shared/types/common";
import { useCallback, useMemo } from "react";
import {
  type ActivityEntry,
  type DetailTask,
  type DisplayProject,
  type FocusSession,
  type Priority,
  type Status,
  type SubIssue,
  type Suggestion,
  type TaskMemory,
  type TaskState,
  deriveFocusSession,
  deriveTaskState,
  priorityToNumber,
  projectToDisplayProject,
  taskToDetailTask,
  taskToSubIssue,
  timelineToActivity,
} from "../lib/mappers";

export type { DetailTask, TaskState, SubIssue, Suggestion, FocusSession, TaskMemory, ActivityEntry };

export function useIssueDetail(
  issueId: string,
  projectMap: Map<string, DisplayProject>,
  areaMap: Map<string, Area>,
) {
  // Core task data
  const { data: rawTask } = useQuery<Task | null>("task_get", { id: issueId });

  const task: DetailTask | null = useMemo(
    () => (rawTask ? taskToDetailTask(rawTask, projectMap, areaMap) : null),
    [rawTask, projectMap, areaMap],
  );

  const taskState: TaskState = useMemo(
    () => (task ? deriveTaskState(task) : "new"),
    [task],
  );

  // Sub-issues
  const { data: rawChildren } = useQuery<Task[]>(
    "task_list_children",
    { parentId: issueId },
    [],
  );

  const subIssues: SubIssue[] = useMemo(
    () => (rawChildren ?? []).map(taskToSubIssue),
    [rawChildren],
  );

  // Activity from timeline (client-side filtered by entityId)
  const { data: timeline } = useQuery<TimelineResponse>(
    "timeline_query",
    {
      startDate: "2020-01-01T00:00:00Z",
      endDate: new Date().toISOString(),
      sources: ["task"],
    },
  );

  const activity: ActivityEntry[] = useMemo(
    () =>
      (timeline?.entries ?? [])
        .filter((e) => e.entityId === issueId)
        .map(timelineToActivity),
    [timeline, issueId],
  );

  // Mutations
  const updateMutation = useMutation<Task, TaskUpdateParams>("task_update", "params");

  const updateTask = useCallback(
    <K extends keyof DetailTask>(field: K, value: DetailTask[K]) => {
      if (!task) return;
      // Map field updates to TaskUpdateParams
      const params: TaskUpdateParams = { id: task.id };
      switch (field) {
        case "title":
          params.title = value as string;
          break;
        case "description":
          params.description = value as string;
          break;
        case "status": {
          const s = value as Status;
          params.status = s.id;
          break;
        }
        case "priority": {
          const p = value as Priority;
          params.priority = priorityToNumber(p.id);
          break;
        }
        case "energyLevel":
          params.energyLevel = value as TaskUpdateParams["energyLevel"];
          break;
        case "taskType":
          params.taskType = value as TaskUpdateParams["taskType"];
          break;
        case "acceptanceCriteria":
          params.acceptanceCriteria = value as string | null;
          break;
        default:
          return; // Ignore unmapped fields
      }
      updateMutation.mutate(params);
    },
    [task, updateMutation],
  );

  // Suggestions: empty (no backend yet)
  const suggestions: Suggestion[] = [];

  // Memory: null (no backend bridge yet)
  const taskMemory: TaskMemory | null = null;

  // Focus session
  const focusSession: FocusSession | null = useMemo(
    () => (task ? deriveFocusSession(task) : null),
    [task],
  );

  const dismissSuggestion = useCallback((_id: string) => {}, []);
  const applySuggestion = useCallback((_id: string) => {}, []);

  return {
    task: task ?? createPlaceholderTask(issueId),
    taskState,
    activity,
    suggestions,
    focusSession,
    subIssues,
    taskMemory,
    updateTask,
    dismissSuggestion,
    applySuggestion,
  };
}

function createPlaceholderTask(id: string): DetailTask {
  return {
    id,
    identifier: "",
    title: "Loading...",
    description: "",
    status: { id: "backlog", name: "Backlog", color: "#bec2c8", icon: () => null },
    priority: { id: "no-priority", name: "No priority", icon: () => null },
    labels: [],
    project: null,
    area: null,
    tags: [],
    dueDate: null,
    energyLevel: null,
    taskType: "manual",
    estimatedMinutes: null,
    actualMinutes: null,
    totalTrackedSecs: 0,
    focusedAt: null,
    acceptanceCriteria: null,
    completed: false,
    createdAt: "",
    updatedAt: "",
  };
}
```

- [ ] **Step 2: Delete old test file**

```bash
rm desktop-ui/src/features/tasks2/hooks/__tests__/useIssueDetail.test.ts
```

The old test tested against mock data and is no longer valid.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts
git add desktop-ui/src/features/tasks2/hooks/__tests__/
git commit -m "feat(tasks2): rewrite useIssueDetail with real data queries"
```

---

### Task 6: Update tab-store to use real areas

**Files:**
- Modify: `desktop-ui/src/features/tasks2/store/tab-store.ts`

- [ ] **Step 1: Rewrite tab-store initialization**

Replace the entire contents of `desktop-ui/src/features/tasks2/store/tab-store.ts`:

```typescript
import { arrayMove } from "@dnd-kit/sortable";
import { create } from "zustand";

export interface NavEntry {
  type: "my-issues" | "all-issues" | "area" | "project" | "issue";
  targetId: string;
  label: string;
}

export interface Tab {
  id: string;
  navStack: NavEntry[];
}

interface TabState {
  tabs: Tab[];
  activeTabId: string;
  initialized: boolean;

  initFromAreas: (areas: { id: string; name: string }[]) => void;
  openTab: (type: NavEntry["type"], targetId: string, label: string) => void;
  closeTab: (tabId: string) => void;
  closeOthers: (tabId: string) => void;
  closeToRight: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  navigateInPlace: (type: NavEntry["type"], targetId: string, label: string) => void;
  navigateToStackIndex: (index: number) => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
}

let idCounter = 0;
function nextId() {
  return `tab-${++idCounter}`;
}

const defaultTab: Tab = {
  id: nextId(),
  navStack: [{ type: "my-issues", targetId: "my-issues", label: "My Issues" }],
};

export const useTabStore = create<TabState>((set, get) => ({
  tabs: [defaultTab],
  activeTabId: defaultTab.id,
  initialized: false,

  initFromAreas: (areas) => {
    if (get().initialized) return;
    const myIssuesTab: Tab = {
      id: nextId(),
      navStack: [{ type: "my-issues", targetId: "my-issues", label: "My Issues" }],
    };
    const areaTabs: Tab[] = areas.map((area) => ({
      id: nextId(),
      navStack: [{ type: "area" as const, targetId: area.id, label: area.name }],
    }));
    const tabs = [myIssuesTab, ...areaTabs];
    set({ tabs, activeTabId: tabs[0]?.id ?? "", initialized: true });
  },

  openTab: (type, targetId, label) => {
    const { tabs, activeTabId } = get();
    const existing = tabs.find(
      (t) =>
        t.navStack.length > 0 &&
        t.navStack[0].type === type &&
        t.navStack[0].targetId === targetId,
    );
    if (existing) {
      set({ activeTabId: existing.id });
      return;
    }
    const newTab: Tab = {
      id: nextId(),
      navStack: [{ type, targetId, label }],
    };
    const activeIndex = tabs.findIndex((t) => t.id === activeTabId);
    const insertIndex = activeIndex >= 0 ? activeIndex + 1 : tabs.length;
    const newTabs = [...tabs.slice(0, insertIndex), newTab, ...tabs.slice(insertIndex)];
    set({ tabs: newTabs, activeTabId: newTab.id });
  },

  closeTab: (tabId) => {
    const { tabs, activeTabId } = get();
    const index = tabs.findIndex((t) => t.id === tabId);
    if (index === -1) return;
    const newTabs = tabs.filter((t) => t.id !== tabId);
    if (newTabs.length === 0) {
      set({ tabs: [], activeTabId: "" });
      return;
    }
    let newActiveId = activeTabId;
    if (activeTabId === tabId) {
      const newIndex = Math.min(index, newTabs.length - 1);
      newActiveId = newTabs[newIndex].id;
    }
    set({ tabs: newTabs, activeTabId: newActiveId });
  },

  closeOthers: (tabId) => {
    const { tabs } = get();
    const kept = tabs.find((t) => t.id === tabId);
    if (!kept) return;
    set({ tabs: [kept], activeTabId: tabId });
  },

  closeToRight: (tabId) => {
    const { tabs, activeTabId } = get();
    const index = tabs.findIndex((t) => t.id === tabId);
    if (index === -1) return;
    const newTabs = tabs.slice(0, index + 1);
    if (newTabs.length === 0) {
      set({ tabs: [], activeTabId: "" });
      return;
    }
    const newActive = newTabs.find((t) => t.id === activeTabId)
      ? activeTabId
      : newTabs[newTabs.length - 1].id;
    set({ tabs: newTabs, activeTabId: newActive });
  },

  setActiveTab: (tabId) => {
    set({ activeTabId: tabId });
  },

  navigateInPlace: (type, targetId, label) => {
    const { tabs, activeTabId } = get();
    const idx = tabs.findIndex((t) => t.id === activeTabId);
    if (idx === -1) return;
    const updated = [...tabs];
    updated[idx] = {
      ...tabs[idx],
      navStack: [...tabs[idx].navStack, { type, targetId, label }],
    };
    set({ tabs: updated });
  },

  navigateToStackIndex: (index) => {
    const { tabs, activeTabId } = get();
    const idx = tabs.findIndex((t) => t.id === activeTabId);
    if (idx === -1) return;
    if (index < 0 || index >= tabs[idx].navStack.length) return;
    const updated = [...tabs];
    updated[idx] = { ...tabs[idx], navStack: tabs[idx].navStack.slice(0, index + 1) };
    set({ tabs: updated });
  },

  reorderTabs: (fromIndex, toIndex) => {
    const { tabs } = get();
    if (fromIndex < 0 || fromIndex >= tabs.length || toIndex < 0 || toIndex >= tabs.length)
      return;
    set({ tabs: arrayMove(tabs, fromIndex, toIndex) });
  },
}));
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/store/tab-store.ts
git commit -m "feat(tasks2): update tab-store to initialize from real area data"
```

---

## Chunk 2: Wire List/Board Components

### Task 7: Update Tasks2Page as data root

**Files:**
- Modify: `desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx`

- [ ] **Step 1: Add data fetching and pass down**

Replace `desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx`:

```typescript
import { useEffect } from "react";
import "../tasks2.css";
import { CreateIssueModal } from "../components/CreateIssueModal";
import { PortalContainerProvider } from "../components/portal-context";
import { TabBar } from "../components/TabBar";
import { TabContent } from "../components/TabContent";
import { Tasks2Layout } from "../components/Tasks2Layout";
import { useTasks } from "../hooks/useTasks";
import { useTabStore } from "../store/tab-store";

export function Tasks2Page() {
  const tasksData = useTasks();
  const initFromAreas = useTabStore((s) => s.initFromAreas);

  // Initialize tabs from real areas once loaded
  useEffect(() => {
    if (tasksData.areas.length > 0) {
      initFromAreas(tasksData.areas);
    }
  }, [tasksData.areas, initFromAreas]);

  return (
    <PortalContainerProvider>
      <div className="tasks2-scope flex-1 h-full min-w-0">
        <Tasks2Layout>
          <TabBar />
          <TabContent tasksData={tasksData} />
        </Tasks2Layout>
        <CreateIssueModal
          onCreateTask={tasksData.createTask}
          areas={tasksData.areas}
        />
      </div>
    </PortalContainerProvider>
  );
}
```

Note: This changes the props of `TabContent` and `CreateIssueModal`. Those will be updated in subsequent steps.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx
git commit -m "feat(tasks2): wire Tasks2Page as data root with useTasks"
```

---

### Task 8: Update TabContent to pass data through

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/TabContent.tsx`

- [ ] **Step 1: Read current TabContent**

Read `desktop-ui/src/features/tasks2/components/TabContent.tsx` to see the current routing logic.

- [ ] **Step 2: Update TabContent to accept and pass tasksData**

Update the component to accept `tasksData` prop and pass it to child views. The routing logic stays the same, but views now receive `tasksData` instead of using `useIssuesStore`.

The `IssueDetailView` will need `projectMap` and `areaMap` from `tasksData` to pass to `useIssueDetail`. Add these as props.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/TabContent.tsx
git commit -m "feat(tasks2): pass tasksData through TabContent routing"
```

---

### Task 9: Update AllIssues to use useTasks

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/AllIssues.tsx`

- [ ] **Step 1: Replace store with props**

Update `desktop-ui/src/features/tasks2/components/AllIssues.tsx`:

```typescript
import { useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { filterIssues } from "../lib/mappers";
import { useFilterStore } from "../store/filter-store";
import { useSearchStore } from "../store/search-store";
import { IssueBoard } from "./IssueBoard";
import { SearchIssues } from "./SearchIssues";

interface AllIssuesProps {
  tasksData: UseTasksResult;
}

export default function AllIssues({ tasksData }: AllIssuesProps) {
  const { isSearchOpen, searchQuery } = useSearchStore();
  const filters = useFilterStore((s) => s.filters);
  const isFiltered = useFilterStore((s) => s.hasActiveFilters());

  const displayIssues = useMemo(
    () => (isFiltered ? filterIssues(tasksData.issues, filters) : tasksData.issues),
    [isFiltered, filters, tasksData.issues],
  );

  if (isSearchOpen) {
    if (searchQuery.trim()) {
      return <SearchIssues issues={tasksData.issues} />;
    }
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        Search results will appear here
      </div>
    );
  }

  return <IssueBoard issues={displayIssues} />;
}
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AllIssues.tsx
git commit -m "feat(tasks2): wire AllIssues to useTasks data"
```

---

### Task 10: Update AreaView, ProjectView, SearchIssues

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/AreaView.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/ProjectView.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/SearchIssues.tsx`

- [ ] **Step 1: Update AreaView**

Replace `desktop-ui/src/features/tasks2/components/AreaView.tsx`:

```typescript
import { Folder } from "lucide-react";
import { useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { useTabStore } from "../store/tab-store";

interface AreaViewProps {
  areaId: string;
  tasksData: UseTasksResult;
}

export function AreaView({ areaId, tasksData }: AreaViewProps) {
  const area = tasksData.areaMap.get(areaId);
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);

  // Get projects belonging to this area
  const areaProjects = useMemo(
    () => tasksData.projects.filter((p) => p.areaId === areaId),
    [tasksData.projects, areaId],
  );

  // Count issues per project
  const projectIssueCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const issue of tasksData.issues) {
      if (issue.project) {
        counts[issue.project.id] = (counts[issue.project.id] ?? 0) + 1;
      }
    }
    return counts;
  }, [tasksData.issues]);

  if (!area) {
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        Area not found
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {areaProjects.map((project) => {
        const displayProject = tasksData.projectMap.get(project.id);
        const Icon = displayProject?.icon ?? Folder;
        const count = projectIssueCounts[project.id] ?? 0;
        return (
          <button
            key={project.id}
            type="button"
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey) {
                useTabStore.getState().openTab("project", project.id, project.name);
              } else {
                navigateInPlace("project", project.id, project.name);
              }
            }}
            className="flex items-center gap-3 px-4 py-3 text-left hover:bg-[hsl(var(--accent))] transition-colors border-b border-[hsl(var(--border))]"
          >
            <Icon className="h-4 w-4 text-[hsl(var(--muted-foreground))]" />
            <span className="text-sm text-[hsl(var(--foreground))] flex-1">
              {project.name}
            </span>
            <span className="text-xs text-[hsl(var(--muted-foreground))]">
              {count} issues
            </span>
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Update ProjectView**

Replace `desktop-ui/src/features/tasks2/components/ProjectView.tsx`:

```typescript
import { useMemo } from "react";
import type { UseTasksResult } from "../hooks/useTasks";
import { IssueBoard } from "./IssueBoard";

interface ProjectViewProps {
  projectId: string;
  tasksData: UseTasksResult;
}

export function ProjectView({ projectId, tasksData }: ProjectViewProps) {
  const projectIssues = useMemo(
    () => tasksData.issues.filter((issue) => issue.project?.id === projectId),
    [tasksData.issues, projectId],
  );

  if (projectIssues.length === 0) {
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        No issues in this project
      </div>
    );
  }

  return <IssueBoard issues={projectIssues} />;
}
```

- [ ] **Step 3: Update SearchIssues**

Replace `desktop-ui/src/features/tasks2/components/SearchIssues.tsx`:

```typescript
import { useMemo } from "react";
import type { Issue } from "../lib/mappers";
import { searchIssues } from "../lib/mappers";
import { useSearchStore } from "../store/search-store";
import { IssueLine } from "./IssueLine";

interface SearchIssuesProps {
  issues: Issue[];
}

export function SearchIssues({ issues }: SearchIssuesProps) {
  const searchQuery = useSearchStore((s) => s.searchQuery);

  const searchResults = useMemo(
    () => searchIssues(issues, searchQuery),
    [issues, searchQuery],
  );

  return (
    <div className="w-full">
      {searchQuery.trim() !== "" && (
        <div>
          {searchResults.length > 0 ? (
            <div className="border border-[hsl(var(--border))] rounded-md mt-4">
              <div className="py-2 px-4 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))]/50">
                <h3 className="text-sm font-medium">Results ({searchResults.length})</h3>
              </div>
              <div className="divide-y divide-[hsl(var(--border))]">
                {searchResults.map((issue) => (
                  <IssueLine key={issue.id} issue={issue} />
                ))}
              </div>
            </div>
          ) : (
            <div className="text-center py-8 text-[hsl(var(--muted-foreground))]">
              No results found for &quot;{searchQuery}&quot;
            </div>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AreaView.tsx desktop-ui/src/features/tasks2/components/ProjectView.tsx desktop-ui/src/features/tasks2/components/SearchIssues.tsx
git commit -m "feat(tasks2): wire AreaView, ProjectView, SearchIssues to real data"
```

---

### Task 11: Update AddTabMenu

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/AddTabMenu.tsx`

- [ ] **Step 1: Replace mock imports with props**

Update `desktop-ui/src/features/tasks2/components/AddTabMenu.tsx` to receive `areas` and `projects` as props from the parent instead of importing from mock-data:

```typescript
import type { Area, Project } from "@shared/types/tasks";
import { Plus } from "lucide-react";
import { useState } from "react";
import { useTabStore } from "../store/tab-store";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface AddTabMenuProps {
  areas: Area[];
  projects: Project[];
}

const menuItemCls =
  "w-full text-left px-2 py-1.5 text-[13px] rounded-sm hover:bg-[hsl(var(--accent))] text-[hsl(var(--foreground))] transition-colors";

export function AddTabMenu({ areas, projects }: AddTabMenuProps) {
  const openTab = useTabStore((s) => s.openTab);
  const [open, setOpen] = useState(false);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center w-[26px] h-[26px] rounded-md text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))] transition-colors flex-shrink-0"
        >
          <Plus className="h-4 w-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-56 p-2">
        <button
          type="button"
          onClick={() => {
            openTab("all-issues", "all-issues", "All Issues");
            setOpen(false);
          }}
          className={`${menuItemCls} font-medium`}
        >
          All Issues
        </button>
        <div className="h-px bg-[hsl(var(--border))] my-1.5" />
        <div className="text-[11px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider px-2 py-1">
          Areas
        </div>
        {areas.map((area) => (
          <button
            key={area.id}
            type="button"
            onClick={() => {
              openTab("area", area.id, area.name);
              setOpen(false);
            }}
            className={menuItemCls}
          >
            {area.name}
          </button>
        ))}
        <div className="h-px bg-[hsl(var(--border))] my-1.5" />
        <div className="text-[11px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider px-2 py-1">
          Projects
        </div>
        {projects.map((project) => (
          <button
            key={project.id}
            type="button"
            onClick={() => {
              openTab("project", project.id, project.name);
              setOpen(false);
            }}
            className={menuItemCls}
          >
            {project.name}
          </button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
```

Note: The parent `TabBar` component will need to pass `areas` and `projects` down. This requires updating `TabBar` to accept these props (either from context or threaded from `Tasks2Page`). The simplest approach is to have `TabBar` accept them as props and thread from `Tasks2Page`.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AddTabMenu.tsx
git commit -m "feat(tasks2): replace mock data imports in AddTabMenu with props"
```

---

## Chunk 3: Wire Mutation Components

### Task 12: Update StatusSelector and PrioritySelector

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/StatusSelector.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/PrioritySelector.tsx`

- [ ] **Step 1: Update StatusSelector**

Replace `desktop-ui/src/features/tasks2/components/StatusSelector.tsx`:

```typescript
import { useMutation } from "@shared/hooks/useMutation";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import { Check } from "lucide-react";
import { useState } from "react";
import { renderStatusIcon } from "../lib/status-utils";
import { cn } from "../lib/utils";
import type { Status } from "../lib/mappers";
import { status as allStatus } from "../lib/status-icons";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "./ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface StatusSelectorProps {
  issueId: string;
  status: Status;
  onStatusChange?: () => void;
}

export function StatusSelector({ issueId, status, onStatusChange }: StatusSelectorProps) {
  const [open, setOpen] = useState(false);
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center size-5 rounded hover:bg-[hsl(var(--accent))] transition-colors"
          aria-label={`Status: ${status.name}`}
        >
          {renderStatusIcon(status.id)}
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" align="start">
        <Command>
          <CommandInput placeholder="Set status..." />
          <CommandList>
            <CommandEmpty>No status found.</CommandEmpty>
            <CommandGroup>
              {allStatus.map((s) => (
                <CommandItem
                  key={s.id}
                  value={s.name}
                  onSelect={() => {
                    updateTask.mutate({ id: issueId, status: s.id });
                    onStatusChange?.();
                    setOpen(false);
                  }}
                >
                  <span className="mr-2 flex items-center">{renderStatusIcon(s.id)}</span>
                  {s.name}
                  <Check
                    className={cn(
                      "ml-auto h-4 w-4",
                      status.id === s.id ? "opacity-100" : "opacity-0",
                    )}
                  />
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
```

- [ ] **Step 2: Update PrioritySelector**

Replace `desktop-ui/src/features/tasks2/components/PrioritySelector.tsx`:

```typescript
import { useMutation } from "@shared/hooks/useMutation";
import type { Task, TaskUpdateParams } from "@shared/types/tasks";
import { Check } from "lucide-react";
import { useState } from "react";
import { priorityToNumber, type Priority } from "../lib/mappers";
import { cn } from "../lib/utils";
import { priorities } from "../lib/priority-icons";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "./ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface PrioritySelectorProps {
  issueId: string;
  priority: Priority;
  onPriorityChange?: () => void;
}

export function PrioritySelector({ issueId, priority, onPriorityChange }: PrioritySelectorProps) {
  const [open, setOpen] = useState(false);
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");

  const PriorityIcon = priority.icon;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center justify-center size-5 rounded hover:bg-[hsl(var(--accent))] transition-colors text-[hsl(var(--muted-foreground))]"
          aria-label={`Priority: ${priority.name}`}
        >
          <PriorityIcon className="size-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" align="start">
        <Command>
          <CommandInput placeholder="Set priority..." />
          <CommandList>
            <CommandEmpty>No priority found.</CommandEmpty>
            <CommandGroup>
              {priorities.map((p) => {
                const Icon = p.icon;
                return (
                  <CommandItem
                    key={p.id}
                    value={p.name}
                    onSelect={() => {
                      updateTask.mutate({
                        id: issueId,
                        priority: priorityToNumber(p.id),
                      });
                      onPriorityChange?.();
                      setOpen(false);
                    }}
                  >
                    <Icon className="mr-2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
                    {p.name}
                    <Check
                      className={cn(
                        "ml-auto h-4 w-4",
                        priority.id === p.id ? "opacity-100" : "opacity-0",
                      )}
                    />
                  </CommandItem>
                );
              })}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/StatusSelector.tsx desktop-ui/src/features/tasks2/components/PrioritySelector.tsx
git commit -m "feat(tasks2): wire StatusSelector and PrioritySelector to task_update"
```

---

### Task 13: Update CreateIssueModal and IssueContextMenu

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx`

- [ ] **Step 1: Rewrite CreateIssueModal**

Simplify to use `task_create` mutation. Remove assignee/label sections (single-user app, tags can be added later). Keep status + priority selection using the icon components:

Replace `desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx` with a version that:
- Accepts `onCreateTask` and `areas` as props instead of using `useIssuesStore`
- Uses `priorities` from `lib/priority-icons` instead of mock
- Uses `status` from `lib/status-icons` instead of mock
- Calls `onCreateTask({ title, areaId, priority: priorityToNumber(p.id) })` on submit
- Removes assignee picker (single-user app)
- Removes label picker (tags can be set after creation)

- [ ] **Step 2: Rewrite IssueContextMenu**

Replace `desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx` with a version that:
- Accepts `onUpdateTask` and `onDeleteTask` callbacks as props instead of using `useIssuesStore`
- Uses `priorities` from `lib/priority-icons` instead of mock
- Uses `status` from `lib/status-icons` instead of mock
- Uses `tasksData.projects` passed as props instead of mock projects
- Removes assignee submenu (single-user app)
- Removes label submenu (tags managed elsewhere)
- Calls `onUpdateTask({ id, priority: priorityToNumber(p.id) })` for priority changes
- Calls `onUpdateTask({ id, status: s.id })` for status changes
- Calls `onUpdateTask({ id, projectId: project.id })` for project changes
- Calls `onDeleteTask(issue.id)` for delete

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx
git commit -m "feat(tasks2): wire CreateIssueModal and IssueContextMenu to real mutations"
```

---

## Chunk 4: Fix Passthrough Component Imports

### Task 13.5: Update all remaining mock-data imports

Many "passthrough" components import types or values from mock-data files. These must be redirected to `lib/mappers.ts`, `lib/status-icons.tsx`, or `lib/priority-icons.tsx` before mock files are deleted.

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/IssueBoard.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/GroupIssues.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/IssueLine.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/IssueGrid.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/Filter.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/LabelBadge.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/ProjectBadge.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/AssigneeUser.tsx`
- Modify: `desktop-ui/src/features/tasks2/store/create-issue-store.ts`
- Modify: `desktop-ui/src/features/tasks2/components/TabBar.tsx`

- [ ] **Step 1: Update type-only imports (IssueLine, IssueGrid, LabelBadge, ProjectBadge, AssigneeUser)**

These files only import types. Change import paths:

```typescript
// IssueLine.tsx — change:
import type { Issue } from "../mock-data/issues";
// to:
import type { Issue } from "../lib/mappers";

// IssueGrid.tsx — same change
import type { Issue } from "../lib/mappers";

// LabelBadge.tsx — change:
import type { LabelInterface } from "../mock-data/labels";
// to:
import type { LabelInterface } from "../lib/mappers";

// ProjectBadge.tsx — change:
import type { Project } from "../mock-data/projects";
// to:
import type { DisplayProject } from "../lib/mappers";
// Also update the prop type and component usage:
// { project: Project } → { project: DisplayProject }

// AssigneeUser.tsx — the User type is only used for the assignee display.
// Since single-user app has no assignees, this component still works but needs
// a local type definition or import removal. Simplest: define User inline:
// interface User { id: string; name: string; avatarUrl: string; }
// Or keep the component as-is since Issue.assignee is always null.
```

- [ ] **Step 2: Update IssueBoard.tsx**

Replace mock imports:

```typescript
// Before:
import type { Issue } from "../mock-data/issues";
import { groupIssuesByStatus } from "../mock-data/issues";
import { status as allStatus } from "../mock-data/status";
import { useIssuesStore } from "../store/issues-store";

// After:
import type { Issue } from "../lib/mappers";
import { groupIssuesByStatus } from "../lib/mappers";
import { status as allStatus } from "../lib/status-icons";
```

Remove `useIssuesStore` usage. The `handleDragEnd` currently calls `updateIssueStatus(issueId, targetStatus)` — replace with a prop callback:

```typescript
interface IssueBoardProps {
  issues: Issue[];
  onUpdateStatus?: (issueId: string, statusId: string) => void;
}
```

In `handleDragEnd`, call `onUpdateStatus?.(issueId, targetStatus.id)` instead of `updateIssueStatus`. Parent (`AllIssues`) passes `tasksData.updateTask` wrapped appropriately.

- [ ] **Step 3: Update GroupIssues.tsx**

```typescript
// Before:
import type { Issue } from "../mock-data/issues";
import { sortIssuesByPriority } from "../mock-data/issues";
import type { Status } from "../mock-data/status";

// After:
import type { Issue, Status } from "../lib/mappers";
import { sortIssuesByPriority } from "../lib/mappers";
```

Also update `create-issue-store.ts`:
```typescript
// Before:
import type { Status } from "../mock-data/status";
// After:
import type { Status } from "../lib/mappers";
```

- [ ] **Step 4: Rewrite Filter.tsx**

`Filter.tsx` imports mock data arrays (`labels`, `priorities`, `projects`, `status`, `users`) and `useIssuesStore`. Replace with data from props:

```typescript
interface FilterProps {
  issues: Issue[];
  projects: DisplayProject[];
}

export function Filter({ issues, projects }: FilterProps) {
  // Use issues for count computation (already mapped)
  // Use status from lib/status-icons
  // Use priorities from lib/priority-icons
  // Remove assignee filter (single-user app)
  // labels: derive unique labels from issues instead of mock array
  // projects: use prop
```

Key changes:
- Import `status as allStatus` from `../lib/status-icons`
- Import `priorities` from `../lib/priority-icons`
- Remove `useIssuesStore` import — `issues` comes as prop
- Remove `users` import and `renderAssigneeItems()` — single-user app
- Replace `labels` import with dynamic extraction from issues: `const uniqueLabels = useMemo(() => [...new Map(issues.flatMap(i => i.labels).map(l => [l.id, l])).values()], [issues])`
- Replace `projects` import with prop

- [ ] **Step 5: Update TabBar.tsx to pass areas/projects to AddTabMenu**

`TabBar` renders `AddTabMenu`. Now that `AddTabMenu` expects `areas` and `projects` props, `TabBar` needs to accept and forward them. Add props to `TabBar`:

```typescript
interface TabBarProps {
  areas?: Area[];
  projects?: Project[];
}
```

Thread these from `Tasks2Page` → `TabBar` → `AddTabMenu`.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): update all passthrough component imports from mock-data to mappers"
```

---

## Chunk 5: Detail View Components

### Task 14: Update detail view type imports

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailView.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailSidebar.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarTime.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarAiInsights.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarProperties.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx`

- [ ] **Step 1: Update IssueDetailView**

The hook signature changed — `useIssueDetail` now takes `(issueId, projectMap, areaMap)`. Update `IssueDetailView` to accept `projectMap` and `areaMap` as props and pass them to the hook:

```typescript
// In IssueDetailView.tsx, update the props and hook call:
import type { Area } from "@shared/types/tasks";
import type { DisplayProject } from "../../lib/mappers";

interface IssueDetailViewProps {
  issueId: string;
  projectMap: Map<string, DisplayProject>;
  areaMap: Map<string, Area>;
}

export function IssueDetailView({ issueId, projectMap, areaMap }: IssueDetailViewProps) {
  const detail = useIssueDetail(issueId, projectMap, areaMap);
  // ... rest stays the same
}
```

- [ ] **Step 2: Update IssueActivityTab**

Change the import from mock types to mapper types:

```typescript
// Before:
import type { ActorType, MockActivityEntry } from "../../mock-data/issue-detail";

// After:
import type { ActorType, ActivityEntry } from "../../lib/mappers";

// Update the interface:
interface IssueActivityTabProps {
  activity: ActivityEntry[];
}
```

- [ ] **Step 3: Update SidebarTime**

```typescript
// Before:
import type { MockDetailTask, TaskState } from "../../mock-data/issue-detail";

// After:
import type { DetailTask, TaskState } from "../../lib/mappers";

// Update interface:
interface SidebarTimeProps {
  task: DetailTask;
  taskState: TaskState;
}
```

- [ ] **Step 4: Update SidebarWorkState**

```typescript
// Before:
import type { MockDetailTask, MockFocusSession, TaskState } from "../../mock-data/issue-detail";

// After:
import type { DetailTask, FocusSession, TaskState } from "../../lib/mappers";

// Update interface:
interface SidebarWorkStateProps {
  task: DetailTask;
  taskState: TaskState;
  focusSession: FocusSession | null;
}
```

- [ ] **Step 5: Update SidebarAiInsights**

```typescript
// Before:
import type { MockSuggestion, MockTaskMemory, TaskState } from "../../mock-data/issue-detail";

// After:
import type { Suggestion, TaskMemory, TaskState } from "../../lib/mappers";

// Update interface:
interface SidebarAiInsightsProps {
  taskState: TaskState;
  suggestions: Suggestion[];
  taskMemory: TaskMemory | null;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}
```

Also update internal function signatures to use `Suggestion` instead of `MockSuggestion` and `TaskMemory` instead of `MockTaskMemory`.

Handle `taskMemory` being `null`:
- Change prop type: `taskMemory: TaskMemory | null`
- `WhatAiLearned({ memory })`: add `if (!memory) return null;` at top
- `TaskMemorySection({ memory })`: add `if (!memory) return null;` at top
- In `SidebarAiInsights` body: guard `taskMemory` before passing to children:
  ```typescript
  {taskState === "completed" && taskMemory ? (
    <WhatAiLearned memory={taskMemory} />
  ) : taskState === "new" && suggestions.filter((s) => s.status === "pending").length === 0 ? (
    <WhyThisTaskNow />
  ) : (
    <SuggestionsList suggestions={suggestions} onApply={onApply} onDismiss={onDismiss} />
  )}
  {taskState !== "completed" && taskState !== "new" && taskMemory && (
    <TaskMemorySection memory={taskMemory} />
  )}
  ```

- [ ] **Step 6: Update SidebarProperties**

```typescript
// Before:
import type { EnergyLevel, MockDetailTask, TaskType } from "../../mock-data/issue-detail";
import { priorities } from "../../mock-data/priorities";
import { status as allStatus } from "../../mock-data/status";

// After:
import type { DetailTask } from "../../lib/mappers";
import { priorities } from "../../lib/priority-icons";
import { status as allStatus } from "../../lib/status-icons";

type EnergyLevel = "low" | "medium" | "high" | "deep";
type TaskType = "manual" | "agentic" | "hybrid";

// Update interface:
interface SidebarPropertiesProps {
  task: DetailTask;
  compact: boolean;
  onUpdate: <K extends keyof DetailTask>(field: K, value: DetailTask[K]) => void;
}
```

- [ ] **Step 7: Update IssueContentTab**

```typescript
// Before:
import type { MockSubIssue } from "../../mock-data/issue-detail";

// After:
import type { SubIssue } from "../../lib/mappers";

// Update SubIssuesList function signature:
function SubIssuesList({ issues }: { issues: SubIssue[] }) {
```

- [ ] **Step 8: Update IssueDetailSidebar**

The sidebar uses `ReturnType<typeof useIssueDetail>` — this will automatically pick up the new types since the hook is rewritten. No changes needed unless the return shape changed. The key difference is `taskMemory` is now `TaskMemory | null` instead of `MockTaskMemory`. Verify this is handled in SidebarAiInsights (done in step 5).

- [ ] **Step 9: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/
git commit -m "feat(tasks2): update detail view components to use real data types"
```

---

## Chunk 6: Cleanup

### Task 15: Delete mock data files and issues-store

**Files:**
- Delete: `desktop-ui/src/features/tasks2/mock-data/issues.ts`
- Delete: `desktop-ui/src/features/tasks2/mock-data/issue-detail.ts`
- Delete: `desktop-ui/src/features/tasks2/mock-data/areas.ts`
- Delete: `desktop-ui/src/features/tasks2/mock-data/projects.ts`
- Delete: `desktop-ui/src/features/tasks2/mock-data/users.ts`
- Delete: `desktop-ui/src/features/tasks2/mock-data/labels.ts`
- Delete: `desktop-ui/src/features/tasks2/store/issues-store.ts`

- [ ] **Step 1: Delete mock data files**

```bash
rm desktop-ui/src/features/tasks2/mock-data/issues.ts
rm desktop-ui/src/features/tasks2/mock-data/issue-detail.ts
rm desktop-ui/src/features/tasks2/mock-data/areas.ts
rm desktop-ui/src/features/tasks2/mock-data/projects.ts
rm desktop-ui/src/features/tasks2/mock-data/users.ts
rm desktop-ui/src/features/tasks2/mock-data/labels.ts
rm desktop-ui/src/features/tasks2/store/issues-store.ts
```

- [ ] **Step 2: Verify no remaining imports of deleted files**

```bash
cd desktop-ui && grep -r "mock-data/issues\|mock-data/issue-detail\|mock-data/areas\|mock-data/projects\|mock-data/users\|mock-data/labels\|issues-store" src/features/tasks2/ --include="*.ts" --include="*.tsx" | grep -v "status-icons\|priority-icons"
```

Expected: no results. If any remain, fix them.

- [ ] **Step 3: Build check**

Run: `cd desktop-ui && bun run build 2>&1 | tail -20`
Expected: build succeeds

- [ ] **Step 4: Lint check**

Run: `cd desktop-ui && bun run lint:fix 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add -A desktop-ui/src/features/tasks2/
git commit -m "chore(tasks2): delete mock data files and issues-store"
```

---

### Task 16: Final verification

- [ ] **Step 1: Full frontend build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -20`
Expected: successful build

- [ ] **Step 2: Run frontend tests**

Run: `cd desktop-ui && bun run test 2>&1 | tail -20`
Expected: tests pass (some may need updating if they imported mock data)

- [ ] **Step 3: Full backend build**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: successful build

- [ ] **Step 4: Backend tests**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: all tests pass

- [ ] **Step 5: Lint**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10`
Expected: 0 warnings

- [ ] **Step 6: Manual smoke test**

Run: `cargo tauri dev`
- Navigate to the tasks2 route
- Verify tasks load from real data (not mock)
- Try creating a task
- Try changing status/priority
- Navigate to a task detail
- Verify activity tab shows real timeline data
- Verify sub-issues load
- Verify AI insights shows empty state

- [ ] **Step 7: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix(tasks2): final integration fixes"
```
