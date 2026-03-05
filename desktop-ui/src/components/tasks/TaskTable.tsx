import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo } from "react";
import type { Area, Objective, Project, Tab, Task, TaskUpdateParams } from "../../lib/types";
import { AddSubtaskRow } from "./AddSubtaskRow";
import { ProjectHeader } from "./ProjectHeader";
import { RootTaskRow, SubtaskRow } from "./TaskRow";
import { TaskTableContext, useTaskTable } from "./TaskTableContext";

interface TaskTableProps {
  tasks: Task[];
  projectMap: Map<string, Project>;
  objectives: Objective[];
  areaMap: Map<string, Area>;
  activeTab: Tab;
  completedTasks: Set<string>;
  collapsedProjects: Set<string>;
  expandedTasks: Set<string>;
  childrenCache: Map<string, Task[]>;
  onToggleTask: (taskId: string) => void;
  onToggleProject: (projectId: string) => void;
  onToggleExpandTask: (taskId: string) => void;
  onUpdate: (params: TaskUpdateParams) => void;
  onCreateSubtask: (parentId: string, title: string) => void;
}

const UNASSIGNED = "_unassigned";

export function TaskTable({
  tasks,
  projectMap,
  objectives,
  areaMap,
  activeTab,
  completedTasks,
  collapsedProjects,
  expandedTasks,
  childrenCache,
  onToggleTask,
  onToggleProject,
  onToggleExpandTask,
  onUpdate,
  onCreateSubtask,
}: TaskTableProps) {
  const showArea = activeTab === "All";

  const projects = useMemo(() => Array.from(projectMap.values()), [projectMap]);
  const areas = useMemo(() => Array.from(areaMap.values()), [areaMap]);

  // Tasks are already root-only from the backend (root_only: true)
  const tasksByProject = useMemo(
    () =>
      tasks.reduce(
        (acc, task) => {
          const key = task.projectId ?? UNASSIGNED;
          if (!acc[key]) acc[key] = [];
          acc[key].push(task);
          return acc;
        },
        {} as Record<string, Task[]>,
      ),
    [tasks],
  );

  const objectiveMap = useMemo(() => new Map(objectives.map((o) => [o.id, o])), [objectives]);

  // Sort groups: assigned projects first, unassigned last
  const sortedEntries = useMemo(() => {
    const entries = Object.entries(tasksByProject);
    return entries.sort(([a], [b]) => {
      if (a === UNASSIGNED) return 1;
      if (b === UNASSIGNED) return -1;
      return 0;
    });
  }, [tasksByProject]);

  const ctx = useMemo<import("./TaskTableContext").TaskTableCtx>(
    () => ({
      completedTasks,
      expandedTasks,
      childrenCache,
      projects,
      areas,
      showArea,
      onToggleTask,
      onToggleExpandTask,
      onUpdate,
      onCreateSubtask,
    }),
    [
      completedTasks,
      expandedTasks,
      childrenCache,
      projects,
      areas,
      showArea,
      onToggleTask,
      onToggleExpandTask,
      onUpdate,
      onCreateSubtask,
    ],
  );

  return (
    <TaskTableContext value={ctx}>
      <div className="mb-10 glass-card overflow-hidden">
        <table className="w-full border-collapse">
          <thead>
            <tr className="border-b border-white/[0.06] text-[11px] text-muted font-light text-left bg-white/[0.03]">
              <th className="px-5 py-2.5 w-9 font-light" />
              <th className="px-5 py-2.5 font-light tracking-wide uppercase">Task</th>
              <th className="px-5 py-2.5 font-light tracking-wide uppercase">Project</th>
              {showArea && <th className="px-5 py-2.5 font-light tracking-wide uppercase">Area</th>}
              <th className="px-5 py-2.5 font-light tracking-wide uppercase">Priority</th>
              <th className="px-5 py-2.5 font-light tracking-wide uppercase">Status</th>
              <th className="px-5 py-2.5 font-light tracking-wide uppercase">Due Date</th>
              <th className="px-5 py-2.5 font-light tracking-wide uppercase">Tags</th>
            </tr>
          </thead>
          <tbody>
            {sortedEntries.map(([projectId, projectTasks]) => {
              const project = projectMap.get(projectId);

              const isCollapsed = collapsedProjects.has(projectId);

              // Unassigned tasks group
              if (projectId === UNASSIGNED) {
                return (
                  <TaskGroup
                    key={UNASSIGNED}
                    header={
                      <button
                        type="button"
                        onClick={() => onToggleProject(UNASSIGNED)}
                        aria-expanded={!isCollapsed}
                        className="w-full flex items-center gap-3 px-5 py-2.5 bg-white/[0.03] hover:bg-white/[0.05] transition-colors text-left"
                      >
                        {isCollapsed ? (
                          <ChevronRight
                            className="w-[14px] h-[14px] text-muted flex-shrink-0"
                            strokeWidth={1.5}
                          />
                        ) : (
                          <ChevronDown
                            className="w-[14px] h-[14px] text-muted flex-shrink-0"
                            strokeWidth={1.5}
                          />
                        )}
                        <span className="text-[12px] font-light text-muted">No Project</span>
                        <span className="text-[11px] text-dim font-light">
                          ({projectTasks.length})
                        </span>
                      </button>
                    }
                    tasks={projectTasks}
                    isCollapsed={isCollapsed}
                  />
                );
              }

              // Skip unknown projects
              if (!project) return null;

              const projectObjectives = project.objectiveIds
                ? project.objectiveIds.flatMap((id) => {
                    const obj = objectiveMap.get(id);
                    return obj ? [obj] : [];
                  })
                : [];

              return (
                <TaskGroup
                  key={projectId}
                  header={
                    <ProjectHeader
                      project={project}
                      tasks={projectTasks}
                      objectives={projectObjectives}
                      isCollapsed={isCollapsed}
                      onToggle={() => onToggleProject(projectId)}
                    />
                  }
                  tasks={projectTasks}
                  isCollapsed={isCollapsed}
                />
              );
            })}
          </tbody>
        </table>
      </div>
    </TaskTableContext>
  );
}

