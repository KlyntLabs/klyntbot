import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface DeckSummary {
  name: string;
  cardCount: number;
  dueCount: number;
}

export interface Flashcard {
  id: string;
  deck: string;
  question: string;
  answer: string;
  cardType: string;
  choices: { label: string; text: string }[] | null;
  stability: number;
  difficulty: number;
  dueAt: string | null;
  state: string;
  reviewCount: number;
  createdAt: string;
}

type ReviewQuality = "again" | "hard" | "good" | "easy";

export function useFlashcards() {
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);

  const fetchDecks = useCallback(async () => {
    const result = await ipc<DeckSummary[]>("flashcard_list_decks", {});
    setDecks(result);
  }, []);

  const startReview = useCallback(async (deck: string) => {
    const due = await ipc<Flashcard[]>("flashcard_get_due", { deck, limit: 20 });
    setCards(due);
    setCurrentIndex(0);
    setRevealed(false);
  }, []);

  const reveal = useCallback(() => setRevealed(true), []);

  const review = useCallback(
    async (quality: ReviewQuality) => {
      const card = cards[currentIndex];
      if (!card) return;
      await ipc("flashcard_record_review", { cardId: card.id, quality });
      setRevealed(false);
      setCurrentIndex((i) => i + 1);
    },
    [cards, currentIndex],
  );

  const current = cards[currentIndex] ?? null;
  const remaining = Math.max(0, cards.length - currentIndex);
  const done = currentIndex >= cards.length && cards.length > 0;

  return {
    decks,
    cards,
    current,
    remaining,
    done,
    revealed,
    fetchDecks,
    startReview,
    reveal,
    review,
  };
}
