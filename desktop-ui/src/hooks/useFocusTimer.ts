import { useCallback, useEffect, useState } from "react";
import { useEvent } from "./useEvent";
import { useMutation } from "./useMutation";
import { useQuery } from "./useQuery";
import type {
  FocusCompletedPayload,
  FocusSession,
  FocusTickPayload,
  FocusTimerStatus,
} from "../lib/types";

const STORAGE_KEYS = {
  focusDuration: "klynt:focus:lastDuration",
  pomodoroWork: "klynt:pomodoro:lastWorkDuration",
  pomodoroBreak: "klynt:pomodoro:lastBreakDuration",
} as const;

function getStoredDuration(key: string, fallback: number): number {
  try {
    const val = localStorage.getItem(key);
    return val ? Number.parseInt(val, 10) : fallback;
  } catch {
    return fallback;
  }
}

function storeDuration(key: string, value: number) {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // localStorage may be unavailable
  }
}

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

  // Stored durations
  const [focusDuration, setFocusDuration] = useState(() =>
    getStoredDuration(STORAGE_KEYS.focusDuration, 25),
  );
  const [pomodoroWork, setPomodoroWork] = useState(() =>
    getStoredDuration(STORAGE_KEYS.pomodoroWork, 25),
  );
  const [pomodoroBreak, setPomodoroBreak] = useState(() =>
    getStoredDuration(STORAGE_KEYS.pomodoroBreak, 5),
  );

  // Listen to tick events
  useEvent<FocusTickPayload>("focus:tick", (payload) => {
    if (payload) {
      setRemainingSecs(payload.remainingSecs);
      setTotalSecs(payload.totalSecs);
    }
  });

  // Listen to completion events
  useEvent<FocusCompletedPayload>("focus:completed", (payload) => {
    if (payload) {
      setCompleted(payload);
      setRemainingSecs(null);
      setTotalSecs(null);
      refetch();
    }
  });

  // Sync with status on mount
  useEffect(() => {
    if (timerStatus.active && timerStatus.totalSecs) {
      setTotalSecs(timerStatus.totalSecs);
    }
  }, [timerStatus.active, timerStatus.totalSecs]);

  const startFocus = useCallback(
    async (mins: number) => {
      storeDuration(STORAGE_KEYS.focusDuration, mins);
      setFocusDuration(mins);
      setCompleted(null);
      await startTimer.mutate({ mode: "focus", work_mins: mins });
      refetch();
    },
    [startTimer, refetch],
  );

  const startPomodoro = useCallback(
    async (workMins: number, breakMins: number) => {
      storeDuration(STORAGE_KEYS.pomodoroWork, workMins);
      storeDuration(STORAGE_KEYS.pomodoroBreak, breakMins);
      setPomodoroWork(workMins);
      setPomodoroBreak(breakMins);
      setCompleted(null);
      await startTimer.mutate({ mode: "pomodoro", work_mins: workMins, break_mins: breakMins });
      refetch();
    },
    [startTimer, refetch],
  );

  const stop = useCallback(
    async (notes?: string) => {
      await stopTimer.mutate({ notes });
      setRemainingSecs(null);
      setTotalSecs(null);
      refetch();
    },
    [stopTimer, refetch],
  );

  const dismissCompleted = useCallback(() => setCompleted(null), []);

  return {
    active: timerStatus.active,
    mode: timerStatus.mode,
    session: timerStatus.session,
    remainingSecs,
    totalSecs,
    completed,
    loading: startTimer.loading || stopTimer.loading,
    // Stored defaults
    focusDuration,
    pomodoroWork,
    pomodoroBreak,
    // Actions
    startFocus,
    startPomodoro,
    stop,
    dismissCompleted,
  };
}
