# Task Tab Bar — Design Spec

## Overview

Add a Chrome-style tab bar to the top of the tasks2 page. Each tab represents a navigable view (Area, Project, or Issue). Clicking navigates in-place within the current tab (with breadcrumbs); Cmd+click opens a new tab. Default tabs are seeded from the user's PARA Areas on first load.

## Goals

- Give users fast context-switching between Areas, Projects, and Issues
- Follow Chrome's tab mental model — simple, familiar, no special tab types
- Keep the UI minimal: monochrome tabs, text labels only, subtle active state

## Non-Goals

- Custom filter tabs (filters work within a tab via the existing filter bar)
- Tab persistence across app restarts (future enhancement; localStorage for now)
- Backend API for tab state (UI-only, mock data phase)

## Data Model

```typescript
interface Tab {
  id: string;
  type: "my-issues" | "area" | "project" | "issue";
  targetId: string;         // area_id, project_id, or issue_id ("my-issues" for the special tab)
  navStack: NavEntry[];     // navigation history within this tab
}

interface NavEntry {
  type: "my-issues" | "area" | "project" | "issue";
  targetId: string;
  label: string;            // display name, e.g. "My Issues", "Work", "API Gateway", "TSK-42"
}
```

- `navStack[0]` is the root (what the tab was opened for)
- `navStack[navStack.length - 1]` is the current view
- Tab label is built from `navStack`: entries joined with " › " (e.g. "Work › API Gateway")
- If navStack has only one entry, just show the label directly

## Mock Data: Areas (`mock-data/areas.ts`)

New mock data file mapping Areas to Projects. This bridges the gap between the backend's PARA hierarchy and the current mock data:

```typescript
interface MockArea {
  id: string;
  name: string;
  projectIds: string[];   // references to mock-data/projects.ts IDs
}

const areas: MockArea[] = [
  { id: "area-work", name: "Work", projectIds: ["1", "2", "3", "4", "5"] },
  { id: "area-personal", name: "Personal", projectIds: ["6", "7"] },
  { id: "area-side", name: "Side Projects", projectIds: ["8", "9", "10"] },
];
```

Each mock issue already has an optional `project` field. AreaView filters issues by checking if `issue.project?.id` is in the area's `projectIds`. Issues with no project appear in a special "Unassigned" section within "My Issues".

## Tab Store (`tab-store.ts`)

Zustand store managing tab state:

```typescript
interface TabState {
  tabs: Tab[];
  activeTabId: string;

  // Actions
  openTab: (type: Tab["type"], targetId: string, label: string) => void;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  navigateInPlace: (type: NavEntry["type"], targetId: string, label: string) => void;
  navigateToStackIndex: (index: number) => void;
  reorderTabs: (fromIndex: number, toIndex: number) => void;
  initDefaultTabs: (areas: MockArea[]) => void;
}
```

### `initDefaultTabs(areas)`

