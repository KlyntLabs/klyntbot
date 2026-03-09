export interface ApiError {
  code: string;
  message: string;
}

// ── Status Workflows ──────────────────────────────────

export type StatusGroup = "not_started" | "active" | "done" | "stuck";

export interface StatusLabel {
  id: string;
  workflowId: string;
  name: string;
  color: string;
  statusGroup: StatusGroup;
  position: number;
}

export interface StatusWorkflow {
  id: string;
  name: string;
  isTemplate: boolean;
  isGlobalDefault: boolean;
  labels: StatusLabel[];
}

export interface Task {
  id: string;
  title: string;
  completed: boolean;
  priority: string | null;
  status: string;
  dueDate: string | null;
  tags: string[];
  projectId: string | null;
  areaId: string;
  objectiveId?: string;
  description?: string;
  parentId: string | null;
  subtaskCount: number;
  subtaskCompletedCount: number;
  statusLabelId: string | null;
  statusLabel: StatusLabel | null;
  groupId: string | null;
}

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

export interface TaskGroup {
  id: string;
  projectId: string | null;
  name: string;
  color: string | null;
  position: number;
  taskCount: number;
}

export interface Project {
  id: string;
  name: string;
  color: string;
  areaId: string;
  taskCount: number;
  completedCount: number;
  objectiveIds?: string[];
  workflowId?: string;
}

export interface Objective {
  id: string;
  title: string;
  status: string;
  progress: number;
  projectId: string;
  keyResults?: KeyResult[];
}

export interface KeyResult {
  id: string;
  title: string;
  progress: number;
  current: number;
  target: number;
  unit: string;
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
  createdAt: string;
  updatedAt: string;
}

export type MessageSegment =
  | { type: "text"; content: string }
  | {
      type: "tool";
      name: string;
      action?: string;
      success: boolean;
      durationMs: number;
      result?: string;
      estimatedTokens?: number;
      agent?: string;
    };

export interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "interaction";
  content: string;
  timestamp?: string;
  segments?: MessageSegment[];
  transparency?: TransparencyData;
}

export interface ChatThread {
  sessionKey: string;
  title: string;
  messageCount: number;
  updatedAt: string;
  contextType?: string;
  entityKind?: string;
  entityId?: string;
  areaId?: string;
  areaName?: string;
  projectId?: string;
  projectName?: string;
}

export interface SessionContext {
  entityKind?: string;
  entityId?: string;
  contextType?: string;
  isEphemeral?: boolean;
}

// ── Agent Streaming Events ──────────────────────────────────────────────

export interface ContentChunkPayload {
  sessionKey: string;
  data: string;
}

export interface ToolStartPayload {
  sessionKey: string;
  name: string;
  action?: string;
  agent?: string;
}

export interface ToolEndPayload {
  sessionKey: string;
  name: string;
  action?: string;
  success: boolean;
  durationMs: number;
  result?: string;
  estimatedTokens?: number;
  agent?: string;
}

export interface AgentDonePayload {
  sessionKey: string;
  content: string;
}

export interface AgentErrorPayload {
  sessionKey: string;
  message: string;
}

export interface ClassificationCompletePayload {
  sessionKey: string;
  strategy: string;
  confidence: number;
  source: string;
}

export interface ExecutionStartedPayload {
  sessionKey: string;
  engine: string;
  maxIterations: number;
}

// ── Transparency Events ───────────────────────────────────────────────

export interface IterationStartPayload {
  sessionKey: string;
  iteration: number;
  maxIterations: number;
}

export interface UsageReportPayload {
  sessionKey: string;
  promptTokens: number;
  completionTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  estimatedCostUsd: number;
  model: string;
  responseTimeMs: number;
}

export interface MemoryAccessPayload {
  sessionKey: string;
  action: string;
  query?: string;
  resultsCount: number;
}

export interface SkillLoadedPayload {
  sessionKey: string;
  name: string;
  trigger: string;
  agent?: string;
}

export interface LearningEventPayload {
  sessionKey: string;
  eventType: string;
  detail: string;
}

export interface SubagentSpawnedPayload {
  sessionKey: string;
  label: string;
  profile: string;
}

export interface AgentSelectedPayload {
  sessionKey: string;
  name: string;
  description: string;
}

// ── Plan Events ─────────────────────────────────────────────────────

export interface PlanGeneratedPayload {
  sessionKey: string;
  steps: string[];
  rawPlan: string;
}

export interface PlanStepCompletedPayload {
  sessionKey: string;
  stepIndex: number;
  description: string;
  toolName: string;
}

// ── Delegation Events ────────────────────────────────────────────────

export interface DelegationStartedPayload {
  sessionKey: string;
  fromAgent: string;
  toAgent: string;
  query: string;
  depth: number;
}

