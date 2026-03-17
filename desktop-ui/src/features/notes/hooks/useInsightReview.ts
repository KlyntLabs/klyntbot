import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useRef, useState } from "react";

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export type TabId = "synthesis" | "gaps" | "assessment" | "concept-map" | "perspectives";
export type TabStatus = "idle" | "streaming" | "loading" | "done" | "error";

export interface QuizQuestion {
  id: string;
  type: string;
  question: string;
  choices: string[] | null;
  correctAnswer: string;
  explanation: string;
  sourceNotes: string[];
  difficulty: string;
  difficultyScore: number;
}

export interface PersonaMeta {
  id: string;
  name: string;
  role: string;
  icon: string;
  tone: string;
}

export interface InsightReviewState {
  isOpen: boolean;
  noteId: string | null;
  insightReviewId: string | null;
  contentHash: string | null;
  activeTab: TabId;
  tabs: {
    synthesis: { status: TabStatus; content: string };
    gaps: { status: TabStatus; content: string };
    assessment: { status: TabStatus; questions: QuizQuestion[] };
    conceptMap: { status: TabStatus; mermaid: string; fallbackText: string };
    perspectives: { status: TabStatus; content: string; personas: PersonaMeta[] };
  };
  quizState: {
    answers: Record<string, string>;
    revealed: Set<string>;
    score: number;
    total: number;
  };
  changesSummary: string | null;
}

export interface InsightReviewActions {
  open: (noteId: string) => Promise<void>;
  applyCachedContent: (
    cached: InsightReviewCachedResponse,
    extraState?: Partial<InsightReviewState>,
  ) => void;
  close: () => void;
  switchTab: (tab: TabId) => void;
  regenerateTab: (tab: TabId) => Promise<void>;
  saveFlashcards: (deckName: string) => Promise<void>;
  answerQuestion: (questionId: string, answer: string) => void;
  revealAnswer: (questionId: string) => void;
  revealAll: () => void;
}

// ---------------------------------------------------------------------------
// Local IPC response types
// ---------------------------------------------------------------------------

export interface InsightReviewCachedResponse {
  insightReviewId: string;
  noteId: string;
  synthesis: string | null;
  gapAnalysis: string | null;
  selfAssessment: QuizQuestion[] | null;
  conceptMap: string | null;
  perspectives: string | null;
  personaIds: string[] | null;
}

