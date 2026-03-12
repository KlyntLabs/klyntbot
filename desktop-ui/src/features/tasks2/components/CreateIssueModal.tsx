import { useState } from "react";
import { renderStatusIcon } from "../lib/status-utils";
import type { LabelInterface } from "../mock-data/labels";
import { labels } from "../mock-data/labels";
import type { Priority } from "../mock-data/priorities";
import { priorities } from "../mock-data/priorities";
import type { Status } from "../mock-data/status";
import { status as allStatus } from "../mock-data/status";
import type { User } from "../mock-data/users";
import { users } from "../mock-data/users";
import { useCreateIssueStore } from "../store/create-issue-store";
import { useIssuesStore } from "../store/issues-store";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";

export function CreateIssueModal() {
  const { isOpen, defaultStatus, closeModal } = useCreateIssueStore();
  const addIssue = useIssuesStore((s) => s.addIssue);

  const [title, setTitle] = useState("");
  const [selectedStatus, setSelectedStatus] = useState<Status>(allStatus[0]);
  const [selectedPriority, setSelectedPriority] = useState<Priority>(priorities[0]);
  const [selectedAssignee, setSelectedAssignee] = useState<User | null>(null);
  const [selectedLabels, setSelectedLabels] = useState<LabelInterface[]>([]);

  const handleOpenChange = (open: boolean) => {
    if (open) {
      // Sync form status from store when opening
      setSelectedStatus(defaultStatus ?? allStatus[0]);
      setTitle("");
      setSelectedPriority(priorities[0]);
      setSelectedAssignee(null);
      setSelectedLabels([]);
    } else {
      closeModal();
    }
  };

  const handleSubmit = () => {
    if (!title.trim()) return;

    const newIssue = {
      id: `new-${Date.now()}`,
      identifier: `LNUI-${Math.floor(Math.random() * 900) + 100}`,
      title: title.trim(),
      description: "",
      status: selectedStatus,
      assignee: selectedAssignee,
      priority: selectedPriority,
      labels: selectedLabels,
      createdAt: new Date().toISOString(),
      cycleId: "cycle-1",
      rank: `0|new-${Date.now()}:`,
    };

    addIssue(newIssue);
    handleOpenChange(false);
  };

  const toggleLabel = (label: LabelInterface) => {
    setSelectedLabels((prev) =>
      prev.some((l) => l.id === label.id)
        ? prev.filter((l) => l.id !== label.id)
        : [...prev, label],
    );
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
            <label
              htmlFor="issue-title"
              className="text-sm font-medium text-[hsl(var(--foreground))]"
            >
              Title <span className="text-[hsl(var(--destructive))]">*</span>
            </label>
            <input
              id="issue-title"
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Issue title"
              className="w-full px-3 py-2 text-sm rounded-md border border-[hsl(var(--border))] bg-[hsl(var(--background))] text-[hsl(var(--foreground))] placeholder:text-[hsl(var(--muted-foreground))] focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))]"
              autoFocus
            />
          </div>

          {/* Status */}
          <div className="space-y-2">
            <span className="text-sm font-medium text-[hsl(var(--foreground))]">Status</span>
            <div className="flex flex-wrap gap-1.5">
              {allStatus.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => setSelectedStatus(s)}
                  className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs border transition-colors ${
                    selectedStatus.id === s.id
                      ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))]/10 text-[hsl(var(--foreground))]"
                      : "border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]"
                  }`}
                >
                  <span className="flex items-center">{renderStatusIcon(s.id)}</span>
                  {s.name}
                </button>
              ))}
            </div>
          </div>

          {/* Priority */}
          <div className="space-y-2">
            <span className="text-sm font-medium text-[hsl(var(--foreground))]">Priority</span>
            <div className="flex flex-wrap gap-1.5">
              {priorities.map((p) => {
                const Icon = p.icon;
                return (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => setSelectedPriority(p)}
                    className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs border transition-colors ${
                      selectedPriority.id === p.id
                        ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))]/10 text-[hsl(var(--foreground))]"
                        : "border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]"
                    }`}
                  >
                    <Icon className="size-3.5" />
                    {p.name}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Assignee */}
          <div className="space-y-2">
            <span className="text-sm font-medium text-[hsl(var(--foreground))]">Assignee</span>
            <div className="flex flex-wrap gap-1.5">
              <button
                type="button"
                onClick={() => setSelectedAssignee(null)}
                className={`px-2.5 py-1 rounded-md text-xs border transition-colors ${
                  selectedAssignee === null
                    ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))]/10 text-[hsl(var(--foreground))]"
                    : "border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]"
                }`}
              >
                Unassigned
              </button>
              {users.map((user) => (
                <button
                  key={user.id}
                  type="button"
                  onClick={() => setSelectedAssignee(user)}
                  className={`px-2.5 py-1 rounded-md text-xs border transition-colors ${
                    selectedAssignee?.id === user.id
                      ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))]/10 text-[hsl(var(--foreground))]"
                      : "border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]"
                  }`}
                >
                  {user.name}
                </button>
              ))}
            </div>
          </div>

          {/* Labels */}
          <div className="space-y-2">
            <span className="text-sm font-medium text-[hsl(var(--foreground))]">Labels</span>
            <div className="flex flex-wrap gap-1.5">
              {labels.map((label) => {
                const isSelected = selectedLabels.some((l) => l.id === label.id);
                return (
                  <button
                    key={label.id}
                    type="button"
                    onClick={() => toggleLabel(label)}
                    className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs border transition-colors ${
                      isSelected
                        ? "border-[hsl(var(--primary))] bg-[hsl(var(--primary))]/10 text-[hsl(var(--foreground))]"
                        : "border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))] hover:bg-[hsl(var(--accent))]"
                    }`}
                  >
                    <span
                      className="size-2 rounded-full"
                      style={{ backgroundColor: label.color }}
                    />
                    {label.name}
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
          <Button size="sm" onClick={handleSubmit} disabled={!title.trim()}>
            Create Issue
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
