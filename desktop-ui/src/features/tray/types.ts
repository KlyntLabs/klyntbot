export interface TodayTask {
  id: string;
  title: string;
  priority: string | null;
  status: string;
  completed: boolean;
  isOverdue: boolean;
  isDueToday: boolean;
  dueDisplay: string | null;
}

export interface CalendarEvent {
  id: string;
  calendarId: string;
  title: string;
  description: string | null;
  startedAt: string;
  endedAt: string;
  location: string | null;
  attendeesCount: number;
  isRecurring: boolean;
  recurrenceId: string | null;
  source: string;
  externalUid: string;
  sessionId: string | null;
  color: string | null;
  syncedAt: string;
}

export interface FocusSession {
  id: string;
  actionId: string | null;
  projectId: string | null;
  sessionType: string;
  targetMins: number | null;
  startedAt: string;
  endedAt: string | null;
  actualMins: number | null;
  interruptions: number;
  qualityScore: number | null;
  completed: boolean;
  notes: string | null;
}

export interface FocusSyncPayload {
  phase: "working" | "break_pending" | "break" | "paused";
  remainingSecs: number;
  totalSecs: number;
  cyclePosition: number;
  longBreakAfter: number;
  paused: boolean;
  actionTitle: string | null;
  dndActive: boolean;
}

export interface FocusWarningPayload {
  phase: string;
  remainingSecs: number;
}

export interface FocusDndUnavailablePayload {
  message: string;
}

export interface FocusSessionStatus {
  active: boolean;
  sync: FocusSyncPayload | null;
  session: FocusSession | null;
}

export interface FocusSettings {
  focusDuration: number;
  shortBreak: number;
  longBreak: number;
  longBreakAfter: number;
  dndEnabled: boolean;
  soundEnabled: boolean;
  notificationEnabled: boolean;
}

export interface FocusPreset {
  label: string;
  focusDuration: number;
  shortBreak: number;
}

export type FocusPhase = "idle" | "working" | "break_pending" | "break";
