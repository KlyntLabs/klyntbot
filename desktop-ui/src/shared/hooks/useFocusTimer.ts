import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import type {
  FocusCompletedPayload,
  FocusSession,
  FocusTickPayload,
  FocusTimerStatus,
} from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

// ── Settings persistence ────────────────────────────────────────────

const SETTINGS_KEY = "klynt:focus:settings";

export interface FocusSettings {
  focusDuration: number; // work session (minutes)
  shortBreak: number; // short break (minutes)
  longBreak: number; // long break (minutes)
  longBreakAfter: number; // sessions before long break
  dndEnabled: boolean; // macOS Do Not Disturb
  soundEnabled: boolean; // play sound on completion
  notificationEnabled: boolean; // show OS notification on completion
}

const DEFAULT_SETTINGS: FocusSettings = {
  focusDuration: 25,
  shortBreak: 5,
  longBreak: 15,
  longBreakAfter: 4,
  dndEnabled: false,
  soundEnabled: true,
  notificationEnabled: true,
};

export interface FocusPreset {
  label: string;
  focusDuration: number;
  shortBreak: number;
}

export const FOCUS_PRESETS: FocusPreset[] = [
  { label: "Standard", focusDuration: 25, shortBreak: 5 },
  { label: "Deep Work", focusDuration: 50, shortBreak: 10 },
  { label: "Sprint", focusDuration: 15, shortBreak: 3 },
];

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

// ── Phase state machine ─────────────────────────────────────────────

export type FocusPhase = "idle" | "focus" | "break_pending" | "break";

// ── Coaching intervention ────────────────────────────────────────────

interface CoachingIntervention {
  message: string;
  interventionType: string;
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
    {
      mode: string;
      work_mins: number;
      break_mins?: number;
      action_id?: string;
      action_title?: string;
      sound_enabled?: boolean;
      notification_enabled?: boolean;
    }
  >("focus_timer_start");
  const stopTimer = useMutation<FocusSession | null, { notes?: string }>("focus_timer_stop");
  const breakStartMut = useMutation<void, { break_mins: number }>("focus_break_start");
  const extendMut = useMutation<boolean, { extra_secs: number }>("focus_timer_extend");
  const pauseMut = useMutation<boolean, Record<string, never>>("focus_timer_pause");
  const resumeMut = useMutation<boolean, Record<string, never>>("focus_timer_resume");
  const logDistractionMut = useMutation<void, { app_name: string }>("distraction_dismiss");

  const [todayDate] = useState(todayISO);
  const { data: todaySessions, refetch: refetchToday } = useQuery<FocusSession[]>(
    "productivity_sessions",
    { date: todayDate },
    [],
  );

  const [phase, setPhase] = useState<FocusPhase>("idle");
  const [paused, setPaused] = useState(false);
  const [remainingSecs, setRemainingSecs] = useState<number | null>(null);
  const [totalSecs, setTotalSecs] = useState<number | null>(null);
  const [completed, setCompleted] = useState<FocusCompletedPayload | null>(null);
  const [settings, setSettings] = useState(loadSettings);
  const [actionTitle, setActionTitle] = useState<string | null>(null);
  const [selectedTask, setSelectedTask] = useState<{ id: string; title: string } | null>(null);
  const [coaching, setCoaching] = useState<CoachingIntervention | null>(null);
  const autoBreakTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Stable refs for mutation functions used in effects/timeouts
  const breakStartRef = useRef(breakStartMut.mutate);
  breakStartRef.current = breakStartMut.mutate;
  const refetchRef = useRef(refetch);
  refetchRef.current = refetch;
  const refetchTodayRef = useRef(refetchToday);
  refetchTodayRef.current = refetchToday;

  // Real-time tick events from Rust timer
  useEvent<FocusTickPayload>("focus:tick", (payload) => {
    if (payload) {
      setRemainingSecs(payload.remainingSecs);
      setTotalSecs(payload.totalSecs);
      setPaused(payload.paused);
      setActionTitle(payload.actionTitle ?? null);
    }
  });

  // Coaching intervention after focus completion
  useEvent<CoachingIntervention>("coaching:intervention", (payload) => {
    if (payload?.message) {
      setCoaching(payload);
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
      }
      refetch();
      refetchToday();
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

  // Derive session count + today stats from SQLite in a single pass
  const { completedSessions, todayStats } = useMemo(() => {
    let sessions = 0;
    let totalMins = 0;
    let qualitySum = 0;
    let qualityCount = 0;
    for (const s of todaySessions) {
      if (!s.completed) continue;
      sessions++;
      totalMins += s.actualMins ?? 0;
      if (s.qualityScore != null) {
        qualitySum += s.qualityScore;
        qualityCount++;
      }
    }
    return {
      completedSessions: sessions,
      todayStats: {
        sessions,
        totalMins,
        avgQuality: qualityCount > 0 ? qualitySum / qualityCount : null,
      },
    };
  }, [todaySessions]);

  // Shared helper: reset cycle if needed and start a focus timer
  const launchFocus = useCallback(
    async (workMins: number) => {
      setCompleted(null);
      setCoaching(null);
      setPaused(false);
      setPhase("focus");
      const cyclePosition = completedSessions % settings.longBreakAfter;
      const nextIsLongBreak = cyclePosition + 1 >= settings.longBreakAfter;
      const breakMins = nextIsLongBreak ? settings.longBreak : settings.shortBreak;
      await startTimer.mutate({
        mode: "focus",
        work_mins: workMins,
        break_mins: breakMins,
        action_id: selectedTask?.id,
        action_title: selectedTask?.title,
        sound_enabled: settings.soundEnabled,
        notification_enabled: settings.notificationEnabled,
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
      refetchToday();
    },
    [stopTimer, refetch, refetchToday],
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
    const cyclePosition = completedSessions % settings.longBreakAfter;
    const nextIsLongBreak = cyclePosition + 1 >= settings.longBreakAfter;
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

  const logDistraction = useCallback(
    async (category: string) => {
      await logDistractionMut.mutate({ app_name: category });
    },
    [logDistractionMut],
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
        sound_enabled: settings.soundEnabled,
        notification_enabled: settings.notificationEnabled,
      });
      refetch();
    },
    [startTimer, refetch, completed, settings, selectedTask],
  );

  const skipBreak = useCallback(async () => {
    if (autoBreakTimer.current) clearTimeout(autoBreakTimer.current);
    if (phase === "break") {
      await stopTimer.mutate({});
    }
    await launchFocus(settings.focusDuration);
  }, [stopTimer, phase, launchFocus, settings.focusDuration]);

  const activePreset = useMemo(
    () =>
      FOCUS_PRESETS.find(
        (p) => p.focusDuration === settings.focusDuration && p.shortBreak === settings.shortBreak,
      )?.label ?? "Custom",
    [settings.focusDuration, settings.shortBreak],
  );

  const applyPreset = useCallback(
    (preset: FocusPreset) => {
      updateSettings({ focusDuration: preset.focusDuration, shortBreak: preset.shortBreak });
    },
    [updateSettings],
  );

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
      resumeMut.loading ||
      logDistractionMut.loading,
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
    logDistraction,
    coaching,
    dismissCoaching: useCallback(() => setCoaching(null), []),
    skipBreak,
    todayStats,
    activePreset,
    applyPreset,
    dismissCompleted,
    selectedTaskId: selectedTask?.id ?? null,
    selectedTaskTitle: selectedTask?.title ?? null,
    selectTask: (id: string | null, title: string | null) => {
      setSelectedTask(id && title ? { id, title } : null);
    },
  };
}