export interface DelegationCompletedPayload {
  sessionKey: string;
  fromAgent: string;
  toAgent: string;
  success: boolean;
  durationMs: number;
}

export interface DelegationInfo {
  fromAgent: string;
  toAgent: string;
  query: string;
  depth: number;
  status: "active" | "completed" | "failed";
  durationMs?: number;
}

export interface PlanData {
  steps: string[];
  completedSteps: number[];
}

// ── Transparency Data (per-message) ───────────────────────────────────

export interface TransparencyData {
  usage?: {
    promptTokens: number;
    completionTokens: number;
    cacheReadTokens: number;
    cacheWriteTokens: number;
  };
  cost?: { estimatedUsd: number; model: string };
  timing?: { totalMs: number; classificationMs?: number; contextAssemblyMs?: number };
  tools?: {
    name: string;
    action?: string;
    success: boolean;
    durationMs: number;
    estimatedTokens?: number;
    agent?: string;
  }[];
  toolTokensTotal?: number;
  memoryAccesses?: { action: string; query?: string; resultsCount: number }[];
  skills?: { name: string; trigger: string; agent?: string }[];
  execution?: { engine: string; iterations: number; maxIterations: number; escalations: number };
  classification?: { strategy: string; confidence: number; source: string };
  agentSelected?: { name: string; description: string };
  subagents?: { label: string; profile: string }[];
  learning?: { eventType: string; detail: string }[];
  delegations?: DelegationInfo[];
  plan?: PlanData;
}

// ── Interaction (ask_user) ─────────────────────────────────────────────

export interface ActiveInteraction {
  requestId: string;
  request: InteractionRequest;
}

export interface InteractionRequestPayload {
  sessionKey: string;
  requestId: string;
  request: InteractionRequest;
}

export interface InteractionRequest {
  title: string;
  questions: Question[];
}

export interface Question {
  id: string;
  title: string;
  text: string;
  answer_type: AnswerType;
}

export type AnswerType =
  | { type: "single_select"; options: AnswerOption[] }
  | { type: "multi_select"; options: AnswerOption[] }
  | { type: "yes_no"; default?: boolean }
  | { type: "free_text"; placeholder?: string };

export interface AnswerOption {
  value: string;
  label: string;
  description?: string;
}

export type AnswerValue =
  | { type: "selected"; value: string }
  | { type: "multi_selected"; values: string[] }
  | { type: "yes_no"; answer: boolean }
  | { type: "text"; content: string }
  | { type: "skipped" };

export interface Answer {
  question_id: string;
  value: AnswerValue;
}

export type FormResponse = { Completed: Answer[] } | "Cancelled";

export interface Area {
  id: string;
  name: string;
  color: string;
  icon: string | null;
  projectCount: number;
  taskCount: number;
}

export interface AgentStatus {
  status: string;
  activeTaskCount: number;
  focusTask: Task | null;
}

export interface LauncherItem {
  id: string;
  title: string;
  subtitle: string;
  icon: string;
  shortcut: string;
}

// ── Notes ───────────────────────────────────────────────────────────────

