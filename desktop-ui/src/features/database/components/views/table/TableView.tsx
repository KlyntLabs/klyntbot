import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { useCollapsedGroups } from "@features/database/hooks/useCollapsedGroups";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { groupEntities } from "@features/database/lib/grouping";
import { computeReorderAnchors } from "@features/database/lib/ordering";
import type { DatabaseSchema, Entity, SortRule, ViewDefinition } from "@shared/types";
import type { Row } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef } from "react";
import { GroupHeader } from "../GroupHeader";
import { TableBodyRow } from "./TableBodyRow";
import { TableHeaderRow } from "./TableHeaderRow";
import { useTableColumns } from "./useTableColumns";

const ROW_HEIGHT = 36;
const HEADER_HEIGHT = 32;

type FlatItem =
  | { type: "header"; key: string; label: string; count: number; collapsed: boolean }
  | { type: "row"; row: Row<Entity> };

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

  const groupBy = view.config.groupBy;
  const grouped = Boolean(groupBy);
  const buckets = useMemo(
    () => groupEntities(entities, schema, groupBy),
    [entities, schema, groupBy],
  );
  const { collapsed, toggle } = useCollapsedGroups(schema.id, view);

  const rowById = useMemo(() => new Map(rows.map((r) => [r.original.id, r])), [rows]);

  const items = useMemo<FlatItem[]>(() => {
    if (!grouped) return rows.map((row) => ({ type: "row" as const, row }));
    const out: FlatItem[] = [];
    for (const g of buckets) {
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
  }, [grouped, buckets, rows, rowById, collapsed]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (index) => (items[index]?.type === "header" ? HEADER_HEIGHT : ROW_HEIGHT),
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
            {virtualizer.getVirtualItems().map((vi) => {
              const item = items[vi.index];
              if (!item) return null;
              if (item.type === "header") {
                return (
                  <div
                    key={`h:${item.key}`}
                    className="absolute left-0 top-0 w-full"
                    style={{
                      transform: `translateY(${vi.start}px)`,
                      height: HEADER_HEIGHT,
                    }}
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
          </div>
        </SortableContext>
      </DndContext>
    </div>
  );
}
