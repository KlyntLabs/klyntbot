import { X } from "lucide-react";
import type { useIssueDetail } from "../../hooks/useIssueDetail";
import { SidebarAiInsights } from "./SidebarAiInsights";
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
    <div className="w-[260px] shrink-0 border-l border-border overflow-y-auto">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
          Details
        </span>
        <button
          type="button"
          onClick={onClose}
          className="p-0.5 rounded hover:bg-accent text-muted-foreground transition-colors"
          aria-label="Close sidebar"
        >
          <X className="size-3.5" />
        </button>
      </div>
      <div className="divide-y divide-border">
        <SidebarProperties task={detail.task} compact={false} onUpdate={detail.updateTask} />
        {showTime && (
          <SidebarTime
            task={detail.task}
            taskState={taskState}
            forecast={detail.forecast}
            forecastLoading={detail.forecastLoading}
            onFetchForecast={detail.fetchForecast}
          />
        )}
        <SidebarAiInsights
          task={detail.task}
          taskState={taskState}
          suggestions={detail.suggestions}
          onApply={detail.applySuggestion}
          onDismiss={detail.dismissSuggestion}
          onFetchSuggestions={detail.fetchSuggestions}
          suggestionsLoading={detail.suggestionsLoading}
          aiError={detail.aiError}
        />
      </div>
    </div>
  );
}
