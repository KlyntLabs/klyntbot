# Notion View System — Phase 6: Grouping on All Views

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable `groupBy` on Table, List, and Gallery views with collapsible group sections, matching the existing Board behavior, persisted in `ViewConfig`.

**Architecture:** Build one reusable `groupEntities()` helper. Lift the Board-only groupBy selector in `ViewConfigPanel` to all applicable view types and widen its field-type filter. Add a small `useCollapsedGroups(viewId)` hook that reads/writes `view.config.collapsedGroups` (debounced). Each view renders an array of group sections; in the Table, groups are flattened into a `(header | row)` virtualizer item list so TanStack Virtual still works. Board is already grouped — we only ensure the selector still drives it.

**Tech Stack:** React + TypeScript, TanStack Table/Virtual, existing `useUpdateView` IPC. No new deps.

---

## Scope & non-goals

- **In scope:** grouping by `select`, `multi_select`, `checkbox`, `status` (if it exists as its own type; else treated as select). Entities missing the group field go into a single "No value" group at the end. Group order follows the field's option order when available, else alphabetical by label, with "No value" last. Collapsed keys persist per view.
- **Out of scope:** date bucketing (year/month), numeric range bucketing, drag-between-groups (group-change via DnD stays a Phase 7 item), Board UI changes, Calendar/Timeline grouping.

## File structure

| File | Responsibility |
|---|---|
| `desktop-ui/src/features/database/lib/grouping.ts` (new) | `groupEntities(entities, schema, groupBy)` → `GroupBucket[]`; single source of truth. |
| `desktop-ui/src/features/database/hooks/useCollapsedGroups.ts` (new) | Local state + debounced persist of `view.config.collapsedGroups`. |
| `desktop-ui/src/features/database/components/views/GroupHeader.tsx` (new) | Shared collapse/expand header row (chevron + label + count). |
| `desktop-ui/src/features/database/components/views/ViewConfigPanel.tsx` (modify) | Show groupBy selector on table/list/gallery/board; widen allowed field types. |
| `desktop-ui/src/features/database/components/views/ListView.tsx` (modify) | Render grouped sections when `view.config.groupBy` is set. |
| `desktop-ui/src/features/database/components/views/GalleryView.tsx` (modify) | Same — grouped sections above each grid. |
| `desktop-ui/src/features/database/components/views/table/TableView.tsx` (modify) | Flatten into `(header \| row)` items for virtualizer; render group headers. |
| `desktop-ui/src/features/database/components/views/table/TableBodyRow.tsx` (read-only ref) | Unchanged — still renders a normal row. |
| Views still passing `view.config.groupBy` to BoardView (`ViewShell.tsx`) (verify only) | Make sure the widened selector feeds Board too. |

## Conventions

- Grouping is **pure frontend** for Phase 6. No backend changes. The IPC already returns the full entity list; we bucket in memory.
- Sorts (if any) apply **within** each group.
- Use existing `useUpdateView` for persistence — debounce writes to `collapsedGroups` by 400ms using the same `useRef`-timer pattern already used in `useTableColumns.ts`.
- Style tokens only. `bg-surface-base`, `text-muted`, `border-border`. No raw hex.

---

## Task 1: Grouping helper

**Files:**
- Create: `desktop-ui/src/features/database/lib/grouping.ts`
- Test: `desktop-ui/src/features/database/lib/grouping.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// desktop-ui/src/features/database/lib/grouping.test.ts
import { describe, expect, it } from "vitest";
import type { DatabaseSchema, Entity } from "@shared/types";
import { groupEntities, NO_VALUE_GROUP_KEY } from "./grouping";

const schema = {
  id: "db1",
  name: "Tasks",
  fields: [
    {
      id: "f1",
      slug: "status",
      name: "Status",
      fieldType: "select",
      config: { options: [
        { id: "o1", value: "todo", label: "Todo" },
        { id: "o2", value: "done", label: "Done" },
      ] },
    },
    {
      id: "f2",
      slug: "tags",
      name: "Tags",
      fieldType: "multi_select",
      config: { options: [
        { id: "t1", value: "urgent", label: "Urgent" },
        { id: "t2", value: "home", label: "Home" },
      ] },
    },
  ],
  views: [],
} as unknown as DatabaseSchema;

const entities: Entity[] = [
  { id: "a", databaseId: "db1", fields: { status: "todo", tags: ["urgent"] } } as Entity,
  { id: "b", databaseId: "db1", fields: { status: "done", tags: ["urgent", "home"] } } as Entity,
  { id: "c", databaseId: "db1", fields: { status: null } } as Entity,
];

describe("groupEntities", () => {
  it("groups by select in option order with No value last", () => {
    const groups = groupEntities(entities, schema, "status");
    expect(groups.map((g) => g.key)).toEqual(["todo", "done", NO_VALUE_GROUP_KEY]);
    expect(groups.map((g) => g.entities.map((e) => e.id))).toEqual([["a"], ["b"], ["c"]]);
  });

  it("fans entities across every multi_select value", () => {
    const groups = groupEntities(entities, schema, "tags");
    const urgent = groups.find((g) => g.key === "urgent");
    const home = groups.find((g) => g.key === "home");
    expect(urgent?.entities.map((e) => e.id).sort()).toEqual(["a", "b"]);
    expect(home?.entities.map((e) => e.id)).toEqual(["b"]);
  });

  it("returns a single No value group when field missing", () => {
    const groups = groupEntities(entities, schema, "nonexistent");
    expect(groups).toHaveLength(1);
    expect(groups[0]?.key).toBe(NO_VALUE_GROUP_KEY);
    expect(groups[0]?.entities).toHaveLength(3);
  });
});
```

