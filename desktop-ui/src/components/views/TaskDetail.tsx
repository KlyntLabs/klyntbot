import { ArrowLeft, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { useEvent } from "../../hooks/useEvent";
import { useMutation } from "../../hooks/useMutation";
import { useQuery } from "../../hooks/useQuery";
import type { Project, Task, TaskUpdateParams } from "../../lib/types";
import { Badge } from "../ui/Badge";

const PRIORITIES = ["P1", "P2", "P3", "P4", null] as const;
const STATUSES = ["todo", "doing", "done"] as const;

export function TaskDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const { data: task, refetch } = useQuery<Task | null>("task_get", id ? { id } : null, null);
  const { data: projects } = useQuery<Project[]>("project_list", undefined, []);
  const updateTask = useMutation<Task, TaskUpdateParams>("task_update", "params");
  const deleteTask = useMutation<boolean, { id: string }>("task_delete");

  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [editingDesc, setEditingDesc] = useState(false);
  const [descDraft, setDescDraft] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEvent<{ entityKind: string; id: string }>("entity:updated", () => {
    refetch();
  });

  const handleUpdate = useCallback(
    async (params: Partial<TaskUpdateParams>) => {
      if (!id) return;
      await updateTask.mutate({ id, ...params });
    },
    [id, updateTask],
  );

  const handleDelete = useCallback(async () => {
    if (!id) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    await deleteTask.mutate({ id });
    navigate("/");
  }, [id, confirmDelete, deleteTask, navigate]);

  const cyclePriority = useCallback(() => {
    if (!task) return;
    const currentIdx = PRIORITIES.indexOf(task.priority as (typeof PRIORITIES)[number]);
    const nextIdx = (currentIdx + 1) % PRIORITIES.length;
    const next = PRIORITIES[nextIdx];
    handleUpdate({ priority: next ? parseInt(next[1], 10) : null });
  }, [task, handleUpdate]);

  if (!task) {
    return (
      <div className="h-screen w-screen bg-background text-primary flex items-center justify-center">
        <p className="text-muted text-sm font-light">Task not found</p>
      </div>
    );
  }

  return (
    <div className="h-screen w-screen bg-background text-primary flex flex-col gap-2 p-2 overflow-hidden">
      {/* Header */}
      <div className="h-12 flex items-center px-6 gap-3 shrink-0">
        <button
          type="button"
          onClick={() => navigate("/")}
          className="text-muted hover:text-secondary transition-colors"
        >
          <ArrowLeft className="w-4 h-4" strokeWidth={1.5} />
        </button>
        <span className="text-[12px] text-muted font-light">Task Detail</span>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6 max-w-2xl">
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
            className="text-[22px] font-light text-primary bg-transparent border-b border-brand outline-none w-full mb-4"
          />
        ) : (
          <button
            type="button"
            onClick={() => {
              setTitleDraft(task.title);
              setEditingTitle(true);
            }}
            className="text-[22px] font-light text-primary cursor-text mb-4 hover:text-secondary transition-colors text-left"
          >
            {task.title}
          </button>
        )}

        {/* Metadata Grid */}
        <div className="grid grid-cols-[120px_1fr] gap-y-4 gap-x-4 mb-6">
          {/* Status */}
          <span className="text-[12px] text-muted font-light self-center">Status</span>
          <div className="flex gap-2">
            {STATUSES.map((s) => (
              <button
                type="button"
                key={s}
                onClick={() => handleUpdate({ status: s })}
                className={`px-3 py-1 rounded-md text-[12px] font-light transition-colors ${
                  task.status === s
                    ? "bg-brand text-white"
                    : "bg-white/[0.04] text-muted hover:bg-white/[0.06]"
                }`}
              >
                {s}
              </button>
            ))}
          </div>

          {/* Priority */}
          <span className="text-[12px] text-muted font-light self-center">Priority</span>
          <button type="button" onClick={cyclePriority} className="self-start">
            <Badge variant="priority" value={task.priority ?? "—"} />
          </button>

          {/* Due Date */}
          <label htmlFor="task-due-date" className="text-[12px] text-muted font-light self-center">
            Due Date
          </label>
          <input
            id="task-due-date"
            type="date"
            value={task.dueDate ?? ""}
            onChange={(e) => handleUpdate({ dueDate: e.target.value || null })}
            className="bg-white/[0.04] rounded-md px-3 py-1.5 text-[12px] font-light text-secondary border border-white/[0.04] outline-none w-40"
          />

          {/* Project */}
          <label htmlFor="task-project" className="text-[12px] text-muted font-light self-center">
            Project
          </label>
          <select
            id="task-project"
            value={task.projectId ?? ""}
            onChange={(e) => handleUpdate({ projectId: e.target.value || null })}
            className="bg-white/[0.04] rounded-md px-3 py-1.5 text-[12px] font-light text-secondary border border-white/[0.04] outline-none w-48"
          >
            <option value="">No project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>

          {/* Tags */}
          <span className="text-[12px] text-muted font-light self-center">Tags</span>
          <div className="flex items-center gap-1.5">
            {task.tags.length > 0 ? (
              task.tags.map((tag) => <Badge key={tag} variant="tag" value={tag} />)
            ) : (
              <span className="text-[12px] text-dim font-light">None</span>
            )}
          </div>
        </div>

        {/* Description */}
        <div className="mb-6">
          <span className="text-[12px] text-muted font-light block mb-2">Description</span>
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
              rows={5}
              className="w-full bg-white/[0.04] rounded-lg px-4 py-3 text-[13px] font-light text-secondary border border-white/[0.04] outline-none resize-none"
            />
          ) : (
            <button
              type="button"
              onClick={() => {
                setDescDraft(task.description ?? "");
                setEditingDesc(true);
              }}
              className="w-full bg-white/[0.04] rounded-lg px-4 py-3 text-[13px] font-light text-secondary min-h-[80px] cursor-text hover:bg-white/[0.06] transition-colors text-left"
            >
              {task.description || <span className="text-dim">Click to add description...</span>}
            </button>
          )}
        </div>

        {/* Delete */}
        <div className="pt-4 border-t border-white/[0.08]">
          <button
            type="button"
            onClick={handleDelete}
            onBlur={() => setConfirmDelete(false)}
            className={`flex items-center gap-2 px-3 py-2 rounded-md text-[12px] font-light transition-colors ${
              confirmDelete
                ? "bg-destructive text-white"
                : "text-muted hover:text-destructive hover:bg-white/[0.04]"
            }`}
          >
            <Trash2 className="w-3.5 h-3.5" strokeWidth={1.5} />
            {confirmDelete ? "Click again to delete" : "Delete task"}
          </button>
        </div>
      </div>
    </div>
  );
}
