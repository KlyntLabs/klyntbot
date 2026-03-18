import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";
import type { Flashcard, ReviewQuality } from "../../notes/hooks/useFlashcards";

export function useReviewSession() {
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [totalReviewed, setTotalReviewed] = useState(0);
  const [correctCount, setCorrectCount] = useState(0);
  const startTime = useRef(Date.now());
  const cardShownAt = useRef(Date.now());

  const startReview = useCallback(async (deck?: string) => {
    const due = deck
      ? await ipc<Flashcard[]>("flashcard_get_due", { deck, limit: 50 })
      : await ipc<Flashcard[]>("flashcard_get_all_due", { limit: 50 });
    setCards(due);
    setCurrentIndex(0);
    setRevealed(false);
    setTotalReviewed(0);
    setCorrectCount(0);
    startTime.current = Date.now();
    cardShownAt.current = Date.now();
  }, []);

  const reveal = useCallback(() => setRevealed(true), []);

  const rate = useCallback(
    async (quality: ReviewQuality) => {
      const card = cards[currentIndex];
      if (!card) return;
      const recallSpeedMs = Date.now() - cardShownAt.current;
      await ipc("flashcard_record_review", {
        cardId: card.id,
        quality,
        recallSpeedMs,
      });
      setTotalReviewed((n) => n + 1);
      if (quality !== "again") setCorrectCount((n) => n + 1);
      setRevealed(false);
      const nextIndex = currentIndex + 1;
      setCurrentIndex(nextIndex);
      cardShownAt.current = Date.now();
    },
    [cards, currentIndex],
  );

  const current = cards[currentIndex] ?? null;
  const remaining = Math.max(0, cards.length - currentIndex);
  const done = currentIndex >= cards.length && cards.length > 0;
  const elapsedSeconds = Math.round((Date.now() - startTime.current) / 1000);

  return {
    cards,
    current,
    currentIndex,
    revealed,
    done,
    remaining,
    totalReviewed,
    correctCount,
    elapsedSeconds,
    startReview,
    reveal,
    rate,
  };
}
