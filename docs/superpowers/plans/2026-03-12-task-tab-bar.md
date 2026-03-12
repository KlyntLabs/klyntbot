# Task Tab Bar Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Chrome-style tab bar to the tasks2 page, seeded with PARA Area tabs by default, supporting in-place navigation with breadcrumbs and Cmd+click to open new tabs.

**Architecture:** New Zustand store (`tab-store`) manages tab state (open tabs, active tab, per-tab navigation stacks). A `TabBar` component renders above the existing layout. A `TabContent` component routes rendering based on the active tab's current navStack entry. Existing components (`AllIssues`, `HeaderNav`, etc.) are reused within tab views.

**Tech Stack:** React 19, TypeScript, Zustand, Tailwind v4 (CSS variable tokens), @dnd-kit/sortable (tab reordering), Radix UI (popover for + menu, context menu for tabs)

**Spec:** `docs/superpowers/specs/2026-03-12-task-tab-bar-design.md`

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `mock-data/areas.ts` | Mock Area data mapping areas → project IDs |
| `store/tab-store.ts` | Tab state: tabs array, active tab, navStack per tab, all actions |
| `components/TabBar.tsx` | Horizontal scrollable tab row + "+" button |
| `components/TabPill.tsx` | Single tab pill: label (with breadcrumb), × button, click/context handlers |
| `components/AddTabMenu.tsx` | Popover from "+" button listing Areas and Projects |
| `components/TabContent.tsx` | Routes to correct view based on active tab's current navStack entry |
| `components/AreaView.tsx` | Lists projects in an area; clicking a project navigates in-place |
| `components/ProjectView.tsx` | Filters issues by project; reuses existing `GroupIssues`/`IssueLine` |
| `components/TabContextMenu.tsx` | Right-click on tab: Close, Close Others, Close to Right |

### Modified Files
| File | Changes |
|------|---------|
| `pages/Tasks2Page.tsx` | Add `TabBar` above `Tasks2Layout`, replace direct `AllIssues` with `TabContent` |
| `components/HeaderNav.tsx` | Show breadcrumb segments (clickable) from active tab's navStack |
| `components/IssueContextMenu.tsx` | Add "Open in new tab" menu item |

All paths below are relative to `desktop-ui/src/features/tasks2/`.

---

## Chunk 1: Data Layer (dependencies, mock data, tab store)

### Task 0: Verify @dnd-kit/sortable Dependency

**Files:**
- Possibly modify: `desktop-ui/package.json`

- [ ] **Step 1: Check if @dnd-kit/sortable is already a dependency**

Run: `cd desktop-ui && grep dnd-kit package.json`

The project already uses `@dnd-kit/core`. Check if `@dnd-kit/sortable` and `@dnd-kit/utilities` are also present.

- [ ] **Step 2: Install missing packages if needed**

Run: `cd desktop-ui && bun add @dnd-kit/sortable @dnd-kit/utilities` (only if not already present)

- [ ] **Step 3: Commit if packages were added**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock
git commit -m "chore(tasks2): add @dnd-kit/sortable dependency"
```

### Task 1: Create Areas Mock Data

**Files:**
- Create: `mock-data/areas.ts`

- [ ] **Step 1: Create the areas mock data file**

```typescript
// mock-data/areas.ts
export interface MockArea {
  id: string;
  name: string;
  projectIds: string[];
}

export const areas: MockArea[] = [
  { id: "area-work", name: "Work", projectIds: ["1", "2", "3", "4", "5"] },
  { id: "area-personal", name: "Personal", projectIds: ["6", "7"] },
  { id: "area-side", name: "Side Projects", projectIds: ["8", "9", "10"] },
];

export function getAreaById(id: string): MockArea | undefined {
  return areas.find((a) => a.id === id);
}

