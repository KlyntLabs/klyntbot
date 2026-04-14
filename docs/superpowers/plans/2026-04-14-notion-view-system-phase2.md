# Phase 2 — Frontend plumbing for Notion view system

**Date:** 2026-04-14
**Status:** Proposed — awaiting green light
**Depends on:** Phase 1 (backend) landed at commit `6b67458e` (and preceding).
**Scope:** Lay the frontend foundations so phases 3–8 (view CRUD UI, DnD, TanStack table rewrite, grouping, new view types) have everything they need.

## Why this phase exists

Phase 1 added three new capabilities on the Rust side that the frontend currently cannot reach:

1. **Fractional ordering** (`Entity.position`, `db_reorder_entity`, `db_reorder_views`).
2. **Nested filter groups** (`QueryParams.filter: FilterGroup`, `ViewConfig.filter`).
3. **The `position` field on Entity responses** — frontend types don't declare it yet.

This phase is pure plumbing: types, hooks, constants. **No user-visible UI changes.** Phases 3+ consume these.

Shipping this separately is deliberate: the cognitive load of "add a lib + rewrite a view" is already high; bundling type widening and hook creation into the same PR makes review impossible.

## Non-goals

- No UI changes. The `/database/:slug` page should look identical after phase 2.
- No DnD wiring yet (phase 4). We just install the library and confirm it tree-shakes correctly.
- No TableView rewrite (phase 5). We install TanStack but don't use it yet.
- No filter builder UI (phase 3). We widen the types; builder component comes next.

## Libraries to install

All installed via `bun add` in `desktop-ui/`. Confirm each tree-shakes cleanly against Vite + React Compiler.

