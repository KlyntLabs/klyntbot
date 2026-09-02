// ── Common Types ──────────────────────────────────────────

export type {
  ApiError,
  AppInfoResponse,
  ColumnType,
  CronJob,
  CronJobCreateParams,
  CronJobState,
  CronJobUpdateParams,
  CronOrigin,
  CronPayload,
  CronSchedule,
  CronStatusResponse,
  DelegationInfo,
  LauncherItem,
  Pagination,
  PlanData,
  SidebarItem,
  SourceBreakdown,
  StatusGroup,
  StatusLabel,
  StatusWorkflow,
  Tab,
  TimelineEntry,
  TimelineEntryType,
  TimelineQuery,
  TimelineResponse,
  TimelineSource,
  TimelineSummary,
  TopAppSummary,
  ViewMode,
} from "./common";

export { EMPTY_TIMELINE_RESPONSE } from "./common";

// ── Chat Types ────────────────────────────────────────────

export type {
  ActiveInteraction,
  AgentDonePayload,
  AgentErrorPayload,
  AgentSelectedPayload,
  AgentStatus,
  Answer,
  AnswerOption,
  AnswerType,
  AnswerValue,
  ChatMessage,
  ChatThread,
  ClassificationCompletePayload,
  ConsensusReachedPayload,
  ContentChunkPayload,
  DebateJudgeDecisionPayload,
  DebatePhase,
  DebateRound,
  DebateRoundCompletedPayload,
  DebateRoundStartedPayload,
  DelegationCompletedPayload,
  DelegationStartedPayload,
  EnhancementStage,
  ExecutionStartedPayload,
  FormResponse,
  InteractionRequest,
  InteractionRequestPayload,
  IterationStartPayload,
  JudgeDecisionEntry,
  LearningEventPayload,
  MemoryAccessPayload,
  MemoryPromotedPayload,
  MessageSegment,
  PersonaPerspectivePayload,
  PersonaSegment,
  PlanGeneratedPayload,
  PlanStepCompletedPayload,
  Question,
  RetrievalEnhancedPayload,
  SessionContext,
  SkillLoadedPayload,
  SubagentSpawnedPayload,
  ToolEndPayload,
  ToolStartPayload,
  TransparencyData,
  UsageReportPayload,
} from "./chat";

// ── Task Types ────────────────────────────────────────────

export type {
  Area,
  AreaCreateParams,
  AreaUpdateParams,
  ColumnCreateParams,
  ColumnReorderParams,
  ColumnUpdateParams,
  ColumnValueSetParams,
  CustomColumn,
  CustomColumnValue,
  KeyResult,
  KeyResultCreateParams,
  KeyResultUpdateParams,
  Objective,
  ObjectiveCreateParams,
  ObjectiveUpdateParams,
  Project,
  ProjectCreateParams,
  ProjectUpdateParams,
  Task,
  TaskCreateParams,
  TaskGroup,
  TaskUpdateParams,
  TodayTask,
} from "./tasks";

// ── Productivity Types ────────────────────────────────────

export type {
  ActivityCategory,
  ActivitySwitchPayload,
  ActivityTimeline,
  AppUsage,
  CategoryRules,
  CategoryUsage,
  FocusDndUnavailablePayload,
  FocusSession,
  FocusSessionStatus,
  FocusStatePayload,
  FocusSyncPayload,
  FocusWarningPayload,
  GoalCreateParams,
  GoalProgress,
  InsightCard,
  InsightPayload,
  IntelligenceSession,
  LearnedRule,
  ProductivityProject,
  ProductivitySummary,
  ProjectUsage,
  ScorePayload,
  TimeEntry,
  TrackedApp,
  WeeklyAssessment,
} from "./productivity";

// ── Notes Types ───────────────────────────────────────────

export type {
  AgentFileContent,
  AgentFileSummary,
  AgentProfileSummary,
  InboxItem,
  Note,
  Notebook,
  NotebookCreateParams,
  NoteCreateParams,
  NoteImportResult,
  NoteLink,
  NoteListItem,
  NoteUpdateParams,
  NoteVersion,
  WorkspaceFile,
  WorkspaceFileContent,
} from "./notes";

// ── Dashboard Types ───────────────────────────────────────

export type { CalendarEvent } from "./dashboard";

// ── Config Types ──────────────────────────────────────────

export type {
  EmbeddedMcpRejection,
  EmbeddedMcpState,
  EmbeddedMcpStatusResponse,
  McpAddServerParams,
  McpConfigResponse,
  McpServerConfig,
  McpToggleServerParams,
  OAuthStartParams,
  RecommendedMcpServer,
} from "./config";

// ── Entity Link Types ─────────────────────────────────────

export type {
  ActionSummary,
  EntityLink,
  EntityLinkCreateParams,
  KeyResultSummary,
  LinkedEntities,
  NoteSummary,
  ObjectiveSummary,
  ProjectSource,
  ProjectSourceCreateParams,
  SessionSummary,
} from "./entity-links";

// ── Tree Path Types ──────────────────────────────────────

export type { PathSegment, TreePathRef } from "./tree-path";

// ── Agent Types ───────────────────────────────────────────

export type { CoachingIntervention } from "./agent";

// ── Work Context Types ────────────────────────────────────

export type {
  ActivityEvent,
  ContextResumeData,
  ContextTimelineBlock,
  WorkContext,
  WorkContextDetail,
  WorkContextStatus,
  WorkContextType,
  WorkContextUpdateParams,
  WorkResource,
} from "./workContexts";

// ── Fabric Graph Types ────────────────────────────────────

export type {
  FabricCommunity,
  FabricCommunityDetail,
  FabricEntity,
  FabricEntityEdge,
  FabricGraphBase,
  FabricGraphEvent,
  FabricLayer,
  FabricLink,
  FabricMember,
  FabricNote,
  FabricTreeNode,
} from "./fabric";
