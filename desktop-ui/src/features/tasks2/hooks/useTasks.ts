import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import type { ApiError } from "@shared/types";
import type { Area, Project, Task, TaskCreateParams, TaskUpdateParams } from "@shared/types/tasks";
import { useCallback, useMemo } from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import {
  type DisplayProject,
  type Issue,
  projectToDisplayProject,
  taskToIssue,
} from "../lib/mappers";

export interface UseTasksResult {
  issues: Issue[];
  loading: boolean;
  error: ApiError | null;
  refetch: () => void;
  updateTask: ReturnType<typeof useMutation<Task, TaskUpdateParams>>;
  createTask: ReturnType<typeof useMutation<Task, TaskCreateParams>>;
  deleteTask: ReturnType<typeof useMutation<boolean, { id: string }>>;
  toggleComplete: ReturnType<typeof useMutation<Task, { id: string }>>;
  projectMap: Map<string, DisplayProject>;
  areaMap: Map<string, Area>;
  areas: Area[];
  projects: Project[];
}

export function useTasks(): UseTasksResult {
  const { labels } = useStatusWorkflow();
  const {
    data: tasks,
    loading: tasksLoading,
    error: tasksError,
    refetch: refetchTasks,
  } = useQuery<Task[]>("task_list", undefined, []);
  const { data: projects, refetch: refetchProjects } = useQuery<Project[]>(
    "project_list",
    undefined,
    [],
  );
  const { data: areas, refetch: refetchAreas } = useQuery<Area[]>("area_list", undefined, []);

  const projectMap = useMemo(
    () => new Map(projects.map((p) => [p.id, projectToDisplayProject(p)])),
    [projects],
  );
  const areaMap = useMemo(() => new Map(areas.map((a) => [a.id, a])), [areas]);

  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const createTask = useMutation<Task, TaskCreateParams>("task_create", "params");
  const deleteTask = useMutation<boolean, { id: string }>("task_delete");
  const toggleComplete = useMutation<Task, { id: string }>("task_toggle_complete");

  const issues = useMemo(
    () => tasks.map((t) => taskToIssue(t, projectMap, labels)),
    [tasks, projectMap, labels],
  );

  const refetch = useCallback(() => {
    refetchTasks();
    refetchProjects();
    refetchAreas();
  }, [refetchTasks, refetchProjects, refetchAreas]);

  // Auto-refresh when relevant entities change
  useEvent<{ entityKind: string; id: string }>("entity:updated", (payload) => {
    const kind = payload?.entityKind;
    if (!kind) {
      refetchTasks();
      refetchProjects();
      refetchAreas();
      return;
    }
    if (kind === "task" || kind === "area") refetchTasks();
    if (kind === "project") refetchProjects();
    if (kind === "area") refetchAreas();
  });

  return {
    issues,
    loading: tasksLoading,
    error: tasksError,
    refetch,
    updateTask,
    createTask,
    deleteTask,
    toggleComplete,
    projectMap,
    areaMap,
    areas,
    projects,
  };
}
