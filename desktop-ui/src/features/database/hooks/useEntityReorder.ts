import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import type { Entity } from "@shared/types";
import { useCallback } from "react";

interface ReorderArgs {
  entityId: string;
  beforeId?: string;
  afterId?: string;
}

export function useEntityReorder(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<Entity, Record<string, unknown>>("db_reorder_entity");

  const mutate = useCallback(
    async ({ entityId, beforeId, afterId }: ReorderArgs) => {
      const result = await rawMutate({ databaseId, entityId, beforeId, afterId });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
