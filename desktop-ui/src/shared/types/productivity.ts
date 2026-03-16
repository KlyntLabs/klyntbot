// ── Productivity Summary ────────────────────────────────────

export interface ProductivitySummary {
  date: string;
  totalActiveSecs: number;
  totalFocusSecs: number;
  totalBreakSecs: number;
  totalIdleSecs: number;
  productiveSecs: number;
  neutralSecs: number;
  distractingSecs: number;
  focusSessionsCount: number;
  avgSessionQuality: number | null;
  interruptionsCount: number;
  contextSwitches: number;
  topApps: AppUsage[];
  topCategories: CategoryUsage[];
  topProjects: ProjectUsage[];
  aiSummary: string | null;
  productivityScore: number | null;
  scoreTrend?: number | null;
  focusTimeTrend?: number | null;
  activeTimeTrend?: number | null;
  deepWorkBlocks: number;
  deepWorkSecs: number;
  avgRecoverySecs: number | null;
}

// ── Project & App Usage ─────────────────────────────────────

export interface ProjectUsage {
  projectId: string;
  displayName: string;
  durationSecs: number;
  color: string | null;
}

export interface AppUsage {
  appName: string;
  durationSecs: number;
  category: string | null;
}

export interface CategoryUsage {
  categoryId: string;
  category: string;
  categoryType: "productive" | "neutral" | "distracting";
  durationSecs: number;
}

export interface TrackedApp {
  displayName: string;
  appName: string;
  siteName: string | null;
  categoryId: string | null;
  categoryName: string | null;
  totalSecs: number;
  eventCount: number;
}

// ── Productivity Project Config ─────────────────────────────

export interface ProductivityProject {
  id: string;
  displayName: string;
  path: string;
  urlPatterns: string[];
  color: string | null;
  isAutoDetected: boolean;
}

// ── Focus Sessions ──────────────────────────────────────────

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

// ── Activity Timeline ───────────────────────────────────────

export interface ActivityTimeline {
  appName: string;
  windowTitle: string | null;
  siteName: string | null;
  categoryId: string | null;
  startedAt: string;
  durationSecs: number | null;
  isIdle: boolean;
  projectId: string | null;
  focusSessionId: string | null;
}

// ── Activity Categories ─────────────────────────────────────

export interface CategoryRules {
  appNames: string[];
  bundleIds: string[];
  urlPatterns: string[];
}

export interface ActivityCategory {
  id: string;
  name: string;
  categoryType: string;
  color: string | null;
  icon: string | null;
  isSystem: boolean;
  rules: CategoryRules | null;
}

// ── Goals & Time Entries ────────────────────────────────────

export interface GoalProgress {
  id: number;
  goalType: string;
  metric: string;
  targetValue: number;
  currentValue: number;
  met: boolean;
  projectId: string | null;
}

export interface TimeEntry {
  id: number;
  description: string;
  categoryId: string | null;
  projectId: string | null;
  startedAt: string;
  durationSecs: number;
  source: string;
}

// ── Learned Rules ───────────────────────────────────────────

export interface LearnedRule {
  id: number;
  pattern: string;
  patternType: string;
  classification: string;
  confidence: number;
  hitCount: number;
  lastUsedAt: string;
  createdAt: string;
}

// ── Real-time Payloads ──────────────────────────────────────

export interface ActivitySwitchPayload {
  fromApp: string | null;
  toApp: string;
  toSite: string | null;
  categoryType: string | null;
}

export interface AutoFocusPayload {
  startedAt: string;
  endedAt: string;
  durationMins: number;
  dominantApp: string;
  productiveRatio: number;
}

export interface ScorePayload {
  score: number;
  productiveSecs: number;
  distractingSecs: number;
}

export interface FocusStatePayload {
  state: string;
  since: string;
}

// ── Insights ────────────────────────────────────────────────

export interface InsightCard {
  id: string;
  insightType: string;
  title: string;
  body: string;
  sentiment: string;
  metricValue: number | null;
  baselineValue: number | null;
  date: string;
  dismissed: boolean;
  generatedAt: string;
}

export type InsightPayload = Pick<InsightCard, "id" | "insightType" | "title" | "sentiment">;

// ── Focus Timer ─────────────────────────────────────────────

export interface FocusTimerStatus {
  active: boolean;
  mode: string | null;
  remainingSecs: number | null;
  totalSecs: number | null;
  session: FocusSession | null;
}

export interface FocusTickPayload {
  remainingSecs: number;
  totalSecs: number;
  mode: string;
  paused: boolean;
  actionTitle: string | null;
}

export interface FocusCompletedPayload {
  mode: string;
  durationMins: number;
  qualityScore: number | null;
  breakMins: number | null;
}

// ── Intelligence Sessions (backend-scored productivity sessions) ─

export interface IntelligenceSession {
  id: string;
  sessionType: string;
  startedAt: string;
  endedAt: string | null;
  durationSecs: number | null;
  dominantCategory: string | null;
  categoryPurity: number | null;
  qualityScore: number | null;
  title: string | null;
  description: string | null;
  appBreakdown: string | null;
  contextSwitches: number;
  distractionCount: number;
  source: string;
}

// ── Weekly Assessment ───────────────────────────────────────

export interface WeeklyAssessment {
  id: string;
  weekStart: string;
  weekEnd: string;
  avgScore: number | null;
  totalFocusMins: number | null;
  totalProductiveSecs: number | null;
  totalDistractingSecs: number | null;
  topApps: string | null;
  summary: string | null;
}

// ── Utility Types ───────────────────────────────────────────

// ── Productivity Patterns ──────────────────────────────────

export interface ProductivityPatterns {
  peakFocusHours: number[];
  avgSessionMins: number;
  productiveRatio: number;
  avgContextSwitches: number;
  bestDayOfWeek: string | null;
  daysAnalyzed: number;
}

// ── Hourly Breakdown ───────────────────────────────────────

export interface HourlyBreakdown {
  hour: number;
  productiveSecs: number;
  neutralSecs: number;
  distractingSecs: number;
  idleSecs: number;
  totalSecs: number;
  productiveRatio: number;
}

export type ProductivityPeriod = "day" | "week" | "month";

// ── Mutation Parameters ─────────────────────────────────────

export interface GoalCreateParams {
  goal_type: string;
  metric: string;
  target_value: number;
}
