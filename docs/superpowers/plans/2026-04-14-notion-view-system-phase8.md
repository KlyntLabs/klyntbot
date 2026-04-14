# Notion View System — Phase 8: Polish

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish pass — loading skeletons during entity fetch, richer empty states per view, undo toast for destructive view actions (delete view), and a `useReducedMotion` hook applied to DnD + animations.

**Architecture:** Reuse the existing `Skeleton` (`@shared/ui/Skeleton`) and `useToast` (`@shared/hooks/useToast`) primitives. Thread `loading` from `useEntities` through `DatabasePage` → `ViewShell` so each view can render skeletons. A small `useReducedMotion` hook reads `prefers-reduced-motion` and is consumed by DnD sensors (raised activation distance to discourage accidental drags) and by the `<Skeleton>` pulse animation. Undo for `deleteView` snapshots the view object before deleting and re-creates via existing `useCreateView` if the user clicks Undo within the toast TTL.

**Tech Stack:** React + TypeScript, existing `useToast`, existing `Skeleton`, `@dnd-kit` (already wired). No new deps.

---

## Scope & non-goals

- **In scope:** loading skeletons for Table/List/Gallery/Board/Feed/Chart, prettier empty states (icon + heading + sub + CTA) for all six, view-delete undo toast, reduced-motion hook applied to DnD activation distance and Skeleton pulse.
- **Out of scope:** undo for entity reorder (no backend snapshot today; would need pre-mutation position to round-trip), undo for entity delete, "are you sure" dialogs, notification-style toasts on every reorder.

## File structure

| File | Responsibility |
|---|---|
| `desktop-ui/src/shared/hooks/useReducedMotion.ts` (new) | Subscribe to `matchMedia('(prefers-reduced-motion: reduce)')` and return `boolean`. |
| `desktop-ui/src/shared/ui/Skeleton.tsx` (modify) | Honor `useReducedMotion` — drop `animate-pulse` when reduced. |
| `desktop-ui/src/features/database/components/views/ViewLoadingSkeleton.tsx` (new) | View-type-aware skeleton (table rows / list rows / card grid / board columns / chart rect / feed cards). |
| `desktop-ui/src/features/database/components/views/ViewEmptyState.tsx` (new) | Reusable empty-state with icon + title + subtitle + optional `onAction`. |
| `desktop-ui/src/features/database/components/ViewShell.tsx` (modify) | Accept `loading: boolean`. When `loading && entities.length === 0`, render `<ViewLoadingSkeleton viewType={...} />`. Pass `onNewEntity` to empty state. |
| `desktop-ui/src/features/database/pages/DatabasePage.tsx` (modify) | Forward `entitiesLoading` to `ViewShell`. |
| `desktop-ui/src/features/database/components/views/ListView.tsx` (modify) | Replace inline empty text with `<ViewEmptyState onAction={onNewEntity} />`. |
| `desktop-ui/src/features/database/components/views/GalleryView.tsx` (modify) | Same. |
| `desktop-ui/src/features/database/components/views/table/TableView.tsx` (modify) | Same; also add empty state when grouped + zero rows. |
| `desktop-ui/src/features/database/components/views/BoardView.tsx` (modify) | Empty state when zero entities (currently shows empty columns only). |
| `desktop-ui/src/features/database/components/views/CalendarView.tsx` (modify) | Empty state overlay on top of grid when no events at all. |
| `desktop-ui/src/features/database/components/views/FeedView.tsx` (modify) | Replace inline text empty state. |
| `desktop-ui/src/features/database/components/views/ViewSwitcher.tsx` (modify) | After successful `deleteView.mutate(view.id)`, fire toast with Undo action that re-creates the view via `createView.mutate(name, viewType, config)`. |
| `desktop-ui/src/features/database/lib/dndConfig.ts` (new) | Shared `useDndSensors()` that picks PointerSensor activation distance (5 normal, 12 reduced-motion). |
| `desktop-ui/src/features/database/components/views/{List,Gallery,Board,table/Table}View.tsx` (modify) | Replace inline `useSensors(useSensor(PointerSensor, {activationConstraint: {distance: 5}}))` with shared hook. |

## Conventions

- New entity action propagates from `DatabasePage` → `ViewShell` → empty states. Use the existing `onNewEntity` prop already on `ViewShell`.
- Toast variants: success / error already exist; "info" with `action` button is new. If `useToast` doesn't support actions, extend its `Toast` type with optional `action: { label: string; onClick: () => void }` and render a button after the message.
- Skeletons render at the same density as real content (table = 8 rows of 36px, gallery = 8 cards in grid, board = 4 columns of 3 cards).
- Reduced-motion: when true, skip `animate-pulse`, raise PointerSensor activation distance, and disable any future spring/fade transitions added.

