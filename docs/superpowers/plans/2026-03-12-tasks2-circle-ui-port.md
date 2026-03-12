# Tasks2 Circle UI Port — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the Circle (Linear-inspired) issue tracker UI as a new page at `/#/tasks2` with mock data, not touching the existing task page.

**Architecture:** New `features/tasks2/` feature folder with Zustand stores for state, shadcn-style Radix UI primitives, @dnd-kit for board drag-and-drop, and Circle's mock data. Page renders inside existing AppShell.

**Tech Stack:** React 19, TypeScript, Tailwind v4, Zustand, @dnd-kit, cmdk, Radix UI, motion (framer-motion v12), date-fns, lucide-react

---

## Chunk 1: Foundation — Dependencies, Mock Data, Stores, UI Primitives

### Task 1: Install Dependencies

**Files:**
- Modify: `desktop-ui/package.json`

- [ ] **Step 1: Install new packages**

```bash
cd desktop-ui && bun add zustand motion date-fns cmdk class-variance-authority @radix-ui/react-popover @radix-ui/react-dropdown-menu @radix-ui/react-dialog @radix-ui/react-context-menu @radix-ui/react-avatar @radix-ui/react-separator
```

- [ ] **Step 2: Verify installation**

```bash
cd desktop-ui && bun run build
```

Expected: Build succeeds with no errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/package.json desktop-ui/bun.lockb
git commit -m "feat(tasks2): add dependencies for Circle UI port"
```

---

### Task 2: Mock Data — Types and Static Data

**Files:**
- Create: `desktop-ui/src/features/tasks2/mock-data/status.tsx`
- Create: `desktop-ui/src/features/tasks2/mock-data/priorities.tsx`
- Create: `desktop-ui/src/features/tasks2/mock-data/labels.ts`
- Create: `desktop-ui/src/features/tasks2/mock-data/users.ts`
- Create: `desktop-ui/src/features/tasks2/mock-data/projects.ts`
- Create: `desktop-ui/src/features/tasks2/mock-data/issues.ts`

Port these directly from Circle's source. Key adaptations:
- Remove all `'use client'` directives (not Next.js)
- Replace `@/` imports with relative imports
- Keep all type interfaces exactly as Circle defines them

- [ ] **Step 1: Create `status.tsx`**

Port Circle's `mock-data/status.tsx` exactly — 6 statuses (In Progress, Technical Review, Completed, Paused, Todo, Backlog) with their SVG icon components and color strings. Includes the `Status` interface and `StatusIcon` helper component.

- [ ] **Step 2: Create `priorities.tsx`**

Port Circle's `mock-data/priorities.tsx` — 5 priorities (No Priority, Urgent, High, Medium, Low) with SVG icon components. Includes the `Priority` interface.

- [ ] **Step 3: Create `labels.ts`**

Port Circle's `mock-data/labels.ts` — 11 labels with `LabelInterface` type (id, name, color string).

- [ ] **Step 4: Create `users.ts`**

Port Circle's `mock-data/users.ts` — 4 mock users with `User` interface (id, name, avatarUrl). Set `avatarUrl` to empty string — the `AssigneeUser` component renders `AvatarFallback` with initials when the image fails to load. This keeps the app offline-friendly.

- [ ] **Step 5: Create `projects.ts`**

Port Circle's `mock-data/projects.ts` — 10 mock projects with `Project` interface (id, name, icon component). Use lucide-react icons for project icons.

- [ ] **Step 6: Create `issues.ts`**

Port Circle's `mock-data/issues.ts` — the `Issue` interface, 30 mock issues, `groupIssuesByStatus()` and `sortIssuesByPriority()` helper functions. Import `LexoRank` from `../lib/utils` (defined in Task 4) — do NOT duplicate LexoRank here. For now, hardcode the rank strings inline so this file compiles independently; Task 4 will add the LexoRank class and this file can be updated to use it.

- [ ] **Step 7: Verify types compile**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: No type errors in `features/tasks2/mock-data/`.

- [ ] **Step 8: Commit**

```bash
git add desktop-ui/src/features/tasks2/mock-data/
git commit -m "feat(tasks2): add mock data ported from Circle"
```

---

### Task 3: Zustand Stores

**Files:**
- Create: `desktop-ui/src/features/tasks2/store/issues-store.ts`
- Create: `desktop-ui/src/features/tasks2/store/filter-store.ts`
- Create: `desktop-ui/src/features/tasks2/store/view-store.ts`
- Create: `desktop-ui/src/features/tasks2/store/search-store.ts`
- Create: `desktop-ui/src/features/tasks2/store/create-issue-store.ts`

Port directly from Circle's stores. Key adaptations:
- Replace `@/mock-data/` imports with relative `../mock-data/`
- `view-store` uses `zustand/middleware` persist with `createJSONStorage(() => localStorage)`

- [ ] **Step 1: Create `view-store.ts`**

Simplest store — `viewType: 'list' | 'grid'`, `setViewType()`, persisted to localStorage under key `'tasks2-view-storage'`.

- [ ] **Step 2: Create `search-store.ts`**

Store with `isSearchOpen`, `searchQuery`, `toggleSearch()`, `closeSearch()`, `setSearchQuery()`.

- [ ] **Step 3: Create `create-issue-store.ts`**

Store with `isOpen`, `defaultStatus`, `openModal(status?)`, `closeModal()`.

- [ ] **Step 4: Create `filter-store.ts`**

Store with `filters: { status: string[], assignee: string[], priority: string[], labels: string[], project: string[] }`, `setFilter()`, `toggleFilter()`, `clearFilters()`, `clearFilterType()`, `hasActiveFilters()`, `getActiveFiltersCount()`.

- [ ] **Step 5: Create `issues-store.ts`**

The main store — holds `issues[]`, `issuesByStatus` (derived). Explicit methods:
- **CRUD:** `addIssue(issue)`, `updateIssue(id, partial)`, `deleteIssue(id)`
- **Filters:** `filterByStatus(statusId)`, `filterByPriority(priorityId)`, `filterByAssignee(userId|null)`, `filterByLabel(labelId)`, `filterByProject(projectId)`, `searchIssues(query)`, `filterIssues(filters)`
- **Updates:** `updateIssueStatus(issueId, newStatus)`, `updateIssuePriority(issueId, newPriority)`, `updateIssueAssignee(issueId, newAssignee)`
- **Labels:** `addIssueLabel(issueId, label)`, `removeIssueLabel(issueId, labelId)`
- **Utility:** `getIssueById(id)`, `getAllIssues()`

- [ ] **Step 6: Verify stores compile**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: No type errors.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/tasks2/store/
git commit -m "feat(tasks2): add Zustand stores ported from Circle"
```

