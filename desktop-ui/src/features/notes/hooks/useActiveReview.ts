import { ipc } from "@shared/hooks/useIpc";
import type { AnswerMode, GradeResult, SessionStats } from "@shared/types/notes";
import { useCallback, useRef, useState } from "react";
import type { DeckSummary, Flashcard, ReviewQuality } from "./useFlashcards";

// ── Phase types ─────────────────────────────────────────────────────────────

/** Top-level session phase */
export type SessionPhase = "idle" | "deck_picker" | "reviewing" | "complete";

/** Per-card review phase */
export type CardPhase = "answering" | "grading" | "graded" | "socratic" | "confirming";

// ── Hook state ───────────────────────────────────────────────────────────────

interface ActiveReviewState {
  phase: SessionPhase;
  cardPhase: CardPhase;
  decks: DeckSummary[];
  /** Cards still to review (including current) */
  queue: Flashcard[];
  currentIndex: number;
  gradeResult: GradeResult | null;
  lastAnswer: string;
  selectedMode: AnswerMode;
  selectedDeck: string | null;
  sessionId: string;
  error: string | null;
}

function makeSessionId(): string {
  return `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

function initialState(): ActiveReviewState {
  return {
    phase: "idle",
    cardPhase: "answering",
    decks: [],
    queue: [],
    currentIndex: 0,
    gradeResult: null,
    lastAnswer: "",
    selectedMode: "auto",
    selectedDeck: null,
    sessionId: makeSessionId(),
    error: null,
  };
}

// ── Hook ─────────────────────────────────────────────────────────────────────

export function useActiveReview() {
  const [state, setState] = useState<ActiveReviewState>(initialState);

  const statsRef = useRef<SessionStats>({
    cardsReviewed: 0,
    totalScore: 0,
    modeUsage: {},
    weakCards: [],
    propagationCount: 0,
    startTime: Date.now(),
  });

  // ── Derived values ──────────────────────────────────────────────────────

  const current = state.queue[state.currentIndex] ?? null;
  const remaining = Math.max(0, state.queue.length - state.currentIndex);

  const avgScore =
    statsRef.current.cardsReviewed > 0
      ? statsRef.current.totalScore / statsRef.current.cardsReviewed
      : null;

  // ── Helpers ─────────────────────────────────────────────────────────────

  function patchState(patch: Partial<ActiveReviewState>) {
    setState((prev) => ({ ...prev, ...patch }));
  }

  function recordModeScore(mode: string, score: number | null) {
    const stats = statsRef.current;
    stats.cardsReviewed += 1;
    const numScore = score ?? 0;
    stats.totalScore += numScore;

    const bucket = stats.modeUsage[mode] ?? { count: 0, totalScore: 0 };
    bucket.count += 1;
    bucket.totalScore += numScore;
    stats.modeUsage[mode] = bucket;
  }

  const saveSession = useCallback(
    async (status: "completed" | "abandoned") => {
      const s = statsRef.current;
      try {
        await ipc("flashcard_save_session", {
          sessionId: state.sessionId,
          cardsReviewed: s.cardsReviewed,
          avgScore: s.cardsReviewed > 0 ? s.totalScore / s.cardsReviewed : 0,
          durationSeconds: Math.round((Date.now() - s.startTime) / 1000),
          modesUsed: Object.keys(s.modeUsage),
          propagationCount: s.propagationCount,
          weakCardIds: s.weakCards.map((w) => w.id),
          sessionData: JSON.stringify(s),
          status,
        });
      } catch {
        // best-effort
      }
    },
    [state.sessionId],
  );

  function advanceQueue() {
    setState((prev) => {
      const nextIndex = prev.currentIndex + 1;
      if (nextIndex >= prev.queue.length) {
        // Fire-and-forget session save
        saveSession("completed");
        return {
          ...prev,
          phase: "complete",
          cardPhase: "answering",
          currentIndex: nextIndex,
          lastAnswer: "",
        };
      }
      return {
        ...prev,
        currentIndex: nextIndex,
        cardPhase: "answering",
        gradeResult: null,
        lastAnswer: "",
      };
    });
  }

  // ── Actions ──────────────────────────────────────────────────────────────

  const fetchDecks = useCallback(async () => {
    try {
      const result = await ipc<DeckSummary[]>("flashcard_list_decks", {});
      patchState({ decks: result, phase: "deck_picker", error: null });
    } catch (e: unknown) {
      patchState({ error: e instanceof Error ? e.message : "Failed to load decks" });
    }
  }, []);

  const startReview = useCallback(async (deck: string) => {
    try {
      // Load saved mode preference for this deck
      let savedMode: AnswerMode = "auto";
      try {
        const pref = await ipc<{ mode: AnswerMode } | null>("flashcard_get_mode_preference", {
          deck,
        });
        if (pref?.mode) savedMode = pref.mode;
      } catch {
        // preference is optional — ignore errors
      }

      const due = await ipc<Flashcard[]>("flashcard_get_due", { deck, limit: 20 });

      // Reset stats for new session
      statsRef.current = {
        cardsReviewed: 0,
        totalScore: 0,
        modeUsage: {},
        weakCards: [],
        propagationCount: 0,
        startTime: Date.now(),
      };

      patchState({
        queue: due,
        currentIndex: 0,
        gradeResult: null,
        cardPhase: "answering",
        selectedMode: savedMode,
        selectedDeck: deck,
        sessionId: makeSessionId(),
        phase: due.length > 0 ? "reviewing" : "complete",
        error: null,
      });
    } catch (e: unknown) {
      patchState({ error: e instanceof Error ? e.message : "Failed to start review" });
    }
  }, []);

  const submitAnswer = useCallback(
    async (text: string) => {
      if (!current) return;
      patchState({ cardPhase: "grading", error: null });
      try {
        const result = await ipc<GradeResult>("flashcard_submit_answer", {
          cardId: current.id,
          userAnswer: text,
          mode: state.selectedMode,
        });

        recordModeScore(state.selectedMode, result.score);

        // Track weak cards (score < 0.6)
        if ((result.score ?? 1) < 0.6) {
          statsRef.current.weakCards.push({
            id: current.id,
            front: current.front,
            score: result.score ?? 0,
          });
        }

        patchState({ gradeResult: result, cardPhase: "graded", lastAnswer: text });
      } catch (e: unknown) {
        patchState({
          cardPhase: "answering",
          error: e instanceof Error ? e.message : "Failed to grade answer",
        });
      }
    },
    [current, state.selectedMode],
  );

  const confirmRating = useCallback(
    async (quality?: ReviewQuality) => {
      if (!current) return;

      // Determine quality from gradeResult if not explicitly provided
      const effectiveQuality: ReviewQuality =
        quality ?? ratingFromSuggestion(state.gradeResult?.suggestedRating);

      patchState({ cardPhase: "confirming", error: null });
      try {
        await ipc("flashcard_record_review", {
          cardId: current.id,
          quality: effectiveQuality,
          recallSpeedMs: null,
        });
        advanceQueue();
      } catch (e: unknown) {
        patchState({
          cardPhase: "graded",
          error: e instanceof Error ? e.message : "Failed to record review",
        });
      }
    },
    [current, state.gradeResult],
  );

  const requestExplanation = useCallback(() => {
    patchState({ cardPhase: "socratic" });
  }, []);

  const switchMode = useCallback(
    (mode: AnswerMode) => {
      patchState({ selectedMode: mode });
      // Persist preference for this deck
      if (state.selectedDeck) {
        ipc("flashcard_save_mode_preference", { deck: state.selectedDeck, mode }).catch(() => {});
      }
    },
    [state.selectedDeck],
  );

  const skipCard = useCallback(async () => {
    if (!current) return;
    try {
      await ipc("flashcard_record_review", {
        cardId: current.id,
        quality: "again" satisfies ReviewQuality,
        recallSpeedMs: null,
      });
    } catch {
      // best-effort — advance regardless
    }
    advanceQueue();
  }, [current]);

  return {
    // State
    phase: state.phase,
    cardPhase: state.cardPhase,
    decks: state.decks,
    current,
    remaining,
    gradeResult: state.gradeResult,
    lastAnswer: state.lastAnswer,
    selectedMode: state.selectedMode,
    selectedDeck: state.selectedDeck,
    sessionId: state.sessionId,
    avgScore,
    stats: statsRef,
    error: state.error,

    // Actions
    fetchDecks,
    startReview,
    submitAnswer,
    confirmRating,
    requestExplanation,
    switchMode,
    skipCard,
    saveSession,
  };
}

// ── Utilities ────────────────────────────────────────────────────────────────

function ratingFromSuggestion(suggested: string | undefined): ReviewQuality {
  switch (suggested?.toLowerCase()) {
    case "again":
      return "again";
    case "hard":
      return "hard";
    case "easy":
      return "easy";
    default:
      return "good";
  }
}
