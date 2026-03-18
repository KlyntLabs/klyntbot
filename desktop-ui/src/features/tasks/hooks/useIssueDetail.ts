import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import type { TimelineResponse } from "@shared/types/common";
import type { Area, Task, TaskUpdateParams } from "@shared/types/tasks";
import { useCallback, useMemo, useState } from "react";
import type { DecompositionResult } from "../components/detail/DecompositionPanel";
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
  type SuggestionStatus,
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

export interface TaskForecast {
  estimatedMinutes: number;
  confidenceLow: number;
  confidenceHigh: number;
  methodology: string;
  sampleSize: number;
  dataQuality: string;
  risks: Array<{ kind: string; description: string; impactMinutes: number | null }>;
}

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
  complexityScore: null,
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
  const { data: rawTask, refetch } = useQuery<Task | null>("task_get", { id: issueId }, null);

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

  // AI suggestions
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [suggestionsLoading, setSuggestionsLoading] = useState(false);

  const [aiError, setAiError] = useState<string | null>(null);

  const fetchSuggestions = useCallback(async () => {
    setSuggestionsLoading(true);
    setAiError(null);
    try {
      const results = await ipc<
        Array<{
          id: string;
          suggestionType: string;
          title: string;
          description: string | null;
          confidence: number;
          status: string;
          createdAt: string;
        }>
      >("task_get_suggestions", { taskId: task.id });
      setSuggestions(
        results.map((s) => ({
          id: s.id,
          suggestionType: s.suggestionType,
          title: s.title,
          description: s.description ?? "",
          confidence: s.confidence,
          status: s.status as SuggestionStatus,
        })),
      );
    } catch (e: unknown) {
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === "object" && e !== null
            ? JSON.stringify(e)
            : String(e);
      setAiError(msg);
    } finally {
      setSuggestionsLoading(false);
    }
  }, [task.id]);

  const applySuggestion = useCallback(
    async (id: string) => {
      await ipc("task_apply_suggestion", { suggestionId: id });
      await fetchSuggestions();
      refetch();
    },
    [fetchSuggestions, refetch],
  );

  const dismissSuggestion = useCallback(async (id: string) => {
    await ipc("task_dismiss_suggestion", { suggestionId: id });
    setSuggestions((prev) => prev.filter((s) => s.id !== id));
  }, []);

  // Decomposition
  const [decompositionResult, setDecompositionResult] = useState<DecompositionResult | null>(null);
  const [decompositionOpen, setDecompositionOpen] = useState(false);
  const [decompositionApplying, setDecompositionApplying] = useState(false);
  const [decompositionLoading, setDecompositionLoading] = useState(false);

  const openDecomposition = useCallback(() => {
    setDecompositionOpen(true);
    setDecompositionResult(null);
    setAiError(null);
  }, []);

  const decompose = useCallback(async () => {
    setAiError(null);
    setDecompositionLoading(true);
    try {
      const result = await ipc<DecompositionResult>("task_decompose", { taskId: task.id });
      if (result.autoApplied) {
        setDecompositionOpen(false);
        refetch();
      } else {
        setDecompositionResult(result);
      }
    } catch (e: unknown) {
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === "object" && e !== null
            ? JSON.stringify(e)
            : String(e);
      setAiError(msg);
    } finally {
      setDecompositionLoading(false);
    }
  }, [task.id, refetch]);

  const applyDecomposition = useCallback(
    async (id: string) => {
      setDecompositionApplying(true);
      try {
        await ipc("task_apply_decomposition", { decompositionId: id });
        setDecompositionOpen(false);
        setDecompositionResult(null);
        refetch();
      } finally {
        setDecompositionApplying(false);
      }
    },
    [refetch],
  );

  const closeDecomposition = useCallback(() => {
    setDecompositionOpen(false);
    setDecompositionResult(null);
  }, []);

  const rejectDecomposition = useCallback(
    async (id: string) => {
      await ipc("task_reject_decomposition", { decompositionId: id });
      closeDecomposition();
    },
    [closeDecomposition],
  );

  // Forecast
  const [forecast, setForecast] = useState<TaskForecast | null>(null);
  const [forecastLoading, setForecastLoading] = useState(false);

  const fetchForecast = useCallback(async () => {
    setForecastLoading(true);
    try {
      const result = await ipc<TaskForecast>("task_forecast", { taskId: task.id });
      setForecast(result);
    } catch (e: unknown) {
      setForecast(null);
      const msg =
        e instanceof Error
          ? e.message
          : typeof e === "object" && e !== null
            ? JSON.stringify(e)
            : String(e);
      setAiError(msg);
    } finally {
      setForecastLoading(false);
    }
  }, [task.id]);

  const taskMemory: TaskMemory | null = null;
  const focusSession: FocusSession | null = buildFocusSession(task);

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
    suggestionsLoading,
    fetchSuggestions,
    focusSession,
    subIssues,
    taskMemory,
    updateTask,
    dismissSuggestion,
    applySuggestion,
    stopFocus,
    decompositionResult,
    decompositionOpen,
    decompositionLoading,
    decompositionApplying,
    openDecomposition,
    decompose,
    applyDecomposition,
    rejectDecomposition,
    closeDecomposition,
    forecast,
    forecastLoading,
    fetchForecast,
    aiError,
  };
}