interface TabContentResponse {
  tab: string;
  content: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function parseConceptMap(raw: string | null): {
  mermaid: string;
  fallbackText: string;
} {
  if (!raw) return { mermaid: "", fallbackText: "" };
  if (raw.startsWith("FALLBACK:"))
    return { mermaid: "", fallbackText: raw.slice("FALLBACK:".length) };
  return { mermaid: raw, fallbackText: "" };
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

const INITIAL_STATE: InsightReviewState = {
  isOpen: false,
  noteId: null,
  insightReviewId: null,
  contentHash: null,
  activeTab: "synthesis",
  tabs: {
    synthesis: { status: "idle", content: "" },
    gaps: { status: "idle", content: "" },
    assessment: { status: "idle", questions: [] },
    conceptMap: { status: "idle", mermaid: "", fallbackText: "" },
    perspectives: { status: "idle", content: "", personas: [] },
  },
  quizState: {
    answers: {},
    revealed: new Set(),
    score: 0,
    total: 0,
  },
  changesSummary: null,
};

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useInsightReview(): [InsightReviewState, InsightReviewActions] {
  const [state, setState] = useState<InsightReviewState>(INITIAL_STATE);
  const insightReviewIdRef = useRef<string | null>(null);
  insightReviewIdRef.current = state.insightReviewId;

  // -------------------------------------------------------------------------
  // Event listeners
  // -------------------------------------------------------------------------

  useEvent<{ content: string }>("insight:synthesis-chunk", ({ content }) => {
    setState((prev) => ({
      ...prev,
      tabs: {
        ...prev.tabs,
        synthesis: {
          status: "streaming",
          content: prev.tabs.synthesis.content + content,
        },
      },
    }));
  });

  useEvent<Record<string, never>>("insight:synthesis-done", () => {
    setState((prev) => ({
      ...prev,
      tabs: {
        ...prev.tabs,
        synthesis: {
          ...prev.tabs.synthesis,
          status: "done",
        },
      },
    }));
  });

  useEvent<{ tab: string; content: string }>("insight:tab-done", ({ tab, content }) => {
    setState((prev) => {
      const tabs = { ...prev.tabs };

      if (tab === "gaps") {
        tabs.gaps = { status: "done", content };
      } else if (tab === "assessment") {
        let questions: QuizQuestion[] = [];
        // Strip markdown fences if present (LLMs often wrap JSON despite instructions)
        let cleaned = content.trim();
        if (cleaned.startsWith("```")) {
          const endIdx = cleaned.lastIndexOf("```");
          if (endIdx > 3) {
            const firstNewline = cleaned.indexOf("\n");
            cleaned = cleaned.slice(firstNewline + 1, endIdx).trim();
          }
        }
        try {
          questions = JSON.parse(cleaned) as QuizQuestion[];
        } catch {
          // malformed payload — keep empty
        }
        tabs.assessment = { status: questions.length > 0 ? "done" : "error", questions };
      } else if (tab === "concept-map") {
        tabs.conceptMap = { status: "done", ...parseConceptMap(content) };
      } else if (tab === "perspectives") {
        tabs.perspectives = { ...tabs.perspectives, status: "done", content };
      }

      return { ...prev, tabs };
    });
  });

  useEvent<{ tab: string; error: string }>("insight:error", ({ tab, error: _error }) => {
    setState((prev) => {
      const tabs = { ...prev.tabs };

      if (tab === "synthesis") {
        tabs.synthesis = { ...tabs.synthesis, status: "error" };
      } else if (tab === "gaps") {
        tabs.gaps = { ...tabs.gaps, status: "error" };
      } else if (tab === "assessment") {
        tabs.assessment = { ...tabs.assessment, status: "error" };
      } else if (tab === "concept-map") {
        tabs.conceptMap = { ...tabs.conceptMap, status: "error" };
      } else if (tab === "perspectives") {
        tabs.perspectives = { ...tabs.perspectives, status: "error" };
      }

      return { ...prev, tabs };
    });
  });

  useEvent<{ personas: PersonaMeta[] }>("insight:perspectives-meta", ({ personas }) => {
    setState((prev) => ({
      ...prev,
      tabs: {
        ...prev.tabs,
        perspectives: {
          ...prev.tabs.perspectives,
          personas,
        },
      },
    }));
  });

  useEvent<{ summary: string }>("insight:changes-summary", ({ summary }) => {
    setState((prev) => ({ ...prev, changesSummary: summary }));
  });

  // -------------------------------------------------------------------------
  // Actions
  // -------------------------------------------------------------------------

  // Shared helper: apply cached insight content to tab state
  // Only marks tabs as "done" if they have content; leaves empty tabs "idle"
  const applyCachedContent = useCallback(
    (cached: InsightReviewCachedResponse, extraState?: Partial<InsightReviewState>) => {
      setState((prev) => ({
        ...prev,
        ...extraState,
        tabs: {
          synthesis: cached.synthesis
            ? { status: "done" as const, content: cached.synthesis }
            : { status: "idle" as const, content: "" },
          gaps: cached.gapAnalysis
            ? { status: "done" as const, content: cached.gapAnalysis }
            : { status: "idle" as const, content: "" },
          assessment: cached.selfAssessment?.length
            ? { status: "done" as const, questions: cached.selfAssessment }
            : { status: "idle" as const, questions: [] },
          conceptMap: cached.conceptMap
            ? { status: "done" as const, ...parseConceptMap(cached.conceptMap) }
            : { status: "idle" as const, mermaid: "", fallbackText: "" },
          perspectives: cached.perspectives
            ? { status: "done" as const, content: cached.perspectives, personas: [] }
            : { status: "idle" as const, content: "", personas: [] },
        },
      }));
    },
    [],
  );

  const open = useCallback(
    async (noteId: string) => {
      setState({ ...INITIAL_STATE, isOpen: true, noteId });

      try {
        const cached = await ipc<InsightReviewCachedResponse | null>("note_insight_cache_get", {
          noteId,
        });
        if (cached) {
          applyCachedContent(cached, { insightReviewId: cached.insightReviewId });
        }
      } catch {
        // No cache — all tabs stay "idle"
      }
    },
    [applyCachedContent],
  );

  const close = useCallback(() => {
    setState(INITIAL_STATE);
  }, []);

  const switchTab = useCallback((tab: TabId) => {
    setState((prev) => ({ ...prev, activeTab: tab }));
  }, []);

  const regenerateTab = useCallback(
    async (tab: TabId) => {
      setState((prev) => {
        const tabs = { ...prev.tabs };

        if (tab === "synthesis") {
          tabs.synthesis = { ...tabs.synthesis, status: "loading" };
        } else if (tab === "gaps") {
          tabs.gaps = { ...tabs.gaps, status: "loading" };
        } else if (tab === "assessment") {
          tabs.assessment = { ...tabs.assessment, status: "loading" };
        } else if (tab === "concept-map") {
          tabs.conceptMap = { ...tabs.conceptMap, status: "loading" };
        } else if (tab === "perspectives") {
          tabs.perspectives = { ...tabs.perspectives, status: "loading" };
        }

        return { ...prev, tabs };
      });

      const response = await ipc<TabContentResponse>("note_insight_regenerate_tab", {
        noteId: state.noteId,
        tab,
      });

      setState((prev) => {
        const tabs = { ...prev.tabs };

        if (response.tab === "synthesis") {
          tabs.synthesis = { status: "done", content: response.content };
        } else if (response.tab === "gaps") {
          tabs.gaps = { status: "done", content: response.content };
        } else if (response.tab === "assessment") {
          let questions: QuizQuestion[] = [];
          try {
            questions = JSON.parse(response.content) as QuizQuestion[];
          } catch {
            // malformed — keep empty
          }
          tabs.assessment = { status: "done", questions };
        } else if (response.tab === "concept-map") {
          tabs.conceptMap = { status: "done", ...parseConceptMap(response.content) };
        } else if (response.tab === "perspectives") {
          tabs.perspectives = { ...tabs.perspectives, status: "done", content: response.content };
        }

        return { ...prev, tabs };
      });
    },
    [state.noteId],
  );

  const saveFlashcards = useCallback(
    async (deckName: string) => {
      await ipc("note_insight_save_flashcards", {
        noteId: state.noteId,
        insightReviewId: state.insightReviewId,
        deckName,
        questions: state.tabs.assessment.questions,
      });
    },
    [state.noteId, state.insightReviewId, state.tabs.assessment.questions],
  );

  const answerQuestion = useCallback((questionId: string, answer: string) => {
    setState((prev) => ({
      ...prev,
      quizState: {
        ...prev.quizState,
        answers: {
          ...prev.quizState.answers,
          [questionId]: answer,
        },
      },
    }));
  }, []);

  const revealAnswer = useCallback((questionId: string) => {
    setState((prev) => {
      const question = prev.tabs.assessment.questions.find((q) => q.id === questionId);
      const userAnswer = prev.quizState.answers[questionId];
      const isCorrect =
        question !== undefined && userAnswer !== undefined && userAnswer === question.correctAnswer;

      const newRevealed = new Set(prev.quizState.revealed);
      newRevealed.add(questionId);

      return {
        ...prev,
        quizState: {
          ...prev.quizState,
          revealed: newRevealed,
          score: isCorrect ? prev.quizState.score + 1 : prev.quizState.score,
          total: prev.tabs.assessment.questions.length,
        },
      };
    });
  }, []);

  const revealAll = useCallback(() => {
    // Compute score synchronously before setState to avoid side-effects in updater
    let computedScore = 0;
    let computedTotal = 0;

    setState((prev) => {
      const questions = prev.tabs.assessment.questions;
      const newRevealed = new Set(questions.map((q) => q.id));

      let score = 0;
      for (const q of questions) {
        const userAnswer = prev.quizState.answers[q.id];
        if (userAnswer !== undefined && userAnswer === q.correctAnswer) {
          score += 1;
        }
      }

      // Capture for IPC call after setState
      computedScore = score;
      computedTotal = questions.length;

      return {
        ...prev,
        quizState: {
          ...prev.quizState,
          revealed: newRevealed,
          score,
          total: questions.length,
        },
      };
    });

    // Submit quiz score to backend (best-effort, outside setState)
    const reviewId = insightReviewIdRef.current;
    if (reviewId && computedTotal > 0) {
      ipc("note_insight_submit_quiz", {
        insightReviewId: reviewId,
        score: computedScore / computedTotal,
        total: computedTotal,
      }).catch(() => {});
    }
  }, []);

  const actions: InsightReviewActions = {
    open,
    applyCachedContent,
    close,
    switchTab,
    regenerateTab,
    saveFlashcards,
    answerQuestion,
    revealAnswer,
    revealAll,
  };

  return [state, actions];
}