Called on first render. If tabs already exist (e.g. hot-reload), does nothing. Otherwise creates:
1. A "My Issues" tab (type: `"my-issues"`, targetId: `"my-issues"`, navStack: `[{ type: "my-issues", targetId: "my-issues", label: "My Issues" }]`)
2. One tab per Area (type: `"area"`, targetId: area's id, navStack: `[{ type: "area", targetId: area.id, label: area.name }]`)

Sets the first tab as active.

### `openTab(type, targetId, label)`

If a tab with the same `type` and `targetId` already exists, switch to it instead of creating a duplicate. Otherwise, creates a new tab with a single navStack entry. Inserts it after the currently active tab. Sets it as active.

### `closeTab(tabId)`

Removes the tab. If it was active, activates the nearest neighbor (prefer left, fallback right). If no tabs remain, no active tab (the tab bar just shows the + button).

### `navigateInPlace(type, targetId, label)`

Pushes a new NavEntry onto the active tab's `navStack`. The tab label updates to show the breadcrumb trail.

### `navigateToStackIndex(index)`

Truncates the active tab's `navStack` to `index + 1`, effectively navigating back to that breadcrumb level. For example, clicking "Work" in "Work › API Gateway › TSK-42" calls `navigateToStackIndex(0)`, which pops both "API Gateway" and "TSK-42" off the stack.

### `reorderTabs(fromIndex, toIndex)`

Moves a tab from one position to another. For drag-and-drop reordering.

## Tab Bar Component (`TabBar.tsx`)

Renders horizontally above the existing `HeaderNav`. Contains:

- A scrollable row of tab pills
- A "+" button at the end

### Tab Pill Visual Design

```
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  ┌───┐
│  My Issues    ×  │  │  Work         ×  │  │  Personal     ×  │  │ + │
└─────────────────┘  └─────────────────┘  └─────────────────┘  └───┘
```

**Active tab:**
- Background: `bg-[hsl(var(--accent))]` (use CSS token, never hardcoded rgba)
- Text: `text-[hsl(var(--foreground))]`
- Border-radius: `rounded-t-lg` (rounded top, flat bottom connects to content)

**Inactive tab:**
- No background
- Text: `text-[hsl(var(--muted-foreground))]`

**All tabs:**
- Font size: `text-[13px]`
- Padding: `px-3.5 py-1.5`
- × button: always present, `text-[hsl(var(--muted-foreground))]`, brighter on hover
- White-space: `whitespace-nowrap` (tabs don't wrap text)

**+ button:**
- `w-[26px] h-[26px]`, rounded, muted color
- On click: opens a popover listing available Areas and recent Projects

### Tab Breadcrumb

When a user navigates in-place, the tab label updates:

- Root: `"Work"`
- One level deep: `"Work › API Gateway"`
- Two levels deep: `"Work › API Gateway › TSK-42"`

The `›` separator is rendered in `text-[hsl(var(--muted-foreground))]`. Each breadcrumb segment is clickable, calling `navigateToStackIndex(segmentIndex)`.

### Scrolling

If tabs overflow the container width, the tab bar scrolls horizontally with `overflow-x-auto`. Fade edges on left/right indicate scrollable content (CSS `mask-image` gradient). The + button popover and tab context menus must use Radix portals to avoid clipping by the scroll container.

## Navigation Behavior

### Click (normal click on an item in the task list)

Navigates in-place within the current tab:
- Clicking a project from an Area view → pushes project onto navStack
- Clicking an issue from a project view → pushes issue onto navStack
- Tab label updates with breadcrumb

### Cmd+Click (or right-click → "Open in new tab")

Opens a new tab:
- Creates a new tab for the clicked item (or switches to existing if duplicate)
- Inserts after the current tab
- Switches to the new tab
- Original tab stays unchanged

Note: Cmd+click can open tabs for any item type — Areas, Projects, and Issues alike. The + button menu is a convenient alternative for opening Areas/Projects without navigating to them first.

### Context Menu (right-click on an item)

Add to existing IssueContextMenu:
- "Open in new tab" — opens the item in a new tab

### Tab Context Menu (right-click on a tab)

- "Close" — close this tab
- "Close Others" — close all tabs except this one
- "Close Tabs to the Right" — close all tabs after this one

## + Button Menu

Opens a popover with:

**Areas** section — lists all Areas. Clicking one opens a new tab (or switches to existing).

**All Projects** section — lists all projects from mock data. Clicking one opens a new tab (or switches to existing). (In a future backend-integrated phase, this can be replaced with "Recent Projects" using server-tracked recency.)

## Component Hierarchy

```
Tasks2Page
├── TabBar                    ← NEW
│   ├── Tab (for each tab)    ← NEW
│   └── AddTabButton (+)      ← NEW
├── TabContent                ← NEW (replaces direct AllIssues render)
│   ├── MyIssuesView          ← wraps existing AllIssues (shows all issues)
│   ├── AreaView              ← NEW (lists projects in area, clicking drills into ProjectView)
│   ├── ProjectView           ← NEW (filters issues by project)
│   └── IssueDetailView       ← NEW (shows single issue detail — placeholder for now)
├── HeaderNav                 (existing, now inside TabContent)
├── HeaderOptions             (existing, now inside TabContent)
└── CreateIssueModal          (existing)
```

## Page Layout Change

Current:
```
┌──────────────────────────────┐
│ HeaderNav                    │
│ HeaderOptions                │
│ AllIssues                    │
└──────────────────────────────┘
```

Proposed:
```
┌──────────────────────────────┐
│ TabBar                       │  ← new, fixed at top
├──────────────────────────────┤
│ HeaderNav (breadcrumb + nav) │
│ HeaderOptions (filters/view) │
│ TabContent (varies by tab)   │
└──────────────────────────────┘
```

## Mock Data Phase

Since we're still in UI-only mode:
- Create `mock-data/areas.ts` with 3 areas mapping to existing mock projects
- Default tabs generated from these areas on first render
- "My Issues" tab renders the existing `AllIssues` component (shows all issues regardless of assignee)
- AreaView filters issues where `issue.project?.id` is in the area's `projectIds`
- ProjectView filters issues where `issue.project?.id === targetProjectId`
- IssueDetailView is a placeholder for now (title + description)

## Tab Drag-and-Drop

Use `@dnd-kit/core` (already a project dependency) for tab reordering. `useSortable` on each tab pill, `SortableContext` wrapping the tab row.

## Interactions Summary

| Trigger | Action |
|---------|--------|
| Page load | Default tabs created: "My Issues" + one per Area (skip if tabs already exist) |
| Click tab | Switch active tab |
| Click item in list | Navigate in-place (push navStack), tab label updates with breadcrumb |
| Click breadcrumb segment | Navigate to that stack level (truncate navStack) |
| Cmd+click item | Open new tab (or switch to existing duplicate) |
| Right-click item → "Open in new tab" | Open new tab (or switch to existing duplicate) |
| × on tab | Close tab |
| + button | Popover to pick Area/Project |
| Right-click tab | Context menu: Close, Close Others, Close to Right |
| Drag tab | Reorder |