export interface Note {
  id: string;
  notebookId: string | null;
  title: string;
  body: string;
  bodyHtml: string | null;
  pinned: boolean;
  archived: boolean;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

export interface NoteCreateParams {
  title: string;
  notebookId?: string;
  body?: string;
  tags?: string[];
}

export interface NoteUpdateParams {
  id: string;
  title?: string;
  body?: string;
  bodyHtml?: string;
  pinned?: boolean;
  notebookId?: string | null;
  tags?: string[];
}

export interface NoteVersion {
  id: string;
  noteId: string;
  body: string;
  createdAt: string;
}

export interface NoteLink {
  sourceId: string;
  targetId: string;
}

export interface Notebook {
  id: string;
  parentId: string | null;
  title: string;
  icon: string | null;
  sortOrder: number;
  noteCount: number;
}

export interface NotebookCreateParams {
  title: string;
  parentId?: string;
  icon?: string;
}

// ── Finance ─────────────────────────────────────────────────────────────

export interface FinanceAccount {
  id: string;
  name: string;
  accountType: string;
  currency: string;
  balance: number;
  institution: string | null;
  notes: string | null;
  isArchived: boolean;
}

export interface FinanceTransaction {
  id: string;
  accountId: string;
  txType: "income" | "expense" | "transfer";
  amount: number;
  currency: string;
  category: string | null;
  subcategory: string | null;
  counterparty: string | null;
  notes: string | null;
  txDate: string;
  transferId: string | null;
}

export interface FinanceBudgetUsage {
  id: string;
  name: string;
  amount: number;
  currency: string;
  period: string;
  category: string | null;
  method: string;
  jarType: string | null;
  isActive: boolean;
  alertThreshold: number;
  spent: number;
}

export interface FinancePortfolio {
  id: string;
  name: string;
  description: string | null;
  currency: string;
  totalValue: number;
  totalCostBasis: number;
  holdingCount: number;
}

export interface FinanceInvestment {
  id: string;
  portfolioId: string;
  assetType: string;
  symbol: string | null;
  name: string;
  quantity: number;
  costBasis: number;
  currency: string;
  currentPrice: number | null;
  currentValue: number | null;
}

export interface FinanceGoal {
  id: string;
  name: string;
  goalType: string;
  targetAmount: number;
  currentAmount: number;
  currency: string;
  status: string;
  deadline: string | null;
  monthlyContribution: number | null;
}

export interface FinanceLiability {
  id: string;
  name: string;
  liabilityType: string;
  principal: number;
  remaining: number;
  currency: string;
  interestRate: number | null;
  monthlyPayment: number | null;
  dueDate: string | null;
}

export interface FinanceNetWorth {
  totalsByCurrency: {
    currency: string;
    accounts: number;
    investments: number;
    liabilities: number;
    net: number;
  }[];
}

// ── Finance Mutation Params ──────────────────────────────────────────

export interface FinanceAccountCreateParams {
  name: string;
  accountType: string;
  currency?: string;
  balance?: number;
  institution?: string;
  notes?: string;
}

export interface FinanceTransactionCreateParams {
  accountId: string;
  txType: "income" | "expense" | "transfer";
  amount: number;
  currency?: string;
  category?: string;
  subcategory?: string;
  counterparty?: string;
  txDate?: string;
  notes?: string;
}

export interface FinanceBudgetCreateParams {
  name: string;
  amount: number;
  period: string;
  currency?: string;
  category?: string;
  alertThreshold?: number;
}

export interface FinanceGoalCreateParams {
  name: string;
  goalType: string;
  targetAmount: number;
  currency?: string;
  currentAmount?: number;
  deadline?: string;
  monthlyContribution?: number;
  notes?: string;
}

export interface FinanceLiabilityCreateParams {
  name: string;
  liabilityType: string;
  principal: number;
  currency?: string;
  remaining?: number;
  interestRate?: number;
  monthlyPayment?: number;
  dueDate?: string;
  notes?: string;
}

export interface FinancePortfolioCreateParams {
  name: string;
  description?: string;
  currency?: string;
}

export interface FinanceInvestmentCreateParams {
  portfolioId: string;
  assetType: string;
  costBasis: number;
  quantity: number;
  symbol?: string;
  name?: string;
  currency?: string;
}

// ── Finance Report Types ────────────────────────────────────────────

export interface FinanceCategoryReport {
  total: number;
  breakdown: { category: string; amount: number; pct: number }[];
}

export interface FinanceTrendPoint {
  period: string;
  value: number;
  changePct: number | null;
}

// ── Productivity ─────────────────────────────────────────────────────

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
}

export interface ProjectUsage {
  projectId: string;
  displayName: string;
  durationSecs: number;
  color: string | null;
}

export interface ProductivityProject {
  id: string;
  displayName: string;
  path: string;
  urlPatterns: string[];
  color: string | null;
  isAutoDetected: boolean;
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

export interface TrackedApp {
  displayName: string;
  appName: string;
  siteName: string | null;
  categoryId: string | null;
  categoryName: string | null;
  totalSecs: number;
  eventCount: number;
}

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

export interface GoalCreateParams {
  goal_type: string;
  metric: string;
  target_value: number;
}

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

// ── Productivity V2 Real-time Payloads ───────────────────────────────

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

export type ProductivityPeriod = "day" | "week" | "month";

// ── Coaching ─────────────────────────────────────────────────────────

export interface CoachingIntervention {
  id: string;
  interventionType: string;
  message: string;
  triggerName: string;
  timestamp: string;
}

// ── Custom Columns ──────────────────────────────────────────────────────

export type ColumnType =
  | "text"
  | "number"
  | "date"
  | "dropdown"
  | "multi_select"
  | "checkbox"
  | "tags"
  | "link"
  | "rating"
  | "progress"
  | "duration"
  | "currency";

export interface CustomColumn {
  id: string;
  projectId: string;
  name: string;
  columnType: ColumnType;
  options: string[] | null;
  position: number;
  width: number;
}

export interface CustomColumnValue {
  taskId: string;
  columnId: string;
  value: unknown;
}

export interface ColumnCreateParams {
  projectId: string;
  name: string;
  columnType: ColumnType;
  options?: string[];
  width?: number;
}

export interface ColumnUpdateParams {
  id: string;
  name?: string;
  options?: string[] | null;
  width?: number;
}

export interface ColumnReorderParams {
  projectId: string;
  ids: string[];
}

export interface ColumnValueSetParams {
  taskId: string;
  columnId: string;
  value: unknown;
}

export type Tab = "All" | string;
export type SidebarItem =
  | "Dashboard"
  | "Chat"
  | "Tasks"
  | "OKR"
  | "Calendar"
  | "Notes"
  | "Finance"
  | "Productivity"
  | "Debug"
  | "Settings";
export type ViewMode = "table" | "board" | "tree";

// ── Mutation Params ─────────────────────────────────────────────────────

export interface TaskUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  priority?: number | null;
  status?: string;
  dueDate?: string | null;
  projectId?: string | null;
  areaId?: string;
  tags?: string[];
  keyResultId?: string | null;
  statusLabelId?: string | null;
  position?: number;
  groupId?: string | null;
}

