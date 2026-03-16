import { useQuery } from "@shared/hooks/useQuery";

export interface Backlink {
  note: { id: string; title: string; tags: string[]; updatedAt: string };
  context: string | null;
}

export function useBacklinks(noteId: string | null) {
  return useQuery<Backlink[]>("note_backlinks", noteId ? { id: noteId } : null, []);
}
