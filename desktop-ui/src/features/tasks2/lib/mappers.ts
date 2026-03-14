import type { TimelineEntry } from "@shared/types/common";
import type { Area, Project, Task } from "@shared/types/tasks";
import { Folder, type LucideProps } from "lucide-react";
import type React from "react";
import { type Priority, priorities } from "./priority-icons";
import { status as allStatusDefs, BacklogIcon, type Status } from "./status-icons";

// ── Display types ─────────────────────────────────────────

export interface DisplayProject {
  id: string;
  name: string;
  color: string;
  icon: React.FC<LucideProps>;
}

export interface LabelInterface {
  id: string;
  name: string;
  color: string;
}

export type { Status, Priority };

export type TaskState = "new" | "focused" | "has-history" | "completed";
export type ActorType = "user" | "agent" | "system";
export type SuggestionStatus = "pending" | "applied" | "dismissed";

export interface DetailTask {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  priority: Priority;
  labels: LabelInterface[];
  project: DisplayProject | null;
  area: { id: string; name: string } | null;
  tags: string[];
  dueDate: string | null;
  energyLevel: string | null;
  taskType: string;
  estimatedMinutes: number | null;
  actualMinutes: number | null;
  totalTrackedSecs: number;
  focusedAt: string | null;
  acceptanceCriteria: string | null;
  completed: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ActivityEntry {
  id: string;
  actorType: ActorType;
  actorName: string;
  action: string;
  detail: string | null;
  createdAt: string;
}

export interface Suggestion {
  id: string;
  title: string;
  description: string;
  confidence: number;
  status: SuggestionStatus;
}

export interface SubIssue {
  id: string;
  identifier: string;
  title: string;
  status: Status;
  priority: Priority;
  completed: boolean;
}

export interface TaskMemory {
  lastSessionSummary: string;
  continuityNote: string;
  relatedFacts: string[];
}

export interface FocusSession {
  focusMode: string;
  qualityScore: number;
  distractionCount: number;
  flowState: string;
  qualityHistory: number[];
}

/** The Issue shape consumed by list/board components. */
export interface Issue {
  id: string;
  identifier: string;
  title: string;
  description: string;
  status: Status;
  assignee: null;
  priority: Priority;
  labels: LabelInterface[];
  createdAt: string;
  cycleId: string;
  project?: DisplayProject;
  subissues?: string[];
  rank: string;
  dueDate?: string;
}

// ── Status resolution (option C) ──────────────────────────

const KNOWN_STATUS_MAP: Record<string, Status> = {};
for (const s of allStatusDefs) {
  KNOWN_STATUS_MAP[s.name.toLowerCase()] = s;
}
const STATUS_ALIASES: Record<string, string> = {
  "to do": "todo",
  "on hold": "paused",
  "in review": "technical review",
  done: "completed",
};

export function resolveStatus(task: Task): Status {
  if (task.statusLabel) {
    const name = task.statusLabel.name.toLowerCase();
    const aliased = STATUS_ALIASES[name] ?? name;
    const match = KNOWN_STATUS_MAP[aliased];
    if (match) return match;

    const found = allStatusDefs.find(
      (s) => s.color.toLowerCase() === task.statusLabel!.color.toLowerCase(),
    );
    if (found) return { ...found, name: task.statusLabel.name };

    return {
      id: task.statusLabel.id,
      name: task.statusLabel.name,
      color: task.statusLabel.color,
      icon: BacklogIcon,
    };
  }

  switch (task.status) {
    case "completed":
      return allStatusDefs.find((s) => s.id === "completed") ?? allStatusDefs[0];
    case "in_progress":
      return allStatusDefs.find((s) => s.id === "in-progress") ?? allStatusDefs[0];
    case "open":
      return allStatusDefs.find((s) => s.id === "to-do") ?? allStatusDefs[0];
    default:
      return allStatusDefs.find((s) => s.id === "backlog") ?? allStatusDefs[0];
  }
}

// ── Priority resolution ───────────────────────────────────

const PRIORITY_MAP: Record<string, Priority> = {
  "": priorities[0],
  P0: priorities[0],
  P1: priorities[1],
  P2: priorities[2],
  P3: priorities[3],
  P4: priorities[4],
};

export function resolvePriority(task: Task): Priority {
  if (!task.priority) return priorities[0];
  return PRIORITY_MAP[task.priority] ?? priorities[0];
}

export function priorityToNumber(priorityId: string): number | null {
  switch (priorityId) {
    case "urgent":
      return 1;
    case "high":
      return 2;
    case "medium":
      return 3;
    case "low":
      return 4;
    default:
      return null;
  }
}

// ── Tags → Labels ─────────────────────────────────────────

const TAG_COLORS = [
  "purple",
  "red",
  "green",
  "blue",
  "yellow",
  "orange",
  "pink",
  "gray",
  "indigo",
  "teal",
  "cyan",
];

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash * 31 + str.charCodeAt(i)) | 0;
  }
  return Math.abs(hash);
}

export function tagsToLabels(tags: string[]): LabelInterface[] {
  return tags.map((tag) => ({
    id: tag,
    name: tag,
    color: TAG_COLORS[hashString(tag) % TAG_COLORS.length],
  }));
}

// ── Short ID ──────────────────────────────────────────────

export function shortId(id: string): string {
  const hex = id.replace(/-/g, "").slice(0, 4);
  return `T-${hex}`;
}

// ── Project mapping ───────────────────────────────────────

export function projectToDisplayProject(project: Project): DisplayProject {
  return {
    id: project.id,
    name: project.name,
    color: project.color,
    icon: Folder,
  };
}

