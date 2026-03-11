import { formatHumanDuration, formatRelativeTime } from "@shared/lib/dates";
import type { Task, TaskUpdateParams } from "@shared/types";
import { Badge, Checkbox } from "@shared/ui";
import { ChevronDown, ChevronRight, Target } from "lucide-react";
import type { ColumnId } from "../hooks/useColumnVisibility";
import { InlineDatePicker } from "./editors/InlineDatePicker";
import { InlineNumber } from "./editors/InlineNumber";
import { InlineSelect } from "./editors/InlineSelect";
import { InlineTagsEditor } from "./editors/InlineTagsEditor";
import { InlineTextEditor } from "./editors/InlineTextEditor";
import { SubtaskProgress } from "./SubtaskProgress";
import { useTaskTable } from "./TaskTableContext";

const PRIORITY_OPTIONS = [
  { value: "P1", label: "P1" },
  { value: "P2", label: "P2" },
  { value: "P3", label: "P3" },
  { value: "P4", label: "P4" },
  { value: null, label: "None" },
];

const TASK_TYPE_OPTIONS = [
  { value: "manual", label: "Manual" },
  { value: "agentic", label: "Agentic" },
  { value: "hybrid", label: "Hybrid" },
];

const ENERGY_OPTIONS = [
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "deep", label: "Deep" },
];

const ENERGY_COLORS: Record<string, string> = {
  low: "bg-emerald-500/20 text-emerald-400",
  medium: "bg-yellow-500/20 text-yellow-400",
  high: "bg-orange-500/20 text-orange-400",
  deep: "bg-red-500/20 text-red-400",
};

const EXEC_STATE_COLORS: Record<string, string> = {
  idle: "bg-white/[0.06] text-dim",
  queued: "bg-blue-500/20 text-blue-400",
  running: "bg-brand/20 text-brand",
  paused: "bg-yellow-500/20 text-yellow-400",
  completed: "bg-emerald-500/20 text-emerald-400",
  failed: "bg-destructive/20 text-destructive",
};

interface RootTaskRowProps {
  task: Task;
  isExpanded: boolean;
  isCompleted: boolean;
  onToggle: () => void;
  onUpdate: (params: TaskUpdateParams) => void;
}

function ColumnCell({
  col,
  task,
  onUpdate,
}: {
  col: ColumnId;
  task: Task;
  onUpdate: (params: TaskUpdateParams) => void;
}) {
  const { projects, areas, statusLabels } = useTaskTable();

  switch (col) {
    case "project":
      return (
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
      );
    case "area":
      return (
        <InlineSelect
          value={task.areaId}
          options={areas.map((a) => ({ value: a.id, label: a.name }))}
          onSelect={(val) => {
            if (val) onUpdate({ id: task.id, areaId: val });
          }}
          renderDisplay={(val) => {
            const a = areas.find((ar) => ar.id === val);
            return a ? (
              <Badge variant="info">{a.name}</Badge>
            ) : (
              <span className="text-[11px] text-dim">&mdash;</span>
            );
          }}
        />
      );
    case "priority":
      return (
        <InlineSelect
          value={task.priority}
          options={PRIORITY_OPTIONS}
          onSelect={(val) =>
            onUpdate({ id: task.id, priority: val ? Number(val.replace("P", "")) : null })
          }
          renderDisplay={(val) =>
            val ? (
              <Badge variant="brand">{val}</Badge>
            ) : (
              <span className="text-[11px] text-dim">&mdash;</span>
            )
          }
        />
      );
    case "status": {
      const statusOptions = statusLabels.map((sl) => ({ value: sl.id, label: sl.name }));
      return (
        <InlineSelect
          value={task.statusLabelId}
          options={statusOptions}
          onSelect={(val) => {
            if (val) onUpdate({ id: task.id, statusLabelId: val });
          }}
          renderDisplay={(val) => {
            const label = statusLabels.find((sl) => sl.id === val);
            if (!label) return <Badge variant="info">{task.status ?? "todo"}</Badge>;
            return <Badge variant="info">{label.name}</Badge>;
          }}
        />
      );
    }
    case "dueDate":
      return (
        <InlineDatePicker
          value={task.dueDate}
          onSave={(val) => onUpdate({ id: task.id, dueDate: val })}
        />
      );
    case "tags":
      return (
        <InlineTagsEditor tags={task.tags} onSave={(tags) => onUpdate({ id: task.id, tags })} />
      );
    case "taskType":
      return (
        <InlineSelect
          value={task.taskType ?? null}
          options={[{ value: null, label: "\u2014" }, ...TASK_TYPE_OPTIONS]}
          onSelect={(val) =>
            onUpdate({ id: task.id, taskType: (val as Task["taskType"]) ?? undefined })
          }
          renderDisplay={(val) =>
            val ? (
              <Badge variant="default">{val}</Badge>
            ) : (
              <span className="text-[11px] text-dim">&mdash;</span>
            )
          }
        />
      );
    case "energyLevel":
      return (
        <InlineSelect
          value={task.energyLevel ?? null}
          options={[{ value: null, label: "\u2014" }, ...ENERGY_OPTIONS]}
          onSelect={(val) =>
            onUpdate({ id: task.id, energyLevel: (val as Task["energyLevel"]) ?? undefined })
          }
          renderDisplay={(val) =>
            val ? (
              <span
                className={`text-[10px] font-light px-1.5 py-0.5 rounded-md ${ENERGY_COLORS[val] ?? ""}`}
              >
                {val}
              </span>
            ) : (
              <span className="text-[11px] text-dim">&mdash;</span>
            )
          }
        />
      );
    case "estimatedMinutes":
      return (
        <InlineNumber
          value={task.estimatedMinutes}
          onSave={(val) => onUpdate({ id: task.id, estimatedMinutes: val })}
          suffix="m"
          min={0}
        />
      );
    case "actualMinutes":
      return (
        <span className="text-[11px] font-light text-muted">
          {task.actualMinutes != null ? `${task.actualMinutes}m` : "\u2014"}
        </span>
      );
    case "executionState":
      return task.executionState ? (
        <span
          className={`text-[10px] font-light px-1.5 py-0.5 rounded-md ${EXEC_STATE_COLORS[task.executionState] ?? ""}`}
        >
          {task.executionState}
        </span>
      ) : (
        <span className="text-[11px] text-dim">&mdash;</span>
      );
    case "complexityScore":
      return (
        <span className="text-[11px] font-light text-muted">
          {task.complexityScore != null ? task.complexityScore : "\u2014"}
        </span>
      );
    case "totalTrackedSecs":
      return (
        <span className="text-[11px] font-light text-muted">
          {task.totalTrackedSecs ? formatHumanDuration(task.totalTrackedSecs) : "\u2014"}
        </span>
      );
    case "focusedAt":
      return (
        <span className="text-[11px] font-light text-muted">
          {task.focusedAt ? formatRelativeTime(task.focusedAt) : "\u2014"}
        </span>
      );
  }
}

