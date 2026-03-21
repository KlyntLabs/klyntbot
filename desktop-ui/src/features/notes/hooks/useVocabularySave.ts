import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useState } from "react";
import type { WordBreakdown } from "./useLanguageBreakdown";

type SaveState = "idle" | "saving" | "saved" | "error";

export function useVocabularySave() {
  const [state, setState] = useState<SaveState>("idle");
  const [savedCount, setSavedCount] = useState<number | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

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
        invalidateQueries("atoms_for_note");
        setTimeout(() => {
          setState("idle");
          setSavedCount(null);
        }, 5000);
      } catch (e: unknown) {
        setState("error");
        setErrorMessage(e instanceof Error ? e.message : "Failed to save words");
        setTimeout(() => {
          setState("idle");
          setErrorMessage(null);
        }, 5000);
      }
    },
    [],
  );

  const dismissSaved = useCallback(() => {
    setSavedCount(null);
    setState("idle");
  }, []);

  const dismissError = useCallback(() => {
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
