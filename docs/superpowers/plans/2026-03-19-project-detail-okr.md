# Project Detail Page + OKR System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Project Detail Page with 4 tabs (Overview, Tasks, OKR, Notes) that transforms projects from task folders into intelligent "second brain" containers with full OKR CRUD.

**Architecture:** New `/project/:id` route with lazy-loaded tab components. Page-level `ProjectContext` provides project + objective data. Each tab fetches its own data independently. Glass aesthetic throughout. All backend commands already exist — this is purely frontend work (with one optional backend enhancement).

**Tech Stack:** React 19, React Router v7 (HashRouter), Tailwind v4, dnd-kit, TipTap, Cytoscape, Zustand, Vitest

**Spec:** `docs/superpowers/specs/2026-03-19-project-detail-okr-design.md`

---

## File Structure

### New files (create)

```
desktop-ui/src/features/projects/
├── index.ts                                    # Re-exports ProjectDetailPage
├── pages/
│   └── ProjectDetailPage.tsx                   # Route entry, ProjectContext provider, layout shell
├── components/
│   ├── ProjectHeader.tsx                       # Sticky header: back, name, area, health ring, actions
│   ├── GlassTabBar.tsx                         # Draggable glass tab bar with badges + indicators
│   ├── QuickAddFAB.tsx                         # Split button FAB (Task/Note/Objective)
│   ├── overview/
│   │   ├── OverviewTab.tsx                     # 3-row grid: stats, intelligence, timeline
│   │   ├── HealthScoreCard.tsx                 # Health Score gradient ring card
│   │   ├── TaskProgressCard.tsx                # Task counts + due today/overdue
│   │   ├── OkrSummaryCard.tsx                  # Top objectives with progress bars
│   │   ├── WorkContextCard.tsx                 # Active work context intelligence card
│   │   ├── InsightCard.tsx                     # Latest insight intelligence card
│   │   ├── CoachingCard.tsx                    # Coaching signal intelligence card
│   │   └── ActivityTimeline.tsx                # Smart-grouped recent activity
│   ├── okr/
│   │   ├── OkrTab.tsx                          # OKR tree container with filter + create
│   │   ├── ObjectiveCard.tsx                   # Collapsible objective with KR children
│   │   ├── KeyResultRow.tsx                    # KR with inline metric edit + linked tasks
│   │   ├── LinkedTasksList.tsx                 # Expandable task list under KR
│   │   ├── ObjectiveCreateModal.tsx            # Create/edit objective modal
│   │   └── KeyResultCreateForm.tsx             # Inline KR creation form
│   ├── tasks/
│   │   └── ProjectTasksTab.tsx                 # Adapted kanban scoped to project
│   └── notes/
│       ├── ProjectNotesTab.tsx                 # Split view: sidebar + editor
│       ├── NoteSidebar.tsx                     # Notebook tree + search + mini graph
│       └── NoteActionBar.tsx                   # Generate Insight, Link KR, Create Task, Flashcards
├── hooks/
│   ├── useProject.ts                           # useQuery("project_get") wrapper
│   ├── useProjectObjectives.ts                 # useQuery("objective_list") wrapper
│   ├── useProjectTasks.ts                      # useQuery("task_list") scoped to project
│   ├── useProjectNotes.ts                      # Multi-notebook note_list + merge
│   ├── useHealthScore.ts                       # Client-side health score computation
│   └── useProjectTabOrder.ts                   # Read/write tab order to project.settings
├── contexts/
│   └── ProjectContext.tsx                      # Project + objectives context provider
├── lib/
│   ├── health-score.ts                         # Health score formula + breakdown
│   └── mappers.ts                              # Type conversions and helpers
└── store/
    └── project-detail-store.ts                 # Zustand: expanded objectives, active note, etc.
```

### New shared components (create)

```
desktop-ui/src/shared/ui/ProgressRing.tsx       # SVG circular progress with gradient + size variants
```

### Existing files (modify)

```
desktop-ui/src/app/router.tsx                   # Add /project/:id routes
desktop-ui/src/app/layouts/Sidebar.tsx           # Add Projects nav section
desktop-ui/src/shared/types/tasks.ts            # Add missing mutation param types if needed
desktop-ui/src/shared/types/index.ts            # Re-export new types
```

---

## Task Breakdown

### Task 1: ProgressRing Shared Component

**Files:**
- Create: `desktop-ui/src/shared/ui/ProgressRing.tsx`
- Create: `desktop-ui/src/shared/ui/__tests__/ProgressRing.test.tsx`
- Modify: `desktop-ui/src/shared/ui/index.ts` (add re-export)

- [ ] **Step 1: Write the test**

```tsx
// desktop-ui/src/shared/ui/__tests__/ProgressRing.test.tsx
import { render, screen } from "@testing-library/react";
import { ProgressRing } from "../ProgressRing";

describe("ProgressRing", () => {
  it("renders with correct progress percentage", () => {
    render(<ProgressRing progress={73} size="md" />);
    expect(screen.getByText("73%")).toBeInTheDocument();
  });

  it("renders small size without text", () => {
    const { container } = render(<ProgressRing progress={50} size="sm" />);
    expect(container.querySelector("svg")).toHaveAttribute("width", "28");
    expect(screen.queryByText("50%")).not.toBeInTheDocument();
  });

  it("applies gradient when gradient prop is true", () => {
    const { container } = render(<ProgressRing progress={80} size="md" gradient />);
    expect(container.querySelector("linearGradient")).toBeInTheDocument();
  });

  it("uses custom color", () => {
    const { container } = render(<ProgressRing progress={60} size="md" color="#f59e0b" />);
    const circle = container.querySelectorAll("circle")[1];
    expect(circle).toHaveAttribute("stroke", "#f59e0b");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test -- --run src/shared/ui/__tests__/ProgressRing.test.tsx`
