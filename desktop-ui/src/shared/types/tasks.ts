import type { ColumnType, StatusLabel } from "./common";

// ── Task Core Types ─────────────────────────────────────────

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
  taskType?: "manual" | "agentic" | "hybrid";
  executionState?: "idle" | "queued" | "running" | "paused" | "completed" | "failed";
  energyLevel?: "low" | "medium" | "high" | "deep";
  acceptanceCriteria?: string;
  estimatedMinutes?: number;
  actualMinutes?: number;
  complexityScore?: number;
  totalTrackedSecs?: number;
  focusedAt?: string;
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

// ── Project Types ───────────────────────────────────────────

export interface Project {
  id: string;
  name: string;
  color: string;
  areaId: string;
  taskCount: number;
  completedCount: number;
  objectiveIds?: string[];
  workflowId?: string;
  description?: string;
  instructions?: {
    context?: string;
    guidelines?: string;
    constraints?: string;
    persona?: string;
  };
  aiPersonality?: string;
  userRole?: string;
  startDate?: string;
  targetEndDate?: string;
  settings?: Record<string, unknown>;
}

// ── Objective & Key Results ─────────────────────────────────

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

// ── Area ─────────────────────────────────────────────────────

export interface Area {
  id: string;
  name: string;
  color: string;
  icon: string | null;
  projectCount: number;
  taskCount: number;
}

// ── Custom Columns ──────────────────────────────────────────

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

// ── Task Mutation Parameters ────────────────────────────────

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
  taskType?: "manual" | "agentic" | "hybrid";
  acceptanceCriteria?: string | null;
  energyLevel?: "low" | "medium" | "high" | "deep";
  executionState?: "idle" | "queued" | "running" | "paused" | "completed" | "failed";
  estimatedMinutes?: number | null;
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
  taskType?: "manual" | "agentic" | "hybrid";
  acceptanceCriteria?: string;
  energyLevel?: "low" | "medium" | "high" | "deep";
  estimatedMinutes?: number;
}

// ── Area Mutation Parameters ────────────────────────────────

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

// ── Project Mutation Parameters ─────────────────────────────

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
  workflowId?: string | null;
  instructions?: Record<string, unknown>;
  aiPersonality?: string | null;
  userRole?: string | null;
  startDate?: string | null;
  targetEndDate?: string | null;
  settings?: Record<string, unknown>;
}

// ── Objective Mutation Parameters ───────────────────────────

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

// ── Key Result Mutation Parameters ──────────────────────────

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

// ── Custom Column Mutation Parameters ───────────────────────

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
