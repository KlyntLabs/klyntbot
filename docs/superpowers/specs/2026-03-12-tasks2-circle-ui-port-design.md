# Tasks2 — Circle UI Port

Port the [Circle](https://github.com/ln-dev7/circle) project management UI (Linear-inspired) as a new page at `/#/tasks2`. UI-only with mock data; backend integration deferred.

## Scope

- New route `/tasks2` inside existing AppShell (sidebar untouched)
- Pixel-close replica of Circle's issue tracking interface
- Mock data only — no Tauri IPC, no backend calls
- Does not modify any existing task page code

## What We're Porting

Circle provides a full issue tracker UI with:

1. **Header nav bar** — breadcrumb title + search toggle + notifications
2. **Options bar** — multi-level filter popover + display mode toggle (list/board)
3. **List view** (default) — issues grouped by status, each row: priority icon | identifier | status icon | title | labels | project badge | date | assignee avatar
4. **Board view** — kanban columns by status, draggable cards with the same fields
5. **Inline selectors** — click status/priority icons to change via command popover
6. **Search** — inline search bar that filters issues by title/identifier
7. **Filters** — multi-level popover filtering by status, assignee, priority, labels, project
8. **Context menu** — right-click on issues for actions
9. **Create issue modal** — triggered from status group headers

## Architecture

### File Structure

```
desktop-ui/src/features/tasks2/
├── pages/
│   └── Tasks2Page.tsx
├── components/
│   ├── Tasks2Layout.tsx
│   ├── HeaderNav.tsx
│   ├── HeaderOptions.tsx
│   ├── Filter.tsx
│   ├── AllIssues.tsx
│   ├── GroupIssues.tsx
│   ├── IssueLine.tsx
│   ├── IssueGrid.tsx
│   ├── SearchIssues.tsx
│   ├── StatusSelector.tsx
│   ├── PrioritySelector.tsx
│   ├── AssigneeUser.tsx
│   ├── LabelBadge.tsx
│   ├── ProjectBadge.tsx
│   ├── IssueContextMenu.tsx
│   └── CreateIssueModal.tsx
├── store/
│   ├── issues-store.ts
│   ├── filter-store.ts
│   ├── view-store.ts
│   ├── search-store.ts
│   └── create-issue-store.ts
├── mock-data/
│   ├── issues.ts
│   ├── status.tsx
│   ├── priorities.tsx
│   ├── labels.ts
│   ├── projects.ts
│   └── users.ts
├── lib/
│   ├── utils.ts
│   └── status-utils.tsx
└── index.ts
```

### New Dependencies

| Package | Purpose | Notes |
|---------|---------|-------|
| `zustand` | State management for stores | Circle uses zustand; lightweight alternative to context |
| `motion` | Layout animations (list↔board transitions, drag previews) | framer-motion v12+ (import from `motion/react`) |
| `date-fns` | Date formatting (`format(date, 'MMM dd')`) | Used throughout issue display |
| `cmdk` | Command palette for selectors and filters | Circle uses shadcn Command which wraps cmdk |
| `@radix-ui/react-popover` | Popover primitive for filter and selector dropdowns | Required by shadcn-style Popover component |
| `@radix-ui/react-dropdown-menu` | Dropdown primitive for display toggle | Required by shadcn-style DropdownMenu component |
| `@radix-ui/react-dialog` | Dialog primitive for create issue modal | Required by shadcn-style Dialog component |
| `@radix-ui/react-context-menu` | Context menu primitive for right-click actions | Required by shadcn-style ContextMenu component |
| `@radix-ui/react-avatar` | Avatar primitive for assignee display | Required by shadcn-style Avatar component |
| `@radix-ui/react-separator` | Separator primitive | Used in command lists and filter UI |

**Already available:** `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities` (use these instead of Circle's `react-dnd`), `lucide-react`, `clsx`, `tailwind-merge`.

### Drag-and-Drop Adaptation

Circle uses `react-dnd` + HTML5 backend. We adapt to `@dnd-kit` which is already installed:

- `DndContext` + `SortableContext` replace `DndProvider`
- `useSortable` replaces `useDrag`/`useDrop`
- `DragOverlay` replaces `CustomDragLayer`
- Drop zones per status column use `useDroppable`

### State Management

Five Zustand stores, each self-contained:

- **issues-store** — Issue array, CRUD, groupByStatus, filtering, search, status/priority/assignee updates
- **filter-store** — Active filter selections (status[], assignee[], priority[], labels[], project[]), toggle/clear
- **view-store** — `list | grid` toggle, persisted to localStorage
- **search-store** — Search open/closed state, query string
- **create-issue-store** — Modal open/closed, default status for new issue

### Styling Strategy

Circle uses shadcn/ui + Tailwind (light/dark). Our app uses custom CSS tokens + Tailwind v4. Strategy:

- **Keep Circle's visual identity** — the `tasks2` page should look like Circle, not like the rest of the app. This is intentional since we're evaluating a redesign.
- **Local CSS variables** — add a `tasks2.css` file in the feature folder that defines shadcn-compatible CSS variables (e.g., `--sidebar`, `--muted-foreground`, `--accent`, `--primary-foreground`, `--border`, `--container`) scoped under a `.tasks2-scope` class on the page root. This avoids polluting the global theme while letting Tailwind utilities like `bg-sidebar/50` and `text-muted-foreground` work correctly within the tasks2 page.
- **SVG status/priority icons** — port directly from Circle (inline SVGs, no icon library dependency)
- **No glass-panel** — Circle's design is flat/bordered, not glassmorphic
- **Color values in mock data** — status colors (`#facc15`, `#22c55e`, etc.) are defined in mock data, not theme tokens. This is fine for the UI-only phase.

### UI Components to Build (not reuse from shared/)

Circle needs several shadcn-style components that don't exist in our shared library:

- **Command** (cmdk wrapper) — for filter popovers and status/priority selectors
- **Popover** — for filter and selector dropdowns (Radix-based)
- **DropdownMenu** — for display toggle
- **Avatar** — for assignee display
- **Dialog** — for create issue modal (our existing Dialog composite can be adapted)
- **Sheet** — not needed initially

These will live in `features/tasks2/components/ui/` to keep them isolated from the shared library.

### Route Registration

```tsx
// In app/router.tsx
const Tasks2Page = lazy(() =>
  import("../features/tasks2").then(m => ({ default: m.Tasks2Page }))
);

// Add to routes array:
{ path: "/tasks2", element: <Tasks2Page /> }
```

No sidebar navigation item needed — this is an experimental page accessed by URL.

## Data Types

### Issue Interface

```typescript
interface Issue {
  id: string;
  identifier: string;        // e.g., "LNUI-101"
  title: string;
  description: string;
  status: Status;             // Embedded object (not ID)
  assignee: User | null;      // Embedded object or null if unassigned
  priority: Priority;         // Embedded object
  labels: LabelInterface[];   // Array of embedded label objects
  createdAt: string;          // ISO date string, displayed as "MMM dd"
  cycleId: string;
  project?: Project;          // Optional embedded project object
  subissues?: string[];       // Issue IDs (not used in UI yet)
  rank: string;               // LexoRank string for ordering
  dueDate?: string;           // Optional ISO date string (not used in UI yet)
}
```

All related types (`Status`, `Priority`, `LabelInterface`, `Project`, `User`) are embedded objects, not foreign keys. The stores operate on these objects directly — no ID lookups needed.

### Create Issue Modal

Fields: title (required), status (pre-selected from group header), priority (default: No Priority), assignee (optional), labels (optional). On submit, generates a new ID/identifier and pushes to the issues-store array.

## Mock Data

Ported directly from Circle with 30 issues across 6 statuses:

- **Statuses:** In Progress, Technical Review, Completed, Paused, Todo, Backlog (each with custom SVG icon + color)
- **Priorities:** No Priority, Urgent, High, Medium, Low (each with custom SVG icon)
- **Labels:** 11 categories (UI Enhancement, Bug, Feature, Documentation, Refactor, Performance, Design, Security, Accessibility, Testing, Internationalization)
- **Projects:** 10 mock projects with icons
- **Users:** 4 mock users with avatar URLs

## Interactions

| Interaction | Implementation |
|------------|----------------|
| Click status icon | Opens command popover to change status |
| Click priority icon | Opens command popover to change priority |
| Toggle list/board | Display dropdown in HeaderOptions, persisted to localStorage |
| Filter | Multi-level popover: category → items with checkmarks |
| Search | Toggle search input in HeaderNav, filters issues by title/identifier |
| Drag card (board view) | @dnd-kit drag between status columns, updates issue status |
| Right-click issue | Context menu with actions (copy ID, change status, delete) |
| Click "+" on group header | Opens create issue modal with that status pre-selected |
| Board ↔ List transition | Motion layout animations for smooth view switching |

## Out of Scope

- Backend integration (Tauri IPC, real data)
- Sidebar navigation changes
- Issue detail page
- Sub-issues expansion
- Cycles view
- Notifications panel content
- Settings pages
- Any changes to the existing `/tasks` page
