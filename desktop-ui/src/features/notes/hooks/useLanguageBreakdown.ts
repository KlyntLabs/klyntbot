import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";

export interface WordBreakdown {
  word: string;
  reading: string | null;
  meaning: string;
  partOfSpeech: string;
  proficiencyLevel: string | null;
  exampleSentence: string | null;
  isNew: boolean;
}

export interface GrammarPattern {
  pattern: string;
  explanation: string;
  patternType: string | null;
}

export interface TranslateBreakdownResponse {
  translation: string;
  words: WordBreakdown[];
  grammarPatterns: GrammarPattern[];
}

export function useLanguageBreakdown() {
  const [result, setResult] = useState<TranslateBreakdownResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestIdRef = useRef(0);

  const translate = useCallback(async (text: string, sourceLang: string, targetLang: string) => {
    const id = ++requestIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const response = await ipc<TranslateBreakdownResponse>("language_translate_breakdown", {
        params: { text, sourceLang, targetLang },
      });
      // Only apply result if this is still the latest request
      if (id === requestIdRef.current) {
        setResult(response);
      }
    } catch (e: unknown) {
      if (id === requestIdRef.current) {
        const msg = e instanceof Error ? e.message : "Translation failed";
        setError(msg);
      }
    } finally {
      if (id === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, []);

  const reset = useCallback(() => {
    requestIdRef.current++;
    setResult(null);
    setError(null);
  }, []);

  return { result, loading, error, translate, reset };
}
