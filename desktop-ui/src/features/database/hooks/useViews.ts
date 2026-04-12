import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import type { ViewConfig, ViewDefinition, ViewType } from "@shared/types";
import { useCallback } from "react";

export function useCreateView(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<ViewDefinition, Record<string, unknown>>("db_create_view");

  const mutate = useCallback(
    async (name: string, viewType: ViewType, config?: ViewConfig) => {
      const result = await rawMutate({
        database_id: databaseId,
        name,
        view_type: viewType,
        config: config ?? {},
      });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}

export function useUpdateView(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<ViewDefinition, Record<string, unknown>>("db_update_view");

  const mutate = useCallback(
    async (viewId: string, updates: Partial<ViewDefinition>) => {
      const result = await rawMutate({
        database_id: databaseId,
        view_id: viewId,
        ...updates,
      });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}

export function useDeleteView(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<void, Record<string, unknown>>("db_delete_view");

  const mutate = useCallback(
    async (viewId: string) => {
      await rawMutate({ database_id: databaseId, view_id: viewId });
      emitDatabaseUpdated();
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
