import { ChevronDown, ChevronRight, Target } from "lucide-react";
import { useNavigate } from "react-router";
import type { Task, TaskUpdateParams } from "../../lib/types";
import { Badge } from "../ui/Badge";
import { Checkbox } from "../ui/Checkbox";
import { InlineDatePicker } from "./editors/InlineDatePicker";
import { InlineSelect } from "./editors/InlineSelect";
import { InlineTagsEditor } from "./editors/InlineTagsEditor";
import { InlineTextEditor } from "./editors/InlineTextEditor";
import { SubtaskProgress } from "./SubtaskProgress";

const PRIORITY_OPTIONS = [
  { value: "P1", label: "P1" },
  { value: "P2", label: "P2" },
  { value: "P3", label: "P3" },
  { value: "P4", label: "P4" },
  { value: null, label: "None" },
] as const;

const STATUS_OPTIONS = [
  { value: "todo", label: "To Do" },
  { value: "doing", label: "In Progress" },
  { value: "done", label: "Done" },
] as const;

import { useTaskTable } from "./TaskTableContext";

interface RootTaskRowProps {
  task: Task;
  isExpanded: boolean;
  isCompleted: boolean;
  onToggle: () => void;
  onUpdate: (params: TaskUpdateParams) => void;
}

export function RootTaskRow({
  task,
  isExpanded,
  isCompleted,
  onToggle,
  onUpdate,
}: RootTaskRowProps) {
  const { projects, areas, showArea, onToggleExpandTask } = useTaskTable();
  const navigate = useNavigate();
  const hasSubtasks = task.subtaskCount > 0;

  return (
    <tr
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          navigate(`/task/${task.id}`);
        }
      }}
      onClick={() => navigate(`/task/${task.id}`)}
      className="hover:bg-white/[0.04] transition-colors border-b border-white/[0.04] last:border-b-0 cursor-pointer whitespace-nowrap"
    >
      <td
        className="px-5 py-2.5 w-9"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-1">
          {hasSubtasks && (
            <button
              type="button"
              onClick={() => onToggleExpandTask(task.id)}
              aria-expanded={isExpanded}
              aria-label="Toggle subtasks"
              className="text-muted hover:text-secondary -ml-5 mr-0.5"
            >
              {isExpanded ? (
                <ChevronDown className="w-[14px] h-[14px]" strokeWidth={1.5} />
              ) : (
                <ChevronRight className="w-[14px] h-[14px]" strokeWidth={1.5} />
              )}
            </button>
          )}
          <Checkbox checked={isCompleted} onCheckedChange={onToggle} />
        </div>
      </td>

      <td
        className="px-5 py-2.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-1.5 min-w-0">
          {task.objectiveId && (
            <Target className="w-[10px] h-[10px] text-brand flex-shrink-0" strokeWidth={1.5} />
          )}
          <InlineTextEditor
            value={task.title}
            onSave={(title) => onUpdate({ id: task.id, title })}
            className={`font-light text-[13px] ${isCompleted ? "text-dim line-through" : "text-secondary"}`}
          />
          {hasSubtasks && (
            <SubtaskProgress total={task.subtaskCount} completed={task.subtaskCompletedCount} />
          )}
        </div>
      </td>

      <td
        className="px-5 py-2.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineSelect
          value={task.projectId}
          options={[
            { value: null, label: "No project" },
            ...projects.map((p) => ({ value: p.id, label: p.name })),
          ]}
          onSelect={(val) => onUpdate({ id: task.id, projectId: val })}
          renderDisplay={(val) => {
            const proj = projects.find((p) => p.id === val);
            if (!proj) return <span className="text-[11px] font-light text-dim">&mdash;</span>;
            return (
              <div className="inline-flex items-center gap-2 px-2 py-1 rounded-md bg-white/[0.06]">
                <div
                  className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                  style={{ backgroundColor: proj.color }}
                />
                <span className="text-[11px] font-light text-muted">{proj.name}</span>
              </div>
            );
          }}
        />
      </td>

      {showArea && (
        <td
          className="px-5 py-2.5"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          <InlineSelect
            value={task.areaId}
            options={areas.map((a) => ({ value: a.id, label: a.name }))}
            onSelect={(val) => {
              if (val) onUpdate({ id: task.id, areaId: val });
            }}
            renderDisplay={(val) => {
              const a = areas.find((ar) => ar.id === val);
              return a ? (
                <Badge variant="area" value={a.name} />
              ) : (
                <span className="text-[11px] text-dim">&mdash;</span>
              );
            }}
          />
        </td>
      )}

      <td
        className="px-5 py-2.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineSelect
          value={task.priority}
          options={PRIORITY_OPTIONS}
          onSelect={(val) =>
            onUpdate({ id: task.id, priority: val ? Number(val.replace("P", "")) : null })
          }
          renderDisplay={(val) =>
            val ? (
              <Badge variant="priority" value={val} />
            ) : (
              <span className="text-[11px] text-dim">&mdash;</span>
            )
          }
        />
      </td>

      <td
        className="px-5 py-2.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineSelect
          value={task.status}
          options={STATUS_OPTIONS}
          onSelect={(val) => {
            if (val) onUpdate({ id: task.id, status: val });
          }}
          renderDisplay={(val) => <Badge variant="status" value={val ?? "todo"} />}
        />
      </td>

      <td
        className="px-5 py-2.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineDatePicker
          value={task.dueDate}
          onSave={(val) => onUpdate({ id: task.id, dueDate: val })}
        />
      </td>

      <td
        className="px-5 py-2.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineTagsEditor tags={task.tags} onSave={(tags) => onUpdate({ id: task.id, tags })} />
      </td>
    </tr>
  );
}

