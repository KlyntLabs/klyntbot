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
  open: (noteId: string, scopeConfig?: Record<string, unknown>) => Promise<void>;
  applyCachedContent: (cached: InsightReviewCachedResponse) => void;
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

interface InsightReviewStartedResponse {
  insightReviewId: string;
  contentHash: string;
  cached: boolean;
}

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
        try {
          questions = JSON.parse(content) as QuizQuestion[];
        } catch {
          // malformed payload — keep empty
        }
        tabs.assessment = { status: "done", questions };
      } else if (tab === "concept-map") {
        if (content.startsWith("FALLBACK:")) {
          tabs.conceptMap = {
            status: "done",
            mermaid: "",
            fallbackText: content.slice("FALLBACK:".length),
          };
        } else {
          tabs.conceptMap = { status: "done", mermaid: content, fallbackText: "" };
        }
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
  const applyCachedContent = useCallback((cached: InsightReviewCachedResponse) => {
    setState((prev) => {
      const tabs = { ...prev.tabs };

      tabs.synthesis = { status: "done", content: cached.synthesis ?? "" };
      tabs.gaps = { status: "done", content: cached.gapAnalysis ?? "" };
      tabs.assessment = {
        status: "done",
        questions: cached.selfAssessment ?? [],
      };

      const cm = cached.conceptMap ?? "";
      if (cm.startsWith("FALLBACK:")) {
        tabs.conceptMap = {
          status: "done",
          mermaid: "",
          fallbackText: cm.slice("FALLBACK:".length),
        };
      } else {
        tabs.conceptMap = { status: "done", mermaid: cm, fallbackText: "" };
      }

      tabs.perspectives = {
        status: cached.perspectives ? "done" : "idle",
        content: cached.perspectives ?? "",
        personas: [],
      };

      return { ...prev, tabs };
    });
  }, []);

  const open = useCallback(
    async (noteId: string, scopeConfig?: Record<string, unknown>) => {
      setState({
        ...INITIAL_STATE,
        isOpen: true,
        noteId,
        tabs: {
          synthesis: { status: "loading", content: "" },
          gaps: { status: "loading", content: "" },
          assessment: { status: "loading", questions: [] },
          conceptMap: { status: "loading", mermaid: "", fallbackText: "" },
          perspectives: { status: "loading", content: "", personas: [] },
        },
      });

      const args: Record<string, unknown> = { noteId };
      if (scopeConfig) args.scopeConfig = scopeConfig;

      const response = await ipc<InsightReviewStartedResponse>("note_insight_review", args);

      setState((prev) => ({
        ...prev,
        insightReviewId: response.insightReviewId,
        contentHash: response.contentHash,
      }));

      if (response.cached) {
        const cached = await ipc<InsightReviewCachedResponse>("note_insight_cache_get", { noteId });
        applyCachedContent(cached);
      } else {
        setState((prev) => ({
          ...prev,
          tabs: {
            ...prev.tabs,
            synthesis: { status: "streaming", content: "" },
            perspectives: { status: "loading", content: "", personas: [] },
          },
        }));
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
          if (response.content.startsWith("FALLBACK:")) {
            tabs.conceptMap = {
              status: "done",
              mermaid: "",
              fallbackText: response.content.slice("FALLBACK:".length),
            };
          } else {
            tabs.conceptMap = {
              status: "done",
              mermaid: response.content,
              fallbackText: "",
            };
          }
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
