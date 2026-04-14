# Phase 5 — TableView on TanStack Table + Virtual

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite TableView on `@tanstack/react-table` + `@tanstack/react-virtual` to get Notion-grade column management (resize, reorder, pin, visibility), smooth scrolling for 10k+ rows, and a sticky header — while preserving the existing `@dnd-kit` row reorder and the `ViewConfig.columnWidths` / `columnOrder` persistence already in the schema.

**Architecture:** Keep the outer DndContext + SortableContext for rows. Replace the hand-rolled `<colgroup>` + `<thead>` + `<tbody>` with a TanStack Table model driven by field definitions. Use flex divs for row layout (not `<table>`) so column widths and virtualization play together cleanly. Virtualize the row list via `useVirtualizer`. Persist per-column width via `view.config.columnWidths` and a debounced `updateView` call.

**Out of scope (follow-up phase):** drag-to-reorder columns and pin-column UI. The ViewConfig already holds `columnOrder`/`pinnedColumn` so wiring them later won't require schema changes.

**Tech Stack:** `@tanstack/react-table@8`, `@tanstack/react-virtual@3`, `@dnd-kit/core` + `@dnd-kit/sortable` (kept), Tailwind v4.

---

## File Structure

- **Create**: `desktop-ui/src/features/database/components/views/table/TableView.tsx` — new entry component, thin composition of header + virtualized body + DnD + column config wiring. Replaces current `views/TableView.tsx` (which is deleted at the end).
- **Create**: `desktop-ui/src/features/database/components/views/table/useTableColumns.ts` — hook that builds TanStack `ColumnDef<Entity>[]` from schema + view config, and wires width/order/sort/pinning state.
- **Create**: `desktop-ui/src/features/database/components/views/table/TableHeaderRow.tsx` — sticky header row with sort toggle, drag-to-reorder columns, and resize grip.
- **Create**: `desktop-ui/src/features/database/components/views/table/TableBodyRow.tsx` — single virtualized sortable row (ports `SortableRow` from the current file).
- **Create**: `desktop-ui/src/features/database/lib/columnDefaults.ts` — field-type → default width map (ports `getColumnStyle` as numeric widths).
- **Modify**: `desktop-ui/src/features/database/components/ViewShell.tsx:121-131` — update import path and the `TableView` call site only.
- **Modify**: `desktop-ui/src/features/database/hooks/useViews.ts` — if `useUpdateView` doesn't already exist or isn't exposed, confirm it's reused from here.
- **Modify**: `desktop-ui/src/shared/types/database.ts` — no schema changes; `columnWidths` + `columnOrder` + `pinnedColumn` already present (verify pinnedColumn; add if missing).
- **Delete**: `desktop-ui/src/features/database/components/views/TableView.tsx` — old implementation (after new one is wired).

**Why split from one file into four:** The current 202-line TableView bundles header markup, row markup, column-width heuristics, sort toggling, DnD wiring, and empty state. TanStack will grow all of those (resize, reorder, pinning, virtualization), so the file would cross 400 lines. Splitting by responsibility keeps each file under ~150 lines and lets the table primitives be tested in isolation.

---

## Task 1: Create `columnDefaults.ts`

**Files:**
- Create: `desktop-ui/src/features/database/lib/columnDefaults.ts`

- [ ] **Step 1: Write the module**

```ts
import type { FieldDefinition } from "@shared/types";

/** Default pixel widths per field type — used when the view has no explicit width. */
export function defaultColumnWidth(field: FieldDefinition, isTitle: boolean): number {
  if (isTitle) return 280;
  switch (field.fieldType) {
    case "text":
      return 180;
    case "select":
    case "multi_select":
      return 140;
    case "number":
    case "date":
    case "created_time":
    case "last_edited":
      return 140;
    case "checkbox":
      return 60;
    default:
      return 140;
  }
}

export const MIN_COLUMN_WIDTH = 60;
export const MAX_COLUMN_WIDTH = 800;
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/database/lib/columnDefaults.ts
git commit -m "feat(desktop-ui): add column-width defaults for table view"
```

---

## Task 2: Create `useTableColumns` hook

**Files:**
- Create: `desktop-ui/src/features/database/components/views/table/useTableColumns.ts`

- [ ] **Step 1: Write the hook**

