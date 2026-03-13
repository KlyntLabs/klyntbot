# Task Detail View Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a two-column task detail view inside the tasks2 tab system, replacing the "coming soon" placeholder with a full work cockpit — all driven by mock data.

**Architecture:** The detail view renders inline within `TabContent.tsx` when `currentView.type === "issue"`. A `useIssueDetail` hook provides all mock data and derives sidebar state. Components live in `features/tasks2/components/detail/`. The notes TipTap editor is imported directly for the Content tab.

**Tech Stack:** React 19, TypeScript, TipTap (from notes feature), Radix UI (Popover/Command), Tailwind v4 + CSS variables (tasks2-scope), Zustand (tab store)

**Spec:** `docs/superpowers/specs/2026-03-13-task-detail-view-design.md`

---

## File Map

```
desktop-ui/src/features/tasks2/
├── mock-data/
│   └── issue-detail.ts                    ← CREATE: all mock data + types
├── hooks/
│   └── useIssueDetail.ts                  ← CREATE: data hook + state derivation
├── components/
│   ├── TabContent.tsx                     ← MODIFY: wire "issue" case
│   └── detail/
│       ├── IssueDetailView.tsx            ← CREATE: root two-column layout
│       ├── IssueDetailBreadcrumb.tsx     ← CREATE: nav breadcrumb (Area / Project / ID)
│       ├── IssueDetailTitle.tsx           ← CREATE: inline-editable textarea
│       ├── IssueDetailTabs.tsx            ← CREATE: Content | Activity tab switcher
│       ├── IssueContentTab.tsx            ← CREATE: TipTap + acceptance criteria + sub-issues
│       ├── IssueActivityTab.tsx           ← CREATE: chronological activity feed
│       ├── IssueDetailSidebar.tsx         ← CREATE: sidebar shell with state-based sections
│       ├── SidebarProperties.tsx          ← CREATE: Linear-style property rows
│       ├── SidebarWorkState.tsx           ← CREATE: live focus session data
│       ├── SidebarTime.tsx                ← CREATE: estimated vs tracked progress
│       └── SidebarAiInsights.tsx          ← CREATE: suggestions + memory
```

---

## Chunk 1: Mock Data, Types, Hook, and Root Layout

### Task 1: Mock data and types

**Files:**
- Create: `desktop-ui/src/features/tasks2/mock-data/issue-detail.ts`

This file defines all types and mock data for the detail view. Types are local to mock data (not shared/types) since this is UI-first.

- [ ] **Step 1: Create the mock data file with types and exports**

```typescript
// desktop-ui/src/features/tasks2/mock-data/issue-detail.ts
import type { Priority } from "./priorities";
import type { Status } from "./status";
import type { LabelInterface } from "./labels";
import type { Project } from "./projects";
import type { MockArea } from "./areas";
import { status, priorities, labels, projects, areas } from "./index-helpers";

// ── Types ──────────────────────────────────────────────────────────

export type TaskState = "new" | "focused" | "has-history" | "completed";
export type ActorType = "user" | "agent" | "system";
export type FlowState = "active" | "building" | "lost";
export type FocusMode = "deep-work" | "focus" | "pomodoro";
export type EnergyLevel = "low" | "medium" | "high" | "deep";
export type TaskType = "manual" | "agentic" | "hybrid";
export type SuggestionStatus = "pending" | "applied" | "dismissed";

export interface MockDetailTask {
  id: string;
  identifier: string;
  title: string;
  description: string; // HTML for TipTap
  status: Status;
  priority: Priority;
  labels: LabelInterface[];
  project: Project | null;
  area: MockArea;
  tags: string[];
  dueDate: string | null; // ISO
  energyLevel: EnergyLevel | null;
  taskType: TaskType;
  estimatedMinutes: number | null;
  actualMinutes: number | null;
  totalTrackedSecs: number;
  focusedAt: string | null; // ISO timestamp
  acceptanceCriteria: string | null;
  completed: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface MockActivityEntry {
  id: string;
  actorType: ActorType;
  actorName: string;
  action: string;
  detail: string | null;
  createdAt: string; // ISO
}

export interface MockSuggestion {
  id: string;
  title: string;
  description: string;
  confidence: number; // 0-1
  status: SuggestionStatus;
}

export interface MockFocusSession {
  focusMode: FocusMode;
  qualityScore: number; // 0-1
  distractionCount: number;
  flowState: FlowState;
  qualityHistory: number[]; // 5-min buckets
}

export interface MockSubIssue {
  id: string;
  identifier: string;
  title: string;
  status: Status;
  priority: Priority;
  completed: boolean;
}

export interface MockTaskMemory {
  lastSessionSummary: string;
  continuityNote: string;
  relatedFacts: string[];
}
```

Note: The imports from `./index-helpers` won't exist yet — use direct imports from `./status`, `./priorities`, `./labels`, `./projects`, `./areas` instead. The helpers `s()`, `p()`, `l()` from `issues.ts` can be replicated locally.

Now add the mock data instances:

```typescript
// ── Helper lookups (same pattern as issues.ts) ──────────────────────

import { status as allStatus } from "./status";
import { priorities as allPriorities } from "./priorities";
import { labels as allLabels } from "./labels";
import { projects as allProjects } from "./projects";
import { areas as allAreas } from "./areas";

function s(id: string): Status {
  return allStatus.find((x) => x.id === id)!;
}
function p(id: string): Priority {
  return allPriorities.find((x) => x.id === id)!;
}
function pr(id: string): Project {
  return allProjects.find((x) => x.id === id)!;
}
function a(id: string): MockArea {
  return allAreas.find((x) => x.id === id)!;
}
const l = (...ids: string[]): LabelInterface[] =>
  allLabels.filter((x) => ids.includes(x.id));

// ── Mock Data ───────────────────────────────────────────────────────

export const mockDetailTask: MockDetailTask = {
  id: "detail-1",
  identifier: "LNUI-101",
  title: "Implement drag-and-drop issue reordering",
  description:
    "<p>Add drag-and-drop support for reordering issues within status groups using <strong>DnD Kit</strong>.</p><h2>Requirements</h2><ul><li>Drag handle on each issue row</li><li>Visual feedback during drag</li><li>Persist new order via rank field</li></ul><p>See <code>@dnd-kit/sortable</code> docs for the <code>SortableContext</code> API.</p>",
  status: s("in-progress"),
  priority: p("high"),
  labels: l("ui", "feature"),
  project: pr("1"),
  area: a("area-work"),
  tags: ["frontend", "ux"],
  dueDate: "2026-03-20T00:00:00Z",
  energyLevel: "high",
  taskType: "hybrid",
  estimatedMinutes: 240,
  actualMinutes: null,
  totalTrackedSecs: 4500, // 1h 15m
  focusedAt: new Date(Date.now() - 25 * 60 * 1000).toISOString(), // 25 min ago
  acceptanceCriteria:
    "- [ ] Drag handle visible on hover\n- [ ] Items reorder smoothly with animation\n- [ ] New rank persists across page reload\n- [ ] Works with keyboard (a11y)",
  completed: false,
  createdAt: "2026-01-15T10:00:00Z",
  updatedAt: "2026-03-12T14:30:00Z",
};

export const mockActivityEntries: MockActivityEntry[] = [
  {
    id: "act-1",
    actorType: "user",
    actorName: "You",
    action: "created this task",
    detail: null,
    createdAt: "2026-01-15T10:00:00Z",
  },
  {
    id: "act-2",
    actorType: "user",
    actorName: "You",
    action: "changed priority",
    detail: "Medium → High",
    createdAt: "2026-01-20T09:15:00Z",
  },
  {
    id: "act-3",
    actorType: "agent",
    actorName: "AI Assistant",
    action: "suggested decomposition",
    detail: "Break into 3 sub-tasks: drag handle, visual feedback, persistence",
    createdAt: "2026-02-01T11:00:00Z",
  },
  {
    id: "act-4",
    actorType: "user",
    actorName: "You",
    action: "changed status",
    detail: "Todo → In Progress",
    createdAt: "2026-02-15T08:30:00Z",
  },
  {
    id: "act-5",
    actorType: "system",
    actorName: "System",
    action: "focus session completed",
    detail: "45 min · Quality 0.82 · 1 distraction",
    createdAt: "2026-03-10T15:45:00Z",
  },
  {
    id: "act-6",
    actorType: "agent",
    actorName: "AI Assistant",
    action: "applied suggestion",
    detail: "Adjusted estimate from 3h to 4h based on complexity analysis",
    createdAt: "2026-03-11T10:20:00Z",
  },
  {
    id: "act-7",
    actorType: "user",
    actorName: "You",
    action: "updated description",
    detail: null,
    createdAt: "2026-03-12T14:30:00Z",
  },
  {
    id: "act-8",
    actorType: "agent",
    actorName: "AI Assistant",
    action: "added insight",
    detail: "Similar pattern used in Kanban board — reuse DnD context provider",
    createdAt: "2026-03-13T09:00:00Z",
  },
];

export const mockSuggestions: MockSuggestion[] = [
  {
    id: "sug-1",
    title: "Extract DnD context provider",
    description:
      "The sortable context setup is identical to the Kanban board. Extract a shared DnDProvider to reduce duplication.",
    confidence: 0.87,
    status: "pending",
  },
  {
    id: "sug-2",
    title: "Add keyboard reordering",
    description:
      "DnD Kit supports keyboard sensors out of the box. Enable KeyboardSensor for accessibility compliance.",
    confidence: 0.72,
    status: "pending",
  },
  {
    id: "sug-3",
    title: "Consider optimistic rank updates",
    description:
      "Persist rank changes optimistically to avoid UI flicker on reorder. Rollback on save failure.",
    confidence: 0.65,
    status: "pending",
  },
];

export const mockFocusSession: MockFocusSession = {
  focusMode: "deep-work",
  qualityScore: 0.78,
  distractionCount: 2,
  flowState: "active",
  qualityHistory: [0.6, 0.65, 0.72, 0.78, 0.81, 0.76, 0.78, 0.82, 0.79, 0.78],
};

export const mockSubIssues: MockSubIssue[] = [
  {
    id: "sub-1",
    identifier: "LNUI-101a",
    title: "Add drag handle to issue rows",
    status: s("completed"),
    priority: p("high"),
    completed: true,
  },
  {
    id: "sub-2",
    identifier: "LNUI-101b",
    title: "Implement visual drag feedback",
    status: s("in-progress"),
    priority: p("high"),
    completed: false,
  },
  {
    id: "sub-3",
    identifier: "LNUI-101c",
    title: "Persist reorder via rank field",
    status: s("to-do"),
    priority: p("medium"),
    completed: false,
  },
  {
    id: "sub-4",
    identifier: "LNUI-101d",
    title: "Keyboard reordering support",
    status: s("backlog"),
    priority: p("low"),
    completed: false,
  },
];

export const mockTaskMemory: MockTaskMemory = {
  lastSessionSummary:
    "Explored DnD Kit docs and set up SortableContext. Got basic drag working but visual feedback needs polish.",
  continuityNote: "Left off debugging the drag overlay — ghost image flickers on fast moves.",
  relatedFacts: [
    "Kanban board uses same DnD Kit version",
    "LexoRank utility already exists in tasks2/lib/utils.ts",
  ],
};
```

