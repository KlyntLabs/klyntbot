import { X } from "lucide-react";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { SidebarProperties } from "./SidebarProperties";
import { SidebarTime } from "./SidebarTime";

interface IssueDetailSidebarProps {
  detail: ReturnType<typeof useIssueDetail>;
  onClose: () => void;
}

export function IssueDetailSidebar({ detail, onClose }: IssueDetailSidebarProps) {
  const { taskState } = detail;
  const showTime = taskState !== "new" || detail.task.estimatedMinutes != null;

  return (
    <div className="w-[260px] shrink-0 border-l border-separator overflow-y-auto">
      <div className="flex items-center justify-between px-4 py-3 border-b border-separator">
        <span className="text-ui-sm font-medium text-fg-secondary uppercase tracking-wider">
          Details
        </span>
        <button
          type="button"
          onClick={onClose}
          className="p-0.5 rounded hover:bg-control-hover text-fg-secondary transition-colors"
          aria-label="Close sidebar"
        >
          <X className="size-3.5" />
        </button>
      </div>
      <div className="divide-y divide-separator">
        <SidebarProperties task={detail.task} compact={false} onUpdate={detail.updateTask} />
        {showTime && <SidebarTime task={detail.task} taskState={taskState} />}
      </div>
    </div>
  );
}
