import type {
  ActivityCategoryResponse,
  ActivityTimelineResponse,
  CalendarEvent,
  CalendarEventInput,
  DashboardIntelligenceResponse,
  GoalProgressResponse,
  HourlyBreakdownResponse,
  IntelligenceSessionResponse,
  ProductivityPatternsResponse,
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

export async function productivityCalendarEvents(date: string): Promise<CalendarEvent[]> {
  const r = await commands.productivityCalendarEvents(date);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity calendar events failed");
  return r.data;
}

export async function productivitySummaryRangeQuery(
  startDate: string,
  endDate: string,
): Promise<ProductivitySummaryResponse[]> {
  const r = await commands.productivitySummaryRange(startDate, endDate);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity summary range failed");
  return r.data;
}

export async function productivityWeeklyQuery(): Promise<ProductivitySummaryResponse[]> {
  const r = await commands.productivityWeekly();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity weekly failed");
  return r.data;
}

export async function productivityPatternsQuery(
  days: number | null,
): Promise<ProductivityPatternsResponse> {
  const r = await commands.productivityPatterns(days);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity patterns failed");
  return r.data;
}

export async function productivityHourlyBreakdownQuery(
  startDate: string,
  endDate: string,
  tzOffsetMins: number | null,
): Promise<HourlyBreakdownResponse[]> {
  const r = await commands.productivityHourlyBreakdown(startDate, endDate, tzOffsetMins);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity hourly breakdown failed");
  return r.data;
}

export async function productivityTimelineQuery(
  date: string,
  limit: number | null,
  offset: number | null,
  tzOffsetMins: number | null,
): Promise<ActivityTimelineResponse[]> {
  const r = await commands.productivityTimeline(date, limit, offset, tzOffsetMins);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity timeline failed");
  return r.data;
}

export async function productivityCategoriesQuery(): Promise<ActivityCategoryResponse[]> {
  const r = await commands.productivityCategories();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity categories failed");
  return r.data;
}

export async function productivityIntelligenceSessionsQuery(
  date: string,
  tzOffsetMins: number | null,
): Promise<IntelligenceSessionResponse[]> {
  const r = await commands.productivityIntelligenceSessions(date, tzOffsetMins);
  if (r.status !== "ok")
    throw new Error(r.error.message ?? "productivity intelligence sessions failed");
  return r.data;
}

export async function productivityGoalsQuery(): Promise<GoalProgressResponse[]> {
  const r = await commands.productivityGoals();
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity goals failed");
  return r.data;
}

export interface GoalCreateParams {
  goalType: string;
  metric: string;
  targetValue: number;
}

export async function productivityGoalCreate(
  params: GoalCreateParams,
): Promise<GoalProgressResponse> {
  const r = await commands.productivityGoalCreate(
    params.goalType,
    params.metric,
    params.targetValue,
  );
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity goal create failed");
  return r.data;
}

export async function productivityGoalDelete(id: number): Promise<void> {
  const r = await commands.productivityGoalDelete(id);
  if (r.status !== "ok") throw new Error(r.error.message ?? "productivity goal delete failed");
  return;
}

