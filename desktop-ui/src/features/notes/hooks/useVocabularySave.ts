import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useState } from "react";
import type { WordBreakdown } from "./useLanguageBreakdown";

export function useVocabularySave() {
  const [saving, setSaving] = useState(false);
  const [savedCount, setSavedCount] = useState<number | null>(null);

  const saveWords = useCallback(
    async (words: WordBreakdown[], noteId: string | null, deck: string) => {
      setSaving(true);
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
        setSavedCount(words.length);
        invalidateQueries("flashcard_");
        setTimeout(() => setSavedCount(null), 5000);
      } catch {
        // Silently fail — vocab save is non-critical
      } finally {
        setSaving(false);
      }
    },
    [],
  );

  const dismissSaved = useCallback(() => setSavedCount(null), []);

  return { saving, savedCount, saveWords, dismissSaved };
}
