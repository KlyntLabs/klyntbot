import { Dialog } from "@shared/composites/Dialog/Dialog";
import { useMutation } from "@shared/hooks/useMutation";
import type { Objective, ObjectiveCreateParams, ObjectiveUpdateParams } from "@shared/types";
import { useCallback, useEffect, useState } from "react";
import { useProjectContext } from "../../contexts/ProjectContext";

interface ObjectiveCreateModalProps {
  open: boolean;
  onClose: () => void;
  /** If provided, the modal edits this objective instead of creating a new one. */
  editingObjective?: Objective;
}

export function ObjectiveCreateModal({
  open,
  onClose,
  editingObjective,
}: ObjectiveCreateModalProps) {
  const { project, refetchObjectives } = useProjectContext();

  const isEdit = !!editingObjective;

  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState("3");
  const [dueDate, setDueDate] = useState("");

  // Reset form when modal opens or editingObjective changes
  useEffect(() => {
    if (open) {
      if (editingObjective) {
        setTitle(editingObjective.title);
        setDescription(editingObjective.description ?? "");
        setPriority(String(editingObjective.priority ?? 3));
        setDueDate(editingObjective.dueDate ?? "");
      } else {
        setTitle("");
        setDescription("");
        setPriority("3");
        setDueDate("");
      }
    }
  }, [open, editingObjective]);

  const { mutate: createObjective, loading: creating } = useMutation<void, ObjectiveCreateParams>(
    "objective_create",
    "params",
  );
  const { mutate: updateObjective, loading: updating } = useMutation<void, ObjectiveUpdateParams>(
    "objective_update",
    "params",
  );

  const loading = creating || updating;

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!title.trim() || !project) return;

      if (isEdit && editingObjective) {
        const params: ObjectiveUpdateParams = {
          id: editingObjective.id,
          title: title.trim(),
        };
        if (description.trim()) params.description = description.trim();
        if (priority) params.priority = Number.parseInt(priority, 10);
        if (dueDate) params.dueDate = dueDate;
        await updateObjective(params);
      } else {
        const params: ObjectiveCreateParams = {
          title: title.trim(),
          projectId: project.id,
        };
        if (description.trim()) params.description = description.trim();
        if (priority) params.priority = Number.parseInt(priority, 10);
        if (dueDate) params.dueDate = dueDate;
        await createObjective(params);
      }

      // CRITICAL: OKR mutations don't auto-dispatch entity:updated events
      refetchObjectives();
      onClose();
    },
    [
      title,
      description,
      priority,
      dueDate,
      project,
      isEdit,
      editingObjective,
      createObjective,
      updateObjective,
      refetchObjectives,
      onClose,
    ],
  );

  return (
    <Dialog open={open} onClose={onClose} title={isEdit ? "Edit Objective" : "New Objective"}>
      <form onSubmit={handleSubmit} className="space-y-4">
        {/* Title */}
        <div>
          <label
            htmlFor="obj-title"
            className="block text-ui-sm font-medium text-fg-secondary mb-1"
          >
            Title <span className="text-red-400">*</span>
          </label>
          <input
            id="obj-title"
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="e.g. Improve API latency by 50%"
            autoFocus
            required
            className="w-full px-3 py-2 text-sm bg-transparent border border-separator rounded-md text-fg placeholder:text-fg-secondary focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
          />
        </div>

        {/* Description */}
        <div>
          <label
            htmlFor="obj-desc"
            className="block text-ui-sm font-medium text-fg-secondary mb-1"
          >
            Description
          </label>
          <textarea
            id="obj-desc"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Why this objective matters..."
            rows={3}
            className="w-full px-3 py-2 text-sm bg-transparent border border-separator rounded-md text-fg placeholder:text-fg-secondary focus:outline-none focus:ring-1 focus:ring-fg-secondary/30 resize-none"
          />
        </div>

        {/* Priority + Due date */}
        <div className="flex gap-3">
          <div className="flex-1">
            <label
              htmlFor="obj-priority"
              className="block text-ui-sm font-medium text-fg-secondary mb-1"
            >
              Priority
            </label>
            <select
              id="obj-priority"
              value={priority}
              onChange={(e) => setPriority(e.target.value)}
              className="w-full px-3 py-2 text-sm bg-transparent border border-separator rounded-md text-fg focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
            >
              <option value="1">1 - Critical</option>
              <option value="2">2 - High</option>
              <option value="3">3 - Medium</option>
              <option value="4">4 - Low</option>
              <option value="5">5 - Minimal</option>
            </select>
          </div>
          <div className="flex-1">
            <label
              htmlFor="obj-due"
              className="block text-ui-sm font-medium text-fg-secondary mb-1"
            >
              Due Date
            </label>
            <input
              id="obj-due"
              type="date"
              value={dueDate}
              onChange={(e) => setDueDate(e.target.value)}
              className="w-full px-3 py-2 text-sm bg-transparent border border-separator rounded-md text-fg focus:outline-none focus:ring-1 focus:ring-fg-secondary/30"
            />
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 pt-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 text-ui-sm font-medium text-fg-secondary hover:text-fg transition-colors"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={!title.trim() || loading}
            className="px-4 py-2 text-ui-sm font-medium rounded-md bg-brand text-white hover:bg-brand/90 disabled:opacity-50 transition-colors"
          >
            {loading ? "Saving..." : isEdit ? "Update Objective" : "Create Objective"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