- [ ] **Step 2: Verify file compiles**

Run: `cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors from `issue-detail.ts`

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/mock-data/issue-detail.ts
git commit -m "feat(tasks2): add mock data and types for issue detail view"
```

---

### Task 2: useIssueDetail hook

**Files:**
- Create: `desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts`

- [ ] **Step 1: Create the hook**

```typescript
// desktop-ui/src/features/tasks2/hooks/useIssueDetail.ts
import { useCallback, useState } from "react";
import {
  type MockDetailTask,
  type MockActivityEntry,
  type MockSuggestion,
  type MockFocusSession,
  type MockSubIssue,
  type MockTaskMemory,
  type TaskState,
  mockDetailTask,
  mockActivityEntries,
  mockSuggestions,
  mockFocusSession,
  mockSubIssues,
  mockTaskMemory,
} from "../mock-data/issue-detail";

export function deriveTaskState(task: MockDetailTask): TaskState {
  if (task.completed) return "completed";
  if (task.focusedAt) return "focused";
  if (task.totalTrackedSecs > 0) return "has-history";
  return "new";
}

interface UseIssueDetailReturn {
  task: MockDetailTask;
  taskState: TaskState;
  activity: MockActivityEntry[];
  suggestions: MockSuggestion[];
  focusSession: MockFocusSession | null;
  subIssues: MockSubIssue[];
  taskMemory: MockTaskMemory;
  updateTask: (field: string, value: unknown) => void;
  dismissSuggestion: (id: string) => void;
  applySuggestion: (id: string) => void;
}

export function useIssueDetail(_issueId: string): UseIssueDetailReturn {
  const [task, setTask] = useState<MockDetailTask>(mockDetailTask);
  const [suggestions, setSuggestions] = useState(mockSuggestions);

  const taskState = deriveTaskState(task);

  const updateTask = useCallback((field: string, value: unknown) => {
    setTask((prev) => ({ ...prev, [field]: value }));
  }, []);

  const dismissSuggestion = useCallback((id: string) => {
    setSuggestions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, status: "dismissed" as const } : s)),
    );
  }, []);

  const applySuggestion = useCallback((id: string) => {
    setSuggestions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, status: "applied" as const } : s)),
    );
  }, []);

  return {
    task,
    taskState,
    activity: mockActivityEntries,
    suggestions,
    focusSession: taskState === "focused" ? mockFocusSession : null,
    subIssues: mockSubIssues,
    taskMemory: mockTaskMemory,
    updateTask,
    dismissSuggestion,
    applySuggestion,
  };
}
```

- [ ] **Step 2: Write test for deriveTaskState**

Create: `desktop-ui/src/features/tasks2/hooks/__tests__/useIssueDetail.test.ts`

```typescript
import { describe, expect, it } from "vitest";
import { deriveTaskState } from "../useIssueDetail";
import { mockDetailTask } from "../../mock-data/issue-detail";

describe("deriveTaskState", () => {
  it("returns 'completed' when task.completed is true", () => {
    expect(deriveTaskState({ ...mockDetailTask, completed: true })).toBe("completed");
  });

  it("returns 'focused' when focusedAt is set", () => {
    expect(
      deriveTaskState({ ...mockDetailTask, completed: false, focusedAt: "2026-03-13T10:00:00Z" }),
    ).toBe("focused");
  });

  it("returns 'has-history' when tracked time exists but not focused", () => {
    expect(
      deriveTaskState({
        ...mockDetailTask,
        completed: false,
        focusedAt: null,
        totalTrackedSecs: 100,
      }),
    ).toBe("has-history");
  });

  it("returns 'new' when no completion, focus, or tracking", () => {
    expect(
      deriveTaskState({
        ...mockDetailTask,
        completed: false,
        focusedAt: null,
        totalTrackedSecs: 0,
      }),
    ).toBe("new");
  });

  it("completed takes priority over focused", () => {
    expect(
      deriveTaskState({
        ...mockDetailTask,
        completed: true,
        focusedAt: "2026-03-13T10:00:00Z",
      }),
    ).toBe("completed");
  });
});
```

- [ ] **Step 3: Run tests**

Run: `cd desktop-ui && bun run test -- --run src/features/tasks2/hooks/__tests__/useIssueDetail.test.ts`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/hooks/
git commit -m "feat(tasks2): add useIssueDetail hook with state derivation"
```

---

### Task 3: IssueDetailView root layout + TabContent wiring

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/detail/IssueDetailView.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/TabContent.tsx`

- [ ] **Step 1: Create IssueDetailView with two-column layout**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueDetailView.tsx
import { PanelRight } from "lucide-react";
import { useState } from "react";
import { useIssueDetail } from "../../hooks/useIssueDetail";
import { IssueDetailBreadcrumb } from "./IssueDetailBreadcrumb";
import { IssueDetailSidebar } from "./IssueDetailSidebar";
import { IssueDetailTabs } from "./IssueDetailTabs";
import { IssueDetailTitle } from "./IssueDetailTitle";

interface IssueDetailViewProps {
  issueId: string;
}

