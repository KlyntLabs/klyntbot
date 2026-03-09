import { Maximize2, Trash2, X } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import {
  useColumnMutations,
  useColumnValues,
  useCustomColumns,
} from "../../hooks/useCustomColumns";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import { useEffectiveLabels } from "../../hooks/useWorkflows";
import type { Project, StatusLabel, Task, TaskUpdateParams } from "../../lib/types";
import { LinkedNotes } from "../notes/LinkedNotes";
import { Badge } from "../ui/Badge";
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
            <Badge variant="priority" value={task.priority ?? "\u2014"} />
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
              task.tags.map((tag) => <Badge key={tag} variant="tag" value={tag} />)
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
