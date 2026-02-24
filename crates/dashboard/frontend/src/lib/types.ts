/**
 * TypeScript interfaces matching Rust row types.
 * All types use camelCase to match `#[serde(rename_all = "camelCase")]`.
 * Monetary values are integer cents (i64 in Rust → number in TS).
 * Dates are ISO 8601 strings from Rust's DateTime<Utc>.
 */

// ── Task (TodoRow) ────────────────────────────────────────────────────────────

export interface Task {
  id: string;
  title: string;
  description: string | null;
  priority: number | null;
  dueDate: string | null;
  tags: string[];
  status: string;
  focusedAt: string | null;
  focusDeadline: string | null;
  focusExpiredCount: number;
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
  parentId: string | null;
  projectId: string | null;
  totalTrackedSecs: number;
  estimatedMinutes: number | null;
  calendarEventUid: string | null;
  lastRemindedAt: string | null;
  recurrenceRule: string | null;
  recurrenceParentId: string | null;
  isTemplate: boolean;
  nextInstanceDate: string | null;
}

// ── Project (ProjectRow) ──────────────────────────────────────────────────────

export interface Project {
  id: string;
  name: string;
  description: string | null;
  color: string;
  tags: string[];
  status: string;
  createdAt: string;
  updatedAt: string;
}

// ── Plan (PlanRow + PlanStepRow) ──────────────────────────────────────────────

export interface PlanStep {
  id: string;
  planId: string;
  stepIndex: number;
  description: string;
  reasoning: string;
  expectedTools: string[];
  status: string;
  attemptCount: number;
  maxAttempts: number;
  result: string | null;
  startedAt: string | null;
  completedAt: string | null;
}

export interface Plan {
  id: string;
  sessionKey: string;
  goalId: string | null;
  title: string;
  description: string;
  status: string;
  currentStepIndex: number;
  iterationLimit: number;
  backtrackHistory: unknown;
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
  steps?: PlanStep[];
}

// ── Session ───────────────────────────────────────────────────────────────────

export interface Session {
  key: string;
  metadata: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface SessionWithCount extends Session {
  messageCount: number;
}

export interface SessionMessage {
  id: string;
  sessionKey: string;
  role: string;
  content: string;
  timestamp: string;
  requestId: string | null;
  toolCalls: unknown | null;
  metadata: unknown | null;
}

// ── Cron Job (CronJobRow) ─────────────────────────────────────────────────────

export interface CronJob {
  id: string;
  name: string;
  enabled: boolean;
  schedule: unknown;
  payload: unknown;
  nextRunAtMs: number | null;
  lastRunAtMs: number | null;
  lastStatus: string | null;
  lastError: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  deleteAfterRun: boolean;
}

// ── Finance ───────────────────────────────────────────────────────────────────

export interface FinanceAccount {
  id: string;
  name: string;
  accountType: string;
  currency: string;
  balance: number;
  institution: string | null;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface FinanceTransaction {
  id: string;
  accountId: string;
  txType: string;
  amount: number;
  currency: string;
  category: string | null;
  subcategory: string | null;
  counterparty: string | null;
  notes: string | null;
  txDate: string;
  createdAt: string;
  updatedAt: string;
}

export interface FinanceBudget {
  id: string;
  name: string;
  amount: number;
  currency: string;
  period: string;
  category: string | null;
  spent: number;
  createdAt: string;
  updatedAt: string;
}

export interface FinanceGoal {
  id: string;
  name: string;
  targetAmount: number;
  currentAmount: number;
  currency: string;
  targetDate: string | null;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
}

// ── Skill ─────────────────────────────────────────────────────────────────────

export interface Skill {
  name: string;
  description: string;
  source: 'builtin' | 'workspace';
  enabled: boolean;
}

// ── Status ────────────────────────────────────────────────────────────────────

export interface AgentStatus {
  model: string;
  uptime: number;
  version: string;
  storageStats: {
    taskCount: number;
    sessionCount: number;
    planCount: number;
  };
}

// ── WebSocket events ──────────────────────────────────────────────────────────

export interface AgentEvent {
  type: string;
}

export interface ContentChunkEvent extends AgentEvent {
  type: 'contentChunk';
  data: string;
}

export interface DoneEvent extends AgentEvent {
  type: 'done';
  content: string;
}

export interface ErrorEvent extends AgentEvent {
  type: 'error';
  message: string;
}

export interface ClassificationCompleteEvent extends AgentEvent {
  type: 'classificationComplete';
  strategy: string;
  confidence: number;
  source: string;
}

export interface ContextAssembledEvent extends AgentEvent {
  type: 'contextAssembled';
  totalTokens: number;
  budget: number;
}

export interface ExecutionStartedEvent extends AgentEvent {
  type: 'executionStarted';
  engine: string;
  maxIterations: number;
}

export interface IterationStartEvent extends AgentEvent {
  type: 'iterationStart';
  iteration: number;
  max: number;
}

export interface ToolStartEvent extends AgentEvent {
  type: 'toolStart';
  name: string;
  args: Record<string, unknown>;
}

export interface ToolEndEvent extends AgentEvent {
  type: 'toolEnd';
  name: string;
  success: boolean;
  durationMs: number;
}

export interface InteractionRequestEvent extends AgentEvent {
  type: 'interaction.request';
  requestId: string;
  title: string;
  questions: InteractionQuestion[];
}

export type InteractionQuestion =
  | { type: 'singleSelect'; label: string; options: string[] }
  | { type: 'multiSelect'; label: string; options: string[] }
  | { type: 'yesNo'; label: string; default?: boolean }
  | { type: 'freeText'; label: string; placeholder?: string };

// ── Chat message (UI) ─────────────────────────────────────────────────────────

export type MessageRole = 'user' | 'assistant' | 'system';

export interface ChatMessage {
  id: string;
  role: MessageRole;
  content: string;
  timestamp: Date;
  isStreaming?: boolean;
}
