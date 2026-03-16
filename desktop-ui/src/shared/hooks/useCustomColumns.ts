import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type {
  ColumnCreateParams,
  ColumnReorderParams,
  ColumnUpdateParams,
  ColumnValueSetParams,
  CustomColumn,
  CustomColumnValue,
} from "@shared/types";

export function useCustomColumns(projectId: string | null) {
  return useQuery<CustomColumn[]>("custom_column_list", projectId ? { projectId } : null, []);
}

export function useColumnValues(taskId: string | null) {
  return useQuery<CustomColumnValue[]>("custom_column_values", taskId ? { taskId } : null, []);
}

export function useColumnMutations() {
  const create = useMutation<CustomColumn, ColumnCreateParams>("custom_column_create", "params");
  const update = useMutation<CustomColumn, ColumnUpdateParams>("custom_column_update", "params");
  const remove = useMutation<boolean, { id: string }>("custom_column_delete");
  const reorder = useMutation<void, ColumnReorderParams>("custom_column_reorder", "params");
  const setValue = useMutation<void, ColumnValueSetParams>("custom_column_value_set", "params");
  const deleteValue = useMutation<boolean, { taskId: string; columnId: string }>(
    "custom_column_value_delete",
  );

  return { create, update, remove, reorder, setValue, deleteValue };
}
