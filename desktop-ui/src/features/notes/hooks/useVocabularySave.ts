import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useEffect, useRef, useState } from "react";
import type { WordBreakdown } from "./useLanguageBreakdown";

type SaveState = "idle" | "saving" | "saved" | "error";

export function useVocabularySave() {
  const [state, setState] = useState<SaveState>("idle");
  const [savedCount, setSavedCount] = useState<number | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clean up timer on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const resetAfterDelay = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      setState("idle");
      setSavedCount(null);
      setErrorMessage(null);
      timerRef.current = null;
    }, 5000);
  }, []);

  const saveWords = useCallback(
    async (words: WordBreakdown[], noteId: string | null, deck: string) => {
      setState("saving");
      setErrorMessage(null);
      try {
        const vocabItems = words.map((w) => ({
          word: w.word,
          reading: w.reading,
          meaning: w.meaning,
          partOfSpeech: w.partOfSpeech,
          exampleSentence: w.exampleSentence,
        }));
        await ipc("language_save_vocabulary", {
          params: { words: vocabItems, noteId, deck },
        });
        setState("saved");
        setSavedCount(words.length);
        invalidateQueries("flashcard_");
        resetAfterDelay();
      } catch (e: unknown) {
        setState("error");
        setErrorMessage(e instanceof Error ? e.message : "Failed to save words");
        resetAfterDelay();
      }
    },
    [resetAfterDelay],
  );

  const dismissSaved = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setSavedCount(null);
    setState("idle");
  }, []);

  const dismissError = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setErrorMessage(null);
    setState("idle");
  }, []);

  return {
    saving: state === "saving",
    saved: state === "saved",
    savedCount,
    errorMessage,
    saveWords,
    dismissSaved,
    dismissError,
  };
}
