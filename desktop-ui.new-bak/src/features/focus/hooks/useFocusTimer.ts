import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { FocusDefaultsUpdate } from "@/bindings";
import { useEvent } from "@/hooks/useEvent";
import { todayISO } from "@/lib/dates";
import { qk, useTauriMutation, useTauriQuery } from "@/lib/query";
import type {
  FocusDndUnavailablePayload,
  FocusPhase,
  FocusPreset,
  FocusSession,
  FocusSessionStatus,
  FocusSettings,
  FocusSyncPayload,
  FocusWarningPayload,
} from "../types";

const SETTINGS_KEY = "klynt:focus:settings";

const DEFAULT_SETTINGS: FocusSettings = {
  focusDuration: 25,
  shortBreak: 5,
  longBreak: 15,
  longBreakAfter: 4,
  dndEnabled: false,
  soundEnabled: true,
  notificationEnabled: true,
};

const FALLBACK_FOCUS_DEFAULTS = {
  workMins: DEFAULT_SETTINGS.focusDuration,
  shortBreakMins: DEFAULT_SETTINGS.shortBreak,
  longBreakMins: DEFAULT_SETTINGS.longBreak,
  longBreakAfter: DEFAULT_SETTINGS.longBreakAfter,
};

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
    /* corrupted */
  }
  return { ...DEFAULT_SETTINGS };
}

function saveSettings(s: FocusSettings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
  } catch {
    /* unavailable */
  }
}

interface CoachingIntervention {
  message: string;
  interventionType: string;
}

export type { FocusPhase, FocusPreset, FocusSettings } from "../types";