---

## Task 1: `useReducedMotion` hook

**Files:**
- Create: `desktop-ui/src/shared/hooks/useReducedMotion.ts`

- [ ] **Step 1: Implement**

```ts
// desktop-ui/src/shared/hooks/useReducedMotion.ts
import { useEffect, useState } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState<boolean>(() => {
    if (typeof window === "undefined" || !window.matchMedia) return false;
    return window.matchMedia(QUERY).matches;
  });

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const mql = window.matchMedia(QUERY);
    const handler = (e: MediaQueryListEvent) => setReduced(e.matches);
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, []);

  return reduced;
}
```

- [ ] **Step 2: Modify `Skeleton.tsx`** to honor it.

Open `desktop-ui/src/shared/ui/Skeleton.tsx`. Replace the body so that when reduced, the `animate-pulse` class is dropped:

```tsx
import { useReducedMotion } from "@shared/hooks/useReducedMotion";

export function Skeleton({ className }: { className?: string }) {
  const reduced = useReducedMotion();
  return (
    <div
      className={`${reduced ? "" : "animate-pulse"} rounded-lg bg-accent ${className ?? ""}`}
    />
  );
}
```

Keep the existing default classes if they differ — only the `animate-pulse` needs to become conditional.

---

## Task 2: Shared DnD sensors hook

**Files:**
- Create: `desktop-ui/src/features/database/lib/dndConfig.ts`

- [ ] **Step 1: Implement**

```ts
// desktop-ui/src/features/database/lib/dndConfig.ts
import { PointerSensor, useSensor, useSensors } from "@dnd-kit/core";
import { useReducedMotion } from "@shared/hooks/useReducedMotion";

const DEFAULT_DISTANCE = 5;
const REDUCED_DISTANCE = 12;

export function useEntityDndSensors() {
  const reduced = useReducedMotion();
  return useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: reduced ? REDUCED_DISTANCE : DEFAULT_DISTANCE },
    }),
  );
}
```

- [ ] **Step 2: Replace inline sensor wiring** in each view.

For each of `ListView.tsx`, `GalleryView.tsx`, `BoardView.tsx`, `table/TableView.tsx`:

Replace the import line:

```ts
import { PointerSensor, useSensor, useSensors } from "@dnd-kit/core";
```

with:

```ts
import { useEntityDndSensors } from "@features/database/lib/dndConfig";
```

(Keep the `closestCenter`, `DndContext`, `DragEndEvent` imports intact.)

Replace this line:

```ts
const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
```

with:

```ts
const sensors = useEntityDndSensors();
```

---

## Task 3: `ViewEmptyState` component

**Files:**
- Create: `desktop-ui/src/features/database/components/views/ViewEmptyState.tsx`

- [ ] **Step 1: Implement**

```tsx
// desktop-ui/src/features/database/components/views/ViewEmptyState.tsx
import { Plus } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icon?: ReactNode;
  title: string;
  subtitle?: string;
  actionLabel?: string;
  onAction?: () => void;
}

export function ViewEmptyState({ icon, title, subtitle, actionLabel, onAction }: Props) {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-3 px-6 py-20 text-center">
      {icon && <div className="text-foreground/35">{icon}</div>}
      <p className="text-[14px] font-medium text-foreground">{title}</p>
      {subtitle && <p className="max-w-sm text-[13px] text-foreground/55">{subtitle}</p>}
      {actionLabel && onAction && (
        <button
          type="button"
          onClick={onAction}
          className="mt-2 inline-flex items-center gap-1 rounded-md bg-brand px-3 py-1.5 text-[13px] font-medium text-white shadow-sm hover:opacity-90"
        >
          <Plus className="h-3.5 w-3.5" />
          {actionLabel}
        </button>
      )}
    </div>
  );
}
```

---

## Task 4: `ViewLoadingSkeleton`

**Files:**
- Create: `desktop-ui/src/features/database/components/views/ViewLoadingSkeleton.tsx`

- [ ] **Step 1: Implement**

