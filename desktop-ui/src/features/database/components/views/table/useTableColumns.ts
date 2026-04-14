import { useUpdateView } from "@features/database/hooks/useViews";
import {
  defaultColumnWidth,
  MAX_COLUMN_WIDTH,
  MIN_COLUMN_WIDTH,
} from "@features/database/lib/columnDefaults";
import { getTitleField } from "@features/database/lib/schema-utils";
import type { DatabaseSchema, Entity, ViewDefinition } from "@shared/types";
import {
  type ColumnDef,
  type ColumnSizingState,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

interface Args {
  schema: DatabaseSchema;
  entities: Entity[];
  view: ViewDefinition;
}

/** Persist column-width changes back to `view.config` (debounced). */
function useDebouncedPersist(schemaId: string, viewId: string) {
  const { mutate: updateView } = useUpdateView(schemaId);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);
  return useCallback(
    (patch: Record<string, unknown>) => {
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        void updateView(viewId, { config: patch });
      }, 400);
    },
    [updateView, viewId],
  );
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