- [ ] **Step 2: Run test, verify it fails**

```
cd desktop-ui && bun run test -- grouping
```
Expected: FAIL with "Cannot find module './grouping'".

- [ ] **Step 3: Implement `grouping.ts`**

```ts
// desktop-ui/src/features/database/lib/grouping.ts
import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";

export const NO_VALUE_GROUP_KEY = "__no_value__";

export interface GroupBucket {
  key: string;           // stable identifier (option value, "true"/"false", or NO_VALUE_GROUP_KEY)
  label: string;         // user-facing
  entities: Entity[];
}

type Option = { value: string; label: string };

function fieldOptions(field: FieldDefinition): Option[] {
  const raw = (field.config as { options?: Option[] } | undefined)?.options;
  return Array.isArray(raw) ? raw : [];
}

function emptyBucket(): GroupBucket {
  return { key: NO_VALUE_GROUP_KEY, label: "No value", entities: [] };
}

function groupBySelect(entities: Entity[], field: FieldDefinition): GroupBucket[] {
  const options = fieldOptions(field);
  const buckets = new Map<string, GroupBucket>();
  for (const opt of options) {
    buckets.set(opt.value, { key: opt.value, label: opt.label, entities: [] });
  }
  const empty = emptyBucket();
  for (const e of entities) {
    const v = e.fields[field.slug];
    if (v === null || v === undefined || v === "") {
      empty.entities.push(e);
      continue;
    }
    const key = String(v);
    let bucket = buckets.get(key);
    if (!bucket) {
      bucket = { key, label: key, entities: [] };
      buckets.set(key, bucket);
    }
    bucket.entities.push(e);
  }
  const ordered = [...buckets.values()];
  if (empty.entities.length > 0) ordered.push(empty);
  return ordered;
}

function groupByMultiSelect(entities: Entity[], field: FieldDefinition): GroupBucket[] {
  const options = fieldOptions(field);
  const buckets = new Map<string, GroupBucket>();
  for (const opt of options) {
    buckets.set(opt.value, { key: opt.value, label: opt.label, entities: [] });
  }
  const empty = emptyBucket();
  for (const e of entities) {
    const raw = e.fields[field.slug];
    const values = Array.isArray(raw) ? raw.map(String) : [];
    if (values.length === 0) {
      empty.entities.push(e);
      continue;
    }
    for (const v of values) {
      let bucket = buckets.get(v);
      if (!bucket) {
        bucket = { key: v, label: v, entities: [] };
        buckets.set(v, bucket);
      }
      bucket.entities.push(e);
    }
  }
  const ordered = [...buckets.values()].filter((b) => b.entities.length > 0);
  if (empty.entities.length > 0) ordered.push(empty);
  return ordered;
}

function groupByCheckbox(entities: Entity[], field: FieldDefinition): GroupBucket[] {
  const yes: GroupBucket = { key: "true", label: "Checked", entities: [] };
  const no: GroupBucket = { key: "false", label: "Unchecked", entities: [] };
  for (const e of entities) {
    if (e.fields[field.slug] === true) yes.entities.push(e);
    else no.entities.push(e);
  }
  return [yes, no].filter((b) => b.entities.length > 0);
}

export function groupEntities(
  entities: Entity[],
  schema: DatabaseSchema,
  groupBy: string | undefined,
): GroupBucket[] {
  if (!groupBy) return [{ key: "all", label: "", entities }];
  const field = schema.fields.find((f) => f.slug === groupBy);
  if (!field) {
    const bucket = emptyBucket();
    bucket.entities = entities;
    return [bucket];
  }
  switch (field.fieldType) {
    case "multi_select":
      return groupByMultiSelect(entities, field);
    case "checkbox":
      return groupByCheckbox(entities, field);
    // status treated like select if present
    default:
      return groupBySelect(entities, field);
  }
}
```