Expected: FAIL — module not found

- [ ] **Step 3: Implement ProgressRing**

```tsx
// desktop-ui/src/shared/ui/ProgressRing.tsx
interface ProgressRingProps {
  progress: number;
  size: "sm" | "md" | "lg";
  color?: string;
  gradient?: boolean;
  className?: string;
}

const SIZES = { sm: 28, md: 48, lg: 80 } as const;
const STROKE = { sm: 2, md: 3, lg: 3.5 } as const;
const RADIUS = 15.5;
// pathLength="100" on the SVG circle means strokeDasharray works as direct percentages

export function ProgressRing({ progress, size, color, gradient, className }: ProgressRingProps) {
  const px = SIZES[size];
  const sw = STROKE[size];
  const clamped = Math.max(0, Math.min(100, progress));
  const dasharray = `${clamped} ${100 - clamped}`;
  const gradientId = `pr-grad-${size}`;

  const strokeColor = gradient ? `url(#${gradientId})` : (color ?? "var(--brand)");

  return (
    <svg width={px} height={px} viewBox="0 0 36 36" className={className}>
      {gradient && (
        <defs>
          <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#10b981" />
            <stop offset="100%" stopColor="#6366f1" />
          </linearGradient>
        </defs>
      )}
      <circle
        cx="18" cy="18" r={RADIUS}
        fill="none"
        stroke="rgba(255,255,255,0.06)"
        strokeWidth={sw}
      />
      <circle
        cx="18" cy="18" r={RADIUS}
        fill="none"
        stroke={strokeColor}
        strokeWidth={sw}
        strokeDasharray={dasharray}
        strokeLinecap="round"
        transform="rotate(-90 18 18)"
        pathLength="100"
      />
      {size !== "sm" && (
        <text
          x="18" y={size === "lg" ? 19 : 20}
          textAnchor="middle"
          fill="currentColor"
          fontSize={size === "lg" ? 8 : 9}
          fontWeight="700"
        >
          {Math.round(clamped)}%
        </text>
      )}
    </svg>
  );
}
```

- [ ] **Step 4: Add re-export in `desktop-ui/src/shared/ui/index.ts`**

Add: `export { ProgressRing } from "./ProgressRing";`

- [ ] **Step 5: Run test to verify it passes**

Run: `cd desktop-ui && bun run test -- --run src/shared/ui/__tests__/ProgressRing.test.tsx`
Expected: PASS (4 tests)

- [ ] **Step 6: Lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/shared/ui/ProgressRing.tsx desktop-ui/src/shared/ui/__tests__/ProgressRing.test.tsx desktop-ui/src/shared/ui/index.ts
git commit -m "feat(ui): add ProgressRing shared component with gradient + size variants"
```

---

### Task 2: Health Score Computation Library

**Files:**
- Create: `desktop-ui/src/features/projects/lib/health-score.ts`
- Create: `desktop-ui/src/features/projects/lib/__tests__/health-score.test.ts`

- [ ] **Step 1: Write the test**

```ts
// desktop-ui/src/features/projects/lib/__tests__/health-score.test.ts
import { computeHealthScore, type HealthScoreInput } from "../health-score";

describe("computeHealthScore", () => {
  it("computes weighted score from all factors", () => {
    const input: HealthScoreInput = {
      okrProgress: 0.8,
      taskVelocity: 0.6,
      insightFreshness: 1.0,
      focusQuality: 0.9,
    };
    // 0.8*0.60 + 0.6*0.20 + 1.0*0.10 + 0.9*0.10 = 0.48 + 0.12 + 0.10 + 0.09 = 0.79
    const result = computeHealthScore(input);
    expect(result.score).toBeCloseTo(79, 0);
    expect(result.color).toBe("green");
    expect(result.breakdown).toHaveLength(4);
  });

  it("returns yellow for mid-range scores", () => {
    const input: HealthScoreInput = {
      okrProgress: 0.5,
      taskVelocity: 0.4,
      insightFreshness: 0.5,
      focusQuality: 0.3,
    };
    const result = computeHealthScore(input);
    expect(result.color).toBe("yellow");
  });

  it("returns red for low scores", () => {
    const input: HealthScoreInput = {
      okrProgress: 0.2,
      taskVelocity: 0.1,
      insightFreshness: 0.0,
      focusQuality: 0.1,
    };
    const result = computeHealthScore(input);
    expect(result.color).toBe("red");
  });

  it("clamps all inputs to 0-1 range", () => {
    const input: HealthScoreInput = {
      okrProgress: 1.5,
      taskVelocity: -0.2,
      insightFreshness: 2.0,
      focusQuality: 1.0,
    };
    const result = computeHealthScore(input);
    expect(result.score).toBeLessThanOrEqual(100);
    expect(result.score).toBeGreaterThanOrEqual(0);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd desktop-ui && bun run test -- --run src/features/projects/lib/__tests__/health-score.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Implement health-score.ts**

```ts
// desktop-ui/src/features/projects/lib/health-score.ts
export interface HealthScoreInput {
  okrProgress: number;     // 0-1: weighted avg of KR progress
  taskVelocity: number;    // 0-1: completed / total tasks in 7 days
  insightFreshness: number; // 0-1: 1.0 if < 7 days, linear decay
  focusQuality: number;    // 0-1: avg productivity %
}

