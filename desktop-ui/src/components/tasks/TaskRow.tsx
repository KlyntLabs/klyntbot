import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router';
import { Target } from 'lucide-react';
import { Checkbox } from '../ui/Checkbox';
import { Badge } from '../ui/Badge';
import { taskGridCols } from '../../lib/utils';
import type { Task, Project } from '../../lib/types';

const PRIORITIES = ['P1', 'P2', 'P3', 'P4', null] as const;

interface TaskRowProps {
  task: Task;
  project: Project;
  isCompleted: boolean;
  showArea: boolean;
  onToggle: () => void;
  onUpdatePriority?: (taskId: string, priority: number | null) => void;
  onRename?: (taskId: string, title: string) => void;
}

export function TaskRow({ task, project, isCompleted, showArea, onToggle, onUpdatePriority, onRename }: TaskRowProps) {
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');

  const cyclePriority = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (!onUpdatePriority) return;
    const currentIdx = PRIORITIES.indexOf(task.priority as typeof PRIORITIES[number]);
    const nextIdx = (currentIdx + 1) % PRIORITIES.length;
    const next = PRIORITIES[nextIdx];
    onUpdatePriority(task.id, next ? parseInt(next[1]) : null);
  }, [task.id, task.priority, onUpdatePriority]);

  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (!onRename) return;
    setDraft(task.title);
    setEditing(true);
  }, [task.title, onRename]);

  const saveRename = useCallback(() => {
    if (draft.trim() && draft !== task.title && onRename) {
      onRename(task.id, draft.trim());
    }
    setEditing(false);
  }, [draft, task.id, task.title, onRename]);

  return (
    <div
      onClick={() => navigate(`/task/${task.id}`)}
      className={`grid ${taskGridCols(showArea)} gap-4 px-6 py-3 hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0 cursor-pointer`}
    >
      {/* Checkbox */}
      <div className="flex items-center" onClick={e => e.stopPropagation()}>
        <Checkbox checked={isCompleted} onCheckedChange={onToggle} />
      </div>

      {/* Task Title */}
      <div className="flex items-center gap-1.5" onClick={editing ? e => e.stopPropagation() : undefined}>
        {task.objectiveId && (
          <Target className="w-[10px] h-[10px] text-brand flex-shrink-0" strokeWidth={1.5} />
        )}
        {editing ? (
          <input
            autoFocus
            value={draft}
            onChange={e => setDraft(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter') saveRename();
              if (e.key === 'Escape') setEditing(false);
            }}
            onBlur={saveRename}
            className="text-[13px] font-light text-primary bg-transparent border-b border-brand outline-none w-full"
          />
        ) : (
          <span
            onDoubleClick={handleDoubleClick}
            className={`text-[13px] font-light truncate ${
              isCompleted ? 'text-muted line-through' : 'text-secondary'
            }`}
          >
            {task.title}
          </span>
        )}
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

      {/* Priority — clickable to cycle */}
      <div className="flex items-center" onClick={cyclePriority}>
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
