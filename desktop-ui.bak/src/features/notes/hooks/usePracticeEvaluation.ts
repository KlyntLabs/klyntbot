import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface PracticeScores {
  meaning: string;
  grammar: string;
  naturalness: string;
  wordChoice: string;
}

export interface PracticeCorrection {
  original: string;
  suggested: string;
  explanation: string;
}

export interface PracticeEvalResponse {
  overallGrade: string;
  scores: PracticeScores;
  corrections: PracticeCorrection[];
  modelTranslation: string;
  encouragement: string;
  improvementHint: string | null;
  coachingNudge: string | null;
}

export function usePracticeEvaluation() {
  const [evaluation, setEvaluation] = useState<PracticeEvalResponse | null>(null);
  const [evaluating, setEvaluating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submitUnit = useCallback(
    async (sessionId: string, index: number, userTranslation: string) => {
      setEvaluating(true);
      setError(null);
      try {
        const response = await ipc<PracticeEvalResponse>("practice_submit_unit", {
          params: { sessionId, index, userTranslation },
        });
        setEvaluation(response);
        return response;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Evaluation failed";
        setError(msg);
        return null;
      } finally {
        setEvaluating(false);
      }
    },
    [],
  );

  const reset = useCallback(() => {
    setEvaluation(null);
    setError(null);
  }, []);

  return { evaluation, evaluating, error, submitUnit, reset };
}