export interface HealthScoreBreakdown {
  label: string;
  weight: number;
  value: number;
  contribution: number;
}

export interface HealthScoreResult {
  score: number;           // 0-100
  color: "green" | "yellow" | "red";
  breakdown: HealthScoreBreakdown[];
}

const WEIGHTS = [
  { key: "okrProgress" as const, label: "OKR Progress", weight: 0.60 },
  { key: "taskVelocity" as const, label: "Task Velocity", weight: 0.20 },
  { key: "insightFreshness" as const, label: "Insight Freshness", weight: 0.10 },
  { key: "focusQuality" as const, label: "Focus Quality", weight: 0.10 },
];

function clamp01(n: number): number {
  return Math.max(0, Math.min(1, n));
}

export function computeHealthScore(input: HealthScoreInput): HealthScoreResult {
  const breakdown: HealthScoreBreakdown[] = WEIGHTS.map(({ key, label, weight }) => {
    const value = clamp01(input[key]);
    return { label, weight, value, contribution: value * weight };
  });

  const raw = breakdown.reduce((sum, b) => sum + b.contribution, 0);
  const score = Math.round(raw * 100);

  let color: "green" | "yellow" | "red";
  if (score > 70) color = "green";
  else if (score >= 40) color = "yellow";
  else color = "red";

  return { score, color, breakdown };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd desktop-ui && bun run test -- --run src/features/projects/lib/__tests__/health-score.test.ts`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/projects/lib/
git commit -m "feat(projects): add health score computation with weighted formula"
```

---

### Task 3: Project Context + Data Hooks

**Files:**
- Create: `desktop-ui/src/features/projects/hooks/useProject.ts`
- Create: `desktop-ui/src/features/projects/hooks/useProjectObjectives.ts`
- Create: `desktop-ui/src/features/projects/hooks/useProjectTasks.ts`
- Create: `desktop-ui/src/features/projects/hooks/useProjectNotes.ts`
- Create: `desktop-ui/src/features/projects/hooks/useHealthScore.ts`
- Create: `desktop-ui/src/features/projects/hooks/useProjectTabOrder.ts`
- Create: `desktop-ui/src/features/projects/contexts/ProjectContext.tsx`

- [ ] **Step 1: Create useProject hook**

```ts
// desktop-ui/src/features/projects/hooks/useProject.ts
import { useQuery } from "@shared/hooks/useQuery";
import type { Project } from "@shared/types";

export function useProject(id: string) {
  return useQuery<Project>("project_get", { id });
}
```

- [ ] **Step 2: Create useProjectObjectives hook**

```ts
// desktop-ui/src/features/projects/hooks/useProjectObjectives.ts
import { useQuery } from "@shared/hooks/useQuery";
import type { Objective } from "@shared/types";

export function useProjectObjectives(projectId: string) {
  return useQuery<Objective[]>("objective_list", { projectId }, []);
}
```

- [ ] **Step 3: Create useProjectTasks hook**

```ts
// desktop-ui/src/features/projects/hooks/useProjectTasks.ts
import { useQuery } from "@shared/hooks/useQuery";
import type { Task } from "@shared/types";

export function useProjectTasks(projectId: string) {
  return useQuery<Task[]>("task_list", { projectId }, []);
}
```

- [ ] **Step 4: Create useProjectNotes hook**

```ts
// desktop-ui/src/features/projects/hooks/useProjectNotes.ts
import { useCallback, useEffect, useState } from "react";
import { ipc } from "@shared/hooks/useIpc";
import type { Note } from "@shared/types";

/**
 * Fetches notes from multiple notebooks (one call per notebook ID),
 * merges and deduplicates client-side.
 */
export function useProjectNotes(notebookIds: string[]) {
  const [data, setData] = useState<Note[]>([]);
  const [loading, setLoading] = useState(true);

  // Stable key for dependency tracking — avoids stale closure from array reference changes
  const idsKey = notebookIds.join(",");

  const fetchAll = useCallback(async () => {
    const ids = idsKey.split(",").filter(Boolean);
    if (ids.length === 0) {
      setData([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    try {
      const results = await Promise.all(
        ids.map((notebookId) => ipc<Note[]>("note_list", { notebookId })),
      );
      const merged = results.flat();
      const seen = new Set<string>();
      const deduped = merged.filter((n) => {
        if (seen.has(n.id)) return false;
        seen.add(n.id);
        return true;
      });
      deduped.sort((a, b) => (b.updatedAt ?? "").localeCompare(a.updatedAt ?? ""));
      setData(deduped);
    } finally {
      setLoading(false);
    }
  }, [idsKey]);

  useEffect(() => { fetchAll(); }, [fetchAll]);

  return { data, loading, refetch: fetchAll };
}
```

- [ ] **Step 5: Create useHealthScore hook**

```ts
// desktop-ui/src/features/projects/hooks/useHealthScore.ts
import { useMemo } from "react";
import { computeHealthScore, type HealthScoreResult } from "../lib/health-score";
import type { Objective, Task } from "@shared/types";

export function useHealthScore(
  objectives: Objective[],
  tasks: Task[],
): HealthScoreResult {
  return useMemo(() => {
    // OKR progress: weighted avg of all KR progress values
    const allKrs = objectives.flatMap((o) => o.keyResults ?? []);
    const okrProgress = allKrs.length > 0
      ? allKrs.reduce((sum, kr) => sum + kr.progress, 0) / allKrs.length / 100
      : 0;

    // Task velocity: completed in last 7 days / total active
    const total = tasks.length || 1;
    const completed = tasks.filter((t) => t.completed).length;
    const taskVelocity = completed / total;

    // Insight freshness and focus quality — placeholder for iteration 1
    // These require additional data sources (dashboard intelligence, insight cache)
    const insightFreshness = 0.5;
    const focusQuality = 0.5;

    return computeHealthScore({ okrProgress, taskVelocity, insightFreshness, focusQuality });
  }, [objectives, tasks]);
}
```

- [ ] **Step 6: Create useProjectTabOrder hook**

```ts
// desktop-ui/src/features/projects/hooks/useProjectTabOrder.ts
import { useCallback } from "react";
import { useMutation } from "@shared/hooks/useMutation";
import type { Project } from "@shared/types";

const DEFAULT_ORDER = ["overview", "tasks", "okr", "notes"];

export function useProjectTabOrder(project: Project | undefined) {
  const tabOrder = (project?.settings as Record<string, unknown>)?.tabOrder as string[] | undefined;
  const order = tabOrder ?? DEFAULT_ORDER;

  const { mutate } = useMutation<Project, ProjectUpdateParams>("project_update", "params");

  const reorder = useCallback(
    async (newOrder: string[]) => {
      if (!project) return;
      const currentSettings = (project.settings ?? {}) as Record<string, unknown>;
      await mutate({ id: project.id, settings: { ...currentSettings, tabOrder: newOrder } });
    },
    [project, mutate],
  );

  return { order, reorder };
}
```

- [ ] **Step 7: Create ProjectContext**

```tsx
// desktop-ui/src/features/projects/contexts/ProjectContext.tsx
import { createContext, useContext, type ReactNode } from "react";
import { useEvent } from "@shared/hooks/useEvent";
import type { Objective, Project } from "@shared/types";
import { useProject } from "../hooks/useProject";
import { useProjectObjectives } from "../hooks/useProjectObjectives";

interface ProjectContextValue {
  project: Project | undefined;
  objectives: Objective[];
  loading: boolean;
  refetchProject: () => void;
  refetchObjectives: () => void;
}

const Ctx = createContext<ProjectContextValue | null>(null);

export function ProjectProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const { data: project, loading: pLoading, refetch: refetchProject } = useProject(projectId);
  const { data: objectives, loading: oLoading, refetch: refetchObjectives } = useProjectObjectives(projectId);

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    const kind = payload?.entityKind;
    if (kind === "project") refetchProject();
    if (kind === "objective" || kind === "key_result") refetchObjectives();
  });

  return (
    <Ctx.Provider value={{ project, objectives, loading: pLoading || oLoading, refetchProject, refetchObjectives }}>
      {children}
    </Ctx.Provider>
  );
}

export function useProjectContext() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useProjectContext must be used within ProjectProvider");
  return ctx;
}
```

- [ ] **Step 8: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/features/projects/hooks/ desktop-ui/src/features/projects/contexts/
git commit -m "feat(projects): add data hooks and ProjectContext provider"
```

---

### Task 4: ProjectDetailPage Shell + Routing

**Files:**
- Create: `desktop-ui/src/features/projects/pages/ProjectDetailPage.tsx`
- Create: `desktop-ui/src/features/projects/components/ProjectHeader.tsx`
- Create: `desktop-ui/src/features/projects/components/GlassTabBar.tsx`
- Create: `desktop-ui/src/features/projects/components/QuickAddFAB.tsx`
- Create: `desktop-ui/src/features/projects/index.ts`
- Modify: `desktop-ui/src/app/router.tsx` — add `/project/:id` routes

- [ ] **Step 1: Create GlassTabBar component**

```tsx
// desktop-ui/src/features/projects/components/GlassTabBar.tsx
import { useNavigate } from "react-router";
import { DndContext, closestCenter, type DragEndEvent, PointerSensor, useSensor, useSensors } from "@dnd-kit/core";
import { SortableContext, horizontalListSortingStrategy, useSortable, arrayMove } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

export interface TabDef {
  id: string;
  label: string;
  badge?: string | number;
  indicatorColor?: string;
}

interface GlassTabBarProps {
  tabs: TabDef[];
  activeTab: string;
  basePath: string;
  onReorder: (newOrder: string[]) => void;
}

function SortableTab({ tab, isActive, basePath }: { tab: TabDef; isActive: boolean; basePath: string }) {
  const navigate = useNavigate();
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id: tab.id });
  const style = { transform: CSS.Transform.toString(transform), transition };

  const path = tab.id === "overview" ? basePath : `${basePath}/${tab.id}`;

  return (
    <button
      ref={setNodeRef}
      style={style}
      type="button"
      onClick={() => navigate(path)}
      className={`flex items-center gap-1.5 px-4 py-2.5 text-xs font-medium transition-colors border-b-2 -mb-px ${
        isActive
          ? "border-brand text-foreground"
          : "border-transparent text-muted-foreground hover:text-foreground"
      }`}
      {...attributes}
      {...listeners}
    >
      {tab.indicatorColor && (
        <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: tab.indicatorColor }} />
      )}
      {tab.label}
      {tab.badge != null && (
        <span className="glass-badge px-1.5 py-0.5 text-[10px] text-muted-foreground font-light">
          {tab.badge}
        </span>
      )}
    </button>
  );
}

