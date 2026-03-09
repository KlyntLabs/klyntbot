import { useCallback, useEffect, useRef, useState } from "react";
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
    return Number.parseInt(localStorage.getItem(SESSIONS_KEY) || "0", 10);
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

// ── Phase state machine ─────────────────────────────────────────────

export type FocusPhase = "idle" | "focus" | "break_pending" | "break";

// ── Hook ────────────────────────────────────────────────────────────

export function useFocusTimer() {
  const { data: timerStatus, refetch } = useQuery<FocusTimerStatus>(
    "focus_timer_status",
    undefined,
    { active: false, mode: null, remainingSecs: null, totalSecs: null, session: null },
  );

  const startTimer = useMutation<
    FocusSession,
    {
      mode: string;
      work_mins: number;
      break_mins?: number;
      action_id?: string;
      action_title?: string;
    }
  >("focus_timer_start");
  const stopTimer = useMutation<FocusSession | null, { notes?: string }>("focus_timer_stop");
  const breakStartMut = useMutation<void, { break_mins: number }>("focus_break_start");
  const extendMut = useMutation<boolean, { extra_secs: number }>("focus_timer_extend");
  const pauseMut = useMutation<boolean, Record<string, never>>("focus_timer_pause");
  const resumeMut = useMutation<boolean, Record<string, never>>("focus_timer_resume");

  const [phase, setPhase] = useState<FocusPhase>("idle");
  const [paused, setPaused] = useState(false);
  const [remainingSecs, setRemainingSecs] = useState<number | null>(null);
  const [totalSecs, setTotalSecs] = useState<number | null>(null);
  const [completed, setCompleted] = useState<FocusCompletedPayload | null>(null);
  const [settings, setSettings] = useState(loadSettings);
  const [completedSessions, setCompletedSessions] = useState(loadSessions);
  const [actionTitle, setActionTitle] = useState<string | null>(null);
  const [selectedTask, setSelectedTask] = useState<{ id: string; title: string } | null>(null);
  const autoBreakTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Stable refs for mutation functions used in effects/timeouts
  const breakStartRef = useRef(breakStartMut.mutate);
  breakStartRef.current = breakStartMut.mutate;
  const refetchRef = useRef(refetch);
  refetchRef.current = refetch;

  // Real-time tick events from Rust timer
  useEvent<FocusTickPayload>("focus:tick", (payload) => {
    if (payload) {
      setRemainingSecs(payload.remainingSecs);
      setTotalSecs(payload.totalSecs);
      setPaused(payload.paused);
      setActionTitle(payload.actionTitle ?? null);
    }
  });

  // Session completion event from Rust timer
  useEvent<FocusCompletedPayload>("focus:completed", (payload) => {
    if (payload) {
      setCompleted(payload);
      setRemainingSecs(null);
      setTotalSecs(null);
      setPaused(false);

      if (payload.mode === "break") {
        setPhase("idle");
      } else {
        setPhase("break_pending");
        setCompletedSessions((prev) => {
          const next = prev + 1;
          saveSessions(next);
          return next;
        });
      }
      refetch();
    }
  });

  // Auto-start break after 5 seconds in break_pending
  useEffect(() => {
    if (phase === "break_pending" && completed?.breakMins) {
      const breakMins = completed.breakMins;
      autoBreakTimer.current = setTimeout(async () => {
        setPhase("break");
        setCompleted(null);
        await breakStartRef.current({ break_mins: breakMins });
        refetchRef.current();
      }, 5000);
      return () => {
        if (autoBreakTimer.current) clearTimeout(autoBreakTimer.current);
      };
    }
  }, [phase, completed?.breakMins]);

  // Sync phase from backend status on mount (reconnect mid-session)
  useEffect(() => {
    if (timerStatus.active) {
      setTotalSecs(timerStatus.totalSecs);
      if (timerStatus.mode === "break") {
        setPhase("break");
      } else {
        setPhase("focus");
      }
    }
  }, [timerStatus.active, timerStatus.totalSecs, timerStatus.mode]);

  const updateSettings = useCallback((partial: Partial<FocusSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...partial };
      saveSettings(next);
      return next;
    });
  }, []);

  // Shared helper: reset cycle if needed and start a focus timer
  const launchFocus = useCallback(
    async (workMins: number) => {
      setCompleted(null);
      setPaused(false);
      setPhase("focus");
      let sessions = completedSessions;
      if (sessions >= settings.longBreakAfter) {
        sessions = 0;
        setCompletedSessions(0);
        saveSessions(0);
      }
      const nextIsLongBreak = sessions + 1 >= settings.longBreakAfter;
      const breakMins = nextIsLongBreak ? settings.longBreak : settings.shortBreak;
      await startTimer.mutate({
        mode: "focus",
        work_mins: workMins,
        break_mins: breakMins,
        action_id: selectedTask?.id,
        action_title: selectedTask?.title,
      });
      refetch();
    },
    [startTimer, refetch, settings, completedSessions, selectedTask],
  );

  const start = useCallback(
    () => launchFocus(settings.focusDuration),
    [launchFocus, settings.focusDuration],
  );

  const stop = useCallback(
    async (notes?: string) => {
      await stopTimer.mutate({ notes });
      setRemainingSecs(null);
      setTotalSecs(null);
      setPhase("idle");
      setCompleted(null);
      setPaused(false);
      setActionTitle(null);
      setSelectedTask(null);
      refetch();
    },
    [stopTimer, refetch],
  );

  const pause = useCallback(async () => {
    await pauseMut.mutate({});
    setPaused(true);
  }, [pauseMut]);

  const resume = useCallback(async () => {
    await resumeMut.mutate({});
    setPaused(false);
  }, [resumeMut]);

  const startBreak = useCallback(async () => {
    if (autoBreakTimer.current) clearTimeout(autoBreakTimer.current);
    const breakMins = completed?.breakMins ?? settings.shortBreak;
    setPhase("break");
    setCompleted(null);
    setPaused(false);
    await breakStartMut.mutate({ break_mins: breakMins });
    refetch();
  }, [breakStartMut, refetch, completed, settings.shortBreak]);

  // Stop focus early and go straight to break
  const takeBreak = useCallback(async () => {
    await stopTimer.mutate({});
    setCompletedSessions((prev) => {
      const next = prev + 1;
      saveSessions(next);
      return next;
    });
    const nextIsLongBreak = completedSessions + 1 >= settings.longBreakAfter;
    const breakMins = nextIsLongBreak ? settings.longBreak : settings.shortBreak;
    setPhase("break");
    setCompleted(null);
    setPaused(false);
    await breakStartMut.mutate({ break_mins: breakMins });
    refetch();
  }, [stopTimer, breakStartMut, refetch, completedSessions, settings]);

  const extend = useCallback(
    async (extraSecs: number) => {
      await extendMut.mutate({ extra_secs: extraSecs });
    },
    [extendMut],
  );

  const extendWork = useCallback(
    async (mins: number = 5) => {
      if (autoBreakTimer.current) clearTimeout(autoBreakTimer.current);
      setPhase("focus");
      setCompleted(null);
      setPaused(false);
      const breakMins = completed?.breakMins ?? settings.shortBreak;
      await startTimer.mutate({
        mode: "focus",
        work_mins: mins,
        break_mins: breakMins,
        action_id: selectedTask?.id,
        action_title: selectedTask?.title,
      });
      refetch();
    },
    [startTimer, refetch, completed, settings.shortBreak, selectedTask],
  );

  const skipBreak = useCallback(async () => {
    if (autoBreakTimer.current) clearTimeout(autoBreakTimer.current);
    if (phase === "break") {
      await stopTimer.mutate({});
    }
    await launchFocus(settings.focusDuration);
  }, [stopTimer, phase, launchFocus, settings.focusDuration]);

  const resetSessions = useCallback(() => {
    setCompletedSessions(0);
    saveSessions(0);
  }, []);

  const dismissCompleted = useCallback(() => setCompleted(null), []);

  return {
    phase,
    paused,
    active: phase === "focus" || phase === "break",
    mode: timerStatus.mode,
    session: timerStatus.session,
    remainingSecs,
    totalSecs,
    completed,
    actionTitle,
    loading:
      startTimer.loading ||
      stopTimer.loading ||
      breakStartMut.loading ||
      extendMut.loading ||
      pauseMut.loading ||
      resumeMut.loading,
    settings,
    updateSettings,
    completedSessions,
    start,
    stop,
    pause,
    resume,
    startBreak,
    takeBreak,
    extend,
    extendWork,
    skipBreak,
    resetSessions,
    dismissCompleted,
    selectedTaskId: selectedTask?.id ?? null,
    selectedTaskTitle: selectedTask?.title ?? null,
    selectTask: (id: string | null, title: string | null) => {
      setSelectedTask(id && title ? { id, title } : null);
    },
  };
}