export function IssueDetailView({ issueId }: IssueDetailViewProps) {
  const detail = useIssueDetail(issueId);
  const [sidebarOpen, setSidebarOpen] = useState(true);

  return (
    <div className="flex h-full relative">
      {/* Left column — main content */}
      <div className="flex-1 min-w-0 overflow-y-auto px-6 py-4">
        <IssueDetailBreadcrumb task={detail.task} />
        <IssueDetailTitle
          title={detail.task.title}
          onUpdate={(title) => detail.updateTask("title", title)}
        />
        <IssueDetailTabs detail={detail} />
      </div>

      {/* Sidebar toggle (visible when sidebar collapsed) */}
      {!sidebarOpen && (
        <button
          type="button"
          onClick={() => setSidebarOpen(true)}
          className="absolute top-3 right-3 p-1.5 rounded hover:bg-[hsl(var(--accent))] text-[hsl(var(--muted-foreground))] z-10"
          aria-label="Show sidebar"
        >
          <PanelRight className="size-4" />
        </button>
      )}

      {/* Right column — sidebar */}
      {sidebarOpen && (
        <IssueDetailSidebar
          detail={detail}
          onClose={() => setSidebarOpen(false)}
        />
      )}
    </div>
  );
}
```

Note: `relative` on root div for absolute sidebar toggle positioning. ResizeObserver for auto-collapse added in Task 13.

- [ ] **Step 2: Wire into TabContent.tsx**

In `desktop-ui/src/features/tasks2/components/TabContent.tsx`, replace the "issue" placeholder case (lines 43-48):

```tsx
// Replace:
case "issue":
  return (
    <div className="px-6 py-8 text-sm text-[hsl(var(--muted-foreground))]">
      Issue detail view coming soon: {currentView.label}
    </div>
  );

// With:
case "issue":
  return <IssueDetailView issueId={currentView.targetId} />;
```

Add the import at the top of `TabContent.tsx`:
```tsx
import { IssueDetailView } from "./detail/IssueDetailView";
```

- [ ] **Step 3: Create placeholder stubs for child components**

Create minimal stubs so the app compiles. These will be replaced in subsequent tasks.

`IssueDetailBreadcrumb.tsx`:
```tsx
import type { MockDetailTask } from "../../mock-data/issue-detail";

interface IssueDetailBreadcrumbProps {
  task: MockDetailTask;
}

export function IssueDetailBreadcrumb({ task }: IssueDetailBreadcrumbProps) {
  return (
    <div className="flex items-center gap-1 text-xs mb-2 text-[hsl(var(--muted-foreground))]">
      {task.area.name} › {task.project?.name ?? "—"} › {task.identifier}
    </div>
  );
}
```

`IssueDetailTitle.tsx`:
```tsx
interface IssueDetailTitleProps {
  title: string;
  onUpdate: (title: string) => void;
}

export function IssueDetailTitle({ title }: IssueDetailTitleProps) {
  return (
    <h1 className="text-2xl font-semibold text-[hsl(var(--foreground))] mb-4">{title}</h1>
  );
}
```

`IssueDetailTabs.tsx`:
```tsx
import type { useIssueDetail } from "../../hooks/useIssueDetail";

interface IssueDetailTabsProps {
  detail: ReturnType<typeof useIssueDetail>;
}

export function IssueDetailTabs({ detail }: IssueDetailTabsProps) {
  return (
    <div className="text-sm text-[hsl(var(--muted-foreground))]">
      Tabs placeholder — {detail.task.identifier}
    </div>
  );
}
```

`IssueDetailSidebar.tsx`:
```tsx
import type { useIssueDetail } from "../../hooks/useIssueDetail";

interface IssueDetailSidebarProps {
  detail: ReturnType<typeof useIssueDetail>;
  onClose: () => void;
}

