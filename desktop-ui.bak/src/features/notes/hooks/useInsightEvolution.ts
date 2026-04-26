import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface EvolutionPoint {
  version: number;
  generatedAt: string;
  flashcardSuccess: number;
  semanticDrift: number;
  gapClosure: number;
  quizScore: number;
  overallProgress: number;
  changeNote: string;
}

interface EvolutionData {
  noteId: string;
  noteTitle: string;
  versions: EvolutionPoint[];
}

interface EvolutionState {
  loading: boolean;
  data: EvolutionData | null;
  error: string | null;
}

export function useInsightEvolution() {
  const [state, setState] = useState<EvolutionState>({
    loading: false,
    data: null,
    error: null,
  });

  const fetch = useCallback(async (noteId: string) => {
    setState({ loading: true, data: null, error: null });
    try {
      const data = await ipc<EvolutionData>("note_insight_get_evolution", {
        noteId,
      });
      setState({ loading: false, data, error: null });
    } catch (e) {
      setState({ loading: false, data: null, error: String(e) });
    }
  }, []);

  const clear = useCallback(() => {
    setState({ loading: false, data: null, error: null });
  }, []);

  return { ...state, fetch, clear };
}
