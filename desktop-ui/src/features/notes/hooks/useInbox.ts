import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { InboxItem } from "@shared/types";

export function useInbox() {
  const { data: items, refetch } = useQuery<InboxItem[]>("inbox_list", undefined, []);
  const { mutate: createItem } = useMutation<InboxItem, { content: string }>(
    "inbox_create",
    "params",
  );
  const { mutate: deleteItem } = useMutation<void, { id: string }>("inbox_delete");

  // Refetch when inbox entity events fire (from useMutation or Tauri backend)
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "inbox") refetch();
  });

  return { items, refetch, createItem, deleteItem };
}
