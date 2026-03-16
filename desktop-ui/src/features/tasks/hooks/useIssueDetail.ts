import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import type { TimelineResponse } from "@shared/types/common";
import type { Area, Task, TaskUpdateParams } from "@shared/types/tasks";
import { useCallback, useMemo } from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import {
  type ActivityEntry,
  buildFocusSession,
  type DetailTask,
  type DisplayProject,
  deriveTaskState,
  type FocusSession,
  priorityToNumber,
  type SubIssue,
  type Suggestion,
  statusToMutationParams,
  type TaskMemory,
  type TaskState,
  taskToDetailTask,
  taskToSubIssue,
  timelineToActivity,
} from "../lib/mappers";

export type {
  DetailTask,
  TaskState,
  SubIssue,
  Suggestion,
  FocusSession,
  TaskMemory,
  ActivityEntry,
};

const PLACEHOLDER_TASK: DetailTask = {
  id: "",
  identifier: "",
  title: "Loading...",
  description: "",
  status: {
    id: "backlog",
    name: "Backlog",
    color: "#94a3b8",
    icon: () => null,
    backendStatus: "backlog",
  },
  priority: { id: "no-priority", name: "No priority", icon: () => null },
  labels: [],
  project: null,
  area: null,
  tags: [],
  dueDate: null,
  energyLevel: null,
  taskType: "manual",
  estimatedMinutes: null,
  actualMinutes: null,
  totalTrackedSecs: 0,
  focusedAt: null,
  acceptanceCriteria: null,
  completed: false,
  createdAt: "",
  updatedAt: "",
};

export function useIssueDetail(
  issueId: string,
  projectMap: Map<string, DisplayProject>,
  areaMap: Map<string, Area>,
) {
  const { labels } = useStatusWorkflow();

  // Core task data
  const { data: rawTask } = useQuery<Task | null>("task_get", { id: issueId }, null);

  const task: DetailTask = useMemo(
    () => (rawTask ? taskToDetailTask(rawTask, projectMap, areaMap, labels) : PLACEHOLDER_TASK),
    [rawTask, projectMap, areaMap, labels],
  );

  const taskState: TaskState = deriveTaskState(task);

  // Sub-issues
  const { data: rawChildren } = useQuery<Task[]>("task_list_children", { parentId: issueId }, []);
  const subIssues: SubIssue[] = useMemo(
    () => rawChildren.map((c) => taskToSubIssue(c, labels)),
    [rawChildren, labels],
  );

  // Activity from timeline (client-side filtered by entityId)
  const endDate = useMemo(() => new Date().toISOString(), []);
  const timelineArgs = useMemo(
    () => ({
      startDate: "2020-01-01T00:00:00Z",
      endDate,
      sources: ["task"],
    }),
    [endDate],
  );
  const { data: timeline } = useQuery<TimelineResponse>(
    "timeline_query",
    timelineArgs,
    EMPTY_TIMELINE_RESPONSE,
  );
  const activity: ActivityEntry[] = useMemo(
    () => timeline.entries.filter((entry) => entry.entityId === issueId).map(timelineToActivity),
    [timeline, issueId],
  );

  // Mutations
  const updateMutation = useMutation<Task, TaskUpdateParams>("task_update", "params");

  const updateTask = useCallback(
    <K extends keyof DetailTask>(field: K, value: DetailTask[K]) => {
      const params: TaskUpdateParams = { id: issueId };

      switch (field) {
        case "title":
          params.title = value as string;
          break;
        case "description":
          params.description = value as string;
          break;
        case "status": {
          const mutationParams = statusToMutationParams(value as DetailTask["status"]);
          params.status = mutationParams.status;
          params.statusLabelId = mutationParams.statusLabelId;
          break;
        }
        case "priority":
          params.priority = priorityToNumber((value as DetailTask["priority"]).id);
          break;
        case "dueDate":
          params.dueDate = value as string | null;
          break;
        case "project":
          params.projectId = (value as DetailTask["project"])?.id ?? null;
          break;
        case "area":
          params.areaId = (value as DetailTask["area"])?.id;
          break;
        case "tags":
          params.tags = value as string[];
          break;
        case "energyLevel":
          params.energyLevel = value as TaskUpdateParams["energyLevel"];
          break;
        case "taskType":
          params.taskType = value as TaskUpdateParams["taskType"];
          break;
        case "estimatedMinutes":
          params.estimatedMinutes = value as number | null;
          break;
        case "acceptanceCriteria":
          params.acceptanceCriteria = value as string | null;
          break;
        default:
          break;
      }

      updateMutation.mutate(params);
    },
    [issueId, updateMutation],
  );

  // Stubbed features (future backend support)
  const suggestions: Suggestion[] = [];
  const taskMemory: TaskMemory | null = null;
  const focusSession: FocusSession | null = buildFocusSession(task);

  const dismissSuggestion = useCallback((_id: string) => {}, []);
  const applySuggestion = useCallback((_id: string) => {}, []);

  // Focus mutations
  const endFocusMutation = useMutation<Task | null, { id: string }>("task_end_focus", "params");

  const stopFocus = useCallback(async () => {
    if (!issueId) return;
    await endFocusMutation.mutate({ id: issueId });
  }, [issueId, endFocusMutation]);

  return {
    task,
    taskState,
    activity,
    suggestions,
    focusSession,
    subIssues,
    taskMemory,
    updateTask,
    dismissSuggestion,
    applySuggestion,
    stopFocus,
  };
}
