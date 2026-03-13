import { LinkedNotes } from "@features/notes/components/LinkedNotes";
import {
  useColumnMutations,
  useColumnValues,
  useCustomColumns,
} from "@shared/hooks/useCustomColumns";
import { useEvent } from "@shared/hooks/useEvent";
import { ipc } from "@shared/hooks/useIpc";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { useEffectiveLabels } from "@shared/hooks/useWorkflows";
import { formatHumanDuration, formatRelativeTime } from "@shared/lib/dates";
import type { Project, StatusLabel, Task, TaskUpdateParams } from "@shared/types";
import { Badge } from "@shared/ui";
import { ChevronDown, ChevronRight, Maximize2, Trash2, X } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { CustomColumnCell } from "./CustomColumnCell";

const PRIORITIES = ["P1", "P2", "P3", "P4", null] as const;

interface TaskDetailPanelProps {
  taskId: string;
  onClose: () => void;
}

export function TaskDetailPanel({ taskId, onClose }: TaskDetailPanelProps) {
  const navigate = useNavigate();

  const { data: task, refetch } = useQuery<Task | null>("task_get", { id: taskId }, null);
  const { data: projects } = useQuery<Project[]>("project_list", undefined, []);
  const { data: statusLabels } = useEffectiveLabels(null);
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const deleteTask = useMutation<boolean, { id: string }>("task_delete");

  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [editingDesc, setEditingDesc] = useState(false);
  const [descDraft, setDescDraft] = useState("");
  const [editingCriteria, setEditingCriteria] = useState(false);
  const [criteriaDraft, setCriteriaDraft] = useState("");
  const [agenticExpanded, setAgenticExpanded] = useState(() => {
    try {
      return localStorage.getItem("klyntbot:tasks:agenticExpanded") !== "false";
    } catch {
      return true;
    }
  });
  const [confirmDelete, setConfirmDelete] = useState(false);

  const { data: customColumns } = useCustomColumns(task?.projectId ?? null);
  const { data: columnValues } = useColumnValues(taskId);
  const { setValue: setColumnValue } = useColumnMutations();

  const columnValueMap = useMemo(() => {
    const map = new Map<string, unknown>();
    for (const cv of columnValues) {
      map.set(cv.columnId, cv.value);
    }
    return map;
  }, [columnValues]);

  useEvent<{ entityKind: string; id: string }>("entity:updated", () => {
    refetch();
  });

  const handleUpdate = useCallback(
    async (params: Partial<TaskUpdateParams>) => {
      await updateTask.mutate({ id: taskId, ...params });
    },
    [taskId, updateTask],
  );

  const handleDelete = useCallback(async () => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    await deleteTask.mutate({ id: taskId });
    onClose();
  }, [taskId, confirmDelete, deleteTask, onClose]);

  const cyclePriority = useCallback(() => {
    if (!task) return;
    const currentIdx = PRIORITIES.indexOf(task.priority as (typeof PRIORITIES)[number]);
    const nextIdx = (currentIdx + 1) % PRIORITIES.length;
    const next = PRIORITIES[nextIdx];
    handleUpdate({ priority: next ? parseInt(next[1], 10) : null });
  }, [task, handleUpdate]);

  if (!task) {
    return (
      <div className="w-[380px] shrink-0 glass-sidebar flex items-center justify-center">
        <p className="text-muted text-sm font-light">Loading...</p>
      </div>
    );
  }

  return (
    <div className="w-[380px] shrink-0 glass-sidebar flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/[0.06] shrink-0">
        <span className="text-[12px] text-muted font-light">Task Detail</span>
        <div className="flex items-center gap-1">
          {!task.focusedAt && (
            <button
              type="button"
              onClick={() => ipc("task_start_focus", { taskId })}
              className="px-2 py-1 rounded text-[11px] text-muted hover:text-primary hover:bg-white/[0.06] transition-colors"
            >
              Focus
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded text-muted hover:text-secondary hover:bg-white/[0.06] transition-colors"
          >
            <X className="w-3.5 h-3.5" strokeWidth={1.5} />
          </button>
        </div>
      </div>

      {/* Scrollable Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {/* Title */}
        {editingTitle ? (
          <input
            value={titleDraft}
            onChange={(e) => setTitleDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                handleUpdate({ title: titleDraft });
                setEditingTitle(false);
              }
              if (e.key === "Escape") setEditingTitle(false);
            }}
            onBlur={() => {
              if (titleDraft !== task.title) handleUpdate({ title: titleDraft });
              setEditingTitle(false);
            }}
            className="text-[16px] font-light text-primary bg-transparent border-b border-brand outline-none w-full mb-4"
          />
        ) : (
          <button
            type="button"
            onClick={() => {
              setTitleDraft(task.title);
              setEditingTitle(true);
            }}
            className="text-[16px] font-light text-primary cursor-text mb-4 hover:text-secondary transition-colors text-left w-full"
          >
            {task.title}
          </button>
        )}

        {/* Metadata Grid */}
        <div className="grid grid-cols-[90px_1fr] gap-y-3 gap-x-3 mb-5">
          {/* Status */}
          <span className="text-[11px] text-muted font-light self-center">Status</span>
          <div className="flex gap-1.5 flex-wrap">
            {statusLabels.map((sl: StatusLabel) => (
              <button
                type="button"
                key={sl.id}
                onClick={() => handleUpdate({ statusLabelId: sl.id })}
                className={`px-2 py-0.5 rounded-md text-[11px] font-light transition-colors ${
                  task.statusLabelId === sl.id
                    ? "text-white"
                    : "bg-white/[0.04] text-muted hover:bg-white/[0.06]"
                }`}
                style={task.statusLabelId === sl.id ? { backgroundColor: sl.color } : undefined}
              >
                {sl.name}
              </button>
            ))}
          </div>

          {/* Priority */}
          <span className="text-[11px] text-muted font-light self-center">Priority</span>
          <button type="button" onClick={cyclePriority} className="self-start">
            <Badge variant="brand">{task.priority ?? "\u2014"}</Badge>
          </button>

          {/* Due Date */}
          <label htmlFor="panel-due-date" className="text-[11px] text-muted font-light self-center">
            Due Date
          </label>
          <input
            id="panel-due-date"
            type="date"
            value={task.dueDate ?? ""}
            onChange={(e) => handleUpdate({ dueDate: e.target.value || null })}
            className="bg-white/[0.04] rounded-md px-2 py-1 text-[11px] font-light text-secondary border border-white/[0.04] outline-none w-36"
          />

          {/* Project */}
          <label htmlFor="panel-project" className="text-[11px] text-muted font-light self-center">
            Project
          </label>
          <select
            id="panel-project"
            value={task.projectId ?? ""}
            onChange={(e) => handleUpdate({ projectId: e.target.value || null })}
            className="bg-white/[0.04] rounded-md px-2 py-1 text-[11px] font-light text-secondary border border-white/[0.04] outline-none w-40"
          >
            <option value="">No project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>

          {/* Tags */}
          <span className="text-[11px] text-muted font-light self-center">Tags</span>
          <div className="flex items-center gap-1">
            {task.tags.length > 0 ? (
              task.tags.map((tag) => (
                <Badge key={tag} variant="default">
                  {tag}
                </Badge>
              ))
            ) : (
              <span className="text-[11px] text-dim font-light">None</span>
            )}
          </div>
        </div>

        {/* Description */}
        <div className="mb-5">
          <span className="text-[11px] text-muted font-light block mb-1.5">Description</span>
          {editingDesc ? (
            <textarea
              value={descDraft}
              onChange={(e) => setDescDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setEditingDesc(false);
              }}
              onBlur={() => {
                handleUpdate({ description: descDraft || null });
                setEditingDesc(false);
              }}
              rows={4}
              className="w-full bg-white/[0.04] rounded-lg px-3 py-2 text-[12px] font-light text-secondary border border-white/[0.04] outline-none resize-none"
            />
          ) : (
            <button
              type="button"
              onClick={() => {
                setDescDraft(task.description ?? "");
                setEditingDesc(true);
              }}
              className="w-full bg-white/[0.04] rounded-lg px-3 py-2 text-[12px] font-light text-secondary min-h-[60px] cursor-text hover:bg-white/[0.06] transition-colors text-left"
            >
              {task.description || <span className="text-dim">Click to add description...</span>}
            </button>
          )}
        </div>

        {/* Agentic Section */}
        <div className="mb-5">
          <button
            type="button"
            onClick={() => {
              const next = !agenticExpanded;
              setAgenticExpanded(next);
              try {
                localStorage.setItem("klyntbot:tasks:agenticExpanded", String(next));
              } catch {}
            }}
            className="flex items-center gap-1.5 mb-2 group"
          >
            {agenticExpanded ? (
              <ChevronDown className="w-3 h-3 text-dim" strokeWidth={1.5} />
            ) : (
              <ChevronRight className="w-3 h-3 text-dim" strokeWidth={1.5} />
            )}
            <span className="text-[11px] text-muted font-light group-hover:text-secondary transition-colors">
              Agentic
            </span>
          </button>

          {agenticExpanded && (
            <>
              <div className="grid grid-cols-[90px_1fr] gap-y-3 gap-x-3 mb-3">
                {/* Task Type */}
                <span className="text-[11px] text-muted font-light self-center">Type</span>
                <div className="flex gap-1.5">
                  {(["manual", "agentic", "hybrid"] as const).map((t) => (
                    <button
                      type="button"
                      key={t}
                      onClick={() => handleUpdate({ taskType: t })}
                      className={`px-2 py-0.5 rounded-md text-[11px] font-light transition-colors ${
                        task.taskType === t
                          ? "bg-brand text-white"
                          : "bg-white/[0.04] text-muted hover:bg-white/[0.06]"
                      }`}
                    >
                      {t[0].toUpperCase() + t.slice(1)}
                    </button>
                  ))}
                </div>

                {/* Execution State */}
                <span className="text-[11px] text-muted font-light self-center">Execution</span>
                <span
                  className={`text-[11px] font-light px-2 py-0.5 rounded-md self-start w-fit ${
                    task.executionState === "running"
                      ? "bg-brand/20 text-brand"
                      : task.executionState === "failed"
                        ? "bg-destructive/20 text-destructive"
                        : task.executionState === "completed"
                          ? "bg-emerald-500/20 text-emerald-400"
                          : task.executionState === "queued"
                            ? "bg-blue-500/20 text-blue-400"
                            : task.executionState === "paused"
                              ? "bg-yellow-500/20 text-yellow-400"
                              : "bg-white/[0.04] text-dim"
                  }`}
                >
                  {task.executionState ?? "idle"}
                </span>

                {/* Energy Level */}
                <span className="text-[11px] text-muted font-light self-center">Energy</span>
                <div className="flex gap-1.5">
                  {(["low", "medium", "high", "deep"] as const).map((e) => {
                    const colors: Record<string, string> = {
                      low: "bg-emerald-500 text-white",
                      medium: "bg-yellow-500 text-white",
                      high: "bg-orange-500 text-white",
                      deep: "bg-red-500 text-white",
                    };
                    return (
                      <button
                        type="button"
                        key={e}
                        onClick={() => handleUpdate({ energyLevel: e })}
                        className={`px-2 py-0.5 rounded-md text-[11px] font-light transition-colors ${
                          task.energyLevel === e
                            ? colors[e]
                            : "bg-white/[0.04] text-muted hover:bg-white/[0.06]"
                        }`}
                      >
                        {e[0].toUpperCase() + e.slice(1)}
                      </button>
                    );
                  })}
                </div>

                {/* Estimated / Actual */}
                <span className="text-[11px] text-muted font-light self-center">Est / Act</span>
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    value={task.estimatedMinutes ?? ""}
                    onChange={(e) =>
                      handleUpdate({
                        estimatedMinutes: e.target.value ? Number(e.target.value) : null,
                      })
                    }
                    placeholder="\u2014"
                    min={0}
                    className="bg-white/[0.04] rounded-md px-2 py-1 text-[11px] font-light text-secondary border border-white/[0.04] outline-none w-16"
                  />
                  <span className="text-[11px] text-dim font-light">m</span>
                  <span className="text-[11px] text-dim font-light">/</span>
                  <span className="text-[11px] font-light text-muted">
                    {task.actualMinutes != null ? `${task.actualMinutes}m` : "\u2014"}
                  </span>
                </div>

                {/* Time Tracked */}
                <span className="text-[11px] text-muted font-light self-center">Tracked</span>
                <span className="text-[11px] font-light text-muted">
                  {task.totalTrackedSecs ? formatHumanDuration(task.totalTrackedSecs) : "\u2014"}
                </span>

                {/* Complexity Score */}
                <span className="text-[11px] text-muted font-light self-center">Complexity</span>
                <span className="text-[11px] font-light text-muted">
                  {task.complexityScore != null ? `${task.complexityScore} / 100` : "\u2014"}
                </span>

                {/* Focused At */}
                <span className="text-[11px] text-muted font-light self-center">Focused</span>
                <span className="text-[11px] font-light text-muted">
                  {task.focusedAt ? formatRelativeTime(task.focusedAt) : "\u2014"}
                </span>
              </div>

              {/* Acceptance Criteria */}
              <span className="text-[11px] text-muted font-light block mb-1.5">
                Acceptance Criteria
              </span>
              {editingCriteria ? (
                <textarea
                  value={criteriaDraft}
                  onChange={(e) => setCriteriaDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") setEditingCriteria(false);
                  }}
                  onBlur={() => {
                    handleUpdate({ acceptanceCriteria: criteriaDraft || null });
                    setEditingCriteria(false);
                  }}
                  rows={3}
                  className="w-full bg-white/[0.04] rounded-lg px-3 py-2 text-[12px] font-light text-secondary border border-white/[0.04] outline-none resize-none"
                />
              ) : (
                <button
                  type="button"
                  onClick={() => {
                    setCriteriaDraft(task.acceptanceCriteria ?? "");
                    setEditingCriteria(true);
                  }}
                  className="w-full bg-white/[0.04] rounded-lg px-3 py-2 text-[12px] font-light text-secondary min-h-[48px] cursor-text hover:bg-white/[0.06] transition-colors text-left"
                >
                  {task.acceptanceCriteria || (
                    <span className="text-dim">Click to add criteria...</span>
                  )}
                </button>
              )}
            </>
          )}
        </div>

        {/* Custom Fields */}
        {customColumns.length > 0 && (
          <div className="mb-5">
            <span className="text-[11px] text-muted font-light block mb-2">Custom Fields</span>
            <div className="grid grid-cols-[90px_1fr] gap-y-2 gap-x-3">
              {customColumns.map((col) => (
                <div key={col.id} className="contents">
                  <span className="text-[11px] text-muted font-light self-center">{col.name}</span>
                  <div className="self-center">
                    <CustomColumnCell
                      taskId={task.id}
                      column={col}
                      value={columnValueMap.get(col.id) ?? null}
                      onSetValue={(params) => setColumnValue.mutate(params)}
                    />
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Linked Notes */}
        <LinkedNotes entityType="task" entityId={taskId} />

        {/* Delete */}
        <div className="pt-3 border-t border-white/[0.08] mt-5">
          <button
            type="button"
            onClick={handleDelete}
            onBlur={() => setConfirmDelete(false)}
            className={`flex items-center gap-2 px-2 py-1.5 rounded-md text-[11px] font-light transition-colors ${
              confirmDelete
                ? "bg-destructive text-white"
                : "text-muted hover:text-destructive hover:bg-white/[0.04]"
            }`}
          >
            <Trash2 className="w-3 h-3" strokeWidth={1.5} />
            {confirmDelete ? "Click again to delete" : "Delete task"}
          </button>
        </div>
      </div>

      {/* Expand Button */}
      <div className="shrink-0 border-t border-white/[0.06] px-4 py-3">
        <button
          type="button"
          onClick={() => navigate(`/task/${taskId}`)}
          className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-muted hover:text-secondary text-[12px] font-light transition-colors"
        >
          <Maximize2 className="w-3.5 h-3.5" strokeWidth={1.5} />
          Expand full view
        </button>
      </div>
    </div>
  );
}
