import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { useEntityReorder } from "@features/database/hooks/useEntityReorder";
import { computeReorderAnchors } from "@features/database/lib/ordering";
import type { DatabaseSchema, Entity, SortRule, ViewDefinition } from "@shared/types";
import { useVirtualizer } from "@tanstack/react-virtual";
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