```ts
import { useUpdateView } from "@features/database/hooks/useViews";
import {
  MAX_COLUMN_WIDTH,
  MIN_COLUMN_WIDTH,
  defaultColumnWidth,
} from "@features/database/lib/columnDefaults";
import { getTitleField } from "@features/database/lib/schema-utils";
import {
  type ColumnDef,
  type ColumnSizingState,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import type { DatabaseSchema, Entity, ViewDefinition } from "@shared/types";
import { useEffect, useMemo, useRef, useState } from "react";

interface Args {
  schema: DatabaseSchema;
  entities: Entity[];
  view: ViewDefinition;
}

/** Persist column-width changes back to `view.config` (debounced). */
function useDebouncedPersist(schemaId: string, viewId: string) {
  const { mutate: updateView } = useUpdateView(schemaId);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  return (patch: Record<string, unknown>) => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void updateView(viewId, { config: patch });
    }, 400);
  };
}

export function useTableColumns({ schema, entities, view }: Args) {
  const titleField = getTitleField(schema);
  const persist = useDebouncedPersist(schema.id, view.id);

  const visibleFields = useMemo(() => {
    const visible = view.config.visibleFields;
    if (visible && visible.length > 0) {
      return schema.fields.filter((f) => visible.includes(f.slug) && !f.hidden);
    }
    return schema.fields.filter((f) => !f.hidden);
  }, [schema.fields, view.config.visibleFields]);

  const columns = useMemo<ColumnDef<Entity>[]>(
    () =>
      visibleFields.map((field) => ({
        id: field.slug,
        accessorFn: (e) => e.fields[field.slug],
        header: field.name,
        size:
          view.config.columnWidths?.[field.slug] ??
          defaultColumnWidth(field, titleField?.slug === field.slug),
        minSize: MIN_COLUMN_WIDTH,
        maxSize: MAX_COLUMN_WIDTH,
        meta: { field },
      })),
    [visibleFields, view.config.columnWidths, titleField],
  );

  const [columnSizing, setColumnSizing] = useState<ColumnSizingState>(
    () => view.config.columnWidths ?? {},
  );

  useEffect(() => {
    persist({ columnWidths: columnSizing });
  }, [columnSizing, persist]);

  const table = useReactTable({
    data: entities,
    columns,
    state: { columnSizing },
    onColumnSizingChange: setColumnSizing,
    columnResizeMode: "onChange",
    getCoreRowModel: getCoreRowModel(),
  });

  return { table, visibleFields };
}

declare module "@tanstack/react-table" {
  // biome-ignore lint/correctness/noUnusedVariables: augmentation signature requires TValue
  interface ColumnMeta<TData, TValue> {
    field: import("@shared/types").FieldDefinition;
  }
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | grep table/useTableColumns`
Expected: no errors (or only unused-import warnings to be cleared in later tasks).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/database/components/views/table/useTableColumns.ts
git commit -m "feat(desktop-ui): useTableColumns hook for TanStack table state"
```

---

## Task 3: Create `TableHeaderRow`

**Files:**
- Create: `desktop-ui/src/features/database/components/views/table/TableHeaderRow.tsx`

- [ ] **Step 1: Write the header row**

```tsx
import type { Header, Table } from "@tanstack/react-table";
import type { Entity, SortRule } from "@shared/types";

interface Props {
  table: Table<Entity>;
  sorts: SortRule[] | undefined;
  onSortChange: ((sorts: SortRule[]) => void) | undefined;
}

export function TableHeaderRow({ table, sorts, onSortChange }: Props) {
  const toggleSort = (slug: string) => {
    if (!onSortChange) return;
    const existing = sorts?.find((s) => s.field === slug);
    if (!existing) onSortChange([{ field: slug, direction: "asc" }]);
    else if (existing.direction === "asc") onSortChange([{ field: slug, direction: "desc" }]);
    else onSortChange([]);
  };

  const totalWidth = table.getTotalSize();

  return (
    <div
      className="sticky top-0 z-10 flex border-b border-border bg-background text-[12px] font-medium text-foreground/70"
      style={{ width: totalWidth }}
    >
      {table.getFlatHeaders().map((header) => (
        <HeaderCell
          key={header.id}
          header={header}
          rule={sorts?.find((s) => s.field === header.id)}
          onToggle={() => toggleSort(header.id)}
        />
      ))}
    </div>
  );
}