export interface TaskCreateParams {
  title: string;
  areaId?: string;
  projectId?: string;
  priority?: number;
  dueDate?: string;
  tags?: string[];
  parentId?: string;
  groupId?: string;
}

export interface AreaCreateParams {
  name: string;
  color?: string;
  icon?: string;
}

export interface AreaUpdateParams {
  id: string;
  name?: string;
  color?: string;
  icon?: string | null;
}

export interface ProjectCreateParams {
  name: string;
  areaId: string;
  color?: string;
  description?: string;
  tags?: string[];
}

export interface ProjectUpdateParams {
  id: string;
  name?: string;
  areaId?: string;
  color?: string;
  description?: string | null;
  tags?: string[];
  status?: string;
}

export interface ObjectiveCreateParams {
  title: string;
  projectId: string;
  description?: string;
  priority?: number;
  dueDate?: string;
}

export interface ObjectiveUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  status?: string;
  priority?: number | null;
  dueDate?: string | null;
}

export interface KeyResultCreateParams {
  objectiveId: string;
  title: string;
  targetValue?: number;
  unit?: string;
  trackingMode?: string;
}

export interface KeyResultUpdateParams {
  id: string;
  title?: string;
  description?: string | null;
  status?: string;
  dueDate?: string | null;
}

// ── MCP Settings ─────────────────────────────────────────────────────

export interface McpServerConfig {
  name: string;
  transport: "stdio" | "http";
  enabled: boolean;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  oauthProvider?: string;
  oauthConnected: boolean;
}

export interface McpConfigResponse {
  enabled: boolean;
  servers: McpServerConfig[];
}

export interface McpAddServerParams {
  name: string;
  transport: "stdio" | "http";
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
}

export interface McpToggleServerParams {
  name: string;
  enabled: boolean;
}

export interface RecommendedMcpServer {
  name: string;
  author: string;
  description: string;
  icon: string;
  transport: "stdio" | "http";
  command?: string;
  args?: string[];
  envKeys?: string[];
  url?: string;
  docsUrl?: string;
  oauthProvider?: string;
}

export interface OAuthStartParams {
  provider: string;
  serverName: string;
}

// ── App Info ─────────────────────────────────────────────────────────────

export interface AppInfoResponse {
  version: string;
  dataDir: string;
  setupCompleted: boolean;
}

// ── Timeline / Dashboard ──────────────────────────────────────────

export type TimelineSource =
  | "productivity"
  | "focus"
  | "task"
  | "todo"
  | "note"
  | "finance"
  | "system";

export type TimelineEntryType =
  | "appUsage"
  | "focusSession"
  | "taskTimeEntry"
  | "taskCreated"
  | "taskCompleted"
  | "taskUpdated"
  | "taskDue"
  | "noteCreated"
  | "noteUpdated"
  | "transactionRecorded"
  | "expenseRecorded"
  | "incomeRecorded"
  | "systemEvent";

export interface TimelineEntry {
  id: string;
  source: TimelineSource;
  entryType: TimelineEntryType;
  title: string;
  description?: string;
  startedAt: string;
  endedAt?: string;
  durationSecs?: number;
  entityId?: string;
  entityRoute?: string;
  color: string;
  metadata?: Record<string, unknown>;
}

export interface TopAppSummary {
  appName: string;
  durationSecs: number;
  percentage: number;
}

export interface SourceBreakdown {
  source: TimelineSource;
  durationSecs: number;
  count: number;
}

export interface TimelineSummary {
  totalTrackedSecs: number;
  focusSecs: number;
  tasksCompleted: number;
  tasksCreated: number;
  notesTouched: number;
  transactionsCount: number;
  topApps: TopAppSummary[];
  sourceBreakdown: SourceBreakdown[];
}

export interface TimelineResponse {
  entries: TimelineEntry[];
  summary: TimelineSummary;
}

export interface TimelineQuery {
  startDate: string;
  endDate: string;
  sources?: TimelineSource[];
  includePointEvents?: boolean;
}

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
