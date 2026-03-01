import { Target } from 'lucide-react';
import { Checkbox } from '../ui/Checkbox';
import { Badge } from '../ui/Badge';
import { taskGridCols } from '../../lib/utils';
import type { Task, Project } from '../../lib/types';

interface TaskRowProps {
  task: Task;
  project: Project;
  isCompleted: boolean;
  showArea: boolean;
  onToggle: () => void;
}

export function TaskRow({ task, project, isCompleted, showArea, onToggle }: TaskRowProps) {
  return (
    <div
      className={`grid ${taskGridCols(showArea)} gap-4 px-6 py-3 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0`}
    >
      {/* Checkbox */}
      <div className="flex items-center">
        <Checkbox checked={isCompleted} onCheckedChange={onToggle} />
      </div>

      {/* Task Title */}
      <div className="flex items-center gap-1.5">
        {task.objectiveId && (
          <Target className="w-[10px] h-[10px] text-brand flex-shrink-0" strokeWidth={1.5} />
        )}
        <span
          className={`text-[13px] font-light truncate ${
            isCompleted ? 'text-muted line-through' : 'text-secondary'
          }`}
        >
          {task.title}
        </span>
      </div>

      {/* Project Badge */}
      <div className="flex items-center">
        <div className="flex items-center gap-2 px-2 py-1 rounded-md bg-surface-base">
          <div
            className="w-1.5 h-1.5 rounded-full"
            style={{ backgroundColor: project.color }}
          />
          <span className="text-[11px] font-light text-muted">{project.name}</span>
        </div>
      </div>

      {/* Area Badge - Only when showing all */}
      {showArea && (
        <div className="flex items-center">
          <Badge variant="area" value={task.areaId} />
        </div>
      )}

      {/* Priority */}
      <div className="flex items-center">
        <Badge variant="priority" value={task.priority ?? ''} />
      </div>

      {/* Status */}
      <div className="flex items-center">
        <Badge variant="status" value={task.status} />
      </div>

      {/* Due Date */}
      <div className="flex items-center">
        <span className="text-[12px] text-muted font-light">{task.dueDate}</span>
      </div>

      {/* Tags */}
      <div className="flex items-center gap-1.5">
        {task.tags.map(tag => (
          <Badge key={tag} variant="tag" value={tag} />
        ))}
      </div>
    </div>
  );
}