---

### Task 4: Utility Functions

**Files:**
- Create: `desktop-ui/src/features/tasks2/lib/utils.ts`
- Create: `desktop-ui/src/features/tasks2/lib/status-utils.tsx`

- [ ] **Step 1: Create `utils.ts`**

Port the `LexoRank` class from Circle's `lib/utils.ts`. Also re-export `cn` from `@shared/lib/cn` for convenience.

- [ ] **Step 2: Create `status-utils.tsx`**

Port Circle's `lib/status-utils.tsx` — the `renderStatusIcon()` helper that maps status ID to the correct SVG icon component.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/lib/
git commit -m "feat(tasks2): add utility functions"
```

---

### Task 5: Shadcn-style UI Primitives

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/ui/popover.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/command.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/dropdown-menu.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/context-menu.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/avatar.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/dialog.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/separator.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ui/button.tsx`

These are standard shadcn/ui component wrappers around Radix primitives. Port from Circle's `components/ui/` directory with these adaptations:
- Replace `@/lib/utils` import with `../../lib/utils` (for `cn`)
- Use the existing project's `cn` function from `@shared/lib/cn`
- Keep all variant definitions (shadcn uses `class-variance-authority` patterns inline with `cn`)

Note on `button.tsx`: Circle's Button has custom size variants (`xs`, `sm`, `default`, `lg`, `icon`) that differ from our shared Button. Create a local Button to avoid conflicts.

- [ ] **Step 1: Create `popover.tsx`**

Standard shadcn Popover wrapping `@radix-ui/react-popover` — exports `Popover`, `PopoverTrigger`, `PopoverContent`.

