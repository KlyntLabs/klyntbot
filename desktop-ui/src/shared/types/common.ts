// ── Error & Response Types ──────────────────────────────────

export interface ApiError {
  code: string;
  message: string;
}

// ── Pagination ──────────────────────────────────────────────

export interface Pagination {
  page: number;
  pageSize: number;
  total: number;
}

// ── Status Workflows ────────────────────────────────────────

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

// ── Column Types ───────────────────────────────────────────

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

// ── UI Navigation ──────────────────────────────────────────

export type Tab = "All" | string;
export type SidebarItem =
  | "Dashboard"
  | "Chat"
  | "Tasks"
  | "OKR"
  | "Calendar"
  | "Notes"
  | "Learn"
  | "Finance"
  | "Automations"
  | "System"
  | "Settings";
export type ViewMode = "table" | "board" | "tree";

// ── Generic Launcher ─────────────────────────────────────────

export interface LauncherItem {
  id: string;
  title: string;
  subtitle: string;
  icon: string;
  shortcut: string;
}

// ── Agent & Plan Data (re-exported for chat) ────────────────

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

// ── Timeline Types ──────────────────────────────────────────

export type TimelineSource =
  | "productivity"
  | "focus"
  | "task"
  | "todo"
  | "note"
  | "finance"
  | "system"
  | "calendar";

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
  | "systemEvent"
  | "calendarEvent"
  | "taskStatusChanged"
  | "taskPriorityChanged"
  | "taskFieldUpdated";

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
  focusSessionId?: string;
  metadata?: {
    focusSessionId?: string;
    inFocusMode?: boolean;
    [key: string]: unknown;
  };
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

// ── Cron / Automations ──────────────────────────────────────

export type CronOrigin = "system" | "user" | "ai" | "plugin";

export type CronSchedule =
  | { kind: "at"; atMs: number }
  | { kind: "every"; everyMs: number }
  | { kind: "cron"; expr: string; tz?: string };

export interface CronPayload {
  kind: string;
  message: string;
  deliver: boolean;
  channel?: string;
  to?: string;
}

export interface CronJobState {
  nextRunAtMs?: number;
  lastRunAtMs?: number;
  lastStatus?: string;
  lastError?: string;
}

export interface CronJob {
  id: string;
  name: string;
  enabled: boolean;
  origin: CronOrigin;
  schedule: CronSchedule;
  payload: CronPayload;
  state: CronJobState;
  createdAtMs: number;
  updatedAtMs: number;
  deleteAfterRun: boolean;
}

export interface CronJobCreateParams {
  name: string;
  schedule: CronSchedule;
  message: string;
  deliver?: boolean;
  channel?: string;
  to?: string;
  deleteAfterRun?: boolean;
}

export interface CronJobUpdateParams {
  id: string;
  name?: string;
  schedule?: CronSchedule;
  message?: string;
  deliver?: boolean;
  channel?: string | null;
  to?: string | null;
}

export interface CronStatusResponse {
  enabled: boolean;
  jobs: number;
  nextWakeAtMs?: number;
}

// ── App Info ────────────────────────────────────────────────

export interface AppInfoResponse {
  version: string;
  dataDir: string;
  setupCompleted: boolean;
}
