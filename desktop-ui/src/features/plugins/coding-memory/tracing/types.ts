export type {
  ContextMessage,
  ErrorByTool,
  HeaderStats,
  KimiTodo,
  ProjectTotals,
  ProviderInfo,
  Scope,
  SemanticCategory,
  SessionDetail,
  SessionState,
  SessionSummary,
  StatsBundle,
  SubagentSummary,
  SubagentTypeCount,
  TokenSeriesPoint,
  ToolUsage,
  TraceEvent,
} from "@/bindings";

export type ViewMode = "chart" | "events" | "timeline" | "decisions";
export type DetailTab = "wireEvents" | "contextMessages" | "state" | "dual" | "agents";
