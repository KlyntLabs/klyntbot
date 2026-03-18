import { ipc } from "@shared/hooks/useIpc";
import { invalidateQueries } from "@shared/hooks/useQuery";
import { useCallback, useState } from "react";

export interface GeneratedCardPreview {
  front: string;
  back: string;
  cardType: string;
  tags: string[];
  sourceContext: string | null;
  clozeData: unknown | null;
  vocabData: unknown | null;
}

interface GenerateResponse {
  cards: GeneratedCardPreview[];
  deckSuggestion: string;
}

interface UseCardGenerationReturn {
  generating: boolean;
  previews: GeneratedCardPreview[];
  deckSuggestion: string;
  error: string | null;
  generateFromNote: (noteId: string, deckHint?: string) => Promise<void>;
  generateFromText: (text: string, deckHint?: string) => Promise<void>;
  toggleCard: (index: number) => void;
  editCard: (index: number, field: "front" | "back", value: string) => void;
  approved: Set<number>;
  saveApproved: (noteId: string | null, deck: string) => Promise<void>;
  saving: boolean;
  reset: () => void;
}

export function useCardGeneration(): UseCardGenerationReturn {
  const [generating, setGenerating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [previews, setPreviews] = useState<GeneratedCardPreview[]>([]);
  const [deckSuggestion, setDeckSuggestion] = useState("");
  const [approved, setApproved] = useState<Set<number>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const generate = useCallback(async (noteId?: string, textContent?: string, deckHint?: string) => {
    setGenerating(true);
    setError(null);
    setPreviews([]);
    setApproved(new Set());

    try {
      const response = await ipc<GenerateResponse>("flashcard_generate", {
        noteId: noteId ?? null,
        textContent: textContent ?? null,
        deckHint: deckHint ?? null,
      });
      setPreviews(response.cards);
      setDeckSuggestion(response.deckSuggestion);
      setApproved(new Set(response.cards.map((_, i) => i)));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }, []);

  const generateFromNote = useCallback(
    (noteId: string, deckHint?: string) => generate(noteId, undefined, deckHint),
    [generate],
  );

  const generateFromText = useCallback(
    (text: string, deckHint?: string) => generate(undefined, text, deckHint),
    [generate],
  );

  const toggleCard = useCallback((index: number) => {
    setApproved((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  }, []);

  const editCard = useCallback((index: number, field: "front" | "back", value: string) => {
    setPreviews((prev) =>
      prev.map((card, i) => (i === index ? { ...card, [field]: value } : card)),
    );
  }, []);

  const saveApproved = useCallback(
    async (noteId: string | null, deck: string) => {
      const approvedCards = previews.filter((_, i) => approved.has(i));
      if (approvedCards.length === 0) return;

      setSaving(true);
      try {
        await ipc("flashcard_save_generated", {
          noteId,
          deck,
          cards: approvedCards,
        });
        invalidateQueries("flashcard_");
        setPreviews([]);
        setApproved(new Set());
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setSaving(false);
      }
    },
    [previews, approved],
  );

  const reset = useCallback(() => {
    setPreviews([]);
    setApproved(new Set());
    setError(null);
    setDeckSuggestion("");
    setGenerating(false);
    setSaving(false);
  }, []);

  return {
    generating,
    previews,
    deckSuggestion,
    error,
    generateFromNote,
    generateFromText,
    toggleCard,
    editCard,
    approved,
    saveApproved,
    saving,
    reset,
  };
}
