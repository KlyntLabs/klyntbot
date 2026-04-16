import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import type {
  FocusDndUnavailablePayload,
  FocusSession,
  FocusSessionStatus,
  FocusSyncPayload,
  FocusWarningPayload,
} from "@shared/types";
import { useCallback, useEffect, useMemo, useState } from "react";

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

// ── Phase type ──────────────────────────────────────────────────────

export type FocusPhase = "idle" | "working" | "break_pending" | "break";

// ── Coaching intervention ────────────────────────────────────────────

interface CoachingIntervention {
  message: string;
  interventionType: string;
}

// ── Hook ────────────────────────────────────────────────────────────

export function useFocusTimer() {
  // Backend status (on mount / reconnect)
  const { data: initialStatus, refetch } = useQuery<FocusSessionStatus>(
    "focus_session_status",
    undefined,
    { active: false, sync: null, session: null },
  );

  // Mutations
  const startMut = useMutation<FocusSession, Record<string, unknown>>("focus_session_start");
  const stopMut = useMutation<FocusSession | null, { notes?: string }>("focus_session_stop");
  const pauseMut = useMutation<boolean, Record<string, never>>("focus_session_pause");
  const resumeMut = useMutation<boolean, Record<string, never>>("focus_session_resume");
  const extendMut = useMutation<boolean, { extra_secs: number }>("focus_session_extend");
  const startBreakMut = useMutation<boolean, Record<string, never>>("focus_session_start_break");
  const extendWorkMut = useMutation<boolean, { extra_mins: number }>("focus_session_extend_work");
  const skipBreakMut = useMutation<boolean, Record<string, never>>("focus_session_skip_break");
  const takeBreakMut = useMutation<boolean, Record<string, never>>("focus_session_take_break");
  const logDistractionMut = useMutation<void, { app_name: string }>("distraction_dismiss");

  // Today's completed sessions (for stats)
  const [todayDate] = useState(todayISO);
  const { data: todaySessions, refetch: refetchToday } = useQuery<FocusSession[]>(
    "productivity_sessions",
    { date: todayDate },
    [],
  );

  // Server state (updated by sync/phase_changed events)
  const [serverState, setServerState] = useState<FocusSyncPayload | null>(null);
  const [receivedAt, setReceivedAt] = useState<number>(0);
  const [settings, setSettings] = useState(loadSettings);
  const [selectedTask, setSelectedTask] = useState<{ id: string; title: string } | null>(null);
  const [coaching, setCoaching] = useState<CoachingIntervention | null>(null);
  const [showWarning, setShowWarning] = useState(false);
  const [dndHint, setDndHint] = useState<string | null>(null);

  // Local 1-second countdown
  const [localTick, setLocalTick] = useState(0);
  const isRunning = !!serverState && !serverState.paused;
  useEffect(() => {
    if (!isRunning) return;
    const id = setInterval(() => setLocalTick((t) => t + 1), 1000);
    return () => clearInterval(id);
  }, [isRunning]);

  // Reset local tick when server state updates
  // biome-ignore lint/correctness/useExhaustiveDependencies: receivedAt is an intentional trigger
  useEffect(() => {
    setLocalTick(0);
  }, [receivedAt]);

  // Sync event (every 5 seconds)
  useEvent<FocusSyncPayload>("focus:sync", (payload) => {
    if (payload) {
      setServerState(payload);
      setReceivedAt(Date.now());
      setShowWarning(false);
    }
  });

  // Phase changed (instant, on every transition)
  useEvent<FocusSyncPayload>("focus:phase_changed", (payload) => {
    if (payload) {
      setServerState(payload);
      setReceivedAt(Date.now());
      setShowWarning(false);
      refetchToday();
    }
  });

  // Warning (30 seconds remaining)
  useEvent<FocusWarningPayload>("focus:warning", (payload) => {
    if (payload) setShowWarning(true);
  });

  // DND unavailable hint
  useEvent<FocusDndUnavailablePayload>("focus:dnd_unavailable", (payload) => {
    if (payload?.message) setDndHint(payload.message);
  });

  // Coaching intervention after focus completion
  useEvent<CoachingIntervention>("coaching:intervention", (payload) => {
    if (payload?.message) setCoaching(payload);
  });

  // Sync from initial status on mount
  useEffect(() => {
    if (initialStatus.active && initialStatus.sync) {
      setServerState(initialStatus.sync);
      setReceivedAt(Date.now());
    }
  }, [initialStatus.active, initialStatus.sync]);

  // Derived state
  const phase: FocusPhase =
    serverState?.phase === "working" ||
    serverState?.phase === "break_pending" ||
    serverState?.phase === "break"
      ? serverState.phase
      : "idle";
  const paused = serverState?.paused ?? false;
  const isActive = phase === "working" || phase === "break";

  // Local countdown interpolation
  const remainingSecs = useMemo(() => {
    if (!serverState || !isActive) return null;
    const elapsed = localTick;
    return Math.max(0, serverState.remainingSecs - elapsed);
  }, [serverState, isActive, localTick]);

  const totalSecs = serverState?.totalSecs ?? null;
  const cyclePosition = serverState?.cyclePosition ?? 0;
  const longBreakAfter = serverState?.longBreakAfter ?? settings.longBreakAfter;
  const actionTitle = serverState?.actionTitle ?? null;

  const updateSettings = useCallback((partial: Partial<FocusSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...partial };
      saveSettings(next);
      return next;
    });
  }, []);

  // Today stats
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

  // Actions
  const start = useCallback(async () => {
    setCoaching(null);
    setShowWarning(false);
    setDndHint(null);
    await startMut.mutate({
      work_secs: settings.focusDuration * 60,
      short_break_secs: settings.shortBreak * 60,
      long_break_secs: settings.longBreak * 60,
      long_break_after: settings.longBreakAfter,
      action_id: selectedTask?.id,
      action_title: selectedTask?.title,
      dnd_enabled: settings.dndEnabled,
      sound_enabled: settings.soundEnabled,
      notification_enabled: settings.notificationEnabled,
    });
    refetch();
  }, [startMut, refetch, settings, selectedTask]);

  const stop = useCallback(
    async (notes?: string) => {
      await stopMut.mutate({ notes });
      setServerState(null);
      setSelectedTask(null);
      setShowWarning(false);
      setCoaching(null);
      refetch();
      refetchToday();
    },
    [stopMut, refetch, refetchToday],
  );

  const pause = useCallback(async () => {
    await pauseMut.mutate({});
  }, [pauseMut]);

  const resume = useCallback(async () => {
    await resumeMut.mutate({});
  }, [resumeMut]);

  const extend = useCallback(
    async (extraSecs: number) => {
      await extendMut.mutate({ extra_secs: extraSecs });
      setShowWarning(false);
    },
    [extendMut],
  );

  const startBreak = useCallback(async () => {
    await startBreakMut.mutate({});
  }, [startBreakMut]);

  const extendWork = useCallback(
    async (mins: number) => {
      await extendWorkMut.mutate({ extra_mins: mins });
    },
    [extendWorkMut],
  );

  const skipBreak = useCallback(async () => {
    await skipBreakMut.mutate({});
  }, [skipBreakMut]);

  const takeBreak = useCallback(async () => {
    await takeBreakMut.mutate({});
  }, [takeBreakMut]);

  const logDistraction = useCallback(
    async (category: string) => {
      await logDistractionMut.mutate({ app_name: category });
    },
    [logDistractionMut],
  );

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

  return {
    // State
    phase,
    paused,
    active: isActive,
    remainingSecs,
    totalSecs,
    actionTitle,
    showWarning,
    dndHint,
    coaching,
    settings,
    completedSessions,
    cyclePosition,
    longBreakAfter,
    todayStats,
    activePreset,
    loading:
      startMut.loading ||
      stopMut.loading ||
      pauseMut.loading ||
      resumeMut.loading ||
      extendMut.loading ||
      startBreakMut.loading ||
      extendWorkMut.loading ||
      skipBreakMut.loading ||
      takeBreakMut.loading ||
      logDistractionMut.loading,

    // Actions
    start,
    stop,
    pause,
    resume,
    extend,
    startBreak,
    extendWork,
    skipBreak,
    takeBreak,
    logDistraction,
    updateSettings,
    applyPreset,
    dismissCoaching: useCallback(() => setCoaching(null), []),
    dismissDndHint: useCallback(() => setDndHint(null), []),
    selectTask: useCallback((id: string | null, title: string | null) => {
      setSelectedTask(id && title ? { id, title } : null);
    }, []),
    selectedTaskId: selectedTask?.id ?? null,
    selectedTaskTitle: selectedTask?.title ?? null,
  };
}
