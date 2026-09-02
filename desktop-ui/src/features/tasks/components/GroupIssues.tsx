import { useDroppable } from "@dnd-kit/core";
import { Plus } from "lucide-react";
import { useMemo } from "react";
import type { Issue, Status } from "../lib/mappers";
import { sortIssuesByPriority } from "../lib/mappers";
import { renderStatusIcon } from "../lib/status-utils";
import { useCreateIssueStore } from "../store/create-issue-store";
import { useViewStore } from "../store/view-store";
import { IssueGrid } from "./IssueGrid";
import { IssueLine } from "./IssueLine";

interface GroupIssuesProps {
  status: Status;
  issues: Issue[];
}

export function GroupIssues({ status, issues }: GroupIssuesProps) {
  const openModal = useCreateIssueStore((s) => s.openModal);
  const viewType = useViewStore((s) => s.viewType);
  const sorted = useMemo(() => sortIssuesByPriority(issues), [issues]);

  const { setNodeRef, isOver } = useDroppable({
    id: status.id,
    data: { status },
  });

  if (viewType === "grid") {
    return (
      <div className="w-[348px] shrink-0 flex flex-col">
        {/* Column Header */}
        <div className="flex items-center gap-2 px-3 py-2 sticky top-0 z-10 bg-bg">
          <span className="flex items-center">{renderStatusIcon(status)}</span>
          <span className="text-sm font-medium text-fg">{status.name}</span>
          <span className="text-ui-sm text-fg-secondary">{issues.length}</span>
          <button
            type="button"
            onClick={() => openModal(status)}
            className="ml-auto flex items-center justify-center size-5 rounded hover:bg-control-hover text-fg-secondary hover:text-fg transition-colors"
            aria-label={`Add issue to ${status.name}`}
          >
            <Plus className="size-4" />
          </button>
        </div>

        {/* Drop zone */}
        <div
          ref={setNodeRef}
          className={`flex-1 flex flex-col gap-2 p-2 rounded-md min-h-[100px] transition-colors ${
            isOver ? "bg-control-hover/50" : ""
          }`}
        >
          {sorted.map((issue) => (
            <IssueGrid key={issue.id} issue={issue} />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="mb-2">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-2 sticky top-0 z-10 bg-bg">
        <span className="flex items-center">{renderStatusIcon(status)}</span>
        <span className="text-sm font-medium text-fg">{status.name}</span>
        <span className="text-ui-sm text-fg-secondary">{issues.length}</span>
        <button
          type="button"
          onClick={() => openModal(status)}
          className="ml-auto flex items-center justify-center size-5 rounded hover:bg-control-hover text-fg-secondary hover:text-fg transition-colors opacity-0 group-hover/group:opacity-100"
          aria-label={`Add issue to ${status.name}`}
        >
          <Plus className="size-4" />
        </button>
      </div>

      {/* Issues */}
      <div>
        {sorted.map((issue) => (
          <IssueLine key={issue.id} issue={issue} />
        ))}
      </div>
    </div>
  );
}