export function useFocusTimer() {
  const initialStatusQuery = useTauriQuery<FocusSessionStatus>({
    queryKey: qk.focus.status(),
    command: "focus_session_status",
    fallback: { active: false, sync: null, session: null },
  });
  const initialStatus = initialStatusQuery.data;
  const refetch = useCallback(() => initialStatusQuery.refetch(), [initialStatusQuery]);

  const defaultsQuery = useTauriQuery<{
    workMins: number;
    shortBreakMins: number;
    longBreakMins: number;
    longBreakAfter: number;
  }>({
    queryKey: qk.focus.defaults(),
    command: "focus_defaults_get",
    fallback: FALLBACK_FOCUS_DEFAULTS,
  });

  const startMut = useTauriMutation<FocusSession, Record<string, unknown>>({
    command: "focus_session_start",
  });
  const stopMut = useTauriMutation<FocusSession | null, { notes?: string }>({
    command: "focus_session_stop",
  });
  const pauseMut = useTauriMutation<boolean, Record<string, never>>({
    command: "focus_session_pause",
  });
  const resumeMut = useTauriMutation<boolean, Record<string, never>>({
    command: "focus_session_resume",
  });
  const extendMut = useTauriMutation<boolean, { extra_secs: number }>({
    command: "focus_session_extend",
  });
  const startBreakMut = useTauriMutation<boolean, Record<string, never>>({
    command: "focus_session_start_break",
  });
  const extendWorkMut = useTauriMutation<boolean, { extra_mins: number }>({
    command: "focus_session_extend_work",
  });
  const skipBreakMut = useTauriMutation<boolean, Record<string, never>>({
    command: "focus_session_skip_break",
  });
  const takeBreakMut = useTauriMutation<boolean, Record<string, never>>({
    command: "focus_session_take_break",
  });
  const saveDefaultsMut = useTauriMutation<unknown, { update: FocusDefaultsUpdate }>({
    command: "focus_defaults_set",
  });
  const logDistractionMut = useTauriMutation<void, { appName: string }>({
    command: "distraction_dismiss",
  });

  const [todayDate] = useState(todayISO);
  const todaySessionsQuery = useTauriQuery<FocusSession[]>({
    queryKey: qk.focus.todaySessions(),
    command: "productivity_sessions",
    args: { date: todayDate },
    fallback: [],
  });
  const todaySessions = todaySessionsQuery.data;
  const refetchToday = useCallback(() => todaySessionsQuery.refetch(), [todaySessionsQuery]);

  const [serverState, setServerState] = useState<FocusSyncPayload | null>(null);
  const hasSeededRef = useRef(false);

  const [settings, setSettings] = useState(() => {
    const backendDefaults = defaultsQuery.data;
    return {
      ...DEFAULT_SETTINGS,
      ...(backendDefaults && {
        focusDuration: backendDefaults.workMins,
        shortBreak: backendDefaults.shortBreakMins,
        longBreak: backendDefaults.longBreakMins,
        longBreakAfter: backendDefaults.longBreakAfter,
      }),
      ...loadSettings(),
    };
  });

  useEffect(() => {
    if (hasSeededRef.current) return;
    const backendDefaults = defaultsQuery.data;
    if (!backendDefaults) return;
    hasSeededRef.current = true;
    setSettings((prev) => {
      // Local storage overrides backend defaults; only seed from backend when no
      // local settings have been saved yet.
      if (localStorage.getItem(SETTINGS_KEY)) return prev;
      return {
        ...prev,
        focusDuration: backendDefaults.workMins,
        shortBreak: backendDefaults.shortBreakMins,
        longBreak: backendDefaults.longBreakMins,
        longBreakAfter: backendDefaults.longBreakAfter,
      };
    });
  }, [defaultsQuery.data]);

  const [selectedTask, setSelectedTask] = useState<{
    id: string;
    title: string;
  } | null>(null);
  const [coaching, setCoaching] = useState<CoachingIntervention | null>(null);
  const [showWarning, setShowWarning] = useState(false);
  const [dndHint, setDndHint] = useState<string | null>(null);

  useEvent<FocusSyncPayload>("focus:sync", (payload) => {
    if (payload) {
      setServerState(payload);
      setShowWarning(false);
    }
  });

  useEvent<FocusSyncPayload>("focus:phase_changed", (payload) => {
    if (payload) {
      setServerState(payload);
      setShowWarning(false);
      refetchToday();
    }
  });

  useEvent<FocusWarningPayload>("focus:warning", (payload) => {
    if (payload) setShowWarning(true);
  });

  useEvent<FocusDndUnavailablePayload>("focus:dnd_unavailable", (payload) => {
    if (payload?.message) setDndHint(payload.message);
  });

  useEvent<CoachingIntervention>("coaching:intervention", (payload) => {
    if (payload?.message) setCoaching(payload);
  });

  useEffect(() => {
    if (initialStatus.active && initialStatus.sync) {
      setServerState(initialStatus.sync);
    }
  }, [initialStatus.active, initialStatus.sync]);

  const phase: FocusPhase =
    serverState?.phase === "working" ||
    serverState?.phase === "break_pending" ||
    serverState?.phase === "break" ||
    serverState?.phase === "suspended"
      ? serverState.phase
      : "idle";
  const paused = serverState?.paused ?? false;
  const isActive = phase === "working" || phase === "break";

  const remainingSecs = isActive ? (serverState?.remainingSecs ?? null) : null;

  const totalSecs = serverState?.totalSecs ?? null;
  const cyclePosition = serverState?.cyclePosition ?? 0;
  const longBreakAfter = serverState?.longBreakAfter ?? settings.longBreakAfter;
  const actionTitle = serverState?.actionTitle ?? null;

  const updateSettings = useCallback(
    (partial: Partial<FocusSettings>) => {
      setSettings((prev) => {
        const next = { ...prev, ...partial };
        saveSettings(next);
        void saveDefaultsMut.mutate({
          update: {
            workMins: next.focusDuration,
            shortBreakMins: next.shortBreak,
            longBreakMins: next.longBreak,
            longBreakAfter: next.longBreakAfter,
            autoStartWork: null,
            autoStartBreak: null,
          },
        });
        return next;
      });
    },
    [saveDefaultsMut],
  );

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

  const start = useCallback(async () => {
    setCoaching(null);
    setShowWarning(false);
    setDndHint(null);
    await startMut.mutate({
      params: {
        workSecs: settings.focusDuration * 60,
        shortBreakSecs: settings.shortBreak * 60,
        longBreakSecs: settings.longBreak * 60,
        longBreakAfter: settings.longBreakAfter,
        actionId: selectedTask?.id ?? null,
        actionTitle: selectedTask?.title ?? null,
        dndEnabled: settings.dndEnabled,
        soundEnabled: settings.soundEnabled,
        notificationEnabled: settings.notificationEnabled,
      },
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
      await logDistractionMut.mutate({ appName: category });
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
      updateSettings({
        focusDuration: preset.focusDuration,
        shortBreak: preset.shortBreak,
      });
    },
    [updateSettings],
  );

  return {
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
    isLoading:
      startMut.isLoading ||
      stopMut.isLoading ||
      pauseMut.isLoading ||
      resumeMut.isLoading ||
      extendMut.isLoading ||
      startBreakMut.isLoading ||
      extendWorkMut.isLoading ||
      skipBreakMut.isLoading ||
      takeBreakMut.isLoading ||
      logDistractionMut.isLoading,

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
