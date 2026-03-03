import { useMemo } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { ProjectHeader } from './ProjectHeader';
import { TaskRow } from './TaskRow';
import { taskGridCols } from '../../lib/utils';
import type { Task, Project, Objective, Tab } from '../../lib/types';

interface TaskTableProps {
  tasks: Task[];
  projects: Project[];
  objectives: Objective[];
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
  projects,
  objectives,
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

  const projectMap = useMemo(() =>
    new Map(projects.map(p => [p.id, p])),
  [projects]);

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
    <div className="mb-10">
      <div className="bg-surface-low backdrop-blur-sm rounded-xl overflow-hidden">
        {/* Table Header */}
        <div className={`grid ${taskGridCols(showArea)} gap-4 border-b border-border text-[11px] text-muted font-light px-6 py-3`}>
          <div></div>
          <div>Task</div>
          <div>Project</div>
          {showArea && <div>Area</div>}
          <div>Priority</div>
          <div>Status</div>
          <div>Due Date</div>
          <div>Tags</div>
        </div>

        {/* Table Body - Grouped by Project */}
        {sortedEntries.map(([projectId, projectTasks]) => {
          const project = projectMap.get(projectId);

          // Unassigned tasks group
          if (projectId === UNASSIGNED) {
            const isCollapsed = collapsedProjects.has(UNASSIGNED);
            return (
              <div key={UNASSIGNED}>
                <button
                  onClick={() => onToggleProject(UNASSIGNED)}
                  className="w-full flex items-center gap-3 px-6 py-3 bg-overlay hover:bg-overlay-heavy transition-colors text-left border-b border-border-subtle"
                >
                  {isCollapsed ? (
                    <ChevronRight className="w-[14px] h-[14px] text-muted flex-shrink-0" strokeWidth={1.5} />
                  ) : (
                    <ChevronDown className="w-[14px] h-[14px] text-muted flex-shrink-0" strokeWidth={1.5} />
                  )}
                  <span className="text-[12px] font-light text-muted">No Project</span>
                  <span className="text-[11px] text-dim font-light">({projectTasks.length})</span>
                </button>
                {!isCollapsed && projectTasks.map(task => (
                  <TaskRow
                    key={task.id}
                    task={task}
                    isCompleted={completedTasks.has(task.id)}
                    showArea={showArea}
                    onToggle={() => onToggleTask(task.id)}
                    onUpdatePriority={onUpdatePriority}
                    onRename={onRenameTask}
                  />
                ))}
              </div>
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
            <div key={projectId}>
              <ProjectHeader
                project={project}
                tasks={projectTasks}
                objectives={projectObjectives}
                isCollapsed={isCollapsed}
                onToggle={() => onToggleProject(projectId)}
              />
              {!isCollapsed && projectTasks.map(task => (
                <TaskRow
                  key={task.id}
                  task={task}
                  project={project}
                  isCompleted={completedTasks.has(task.id)}
                  showArea={showArea}
                  onToggle={() => onToggleTask(task.id)}
                  onUpdatePriority={onUpdatePriority}
                  onRename={onRenameTask}
                />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
