import type { useMutation } from "@shared/hooks/useMutation";
import type { Area, Task, TaskCreateParams } from "@shared/types/tasks";
import { useState } from "react";
import { useStatusWorkflow } from "../contexts/StatusWorkflowContext";
import type { Priority, Status } from "../lib/mappers";
import { priorityToNumber } from "../lib/mappers";
import { priorities } from "../lib/priority-icons";
import { renderStatusIcon } from "../lib/status-utils";
import { useCreateIssueStore } from "../store/create-issue-store";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";

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

  const handleOpenChange = (open: boolean) => {
    if (open) {
      // Sync form status from store when opening
      setSelectedStatus(defaultStatus ?? statuses[0] ?? null);
      setTitle("");
      setSelectedPriority(priorities[0] ?? null);
    } else {
      closeModal();
    }
  };

  const handleSubmit = () => {
    if (!title.trim() || !selectedStatus || !selectedPriority) return;

    const params: TaskCreateParams = {
      title: title.trim(),
      priority: priorityToNumber(selectedPriority.id) ?? undefined,
      areaId: areas[0]?.id,
    };

    onCreateTask.mutate(params);
    handleOpenChange(false);
  };

  return (
    <Dialog open={isOpen} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>Create Issue</DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {/* Title */}
          <div className="space-y-2">
            <label htmlFor="issue-title" className="text-sm font-medium text-foreground">
              Title <span className="text-destructive">*</span>
            </label>
            <input
              id="issue-title"
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Issue title"
              className="w-full px-3 py-2 text-sm rounded-md border border-border bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-brand"
              autoFocus
            />
          </div>

          {/* Status */}
          <div className="space-y-2">
            <span className="text-sm font-medium text-foreground">Status</span>
            <div className="flex flex-wrap gap-1.5">
              {statuses.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => setSelectedStatus(s)}
                  className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs border transition-colors ${
                    selectedStatus?.id === s.id
                      ? "border-primary bg-primary/10 text-foreground"
                      : "border-border text-muted-foreground hover:bg-accent"
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
            <span className="text-sm font-medium text-foreground">Priority</span>
            <div className="flex flex-wrap gap-1.5">
              {priorities.map((p) => {
                const Icon = p.icon;
                return (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => setSelectedPriority(p)}
                    className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs border transition-colors ${
                      selectedPriority?.id === p.id
                        ? "border-primary bg-primary/10 text-foreground"
                        : "border-border text-muted-foreground hover:bg-accent"
                    }`}
                  >
                    <Icon className="size-3.5" />
                    {p.name}
                  </button>
                );
              })}
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={() => handleOpenChange(false)}>
            Cancel
          </Button>
          <Button
            size="sm"
            onClick={handleSubmit}
            disabled={!title.trim() || !selectedStatus || !selectedPriority}
          >
            Create Issue
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