/** Renders a task row plus its lazily-loaded subtasks if expanded. */
function TaskWithSubtasks({ task }: { task: Task }) {
  const { expandedTasks, childrenCache, showArea, completedTasks, onToggleTask, onUpdate } =
    useTaskTable();
  const isExpanded = expandedTasks.has(task.id);
  const subtasks = childrenCache.get(task.id);
  const isLoading = isExpanded && !subtasks;

  return (
    <>
      <RootTaskRow
        task={task}
        isExpanded={isExpanded}
        isCompleted={completedTasks.has(task.id)}
        onToggle={() => onToggleTask(task.id)}
        onUpdate={onUpdate}
      />
      {isLoading && (
        <tr className="border-b border-white/[0.04]">
          <td className="px-5 py-2 w-9" />
          <td colSpan={showArea ? 7 : 6} className="px-5 py-2">
            <span className="text-[12px] text-dim font-light pl-6">Loading\u2026</span>
          </td>
        </tr>
      )}
      {isExpanded &&
        subtasks &&
        subtasks.map((sub) => (
          <SubtaskRow
            key={sub.id}
            task={sub}
            isCompleted={completedTasks.has(sub.id)}
            onToggle={() => onToggleTask(sub.id)}
            onUpdate={onUpdate}
          />
        ))}
      {isExpanded && subtasks && <AddSubtaskRow parentId={task.id} />}
    </>
  );
}

/** Generic task group: header row + task rows. */
function TaskGroup({
  header,
  tasks,
  isCollapsed,
}: {
  header: React.ReactNode;
  tasks: Task[];
  isCollapsed: boolean;
}) {
  const { showArea } = useTaskTable();
  const colCount = showArea ? 8 : 7;
  return (
    <>
      <tr className="border-b border-white/[0.04]">
        <td colSpan={colCount} className="p-0">
          {header}
        </td>
      </tr>
      {!isCollapsed && tasks.map((task) => <TaskWithSubtasks key={task.id} task={task} />)}
    </>
  );
}