- [ ] **Step 2: Create `command.tsx`**

Standard shadcn Command wrapping `cmdk` — exports `Command`, `CommandInput`, `CommandList`, `CommandEmpty`, `CommandGroup`, `CommandItem`, `CommandSeparator`.

- [ ] **Step 3: Create `dropdown-menu.tsx`**

Standard shadcn DropdownMenu wrapping `@radix-ui/react-dropdown-menu` — exports `DropdownMenu`, `DropdownMenuTrigger`, `DropdownMenuContent`, `DropdownMenuItem`.

- [ ] **Step 4: Create `context-menu.tsx`**

Standard shadcn ContextMenu wrapping `@radix-ui/react-context-menu` — exports `ContextMenu`, `ContextMenuTrigger`, `ContextMenuContent`, `ContextMenuItem`, `ContextMenuSeparator`.

- [ ] **Step 5: Create `avatar.tsx`**

Standard shadcn Avatar wrapping `@radix-ui/react-avatar` — exports `Avatar`, `AvatarImage`, `AvatarFallback`.

- [ ] **Step 6: Create `dialog.tsx`**

Standard shadcn Dialog wrapping `@radix-ui/react-dialog` — exports `Dialog`, `DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogTitle`, `DialogDescription`, `DialogFooter`.

- [ ] **Step 7: Create `separator.tsx`**

Standard shadcn Separator wrapping `@radix-ui/react-separator`.

- [ ] **Step 8: Create `button.tsx`**

Port Circle's Button with variants: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link` and sizes: `default`, `xs`, `sm`, `lg`, `icon`. Uses `cn` for className merging.

- [ ] **Step 9: Verify all UI primitives compile**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: No type errors.

- [ ] **Step 10: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/ui/
git commit -m "feat(tasks2): add shadcn-style UI primitives"
```

---

### Task 6: CSS Variables and Page Skeleton

**Files:**
- Create: `desktop-ui/src/features/tasks2/tasks2.css`
- Create: `desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx`
- Create: `desktop-ui/src/features/tasks2/index.ts`
- Modify: `desktop-ui/src/features/index.ts` (add tasks2 export)
- Modify: `desktop-ui/src/app/router.tsx` (add /tasks2 route)

- [ ] **Step 1: Create `tasks2.css`**

Define shadcn-compatible CSS variables for the tasks2 page. These provide the token values that shadcn Tailwind classes reference. Scope under `.tasks2-scope`:

```css
.tasks2-scope {
  --background: 0 0% 100%;
  --foreground: 240 10% 3.9%;
  --card: 0 0% 100%;
  --card-foreground: 240 10% 3.9%;
  --popover: 0 0% 100%;
  --popover-foreground: 240 10% 3.9%;
  --primary: 240 5.9% 10%;
  --primary-foreground: 0 0% 98%;
  --secondary: 240 4.8% 95.9%;
  --secondary-foreground: 240 5.9% 10%;
  --muted: 240 4.8% 95.9%;
  --muted-foreground: 240 3.8% 46.1%;
  --accent: 240 4.8% 95.9%;
  --accent-foreground: 240 5.9% 10%;
  --destructive: 0 84.2% 60.2%;
  --destructive-foreground: 0 0% 98%;
  --border: 240 5.9% 90%;
  --input: 240 5.9% 90%;
  --ring: 240 5.9% 10%;
  --sidebar: 0 0% 98%;
  --container: 0 0% 100%;
  --radius: 0.5rem;
}

