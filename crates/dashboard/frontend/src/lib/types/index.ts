/**
 * Shared type definitions for the Klyntbot dashboard.
 * Mirrors the GraphQL schema types from the backend.
 */

// ─── Task / Todo ──────────────────────────────────────────────────────────────

export type TodoStatus = 'Todo' | 'Doing' | 'Done' | 'Archived';
export type Priority = 1 | 2 | 3 | 4;

export interface Attachment {
  id: string;
  type: 'file' | 'url' | 'note';
  title: string;
  path?: string;
  url?: string;
  note?: string;
}

export interface TimeEntry {
  id: string;
  minutes: number;
  note?: string;
  createdAt: string;
}

export interface Todo {
  id: string;
  title: string;
  description?: string;
  status: TodoStatus;
  priority: Priority;
  tags: string[];
  dueDate?: string;
  parentId?: string;
  projectId?: string;
  attachments: Attachment[];
  timeEntries: TimeEntry[];
  dependencies: string[];
  focusDeadline?: string;
  confidence?: number;
  createdAt: string;
  updatedAt: string;
}

// ─── Project ──────────────────────────────────────────────────────────────────

export type ProjectStatus = 'Active' | 'Paused' | 'Completed' | 'Archived';
export type ProjectColor = 'red' | 'orange' | 'yellow' | 'green' | 'blue' | 'purple' | 'gray';

export interface Project {
  id: string;
  name: string;
  description?: string;
  status: ProjectStatus;
  color: ProjectColor;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

// ─── Chat / Session ───────────────────────────────────────────────────────────

export interface Session {
  id: string;
  name: string;
  lastMessage?: string;
  lastMessageAt?: string;
  createdAt: string;
}

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: Date;
  toolCalls?: ToolCall[];
}

export interface ToolCall {
  id: string;
  name: string;
  inputs: object;
  outputs: object;
  status: 'running' | 'completed' | 'failed';
}

// ─── Notifications ────────────────────────────────────────────────────────────

export type NotificationType =
  | 'task'
  | 'plan'
  | 'calendar'
  | 'focus'
  | 'overdue'
  | 'digest';

export interface Notification {
  id: string;
  type: NotificationType | string;
  title: string;
  body: string;
  read: boolean;
  timestamp: Date;
  href?: string;
}

// ─── Settings / Config ────────────────────────────────────────────────────────

export interface Provider {
  name: string;
  apiKey: string;
  model: string;
  connected: boolean;
  availableModels: string[];
}

export interface EnrichmentConfig {
  enabled: boolean;
  autoApplyThreshold: number;
}

export interface SearchConfig {
  enabled: boolean;
  semanticThreshold: number;
  embeddingModel: string;
  rrfK: number;
}

export interface AppConfig {
  providers: Provider[];
  todo: {
    enrichment: EnrichmentConfig;
    search: SearchConfig;
  };
}

// ─── Plan ─────────────────────────────────────────────────────────────────────

export type PlanStatus =
  | 'Draft'
  | 'Approved'
  | 'Executing'
  | 'Completed'
  | 'Failed'
  | 'Abandoned';

export type StepStatus = 'Pending' | 'Running' | 'Completed' | 'Failed';

export interface PlanStep {
  id: string;
  description: string;
  status: StepStatus;
  result?: string;
  attemptCount: number;
}

export interface BacktrackEntry {
  id: string;
  fromStep: number;
  reason: string;
  regeneratedSteps: number;
  timestamp: string;
}

export interface Plan {
  id: string;
  title: string;
  description?: string;
  status: PlanStatus;
  steps: PlanStep[];
  backtrackHistory: BacktrackEntry[];
  createdAt: string;
  updatedAt: string;
  completedAt?: string;
}
