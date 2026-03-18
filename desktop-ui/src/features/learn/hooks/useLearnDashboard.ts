import { useQuery } from "@shared/hooks/useQuery";
import { useMemo } from "react";
import type { DeckSummary } from "../../notes/hooks/useFlashcards";

export function useLearnDashboard() {
  const { data: decks, loading, refetch } = useQuery<DeckSummary[]>("flashcard_list_decks", {}, []);

  const totalDue = useMemo(() => (decks ?? []).reduce((sum, d) => sum + d.dueCount, 0), [decks]);

  return { decks: decks ?? [], totalDue, loading, refetch };
}
