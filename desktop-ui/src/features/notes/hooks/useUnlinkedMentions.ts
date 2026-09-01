import { useQuery } from "@shared/hooks/useQuery";

interface NoteResponse {
  id: string;
  title: string;
  tags: string[];
  updatedAt: string;
}

export function useUnlinkedMentions(noteId: string | null) {
  return useQuery<NoteResponse[]>("note_unlinked_mentions", noteId ? { id: noteId } : null, [], {
    invalidateOn: ["entity:updated"],
    invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "note",
  });
}
