import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo } from "react";
import type { Area, Objective, Project, Tab, Task, TaskUpdateParams } from "../../lib/types";
import { AddSubtaskRow } from "./AddSubtaskRow";
import { ProjectHeader } from "./ProjectHeader";
import { TaskRow } from "./TaskRow";

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

  return (
    <div className="mb-10 rounded-xl">
      <table className="w-full bg-surface-low border-collapse">
        <thead>
          <tr className="border-b border-border text-[11px] text-muted font-light text-left">
            <th className="px-5 py-3 w-9 font-light" />
            <th className="px-5 py-3 font-light">Task</th>
            <th className="px-5 py-3 font-light">Project</th>
            {showArea && <th className="px-5 py-3 font-light">Area</th>}
            <th className="px-5 py-3 font-light">Priority</th>
            <th className="px-5 py-3 font-light">Status</th>
            <th className="px-5 py-3 font-light">Due Date</th>
            <th className="px-5 py-3 font-light">Tags</th>
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
                      className="w-full flex items-center gap-3 px-5 py-3 bg-overlay hover:bg-overlay-heavy transition-colors text-left"
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
                  childrenCache={childrenCache}
                  projects={projects}
                  areas={areas}
                  completedTasks={completedTasks}
                  expandedTasks={expandedTasks}
                  showArea={showArea}
                  isCollapsed={isCollapsed}
                  onToggleTask={onToggleTask}
                  onToggleExpandTask={onToggleExpandTask}
                  onUpdate={onUpdate}
                  onCreateSubtask={onCreateSubtask}
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
                childrenCache={childrenCache}
                projects={projects}
                areas={areas}
                completedTasks={completedTasks}
                expandedTasks={expandedTasks}
                showArea={showArea}
                isCollapsed={isCollapsed}
                onToggleTask={onToggleTask}
                onToggleExpandTask={onToggleExpandTask}
                onUpdate={onUpdate}
                onCreateSubtask={onCreateSubtask}
              />
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Renders a task row plus its lazily-loaded subtasks if expanded. */
function TaskWithSubtasks({
  task,
  childrenCache,
  projects,
  areas,
  completedTasks,
  expandedTasks,
  showArea,
  onToggleTask,
  onToggleExpandTask,
  onUpdate,
  onCreateSubtask,
}: {
  task: Task;
  childrenCache: Map<string, Task[]>;
  projects: Project[];
  areas: Area[];
  completedTasks: Set<string>;
  expandedTasks: Set<string>;
  showArea: boolean;
  onToggleTask: (id: string) => void;
  onToggleExpandTask: (id: string) => void;
  onUpdate: (params: TaskUpdateParams) => void;
  onCreateSubtask: (parentId: string, title: string) => void;
}) {
  const isExpanded = expandedTasks.has(task.id);
  const subtasks = childrenCache.get(task.id);
  const isLoading = isExpanded && !subtasks;

  return (
    <>
      <TaskRow
        task={task}
        depth={0}
        isExpanded={isExpanded}
        projects={projects}
        areas={areas}
        isCompleted={completedTasks.has(task.id)}
        showArea={showArea}
        onToggle={() => onToggleTask(task.id)}
        onToggleExpand={() => onToggleExpandTask(task.id)}
        onUpdate={onUpdate}
      />
      {isLoading && (
        <tr className="border-b border-border-subtle">
          <td className="px-5 py-2 w-9" />
          <td colSpan={showArea ? 7 : 6} className="px-5 py-2">
            <span className="text-[12px] text-dim font-light" style={{ paddingLeft: 24 }}>
              Loading...
            </span>
          </td>
        </tr>
      )}
      {isExpanded &&
        subtasks &&
        subtasks.map((sub) => (
          <TaskRow
            key={sub.id}
            task={sub}
            depth={1}
            projects={projects}
            areas={areas}
            isCompleted={completedTasks.has(sub.id)}
            showArea={showArea}
            onToggle={() => onToggleTask(sub.id)}
            onUpdate={onUpdate}
          />
        ))}
      {isExpanded && subtasks && (
        <AddSubtaskRow parentId={task.id} showArea={showArea} onCreate={onCreateSubtask} />
      )}
    </>
  );
}

/** Generic task group: header row + task rows. */
function TaskGroup({
  header,
  tasks,
  childrenCache,
  projects,
  areas,
  completedTasks,
  expandedTasks,
  showArea,
  isCollapsed,
  onToggleTask,
  onToggleExpandTask,
  onUpdate,
  onCreateSubtask,
}: {
  header: React.ReactNode;
  tasks: Task[];
  childrenCache: Map<string, Task[]>;
  projects: Project[];
  areas: Area[];
  completedTasks: Set<string>;
  expandedTasks: Set<string>;
  showArea: boolean;
  isCollapsed: boolean;
  onToggleTask: (id: string) => void;
  onToggleExpandTask: (id: string) => void;
  onUpdate: (params: TaskUpdateParams) => void;
  onCreateSubtask: (parentId: string, title: string) => void;
}) {
  const colCount = showArea ? 8 : 7;
  return (
    <>
      <tr className="border-b border-border-subtle">
        <td colSpan={colCount} className="p-0">
          {header}
        </td>
      </tr>
      {!isCollapsed &&
        tasks.map((task) => (
          <TaskWithSubtasks
            key={task.id}
            task={task}
            childrenCache={childrenCache}
            projects={projects}
            areas={areas}
            completedTasks={completedTasks}
            expandedTasks={expandedTasks}
            showArea={showArea}
            onToggleTask={onToggleTask}
            onToggleExpandTask={onToggleExpandTask}
            onUpdate={onUpdate}
            onCreateSubtask={onCreateSubtask}
          />
        ))}
    </>
  );
}