export function IssueDetailSidebar({ detail, onClose }: IssueDetailSidebarProps) {
  return (
    <div className="w-[260px] shrink-0 border-l border-[hsl(var(--border))] overflow-y-auto p-4">
      <div className="flex items-center justify-between mb-4">
        <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
          Properties
        </span>
        <button
          type="button"
          onClick={onClose}
          className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]"
          aria-label="Close sidebar"
        >
          ×
        </button>
      </div>
      <div className="text-sm text-[hsl(var(--muted-foreground))]">
        Sidebar placeholder — {detail.taskState}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Verify app renders**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds. Navigate to tasks2, click an issue row, see the detail view with title + placeholders.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/ desktop-ui/src/features/tasks2/components/TabContent.tsx
git commit -m "feat(tasks2): wire IssueDetailView into tab content with two-column layout"
```

---

## Chunk 2: Left Column — Title, Tabs, Content Tab

### Task 3b: IssueDetailBreadcrumb — nav breadcrumb with clickable segments

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailBreadcrumb.tsx`

- [ ] **Step 1: Replace the stub with the full breadcrumb implementation**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueDetailBreadcrumb.tsx
import { useShallow } from "zustand/react/shallow";
import type { MockDetailTask } from "../../mock-data/issue-detail";
import { useTabStore } from "../../store/tab-store";

interface IssueDetailBreadcrumbProps {
  task: MockDetailTask;
}

export function IssueDetailBreadcrumb({ task }: IssueDetailBreadcrumbProps) {
  const navStack = useTabStore(
    useShallow((s) => {
      const active = s.tabs.find((t) => t.id === s.activeTabId);
      return active?.navStack ?? [];
    }),
  );
  const navigateToStackIndex = useTabStore((s) => s.navigateToStackIndex);

  // Build breadcrumb segments from the nav stack
  // The last entry is the current issue — show it as non-clickable
  return (
    <div className="flex items-center gap-1 mb-3 min-w-0">
      {navStack.map((entry, index) => {
        const isLast = index === navStack.length - 1;
        return (
          <div key={`${entry.type}-${entry.targetId}`} className="flex items-center gap-1 min-w-0">
            {index > 0 && (
              <span className="text-xs text-[hsl(var(--muted-foreground))] shrink-0">›</span>
            )}
            {isLast ? (
              <span className="text-xs font-medium text-[hsl(var(--foreground))] truncate">
                {entry.label}
              </span>
            ) : (
              <button
                type="button"
                onClick={() => navigateToStackIndex(index)}
                className="text-xs text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors truncate"
              >
                {entry.label}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
```

Note: This follows the exact same pattern as `HeaderNav.tsx` (lines 26-51) which already renders breadcrumbs from the nav stack. The difference is this is positioned inside the detail view content area rather than in the header bar.

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/IssueDetailBreadcrumb.tsx
git commit -m "feat(tasks2): implement IssueDetailBreadcrumb with clickable nav segments"
```

---

### Task 4: Inline-editable title

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailTitle.tsx`

- [ ] **Step 1: Implement auto-resizing textarea title**

Replace the stub with the full implementation:

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueDetailTitle.tsx
import { useCallback, useEffect, useRef, useState } from "react";

interface IssueDetailTitleProps {
  title: string;
  onUpdate: (title: string) => void;
}

export function IssueDetailTitle({ title, onUpdate }: IssueDetailTitleProps) {
  const [value, setValue] = useState(title);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Sync external title changes
  useEffect(() => {
    setValue(title);
  }, [title]);

  // Auto-resize
  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [value]);

  const handleBlur = useCallback(() => {
    const trimmed = value.trim();
    if (trimmed && trimmed !== title) {
      onUpdate(trimmed);
    } else {
      setValue(title); // revert if empty
    }
  }, [value, title, onUpdate]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        (e.target as HTMLTextAreaElement).blur();
      }
    },
    [],
  );

  return (
    <textarea
      ref={textareaRef}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      rows={1}
      className="w-full text-2xl font-semibold text-[hsl(var(--foreground))] bg-transparent border-none outline-none resize-none mb-4 p-0 leading-tight placeholder:text-[hsl(var(--muted-foreground))]"
      placeholder="Task title"
    />
  );
}
```

- [ ] **Step 2: Verify it renders**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/IssueDetailTitle.tsx
git commit -m "feat(tasks2): implement inline-editable title with auto-resize textarea"
```

---

### Task 5: Tab switcher (Content / Activity Log)

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailTabs.tsx`
- Create: `desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx`
- Create: `desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx`

- [ ] **Step 1: Implement the tab switcher**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueDetailTabs.tsx
import { useState } from "react";
import { cn } from "../../lib/utils";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { IssueContentTab } from "./IssueContentTab";
import { IssueActivityTab } from "./IssueActivityTab";

type TabId = "content" | "activity";

interface IssueDetailTabsProps {
  detail: ReturnType<typeof useIssueDetail>;
}

const tabs: { id: TabId; label: string }[] = [
  { id: "content", label: "Content" },
  { id: "activity", label: "Activity Log" },
];

export function IssueDetailTabs({ detail }: IssueDetailTabsProps) {
  const [activeTab, setActiveTab] = useState<TabId>("content");

  return (
    <div className="flex flex-col min-h-0">
      {/* Tab bar */}
      <div className="flex gap-4 border-b border-[hsl(var(--border))] mb-4">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "pb-2 text-sm font-medium transition-colors border-b-2 -mb-px",
              activeTab === tab.id
                ? "border-[hsl(var(--foreground))] text-[hsl(var(--foreground))]"
                : "border-transparent text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      {activeTab === "content" ? (
        <IssueContentTab detail={detail} />
      ) : (
        <IssueActivityTab activity={detail.activity} />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create IssueContentTab stub**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx
import type { useIssueDetail } from "../../hooks/useIssueDetail";

interface IssueContentTabProps {
  detail: ReturnType<typeof useIssueDetail>;
}

export function IssueContentTab({ detail }: IssueContentTabProps) {
  return (
    <div className="space-y-6">
      <div className="text-sm text-[hsl(var(--muted-foreground))]">
        Editor placeholder for: {detail.task.identifier}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create IssueActivityTab stub**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx
import type { MockActivityEntry } from "../../mock-data/issue-detail";

interface IssueActivityTabProps {
  activity: MockActivityEntry[];
}

export function IssueActivityTab({ activity }: IssueActivityTabProps) {
  return (
    <div className="text-sm text-[hsl(var(--muted-foreground))]">
      Activity placeholder — {activity.length} entries
    </div>
  );
}
```

- [ ] **Step 4: Verify build + tab switching works**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/IssueDetailTabs.tsx desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx
git commit -m "feat(tasks2): add Content/Activity tab switcher"
```

---

### Task 6: Content tab — TipTap editor, acceptance criteria, sub-issues

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx`

- [ ] **Step 1: Implement the full Content tab**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx
import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import {
  EditorContentWrapper,
  useNoteEditor,
} from "@features/notes/components/editor/EditorCore";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { renderStatusIcon } from "../../lib/status-utils";
import { useTabStore } from "../../store/tab-store";
import type { MockSubIssue } from "../../mock-data/issue-detail";

interface IssueContentTabProps {
  detail: ReturnType<typeof useIssueDetail>;
}

export function IssueContentTab({ detail }: IssueContentTabProps) {
  return (
    <div className="space-y-6">
      <DescriptionEditor
        content={detail.task.description}
        onUpdate={(html) => detail.updateTask("description", html)}
      />
      {detail.task.acceptanceCriteria && (
        <AcceptanceCriteria text={detail.task.acceptanceCriteria} />
      )}
      {detail.subIssues.length > 0 && <SubIssuesList issues={detail.subIssues} />}
    </div>
  );
}

// ── Description Editor ──────────────────────────────────────────────

function DescriptionEditor({
  content,
  onUpdate,
}: { content: string; onUpdate: (html: string) => void }) {
  const editor = useNoteEditor({
    content,
    onUpdate: (html, _text) => onUpdate(html),
    onNavigateNote: () => {},
    onNavigateEntity: () => {},
  });

  return <EditorContentWrapper editor={editor} className="min-h-[200px]" />;
}

// ── Acceptance Criteria ─────────────────────────────────────────────

function AcceptanceCriteria({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const lines = text.split("\n").filter(Boolean);
  const preview = lines[0] ?? "";

  return (
    <div className="border border-[hsl(var(--border))] rounded-md">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-3 py-2 text-sm font-medium text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]/50 transition-colors"
      >
        {expanded ? (
          <ChevronDown className="size-4 text-[hsl(var(--muted-foreground))]" />
        ) : (
          <ChevronRight className="size-4 text-[hsl(var(--muted-foreground))]" />
        )}
        Acceptance Criteria
        {!expanded && (
          <span className="text-[hsl(var(--muted-foreground))] font-normal truncate">
            — {preview}
          </span>
        )}
      </button>
      {expanded && (
        <div className="px-3 pb-3 text-sm text-[hsl(var(--foreground))] whitespace-pre-wrap font-mono">
          {text}
        </div>
      )}
    </div>
  );
}

// ── Sub-Issues List ─────────────────────────────────────────────────

function SubIssuesList({ issues }: { issues: MockSubIssue[] }) {
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);
  const completedCount = issues.filter((i) => i.completed).length;

  return (
    <div>
      <h3 className="text-sm font-medium text-[hsl(var(--foreground))] mb-2">
        Sub-issues ({completedCount}/{issues.length} done)
      </h3>
      <div className="border border-[hsl(var(--border))] rounded-md divide-y divide-[hsl(var(--border))]">
        {issues.map((issue) => (
          <SubIssueLine
            key={issue.id}
            issue={issue}
            onNavigate={() =>
              navigateInPlace("issue", issue.id, issue.identifier)
            }
          />
        ))}
      </div>
    </div>
  );
}

function SubIssueLine({
  issue,
  onNavigate,
}: { issue: MockSubIssue; onNavigate: () => void }) {
  const PriorityIcon = issue.priority.icon;

  return (
    <button
      type="button"
      onClick={onNavigate}
      className="flex items-center gap-2 w-full px-3 py-2 text-sm hover:bg-[hsl(var(--accent))]/50 transition-colors text-left"
    >
      <span className="flex items-center justify-center size-4">
        {renderStatusIcon(issue.status.id)}
      </span>
      <PriorityIcon className="size-3.5 text-[hsl(var(--muted-foreground))] shrink-0" />
      <span className="text-xs text-[hsl(var(--muted-foreground))] shrink-0">
        {issue.identifier}
      </span>
      <span className="truncate text-[hsl(var(--foreground))]">{issue.title}</span>
    </button>
  );
}
```

Important: Check whether `@features/notes/...` path alias works. If the project uses `@features` as a path alias, this import is fine. Otherwise use relative: `../../../../notes/components/editor/EditorCore`. Check `tsconfig.json` or `vite.config.ts` for path aliases.

- [ ] **Step 2: Verify the import path for EditorCore**

Run: `cd desktop-ui && grep -r '@features' src/features/tasks2/ | head -5` and `grep 'features' tsconfig.json vite.config.ts 2>/dev/null | head -10`

If `@features` is not aliased, use the relative import path instead:
```tsx
import { EditorContentWrapper, useNoteEditor } from "../../../notes/components/editor/EditorCore";
```

Also check that `renderStatusIcon` exists in `lib/status-utils`:
Run: `cat desktop-ui/src/features/tasks2/lib/status-utils.ts | head -5`

- [ ] **Step 3: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/IssueContentTab.tsx
git commit -m "feat(tasks2): implement Content tab with TipTap editor, acceptance criteria, and sub-issues"
```

---

### Task 7: Activity Log tab

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx`

- [ ] **Step 1: Implement the activity feed**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx
import { Bot, Monitor, User } from "lucide-react";
import { formatDate, formatRelativeTime, formatTime } from "@shared/lib/dates";
import { cn } from "../../lib/utils";
import type { MockActivityEntry, ActorType } from "../../mock-data/issue-detail";

const DAY_MS = 24 * 60 * 60 * 1000;

function formatActivityTime(iso: string): string {
  const age = Date.now() - new Date(iso).getTime();
  if (age < DAY_MS) return formatRelativeTime(iso);
  // Older than 24h — show absolute date + time
  return `${formatDate(iso.split("T")[0])} ${formatTime(iso)}`;
}

interface IssueActivityTabProps {
  activity: MockActivityEntry[];
}

export function IssueActivityTab({ activity }: IssueActivityTabProps) {
  return (
    <div className="space-y-0">
      {activity.map((entry) => (
        <ActivityEntry key={entry.id} entry={entry} />
      ))}
    </div>
  );
}

function ActivityEntry({ entry }: { entry: MockActivityEntry }) {
  return (
    <div className="flex gap-3 py-3 border-b border-[hsl(var(--border))]/50 last:border-b-0">
      <ActorAvatar type={entry.actorType} />
      <div className="flex-1 min-w-0">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium text-[hsl(var(--foreground))]">
            {entry.actorName}
          </span>
          <span className="text-sm text-[hsl(var(--muted-foreground))]">{entry.action}</span>
          <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))] shrink-0">
            {formatActivityTime(entry.createdAt)}
          </span>
        </div>
        {entry.detail && (
          <p className="text-sm text-[hsl(var(--muted-foreground))] mt-0.5">{entry.detail}</p>
        )}
      </div>
    </div>
  );
}

function ActorAvatar({ type }: { type: ActorType }) {
  if (type === "agent") {
    return (
      <div className="size-7 rounded-full shrink-0 flex items-center justify-center bg-gradient-to-br from-purple-500 to-indigo-600">
        <Bot className="size-3.5 text-white" />
      </div>
    );
  }
  if (type === "system") {
    return (
      <div className="size-7 rounded-full shrink-0 flex items-center justify-center bg-[hsl(var(--muted))]">
        <Monitor className="size-3.5 text-[hsl(var(--muted-foreground))]" />
      </div>
    );
  }
  return (
    <div className="size-7 rounded-full shrink-0 flex items-center justify-center bg-[hsl(var(--accent))]">
      <User className="size-3.5 text-[hsl(var(--foreground))]" />
    </div>
  );
}
```

Note: Verify `@shared/lib/dates` path alias works. If not, use relative path: `../../../../shared/lib/dates`.

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/IssueActivityTab.tsx
git commit -m "feat(tasks2): implement Activity Log tab with actor avatars"
```

---

## Chunk 3: Sidebar — Properties, Work State, Time, AI Insights

### Task 8: Sidebar shell with state-based section rendering

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailSidebar.tsx`

- [ ] **Step 1: Implement the sidebar shell**

```tsx
// desktop-ui/src/features/tasks2/components/detail/IssueDetailSidebar.tsx
import { X } from "lucide-react";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { SidebarProperties } from "./SidebarProperties";
import { SidebarWorkState } from "./SidebarWorkState";
import { SidebarTime } from "./SidebarTime";
import { SidebarAiInsights } from "./SidebarAiInsights";

interface IssueDetailSidebarProps {
  detail: ReturnType<typeof useIssueDetail>;
  onClose: () => void;
}

export function IssueDetailSidebar({ detail, onClose }: IssueDetailSidebarProps) {
  const { taskState } = detail;

  // State table takes precedence: "focused" shows live data, "completed" shows session summary.
  // The spec section header says "only when focused" but the state table row for completed says "Session summary".
  const showWorkState = taskState === "focused" || taskState === "completed";
  const showTime =
    taskState !== "new" ||
    (taskState === "new" && detail.task.estimatedMinutes != null);

  return (
    <div className="w-[260px] shrink-0 border-l border-[hsl(var(--border))] overflow-y-auto">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[hsl(var(--border))]">
        <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
          Details
        </span>
        <button
          type="button"
          onClick={onClose}
          className="p-0.5 rounded hover:bg-[hsl(var(--accent))] text-[hsl(var(--muted-foreground))] transition-colors"
          aria-label="Close sidebar"
        >
          <X className="size-3.5" />
        </button>
      </div>

      {/* Sections */}
      <div className="divide-y divide-[hsl(var(--border))]">
        <SidebarProperties
          task={detail.task}
          compact={taskState === "focused"}
          onUpdate={detail.updateTask}
        />

        {showWorkState && (
          <SidebarWorkState
            task={detail.task}
            taskState={taskState}
            focusSession={detail.focusSession}
          />
        )}

        {showTime && (
          <SidebarTime
            task={detail.task}
            taskState={taskState}
            onUpdate={detail.updateTask}
          />
        )}

        <SidebarAiInsights
          taskState={taskState}
          suggestions={detail.suggestions}
          taskMemory={detail.taskMemory}
          onApply={detail.applySuggestion}
          onDismiss={detail.dismissSuggestion}
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create stubs for all 4 sidebar sections**

`SidebarProperties.tsx`:
```tsx
import type { MockDetailTask } from "../../mock-data/issue-detail";

interface SidebarPropertiesProps {
  task: MockDetailTask;
  compact: boolean;
  onUpdate: (field: string, value: unknown) => void;
}

export function SidebarProperties({ task, compact }: SidebarPropertiesProps) {
  return (
    <div className="px-4 py-3">
      <div className="text-xs text-[hsl(var(--muted-foreground))]">
        Properties {compact ? "(compact)" : "(full)"} — {task.status.name}
      </div>
    </div>
  );
}
```

`SidebarWorkState.tsx`:
```tsx
import type { MockDetailTask, MockFocusSession, TaskState } from "../../mock-data/issue-detail";

interface SidebarWorkStateProps {
  task: MockDetailTask;
  taskState: TaskState;
  focusSession: MockFocusSession | null;
}

export function SidebarWorkState({ taskState }: SidebarWorkStateProps) {
  return (
    <div className="px-4 py-3">
      <div className="text-xs text-[hsl(var(--muted-foreground))]">
        Work State — {taskState}
      </div>
    </div>
  );
}
```

`SidebarTime.tsx`:
```tsx
import type { MockDetailTask, TaskState } from "../../mock-data/issue-detail";

interface SidebarTimeProps {
  task: MockDetailTask;
  taskState: TaskState;
  onUpdate: (field: string, value: unknown) => void;
}

export function SidebarTime({ taskState }: SidebarTimeProps) {
  return (
    <div className="px-4 py-3">
      <div className="text-xs text-[hsl(var(--muted-foreground))]">
        Time — {taskState}
      </div>
    </div>
  );
}
```

`SidebarAiInsights.tsx`:
```tsx
import type { MockSuggestion, MockTaskMemory, TaskState } from "../../mock-data/issue-detail";

interface SidebarAiInsightsProps {
  taskState: TaskState;
  suggestions: MockSuggestion[];
  taskMemory: MockTaskMemory;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}

export function SidebarAiInsights({ taskState }: SidebarAiInsightsProps) {
  return (
    <div className="px-4 py-3">
      <div className="text-xs text-[hsl(var(--muted-foreground))]">
        AI Insights — {taskState}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/
git commit -m "feat(tasks2): add sidebar shell with state-based section rendering"
```

---

### Task 9: SidebarProperties — Linear-style property rows

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarProperties.tsx`

- [ ] **Step 1: Implement property rows with dropdowns**

```tsx
// desktop-ui/src/features/tasks2/components/detail/SidebarProperties.tsx
import { Check } from "lucide-react";
import { useState } from "react";
import { formatDate } from "@shared/lib/dates";
import { cn } from "../../lib/utils";
import { renderStatusIcon } from "../../lib/status-utils";
import type { MockDetailTask, EnergyLevel, TaskType } from "../../mock-data/issue-detail";
import { areas } from "../../mock-data/areas";
import { labels } from "../../mock-data/labels";
import { priorities } from "../../mock-data/priorities";
import { projects } from "../../mock-data/projects";
import { status as allStatus } from "../../mock-data/status";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "../ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "../ui/popover";

interface SidebarPropertiesProps {
  task: MockDetailTask;
  compact: boolean;
  onUpdate: (field: string, value: unknown) => void;
}

export function SidebarProperties({ task, compact, onUpdate }: SidebarPropertiesProps) {
  return (
    <div className="px-4 py-3 space-y-1">
      <PropertyRow label="Status">
        <StatusProperty task={task} onUpdate={onUpdate} />
      </PropertyRow>

      <PropertyRow label="Priority">
        <PriorityProperty task={task} onUpdate={onUpdate} />
      </PropertyRow>

      <PropertyRow label="Energy">
        <EnergyProperty task={task} onUpdate={onUpdate} />
      </PropertyRow>

      <PropertyRow label="Due">
        <span className="text-sm text-[hsl(var(--foreground))]">
          {task.dueDate ? formatDate(task.dueDate.split("T")[0]) : "No due date"}
        </span>
      </PropertyRow>

      <PropertyRow label="Estimate">
        <EstimateProperty task={task} onUpdate={onUpdate} />
      </PropertyRow>

      {!compact && (
        <>
          <PropertyRow label="Type">
            <TypeProperty task={task} onUpdate={onUpdate} />
          </PropertyRow>

          <PropertyRow label="Area">
            <span className="text-sm text-[hsl(var(--foreground))]">{task.area.name}</span>
          </PropertyRow>

          <PropertyRow label="Project">
            <span className="text-sm text-[hsl(var(--foreground))]">
              {task.project?.name ?? "No project"}
            </span>
          </PropertyRow>

          <PropertyRow label="Tags">
            <div className="flex flex-wrap gap-1">
              {task.tags.length > 0 ? (
                task.tags.map((tag) => (
                  <span
                    key={tag}
                    className="text-xs px-1.5 py-0.5 rounded bg-[hsl(var(--accent))] text-[hsl(var(--foreground))]"
                  >
                    {tag}
                  </span>
                ))
              ) : (
                <span className="text-sm text-[hsl(var(--muted-foreground))]">None</span>
              )}
            </div>
          </PropertyRow>
        </>
      )}
    </div>
  );
}

// ── Layout ──────────────────────────────────────────────────────────

function PropertyRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 min-h-[28px]">
      <span className="text-xs text-[hsl(var(--muted-foreground))] w-[72px] shrink-0">
        {label}
      </span>
      <div className="flex-1 min-w-0">{children}</div>
    </div>
  );
}

// ── Status ──────────────────────────────────────────────────────────

function StatusProperty({
  task,
  onUpdate,
}: { task: MockDetailTask; onUpdate: (f: string, v: unknown) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center gap-1.5 text-sm text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]/50 rounded px-1 -mx-1 transition-colors"
        >
          {renderStatusIcon(task.status.id)}
          {task.status.name}
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
                    onUpdate("status", s);
                    setOpen(false);
                  }}
                >
                  <span className="mr-2 flex items-center">{renderStatusIcon(s.id)}</span>
                  {s.name}
                  <Check
                    className={cn(
                      "ml-auto h-4 w-4",
                      task.status.id === s.id ? "opacity-100" : "opacity-0",
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

// ── Priority ────────────────────────────────────────────────────────

function PriorityProperty({
  task,
  onUpdate,
}: { task: MockDetailTask; onUpdate: (f: string, v: unknown) => void }) {
  const [open, setOpen] = useState(false);
  const PriorityIcon = task.priority.icon;
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex items-center gap-1.5 text-sm text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]/50 rounded px-1 -mx-1 transition-colors"
        >
          <PriorityIcon className="size-3.5 text-[hsl(var(--muted-foreground))]" />
          {task.priority.name}
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[200px] p-0" align="start">
        <Command>
          <CommandInput placeholder="Set priority..." />
          <CommandList>
            <CommandEmpty>No priority found.</CommandEmpty>
            <CommandGroup>
              {priorities.map((pr) => {
                const Icon = pr.icon;
                return (
                  <CommandItem
                    key={pr.id}
                    value={pr.name}
                    onSelect={() => {
                      onUpdate("priority", pr);
                      setOpen(false);
                    }}
                  >
                    <Icon className="mr-2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
                    {pr.name}
                    <Check
                      className={cn(
                        "ml-auto h-4 w-4",
                        task.priority.id === pr.id ? "opacity-100" : "opacity-0",
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

// ── Energy ──────────────────────────────────────────────────────────

const energyOptions: { id: EnergyLevel; label: string }[] = [
  { id: "low", label: "Low" },
  { id: "medium", label: "Medium" },
  { id: "high", label: "High" },
  { id: "deep", label: "Deep" },
];

function EnergyProperty({
  task,
  onUpdate,
}: { task: MockDetailTask; onUpdate: (f: string, v: unknown) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="text-sm text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]/50 rounded px-1 -mx-1 transition-colors capitalize"
        >
          {task.energyLevel ?? "Not set"}
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[160px] p-0" align="start">
        <Command>
          <CommandList>
            <CommandGroup>
              {energyOptions.map((e) => (
                <CommandItem
                  key={e.id}
                  value={e.label}
                  onSelect={() => {
                    onUpdate("energyLevel", e.id);
                    setOpen(false);
                  }}
                >
                  {e.label}
                  <Check
                    className={cn(
                      "ml-auto h-4 w-4",
                      task.energyLevel === e.id ? "opacity-100" : "opacity-0",
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

// ── Type ────────────────────────────────────────────────────────────

const typeOptions: { id: TaskType; label: string }[] = [
  { id: "manual", label: "Manual" },
  { id: "agentic", label: "Agentic" },
  { id: "hybrid", label: "Hybrid" },
];

function TypeProperty({
  task,
  onUpdate,
}: { task: MockDetailTask; onUpdate: (f: string, v: unknown) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="text-sm text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]/50 rounded px-1 -mx-1 transition-colors capitalize"
        >
          {task.taskType}
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[160px] p-0" align="start">
        <Command>
          <CommandList>
            <CommandGroup>
              {typeOptions.map((t) => (
                <CommandItem
                  key={t.id}
                  value={t.label}
                  onSelect={() => {
                    onUpdate("taskType", t.id);
                    setOpen(false);
                  }}
                >
                  {t.label}
                  <Check
                    className={cn(
                      "ml-auto h-4 w-4",
                      task.taskType === t.id ? "opacity-100" : "opacity-0",
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

// ── Estimate ────────────────────────────────────────────────────────

function EstimateProperty({
  task,
  onUpdate,
}: { task: MockDetailTask; onUpdate: (f: string, v: unknown) => void }) {
  const display = task.estimatedMinutes
    ? `${Math.floor(task.estimatedMinutes / 60)}h ${task.estimatedMinutes % 60}m`
    : "No estimate";

  return (
    <span className="text-sm text-[hsl(var(--foreground))]">{display}</span>
  );
}
```

Note: Check if `renderStatusIcon` exists in `lib/status-utils.ts`. If not, inline it or create it. The `StatusSelector.tsx` uses it, so it likely exists.

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -10`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/SidebarProperties.tsx
git commit -m "feat(tasks2): implement Linear-style property rows with dropdowns"
```

---

### Task 10: SidebarWorkState — live focus session

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx`

- [ ] **Step 1: Implement the work state section**

```tsx
// desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx
import { Pause, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { formatElapsed } from "@shared/lib/dates";
import { cn } from "../../lib/utils";
import type { MockDetailTask, MockFocusSession, TaskState } from "../../mock-data/issue-detail";

interface SidebarWorkStateProps {
  task: MockDetailTask;
  taskState: TaskState;
  focusSession: MockFocusSession | null;
}

export function SidebarWorkState({ task, taskState, focusSession }: SidebarWorkStateProps) {
  if (taskState === "completed") {
    return (
      <div className="px-4 py-3">
        <SectionLabel>Session Summary</SectionLabel>
        <p className="text-sm text-[hsl(var(--muted-foreground))] mt-1">
          Total tracked: {formatElapsed(task.totalTrackedSecs)}
        </p>
      </div>
    );
  }

  if (!focusSession) return null;

  return (
    <div className="px-4 py-3 space-y-3">
      <SectionLabel>Work State</SectionLabel>

      {/* Timer */}
      <FocusTimer focusedAt={task.focusedAt!} />

      {/* Focus mode badge */}
      <div className="flex items-center justify-between">
        <span className="text-xs text-[hsl(var(--muted-foreground))]">Mode</span>
        <span className="text-xs px-1.5 py-0.5 rounded bg-[hsl(var(--accent))] text-[hsl(var(--foreground))] capitalize">
          {focusSession.focusMode.replace("-", " ")}
        </span>
      </div>

      {/* Quality score */}
      <div className="flex items-center justify-between">
        <span className="text-xs text-[hsl(var(--muted-foreground))]">Quality</span>
        <span
          className={cn(
            "text-sm font-mono tabular-nums",
            focusSession.qualityScore > 0.7
              ? "text-green-400"
              : focusSession.qualityScore > 0.4
                ? "text-amber-400"
                : "text-red-400",
          )}
        >
          {focusSession.qualityScore.toFixed(2)}
        </span>
      </div>

      {/* Distractions */}
      <div className="flex items-center justify-between">
        <span className="text-xs text-[hsl(var(--muted-foreground))]">Distractions</span>
        <span className="text-sm text-[hsl(var(--foreground))]">
          {focusSession.distractionCount}
        </span>
      </div>

      {/* Flow state */}
      <div className="flex items-center justify-between">
        <span className="text-xs text-[hsl(var(--muted-foreground))]">Flow</span>
        <FlowBadge state={focusSession.flowState} />
      </div>

      {/* Sparkline */}
      <QualitySparkline values={focusSession.qualityHistory} />

      {/* Controls */}
      <div className="flex gap-2">
        <button
          type="button"
          className="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs rounded border border-[hsl(var(--border))] text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]/50 transition-colors"
        >
          <Pause className="size-3" />
          Pause
        </button>
        <button
          type="button"
          className="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs rounded border border-[hsl(var(--border))] text-red-400 hover:bg-red-500/10 transition-colors"
        >
          <Square className="size-3" />
          Stop
        </button>
      </div>
    </div>
  );
}

// ── Sub-components ──────────────────────────────────────────────────

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
      {children}
    </span>
  );
}

function FocusTimer({ focusedAt }: { focusedAt: string }) {
  const [elapsed, setElapsed] = useState(() =>
    Math.floor((Date.now() - new Date(focusedAt).getTime()) / 1000),
  );

  useEffect(() => {
    const interval = setInterval(() => {
      setElapsed(Math.floor((Date.now() - new Date(focusedAt).getTime()) / 1000));
    }, 1000);
    return () => clearInterval(interval);
  }, [focusedAt]);

  return (
    <div className="text-2xl font-mono tabular-nums text-[hsl(var(--foreground))] text-center">
      {formatElapsed(elapsed)}
    </div>
  );
}

function FlowBadge({ state }: { state: string }) {
  const color =
    state === "active"
      ? "text-green-400"
      : state === "building"
        ? "text-amber-400"
        : "text-red-400";

  return (
    <span className={cn("text-xs px-1.5 py-0.5 rounded bg-[hsl(var(--accent))] capitalize", color)}>
      {state}
    </span>
  );
}

function QualitySparkline({ values }: { values: number[] }) {
  const max = Math.max(...values, 1);
  return (
    <div className="flex items-end gap-px h-8">
      {values.map((v, i) => {
        const height = `${(v / max) * 100}%`;
        const color =
          v > 0.7
            ? "bg-green-400/70"
            : v > 0.4
              ? "bg-amber-400/70"
              : "bg-red-400/70";
        return (
          <div
            key={`bar-${i}`}
            className={cn("flex-1 rounded-sm", color)}
            style={{ height }}
          />
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/SidebarWorkState.tsx
git commit -m "feat(tasks2): implement SidebarWorkState with live timer, quality sparkline, and controls"
```

---

### Task 11: SidebarTime — progress tracking

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarTime.tsx`

- [ ] **Step 1: Implement the time section**

```tsx
// desktop-ui/src/features/tasks2/components/detail/SidebarTime.tsx
import { formatHumanDuration } from "@shared/lib/dates";
import { cn } from "../../lib/utils";
import type { MockDetailTask, TaskState } from "../../mock-data/issue-detail";

interface SidebarTimeProps {
  task: MockDetailTask;
  taskState: TaskState;
  onUpdate: (field: string, value: unknown) => void;
}

export function SidebarTime({ task, taskState }: SidebarTimeProps) {
  const estimatedSecs = (task.estimatedMinutes ?? 0) * 60;
  const trackedSecs = task.totalTrackedSecs;
  const forecastSecs = estimatedSecs > 0 ? Math.round(estimatedSecs * 1.1) : 0; // Mock: 10% buffer

  if (taskState === "new") {
    return (
      <div className="px-4 py-3">
        <SectionLabel>Time</SectionLabel>
        <div className="mt-2 text-sm text-[hsl(var(--foreground))]">
          {task.estimatedMinutes
            ? `Estimate: ${formatHumanDuration(estimatedSecs)}`
            : "No estimate"}
        </div>
      </div>
    );
  }

  if (taskState === "completed") {
    const deviation = estimatedSecs > 0
      ? Math.round(((trackedSecs - estimatedSecs) / estimatedSecs) * 100)
      : 0;
    return (
      <div className="px-4 py-3">
        <SectionLabel>Time — Final</SectionLabel>
        <div className="mt-2 space-y-1">
          <TimeRow label="Estimated" value={estimatedSecs > 0 ? formatHumanDuration(estimatedSecs) : "—"} />
          <TimeRow label="Actual" value={formatHumanDuration(trackedSecs)} />
          {estimatedSecs > 0 && (
            <div className={cn(
              "text-xs mt-1",
              deviation > 0 ? "text-red-400" : "text-green-400",
            )}>
              {deviation > 0 ? `${deviation}% over estimate` : `${Math.abs(deviation)}% under estimate`}
            </div>
          )}
        </div>
      </div>
    );
  }

  // focused or has-history
  const ratio = estimatedSecs > 0 ? trackedSecs / estimatedSecs : 0;
  const percentage = Math.round(ratio * 100);
  const barWidth = Math.min(percentage, 100);
  const barColor =
    ratio < 0.8 ? "bg-green-400" : ratio < 1.0 ? "bg-amber-400" : "bg-red-400";

  const statusText =
    ratio < 1.0
      ? `${percentage}% · ahead of schedule`
      : `${percentage}% · over estimate`;

  return (
    <div className="px-4 py-3">
      <SectionLabel>Time</SectionLabel>
      <div className="mt-2 space-y-1">
        <TimeRow label="Estimated" value={estimatedSecs > 0 ? formatHumanDuration(estimatedSecs) : "—"} />
        <TimeRow label="Tracked" value={formatHumanDuration(trackedSecs)} />
        <TimeRow label="Forecast" value={forecastSecs > 0 ? formatHumanDuration(forecastSecs) : "—"} />

        {/* Progress bar */}
        {estimatedSecs > 0 && (
          <>
            <div className="h-1.5 rounded-full bg-[hsl(var(--accent))] mt-2 overflow-hidden">
              <div
                className={cn("h-full rounded-full transition-all", barColor)}
                style={{ width: `${barWidth}%` }}
              />
            </div>
            <div className="text-xs text-[hsl(var(--muted-foreground))] mt-1">{statusText}</div>
          </>
        )}
      </div>
    </div>
  );
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
      {children}
    </span>
  );
}

function TimeRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-[hsl(var(--muted-foreground))]">{label}</span>
      <span className="text-sm text-[hsl(var(--foreground))] font-mono tabular-nums">{value}</span>
    </div>
  );
}
```

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/SidebarTime.tsx
git commit -m "feat(tasks2): implement SidebarTime with progress bar and time tracking"
```

---

### Task 12: SidebarAiInsights — suggestions, memory, contextual content

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/SidebarAiInsights.tsx`

- [ ] **Step 1: Implement the AI insights section**

```tsx
// desktop-ui/src/features/tasks2/components/detail/SidebarAiInsights.tsx
import { Bot, ChevronDown, Sparkles, Zap } from "lucide-react";
import { useState } from "react";
import { cn } from "../../lib/utils";
import type { MockSuggestion, MockTaskMemory, TaskState } from "../../mock-data/issue-detail";

interface SidebarAiInsightsProps {
  taskState: TaskState;
  suggestions: MockSuggestion[];
  taskMemory: MockTaskMemory;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}

export function SidebarAiInsights({
  taskState,
  suggestions,
  taskMemory,
  onApply,
  onDismiss,
}: SidebarAiInsightsProps) {
  return (
    <div className="px-4 py-3 space-y-4">
      <SectionLabel>AI Insights</SectionLabel>

      {taskState === "completed" ? (
        <WhatAiLearned memory={taskMemory} />
      ) : taskState === "new" && suggestions.filter((s) => s.status === "pending").length === 0 ? (
        <WhyThisTaskNow />
      ) : (
        <SuggestionsList
          suggestions={suggestions}
          onApply={onApply}
          onDismiss={onDismiss}
        />
      )}

      {taskState !== "completed" && taskState !== "new" && (
        <TaskMemorySection memory={taskMemory} />
      )}
    </div>
  );
}

// ── Sub-components ──────────────────────────────────────────────────

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
      {children}
    </span>
  );
}

function SuggestionsList({
  suggestions,
  onApply,
  onDismiss,
}: {
  suggestions: MockSuggestion[];
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const pending = suggestions.filter((s) => s.status === "pending");
  const top = pending[0];
  const rest = pending.slice(1);

  if (!top) return null;

  return (
    <div className="space-y-3">
      <SuggestionCard suggestion={top} onApply={onApply} onDismiss={onDismiss} />

      {rest.length > 0 && !expanded && (
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="text-xs text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors flex items-center gap-1"
        >
          <ChevronDown className="size-3" />
          See all ({rest.length} more)
        </button>
      )}

      {expanded &&
        rest.map((s) => (
          <SuggestionCard key={s.id} suggestion={s} onApply={onApply} onDismiss={onDismiss} />
        ))}
    </div>
  );
}

function SuggestionCard({
  suggestion,
  onApply,
  onDismiss,
}: {
  suggestion: MockSuggestion;
  onApply: (id: string) => void;
  onDismiss: (id: string) => void;
}) {
  return (
    <div className="rounded-md border border-[hsl(var(--border))] p-3 space-y-2">
      <div className="flex items-start gap-2">
        <Sparkles className="size-3.5 text-purple-400 shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-[hsl(var(--foreground))]">
              {suggestion.title}
            </span>
            <span className="text-[10px] px-1 py-0.5 rounded bg-purple-500/20 text-purple-300 shrink-0">
              {Math.round(suggestion.confidence * 100)}%
            </span>
          </div>
          <p className="text-xs text-[hsl(var(--muted-foreground))] mt-0.5 line-clamp-2">
            {suggestion.description}
          </p>
        </div>
      </div>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onApply(suggestion.id)}
          className="text-xs px-2 py-1 rounded bg-purple-500/20 text-purple-300 hover:bg-purple-500/30 transition-colors"
        >
          Apply
        </button>
        <button
          type="button"
          onClick={() => onDismiss(suggestion.id)}
          className="text-xs px-2 py-1 rounded text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))] transition-colors"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}

function WhyThisTaskNow() {
  const reasons = [
    { icon: Zap, text: "High priority — P1" },
    { icon: Zap, text: "Due in 2 days" },
    { icon: Zap, text: "Matches your current energy window" },
  ];

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-[hsl(var(--foreground))]">
        <Bot className="size-3.5 text-purple-400" />
        Why This Task Now?
      </div>
      <div className="space-y-1.5">
        {reasons.map((r) => (
          <div key={r.text} className="flex items-center gap-2 text-xs text-[hsl(var(--muted-foreground))]">
            <r.icon className="size-3 text-purple-400/60 shrink-0" />
            {r.text}
          </div>
        ))}
      </div>
    </div>
  );
}

function WhatAiLearned({ memory }: { memory: MockTaskMemory }) {
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-1.5 text-sm font-medium text-[hsl(var(--foreground))]">
        <Bot className="size-3.5 text-purple-400" />
        What AI Learned
      </div>
      <p className="text-xs text-[hsl(var(--muted-foreground))]">
        {memory.lastSessionSummary}
      </p>
      {memory.relatedFacts.map((fact) => (
        <div key={fact} className="flex items-start gap-1.5 text-xs text-[hsl(var(--muted-foreground))]">
          <span className="text-purple-400/60 shrink-0">•</span>
          {fact}
        </div>
      ))}
    </div>
  );
}

function TaskMemorySection({ memory }: { memory: MockTaskMemory }) {
  return (
    <div className="space-y-2 pt-2 border-t border-[hsl(var(--border))]/50">
      <span className="text-[10px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
        Task Memory
      </span>
      <p className="text-xs text-[hsl(var(--muted-foreground))]">
        {memory.lastSessionSummary}
      </p>
      {memory.continuityNote && (
        <p className="text-xs text-[hsl(var(--muted-foreground))] italic">
          {memory.continuityNote}
        </p>
      )}
      {memory.relatedFacts.length > 0 && (
        <div className="space-y-0.5">
          {memory.relatedFacts.map((fact) => (
            <div key={fact} className="flex items-start gap-1.5 text-xs text-[hsl(var(--muted-foreground))]">
              <span className="text-purple-400/60 shrink-0">•</span>
              {fact}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/SidebarAiInsights.tsx
git commit -m "feat(tasks2): implement SidebarAiInsights with suggestions, memory, and contextual content"
```

---

## Chunk 4: Polish — Responsive Collapse, Lint, Final Verification

### Task 13: Responsive sidebar collapse with @container

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/detail/IssueDetailView.tsx`

- [ ] **Step 1: Add container query CSS and auto-collapse logic**

Update `IssueDetailView.tsx` to add `container-type: inline-size` and use a `useEffect` to auto-collapse the sidebar when container width drops below 900px. Since CSS `@container` queries work at the CSS level, and we want JS state sync, use a `ResizeObserver`:

```tsx
// Add to IssueDetailView.tsx — replace the container div and add auto-collapse
import { PanelRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useIssueDetail } from "../../hooks/useIssueDetail";
import { IssueDetailSidebar } from "./IssueDetailSidebar";
import { IssueDetailTabs } from "./IssueDetailTabs";
import { IssueDetailTitle } from "./IssueDetailTitle";

interface IssueDetailViewProps {
  issueId: string;
}

export function IssueDetailView({ issueId }: IssueDetailViewProps) {
  const detail = useIssueDetail(issueId);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [manualOverride, setManualOverride] = useState<boolean | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Auto-collapse below 900px, but respect manual override
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new ResizeObserver((entries) => {
      const width = entries[0]?.contentRect.width ?? 0;
      if (manualOverride === null) {
        setSidebarOpen(width >= 900);
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [manualOverride]);

  const toggleSidebar = () => {
    setManualOverride(!sidebarOpen ? true : false);
    setSidebarOpen(!sidebarOpen);
  };

  return (
    <div ref={containerRef} className="flex h-full relative">
      {/* Left column */}
      <div className="flex-1 min-w-0 overflow-y-auto px-6 py-4">
        <IssueDetailTitle
          title={detail.task.title}
          onUpdate={(title) => detail.updateTask("title", title)}
        />
        <IssueDetailTabs detail={detail} />
      </div>

      {/* Sidebar toggle */}
      {!sidebarOpen && (
        <button
          type="button"
          onClick={toggleSidebar}
          className="absolute top-3 right-3 p-1.5 rounded hover:bg-[hsl(var(--accent))] text-[hsl(var(--muted-foreground))] z-10"
          aria-label="Show sidebar"
        >
          <PanelRight className="size-4" />
        </button>
      )}

      {/* Right column */}
      {sidebarOpen && (
        <IssueDetailSidebar detail={detail} onClose={toggleSidebar} />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Verify build**

Run: `cd desktop-ui && bun run build 2>&1 | tail -5`
Expected: Build succeeds

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/detail/IssueDetailView.tsx
git commit -m "feat(tasks2): add responsive sidebar collapse with ResizeObserver"
```

---

### Task 14: Lint, format, final verification

- [ ] **Step 1: Run Biome lint + format**

Run: `cd desktop-ui && bun run lint:fix`
Expected: Auto-fixes applied, no remaining errors

- [ ] **Step 2: Run TypeScript check**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -20`
Expected: No type errors

- [ ] **Step 3: Run tests**

Run: `cd desktop-ui && bun run test -- --run`
Expected: All tests pass (including the `deriveTaskState` tests from Task 2)

- [ ] **Step 4: Run build**

Run: `cd desktop-ui && bun run build`
Expected: Production build succeeds

- [ ] **Step 5: Verify sidebar state branches visually**

Temporarily modify `mockDetailTask` in `issue-detail.ts` to test each `taskState`:
- Set `focusedAt: null`, `totalTrackedSecs: 0`, `completed: false`, `estimatedMinutes: null` → should show `"new"` state with SidebarTime hidden
- Set `estimatedMinutes: 240` (keep others null/0) → should show SidebarTime with estimate only
- Restore original mock data when done

- [ ] **Step 6: Fix any issues found in steps 1-5**

Address lint errors, type errors, or test failures. Common issues:
- Import path aliases (`@shared/`, `@features/`) — may need relative paths
- Missing `React` import (if JSX transform doesn't auto-inject)
- Unused imports flagged by Biome

- [ ] **Step 7: Final commit**

```bash
git add -A desktop-ui/src/features/tasks2/
git commit -m "chore(tasks2): lint and format task detail view components"
```

---

## Notes for Implementation

### Path alias verification
Before writing any imports, check which aliases exist:
```bash
grep -A5 'paths\|alias' desktop-ui/tsconfig.json desktop-ui/vite.config.ts 2>/dev/null
```

Common patterns in this project:
- `@shared/` → `src/shared/`
- `@features/` → `src/features/`
- If aliases don't exist, use relative paths (e.g., `../../../../shared/lib/dates`)

### renderStatusIcon
The `StatusSelector.tsx` imports `renderStatusIcon` from `../../lib/status-utils`. Verify it exists:
```bash
cat desktop-ui/src/features/tasks2/lib/status-utils.ts
```
If it doesn't exist, check how `StatusSelector` renders status icons and follow the same pattern.

### EditorCore side effects
When importing `useNoteEditor`, it will fire `useEntityResolution` internally which calls `task_list`, `project_list`, `note_list` IPC commands. In browser dev mode without `cargo tauri dev` running, these will fail silently. This is acceptable for UI development — the editor will work, just entity resolution won't highlight unresolved references.