@media (prefers-color-scheme: dark) {
  .tasks2-scope {
    --background: 240 10% 3.9%;
    --foreground: 0 0% 98%;
    --card: 240 10% 3.9%;
    --card-foreground: 0 0% 98%;
    --popover: 240 10% 3.9%;
    --popover-foreground: 0 0% 98%;
    --primary: 0 0% 98%;
    --primary-foreground: 240 5.9% 10%;
    --secondary: 240 3.7% 15.9%;
    --secondary-foreground: 0 0% 98%;
    --muted: 240 3.7% 15.9%;
    --muted-foreground: 240 5% 64.9%;
    --accent: 240 3.7% 15.9%;
    --accent-foreground: 0 0% 98%;
    --destructive: 0 62.8% 30.6%;
    --destructive-foreground: 0 0% 98%;
    --border: 240 3.7% 15.9%;
    --input: 240 3.7% 15.9%;
    --ring: 240 4.9% 83.9%;
    --sidebar: 240 5.9% 10%;
    --container: 240 10% 3.9%;
    --radius: 0.5rem;
  }
}
```

**IMPORTANT — Tailwind v4 CSS variable resolution:** Tailwind v4 resolves utilities from `@theme` at build time, not from ancestor CSS variables at runtime. Therefore, `bg-background` will always resolve to the global theme's `--background`, ignoring `.tasks2-scope` overrides. All shadcn-ported components MUST use arbitrary value syntax for scoped tokens:

- Use `bg-[hsl(var(--background))]` instead of `bg-background`
- Use `text-[hsl(var(--foreground))]` instead of `text-foreground`
- Use `border-[hsl(var(--border))]` instead of `border-border`
- Use `text-[hsl(var(--muted-foreground))]` instead of `text-muted-foreground`
- And so on for all scoped tokens

This is the single source of truth for styling. Do not mix global Tailwind utilities with scoped CSS variables.

- [ ] **Step 2: Create `Tasks2Page.tsx`**

```tsx
import "../tasks2.css";