const COLUMN_ORDER: ColumnId[] = [
  "project",
  "area",
  "priority",
  "status",
  "dueDate",
  "tags",
  "taskType",
  "energyLevel",
  "estimatedMinutes",
  "actualMinutes",
  "executionState",
  "complexityScore",
  "totalTrackedSecs",
  "focusedAt",
];

function useVisibleColumnOrder() {
  const { showArea, visibleColumns } = useTaskTable();
  return COLUMN_ORDER.filter((col) => (col === "area" ? showArea : visibleColumns.has(col)));
}

export function RootTaskRow({
  task,
  isExpanded,
  isCompleted,
  onToggle,
  onUpdate,
}: RootTaskRowProps) {
  const { onToggleExpandTask, onSelectTask } = useTaskTable();
  const columns = useVisibleColumnOrder();
  const hasSubtasks = task.subtaskCount > 0;

  return (
    <tr
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelectTask(task.id);
        }
      }}
      onClick={() => onSelectTask(task.id)}
      className="hover:bg-white/[0.04] transition-colors border-b border-white/[0.04] last:border-b-0 cursor-pointer whitespace-nowrap"
    >
      <td className="px-5 py-2.5 w-9">
        {/* biome-ignore lint/a11y/useKeyWithClickEvents: stopPropagation only, not interactive */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: stopPropagation only, not interactive */}
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          {hasSubtasks && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onToggleExpandTask(task.id);
              }}
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

      <td className="px-5 py-2.5">
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

      {columns.map((col) => (
        <td key={col} className="px-5 py-2.5">
          <ColumnCell col={col} task={task} onUpdate={onUpdate} />
        </td>
      ))}
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
  const { onSelectTask } = useTaskTable();
  const columns = useVisibleColumnOrder();

  return (
    <tr
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelectTask(task.id);
        }
      }}
      onClick={() => onSelectTask(task.id)}
      className="hover:bg-white/[0.06] transition-colors border-b border-white/[0.04] last:border-b-0 cursor-pointer whitespace-nowrap bg-white/[0.02] relative"
      style={{ boxShadow: "inset 3px 0 0 var(--brand)" }}
    >
      <td className="px-5 py-1.5 w-9">
        {/* biome-ignore lint/a11y/useKeyWithClickEvents: stopPropagation only, not interactive */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: stopPropagation only, not interactive */}
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          <Checkbox checked={isCompleted} onCheckedChange={onToggle} />
        </div>
      </td>

      <td className="px-5 py-1.5">
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

      {columns.map((col) => {
        // Subtasks skip project and area (inherit from parent)
        if (col === "project" || col === "area") {
          return <td key={col} className="px-5 py-1.5" />;
        }
        return (
          <td key={col} className="px-5 py-1.5">
            <ColumnCell col={col} task={task} onUpdate={onUpdate} />
          </td>
        );
      })}
    </tr>
  );
}
