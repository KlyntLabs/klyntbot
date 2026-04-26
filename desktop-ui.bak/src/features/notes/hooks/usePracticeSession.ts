import { ipc } from "@shared/hooks/useIpc";
import { useCallback, useState } from "react";

export interface PracticeSegment {
  index: number;
  text: string;
  segmentType: string;
  suggestedFocus: string;
  skipped: boolean;
}

export interface PracticeSessionResponse {
  id: string;
  noteId: string;
  sourceLang: string;
  targetLang: string;
  status: string;
  segments: string;
  currentIndex: number;
  results: string;
  userTranslationDoc: string | null;
  averageScore: number | null;
  startedAt: string;
  completedAt: string | null;
}

export interface PracticeConfirmResponse {
  nextIndex: number;
  isComplete: boolean;
}

export interface PracticeCompleteResponse {
  averageScore: number;
  weakUnitCount: number;
  flashcardsCreated: number;
}

export function usePracticeSession() {
  const [session, setSession] = useState<PracticeSessionResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const startSession = useCallback(
    async (
      noteId: string,
      segments: PracticeSegment[],
      sourceLang: string,
      targetLang: string,
      startIndex?: number,
    ) => {
      setLoading(true);
      setError(null);
      try {
        const response = await ipc<PracticeSessionResponse>("practice_start_session", {
          params: { noteId, segments, sourceLang, targetLang, startIndex: startIndex ?? null },
        });
        setSession(response);
        return response;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Failed to start session";
        setError(msg);
        return null;
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  const getSession = useCallback(async (noteId: string) => {
    setLoading(true);
    setError(null);
    try {
      const response = await ipc<PracticeSessionResponse | null>("practice_get_session", {
        params: { noteId },
      });
      setSession(response);
      return response;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Failed to get session";
      setError(msg);
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  const confirmUnit = useCallback(
    async (
      sessionId: string,
      index: number,
      finalTranslation: string,
      confidenceRating: number,
      edited: boolean,
      overallGrade?: string,
      scoresJson?: string,
    ) => {
      setError(null);
      try {
        const response = await ipc<PracticeConfirmResponse>("practice_confirm_unit", {
          params: {
            sessionId,
            index,
            finalTranslation,
            confidenceRating,
            edited,
            overallGrade: overallGrade ?? "",
            scoresJson,
          },
        });
        return response;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : "Failed to confirm unit";
        setError(msg);
        return null;
      }
    },
    [],
  );

  const completeSession = useCallback(async (sessionId: string, saveToSr: boolean) => {
    setLoading(true);
    setError(null);
    try {
      const response = await ipc<PracticeCompleteResponse>("practice_complete_session", {
        params: { sessionId, saveToSr },
      });
      return response;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Failed to complete session";
      setError(msg);
      return null;
    } finally {
      setLoading(false);
    }
  }, []);

  const listSessions = useCallback(async (noteId: string) => {
    setLoading(true);
    setError(null);
    try {
      const response = await ipc<PracticeSessionResponse[]>("practice_list_sessions", {
        params: { noteId },
      });
      return response;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : "Failed to list sessions";
      setError(msg);
      return [];
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    session,
    loading,
    error,
    startSession,
    getSession,
    confirmUnit,
    completeSession,
    listSessions,
  };
}
