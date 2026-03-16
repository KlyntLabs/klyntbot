import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";

export interface Backlink {
  note: { id: string; title: string; tags: string[]; updatedAt: string };
  context: string | null;
}

export function useBacklinks(noteId: string | null) {
  const result = useQuery<Backlink[]>("note_backlinks", noteId ? { id: noteId } : null, []);

  // Refetch backlinks when any note is mutated (links may have changed)
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "note" && noteId) result.refetch();
  });

  return result;
}
