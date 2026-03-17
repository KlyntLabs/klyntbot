import { X } from "lucide-react";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { SidebarAiInsights } from "./SidebarAiInsights";
import { SidebarProperties } from "./SidebarProperties";
import { SidebarTime } from "./SidebarTime";
import { SidebarWorkState } from "./SidebarWorkState";

interface IssueDetailSidebarProps {
  detail: ReturnType<typeof useIssueDetail>;
  onClose: () => void;
}

export function IssueDetailSidebar({ detail, onClose }: IssueDetailSidebarProps) {
  const { taskState } = detail;
  // State table: focused + completed show work state section
  const showWorkState = taskState === "focused" || taskState === "completed";
  const showTime = taskState !== "new" || detail.task.estimatedMinutes != null;

  return (
    <div className="w-[260px] shrink-0 border-l border-[hsl(var(--border))] overflow-y-auto">
      <div className="flex items-center justify-between px-4 py-3 border-b border-[hsl(var(--border))]">
        <span className="text-xs font-medium text-[hsl(var(--muted-foreground))] uppercase tracking-wider">
          Details
        </span>
        <button
          type="button"
          onClick={onClose}
          className="p-0.5 rounded hover:bg-[hsl(var(--accent))] text-[hsl(var(--muted-foreground))] transition-colors"
          aria-label="Close sidebar"
        >
          <X className="size-3.5" />
        </button>
      </div>
      <div className="divide-y divide-[hsl(var(--border))]">
        <SidebarProperties
          task={detail.task}
          compact={taskState === "focused"}
          onUpdate={detail.updateTask}
        />
        {showWorkState && (
          <SidebarWorkState
            task={detail.task}
            taskState={taskState}
            focusSession={detail.focusSession}
            onStopFocus={detail.stopFocus}
          />
        )}
        {showTime && <SidebarTime task={detail.task} taskState={taskState} />}
        <SidebarAiInsights
          taskState={taskState}
          suggestions={detail.suggestions}
          taskMemory={detail.taskMemory}
          onApply={detail.applySuggestion}
          onDismiss={detail.dismissSuggestion}
          onFetchSuggestions={detail.fetchSuggestions}
          suggestionsLoading={detail.suggestionsLoading}
        />
      </div>
    </div>
  );
}
