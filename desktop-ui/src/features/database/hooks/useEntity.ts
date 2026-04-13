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
      const result = await rawMutate({ databaseId, fields: input.fields });
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
      const result = await rawMutate({ databaseId, entityId, fields: input.fields });
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
      await rawMutate({ databaseId, entityId });
      emitDatabaseUpdated();
    },
    [databaseId, rawMutate],
  );

  return { mutate, loading, error };
}
