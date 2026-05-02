import type {
  CalendarEventInput,
  DashboardIntelligenceResponse,
  ProductivitySummaryResponse,
  TaskResponse,
  TaskUpdateParams,
  TimelineResponse,
  TimelineSource,
} from "@/bindings";
import { commands } from "@/bindings";

export const EMPTY_TIMELINE_RESPONSE: TimelineResponse = {
  entries: [],
  summary: {
    totalTrackedSecs: 0,
    focusSecs: 0,
    tasksCompleted: 0,
    tasksCreated: 0,
    notesTouched: 0,
    transactionsCount: 0,
    topApps: [],
    sourceBreakdown: [],
  },
};

export async function timelineQuery(
  startDate: string,
  endDate: string,
  sources: TimelineSource[] | null,
  includePointEvents: boolean | null,
  tzOffsetMins: number | null,
): Promise<TimelineResponse> {
  const r = await commands.timelineQuery(
    startDate,
    endDate,
    sources,
    includePointEvents,
    tzOffsetMins,
  );
  if (r.status !== "ok") throw new Error(r.error.message ?? "timeline query failed");
  return r.data;
}

export async function taskUpdate(params: TaskUpdateParams): Promise<TaskResponse> {
  const r = await commands.taskUpdate(params);
  if (r.status !== "ok") throw new Error(r.error.message ?? "task update failed");
  return r.data;
}

/**
 * Trigger a calendar sync. The backend command takes an `events` array; the
 * frontend sends an empty array to request a pull-mode sync.
 */
export async function calendarSyncEvents(): Promise<void> {
  // commands.calendarSyncEvents signature is auto-generated from bindings.ts;
  // pass an empty events array per the existing convention in CalendarSync.tsx.
  const r = await commands.calendarSyncEvents([] as CalendarEventInput[]);
  if (r.status !== "ok") throw new Error(r.error.message ?? "calendar sync failed");
  return;
}

export async function productivityTodayQuery(): Promise<ProductivitySummaryResponse | null> {
  const r = await commands.productivityToday();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity today failed");
  return r.data;
}

export async function dashboardIntelligenceQuery(
  date: string,
  tzOffsetMins: number | null,
): Promise<DashboardIntelligenceResponse> {
  const r = await commands.getDashboardIntelligence(date, tzOffsetMins);
  if (r.status !== "ok") throw new Error(r.error.message ?? "dashboard intelligence failed");
  return r.data;
}