export function getAreaForProject(projectId: string): MockArea | undefined {
  return areas.find((a) => a.projectIds.includes(projectId));
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/mock-data/areas.ts`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/mock-data/areas.ts
git commit -m "feat(tasks2): add areas mock data"
```

### Task 2: Create Tab Store

**Files:**
- Create: `store/tab-store.ts`

- [ ] **Step 1: Write the tab store with all actions**

```typescript
// store/tab-store.ts
import { create } from "zustand";
import type { MockArea } from "../mock-data/areas";

export interface NavEntry {
  type: "my-issues" | "area" | "project" | "issue";
  targetId: string;
  label: string;
}

export interface Tab {
  id: string;
  type: "my-issues" | "area" | "project" | "issue";
  targetId: string;
  navStack: NavEntry[];
}

interface TabState {
  tabs: Tab[];
  activeTabId: string;

  initDefaultTabs: (areas: MockArea[]) => void;
  openTab: (type: Tab["type"], targetId: string, label: string) => void;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  navigateInPlace: (type: NavEntry["type"], targetId: string, label: string) => void;
  navigateToStackIndex: (index: number) => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
}

let idCounter = 0;
function nextId() {
  return `tab-${++idCounter}`;
}

export const useTabStore = create<TabState>((set, get) => ({
  tabs: [],
  activeTabId: "",

  initDefaultTabs: (areas) => {
    if (get().tabs.length > 0) return;

    const myIssuesTab: Tab = {
      id: nextId(),
      type: "my-issues",
      targetId: "my-issues",
      navStack: [{ type: "my-issues", targetId: "my-issues", label: "My Issues" }],
    };

    const areaTabs: Tab[] = areas.map((area) => ({
      id: nextId(),
      type: "area" as const,
      targetId: area.id,
      navStack: [{ type: "area" as const, targetId: area.id, label: area.name }],
    }));

    const allTabs = [myIssuesTab, ...areaTabs];
    set({ tabs: allTabs, activeTabId: allTabs[0].id });
  },

  openTab: (type, targetId, label) => {
    const { tabs, activeTabId } = get();

    // Deduplicate: if tab with same type+targetId exists at root, switch to it
    const existing = tabs.find(
      (t) => t.type === type && t.targetId === targetId && t.navStack.length === 1,
    );
    if (existing) {
      set({ activeTabId: existing.id });
      return;
    }

    const newTab: Tab = {
      id: nextId(),
      type,
      targetId,
      navStack: [{ type, targetId, label }],
    };

    // Insert after active tab
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
      // Prefer left neighbor, fallback to right
      const newIndex = Math.min(index, newTabs.length - 1);
      newActiveId = newTabs[newIndex].id;
    }

    set({ tabs: newTabs, activeTabId: newActiveId });
  },

  setActiveTab: (tabId) => {
    set({ activeTabId: tabId });
  },

  navigateInPlace: (type, targetId, label) => {
    const { tabs, activeTabId } = get();
    set({
      tabs: tabs.map((tab) =>
        tab.id === activeTabId
          ? { ...tab, navStack: [...tab.navStack, { type, targetId, label }] }
          : tab,
      ),
    });
  },

  navigateToStackIndex: (index) => {
    const { tabs, activeTabId } = get();
    set({
      tabs: tabs.map((tab) =>
        tab.id === activeTabId
          ? { ...tab, navStack: tab.navStack.slice(0, index + 1) }
          : tab,
      ),
    });
  },

  reorderTabs: (fromIndex, toIndex) => {
    const { tabs } = get();
    const newTabs = [...tabs];
    const [moved] = newTabs.splice(fromIndex, 1);
    newTabs.splice(toIndex, 0, moved);
    set({ tabs: newTabs });
  },
}));
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/store/tab-store.ts`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/store/tab-store.ts
git commit -m "feat(tasks2): add tab store with navigation stack"
```

---

## Chunk 2: Tab Bar UI Components

### Task 3: Create TabPill Component

**Files:**
- Create: `components/TabPill.tsx`

- [ ] **Step 1: Create the tab pill component**

```typescript
// components/TabPill.tsx
import { X } from "lucide-react";
import type { Tab } from "../store/tab-store";
import { useTabStore } from "../store/tab-store";

interface TabPillProps {
  tab: Tab;
  isActive: boolean;
}

export function TabPill({ tab, isActive }: TabPillProps) {
  const setActiveTab = useTabStore((s) => s.setActiveTab);
  const closeTab = useTabStore((s) => s.closeTab);

  // Build label from navStack with breadcrumb
  const label = tab.navStack.map((entry) => entry.label).join(" › ");

  return (
    <button
      type="button"
      onClick={() => setActiveTab(tab.id)}
      className={`group flex items-center gap-1.5 whitespace-nowrap rounded-t-lg px-3.5 py-1.5 text-[13px] transition-colors ${
        isActive
          ? "bg-[hsl(var(--accent))] text-[hsl(var(--foreground))]"
          : "text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))]"
      }`}
    >
      <span className="truncate max-w-[200px]">{label}</span>
      <span
        role="button"
        tabIndex={-1}
        onClick={(e) => {
          e.stopPropagation();
          closeTab(tab.id);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.stopPropagation();
            closeTab(tab.id);
          }
        }}
        className="text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors"
      >
        <X className="h-3 w-3" />
      </span>
    </button>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/TabPill.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/TabPill.tsx
git commit -m "feat(tasks2): add TabPill component"
```

### Task 4: Create AddTabMenu Component

**Files:**
- Create: `components/AddTabMenu.tsx`

- [ ] **Step 1: Create the add tab menu popover**

```typescript
// components/AddTabMenu.tsx
import { Plus } from "lucide-react";
import { useState } from "react";
import { areas } from "../mock-data/areas";
import { projects } from "../mock-data/projects";
import { useTabStore } from "../store/tab-store";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

export function AddTabMenu() {
  const openTab = useTabStore((s) => s.openTab);
  const [open, setOpen] = useState(false);

  const handleOpenArea = (area: (typeof areas)[number]) => {
    openTab("area", area.id, area.name);
    setOpen(false);
  };

  const handleOpenProject = (project: (typeof projects)[number]) => {
    openTab("project", project.id, project.name);
    setOpen(false);
  };

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
        <div className="text-[11px] font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider px-2 py-1">
          Areas
        </div>
        {areas.map((area) => (
          <button
            key={area.id}
            type="button"
            onClick={() => handleOpenArea(area)}
            className="w-full text-left px-2 py-1.5 text-[13px] rounded-sm hover:bg-[hsl(var(--accent))] text-[hsl(var(--foreground))] transition-colors"
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
            onClick={() => handleOpenProject(project)}
            className="w-full text-left px-2 py-1.5 text-[13px] rounded-sm hover:bg-[hsl(var(--accent))] text-[hsl(var(--foreground))] transition-colors"
          >
            {project.name}
          </button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/AddTabMenu.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AddTabMenu.tsx
git commit -m "feat(tasks2): add AddTabMenu popover component"
```

### Task 5: Create TabContextMenu Component

**Files:**
- Create: `components/TabContextMenu.tsx`

- [ ] **Step 1: Create the tab context menu**

```typescript
// components/TabContextMenu.tsx
import type React from "react";
import { useTabStore } from "../store/tab-store";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "./ui/context-menu";

interface TabContextMenuProps {
  tabId: string;
  children: React.ReactNode;
}

export function TabContextMenu({ tabId, children }: TabContextMenuProps) {
  const closeTab = useTabStore((s) => s.closeTab);
  const tabs = useTabStore((s) => s.tabs);

  const handleClose = () => closeTab(tabId);

  const handleCloseOthers = () => {
    for (const tab of tabs) {
      if (tab.id !== tabId) closeTab(tab.id);
    }
  };

  const handleCloseToRight = () => {
    const index = tabs.findIndex((t) => t.id === tabId);
    for (const tab of tabs.slice(index + 1)) {
      closeTab(tab.id);
    }
  };

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onClick={handleClose}>Close</ContextMenuItem>
        <ContextMenuItem onClick={handleCloseOthers}>Close Others</ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onClick={handleCloseToRight}>Close Tabs to the Right</ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/TabContextMenu.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/TabContextMenu.tsx
git commit -m "feat(tasks2): add TabContextMenu component"
```

### Task 6: Create TabBar Component

**Files:**
- Create: `components/TabBar.tsx`

- [ ] **Step 1: Create the tab bar with drag-and-drop reordering**

```typescript
// components/TabBar.tsx
import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { horizontalListSortingStrategy, SortableContext, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type React from "react";
import { useEffect } from "react";
import { areas } from "../mock-data/areas";
import { useTabStore } from "../store/tab-store";
import { AddTabMenu } from "./AddTabMenu";
import { TabContextMenu } from "./TabContextMenu";
import { TabPill } from "./TabPill";

function SortableTab({ tabId, children }: { tabId: string; children: React.ReactNode }) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({ id: tabId });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      {children}
    </div>
  );
}

export function TabBar() {
  const tabs = useTabStore((s) => s.tabs);
  const activeTabId = useTabStore((s) => s.activeTabId);
  const initDefaultTabs = useTabStore((s) => s.initDefaultTabs);
  const reorderTabs = useTabStore((s) => s.reorderTabs);

  useEffect(() => {
    initDefaultTabs(areas);
  }, [initDefaultTabs]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const fromIndex = tabs.findIndex((t) => t.id === active.id);
    const toIndex = tabs.findIndex((t) => t.id === over.id);
    if (fromIndex !== -1 && toIndex !== -1) {
      reorderTabs(fromIndex, toIndex);
    }
  };

  return (
    <div className="flex items-end gap-0.5 px-2 pt-1.5 border-b border-[hsl(var(--border))] bg-[hsl(var(--background))] overflow-x-auto">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={tabs.map((t) => t.id)} strategy={horizontalListSortingStrategy}>
          {tabs.map((tab) => (
            <SortableTab key={tab.id} tabId={tab.id}>
              <TabContextMenu tabId={tab.id}>
                <TabPill tab={tab} isActive={tab.id === activeTabId} />
              </TabContextMenu>
            </SortableTab>
          ))}
        </SortableContext>
      </DndContext>
      <div className="mb-0.5">
        <AddTabMenu />
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/TabBar.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/TabBar.tsx
git commit -m "feat(tasks2): add TabBar with drag-and-drop reordering"
```

---

## Chunk 3: Tab Content Views

### Task 7: Create AreaView Component

**Files:**
- Create: `components/AreaView.tsx`

- [ ] **Step 1: Create the area view showing projects**

This component lists projects within an area. Clicking a project navigates in-place.

```typescript
// components/AreaView.tsx
import { useMemo } from "react";
import { getAreaById } from "../mock-data/areas";
import { projects } from "../mock-data/projects";
import { useIssuesStore } from "../store/issues-store";
import { useTabStore } from "../store/tab-store";

interface AreaViewProps {
  areaId: string;
}

export function AreaView({ areaId }: AreaViewProps) {
  const area = getAreaById(areaId);
  const navigateInPlace = useTabStore((s) => s.navigateInPlace);
  const issues = useIssuesStore((s) => s.issues);

  const areaProjects = useMemo(() => {
    if (!area) return [];
    return projects.filter((p) => area.projectIds.includes(p.id));
  }, [area]);

  const projectIssueCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const issue of issues) {
      if (issue.project) {
        counts[issue.project.id] = (counts[issue.project.id] ?? 0) + 1;
      }
    }
    return counts;
  }, [issues]);

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
        const Icon = project.icon;
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
            <span className="text-sm text-[hsl(var(--foreground))] flex-1">{project.name}</span>
            <span className="text-xs text-[hsl(var(--muted-foreground))]">{count} issues</span>
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/AreaView.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AreaView.tsx
git commit -m "feat(tasks2): add AreaView component"
```

### Task 8: Create ProjectView Component

**Files:**
- Create: `components/ProjectView.tsx`

- [ ] **Step 1: Create the project view filtering issues by project**

Reuses the existing `GroupIssues` component to render issues filtered by project.

```typescript
// components/ProjectView.tsx
import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  DragOverlay,
  type DragStartEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { useMemo, useState } from "react";
import type { Issue } from "../mock-data/issues";
import { groupIssuesByStatus } from "../mock-data/issues";
import { status as allStatus } from "../mock-data/status";
import { useIssuesStore } from "../store/issues-store";
import { useViewStore } from "../store/view-store";
import { GroupIssues } from "./GroupIssues";
import { IssueGrid } from "./IssueGrid";

interface ProjectViewProps {
  projectId: string;
}

export function ProjectView({ projectId }: ProjectViewProps) {
  const issues = useIssuesStore((s) => s.issues);
  const updateIssueStatus = useIssuesStore((s) => s.updateIssueStatus);
  const viewType = useViewStore((s) => s.viewType);

  const [activeIssue, setActiveIssue] = useState<Issue | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
  );

  const projectIssues = useMemo(
    () => issues.filter((issue) => issue.project?.id === projectId),
    [issues, projectId],
  );

  const grouped = useMemo(() => groupIssuesByStatus(projectIssues), [projectIssues]);
  const isGrid = viewType === "grid";

  const handleDragStart = (event: DragStartEvent) => {
    const issue = event.active.data.current?.issue as Issue | undefined;
    if (issue) setActiveIssue(issue);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveIssue(null);
    if (!over) return;
    const issueId = active.id as string;
    const targetStatus = allStatus.find((s) => s.id === (over.id as string));
    if (targetStatus) updateIssueStatus(issueId, targetStatus);
  };

  if (projectIssues.length === 0) {
    return (
      <div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">
        No issues in this project
      </div>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    >
      <div className={isGrid ? "flex gap-4 p-4 h-full min-w-max" : ""}>
        {allStatus.map((s) => {
          const statusIssues = grouped[s.id];
          if (!isGrid && (!statusIssues || statusIssues.length === 0)) return null;
          return <GroupIssues key={s.id} status={s} issues={statusIssues ?? []} />;
        })}
      </div>
      <DragOverlay>
        {activeIssue ? (
          <div className="opacity-80">
            <IssueGrid issue={activeIssue} />
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/ProjectView.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/ProjectView.tsx
git commit -m "feat(tasks2): add ProjectView component"
```

### Task 9: Create TabContent Router

**Files:**
- Create: `components/TabContent.tsx`

- [ ] **Step 1: Create the tab content router**

Routes rendering based on the active tab's current navStack entry.

```typescript
// components/TabContent.tsx
import { useTabStore } from "../store/tab-store";
import AllIssues from "./AllIssues";
import { AreaView } from "./AreaView";
import HeaderNav from "./HeaderNav";
import HeaderOptions from "./HeaderOptions";
import { ProjectView } from "./ProjectView";

export function TabContent() {
  const tabs = useTabStore((s) => s.tabs);
  const activeTabId = useTabStore((s) => s.activeTabId);

  const activeTab = tabs.find((t) => t.id === activeTabId);

  if (!activeTab) {
    return (
      <div className="flex-1 flex items-center justify-center text-sm text-[hsl(var(--muted-foreground))]">
        Click + to open a tab
      </div>
    );
  }

  // Current view is the last entry in the navStack
  const currentView = activeTab.navStack[activeTab.navStack.length - 1];

  const renderContent = () => {
    switch (currentView.type) {
      case "my-issues":
        return <AllIssues />;
      case "area":
        return <AreaView areaId={currentView.targetId} />;
      case "project":
        return <ProjectView projectId={currentView.targetId} />;
      case "issue":
        // Placeholder for future issue detail view
        return (
          <div className="px-6 py-8 text-sm text-[hsl(var(--muted-foreground))]">
            Issue detail view coming soon: {currentView.label}
          </div>
        );
    }
  };

  return (
    <>
      <HeaderNav />
      <HeaderOptions />
      <div className="overflow-auto w-full flex-1 min-w-0">{renderContent()}</div>
    </>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/TabContent.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/TabContent.tsx
git commit -m "feat(tasks2): add TabContent router component"
```

---

## Chunk 4: Wire Everything Together

### Task 10: Update Tasks2Page to Use Tab Bar

**Files:**
- Modify: `pages/Tasks2Page.tsx`

- [ ] **Step 1: Replace direct component rendering with TabBar + TabContent**

Replace the current `Tasks2Page` content with:

```typescript
// pages/Tasks2Page.tsx
import "../tasks2.css";
import { CreateIssueModal } from "../components/CreateIssueModal";
import { PortalContainerProvider } from "../components/portal-context";
import { TabBar } from "../components/TabBar";
import { TabContent } from "../components/TabContent";
import { Tasks2Layout } from "../components/Tasks2Layout";

export function Tasks2Page() {
  return (
    <PortalContainerProvider>
      <div className="tasks2-scope flex-1 h-full min-w-0">
        <Tasks2Layout>
          <TabBar />
          <TabContent />
        </Tasks2Layout>
        <CreateIssueModal />
      </div>
    </PortalContainerProvider>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/pages/Tasks2Page.tsx`
Expected: No errors

- [ ] **Step 3: Visual verification**

Run: `cd desktop-ui && bun run dev`
Open: `http://localhost:1420/#/tasks2`
Expected: Tab bar visible at top with "My Issues", "Work", "Personal", "Side Projects" tabs. Clicking "My Issues" shows the existing issue list. Clicking "Work" shows the area's projects.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx
git commit -m "feat(tasks2): wire TabBar and TabContent into Tasks2Page"
```

### Task 11: Update HeaderNav for Breadcrumb Navigation

**Files:**
- Modify: `components/HeaderNav.tsx`

- [ ] **Step 1: Replace HeaderNav with breadcrumb-aware version**

Replace the entire content of `HeaderNav.tsx` with:

```typescript
// components/HeaderNav.tsx
import { Search, X } from "lucide-react";
import { useCallback } from "react";
import { useSearchStore } from "../store/search-store";
import { useTabStore } from "../store/tab-store";
import { Button } from "./ui/button";

export default function HeaderNav() {
  const { isSearchOpen, searchQuery, toggleSearch, closeSearch, setSearchQuery } = useSearchStore();
  const tabs = useTabStore((s) => s.tabs);
  const activeTabId = useTabStore((s) => s.activeTabId);
  const navigateToStackIndex = useTabStore((s) => s.navigateToStackIndex);

  const activeTab = tabs.find((t) => t.id === activeTabId);
  const navStack = activeTab?.navStack ?? [];

  const inputRef = useCallback((node: HTMLInputElement | null) => {
    node?.focus();
  }, []);

  return (
    <div className="flex items-center justify-between px-4 py-2 border-b border-[hsl(var(--border))]">
      {/* Left — Breadcrumb */}
      <div className="flex items-center gap-1 min-w-0">
        {navStack.map((entry, index) => {
          const isLast = index === navStack.length - 1;
          return (
            <div key={`${entry.type}-${entry.targetId}`} className="flex items-center gap-1 min-w-0">
              {index > 0 && (
                <span className="text-xs text-[hsl(var(--muted-foreground))] flex-shrink-0">›</span>
              )}
              {isLast ? (
                <span className="text-sm font-medium text-[hsl(var(--foreground))] truncate">
                  {entry.label}
                </span>
              ) : (
                <button
                  type="button"
                  onClick={() => navigateToStackIndex(index)}
                  className="text-sm text-[hsl(var(--muted-foreground))] hover:text-[hsl(var(--foreground))] transition-colors truncate"
                >
                  {entry.label}
                </button>
              )}
            </div>
          );
        })}
      </div>

      {/* Right — Search */}
      <div className="flex items-center gap-2">
        {isSearchOpen ? (
          <div className="flex items-center gap-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
              <input
                ref={inputRef}
                type="text"
                placeholder="Search issues..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="h-7 w-[200px] rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--background))] pl-8 pr-2 text-sm text-[hsl(var(--foreground))] placeholder:text-[hsl(var(--muted-foreground))] outline-none focus:ring-1 focus:ring-[hsl(var(--ring))]"
              />
            </div>
            <Button size="xs" variant="ghost" onClick={closeSearch}>
              <X className="size-4" />
            </Button>
          </div>
        ) : (
          <Button size="xs" variant="ghost" onClick={toggleSearch}>
            <Search className="size-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/HeaderNav.tsx`
Expected: No errors

- [ ] **Step 3: Visual verification**

Open: `http://localhost:1420/#/tasks2`
- Click "Work" tab → should show "Work" as title
- Click a project → title should update to "Work › Project Name"
- Click "Work" in breadcrumb → should navigate back to area view

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/HeaderNav.tsx
git commit -m "feat(tasks2): add breadcrumb navigation to HeaderNav"
```

### Task 12: Add "Open in New Tab" to IssueContextMenu

**Files:**
- Modify: `components/IssueContextMenu.tsx`

- [ ] **Step 1: Add import and menu item**

Add this import at the top of `IssueContextMenu.tsx`:

```typescript
import { useTabStore } from "../store/tab-store";
```

Then add a new `ContextMenuItem` inside the first `ContextMenuGroup`, after the "Copy ID" item:

```typescript
          <ContextMenuItem
            onSelect={() => {
              useTabStore.getState().openTab("issue", issue.id, `${issue.identifier} ${issue.title}`);
            }}
          >
            Open in new tab
          </ContextMenuItem>
```

The resulting first group should look like:

```typescript
        <ContextMenuGroup>
          <ContextMenuItem
            onSelect={() => {
              navigator.clipboard.writeText(issue.identifier);
            }}
          >
            Copy ID
            <span className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
              {issue.identifier}
            </span>
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() => {
              useTabStore.getState().openTab("issue", issue.id, `${issue.identifier} ${issue.title}`);
            }}
          >
            Open in new tab
          </ContextMenuItem>
        </ContextMenuGroup>
```

- [ ] **Step 2: Verify no lint errors**

Run: `cd desktop-ui && bunx biome check src/features/tasks2/components/IssueContextMenu.tsx`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx
git commit -m "feat(tasks2): add 'Open in new tab' to issue context menu"
```

### Task 13: Final Lint Check and Build Verification

- [ ] **Step 1: Run full lint**

Run: `cd desktop-ui && bun run lint:fix`
Expected: No errors (auto-fixes applied if any)

- [ ] **Step 2: Run build**

Run: `cd desktop-ui && bun run build`
Expected: Build succeeds with no type errors

- [ ] **Step 3: Commit any lint fixes**

```bash
git add -A desktop-ui/src/features/tasks2/
git commit -m "chore(tasks2): lint fixes for tab bar feature"
```
