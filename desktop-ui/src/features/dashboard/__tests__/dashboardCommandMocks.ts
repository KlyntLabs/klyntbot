/**
 * Default mock factories for dashboard endpoint wrappers.
 * Use in tests via:
 *   vi.mock("@/api/endpoints/dashboard", async () => ({
 *     ...(await vi.importActual<typeof import("@/api/endpoints/dashboard")>("@/api/endpoints/dashboard")),
 *     ...defaultDashboardMocks(),
 *   }));
 */
import { vi } from "vitest";

export function defaultDashboardMocks() {
  return {
    timelineQuery: vi.fn().mockResolvedValue({
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
    }),
    taskUpdate: vi.fn(),
    calendarSyncEvents: vi.fn(),
    productivityCalendarEvents: vi.fn().mockResolvedValue([]),
    productivityTodayQuery: vi.fn().mockResolvedValue(null),
    dashboardIntelligenceQuery: vi.fn().mockResolvedValue({
      activeContext: null,
      focusRecommendation: null,
      sessionSummary: [],
      contextSwitches: 0,
      switchQuality: "neutral",
      productivityScore: 0,
      scoreTrend: 0,
      patterns: [],
      nudges: [],
      resourceClusters: [],
    }),
    productivitySummaryRangeQuery: vi.fn().mockResolvedValue([]),
    productivityWeeklyQuery: vi.fn().mockResolvedValue([]),
    productivityPatternsQuery: vi.fn().mockResolvedValue({
      daysAnalyzed: 0,
      peakFocusHours: [],
      bestDayOfWeek: null,
      avgSessionMins: 0,
    }),
    productivityHourlyBreakdownQuery: vi.fn().mockResolvedValue([]),
    productivityTimelineQuery: vi.fn().mockResolvedValue([]),
    productivityCategoriesQuery: vi.fn().mockResolvedValue([]),
    productivityIntelligenceSessionsQuery: vi.fn().mockResolvedValue([]),
    productivityGoalsQuery: vi.fn().mockResolvedValue([]),
    productivityGoalCreate: vi.fn(),
    productivityGoalDelete: vi.fn(),
  };
}
