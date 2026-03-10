// ── Work Context Types ──────────────────────────────────────

export type WorkContextStatus = "active" | "paused" | "completed" | "archived";
export type WorkContextType =
  | "coding"
  | "research"
  | "communication"
  | "planning"
  | "review"
  | "meeting"
  | "learning"
  | "general";

export interface WorkContext {
  id: string;
  title: string;
  description?: string;
  status: WorkContextStatus;
  contextType: WorkContextType;
  linkedProjectId?: string;
  color?: string;
  tags: string[];
  confidence: number;
  firstSeenAt: string;
  lastActiveAt: string;
  totalDurationSecs: number;
  eventCount: number;
  topResources: WorkResource[];
}

export interface WorkResource {
  id: string;
  resourceType: "file" | "url" | "repo" | "note" | "conversation" | "app" | "command";
  resourceName: string;
  resourcePath?: string;
  resourceUri?: string;
  accessCount: number;
  relevanceScore?: number;
}

export interface WorkContextDetail {
  context: WorkContext;
  resources: WorkResource[];
  linkedActionIds: string[];
  recentEvents: ActivityEvent[];
}

export interface ActivityEvent {
  id: string;
  timestamp: string;
  source: string;
  actor: string;
  resourceName?: string;
  action: string;
  contentPreview?: string;
  appName?: string;
  durationSecs?: number;
}

export interface ContextTimelineBlock {
  contextId?: string;
  contextTitle?: string;
  contextColor?: string;
  contextType?: string;
  startTime: string;
  endTime: string;
  eventCount: number;
  isIdle: boolean;
}

export interface ContextResumeData {
  contextId: string;
  contextTitle: string;
  summary: string;
  suggestedPrompt: string;
  recentResources: string[];
}

export interface WorkContextUpdateParams {
  id: string;
  title?: string;
  color?: string;
  status?: string;
  linkedProjectId?: string;
}
