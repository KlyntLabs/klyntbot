import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";

interface NoteResponse {
  id: string;
  title: string;
  tags: string[];
  updatedAt: string;
}

export function useUnlinkedMentions(noteId: string | null) {
  const result = useQuery<NoteResponse[]>(
    "note_unlinked_mentions",
    noteId ? { id: noteId } : null,
    [],
  );

  // Refetch when any note is mutated (body changes may create/remove mentions)
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    if (payload.entityKind === "note" && noteId) result.refetch();
  });

  return result;
}