export function Tasks2Page() {
  return (
    <div className="tasks2-scope flex-1 flex flex-col overflow-hidden h-full bg-[hsl(var(--container))]">
      <div className="flex flex-col items-center justify-center h-full text-sm text-[hsl(var(--muted-foreground))]">
        Tasks2 page — scaffold working
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Create `index.ts`**

```tsx
export { Tasks2Page } from "./pages/Tasks2Page";
```

- [ ] **Step 4: Add export to `features/index.ts`**

Add `export * from "./tasks2";` to `desktop-ui/src/features/index.ts`.

- [ ] **Step 5: Add route to `router.tsx`**

Add lazy import and route:

```tsx
// ── Tasks2 Feature (Circle UI Port) ─────────────────────────────
const Tasks2Page = lazy(() =>
  import("../features/tasks2").then((m) => ({ default: m.Tasks2Page })),
);
```

Add route inside AppShell children, after the existing `/tasks` route:

```tsx
{ path: "/tasks2", element: <Tasks2Page /> },
```

- [ ] **Step 6: Verify the page loads**

```bash
cd desktop-ui && bun run build
```

Expected: Build succeeds. Navigate to `http://localhost:1420/#/tasks2` and see "Tasks2 page — scaffold working".

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/tasks2/ desktop-ui/src/features/index.ts desktop-ui/src/app/router.tsx
git commit -m "feat(tasks2): add page skeleton, route, and CSS variables"
```

---

## Chunk 2: Core Components — List View

### Task 7: Small Display Components

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/AssigneeUser.tsx`
- Create: `desktop-ui/src/features/tasks2/components/LabelBadge.tsx`
- Create: `desktop-ui/src/features/tasks2/components/ProjectBadge.tsx`

These are small, stateless display components used by both IssueLine and IssueGrid.

- [ ] **Step 1: Create `AssigneeUser.tsx`**

Port from Circle — shows user avatar (Avatar component) or a default placeholder circle when `user` is null.

- [ ] **Step 2: Create `LabelBadge.tsx`**

Port from Circle — renders an array of label badges as small colored pills.

- [ ] **Step 3: Create `ProjectBadge.tsx`**

Port from Circle — shows project icon + name in a compact badge.

- [ ] **Step 4: Verify display components compile**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: No type errors.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AssigneeUser.tsx desktop-ui/src/features/tasks2/components/LabelBadge.tsx desktop-ui/src/features/tasks2/components/ProjectBadge.tsx
git commit -m "feat(tasks2): add display components (avatar, labels, project badge)"
```

---

### Task 8: Status and Priority Selectors

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/StatusSelector.tsx`
- Create: `desktop-ui/src/features/tasks2/components/PrioritySelector.tsx`

These are interactive inline selectors — clicking them opens a Command popover to change the value.

- [ ] **Step 1: Create `StatusSelector.tsx`**

Port from Circle — Popover with Command list of all 6 statuses. On select, calls `updateIssueStatus()` from issues-store. Shows current status SVG icon as trigger.

- [ ] **Step 2: Create `PrioritySelector.tsx`**

Port from Circle — Popover with Command list of all 5 priorities. On select, calls `updateIssuePriority()` from issues-store. Shows current priority SVG icon as trigger.

- [ ] **Step 3: Verify selectors compile**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/StatusSelector.tsx desktop-ui/src/features/tasks2/components/PrioritySelector.tsx
git commit -m "feat(tasks2): add inline status and priority selectors"
```

---

### Task 9: Issue Context Menu

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx`

- [ ] **Step 1: Create `IssueContextMenu.tsx`**

Port from Circle — right-click context menu with actions: Copy ID, Set Status (submenu), Set Priority (submenu), Delete. Uses `ContextMenu`, `ContextMenuContent`, `ContextMenuItem` from local UI primitives.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/IssueContextMenu.tsx
git commit -m "feat(tasks2): add issue context menu"
```

---

### Task 10: IssueLine Component (List View Row)

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/IssueLine.tsx`

- [ ] **Step 1: Create `IssueLine.tsx`**

Port from Circle — a single row in the list view. Layout:
- Left: `PrioritySelector` | identifier (muted, fixed width) | `StatusSelector`
- Center: title (truncated)
- Right: `LabelBadge` | `ProjectBadge` | date (formatted with `date-fns format(date, 'MMM dd')`) | `AssigneeUser`
- Wrapped in `ContextMenu` + `ContextMenuTrigger` for right-click support
- Uses `motion.div` for layout animations (optional `layoutId` prop)
- Height: `h-11`, padding: `px-6`, hover: `hover:bg-[hsl(var(--sidebar))/0.5]`

- [ ] **Step 2: Verify IssueLine compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/IssueLine.tsx
git commit -m "feat(tasks2): add IssueLine list view row component"
```

---

### Task 11: GroupIssues Component (Status Group)

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/GroupIssues.tsx`

- [ ] **Step 1: Create `GroupIssues.tsx`**

Port from Circle — renders a status group header + list of issues. List view only for now (board view added in Chunk 3).

Header: sticky, shows status icon + name + count + "+" button (opens create issue modal). Background tinted with status color at low opacity.

Body (list mode): renders `IssueLine` for each issue, sorted by priority.

- [ ] **Step 2: Verify GroupIssues compiles**

```bash
cd desktop-ui && bunx tsc --noEmit --pretty 2>&1 | head -20
```

Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/GroupIssues.tsx
git commit -m "feat(tasks2): add GroupIssues status group component"
```

---

### Task 12: AllIssues Component

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/AllIssues.tsx`

- [ ] **Step 1: Create `AllIssues.tsx`**

Port from Circle — the main content router. **Use `export default function AllIssues()`** (default export, matching the import in Tasks2Page).

Checks search/filter state and renders:
- If searching: inline placeholder `<div className="px-6 py-8 text-center text-sm text-[hsl(var(--muted-foreground))]">Search results will appear here</div>` (replaced by real SearchIssues in Task 17)
- If filtering: filtered issues grouped by status
- Default: all issues grouped by status via `GroupIssues`

Uses `status` array to iterate all 6 statuses, rendering a `GroupIssues` for each.

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/AllIssues.tsx
git commit -m "feat(tasks2): add AllIssues content router component"
```

---

### Task 13: HeaderNav and HeaderOptions

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/HeaderNav.tsx`
- Create: `desktop-ui/src/features/tasks2/components/HeaderOptions.tsx`

- [ ] **Step 1: Create `HeaderNav.tsx`**

Port from Circle — top header bar with:
- Left: "My Issues" text
- Right: search toggle button (toggles search-store) + notifications icon (static, non-functional)
- When search is open: shows search input with `SearchIcon`, bound to search-store

- [ ] **Step 2: Create `HeaderOptions.tsx`**

Port from Circle — second header bar with:
- Left: Filter button — render a non-functional `<Button size="xs" variant="ghost"><ListFilter className="size-4 mr-1" /> Filter</Button>` as placeholder. Task 18 replaces this with the real `<Filter />` component.
- Right: Display dropdown (DropdownMenu with List/Board options, uses view-store)

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/HeaderNav.tsx desktop-ui/src/features/tasks2/components/HeaderOptions.tsx
git commit -m "feat(tasks2): add header nav and options bars"
```

---

### Task 14: Tasks2Layout and Wire Everything Up

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/Tasks2Layout.tsx`
- Modify: `desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx`
- Modify: `desktop-ui/src/features/tasks2/index.ts`

- [ ] **Step 1: Create `Tasks2Layout.tsx`**

Port Circle's layout structure:
```
<div className="h-full overflow-hidden flex flex-col bg-[hsl(var(--container))]">
  <div className="border rounded-md overflow-hidden flex flex-col h-full">
    {children}
  </div>
</div>
```

Note: no sidebar — the existing AppShell provides that.

- [ ] **Step 2: Update `Tasks2Page.tsx`**

Replace the placeholder with the real composition:

```tsx
import "../tasks2.css";
import { Tasks2Layout } from "../components/Tasks2Layout";
import { HeaderNav } from "../components/HeaderNav";
import { HeaderOptions } from "../components/HeaderOptions";
import AllIssues from "../components/AllIssues";

export function Tasks2Page() {
  return (
    <div className="tasks2-scope flex-1 h-full">
      <Tasks2Layout>
        <HeaderNav />
        <HeaderOptions />
        <div className="overflow-auto w-full flex-1">
          <AllIssues />
        </div>
      </Tasks2Layout>
    </div>
  );
}
```

- [ ] **Step 3: Verify list view renders**

```bash
cd desktop-ui && bun run build
```

Expected: Build succeeds. Navigate to `http://localhost:1420/#/tasks2` — should see the full list view with status groups and issue rows.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): wire up list view — full page working"
```

---

## Chunk 3: Advanced Features — Board View, Filter, Search, Create Modal

### Task 15: IssueGrid Component (Board View Card)

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/IssueGrid.tsx`

- [ ] **Step 1: Create `IssueGrid.tsx`**

Port from Circle but adapt drag-and-drop from `react-dnd` to `@dnd-kit`:
- Card layout: priority + identifier on top, title, labels + project badge, date + avatar at bottom
- Uses `useDraggable` from `@dnd-kit/core` for drag behavior (not `useSortable` — we only support dropping between columns to change status, not reordering within a column)
- Wrapped in `ContextMenu` for right-click support
- Uses `motion.div` with `layoutId` for animations
- Card style: `bg-background rounded-md shadow-xs border border-border/50`

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/IssueGrid.tsx
git commit -m "feat(tasks2): add IssueGrid board view card with @dnd-kit"
```

---

### Task 16: Board View in GroupIssues + DnD Wiring

**Files:**
- Modify: `desktop-ui/src/features/tasks2/components/GroupIssues.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/AllIssues.tsx`

- [ ] **Step 1: Add board mode to `GroupIssues.tsx`**

Extend GroupIssues to support both list and grid views:
- When `viewType === 'grid'`: render as a column (`w-[348px]`, flex-shrink-0) with `IssueGrid` cards
- Use `useDroppable` from `@dnd-kit/core` for the column drop zone
- On drop: call `updateIssueStatus()` from issues-store

- [ ] **Step 2: Add DndContext to `AllIssues.tsx`**

Wrap the grouped issues in `DndContext` from `@dnd-kit/core`:
- `onDragEnd`: determine which status column received the drop, call `updateIssueStatus`
- Add `DragOverlay` to render a card preview during drag
- When `viewType === 'grid'`: render groups in a horizontal flex container with `overflow-x-auto`

- [ ] **Step 3: Verify board view works**

```bash
cd desktop-ui && bun run build
```

Expected: Build succeeds. Toggle to Board view in Display dropdown — should see kanban columns. Drag a card between columns.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/GroupIssues.tsx desktop-ui/src/features/tasks2/components/AllIssues.tsx
git commit -m "feat(tasks2): add board view with @dnd-kit drag-and-drop"
```

---

### Task 17: Search Issues

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/SearchIssues.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/AllIssues.tsx` (replace stub)

- [ ] **Step 1: Create `SearchIssues.tsx`**

Port from Circle — shows search results as a list of `IssueLine` components. Uses `searchIssues()` from issues-store. Shows "No results found" when empty. Results count in header.

- [ ] **Step 2: Wire into AllIssues**

Replace the search stub in `AllIssues.tsx` with the real `SearchIssues` component.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/SearchIssues.tsx desktop-ui/src/features/tasks2/components/AllIssues.tsx
git commit -m "feat(tasks2): add search issues functionality"
```

---

### Task 18: Filter Popover

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/Filter.tsx`
- Modify: `desktop-ui/src/features/tasks2/components/HeaderOptions.tsx` (replace stub)

- [ ] **Step 1: Create `Filter.tsx`**

Port from Circle — multi-level filter popover:
- Level 1: category list (Status, Assignee, Priority, Labels, Project) with chevron + active count
- Level 2: items within selected category with checkmarks for active filters
- Back button to return to level 1
- "Clear all filters" at bottom when filters active
- Uses Popover + Command components

This is a large component (~250 lines). Keep it as a single file since the two levels share state (which category is active) and don't warrant separate components.

- [ ] **Step 2: Wire into HeaderOptions**

Replace the placeholder Filter button with the real `<Filter />` component.

- [ ] **Step 3: Verify filtering works**

```bash
cd desktop-ui && bun run build
```

Expected: Build succeeds. Click Filter → Status → select "In Progress" → only In Progress issues shown.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/Filter.tsx desktop-ui/src/features/tasks2/components/HeaderOptions.tsx
git commit -m "feat(tasks2): add multi-level filter popover"
```

---

### Task 19: Create Issue Modal

**Files:**
- Create: `desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx`
- Modify: `desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx` (add modal provider)

- [ ] **Step 1: Create `CreateIssueModal.tsx`**

Build the create issue dialog:
- Uses Dialog from local UI primitives
- Fields:
  - **Title** (text input, required)
  - **Status** (selector, pre-selected from `create-issue-store.defaultStatus`)
  - **Priority** (selector, default: No Priority)
  - **Assignee** (optional, dropdown of mock users + "Unassigned")
  - **Labels** (optional, multi-select from mock labels)
- On submit: generate new ID (`issues.length + 1`), generate identifier (`LNUI-${600 + issues.length}`), generate rank, push to issues-store via `addIssue()`, close modal
- Controlled by `create-issue-store` (isOpen, closeModal)

- [ ] **Step 2: Add modal to Tasks2Page**

Add `<CreateIssueModal />` to the page, reading `isOpen` from `create-issue-store`.

- [ ] **Step 3: Verify create works**

```bash
cd desktop-ui && bun run build
```

Expected: Click "+" on a status group header → modal opens with that status → enter title → submit → new issue appears in the group.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tasks2/components/CreateIssueModal.tsx desktop-ui/src/features/tasks2/pages/Tasks2Page.tsx
git commit -m "feat(tasks2): add create issue modal"
```

---

### Task 20: Final Polish and Verification

**Files:**
- All existing files (no new files)

- [ ] **Step 1: Run full build**

```bash
cd desktop-ui && bun run build
```

Expected: Clean build with no errors.

- [ ] **Step 2: Run lint**

```bash
cd desktop-ui && bun run lint:fix
```

Expected: No lint errors (or auto-fixed).

- [ ] **Step 3: Manual smoke test checklist**

Navigate to `http://localhost:1420/#/tasks2` and verify:
- List view shows 30 issues grouped by 6 statuses
- Click status/priority icons → popover opens, selection updates
- Toggle Display → Board → see kanban columns
- Drag a card between columns → status updates
- Click Filter → select categories → issues filter correctly
- Type in search → results filter by title/identifier
- Click "+" on group header → create modal opens → create works
- Right-click an issue → context menu shows actions
- Existing `/tasks` page still works unchanged

- [ ] **Step 4: Commit any polish fixes**

```bash
git add desktop-ui/src/features/tasks2/
git commit -m "feat(tasks2): polish and finalize Circle UI port"
```
