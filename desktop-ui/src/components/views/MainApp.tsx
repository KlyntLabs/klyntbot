import { MessageSquare } from "lucide-react";
import {
  useCallback,
  useEffect,
  useMemo,
  useOptimistic,
  useRef,
  useState,
  useTransition,
} from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import { useSetToggle } from "../../hooks/useSetToggle";
import { useSubtasks } from "../../hooks/useSubtasks";
import type {
  Area,
  Objective,
  Project,
  SidebarItem,
  Tab,
  Task,
  TaskCreateParams,
  TaskUpdateParams,
  ViewMode,
} from "../../lib/types";
import { SidebarChat } from "../chat/SidebarChat";
import { Sidebar } from "../layout/Sidebar";
import { KanbanBoard } from "../tasks/KanbanBoard";
import { TaskTable } from "../tasks/TaskTable";
import { TaskTableSkeleton } from "../tasks/TaskTableSkeleton";
import { Toolbar } from "../tasks/Toolbar";
import { OkrView } from "./OkrView";

export function MainApp() {
  const _navigate = useNavigate();
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
  const [activeSidebar, setActiveSidebar] = useState<SidebarItem>("Tasks");
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [openSessionKey, setOpenSessionKey] = useState<string | null>(null);
  const prevSidebarRef = useRef<SidebarItem>("Tasks");
  const [isPending, startTransition] = useTransition();
  const [collapsedProjects, toggleProject] = useSetToggle();
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

  const clearOpenSessionKey = useCallback(() => setOpenSessionKey(null), []);

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

  // Listen for open-chat events from the tray / launcher
  useEvent<{ text?: string; sessionKey?: string }>("open-chat", (payload) => {
    setIsChatOpen(true);
    setActiveSidebar("Chat");
    setOpenSessionKey(payload?.sessionKey ?? null);
  });

  const filteredTasks = useMemo(() => {
    if (activeTab === "All") return tasks;
    const tabLower = activeTab.toLowerCase();
    return tasks.filter((task) => {
      const area = areaMap.get(task.areaId);
      return area?.name.toLowerCase() === tabLower;
    });
  }, [activeTab, tasks, areaMap]);

  // Content view is independent of chat state
  const contentView = activeSidebar === "Chat" ? prevSidebarRef.current : activeSidebar;

  // Derive sidebar chat context from the current view + tab
  const sidebarViewContext = useMemo(() => {
    const section = contentView.toLowerCase();
    if (activeTab === "All") return { entityKind: section };
    return { entityKind: `${section}.${activeTab.toLowerCase()}` };
  }, [contentView, activeTab]);

  return (
    <div className="h-screen w-screen bg-background text-primary flex gap-2 p-2 overflow-hidden">
      <Sidebar
        active={activeSidebar}
        onNavigate={(item) => {
          setActiveSidebar(item);
          if (item !== "Chat") setIsChatOpen(false);
        }}
      />

      {/* Main Content */}
      <div className="flex-1 flex flex-col gap-2 overflow-hidden">
        {/* Header Tabs — glass toolbar */}
        <div className="h-12 flex items-center px-2 shrink-0">
          <div className="flex-1 flex items-center gap-1.5">
            {(["All", "Work", "Personal"] as Tab[]).map((tab) => (
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
          <button
            type="button"
            onClick={() => {
              const nextOpen = !isChatOpen;
              if (nextOpen) {
                prevSidebarRef.current = activeSidebar;
              }
              setIsChatOpen(nextOpen);
              setActiveSidebar(nextOpen ? "Chat" : prevSidebarRef.current);
            }}
            aria-label="Toggle chat"
            className={`w-9 h-9 rounded-xl flex items-center justify-center transition-all duration-200 ml-2 ${
              isChatOpen
                ? "glass-button-active text-brand"
                : "text-muted hover:text-secondary hover:bg-white/[0.04]"
            }`}
          >
            <MessageSquare className="w-[18px] h-[18px]" strokeWidth={1.5} />
          </button>
        </div>

        {/* Scrollable Content — glass panel */}
        <div
          className={`flex-1 overflow-y-auto p-3${isPending ? " opacity-70 transition-opacity" : ""}`}
        >
          {contentView === "OKR" ? (
            <OkrView />
          ) : (
            <>
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
                />
              )}
              {filteredTasks.length === 0 && !addingTask && (
                <div className="flex flex-col items-center justify-center py-20 text-center">
                  <p className="text-muted text-sm font-light">No tasks yet</p>
                  <p className="text-dim text-xs font-light mt-1">Create a task to get started</p>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Sidebar Chat */}
      <SidebarChat
        isOpen={isChatOpen}
        onClose={() => setIsChatOpen(false)}
        viewContext={sidebarViewContext}
        openSessionKey={openSessionKey}
        onSessionKeyUsed={clearOpenSessionKey}
      />
    </div>
  );
}
