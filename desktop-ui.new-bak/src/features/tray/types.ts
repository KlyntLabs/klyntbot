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

export * from "../focus/types";
