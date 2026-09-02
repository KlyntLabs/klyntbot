import { Dialog } from "@shared/composites";
import type { useMutation } from "@shared/hooks/useMutation";
import type { Area, Task, TaskCreateParams } from "@shared/types/tasks";
import { Button } from "@shared/ui/Button";
import { useEffect, useState } from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import type { Priority, Status } from "../lib/mappers";
import { priorityToNumber } from "../lib/mappers";
import { priorities } from "../lib/priority-icons";
import { renderStatusIcon } from "../lib/status-utils";
import { useCreateIssueStore } from "../store/create-issue-store";

interface CreateIssueModalProps {
  onCreateTask: ReturnType<typeof useMutation<Task, TaskCreateParams>>;
  areas: Area[];
}

export function CreateIssueModal({ onCreateTask, areas }: CreateIssueModalProps) {
  const { statuses } = useStatusWorkflow();
  const { isOpen, defaultStatus, closeModal } = useCreateIssueStore();

  const [title, setTitle] = useState("");
  const [selectedStatus, setSelectedStatus] = useState<Status | null>(statuses[0] ?? null);
  const [selectedPriority, setSelectedPriority] = useState<Priority | null>(priorities[0] ?? null);

  useEffect(() => {
    if (!isOpen) return;
    setSelectedStatus(defaultStatus ?? statuses[0] ?? null);
    setTitle("");
    setSelectedPriority(priorities[0] ?? null);
  }, [isOpen, defaultStatus, statuses]);

  const handleClose = () => {
    closeModal();
  };

  const handleSubmit = () => {
    if (!title.trim() || !selectedStatus || !selectedPriority) return;

    const params: TaskCreateParams = {
      title: title.trim(),
      priority: priorityToNumber(selectedPriority.id) ?? undefined,
      areaId: areas[0]?.id,
    };

    onCreateTask.mutate(params);
    handleClose();
  };

  return (
    <Dialog open={isOpen} onClose={handleClose} title="Create Issue" size="lg">
      <div className="space-y-4">
        {/* Title */}
        <div className="space-y-2">
          <label htmlFor="issue-title" className="text-sm font-medium text-fg">
            Title <span className="text-status-danger">*</span>
          </label>
          <input
            id="issue-title"
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Issue title"
            className="w-full px-3 py-2 text-sm rounded-md border border-separator bg-bg text-fg placeholder:text-fg-secondary focus:outline-none focus:ring-2 focus:ring-fg-secondary/30"
            autoFocus
          />
        </div>

        {/* Status */}
        <div className="space-y-2">
          <span className="text-sm font-medium text-fg">Status</span>
          <div className="flex flex-wrap gap-1.5">
            {statuses.map((s) => (
              <button
                key={s.id}
                type="button"
                onClick={() => setSelectedStatus(s)}
                className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-ui-sm border transition-colors ${
                  selectedStatus?.id === s.id
                    ? "border-brand bg-brand/10 text-fg"
                    : "border-separator text-fg-secondary hover:bg-control-hover"
                }`}
              >
                <span className="flex items-center">{renderStatusIcon(s)}</span>
                {s.name}
              </button>
            ))}
          </div>
        </div>

        {/* Priority */}
        <div className="space-y-2">
          <span className="text-sm font-medium text-fg">Priority</span>
          <div className="flex flex-wrap gap-1.5">
            {priorities.map((p) => {
              const Icon = p.icon;
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => setSelectedPriority(p)}
                  className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-ui-sm border transition-colors ${
                    selectedPriority?.id === p.id
                      ? "border-brand bg-brand/10 text-fg"
                      : "border-separator text-fg-secondary hover:bg-control-hover"
                  }`}
                >
                  <Icon className="size-3.5" />
                  {p.name}
                </button>
              );
            })}
          </div>
        </div>

        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button variant="ghost" size="sm" onClick={handleClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={handleSubmit}
            disabled={!title.trim() || !selectedStatus || !selectedPriority}
          >
            Create Issue
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
