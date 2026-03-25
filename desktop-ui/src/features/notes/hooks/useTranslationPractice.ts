import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface TranslationEvalResponse {
  grades: {
    meaning: string;
    grammar: string;
    naturalness: string;
    wordChoice: string;
  };
  corrections: Array<{
    original: string;
    suggested: string;
    explanation: string;
    category: string;
  }>;
  modelTranslation: string;
}

export function useTranslationPractice() {
  const [evaluation, setEvaluation] = useState<TranslationEvalResponse | null>(null);
  const [evaluating, setEvaluating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const evaluate = useCallback(
    async (sourceText: string, userTranslation: string, sourceLang: string, targetLang: string) => {
      setEvaluating(true);
      setError(null);
      try {
        const response = await ipc<TranslationEvalResponse>("language_evaluate_translation", {
          params: { sourceText, userTranslation, sourceLang, targetLang },
        });
        setEvaluation(response);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : "Evaluation failed");
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

  return { evaluation, evaluating, error, evaluate, reset };
}
