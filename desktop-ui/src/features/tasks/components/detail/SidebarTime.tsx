import { formatHumanDuration } from "@shared/lib/dates";
import { cn } from "@shared/lib/utils";
import type { DetailTask, TaskState } from "../../lib/mappers";
import { SectionLabel } from "./SectionLabel";

interface SidebarTimeProps {
  task: DetailTask;
  taskState: TaskState;
}

export function SidebarTime({ task, taskState }: SidebarTimeProps) {
  const estimatedSecs = (task.estimatedMinutes ?? 0) * 60;
  const trackedSecs = task.totalTrackedSecs;

  if (taskState === "new") {
    return (
      <div className="px-4 py-3">
        <SectionLabel>Time</SectionLabel>
        <div className="mt-2 text-sm text-fg">
          {task.estimatedMinutes
            ? `Estimate: ${formatHumanDuration(estimatedSecs)}`
            : "No estimate"}
        </div>
      </div>
    );
  }

  if (taskState === "completed") {
    const deviation =
      estimatedSecs > 0 ? Math.round(((trackedSecs - estimatedSecs) / estimatedSecs) * 100) : 0;
    return (
      <div className="px-4 py-3">
        <SectionLabel>Time — Final</SectionLabel>
        <div className="mt-2 space-y-1">
          <TimeRow
            label="Estimated"
            value={estimatedSecs > 0 ? formatHumanDuration(estimatedSecs) : "—"}
          />
          <TimeRow label="Actual" value={formatHumanDuration(trackedSecs)} />
          {estimatedSecs > 0 && (
            <div
              className={cn("text-ui-sm mt-1", deviation > 0 ? "text-status-danger" : "text-status-success")}
            >
              {deviation > 0
                ? `${deviation}% over estimate`
                : `${Math.abs(deviation)}% under estimate`}
            </div>
          )}
        </div>
      </div>
    );
  }

  // focused or has-history
  const ratio = estimatedSecs > 0 ? trackedSecs / estimatedSecs : 0;
  const percentage = Math.round(ratio * 100);
  const barWidth = Math.min(percentage, 100);
  const barColor = ratio < 0.8 ? "bg-status-success" : ratio < 1.0 ? "bg-status-warning" : "bg-status-danger";

  const statusText =
    ratio < 1.0 ? `${percentage}% · ahead of schedule` : `${percentage}% · over estimate`;

  return (
    <div className="px-4 py-3">
      <SectionLabel>Time</SectionLabel>
      <div className="mt-2 space-y-1">
        <TimeRow
          label="Estimated"
          value={estimatedSecs > 0 ? formatHumanDuration(estimatedSecs) : "—"}
        />
        <TimeRow label="Tracked" value={formatHumanDuration(trackedSecs)} />

        {estimatedSecs > 0 && (
          <>
            <div className="h-1.5 rounded-full bg-control-hover mt-2 overflow-hidden">
              <div
                className={cn("h-full rounded-full transition-all", barColor)}
                style={{ width: `${barWidth}%` }}
              />
            </div>
            <div className="text-ui-sm text-fg-secondary mt-1">{statusText}</div>
          </>
        )}
      </div>
    </div>
  );
}

function TimeRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-ui-sm text-fg-secondary">{label}</span>
      <span className="text-sm text-fg font-mono tabular-nums">{value}</span>
    </div>
  );
}
