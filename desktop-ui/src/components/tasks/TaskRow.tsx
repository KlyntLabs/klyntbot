import { useNavigate } from 'react-router';
import { ChevronRight, ChevronDown, Target } from 'lucide-react';
import { Checkbox } from '../ui/Checkbox';
import { Badge } from '../ui/Badge';
import { InlineTextEditor } from './editors/InlineTextEditor';
import { InlineSelect } from './editors/InlineSelect';
import { InlineTagsEditor } from './editors/InlineTagsEditor';
import { InlineDatePicker } from './editors/InlineDatePicker';
import { SubtaskProgress } from './SubtaskProgress';
import type { Task, Project, Area, TaskUpdateParams } from '../../lib/types';

interface TaskRowProps {
  task: Task;
  depth?: number;
  isExpanded?: boolean;
  projects: Project[];
  areas: Area[];
  isCompleted: boolean;
  showArea: boolean;
  onToggle: () => void;
  onToggleExpand?: () => void;
  onUpdate: (params: TaskUpdateParams) => void;
}

export function TaskRow({
  task,
  depth = 0,
  isExpanded = false,
  projects,
  areas,
  isCompleted,
  showArea,
  onToggle,
  onToggleExpand,
  onUpdate,
}: TaskRowProps) {
  const navigate = useNavigate();
  const isSubtask = depth > 0;
  const hasSubtasks = task.subtaskCount > 0;

  const cellPy = isSubtask ? 'py-1.5' : 'py-3';

  return (
    <tr
      onClick={() => navigate(`/task/${task.id}`)}
      className={`hover:bg-surface-base transition-colors border-b border-border-subtle last:border-b-0 cursor-pointer whitespace-nowrap ${
        isSubtask ? 'bg-surface-lowest relative' : ''
      }`}
      style={isSubtask ? { boxShadow: 'inset 3px 0 0 var(--brand)' } : undefined}
    >
      {/* Checkbox cell — with optional expand chevron */}
      <td className={`px-5 ${cellPy} w-9`} onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-1">
          {!isSubtask && hasSubtasks && (
            <button onClick={onToggleExpand} className="text-muted hover:text-secondary -ml-5 mr-0.5">
              {isExpanded
                ? <ChevronDown className="w-[14px] h-[14px]" strokeWidth={1.5} />
                : <ChevronRight className="w-[14px] h-[14px]" strokeWidth={1.5} />
              }
            </button>
          )}
          <Checkbox checked={isCompleted} onCheckedChange={onToggle} />
        </div>
      </td>

      {/* Title cell */}
      <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-1.5" style={isSubtask ? { paddingLeft: 24 } : undefined}>
          {task.objectiveId && (
            <Target className="w-[10px] h-[10px] text-brand flex-shrink-0" strokeWidth={1.5} />
          )}
          <InlineTextEditor
            value={task.title}
            onSave={(title) => onUpdate({ id: task.id, title })}
            className={`font-light ${isSubtask ? 'text-[12px]' : 'text-[13px]'} ${
              isCompleted ? 'text-dim line-through' : isSubtask ? 'text-muted' : 'text-secondary'
            }`}
          />
          {!isSubtask && hasSubtasks && (
            <SubtaskProgress total={task.subtaskCount} completed={task.subtaskCompletedCount} />
          )}
        </div>
      </td>

      {/* Project cell — hidden for subtasks (inherits from parent) */}
      <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
        {isSubtask ? null : (
          <InlineSelect
            value={task.projectId}
            options={[
              { value: null, label: 'No project' },
              ...projects.map(p => ({ value: p.id, label: p.name })),
            ]}
            onSelect={(val) => onUpdate({ id: task.id, projectId: val })}
            renderDisplay={(val) => {
              const proj = projects.find(p => p.id === val);
              if (!proj) return <span className="text-[11px] font-light text-dim">&mdash;</span>;
              return (
                <div className="inline-flex items-center gap-2 px-2 py-1 rounded-md bg-surface-base">
                  <div className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ backgroundColor: proj.color }} />
                  <span className="text-[11px] font-light text-muted">{proj.name}</span>
                </div>
              );
            }}
          />
        )}
      </td>

      {/* Area cell — hidden for subtasks (inherits from parent) */}
      {showArea && (
        <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
          {isSubtask ? null : (
            <InlineSelect
              value={task.areaId}
              options={areas.map(a => ({ value: a.id, label: a.name }))}
              onSelect={(val) => { if (val) onUpdate({ id: task.id, areaId: val }); }}
              renderDisplay={(val) => {
                const a = areas.find(ar => ar.id === val);
                return a ? <Badge variant="area" value={a.name} /> : <span className="text-[11px] text-dim">&mdash;</span>;
              }}
            />
          )}
        </td>
      )}

      {/* Priority cell */}
      <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
        <InlineSelect
          value={task.priority}
          options={[
            { value: 'P1', label: 'P1' },
            { value: 'P2', label: 'P2' },
            { value: 'P3', label: 'P3' },
            { value: 'P4', label: 'P4' },
            { value: null, label: 'None' },
          ]}
          onSelect={(val) => onUpdate({ id: task.id, priority: val ? Number(val.replace('P', '')) : null })}
          renderDisplay={(val) => val ? <Badge variant="priority" value={val} /> : <span className="text-[11px] text-dim">&mdash;</span>}
        />
      </td>

      {/* Status cell */}
      <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
        <InlineSelect
          value={task.status}
          options={[
            { value: 'todo', label: 'To Do' },
            { value: 'doing', label: 'In Progress' },
            { value: 'done', label: 'Done' },
          ]}
          onSelect={(val) => { if (val) onUpdate({ id: task.id, status: val }); }}
          renderDisplay={(val) => <Badge variant="status" value={val ?? 'todo'} />}
        />
      </td>

      {/* Due Date cell */}
      <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
        <InlineDatePicker
          value={task.dueDate}
          onSave={(val) => onUpdate({ id: task.id, dueDate: val })}
        />
      </td>

      {/* Tags cell */}
      <td className={`px-5 ${cellPy}`} onClick={e => e.stopPropagation()}>
        <InlineTagsEditor
          tags={task.tags}
          onSave={(tags) => onUpdate({ id: task.id, tags })}
        />
      </td>
    </tr>
  );
}