function HeaderCell({
  header,
  rule,
  onToggle,
}: {
  header: Header<Entity, unknown>;
  rule: SortRule | undefined;
  onToggle: () => void;
}) {
  return (
    <div
      className="relative flex items-center border-r border-border/40 px-3 py-2 transition-colors hover:bg-accent hover:text-foreground"
      style={{ width: header.getSize() }}
    >
      <button
        type="button"
        onClick={onToggle}
        className="flex-1 truncate text-left cursor-pointer select-none"
      >
        {String(header.column.columnDef.header)}
        {rule && (
          <span className={`ml-1 text-[10px] ${rule.direction === "desc" ? "rotate-180 inline-block" : ""}`}>
            ▲
          </span>
        )}
      </button>
      <div
        onMouseDown={header.getResizeHandler()}
        onTouchStart={header.getResizeHandler()}
        className={`absolute right-0 top-0 h-full w-[4px] cursor-col-resize select-none touch-none ${
          header.column.getIsResizing() ? "bg-accent" : "hover:bg-accent/60"
        }`}
        aria-hidden="true"
      />
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | grep "table/Table"`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/database/components/views/table/TableHeaderRow.tsx
git commit -m "feat(desktop-ui): TableHeaderRow with sort + resize handles"
```

---

## Task 4: Create `TableBodyRow`

**Files:**
- Create: `desktop-ui/src/features/database/components/views/table/TableBodyRow.tsx`

- [ ] **Step 1: Write the row**

```tsx
import { FieldRenderer } from "@features/database/components/fields/FieldRenderer";
import { getTitleField } from "@features/database/lib/schema-utils";
import { useSortableEntity } from "@features/database/lib/useSortableEntity";
import type { Row } from "@tanstack/react-table";
import type { DatabaseSchema, Entity, FieldDefinition } from "@shared/types";

interface Props {
  row: Row<Entity>;
  schema: DatabaseSchema;
  top: number;
  onClick: () => void;
}

export function TableBodyRow({ row, schema, top, onClick }: Props) {
  const entity = row.original;
  const { setNodeRef, style: dragStyle, dragProps } = useSortableEntity(entity.id);
  const titleSlug = getTitleField(schema)?.slug;

  return (
    <div
      ref={setNodeRef}
      data-entity-id={entity.id}
      onClick={onClick}
      className="absolute left-0 top-0 flex cursor-pointer border-b border-border/40 transition-colors hover:bg-accent/60"
      style={{
        ...dragStyle,
        transform: `translateY(${top}px) ${dragStyle.transform ?? ""}`,
        width: row.getAllCells().reduce((w, c) => w + c.column.getSize(), 0),
      }}
      {...dragProps}
    >
      {row.getVisibleCells().map((cell) => {
        const field = cell.column.columnDef.meta?.field as FieldDefinition | undefined;
        if (!field) return null;
        const isTitle = field.slug === titleSlug;
        return (
          <div
            key={cell.id}
            className={`overflow-hidden text-ellipsis whitespace-nowrap border-r border-border/40 px-3 py-2 text-[13px] ${
              isTitle ? "font-medium text-foreground" : "font-normal text-foreground/85"
            }`}
            style={{ width: cell.column.getSize() }}
          >
            <FieldRenderer field={field} value={entity.fields[field.slug]} />
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | grep "table/TableBodyRow"`
Expected: no errors. `ColumnMeta.field` is already declared in `useTableColumns.ts` (Task 2).

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/database/components/views/table/TableBodyRow.tsx
git commit -m "feat(desktop-ui): TableBodyRow with absolute positioning for virtualization"
```

---

## Task 5: Create the new `TableView`

**Files:**
- Create: `desktop-ui/src/features/database/components/views/table/TableView.tsx`

- [ ] **Step 1: Write the composition**

```tsx
import {
  DndContext,
  type DragEndEvent,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { computeReorderAnchors } from "@features/database/lib/ordering";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { DatabaseSchema, Entity, SortRule, ViewDefinition } from "@shared/types";
import { useMemo, useRef } from "react";
import { TableBodyRow } from "./TableBodyRow";
import { TableHeaderRow } from "./TableHeaderRow";
import { useTableColumns } from "./useTableColumns";

const ROW_HEIGHT = 36;

interface Props {
  schema: DatabaseSchema;
  view: ViewDefinition;
  entities: Entity[];
  sorts: SortRule[] | undefined;
  onSortChange: ((sorts: SortRule[]) => void) | undefined;
  onEntityClick: (entity: Entity) => void;
}

export function TableView({ schema, view, entities, sorts, onSortChange, onEntityClick }: Props) {
  const { table } = useTableColumns({ schema, entities, view });
  const parentRef = useRef<HTMLDivElement>(null);

  const rows = table.getRowModel().rows;
  const orderedIds = useMemo(() => rows.map((r) => r.original.id), [rows]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 8,
  });

  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
  const reorder = useEntityReorder(schema.id);
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over) return;
    const anchors = computeReorderAnchors(orderedIds, String(active.id), String(over.id));
    if (!anchors) return;
    if ((sorts?.length ?? 0) > 0) onSortChange?.([]);
    void reorder.mutate({ entityId: String(active.id), ...anchors });
  };

  if (rows.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20 gap-3">
        <p className="text-[14px] font-medium text-foreground">No items yet</p>
        <p className="text-[13px] text-foreground/60">
          Click <span className="text-brand font-medium">+ New</span> to create your first entry
        </p>
      </div>
    );
  }

  return (
    <div ref={parentRef} className="relative h-full w-full overflow-auto">
      <TableHeaderRow table={table} sorts={sorts} onSortChange={onSortChange} />
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={orderedIds} strategy={verticalListSortingStrategy}>
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {virtualizer.getVirtualItems().map((vRow) => {
              const row = rows[vRow.index];
              return (
                <TableBodyRow
                  key={row.id}
                  row={row}
                  schema={schema}
                  top={vRow.start}
                  onClick={() => onEntityClick(row.original)}
                />
              );
            })}
          </div>
        </SortableContext>
      </DndContext>
    </div>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | grep "table/TableView"`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/database/components/views/table/TableView.tsx
git commit -m "feat(desktop-ui): TanStack-backed TableView with row virtualization"
```

---

## Task 6: Wire the new TableView into ViewShell and delete the old one

**Files:**
- Modify: `desktop-ui/src/features/database/components/ViewShell.tsx`
- Delete: `desktop-ui/src/features/database/components/views/TableView.tsx` (the old top-level file)

- [ ] **Step 1: Update the import in ViewShell**

Change:

```ts
import { TableView } from "./views/TableView";
```

to:

```ts
import { TableView } from "./views/table/TableView";
```

- [ ] **Step 2: Pass `view` to TableView**

In ViewShell's `ActiveViewRenderer` switch, update the `case "table":` branch to:

```tsx
case "table":
  return (
    <TableView
      schema={schema}
      view={view}
      entities={entities}
      sorts={sorts}
      onSortChange={onSortChange}
      onEntityClick={onEntityClick}
    />
  );
```

Remove the now-unused `visibleFields={view.config.visibleFields}` prop (the new TableView reads it from `view.config` directly).

- [ ] **Step 3: Delete the old file**

```bash
rm desktop-ui/src/features/database/components/views/TableView.tsx
```

- [ ] **Step 4: Typecheck + lint**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | head`
Expected: no new errors.

Run: `cd desktop-ui && bun run lint 2>&1 | tail -10`
Expected: clean (or only pre-existing a11y warnings on unrelated files).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/database/components/ViewShell.tsx desktop-ui/src/features/database/components/views/TableView.tsx
git commit -m "refactor(desktop-ui): retire hand-rolled TableView, wire TanStack version"
```

---

## Task 7: Verify in the browser

**Files:** none (manual verification)

- [ ] **Step 1: Start the dev server**

Run in one terminal: `cd desktop-ui && bun run dev`
Run in another: `cargo tauri dev` (or open `http://localhost:1420/#/db/<some-db-id>` in Chrome with the dev HTTP server running)

- [ ] **Step 2: Verify sticky header**

Scroll the table. Expected: header stays pinned to the top of the scroll container.

- [ ] **Step 3: Verify column resize persists**

Grab the right edge of any column header, drag, release. Refresh the page. Expected: the new width persists (debounced save to `view.config.columnWidths`).

- [ ] **Step 4: Verify sort toggle still works**

Click a column header. Expected: asc → desc → none cycle, and an indicator arrow toggles. Rows reorder by backend sort.

- [ ] **Step 5: Verify row DnD still works**

Drag row 2 above row 1. Expected: row moves immediately (optimistic update), backend persists, sorted-view clears local sort to honor manual order.

- [ ] **Step 6: Verify virtualization with many rows**

Open Chrome DevTools console and seed 500 entities:

```js
for (let i = 0; i < 500; i++) {
  await fetch('/api/db_create_entity', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({ databaseId: '<YOUR_DB_ID>', fields: { title: `Seed ${i}` } }) });
}
```

Scroll the table. Expected: smooth 60fps scroll, DOM contains only ~20 row divs at any time (check with `document.querySelectorAll('[data-entity-id]').length`).

- [ ] **Step 7: Verify the empty state**

Filter the view so 0 rows match. Expected: "No items yet" panel renders, no header, no virtualizer empty-height div.

- [ ] **Step 8: Commit verification notes (optional)**

If you fixed anything during verification, commit the fix as a separate commit. Otherwise skip.

---

## Task 8: Clean up seeded test data (optional)

**Files:** none

- [ ] **Step 1: Delete the seed entities**

Either via the UI (select all, delete) or via a script:

```js
const r = await fetch('/api/db_query', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({ databaseId: '<YOUR_DB_ID>', limit: 1000 }) }).then(r => r.json());
for (const e of r.entities) {
  if (e.fields.title?.startsWith('Seed ')) {
    await fetch('/api/db_delete_entity', { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({ databaseId: '<YOUR_DB_ID>', entityId: e.id }) });
  }
}
```

- [ ] **Step 2: Verify**

Reload the page. Expected: no "Seed N" rows.

No commit — this is environment cleanup.
