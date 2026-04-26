import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { InboxItem } from "@shared/types";

export function useInbox() {
  const { data: items, refetch } = useQuery<InboxItem[]>("inbox_list", undefined, [], {
    invalidateOn: ["entity:updated"],
    invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "inbox",
  });
  const { mutate: createItem } = useMutation<InboxItem, { content: string }>(
    "inbox_create",
    "params",
  );
  const { mutate: deleteItem } = useMutation<void, { id: string }>("inbox_delete");

  return { items, refetch, createItem, deleteItem };
}
