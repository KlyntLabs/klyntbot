import { useMemo } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { ProjectHeader } from './ProjectHeader';
import { TaskRow } from './TaskRow';
import type { Task, Project, Objective, Area, Tab } from '../../lib/types';

interface TaskTableProps {
  tasks: Task[];
  projectMap: Map<string, Project>;
  objectives: Objective[];
  areaMap: Map<string, Area>;
  activeTab: Tab;
  completedTasks: Set<string>;
  collapsedProjects: Set<string>;
  onToggleTask: (taskId: string) => void;
  onToggleProject: (projectId: string) => void;
  onUpdatePriority?: (taskId: string, priority: number | null) => void;
  onRenameTask?: (taskId: string, title: string) => void;
}

const UNASSIGNED = '_unassigned';

export function TaskTable({
  tasks,
  projectMap,
  objectives,
  areaMap,
  activeTab,
  completedTasks,
  collapsedProjects,
  onToggleTask,
  onToggleProject,
  onUpdatePriority,
  onRenameTask,
}: TaskTableProps) {
  const showArea = activeTab === 'All';

  const tasksByProject = useMemo(() =>
    tasks.reduce((acc, task) => {
      const key = task.projectId ?? UNASSIGNED;
      if (!acc[key]) acc[key] = [];
      acc[key].push(task);
      return acc;
    }, {} as Record<string, Task[]>),
  [tasks]);

  const objectiveMap = useMemo(() =>
    new Map(objectives.map(o => [o.id, o])),
  [objectives]);

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
    <div className="mb-10 overflow-x-auto rounded-xl">
      <table className="w-full bg-surface-low backdrop-blur-sm border-collapse">
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

            // Unassigned tasks group
            if (projectId === UNASSIGNED) {
              const isCollapsed = collapsedProjects.has(UNASSIGNED);
              return (
                <UnassignedGroup
                  key={UNASSIGNED}
                  tasks={projectTasks}
                  areaMap={areaMap}
                  completedTasks={completedTasks}
                  showArea={showArea}
                  isCollapsed={isCollapsed}
                  onToggle={() => onToggleProject(UNASSIGNED)}
                  onToggleTask={onToggleTask}
                  onUpdatePriority={onUpdatePriority}
                  onRenameTask={onRenameTask}
                />
              );
            }

            // Skip unknown projects
            if (!project) return null;

            const isCollapsed = collapsedProjects.has(projectId);
            const projectObjectives = project.objectiveIds
              ? project.objectiveIds.flatMap(id => {
                  const obj = objectiveMap.get(id);
                  return obj ? [obj] : [];
                })
              : [];

            return (
              <ProjectGroup
                key={projectId}
                project={project}
                tasks={projectTasks}
                objectives={projectObjectives}
                areaMap={areaMap}
                completedTasks={completedTasks}
                showArea={showArea}
                isCollapsed={isCollapsed}
                onToggle={() => onToggleProject(projectId)}
                onToggleTask={onToggleTask}
                onUpdatePriority={onUpdatePriority}
                onRenameTask={onRenameTask}
              />
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Project group: header row + task rows. */
function ProjectGroup({
  project, tasks, objectives, areaMap, completedTasks, showArea, isCollapsed,
  onToggle, onToggleTask, onUpdatePriority, onRenameTask,
}: {
  project: Project; tasks: Task[]; objectives: Objective[];
  areaMap: Map<string, Area>; completedTasks: Set<string>;
  showArea: boolean; isCollapsed: boolean;
  onToggle: () => void; onToggleTask: (id: string) => void;
  onUpdatePriority?: (id: string, p: number | null) => void;
  onRenameTask?: (id: string, t: string) => void;
}) {
  const colCount = showArea ? 8 : 7;
  return (
    <>
      <tr className="border-b border-border-subtle">
        <td colSpan={colCount} className="p-0">
          <ProjectHeader
            project={project}
            tasks={tasks}
            objectives={objectives}
            isCollapsed={isCollapsed}
            onToggle={onToggle}
          />
        </td>
      </tr>
      {!isCollapsed && tasks.map(task => (
        <TaskRow
          key={task.id}
          task={task}
          project={project}
          area={areaMap.get(task.areaId)}
          isCompleted={completedTasks.has(task.id)}
          showArea={showArea}
          onToggle={() => onToggleTask(task.id)}
          onUpdatePriority={onUpdatePriority}
          onRename={onRenameTask}
        />
      ))}
    </>
  );
}

/** Unassigned tasks group: header row + task rows. */
function UnassignedGroup({
  tasks, areaMap, completedTasks, showArea, isCollapsed,
  onToggle, onToggleTask, onUpdatePriority, onRenameTask,
}: {
  tasks: Task[];
  areaMap: Map<string, Area>; completedTasks: Set<string>;
  showArea: boolean; isCollapsed: boolean;
  onToggle: () => void; onToggleTask: (id: string) => void;
  onUpdatePriority?: (id: string, p: number | null) => void;
  onRenameTask?: (id: string, t: string) => void;
}) {
  const colCount = showArea ? 8 : 7;
  return (
    <>
      <tr className="border-b border-border-subtle">
        <td colSpan={colCount} className="p-0">
          <button
            onClick={onToggle}
            className="w-full flex items-center gap-3 px-5 py-3 bg-overlay hover:bg-overlay-heavy transition-colors text-left"
          >
            {isCollapsed ? (
              <ChevronRight className="w-[14px] h-[14px] text-muted flex-shrink-0" strokeWidth={1.5} />
            ) : (
              <ChevronDown className="w-[14px] h-[14px] text-muted flex-shrink-0" strokeWidth={1.5} />
            )}
            <span className="text-[12px] font-light text-muted">No Project</span>
            <span className="text-[11px] text-dim font-light">({tasks.length})</span>
          </button>
        </td>
      </tr>
      {!isCollapsed && tasks.map(task => (
        <TaskRow
          key={task.id}
          task={task}
          area={areaMap.get(task.areaId)}
          isCompleted={completedTasks.has(task.id)}
          showArea={showArea}
          onToggle={() => onToggleTask(task.id)}
          onUpdatePriority={onUpdatePriority}
          onRename={onRenameTask}
        />
      ))}
    </>
  );
}
