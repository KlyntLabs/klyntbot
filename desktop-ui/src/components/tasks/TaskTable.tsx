import { useMemo } from 'react';
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
}

export function TaskTable({
  tasks,
  projects,
  objectives,
  activeTab,
  completedTasks,
  collapsedProjects,
  onToggleTask,
  onToggleProject,
}: TaskTableProps) {
  const showArea = activeTab === 'All';

  const tasksByProject = useMemo(() =>
    tasks.reduce((acc, task) => {
      const key = task.projectId ?? '_unassigned';
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
        {Object.entries(tasksByProject).map(([projectId, projectTasks]) => {
          const project = projectMap.get(projectId);
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
                />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