export function GlassTabBar({ tabs, activeTab, basePath, onReorder }: GlassTabBarProps) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = tabs.findIndex((t) => t.id === active.id);
    const newIndex = tabs.findIndex((t) => t.id === over.id);
    const newOrder = arrayMove(tabs.map((t) => t.id), oldIndex, newIndex);
    onReorder(newOrder);
  }

  return (
    <div className="flex gap-0.5 px-6 glass-toolbar border-b border-border">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={tabs.map((t) => t.id)} strategy={horizontalListSortingStrategy}>
          {tabs.map((tab) => (
            <SortableTab key={tab.id} tab={tab} isActive={tab.id === activeTab} basePath={basePath} />
          ))}
        </SortableContext>
      </DndContext>
    </div>
  );
}
```

- [ ] **Step 2: Create ProjectHeader component**

```tsx
// desktop-ui/src/features/projects/components/ProjectHeader.tsx
import { useNavigate } from "react-router";
import { ArrowLeft, Archive, MoreHorizontal, Pencil, Trash2 } from "lucide-react";
import { ProgressRing } from "@shared/ui";
import { useProjectContext } from "../contexts/ProjectContext";
import { useHealthScore } from "../hooks/useHealthScore";
import { useProjectTasks } from "../hooks/useProjectTasks";

