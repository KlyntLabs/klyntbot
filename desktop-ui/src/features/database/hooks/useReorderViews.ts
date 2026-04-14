import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import type { ViewDefinition } from "@shared/types";
import { useCallback } from "react";

export function useReorderViews(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<ViewDefinition[], Record<string, unknown>>("db_reorder_views");

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
