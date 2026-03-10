import { useCallback, useEffect, useMemo, useOptimistic, useState, useTransition } from "react";
import { useSearchParams } from "react-router";
import { useEvent } from "@shared/hooks/useEvent";
import { useGroups } from "@shared/hooks/useGroups";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { useSetToggle } from "@shared/hooks/useSetToggle";
import { useSubtasks } from "@shared/hooks/useSubtasks";
import { useEffectiveLabels } from "@shared/hooks/useWorkflows";
import type {
  Area,
  Objective,
  Project,
  Tab,
  Task,
  TaskCreateParams,
  TaskUpdateParams,
  ViewMode,
} from "@shared/types";
import { KanbanBoard } from "../components/KanbanBoard";
import { TaskTable } from "../components/TaskTable";
import { TaskTableSkeleton } from "../components/TaskTableSkeleton";
import { Toolbar } from "../components/Toolbar";

export function TasksPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const activeTab = (searchParams.get("tab") as Tab) || "All";
  const viewMode = (searchParams.get("view") as ViewMode) || "table";
  const setActiveTab = useCallback(
    (tab: Tab) => {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev);
        if (tab === "All") next.delete("tab");
        else next.set("tab", tab);
        return next;
      });
    },
    [setSearchParams],
  );
  const setViewMode = useCallback(
    (mode: ViewMode) => {
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev);
        if (mode === "table") next.delete("view");
        else next.set("view", mode);
        return next;
      });
    },
    [setSearchParams],
  );
  const [isPending, startTransition] = useTransition();
  const [collapsedProjects, toggleProject] = useSetToggle();
  const [collapsedGroups, toggleGroup] = useSetToggle();
  const {
    expandedTasks,
    childrenCache,
    toggleExpand: toggleExpandTask,
    fetchChildren,
    invalidateCache,
  } = useSubtasks();

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
  const { data: objectives } = useQuery<Objective[]>("objective_list", undefined, []);
  const { data: areas, refetch: refetchAreas } = useQuery<Area[]>("area_list", undefined, []);
  const { data: statusLabels } = useEffectiveLabels(null);
  const { data: groups } = useGroups(null);

  const areaMap = useMemo(() => new Map(areas.map((a) => [a.id, a])), [areas]);
  const projectMap = useMemo(() => new Map(projects.map((p) => [p.id, p])), [projects]);

  const toggleComplete = useMutation<Task, { id: string }>("task_toggle_complete");
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const createTask = useMutation<Task, TaskCreateParams>("task_create", "params");

  // useOptimistic derives from server state; overlay flips completion for in-flight toggles.
  const [optimisticTasks, addOptimistic] = useOptimistic(tasks, (current, toggledId: string) =>
    current.map((t) => (t.id === toggledId ? { ...t, completed: !t.completed } : t)),
  );

  const completedTasks = useMemo(
    () => new Set(optimisticTasks.filter((t) => t.completed).map((t) => t.id)),
    [optimisticTasks],
  );

  // Auto-fetch children for expanded tasks.
  useEffect(() => {
    for (const taskId of expandedTasks) {
      fetchChildren(taskId);
    }
  }, [expandedTasks, fetchChildren]);

  // Inline add task state
  const [addingTask, setAddingTask] = useState(false);
  const [newTaskTitle, setNewTaskTitle] = useState("");

  const handleToggleTask = useCallback(
    async (taskId: string) => {
      addOptimistic(taskId);
      await toggleComplete.mutate({ id: taskId });
      refetchTasks();
    },
    [addOptimistic, toggleComplete, refetchTasks],
  );

  const handleUpdateTask = useCallback(
    async (params: TaskUpdateParams) => {
      await updateTask.mutate(params);
    },
    [updateTask],
  );

  const handleAddTask = useCallback(async () => {
    if (!newTaskTitle.trim()) return;
    await createTask.mutate({ title: newTaskTitle.trim() });
    setNewTaskTitle("");
    setAddingTask(false);
  }, [newTaskTitle, createTask]);

  const handleCreateSubtask = useCallback(
    async (parentId: string, title: string) => {
      await createTask.mutate({ title, parentId });
    },
    [createTask],
  );

  // Auto-refresh when relevant entities change
  useEvent<{ entityKind: string; id: string }>("entity:updated", (payload) => {
    const kind = payload?.entityKind;
    if (!kind) {
      refetchTasks();
      refetchProjects();
      refetchAreas();
      invalidateCache();
      return;
    }
    if (kind === "task" || kind === "area") {
      refetchTasks();
      invalidateCache();
    }
    if (kind === "project" || kind === "objective") refetchProjects();
    if (kind === "area") refetchAreas();
  });

  const filteredTasks = useMemo(() => {
    if (activeTab === "All") return tasks;
    const tabLower = activeTab.toLowerCase();
    return tasks.filter((task) => {
      const area = areaMap.get(task.areaId);
      return area?.name.toLowerCase() === tabLower;
    });
  }, [activeTab, tasks, areaMap]);

  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      {/* Header Tabs — glass toolbar */}
      <div className="h-12 flex items-center px-2 shrink-0">
        <div className="flex-1 flex items-center gap-1.5">
          {["All" as Tab, ...areas.map((a) => a.name as Tab)].map((tab) => (
            <button
              type="button"
              key={tab}
              onClick={() => startTransition(() => setActiveTab(tab))}
              className={`flex-1 py-2 rounded-xl text-[13px] font-light transition-all duration-200 ${
                activeTab === tab
                  ? "glass-button-active text-primary"
                  : "text-muted hover:text-secondary hover:bg-white/[0.04]"
              }`}
            >
              {tab}
            </button>
          ))}
        </div>
      </div>

      {/* Scrollable Content — glass panel */}
      <div
        className={`flex-1 overflow-y-auto p-3${isPending ? " opacity-70 transition-opacity" : ""}`}
      >
        <Toolbar
          viewMode={viewMode}
          onViewModeChange={setViewMode}
          onAddTask={() => setAddingTask(true)}
        />

        {/* Inline Add Task Row */}
        {addingTask && (
          <div className="mb-2 glass-card px-5 py-3">
            <input
              value={newTaskTitle}
              onChange={(e) => setNewTaskTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleAddTask();
                if (e.key === "Escape") {
                  setAddingTask(false);
                  setNewTaskTitle("");
                }
              }}
              onBlur={() => {
                if (!newTaskTitle.trim()) {
                  setAddingTask(false);
                  setNewTaskTitle("");
                }
              }}
              placeholder="Task title... (Enter to save, Esc to cancel)"
              className="w-full bg-transparent text-[13px] font-light text-primary outline-none placeholder:text-dim"
            />
          </div>
        )}

        {tasksLoading ? (
          <TaskTableSkeleton showArea={activeTab === "All"} />
        ) : tasksError ? (
          <div className="flex flex-col items-center py-10 gap-2">
            <p className="text-muted text-sm font-light">Failed to load tasks</p>
            <button
              type="button"
              onClick={refetchTasks}
              className="text-brand text-xs font-light hover:underline"
            >
              Retry
            </button>
          </div>
        ) : viewMode === "board" ? (
          <KanbanBoard
            tasks={filteredTasks}
            projectMap={projectMap}
            areaMap={areaMap}
            completedTasks={completedTasks}
            statusLabels={statusLabels}
            onUpdateTask={handleUpdateTask}
            onRefetch={refetchTasks}
          />
        ) : (
          <TaskTable
            tasks={filteredTasks}
            projectMap={projectMap}
            objectives={objectives}
            areaMap={areaMap}
            activeTab={activeTab}
            completedTasks={completedTasks}
            collapsedProjects={collapsedProjects}
            expandedTasks={expandedTasks}
            childrenCache={childrenCache}
            onToggleTask={handleToggleTask}
            onToggleProject={toggleProject}
            onToggleExpandTask={toggleExpandTask}
            onUpdate={handleUpdateTask}
            onCreateSubtask={handleCreateSubtask}
            statusLabels={statusLabels}
            groups={groups}
            collapsedGroups={collapsedGroups}
            onToggleGroup={toggleGroup}
          />
        )}
        {filteredTasks.length === 0 && !addingTask && (
          <div className="flex flex-col items-center justify-center py-20 text-center">
            <p className="text-muted text-sm font-light">No tasks yet</p>
            <p className="text-dim text-xs font-light mt-1">Create a task to get started</p>
          </div>
        )}
      </div>
    </div>
  );
}
