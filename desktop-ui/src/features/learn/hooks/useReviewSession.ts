import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";
import type { Flashcard, ReviewQuality } from "../../notes/hooks/useFlashcards";

export interface GradeResult {
  score: number | null;
  suggestedRating: string;
  gradingMethod: string;
  explanation: string | null;
  diffHighlights: { text: string; status: string }[];
  expectedAnswer: string;
  coachingNudge: string | null;
  keyConceptsPresent: string[];
  keyConceptsMissing: string[];
}

export function useReviewSession() {
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [totalReviewed, setTotalReviewed] = useState(0);
  const [correctCount, setCorrectCount] = useState(0);
  const [socraticExplanation, setSocraticExplanation] = useState<string | null>(null);
  const [socraticLoading, setSocraticLoading] = useState(false);
  const [showSocratic, setShowSocratic] = useState(false);
  const [gradeResult, setGradeResult] = useState<GradeResult | null>(null);
  const [grading, setGrading] = useState(false);
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
    setGradeResult(null);
    setGrading(false);
    startTime.current = Date.now();
    cardShownAt.current = Date.now();
  }, []);

  const reveal = useCallback(() => setRevealed(true), []);

  const submitAnswer = useCallback(
    async (userAnswer: string) => {
      const card = cards[currentIndex];
      if (!card) return;
      setGrading(true);
      setGradeResult(null);
      try {
        const result = await ipc<GradeResult>("flashcard_submit_answer", {
          params: { cardId: card.id, userAnswer, mode: "typed" },
        });
        setGradeResult(result);
        setRevealed(true);
      } catch {
        setRevealed(true);
      } finally {
        setGrading(false);
      }
    },
    [cards, currentIndex],
  );

  const rate = useCallback(
    async (quality: ReviewQuality) => {
      const card = cards[currentIndex];
      if (!card) return;
      const recallSpeedMs = Date.now() - cardShownAt.current;
      await ipc("flashcard_record_review", {
        params: { cardId: card.id, quality, recallSpeedMs },
      });
      setTotalReviewed((n) => n + 1);
      if (quality !== "again") setCorrectCount((n) => n + 1);

      // Auto-trigger Socratic explanation on weak answers
      setSocraticExplanation(null);
      setShowSocratic(false);
      if (quality === "again" || quality === "hard") {
        setShowSocratic(true);
        setSocraticLoading(true);
        ipc<{ explanation: string }>("flashcard_explain_answer", {
          params: {
            cardId: card.id,
            userAnswer: `(self-rated as ${quality} after seeing the answer)`,
            gradeExplanation: `Student self-assessed as '${quality}' — they may not fully understand the concept.`,
          },
        })
          .then((r) => setSocraticExplanation(r.explanation))
          .catch(() => setShowSocratic(false))
          .finally(() => setSocraticLoading(false));
      }

      setRevealed(false);
      setGradeResult(null);
      setGrading(false);
      const nextIndex = currentIndex + 1;
      setCurrentIndex(nextIndex);
      cardShownAt.current = Date.now();
    },
    [cards, currentIndex],
  );

  const dismissSocratic = useCallback(() => {
    setShowSocratic(false);
    setSocraticExplanation(null);
  }, []);

  const updateCard = useCallback((index: number, updated: Partial<Flashcard>) => {
    setCards((prev) => prev.map((c, i) => (i === index ? { ...c, ...updated } : c)));
  }, []);

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
    submitAnswer,
    gradeResult,
    grading,
    socraticExplanation,
    socraticLoading,
    showSocratic,
    dismissSocratic,
    updateCard,
  };
}
