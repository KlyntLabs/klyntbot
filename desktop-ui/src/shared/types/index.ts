// ── Common Types ──────────────────────────────────────────

export type {
  ApiError,
  Pagination,
  ColumnType,
  StatusGroup,
  StatusLabel,
  StatusWorkflow,
  Tab,
  SidebarItem,
  ViewMode,
  LauncherItem,
  TimelineSource,
  TimelineEntryType,
  TimelineEntry,
  TopAppSummary,
  SourceBreakdown,
  TimelineSummary,
  TimelineResponse,
  TimelineQuery,
  CronOrigin,
  CronSchedule,
  CronPayload,
  CronJobState,
  CronJob,
  CronJobCreateParams,
  CronJobUpdateParams,
  CronStatusResponse,
  AppInfoResponse,
  DelegationInfo,
  PlanData,
} from "./common";

export { EMPTY_TIMELINE_RESPONSE } from "./common";

// ── Chat Types ────────────────────────────────────────────

export type {
  MessageSegment,
  ChatMessage,
  ChatThread,
  SessionContext,
  ContentChunkPayload,
  ToolStartPayload,
  ToolEndPayload,
  AgentDonePayload,
  AgentErrorPayload,
  ClassificationCompletePayload,
  ExecutionStartedPayload,
  IterationStartPayload,
  UsageReportPayload,
  MemoryAccessPayload,
  SkillLoadedPayload,
  LearningEventPayload,
  SubagentSpawnedPayload,
  AgentSelectedPayload,
  PlanGeneratedPayload,
  PlanStepCompletedPayload,
  DelegationStartedPayload,
  DelegationCompletedPayload,
  TransparencyData,
  ActiveInteraction,
  InteractionRequestPayload,
  InteractionRequest,
  Question,
  AnswerType,
  AnswerOption,
  AnswerValue,
  Answer,
  FormResponse,
  AgentStatus,
} from "./chat";

// ── Task Types ────────────────────────────────────────────

export type {
  Task,
  TodayTask,
  TaskGroup,
  Project,
  Objective,
  KeyResult,
  Area,
  CustomColumn,
  CustomColumnValue,
  TaskUpdateParams,
  TaskCreateParams,
  AreaCreateParams,
  AreaUpdateParams,
  ProjectCreateParams,
  ProjectUpdateParams,
  ObjectiveCreateParams,
  ObjectiveUpdateParams,
  KeyResultCreateParams,
  KeyResultUpdateParams,
  ColumnCreateParams,
  ColumnUpdateParams,
  ColumnReorderParams,
  ColumnValueSetParams,
} from "./tasks";

// ── Finance Types ─────────────────────────────────────────

export type {
  FinanceAccount,
  FinanceTransaction,
  FinanceBudgetUsage,
  FinancePortfolio,
  FinanceInvestment,
  FinanceGoal,
  FinanceLiability,
  FinanceNetWorth,
  FinanceCategoryReport,
  FinanceTrendPoint,
  FinanceAccountCreateParams,
  FinanceTransactionCreateParams,
  FinanceBudgetCreateParams,
  FinanceGoalCreateParams,
  FinanceLiabilityCreateParams,
  FinancePortfolioCreateParams,
  FinanceInvestmentCreateParams,
} from "./finance";

// ── Productivity Types ────────────────────────────────────

export type {
  ProductivitySummary,
  ProjectUsage,
  AppUsage,
  CategoryUsage,
  TrackedApp,
  ProductivityProject,
  FocusSession,
  ActivityTimeline,
  CategoryRules,
  ActivityCategory,
  GoalProgress,
  TimeEntry,
  LearnedRule,
  ActivitySwitchPayload,
  AutoFocusPayload,
  ScorePayload,
  FocusStatePayload,
  InsightCard,
  InsightPayload,
  FocusTimerStatus,
  FocusTickPayload,
  FocusCompletedPayload,
  WeeklyAssessment,
  ProductivityPeriod,
  GoalCreateParams,
} from "./productivity";

// ── Notes Types ───────────────────────────────────────────

export type {
  Note,
  NoteVersion,
  NoteLink,
  Notebook,
  NoteCreateParams,
  NoteUpdateParams,
  NotebookCreateParams,
} from "./notes";

// ── Dashboard Types ───────────────────────────────────────

export type { CalendarEvent } from "./dashboard";

// ── Config Types ──────────────────────────────────────────

export type {
  McpServerConfig,
  McpConfigResponse,
  McpAddServerParams,
  McpToggleServerParams,
  RecommendedMcpServer,
  OAuthStartParams,
} from "./config";

// ── Agent Types ───────────────────────────────────────────

export type { CoachingIntervention } from "./agent";