```tsx
// desktop-ui/src/features/database/components/views/ViewLoadingSkeleton.tsx
import { Skeleton } from "@shared/ui/Skeleton";
import type { ViewType } from "@shared/types";

interface Props {
  viewType: ViewType;
}

const COUNT = 8;

export function ViewLoadingSkeleton({ viewType }: Props) {
  switch (viewType) {
    case "table":
      return (
        <div className="space-y-1 p-4">
          {Array.from({ length: COUNT }).map((_, i) => (
            <Skeleton key={i} className="h-9 w-full" />
          ))}
        </div>
      );
    case "list":
    case "feed":
      return (
        <div className="space-y-2 p-4">
          {Array.from({ length: COUNT }).map((_, i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      );
    case "gallery":
      return (
        <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-3 lg:grid-cols-4">
          {Array.from({ length: COUNT }).map((_, i) => (
            <Skeleton key={i} className="h-32 w-full" />
          ))}
        </div>
      );
    case "board":
      return (
        <div className="flex gap-4 p-4">
          {Array.from({ length: 4 }).map((_, c) => (
            <div key={c} className="flex w-64 shrink-0 flex-col gap-2">
              <Skeleton className="h-6 w-24" />
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-20 w-full" />
              ))}
            </div>
          ))}
        </div>
      );
    case "calendar":
    case "timeline":
      return <Skeleton className="m-4 h-[480px] w-[calc(100%-2rem)]" />;
    case "chart":
      return <Skeleton className="m-4 h-[400px] w-[calc(100%-2rem)]" />;
  }
}
```

If TypeScript complains about exhaustiveness, add a `default: return null;` arm.

---

## Task 5: Wire loading + empty into ViewShell + DatabasePage

**Files:**
- Modify: `desktop-ui/src/features/database/pages/DatabasePage.tsx`
- Modify: `desktop-ui/src/features/database/components/ViewShell.tsx`

- [ ] **Step 1: DatabasePage — pass `entitiesLoading`**

In `DatabasePage.tsx`, find the `useEntities` call. It returns `{ data, loading }`. Currently the page only branches on schema loading. Add `entitiesLoading={entities.loading}` (or whatever field name `useEntities` exposes — confirm by reading the hook) to the `<ViewShell ... />` props.

If `useEntities` exposes the loading flag under a different name (e.g. `isLoading`, `pending`), use that name.

- [ ] **Step 2: ViewShell — accept and route the flag**

Add to `ViewShellProps` (around line 13 of `ViewShell.tsx`):

```ts
entitiesLoading?: boolean;
```

In the body, just before rendering `ActiveViewRenderer`, branch:

```tsx
const showSkeleton = entitiesLoading && entities.length === 0;
const showEmpty = !entitiesLoading && entities.length === 0;
```

Pass these into `ActiveViewRenderer` (or handle right here — easier to keep all view logic in `ActiveViewRenderer`). Decision: handle inline in `ViewShell.tsx` so we don't have to thread two more props through every renderer:

```tsx
<div className="min-h-0 flex-1 overflow-y-auto">
  {activeView && showSkeleton && <ViewLoadingSkeleton viewType={activeView.viewType} />}
  {activeView && showEmpty && (
    <ViewEmptyState
      icon={<Plus className="h-10 w-10" />}
      title="No items yet"
      subtitle="Get started by creating your first entry."
      actionLabel="New entry"
      onAction={onNewEntity}
    />
  )}
  {activeView && !showSkeleton && !showEmpty && (
    <ActiveViewRenderer ... />
  )}
</div>
```

Add the imports:

```tsx
import { Plus } from "lucide-react";
import { ViewEmptyState } from "./views/ViewEmptyState";
import { ViewLoadingSkeleton } from "./views/ViewLoadingSkeleton";
```

This collapses every view's per-file empty-state branch into one shared empty state. The per-file branches become unreachable for `entities.length === 0`, but they should still be defensive (they may stay as harmless dead branches; cleanup is optional and not required for this task).

---

