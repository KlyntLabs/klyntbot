import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo } from "react";
import type {
  Area,
  Objective,
  Project,
  StatusLabel,
  Tab,
  Task,
  TaskGroup,
  TaskUpdateParams,
} from "../../lib/types";
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
  statusLabels: StatusLabel[];
  groups?: TaskGroup[];
  collapsedGroups?: Set<string>;
  onToggleGroup?: (groupId: string) => void;
}

const UNASSIGNED = "_unassigned";
const UNGROUPED = "_ungrouped";

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
  statusLabels,
  groups = [],
  collapsedGroups = new Set(),
  onToggleGroup = () => {},
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

  // Group groups by project
  const groupsByProject = useMemo(() => {
    const map = new Map<string, TaskGroup[]>();
    for (const group of groups) {
      const key = group.projectId ?? UNASSIGNED;
      const arr = map.get(key) ?? [];
      arr.push(group);
      map.set(key, arr);
    }
    return map;
  }, [groups]);

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
      statusLabels,
      groups,
      collapsedGroups,
      showArea,
      onToggleTask,
      onToggleExpandTask,
      onToggleGroup,
      onUpdate,
      onCreateSubtask,
    }),
    [
      completedTasks,
      expandedTasks,
      childrenCache,
      projects,
      areas,
      statusLabels,
      groups,
      collapsedGroups,
      showArea,
      onToggleTask,
      onToggleExpandTask,
      onToggleGroup,
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
              const projectGroups = groupsByProject.get(projectId) ?? [];

              // Unassigned tasks group
              if (projectId === UNASSIGNED) {
                return (
                  <ProjectTaskGroup
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
                    groups={projectGroups}
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
                <ProjectTaskGroup
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
                  groups={projectGroups}
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

/** Project-level group: header row + group sections (or flat tasks if no groups). */
function ProjectTaskGroup({
  header,
  tasks,
  groups,
  isCollapsed,
}: {
  header: React.ReactNode;
  tasks: Task[];
  groups: TaskGroup[];
  isCollapsed: boolean;
}) {
  const { showArea, collapsedGroups, onToggleGroup } = useTaskTable();
  const colCount = showArea ? 8 : 7;

  // If no groups defined, render flat like before
  if (groups.length === 0) {
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

  // Group tasks by groupId
  const tasksByGroup = new Map<string, Task[]>();
  const ungroupedTasks: Task[] = [];
  for (const task of tasks) {
    if (task.groupId) {
      const arr = tasksByGroup.get(task.groupId) ?? [];
      arr.push(task);
      tasksByGroup.set(task.groupId, arr);
    } else {
      ungroupedTasks.push(task);
    }
  }

  return (
    <>
      <tr className="border-b border-white/[0.04]">
        <td colSpan={colCount} className="p-0">
          {header}
        </td>
      </tr>
      {!isCollapsed && (
        <>
          {groups.map((group) => {
            const groupTasks = tasksByGroup.get(group.id) ?? [];
            const isGroupCollapsed = collapsedGroups.has(group.id);

            return (
              <GroupSection
                key={group.id}
                group={group}
                tasks={groupTasks}
                isCollapsed={isGroupCollapsed}
                onToggle={() => onToggleGroup(group.id)}
              />
            );
          })}
          {ungroupedTasks.length > 0 && (
            <GroupSection
              key={UNGROUPED}
              group={null}
              tasks={ungroupedTasks}
              isCollapsed={collapsedGroups.has(UNGROUPED)}
              onToggle={() => onToggleGroup(UNGROUPED)}
            />
          )}
        </>
      )}
    </>
  );
}

/** A group section within a project: header + task rows */
function GroupSection({
  group,
  tasks,
  isCollapsed,
  onToggle,
}: {
  group: TaskGroup | null;
  tasks: Task[];
  isCollapsed: boolean;
  onToggle: () => void;
}) {
  const { showArea } = useTaskTable();
  const colCount = showArea ? 8 : 7;
  const name = group?.name ?? "Ungrouped";
  const color = group?.color;

  return (
    <>
      <tr className="border-b border-white/[0.04]">
        <td colSpan={colCount} className="p-0">
          <button
            type="button"
            onClick={onToggle}
            aria-expanded={!isCollapsed}
            className="w-full flex items-center gap-2 px-5 py-1.5 pl-10 bg-white/[0.01] hover:bg-white/[0.03] transition-colors text-left"
          >
            {isCollapsed ? (
              <ChevronRight className="w-3 h-3 text-dim flex-shrink-0" strokeWidth={1.5} />
            ) : (
              <ChevronDown className="w-3 h-3 text-dim flex-shrink-0" strokeWidth={1.5} />
            )}
            {color && (
              <span
                className="w-2 h-2 rounded-full flex-shrink-0"
                style={{ backgroundColor: color }}
              />
            )}
            <span className="text-[11px] font-light text-dim">{name}</span>
            <span className="text-[10px] text-dim/60 font-light">({tasks.length})</span>
          </button>
        </td>
      </tr>
      {!isCollapsed && tasks.map((task) => <TaskWithSubtasks key={task.id} task={task} />)}
    </>
  );
}
