import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import { updateQueryCache } from "@shared/hooks/useQuery";
import type { Entity, QueryResult } from "@shared/types";
import { useCallback } from "react";

interface ReorderArgs {
  entityId: string;
  beforeId?: string;
  afterId?: string;
}

/** Optimistically reposition an entity in every cached db_query result. */
function optimisticReorder(databaseId: string, { entityId, beforeId, afterId }: ReorderArgs) {
  updateQueryCache<QueryResult>("db_query", (data) => {
    if (!data?.entities || data.entities[0]?.databaseId !== databaseId) return data;
    const entities = [...data.entities];
    const srcIdx = entities.findIndex((e) => e.id === entityId);
    if (srcIdx < 0) return data;
    const [src] = entities.splice(srcIdx, 1);
    let insertAt = entities.length;
    if (beforeId) {
      const i = entities.findIndex((e) => e.id === beforeId);
      if (i >= 0) insertAt = i + 1;
    } else if (afterId) {
      const i = entities.findIndex((e) => e.id === afterId);
      if (i >= 0) insertAt = i;
    }
    entities.splice(insertAt, 0, src);
    return { ...data, entities };
  });
}

export function useEntityReorder(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<Entity, Record<string, unknown>>("db_reorder_entity");

  const mutate = useCallback(
    async (args: ReorderArgs) => {
      optimisticReorder(databaseId, args);
      const result = await rawMutate({ databaseId, ...args });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
