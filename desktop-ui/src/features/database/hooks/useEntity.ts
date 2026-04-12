import { emitDatabaseUpdated } from "@features/database/lib/schema-utils";
import { useMutation } from "@shared/hooks/useMutation";
import type { CreateEntityInput, Entity, UpdateEntityInput } from "@shared/types";
import { useCallback } from "react";

export function useCreateEntity(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<Entity, Record<string, unknown>>("db_create_entity");

  const mutate = useCallback(
    async (input: CreateEntityInput) => {
      const result = await rawMutate({ database_id: databaseId, input });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}

export function useUpdateEntity(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<Entity, Record<string, unknown>>("db_update_entity");

  const mutate = useCallback(
    async (entityId: string, input: UpdateEntityInput) => {
      const result = await rawMutate({ database_id: databaseId, entity_id: entityId, input });
      if (result) emitDatabaseUpdated();
      return result;
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}

export function useDeleteEntity(databaseId: string) {
  const {
    mutate: rawMutate,
    loading,
    error,
  } = useMutation<void, Record<string, unknown>>("db_delete_entity");

  const mutate = useCallback(
    async (entityId: string) => {
      await rawMutate({ database_id: databaseId, entity_id: entityId });
      emitDatabaseUpdated();
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