| Package | Purpose | Notes |
|---|---|---|
| `@atlaskit/pragmatic-drag-and-drop` | Core DnD engine (Trello/Jira's) | Framework-agnostic; we import React adapters lazily |
| `@atlaskit/pragmatic-drag-and-drop-hitbox` | Edge detection for reorder-between-items | Separate entry point = better tree-shake |
| `@atlaskit/pragmatic-drag-and-drop-auto-scroll` | Auto-scroll while dragging near viewport edges | Board columns + long tables need this |
| `fractional-indexing-jittered` | Generate position keys client-side | Optimistic DnD: compute key locally, send to backend |
| `@tanstack/react-table` | Headless table engine | Used only by TableView (phase 5) |
| `@tanstack/react-virtual` | Row virtualization | Pairs with react-table for 1k+ rows |

**Rejected:**
- `@dnd-kit/*` — pragmatic-drag-and-drop wins on bundle size + prod pedigree.
- `react-beautiful-dnd` — abandoned; Atlassian replaced it with pragmatic.
- `recharts` — deferred to phase 7 (Chart view).

## Type changes (`desktop-ui/src/shared/types/database.ts`)

Keep backwards compatibility with the backend's dual `filters` / `filter` field — the backend AND-combines both, so the frontend can migrate gradually.

### 1. `Entity.position`

```ts
export interface Entity {
  id: string;
  databaseId: string;
  fields: Record<string, unknown>;
  position: string;           // NEW — fractional index key
  createdAt: string;
  updatedAt: string;
}
```

### 2. Filter nodes (new types)

```ts
export type LogicOp = "and" | "or";

export type FilterNode =
  | { kind: "rule"; field: string; op: FilterOp; value: unknown }
  | { kind: "group"; op: LogicOp; nodes: FilterNode[] };

export interface FilterGroup {
  op: LogicOp;
  nodes: FilterNode[];
}
```

Backend's tagged enum serializes as `{"kind": "rule", ...}` / `{"kind": "group", ...}` — `FilterNode` above matches exactly.

### 3. Widen `ViewConfig`

```ts
export interface ViewConfig {
  filters?: FilterRule[];         // kept
  filter?: FilterGroup;           // NEW — nested tree
  sorts?: SortRule[];
  visibleFields?: string[];
  groupBy?: string;
  calendarField?: string;
  galleryField?: string;
  cardFields?: string[];
  layout?: Record<string, unknown>;
  // NEW — per-view state we'll populate in later phases:
  collapsedGroups?: string[];
  columnWidths?: Record<string, number>;
  columnOrder?: string[];
  columnVisibility?: Record<string, boolean>;
}
```

### 4. Widen `QueryParams`

```ts
export interface QueryParams {
  filters?: FilterRule[];
  filter?: FilterGroup;           // NEW
  sorts?: SortRule[];
  limit?: number;
  offset?: number;
}
```

## New hooks

All placed under `desktop-ui/src/features/database/hooks/`.

### `useEntityReorder.ts` — drag-to-reorder

```ts
export function useEntityReorder(databaseId: string) {
  const { mutate: rawMutate, loading, error } =
    useMutation<Entity, Record<string, unknown>>("db_reorder_entity");

  const mutate = useCallback(
    async (entityId: string, beforeId?: string, afterId?: string) => {
      const result = await rawMutate({ databaseId, entityId, beforeId, afterId });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
```

### `useEntityPatch.ts` — generic field patch

Thin re-export of `useUpdateEntity`. Rationale: DnD callers don't say "update entity"; they say "entity moved to a new group/day" = patch one field. New name makes call sites read correctly. Single-line file.

```ts
export { useUpdateEntity as useEntityPatch } from "./useEntity";
```

### `useReorderViews.ts` — drag view tabs

```ts
export function useReorderViews(databaseId: string) {
  const { mutate: rawMutate, loading, error } =
    useMutation<ViewDefinition[], Record<string, unknown>>("db_reorder_views");

  const mutate = useCallback(
    async (viewIds: string[]) => {
      const result = await rawMutate({ databaseId, viewIds });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
```

### Update `useEntities.ts` to pass `filter`

One-liner: thread the new `filter` field through to `db_query`.

```ts
return useQuery<QueryResult>(
  "db_query",
  databaseId
    ? {
        databaseId,
        filters: params?.filters,
        filter: params?.filter,        // NEW
        sorts: params?.sorts,
        limit: params?.limit,
        offset: params?.offset,
      }
    : null,
  { invalidateOn: [DATABASE_UPDATED_EVENT] },
);
```

## New utility module — `features/database/lib/ordering.ts`

Wraps `fractional-indexing-jittered` for the common DnD cases. Keeps callers declarative.

```ts
import { generateKeyBetween } from "fractional-indexing-jittered";

/** Compute the fractional key for an item dropped between `before` and `after`. */
export function keyBetween(before: string | null, after: string | null): string {
  return generateKeyBetween(before, after);
}

/** Given the full ordered list and a move (fromIndex → toIndex), compute the new key. */
export function keyForMove(
  ordered: Array<{ position: string }>,
  fromIndex: number,
  toIndex: number,
): string {
  const without = ordered.filter((_, i) => i !== fromIndex);
  const before = without[toIndex - 1]?.position ?? null;
  const after = without[toIndex]?.position ?? null;
  return keyBetween(before, after);
}
```

We use the **client-side** key purely to let the UI update optimistically before the backend round-trips — the backend is still the source of truth and may recompute on `reorder_entity`. If the client key and server key diverge, the next `db_query` invalidation reconciles.

## New utility module — `features/database/lib/dnd.ts`

Thin wrapper that centralizes the pragmatic-drag-and-drop imports so feature code doesn't sprawl. Just re-exports + typed `useDraggable`/`useDropTarget` React helpers adapted to our codebase. Keeping this small on purpose — we don't need a mega-hook; phase 4 will use the raw APIs directly where helpful.

For phase 2, this file exists but is near-empty: just a sanity-check re-export plus a `createDragSource()` helper to pin the bundle shape. Populated in phase 4.

## Verification

1. `cd desktop-ui && bun install` — confirm `bun.lockb` updates cleanly.
2. `bun run lint` — Biome passes with zero errors.
3. `bun run test` — existing Vitest suite unchanged.
4. `bun run build` — production build succeeds. Check bundle delta: pragmatic + fractional + tanstack should add ~25kB gz; flag if >40kB.
5. Manual: open `/database/tasks`, confirm no visual regression, no console errors, existing views still work.
6. Call `db_reorder_entity` from the browser console to verify the hook wires end-to-end:
   ```js
   const r = await (await fetch('/api/db_reorder_entity', {
     method: 'POST',
     headers: {'content-type': 'application/json'},
     body: JSON.stringify({
       databaseId: '<id>', entityId: '<id>', beforeId: null, afterId: '<id>'
     })
   })).json();
   ```
   Expect: 200 + updated entity with new `position` key.

## Phase 2 success criteria

- [ ] All 6 libs installed, tree-shake confirmed (bundle size delta <40kB gz).
- [ ] `Entity`, `ViewConfig`, `QueryParams` widened with new fields.
- [ ] `FilterNode` / `FilterGroup` types match backend tagged-enum serialization.
- [ ] `useEntityReorder`, `useEntityPatch`, `useReorderViews` hooks present.
- [ ] `useEntities` passes the `filter` field through.
- [ ] `ordering.ts` helper with `keyBetween` + `keyForMove`.
- [ ] Database page renders unchanged.
- [ ] Browser-console test of `db_reorder_entity` returns a new position key.

## File-level impact

**Created:**
- `desktop-ui/src/features/database/hooks/useEntityReorder.ts`
- `desktop-ui/src/features/database/hooks/useEntityPatch.ts`
- `desktop-ui/src/features/database/hooks/useReorderViews.ts`
- `desktop-ui/src/features/database/lib/ordering.ts`
- `desktop-ui/src/features/database/lib/dnd.ts`

**Modified:**
- `desktop-ui/package.json` (6 new deps)
- `desktop-ui/src/shared/types/database.ts` (Entity.position, FilterNode, FilterGroup, ViewConfig widened, QueryParams widened)
- `desktop-ui/src/features/database/hooks/useEntities.ts` (pass `filter` through)

**Deleted:** none.

## Step-by-step execution

1. **Install libs.** `bun add @atlaskit/pragmatic-drag-and-drop @atlaskit/pragmatic-drag-and-drop-hitbox @atlaskit/pragmatic-drag-and-drop-auto-scroll fractional-indexing-jittered @tanstack/react-table @tanstack/react-virtual`. Verify `bun run build` still passes.
2. **Widen types** in `database.ts`. Run `bun run lint` — expect zero errors since new fields are optional.
3. **Thread `filter` through `useEntities`.** One-line change.
4. **Add hook files** (`useEntityReorder`, `useEntityPatch`, `useReorderViews`). Model on existing `useEntity` hooks for consistency.
5. **Add `ordering.ts`**. Unit-test manually in browser console: `keyBetween(null, null)` returns a valid key; `keyBetween('a0', 'a1')` returns something between.
6. **Add `dnd.ts` shell**. Just re-exports for now.
7. **Verify** per checklist above.
8. **Stop and report**.

## Open questions (assumed defaults — flag to renegotiate)

- **Entity `position` should be non-optional in TS** (matches backend, which always returns it now). OK? If backend rolls back, this is a lie.
- **`useEntityPatch` as alias vs separate hook?** Alias for now; promote to its own implementation if optimistic-update wiring diverges in phase 4.
- **Should `useReorderViews` emit a different event than `DATABASE_UPDATED`?** Probably not — view config is part of the schema, schema change → full refetch is fine at our scale.
- **Do we need `useEntityQuery` that returns `FilterGroup`-aware results?** Not yet; `useEntities` with `filter` param covers it.

## After phase 2

Phase 3 (view CRUD UI) builds the Notion-style picker on top of `useCreateView`/`useDeleteView`/`useReorderViews`. Phase 4 (DnD) consumes `useEntityReorder` + `ordering.ts` + `dnd.ts`. Nothing in phases 3–8 should need to touch types or add new hooks at this layer.