export function ProjectHeader() {
  const navigate = useNavigate();
  const { project, objectives } = useProjectContext();
  const { data: tasks } = useProjectTasks(project?.id ?? "");
  const health = useHealthScore(objectives, tasks);

  if (!project) return null;

  return (
    <div className="flex items-center gap-3 px-6 py-3 border-b border-border">
      <button type="button" onClick={() => navigate(-1)} className="text-muted-foreground hover:text-foreground">
        <ArrowLeft className="w-4 h-4" />
      </button>
      <div className="w-2.5 h-2.5 rounded-full flex-shrink-0" style={{ backgroundColor: project.color }} />
      <h1 className="text-base font-semibold text-foreground truncate">{project.name}</h1>
      {project.areaId && (
        <span className="text-[11px] px-2 py-0.5 rounded-full bg-brand/10 text-brand">{project.areaId}</span>
      )}
      <div className="ml-auto flex items-center gap-3">
        <button
          type="button"
          onClick={() => navigate(`/project/${project.id}/okr`)}
          title={`Health: ${health.score}%`}
          className="cursor-pointer"
        >
          <ProgressRing progress={health.score} size="sm" gradient />
        </button>
        {/* TODO: "Ask AI about this project" button — wires to SidebarChat */}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create QuickAddFAB component**

```tsx
// desktop-ui/src/features/projects/components/QuickAddFAB.tsx
import { useState, useRef, useEffect } from "react";
import { Plus, ChevronDown } from "lucide-react";

interface QuickAddFABProps {
  onAddTask: () => void;
  onAddNote: () => void;
  onAddObjective: () => void;
}

export function QuickAddFAB({ onAddTask, onAddNote, onAddObjective }: QuickAddFABProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  return (
    <div ref={ref} className="fixed bottom-5 right-6 z-50 flex items-center gap-px">
      <button
        type="button"
        onClick={onAddTask}
        className="flex items-center gap-1.5 px-4 py-2.5 rounded-l-lg bg-brand text-white text-xs font-medium hover:bg-brand/90 transition-colors"
      >
        <Plus className="w-3.5 h-3.5" /> Add Task
      </button>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="px-2 py-2.5 rounded-r-lg bg-brand text-white hover:bg-brand/90 transition-colors border-l border-white/20"
      >
        <ChevronDown className="w-3.5 h-3.5" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 glass-dropdown rounded-lg py-1 min-w-[160px]">
          <button type="button" onClick={() => { onAddTask(); setOpen(false); }} className="w-full px-3 py-2 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors">
            New Task
          </button>
          <button type="button" onClick={() => { onAddNote(); setOpen(false); }} className="w-full px-3 py-2 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors">
            New Note
          </button>
          <button type="button" onClick={() => { onAddObjective(); setOpen(false); }} className="w-full px-3 py-2 text-left text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors">
            New Objective
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create ProjectDetailPage**

```tsx
// desktop-ui/src/features/projects/pages/ProjectDetailPage.tsx
import { Suspense, lazy, useMemo } from "react";
import { useParams, useLocation } from "react-router";
import { Skeleton } from "@shared/ui";
import { ProjectProvider, useProjectContext } from "../contexts/ProjectContext";
import { ProjectHeader } from "../components/ProjectHeader";
import { GlassTabBar, type TabDef } from "../components/GlassTabBar";
import { QuickAddFAB } from "../components/QuickAddFAB";
import { useProjectTabOrder } from "../hooks/useProjectTabOrder";

const OverviewTab = lazy(() => import("../components/overview/OverviewTab").then((m) => ({ default: m.OverviewTab })));
const OkrTab = lazy(() => import("../components/okr/OkrTab").then((m) => ({ default: m.OkrTab })));
const ProjectTasksTab = lazy(() => import("../components/tasks/ProjectTasksTab").then((m) => ({ default: m.ProjectTasksTab })));
const ProjectNotesTab = lazy(() => import("../components/notes/ProjectNotesTab").then((m) => ({ default: m.ProjectNotesTab })));

const TAB_COMPONENTS: Record<string, React.LazyExoticComponent<React.ComponentType>> = {
  overview: OverviewTab,
  tasks: ProjectTasksTab,
  okr: OkrTab,
  notes: ProjectNotesTab,
};

function ProjectDetailInner() {
  const location = useLocation();
  const { project, objectives } = useProjectContext();
  const { order, reorder } = useProjectTabOrder(project);

  // Derive active tab from URL path
  const pathParts = location.pathname.split("/");
  const lastSegment = pathParts[pathParts.length - 1];
  const activeTab = TAB_COMPONENTS[lastSegment] ? lastSegment : "overview";

  const basePath = `/project/${project?.id ?? ""}`;

  const tabs: TabDef[] = useMemo(() => {
    const taskCount = project ? Math.max(0, project.taskCount - project.completedCount) : 0;
    const okrProgress = objectives.length > 0
      ? Math.round(objectives.reduce((s, o) => s + o.progress, 0) / objectives.length)
      : 0;
    const defs: Record<string, TabDef> = {
      overview: { id: "overview", label: "Overview" },
      tasks: { id: "tasks", label: "Tasks", badge: taskCount > 0 ? taskCount : undefined },
      okr: { id: "okr", label: "OKR", badge: `${okrProgress}%` },
      notes: { id: "notes", label: "Notes" },
    };
    return order.map((id) => defs[id]).filter(Boolean);
  }, [order, project, objectives]);

  const ActiveComponent = TAB_COMPONENTS[activeTab] ?? OverviewTab;

  return (
    <div className="flex flex-col h-full">
      <ProjectHeader />
      <GlassTabBar tabs={tabs} activeTab={activeTab} basePath={basePath} onReorder={reorder} />
      <div className="flex-1 overflow-y-auto">
        <Suspense fallback={<div className="p-6"><Skeleton className="h-48 w-full" /></div>}>
          <ActiveComponent />
        </Suspense>
      </div>
      <QuickAddFAB
        onAddTask={() => { /* TODO: open create task modal scoped to project */ }}
        onAddNote={() => { /* TODO: navigate to notes tab + create */ }}
        onAddObjective={() => { /* TODO: open create objective modal */ }}
      />
    </div>
  );
}

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  if (!id) return null;

  return (
    <ProjectProvider projectId={id}>
      <ProjectDetailInner />
    </ProjectProvider>
  );
}
```

- [ ] **Step 5: Create index.ts re-export**

```ts
// desktop-ui/src/features/projects/index.ts
export { ProjectDetailPage } from "./pages/ProjectDetailPage";
```

- [ ] **Step 6: Add routes to router.tsx**

In `desktop-ui/src/app/router.tsx`:
- Add lazy import at top of file (follow existing pattern):
  ```tsx
  const ProjectDetailPage = lazy(() =>
    import("../features/projects").then((m) => ({ default: m.ProjectDetailPage })),
  );
  ```
- Add routes **inside** the `AppShell` `children` array (around line 265, after the existing routes). They MUST be inside `AppShell` or the page renders without sidebar/nav:
  ```tsx
  { path: "/project/:id", element: <ProjectDetailPage /> },
  { path: "/project/:id/:tab", element: <ProjectDetailPage /> },
  ```

- [ ] **Step 7: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/features/projects/ desktop-ui/src/app/router.tsx
git commit -m "feat(projects): add ProjectDetailPage shell with routing, header, tab bar, FAB"
```

---

### Task 5: OKR Tab — Objective Cards + KR Rows + CRUD

**Files:**
- Create: `desktop-ui/src/features/projects/components/okr/OkrTab.tsx`
- Create: `desktop-ui/src/features/projects/components/okr/ObjectiveCard.tsx`
- Create: `desktop-ui/src/features/projects/components/okr/KeyResultRow.tsx`
- Create: `desktop-ui/src/features/projects/components/okr/LinkedTasksList.tsx`
- Create: `desktop-ui/src/features/projects/components/okr/ObjectiveCreateModal.tsx`
- Create: `desktop-ui/src/features/projects/components/okr/KeyResultCreateForm.tsx`
- Create: `desktop-ui/src/features/projects/store/project-detail-store.ts`

- [ ] **Step 1: Create Zustand store for OKR UI state**

```ts
// desktop-ui/src/features/projects/store/project-detail-store.ts
import { create } from "zustand";

interface ProjectDetailState {
  expandedObjectives: Set<string>;
  expandedKrs: Set<string>;
  toggleObjective: (id: string) => void;
  toggleKr: (id: string) => void;
}

export const useProjectDetailStore = create<ProjectDetailState>((set) => ({
  expandedObjectives: new Set(),
  expandedKrs: new Set(),
  toggleObjective: (id) =>
    set((s) => {
      const next = new Set(s.expandedObjectives);
      if (next.has(id)) next.delete(id); else next.add(id);
      return { expandedObjectives: next };
    }),
  toggleKr: (id) =>
    set((s) => {
      const next = new Set(s.expandedKrs);
      if (next.has(id)) next.delete(id); else next.add(id);
      return { expandedKrs: next };
    }),
}));
```

- [ ] **Step 2: Create KeyResultRow component**

Implement `KeyResultRow.tsx` with:
- Mini ProgressRing
- Title + current/target display
- Inline metric editing (click value → number input → blur/Enter saves via `key_result_update_metric` mutation)
- Linked tasks badge (click toggles `LinkedTasksList`)
- Context menu (Edit, Delete)

- [ ] **Step 3: Create LinkedTasksList component**

Implement `LinkedTasksList.tsx` with:
- Reads tasks from `useProjectTasks` filtered by `task.metadata?.keyResultId === kr.id`
- Shows checkboxes + title + status
- Completing a task calls `task_toggle_complete` mutation

- [ ] **Step 4: Create ObjectiveCard component**

Implement `ObjectiveCard.tsx` with:
- Collapsible card (toggle via store)
- ProgressRing (md) + title + KR count + status badge
- AI Confidence badge (computed from KR velocity: `progress / Math.max(0.01, elapsed / total)` clamped 0-100)
- Collapse/expand children KR rows
- "+ Add Key Result" button at bottom
- Context menu (Edit, Delete, Change Status)

- [ ] **Step 5: Create ObjectiveCreateModal**

Implement `ObjectiveCreateModal.tsx` with:
- Glass-panel modal
- Fields: title (required), description, priority (1-5), due date
- Calls `objective_create` mutation with `projectId` from context
- **IMPORTANT:** After successful mutation, manually call `refetchObjectives()` from `useProjectContext()`. The `useMutation` hook's `inferEntityKind` only recognises prefixes `task_`, `project_`, `note_`, `notebook_`, `inbox_` — it does NOT auto-dispatch `entity:updated` events for `objective_*` or `key_result_*` commands. Every OKR mutation must explicitly call `refetchObjectives()` after success.

- [ ] **Step 6: Create KeyResultCreateForm**

Implement `KeyResultCreateForm.tsx` with:
- Inline form (appears below last KR when "Add Key Result" clicked)
- Fields: title (required), target value, unit, tracking mode
- Calls `key_result_create` mutation with `objectiveId`

- [ ] **Step 7: Create OkrTab container**

Implement `OkrTab.tsx` with:
- Header: "Objectives" + overall progress badge + filter (All/On Track/At Risk/Achieved) + "+ New Objective"
- Maps `objectives` from `useProjectContext` → `ObjectiveCard` list
- Filter state in component (not store — local to tab)
- Empty state with dashed card

- [ ] **Step 8: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/features/projects/components/okr/ desktop-ui/src/features/projects/store/
git commit -m "feat(projects): add OKR tab with objective cards, KR rows, inline metric edit, CRUD modals"
```

---

### Task 6: Overview Tab — Stats + Intelligence Cards + Timeline

**Files:**
- Create: `desktop-ui/src/features/projects/components/overview/OverviewTab.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/HealthScoreCard.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/TaskProgressCard.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/OkrSummaryCard.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/WorkContextCard.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/InsightCard.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/CoachingCard.tsx`
- Create: `desktop-ui/src/features/projects/components/overview/ActivityTimeline.tsx`

- [ ] **Step 1: Create HealthScoreCard**

Uses `useHealthScore` + `ProgressRing` (lg, gradient). Shows score, status text ("On Track" / "Needs Attention" / "At Risk"), KR count summary. Click navigates to OKR tab.

- [ ] **Step 2: Create TaskProgressCard**

Shows `taskCount - completedCount` active tasks, completion bar, due today/overdue counts from `useProjectTasks`. Click navigates to Tasks tab.

- [ ] **Step 3: Create OkrSummaryCard**

Shows top 3 objectives from `useProjectContext` with inline progress bars. Click on objective navigates to OKR tab.

- [ ] **Step 4: Create WorkContextCard**

Calls `useQuery("get_dashboard_intelligence")`. Extracts current context summary. Shows dominant app + duration + productivity %. Empty state: "No active session".

- [ ] **Step 5: Create InsightCard**

For the 5 most recent project notes, calls `ipc("note_insight_cache_get", { noteId })` and picks the most recent non-null result. Shows teaser text + "Generated X ago" badge + "Create Task" action button.

- [ ] **Step 6: Create CoachingCard**

Uses `useCoachingNudge({ autoCollapseMs: 60_000 })` (import from `@shared/hooks/useCoachingNudge`, NOT directly from `features/chat/hooks/`). Shows nudge message + Helpful/Dismiss buttons. Empty state: "No active coaching — Deep work mode detected".

- [ ] **Step 7: Create ActivityTimeline**

Reads tasks + objectives from `ProjectContext`. Aggregates `updatedAt` timestamps into a chronological list. Groups by "Today" / "This Week". Color-coded dots per entity type. Max 10 items.

- [ ] **Step 8: Create OverviewTab container**

3-row grid layout:
- Row 1: `grid grid-cols-3 gap-4` — HealthScoreCard, TaskProgressCard, OkrSummaryCard
- Row 2: `grid grid-cols-3 gap-4` — WorkContextCard, InsightCard, CoachingCard
- Row 3: ActivityTimeline (full width)

- [ ] **Step 9: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/features/projects/components/overview/
git commit -m "feat(projects): add Overview tab with health score, stats, intelligence cards, timeline"
```

---

### Task 7: Tasks Tab — Adapted Kanban

**Files:**
- Create: `desktop-ui/src/features/projects/components/tasks/ProjectTasksTab.tsx`

- [ ] **Step 1: Create ProjectTasksTab**

This component adapts the existing Tasks page kanban for project-scoped use:
- Use `useTasks()` from `features/tasks/hooks/useTasks.ts` (the existing hook that returns the full `UseTasksResult` shape including mutations, areas, projects, etc.)
- Filter the tasks client-side: `tasks.filter(t => t.projectId === projectId)`
- Wrap in `StatusWorkflowProvider` (from `features/tasks/contexts/`)
- Reuse `IssueBoard`, `GroupIssues`, `IssueGrid` from `features/tasks/components/` — these expect a `UseTasksResult`-shaped data source
- Add toolbar: Board/List toggle + Filter + "+ Task" button
- Task creation pre-fills `projectId`
- KR link badge: for each task, check `task.metadata?.keyResultId` and render a small badge with KR title (looked up from `objectives` in context)

**Important:** Do NOT create a new data-fetching layer. Reuse `useTasks()` and filter. This avoids impedance mismatch with `IssueBoard`/`GroupIssues` which expect the full tasks hook result shape.

- [ ] **Step 2: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/features/projects/components/tasks/
git commit -m "feat(projects): add Tasks tab reusing existing kanban with KR link badges"
```

---

### Task 8: Notes Tab — Split View + Action Bar

**Files:**
- Create: `desktop-ui/src/features/projects/components/notes/ProjectNotesTab.tsx`
- Create: `desktop-ui/src/features/projects/components/notes/NoteSidebar.tsx`
- Create: `desktop-ui/src/features/projects/components/notes/NoteActionBar.tsx`

- [ ] **Step 1: Create NoteSidebar**

Left panel (w-56):
- Search input → calls `ipc("note_search_hybrid", { query, notebookId })` for each project notebook
- Notebook tree: list notebooks from `project.settings.notebookIds`, each with note count
- Recent notes: top 10 by updatedAt, click selects note

- [ ] **Step 2: Create NoteActionBar**

Row of action buttons below note content:
- "Generate Insight" (primary) → calls `ipc("note_insight_review", { noteId })` (SSE). Shows loading state. On completion, triggers insight cache refetch.
- "Link to KR" (secondary) → opens dropdown of project KRs from context, stores in note metadata
- "Create Task" (secondary) → opens task creation modal pre-scoped to project
- "Flashcards" (secondary) → calls `ipc("flashcard_generate", { noteId })`

- [ ] **Step 3: Create ProjectNotesTab**

Split view layout:
- Left: `NoteSidebar` with notebook tree + note list
- Right: Reuse `NoteEditor` from `features/notes/components/NoteEditor.tsx` for the selected note. Required props: `note: Note` (the selected note object), `onSave: (params: NoteUpdateParams) => void` (wire to `useMutation("note_update")`), `viewMode: "split"` (use split view mode). Check `NoteEditorProps` type in the source file for any additional required props.
- Below editor: `NoteActionBar`
- Below action bar: `InsightPreview` card (if `note_insight_cache_get` returns data for selected note)
- Manages selected note state locally

- [ ] **Step 4: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/features/projects/components/notes/
git commit -m "feat(projects): add Notes tab with split view, action bar, insight preview"
```

---

### Task 9: Sidebar Navigation — Projects Section

**Files:**
- Modify: `desktop-ui/src/app/layouts/Sidebar.tsx`

- [ ] **Step 1: Add Projects to sidebar**

The `SidebarItem` type in `desktop-ui/src/shared/types/common.ts` is a closed union. It already includes `"OKR"` but not `"Projects"`.

For iteration 1, use the existing `"OKR"` sidebar item (currently unused) to navigate to the first project or a project list:

In `Sidebar.tsx`, find the `items` array and add:
```tsx
{ key: "OKR", icon: Target, path: "/tasks" },
```

This uses the already-valid `"OKR"` key. For a future iteration, add `"Projects"` to the `SidebarItem` union in `shared/types/common.ts` and create a dedicated projects list page.

Import `Target` from `lucide-react`.

Alternatively, the project detail page is reached by clicking a specific project in the existing Tasks page sidebar — no new sidebar item is strictly needed for iteration 1.

- [ ] **Step 2: Lint and commit**

Run: `cd desktop-ui && bun run lint:fix`

```bash
git add desktop-ui/src/app/layouts/Sidebar.tsx
git commit -m "feat(projects): add Projects to sidebar navigation"
```

---

### Task 10: Integration Testing + Polish

**Files:**
- Run: full lint, format, build check

- [ ] **Step 1: Run full lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: 0 errors

- [ ] **Step 2: Run full build**

Run: `cd desktop-ui && bun run build`
Expected: successful build with no TypeScript errors

- [ ] **Step 3: Run tests**

Run: `cd desktop-ui && bun run test`
Expected: all tests pass

- [ ] **Step 4: Manual smoke test**

Start dev server: `cd desktop-ui && bun run dev` + `cargo tauri dev`

Test flow:
1. Navigate to `/project/{existing-project-id}` — verify Overview tab renders
2. Click OKR tab → create objective → create KR → edit metric inline → verify progress ring updates
3. Click Tasks tab → verify existing project tasks display in kanban
4. Click Notes tab → verify note sidebar loads project notebooks
5. Drag tab to reorder → refresh page → verify order persisted
6. Click Health Score ring → verify navigation to OKR tab
7. Click FAB → verify dropdown with all 3 options

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat(projects): Project Detail Page with Overview, Tasks, OKR, Notes tabs

Implements the unified Project Detail Page as the 'second brain center':
- /project/:id route with 4 lazy-loaded tabs
- Overview: health score, task progress, OKR summary, intelligence cards
- OKR: full CRUD for objectives + key results with tree view
- Tasks: project-scoped kanban reusing existing components
- Notes: split view with action bar and insight preview
- Glass tab bar with drag reorder + badge indicators
- Quick Add FAB with task/note/objective creation
- Health score computed from OKR progress + task velocity"
```

---

## Optional Backend Enhancement

If displaying `description`, `priority`, and `due_date` on ObjectiveCard is desired:

**File:** `crates/app-core/src/handlers/tasks/converters.rs`

Extend `objective_to_response` to include these fields. Add them to `ObjectiveResponse` in `crates/desktop-shared/src/commands/okr.rs`. Update the TypeScript `Objective` type.

This is not blocking — the OKR tab works without these fields (just won't show due date on objective cards).