- [ ] **Step 4: Run test, verify PASS**

```
cd desktop-ui && bun run test -- grouping
```
Expected: 3 passed.

- [ ] **Step 5: Lint**

```
cd desktop-ui && bun run lint
```
Expected: clean.

---

## Task 2: `useCollapsedGroups` hook

**Files:**
- Create: `desktop-ui/src/features/database/hooks/useCollapsedGroups.ts`

- [ ] **Step 1: Implement the hook**

```ts
// desktop-ui/src/features/database/hooks/useCollapsedGroups.ts
import { useCallback, useEffect, useRef, useState } from "react";
import type { ViewDefinition } from "@shared/types";
import { useUpdateView } from "./useViews";

const PERSIST_DEBOUNCE_MS = 400;

export function useCollapsedGroups(databaseId: string, view: ViewDefinition) {
  const initial = view.config.collapsedGroups ?? [];
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set(initial));
  const updateView = useUpdateView(databaseId);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latest = useRef<string[]>(initial);

  const persist = useCallback(
    (next: string[]) => {
      latest.current = next;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        updateView.mutate({
          viewId: view.id,
          patch: { config: { ...view.config, collapsedGroups: latest.current } },
        });
      }, PERSIST_DEBOUNCE_MS);
    },
    [updateView, view.id, view.config],
  );

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  const toggle = useCallback(
    (key: string) => {
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(key)) next.delete(key);
        else next.add(key);
        persist([...next]);
        return next;
      });
    },
    [persist],
  );

  return { collapsed, toggle };
}
```

- [ ] **Step 2: Verify `useUpdateView` signature**

Open `desktop-ui/src/features/database/hooks/useViews.ts` and confirm the call shape (`mutate({ viewId, patch })`). If the real signature differs (e.g. positional args), correct Step 1 to match before proceeding.

- [ ] **Step 3: Lint**

```
cd desktop-ui && bun run lint
```
Expected: clean.

---

## Task 3: Shared `GroupHeader` component

**Files:**
- Create: `desktop-ui/src/features/database/components/views/GroupHeader.tsx`

- [ ] **Step 1: Implement**

```tsx
// desktop-ui/src/features/database/components/views/GroupHeader.tsx
import { ChevronRight } from "lucide-react";

interface Props {
  label: string;
  count: number;
  collapsed: boolean;
  onToggle: () => void;
}

export function GroupHeader({ label, count, collapsed, onToggle }: Props) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className="flex w-full items-center gap-2 bg-surface-base px-3 py-1.5 text-[12px] font-medium text-foreground/70 hover:text-foreground"
    >
      <ChevronRight
        size={14}
        className={`transition-transform ${collapsed ? "" : "rotate-90"}`}
      />
      <span>{label || "No value"}</span>
      <span className="text-foreground/50">{count}</span>
    </button>
  );
}
```

- [ ] **Step 2: Lint**

```
cd desktop-ui && bun run lint
```
Expected: clean.

---

## Task 4: Expose `groupBy` selector in `ViewConfigPanel`

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/ViewConfigPanel.tsx` (lines 85–94)

- [ ] **Step 1: Read the file** to confirm current structure around the groupBy selector and the `view.viewType === "board"` guard.

- [ ] **Step 2: Replace the board-only guard**

Change the block that currently renders the GroupBy selector only when `view.viewType === "board"`:

```tsx
{["board", "table", "list", "gallery"].includes(view.viewType) && (
  <LabeledRow label="Group by">
    <FieldSelect
      schema={schema}
      value={view.config.groupBy}
      allowedTypes={["select", "multi_select", "checkbox"]}
      onChange={(v) => updateConfig({ groupBy: v })}
      allowClear
    />
  </LabeledRow>
)}
```

If `FieldSelect` does not already support `allowClear` or `allowedTypes` with multiple values, open it and add support. Keep the component minimal.

- [ ] **Step 3: Lint + typecheck**

```
cd desktop-ui && bun run lint
```
Expected: clean.

- [ ] **Step 4: Manual smoke (browser)**

Load the app, open a database, open the view config panel for a Table view, and confirm the Group by dropdown is visible and offers select/multi_select/checkbox fields. Clearing it removes the group. No runtime errors in the console.

---

## Task 5: Group `ListView`

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/ListView.tsx`

