import { useMemo } from 'react';
import { useNavigate } from 'react-router';
import { Badge } from '../ui/Badge';
import { formatDate } from '../../lib/dates';
import type { Task, Project, Area } from '../../lib/types';

interface KanbanBoardProps {
  tasks: Task[];
  projectMap: Map<string, Project>;
  areaMap: Map<string, Area>;
  completedTasks: Set<string>;
}

const COLUMNS = [
  { key: 'todo', label: 'To Do', accent: 'bg-info' },
  { key: 'doing', label: 'In Progress', accent: 'bg-brand' },
  { key: 'done', label: 'Done', accent: 'bg-success' },
] as const;

export function KanbanBoard({ tasks, projectMap, areaMap, completedTasks }: KanbanBoardProps) {
  const navigate = useNavigate();

  const columns = useMemo(() => {
    const grouped: Record<string, Task[]> = { todo: [], doing: [], done: [] };
    for (const task of tasks) {
      const status = task.status?.toLowerCase() ?? 'todo';
      if (status in grouped) grouped[status].push(task);
      else grouped.todo.push(task);
    }
    return grouped;
  }, [tasks]);

  return (
    <div className="flex gap-3 mb-10 min-h-0">
      {COLUMNS.map(({ key, label, accent }) => (
        <div key={key} className="flex-1 min-w-[240px] flex flex-col min-h-0">
          {/* Column header */}
          <div className="flex items-center gap-2.5 px-3 py-2.5 mb-2">
            <div className={`w-1.5 h-1.5 rounded-full ${accent}`} />
            <span className="text-[12px] font-light text-secondary">{label}</span>
            <span className="text-[11px] font-light text-dim">{columns[key].length}</span>
          </div>

          {/* Cards container */}
          <div className="flex-1 overflow-y-auto space-y-2 pr-0.5">
            {columns[key].map(task => {
              const project = task.projectId ? projectMap.get(task.projectId) : undefined;
              const area = areaMap.get(task.areaId);
              const isCompleted = completedTasks.has(task.id);

              return (
                <div
                  key={task.id}
                  onClick={() => navigate(`/task/${task.id}`)}
                  className="bg-surface-low hover:bg-surface-base rounded-lg px-4 py-3 cursor-pointer transition-colors border border-border-subtle"
                >
                  {/* Title */}
                  <p className={`text-[13px] font-light leading-snug mb-2 ${
                    isCompleted ? 'text-muted line-through' : 'text-secondary'
                  }`}>
                    {task.title}
                  </p>

                  {/* Meta row */}
                  <div className="flex items-center gap-1.5 flex-wrap">
                    {task.priority && (
                      <Badge variant="priority" value={task.priority} />
                    )}
                    {project && (
                      <div className="flex items-center gap-1.5 px-1.5 py-0.5 rounded bg-surface-base">
                        <div
                          className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                          style={{ backgroundColor: project.color }}
                        />
                        <span className="text-[10px] font-light text-muted">{project.name}</span>
                      </div>
                    )}
                    {area && (
                      <Badge variant="area" value={area.name} />
                    )}
                  </div>

                  {/* Tags */}
                  {task.tags.length > 0 && (
                    <div className="flex items-center gap-1 mt-2 flex-wrap">
                      {task.tags.map(tag => (
                        <Badge key={tag} variant="tag" value={tag} />
                      ))}
                    </div>
                  )}

                  {/* Due date */}
                  {task.dueDate && (
                    <p className="text-[10px] text-dim font-light mt-2">{formatDate(task.dueDate)}</p>
                  )}
                </div>
              );
            })}

            {columns[key].length === 0 && (
              <div className="flex items-center justify-center py-8">
                <p className="text-[11px] text-dim font-light">No tasks</p>
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
