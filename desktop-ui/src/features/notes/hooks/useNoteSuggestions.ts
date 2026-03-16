export interface NoteSuggestions {
  relatedNotes: string[];
  linkSuggestions: string[];
  suggestedTags: string[];
}

export function useNoteSuggestions(_noteId: string | null) {
  // Phase 1: return empty data — backend not ready yet
  return {
    suggestions: {
      relatedNotes: [] as string[],
      linkSuggestions: [] as string[],
      suggestedTags: [] as string[],
    } satisfies NoteSuggestions,
    refetchSuggestions: () => {},
  };
}
