export interface TrackedSession {
  sessionId: string;
  projectPath: string;
  projectName: string;
  jsonlPath: string;
  status: "active" | "idle" | "completed";
  firstMessagePreview: string | null;
  messageCount: number;
  gitBranch: string | null;
  lastActivity: string | null;
  createdAt: string;
}

export interface SessionMessage {
  type: "user" | "assistant" | "system" | "progress" | "queueOperation";
  uuid?: string;
  timestamp: string;
  text?: string;
  content?: ContentBlock[] | string;
  isMeta?: boolean;
  subtype?: string;
  data?: unknown;
  operation?: string;
  toolUseId?: string;
}

export interface ContentBlock {
  type: "text" | "tool_use" | "tool_result";
  text?: string;
  id?: string;
  name?: string;
  input?: unknown;
  toolUseId?: string;
  content?: unknown;
  isError?: boolean;
}

export interface PinnedMessage {
  id: number;
  sessionId: string;
  messageUuid: string;
  messageContent: string;
  messageRole: string;
  pinOrder: number;
  createdAt: string;
}

export interface BrainstormConversation {
  id: string;
  sessionId: string;
  title: string | null;
  mode: "directModel" | "agent";
  modelKey: string | null;
  agentProfile: string | null;
  createdAt: string;
  updatedAt: string | null;
}

export interface BrainstormMessage {
  id: string;
  conversationId: string;
  role: "user" | "assistant";
  content: string;
  isResultBlock: boolean;
  editedContent: string | null;
  sentToCc: boolean;
  createdAt: string;
}

export interface ClaudeSessionContext {
  rollingSummary: string;
  pinnedMessages: PinnedMessage[];
  recentMessages: SessionMessage[];
  totalMessages: number;
  estimatedTokens: number;
}

const STATUS_COLORS: Record<TrackedSession["status"], string> = {
  active: "text-success",
  idle: "text-brand",
  completed: "text-dim",
};

const STATUS_BG_COLORS: Record<TrackedSession["status"], string> = {
  active: "bg-success",
  idle: "bg-brand",
  completed: "bg-white/20",
};

export function sessionStatusColor(status: TrackedSession["status"]): string {
  return STATUS_COLORS[status];
}

export function sessionStatusBgColor(status: TrackedSession["status"]): string {
  return STATUS_BG_COLORS[status];
}