- [ ] **Step 1: Wrap list rendering in group sections**

Replace the container that maps entities with:

```tsx
import { groupEntities } from "@features/database/lib/grouping";
import { useCollapsedGroups } from "@features/database/hooks/useCollapsedGroups";
import { GroupHeader } from "./GroupHeader";

// inside component body, `view`, `schema`, `entities`, `databaseId` already in scope
const groups = groupEntities(entities, schema, view.config.groupBy);
const { collapsed, toggle } = useCollapsedGroups(databaseId, view);
const grouped = Boolean(view.config.groupBy);

return (
  <div className="w-full">
    {groups.map((g) => (
      <section key={g.key}>
        {grouped && (
          <GroupHeader
            label={g.label}
            count={g.entities.length}
            collapsed={collapsed.has(g.key)}
            onToggle={() => toggle(g.key)}
          />
        )}
        {(!grouped || !collapsed.has(g.key)) && (
          <div className="w-full divide-y divide-border/40">
            {g.entities.map((entity) => (
              <ListRow
                key={entity.id}
                entity={entity}
                /* ...existing props... */
              />
            ))}
          </div>
        )}
      </section>
    ))}
  </div>
);
```

Adjust `ListRow` import / props to match the existing file — do not change the row component. `databaseId` is `schema.id`; if the prop isn't passed yet, read it from `schema.id` directly inside the component.

- [ ] **Step 2: Lint**

```
cd desktop-ui && bun run lint
```
Expected: clean.

- [ ] **Step 3: Manual smoke**

In browser, switch a list view's Group by to `status`. Confirm: headers appear with correct counts, collapse persists across a reload, entities without status fall into a trailing "No value" group.

---

## Task 6: Group `GalleryView`

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/GalleryView.tsx`

- [ ] **Step 1: Wrap the grid in group sections**

Replace the single grid container with one grid per group:

```tsx
import { groupEntities } from "@features/database/lib/grouping";
import { useCollapsedGroups } from "@features/database/hooks/useCollapsedGroups";
import { GroupHeader } from "./GroupHeader";

const groups = groupEntities(entities, schema, view.config.groupBy);
const { collapsed, toggle } = useCollapsedGroups(schema.id, view);
const grouped = Boolean(view.config.groupBy);

return (
  <div className="w-full">
    {groups.map((g) => (
      <section key={g.key} className="pb-4">
        {grouped && (
          <GroupHeader
            label={g.label}
            count={g.entities.length}
            collapsed={collapsed.has(g.key)}
            onToggle={() => toggle(g.key)}
          />
        )}
        {(!grouped || !collapsed.has(g.key)) && (
          <div className="grid grid-cols-2 gap-4 p-4 md:grid-cols-3 lg:grid-cols-4">
            {g.entities.map((entity) => (
              <GalleryCard key={entity.id} entity={entity} /* existing props */ />
            ))}
          </div>
        )}
      </section>
    ))}
  </div>
);
```

Keep the existing card component untouched. Match whichever class list the current file uses (read before editing) — do not silently change breakpoints.

- [ ] **Step 2: Lint + smoke**

```
cd desktop-ui && bun run lint
```
Browser: switch a gallery view's Group by to `tags` (multi_select). Confirm a card that has two tags appears in both groups.

---

## Task 7: Group `TableView` with virtualization

**Files:**
- Modify: `desktop-ui/src/features/database/components/views/table/TableView.tsx`

**Approach:** build a flat `items: Array<{ type: "header"; key: string; label: string; count: number; collapsed: boolean } | { type: "row"; row: Row<Entity> }>` and feed `items.length` to `useVirtualizer`. `estimateSize` returns `HEADER_HEIGHT` (32) for headers and `ROW_HEIGHT` (36) for rows. Rows inside a collapsed group are omitted from the array entirely.

- [ ] **Step 1: Build the items array**

After `table.getRowModel()` returns `rows`, construct:

```ts
import { groupEntities } from "@features/database/lib/grouping";
import { useCollapsedGroups } from "@features/database/hooks/useCollapsedGroups";
import { GroupHeader } from "../GroupHeader";

const ROW_HEIGHT = 36;
const HEADER_HEIGHT = 32;

type FlatItem =
  | { type: "header"; key: string; label: string; count: number; collapsed: boolean }
  | { type: "row"; row: Row<Entity> };

const groups = groupEntities(entities, schema, view.config.groupBy);
const { collapsed, toggle } = useCollapsedGroups(schema.id, view);
const grouped = Boolean(view.config.groupBy);