// ── Task → Issue ──────────────────────────────────────────

export function taskToIssue(task: Task, projectMap: Map<string, DisplayProject>): Issue {
  return {
    id: task.id,
    identifier: shortId(task.id),
    title: task.title,
    description: task.description ?? "",
    status: resolveStatus(task),
    assignee: null,
    priority: resolvePriority(task),
    labels: tagsToLabels(task.tags),
    createdAt: task.createdAt ?? "",
    cycleId: "",
    project: task.projectId ? projectMap.get(task.projectId) : undefined,
    subissues: task.subtaskCount > 0 ? Array(task.subtaskCount).fill("") : undefined,
    rank: "",
    dueDate: task.dueDate ?? undefined,
  };
}

// ── Task → DetailTask ─────────────────────────────────────

export function taskToDetailTask(
  task: Task,
  projectMap: Map<string, DisplayProject>,
  areaMap: Map<string, Area>,
): DetailTask {
  const area = areaMap.get(task.areaId);
  return {
    id: task.id,
    identifier: shortId(task.id),
    title: task.title,
    description: task.description ?? "",
    status: resolveStatus(task),
    priority: resolvePriority(task),
    labels: tagsToLabels(task.tags),
    project: task.projectId ? (projectMap.get(task.projectId) ?? null) : null,
    area: area ? { id: area.id, name: area.name } : null,
    tags: task.tags,
    dueDate: task.dueDate ?? null,
    energyLevel: task.energyLevel ?? null,
    taskType: task.taskType ?? "manual",
    estimatedMinutes: task.estimatedMinutes ?? null,
    actualMinutes: task.actualMinutes ?? null,
    totalTrackedSecs: task.totalTrackedSecs ?? 0,
    focusedAt: task.focusedAt ?? null,
    acceptanceCriteria: task.acceptanceCriteria ?? null,
    completed: task.completed,
    createdAt: task.createdAt ?? "",
    updatedAt: task.updatedAt ?? "",
  };
}

// ── Child Task → SubIssue ─────────────────────────────────

export function taskToSubIssue(task: Task): SubIssue {
  return {
    id: task.id,
    identifier: shortId(task.id),
    title: task.title,
    status: resolveStatus(task),
    priority: resolvePriority(task),
    completed: task.completed,
  };
}

// ── Derive task state ─────────────────────────────────────

export function deriveTaskState(task: DetailTask): TaskState {
  if (task.completed) return "completed";
  if (task.focusedAt) return "focused";
  if (task.totalTrackedSecs > 0) return "has-history";
  return "new";
}

// ── Timeline → Activity ───────────────────────────────────

const ENTRY_TYPE_ACTIONS: Record<string, string> = {
  taskCreated: "created task",
  taskCompleted: "completed task",
  taskUpdated: "updated task",
  taskDue: "task due",
  taskTimeEntry: "tracked time",
  focusSession: "focus session",
};

export function timelineToActivity(entry: TimelineEntry): ActivityEntry {
  return {
    id: entry.id,
    actorType: "system",
    actorName: "System",
    action: ENTRY_TYPE_ACTIONS[entry.entryType] ?? entry.entryType,
    detail: entry.description ?? entry.title,
    createdAt: entry.startedAt,
  };
}

// ── Derive focus session ──────────────────────────────────

export function deriveFocusSession(task: DetailTask): FocusSession | null {
  if (!task.focusedAt) return null;
  return {
    focusMode: "focus",
    qualityScore: 0.7,
    distractionCount: 0,
    flowState: "building",
    qualityHistory: [0.5, 0.6, 0.7],
  };
}

// ── Filter logic (moved from issues-store) ────────────────

interface FilterState {
  status: string[];
  assignee: string[];
  priority: string[];
  labels: string[];
  project: string[];
}

export function filterIssues(issues: Issue[], filters: FilterState): Issue[] {
  let result = issues;

  if (filters.status.length > 0) {
    result = result.filter((issue) => filters.status.includes(issue.status.id));
  }
  if (filters.priority.length > 0) {
    result = result.filter((issue) => filters.priority.includes(issue.priority.id));
  }
  if (filters.labels.length > 0) {
    result = result.filter((issue) => issue.labels.some((l) => filters.labels.includes(l.id)));
  }
  if (filters.project.length > 0) {
    result = result.filter((issue) => issue.project && filters.project.includes(issue.project.id));
  }

  return result;
}

export function searchIssues(issues: Issue[], query: string): Issue[] {
  const q = query.toLowerCase().trim();
  if (!q) return issues;
  return issues.filter(
    (issue) =>
      issue.title.toLowerCase().includes(q) ||
      issue.description.toLowerCase().includes(q) ||
      issue.identifier.toLowerCase().includes(q),
  );
}

// ── Group by status ───────────────────────────────────────

export function groupIssuesByStatus(issueList: Issue[]): Record<string, Issue[]> {
  return issueList.reduce(
    (acc, issue) => {
      const statusId = issue.status.id;
      if (!acc[statusId]) acc[statusId] = [];
      acc[statusId].push(issue);
      return acc;
    },
    {} as Record<string, Issue[]>,
  );
}

export function sortIssuesByPriority(issueList: Issue[]): Issue[] {
  const order: Record<string, number> = {
    urgent: 0,
    high: 1,
    medium: 2,
    low: 3,
    "no-priority": 4,
  };
  return [...issueList].sort((a, b) => (order[a.priority.id] ?? 99) - (order[b.priority.id] ?? 99));
}
