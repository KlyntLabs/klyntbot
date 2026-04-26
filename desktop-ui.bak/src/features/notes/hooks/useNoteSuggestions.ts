import { useQuery } from "@shared/hooks/useQuery";
import type { Note } from "@shared/types";

export interface ScoredNote {
  note: Note;
  score: number;
  reason: string;
}

export interface LinkSuggestion {
  note: Note;
  score: number;
  reason: string;
}

export interface NoteSuggestions {
  relatedNotes: ScoredNote[];
  linkSuggestions: LinkSuggestion[];
  suggestedTags: string[];
}

const EMPTY: NoteSuggestions = {
  relatedNotes: [],
  linkSuggestions: [],
  suggestedTags: [],
};

export function useNoteSuggestions(noteId: string | null) {
  const { data, refetch } = useQuery<NoteSuggestions>(
    "note_suggestions",
    noteId ? { id: noteId } : null,
    EMPTY,
    {
      invalidateOn: ["entity:updated"],
      invalidateFilter: (p) => (p as { entityKind?: string })?.entityKind === "note",
    },
  );

  return {
    suggestions: data,
    refetchSuggestions: refetch,
  };
}
