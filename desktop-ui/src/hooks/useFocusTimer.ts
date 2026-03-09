import { useCallback, useEffect, useState } from "react";
import type {
  FocusCompletedPayload,
  FocusSession,
  FocusTickPayload,
  FocusTimerStatus,
} from "../lib/types";
import { useEvent } from "./useEvent";
import { useMutation } from "./useMutation";
import { useQuery } from "./useQuery";

// ── Settings persistence ────────────────────────────────────────────

const SETTINGS_KEY = "klynt:focus:settings";
const SESSIONS_KEY = "klynt:focus:completedSessions";

export interface FocusSettings {
  focusDuration: number; // work session (minutes)
  shortBreak: number; // short break (minutes)
  longBreak: number; // long break (minutes)
  longBreakAfter: number; // sessions before long break
  dndEnabled: boolean; // macOS Do Not Disturb
}

const DEFAULT_SETTINGS: FocusSettings = {
  focusDuration: 25,
  shortBreak: 5,
  longBreak: 15,
  longBreakAfter: 4,
  dndEnabled: false,
};

function loadSettings(): FocusSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    /* corrupted — fall through */
  }
  return { ...DEFAULT_SETTINGS };
}

function saveSettings(s: FocusSettings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
  } catch {
    /* localStorage may be unavailable */
  }
}

function loadSessions(): number {
  try {
    return Number.parseInt(localStorage.getItem(SESSIONS_KEY) ?? "0", 10);
  } catch {
    return 0;
  }
}

function saveSessions(n: number) {
  try {
    localStorage.setItem(SESSIONS_KEY, String(n));
  } catch {
    /* noop */
  }
}

// ── Hook ────────────────────────────────────────────────────────────

export function useFocusTimer() {
  const { data: timerStatus, refetch } = useQuery<FocusTimerStatus>(
    "focus_timer_status",
    undefined,
    { active: false, mode: null, remainingSecs: null, totalSecs: null, session: null },
  );

  const startTimer = useMutation<
    FocusSession,
    { mode: string; work_mins: number; break_mins?: number }
  >("focus_timer_start");
  const stopTimer = useMutation<FocusSession | null, { notes?: string }>("focus_timer_stop");

  const [remainingSecs, setRemainingSecs] = useState<number | null>(null);
  const [totalSecs, setTotalSecs] = useState<number | null>(null);
  const [completed, setCompleted] = useState<FocusCompletedPayload | null>(null);
  const [settings, setSettings] = useState(loadSettings);
  const [completedSessions, setCompletedSessions] = useState(loadSessions);

  // Real-time tick events from Rust timer
  useEvent<FocusTickPayload>("focus:tick", (payload) => {
    if (payload) {
      setRemainingSecs(payload.remainingSecs);
      setTotalSecs(payload.totalSecs);
    }
  });

  // Session completion event from Rust timer
  useEvent<FocusCompletedPayload>("focus:completed", (payload) => {
    if (payload) {
      setCompleted(payload);
      setRemainingSecs(null);
      setTotalSecs(null);
      setCompletedSessions((prev) => {
        const next = prev + 1;
        saveSessions(next);
        return next;
      });
      refetch();
    }
  });

  // Sync totalSecs from status on mount (in case we reconnect mid-session)
  useEffect(() => {
    if (timerStatus.active && timerStatus.totalSecs) {
      setTotalSecs(timerStatus.totalSecs);
    }
  }, [timerStatus.active, timerStatus.totalSecs]);

  const updateSettings = useCallback((partial: Partial<FocusSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...partial };
      saveSettings(next);
      return next;
    });
  }, []);

  const start = useCallback(async () => {
    setCompleted(null);
    // Auto-reset cycle if previous cycle was fully completed
    setCompletedSessions((prev) => {
      if (prev >= settings.longBreakAfter) {
        saveSessions(0);
        return 0;
      }
      return prev;
    });
    await startTimer.mutate({ mode: "focus", work_mins: settings.focusDuration });
    refetch();
  }, [startTimer, refetch, settings.focusDuration, settings.longBreakAfter]);

  const stop = useCallback(
    async (notes?: string) => {
      await stopTimer.mutate({ notes });
      setRemainingSecs(null);
      setTotalSecs(null);
      refetch();
    },
    [stopTimer, refetch],
  );

  const resetSessions = useCallback(() => {
    setCompletedSessions(0);
    saveSessions(0);
  }, []);

  const dismissCompleted = useCallback(() => setCompleted(null), []);

  return {
    active: timerStatus.active,
    mode: timerStatus.mode,
    session: timerStatus.session,
    remainingSecs,
    totalSecs,
    completed,
    loading: startTimer.loading || stopTimer.loading,
    settings,
    updateSettings,
    completedSessions,
    start,
    stop,
    resetSessions,
    dismissCompleted,
  };
}