## Task 6: Undo toast for view delete

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useToast.ts` (extend type if needed)
- Modify: `desktop-ui/src/shared/components/ToastContainer.tsx` (render action button)
- Modify: `desktop-ui/src/features/database/components/views/ViewSwitcher.tsx` (snapshot + undo)

- [ ] **Step 1: Read `useToast.ts`**

Open `desktop-ui/src/shared/hooks/useToast.ts`. Note the current `Toast` type and `show()` signature. If `show()` already takes an `action` option, skip Step 2.

- [ ] **Step 2: Extend with optional action**

Add (or merge) into the `Toast` type:

```ts
export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  message: string;
  variant: "error" | "success" | "info";
  action?: ToastAction;
}
```

Update `show()` to accept the variant and an optional `action`:

```ts
function show(message: string, opts?: { variant?: Toast["variant"]; action?: ToastAction }) {
  const id = Date.now();
  setToasts((prev) => [...prev, { id, message, variant: opts?.variant ?? "info", action: opts?.action }]);
  // existing auto-dismiss
}
```

Make sure existing call sites still type-check — old `show(msg, "error")` calls become `show(msg, { variant: "error" })` if the signature was previously positional. Update those call sites accordingly. Search:

```
rg -n "useToast\(|toast\.show\(" desktop-ui/src
```

Adjust each to the new shape.

- [ ] **Step 3: Render the action button in `ToastContainer.tsx`**

Inside the toast `<div>` rendering loop, after the message, if `t.action` is set, append:

```tsx
{t.action && (
  <button
    type="button"
    onClick={() => { t.action!.onClick(); dismiss(t.id); }}
    className="ml-3 rounded px-2 py-0.5 text-[12px] font-medium text-brand hover:underline"
  >
    {t.action.label}
  </button>
)}
```

- [ ] **Step 4: ViewSwitcher — snapshot + undo on delete**

Find the `onDelete` handler in `ViewSwitcher.tsx` (currently `() => deleteView.mutate(view.id)`). Replace with:

```tsx
onDelete={async () => {
  const snapshot = view; // ViewDefinition with name, viewType, config
  await deleteView.mutate(view.id);
  toast.show(`Deleted "${snapshot.name}"`, {
    variant: "info",
    action: {
      label: "Undo",
      onClick: () => {
        void createView.mutate(snapshot.name, snapshot.viewType, snapshot.config);
      },
    },
  });
}}
```

Add `const toast = useToast();` near the other hook calls in `ViewSwitcher`. Import it: `import { useToast } from "@shared/hooks/useToast";`.

Note: undo re-creates the view but assigns a new `id` and places it at the end. Position-restoration is out of scope. The toast TTL (4s) is acceptable; the user must click within that window.

- [ ] **Step 5: Verify the `ToastContainer` is mounted near the app root**

Open `desktop-ui/src/app/App.tsx` (or whatever the root layout is). Confirm `<ToastContainer />` is rendered. If not, add it once at the root. (Per the survey it already exists; verify.)

---

## Task 7: Replace per-view inline empty states (cleanup)

This task removes the now-unreachable empty branches in each view (`entities.length === 0` checks) since `ViewShell` handles that case centrally.

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/ListView.tsx`
- Modify: `desktop-ui/src/features/database/components/views/GalleryView.tsx`
- Modify: `desktop-ui/src/features/database/components/views/table/TableView.tsx`
- Modify: `desktop-ui/src/features/database/components/views/FeedView.tsx`
- Modify: `desktop-ui/src/features/database/components/views/BoardView.tsx`
- Modify: `desktop-ui/src/features/database/components/views/CalendarView.tsx`

- [ ] **Step 1: Delete each file's empty-state branch**

For each file above, locate the `if (entities.length === 0) return ...;` (or inline `entities.length === 0 && ...` JSX) and delete it. The view component now assumes it's only rendered with `entities.length > 0`.

Important: keep the **table grouped + zero-visible-rows** branch in `TableView`, because grouping + collapse + filters can produce zero visible rows even with non-empty `entities`. Render `<ViewEmptyState title="No items match" subtitle="Adjust filters or expand groups." />` (no action button) when `items.length === 0` after grouping.

Add the `ViewEmptyState` import to `TableView.tsx`.

- [ ] **Step 2: Confirm typecheck**

```
cd desktop-ui && bunx tsc --noEmit
```

Expected: clean.

---

## Self-review checklist

- [ ] `useReducedMotion` returns `false` when `window.matchMedia` is undefined (SSR safety).
- [ ] DnD activation distance changes when reduced-motion toggles mid-session (handler updates state).
- [ ] Skeleton respects reduced-motion.
- [ ] `ViewLoadingSkeleton` covers all 8 view types.
- [ ] `ViewEmptyState` action button only renders when both `actionLabel` and `onAction` are provided.
- [ ] Centralized loading/empty handling in `ViewShell`; per-view branches removed.
- [ ] Undo toast re-creates view via `createView.mutate(name, viewType, config)`.
- [ ] `useToast` extension is backward-compatible — no existing call sites broken.
- [ ] All theme tokens, no raw hex.

## Verification (deferred per user directive)

Tests are not added in this phase. After execution, the user will run their own smoke pass. Recommended manual matrix when they're ready:

1. Open a fresh database with no entities → see new empty state with "New entry" CTA.
2. Open the Tasks database → no skeleton flash since data is cached; soft-reload (`Cmd+R`) → skeletons render briefly.
3. Switch to Board / Calendar / Chart / Feed → skeletons match each layout density.
4. Delete a view → toast appears with "Undo" → click within 4s → view is recreated at the end of the tab list.
5. Enable macOS "Reduce motion" in System Settings → reload → Skeleton stops pulsing; DnD requires a longer drag distance to activate.

```
cd desktop-ui && bun run lint
cd desktop-ui && bunx tsc --noEmit
cargo build -p entity-store
```