interface SubtaskRowProps {
  task: Task;
  isCompleted: boolean;
  onToggle: () => void;
  onUpdate: (params: TaskUpdateParams) => void;
}

export function SubtaskRow({ task, isCompleted, onToggle, onUpdate }: SubtaskRowProps) {
  const { showArea } = useTaskTable();
  const navigate = useNavigate();

  return (
    <tr
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          navigate(`/task/${task.id}`);
        }
      }}
      onClick={() => navigate(`/task/${task.id}`)}
      className="hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0 cursor-pointer whitespace-nowrap bg-white/[0.02] relative"
      style={{ boxShadow: "inset 3px 0 0 var(--brand)" }}
    >
      <td
        className="px-5 py-1.5 w-9"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-1">
          <Checkbox checked={isCompleted} onCheckedChange={onToggle} />
        </div>
      </td>

      <td
        className="px-5 py-1.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-1.5 pl-6 min-w-0">
          {task.objectiveId && (
            <Target className="w-[10px] h-[10px] text-brand flex-shrink-0" strokeWidth={1.5} />
          )}
          <InlineTextEditor
            value={task.title}
            onSave={(title) => onUpdate({ id: task.id, title })}
            className={`font-light text-[12px] ${isCompleted ? "text-dim line-through" : "text-muted"}`}
          />
        </div>
      </td>

      {/* Project cell — empty for subtasks (inherits from parent) */}
      <td className="px-5 py-1.5" />

      {/* Area cell — empty for subtasks */}
      {showArea && <td className="px-5 py-1.5" />}

      <td
        className="px-5 py-1.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineSelect
          value={task.priority}
          options={PRIORITY_OPTIONS}
          onSelect={(val) =>
            onUpdate({ id: task.id, priority: val ? Number(val.replace("P", "")) : null })
          }
          renderDisplay={(val) =>
            val ? (
              <Badge variant="priority" value={val} />
            ) : (
              <span className="text-[11px] text-dim">&mdash;</span>
            )
          }
        />
      </td>

      <td
        className="px-5 py-1.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineSelect
          value={task.status}
          options={STATUS_OPTIONS}
          onSelect={(val) => {
            if (val) onUpdate({ id: task.id, status: val });
          }}
          renderDisplay={(val) => <Badge variant="status" value={val ?? "todo"} />}
        />
      </td>

      <td
        className="px-5 py-1.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineDatePicker
          value={task.dueDate}
          onSave={(val) => onUpdate({ id: task.id, dueDate: val })}
        />
      </td>

      <td
        className="px-5 py-1.5"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.stopPropagation()}
      >
        <InlineTagsEditor tags={task.tags} onSave={(tags) => onUpdate({ id: task.id, tags })} />
      </td>
    </tr>
  );
}
