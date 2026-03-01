import { ProjectHeader } from './ProjectHeader';
import { TaskRow } from './TaskRow';
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
  // Group tasks by project
  const tasksByProject = tasks.reduce((acc, task) => {
    if (!acc[task.project]) acc[task.project] = [];
    acc[task.project].push(task);
    return acc;
  }, {} as Record<string, Task[]>);

  return (
    <div className="mb-10">
      <div className="bg-[rgba(255,255,255,0.03)] backdrop-blur-sm rounded-xl overflow-hidden">
        {/* Table Header */}
        <div className={`grid ${activeTab === 'All' ? 'grid-cols-[40px_1fr_180px_100px_80px_100px_120px_140px]' : 'grid-cols-[40px_1fr_180px_80px_100px_120px_140px]'} gap-4 border-b border-[rgba(255,255,255,0.08)] text-[11px] text-[#8B949E] font-light px-6 py-3`}>
          <div></div>
          <div>Task</div>
          <div>Project</div>
          {activeTab === 'All' && <div>Area</div>}
          <div>Priority</div>
          <div>Status</div>
          <div>Due Date</div>
          <div>Tags</div>
        </div>

        {/* Table Body - Grouped by Project */}
        {Object.entries(tasksByProject).map(([projectId, projectTasks]) => {
          const project = projects.find(p => p.id === projectId);
          if (!project) return null;

          const isCollapsed = collapsedProjects.has(projectId);
          const projectObjectives = project.objectiveIds
            ? objectives.filter(obj => project.objectiveIds?.includes(obj.id))
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
                  activeTab={activeTab}
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
