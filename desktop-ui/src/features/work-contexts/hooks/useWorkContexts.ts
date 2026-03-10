import { useMutation } from "@shared/hooks/useMutation";
import { invalidateQueries, useQuery } from "@shared/hooks/useQuery";
import type { WorkContext, WorkContextDetail, WorkContextUpdateParams } from "@shared/types";

export function useWorkContexts(status?: string) {
  return useQuery<WorkContext[]>("list_work_contexts", status ? { status } : undefined, []);
}

export function useWorkContext(id: string | null) {
  return useQuery<WorkContext | null>("get_work_context", id ? { id } : null, null);
}

export function useWorkContextDetail(id: string | null) {
  return useQuery<WorkContextDetail | null>("get_work_context_detail", id ? { id } : null, null);
}

export function useSearchWorkContexts(query: string | null) {
  return useQuery<WorkContext[]>("search_work_contexts", query ? { query } : null, []);
}

export function useContextMutations() {
  const update = useMutation<WorkContext, WorkContextUpdateParams>("update_work_context", "params");
  const archive = useMutation<WorkContext, { id: string }>("archive_work_context");
  const merge = useMutation<WorkContext, { keepId: string; removeId: string }>(
    "merge_work_contexts",
  );

  return { update, archive, merge, invalidate: () => invalidateQueries("list_work_contexts") };
}