const items = useMemo<FlatItem[]>(() => {
  if (!grouped) {
    return rows.map((row) => ({ type: "row" as const, row }));
  }
  const rowById = new Map(rows.map((r) => [r.original.id, r]));
  const out: FlatItem[] = [];
  for (const g of groups) {
    const isCollapsed = collapsed.has(g.key);
    out.push({
      type: "header",
      key: g.key,
      label: g.label,
      count: g.entities.length,
      collapsed: isCollapsed,
    });
    if (!isCollapsed) {
      for (const entity of g.entities) {
        const row = rowById.get(entity.id);
        if (row) out.push({ type: "row", row });
      }
    }
  }
  return out;
}, [groups, rows, collapsed, grouped]);
```

- [ ] **Step 2: Drive the virtualizer from `items`**

Change the virtualizer config:

```ts
const virtualizer = useVirtualizer({
  count: items.length,
  getScrollElement: () => parentRef.current,
  estimateSize: (index) => (items[index]?.type === "header" ? HEADER_HEIGHT : ROW_HEIGHT),
  overscan: 8,
});
```

- [ ] **Step 3: Render headers and rows**

Inside the virtualizer loop replace the existing row render with a switch:

```tsx
{virtualizer.getVirtualItems().map((vi) => {
  const item = items[vi.index];
  if (!item) return null;
  if (item.type === "header") {
    return (
      <div
        key={`h:${item.key}`}
        className="absolute left-0 top-0 w-full"
        style={{ transform: `translateY(${vi.start}px)`, height: HEADER_HEIGHT }}
      >
        <GroupHeader
          label={item.label}
          count={item.count}
          collapsed={item.collapsed}
          onToggle={() => toggle(item.key)}
        />
      </div>
    );
  }
  return (
    <TableBodyRow
      key={item.row.id}
      row={item.row}
      schema={schema}
      top={vi.start}
      onClick={() => onEntityClick(item.row.original)}
    />
  );
})}
```

- [ ] **Step 4: Scroll container height**

Confirm the scroll container still uses `virtualizer.getTotalSize()` for its spacer height. No additional change needed — TanStack Virtual handles the mixed sizes via `estimateSize`.

- [ ] **Step 5: DnD note**

Row reorder DnD (Phase 4) must stay intact for the ungrouped case. When `grouped === true`, leave DnD enabled but understand cross-group drags still just reorder by position — field value is **not** rewritten (deferred). No code change needed; call it out in the PR description.

- [ ] **Step 6: Lint + tests**

```
cd desktop-ui && bun run lint && bun run test
```
Expected: clean, all tests pass.

- [ ] **Step 7: Manual smoke**

Browser: switch a Table view's Group by to `status`. Confirm:
- Header rows appear between groups.
- Collapsing a group hides its rows and reduces total scroll height.
- Reload → collapse state restored.
- Ungrouped mode still virtualizes 500+ rows smoothly.

---

## Task 8: Verify Board still works

**Files:** read-only checks.

- [ ] **Step 1:** Open a Board view. Confirm the widened GroupBy selector in `ViewConfigPanel` still drives BoardView's columns (it reads `view.config.groupBy`). No code change expected.

- [ ] **Step 2:** If Board breaks because it assumed `allowedTypes={["select"]}` only, constrain BoardView itself to fall back to a single "All" column when `groupBy` points at a non-select field — **do not** narrow the shared selector.

---

## Self-review checklist (run before handing off)

- [ ] All new files present, no `TODO`/`later`/`similar to` placeholders.
- [ ] `grouping.test.ts` passes.
- [ ] Types used consistently: `GroupBucket`, `NO_VALUE_GROUP_KEY`, `useCollapsedGroups(databaseId, view)`, `GroupHeader({ label, count, collapsed, onToggle })`.
- [ ] No raw hex / rgba introduced; only theme tokens.
- [ ] `view.config.groupBy` and `view.config.collapsedGroups` already exist in `ViewConfig` — confirmed during scoping; no type changes needed.
- [ ] Persistence uses existing `useUpdateView` — no new IPC.
- [ ] BoardView not broken by widened selector.

## Verification

```
cd desktop-ui && bun run lint
cd desktop-ui && bun run test
```

Browser matrix on a Tasks database with ≥50 entities:

1. Table + Group by `status` — headers + collapse + persistence.
2. Table ungrouped — row virtualization unchanged.
3. List + Group by `status` — same expectations.
4. Gallery + Group by `tags` (multi_select) — entity with multiple tags appears in each group.
5. Board — still columns by groupBy.

All phases per the master plan remain uncommitted until the user explicitly approves.
