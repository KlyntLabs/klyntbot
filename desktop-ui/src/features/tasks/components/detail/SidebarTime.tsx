import { formatHumanDuration } from "@shared/lib/dates";
import type { TaskForecast } from "../../hooks/useIssueDetail";
import type { DetailTask, TaskState } from "../../lib/mappers";
import { cn } from "../../lib/utils";
import { SectionLabel } from "./SectionLabel";

interface SidebarTimeProps {
  task: DetailTask;
  taskState: TaskState;
  forecast: TaskForecast | null;
  forecastLoading: boolean;
  onFetchForecast: () => void;
}

export function SidebarTime({
  task,
  taskState,
  forecast,
  forecastLoading,
  onFetchForecast,
}: SidebarTimeProps) {
  const estimatedSecs = (task.estimatedMinutes ?? 0) * 60;
  const trackedSecs = task.totalTrackedSecs;

  if (taskState === "new") {
    return (
      <div className="px-4 py-3">
        <SectionLabel>Time</SectionLabel>
        <div className="mt-2 text-sm text-foreground">
          {task.estimatedMinutes
            ? `Estimate: ${formatHumanDuration(estimatedSecs)}`
            : "No estimate"}
        </div>
        {task.estimatedMinutes != null && (
          <ForecastSection
            forecast={forecast}
            forecastLoading={forecastLoading}
            onFetchForecast={onFetchForecast}
          />
        )}
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
              className={cn("text-xs mt-1", deviation > 0 ? "text-destructive" : "text-success")}
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
  const barColor = ratio < 0.8 ? "bg-success" : ratio < 1.0 ? "bg-warning" : "bg-destructive";

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

        {forecast ? (
          <TimeRow
            label="Forecast"
            value={`${formatHumanDuration(forecast.confidenceLow * 60)} — ${formatHumanDuration(forecast.estimatedMinutes * 60)} — ${formatHumanDuration(forecast.confidenceHigh * 60)}`}
          />
        ) : (
          <TimeRow label="Forecast" value="—" />
        )}

        {estimatedSecs > 0 && (
          <>
            <div className="h-1.5 rounded-full bg-muted mt-2 overflow-hidden">
              <div
                className={cn("h-full rounded-full transition-all", barColor)}
                style={{ width: `${barWidth}%` }}
              />
            </div>
            <div className="text-xs text-muted-foreground mt-1">{statusText}</div>
          </>
        )}

        {task.estimatedMinutes != null && (
          <ForecastSection
            forecast={forecast}
            forecastLoading={forecastLoading}
            onFetchForecast={onFetchForecast}
          />
        )}
      </div>
    </div>
  );
}

function ForecastSection({
  forecast,
  forecastLoading,
  onFetchForecast,
}: {
  forecast: TaskForecast | null;
  forecastLoading: boolean;
  onFetchForecast: () => void;
}) {
  if (!forecast) {
    return (
      <button
        type="button"
        onClick={onFetchForecast}
        disabled={forecastLoading}
        className="mt-2 text-xs text-[hsl(var(--primary))] hover:underline disabled:opacity-50"
      >
        {forecastLoading ? "Loading..." : "Get AI Forecast"}
      </button>
    );
  }

  return (
    <div className="mt-2 space-y-1">
      <div className="text-xs text-[hsl(var(--muted-foreground))]">
        Based on {forecast.sampleSize} similar tasks ({forecast.dataQuality})
      </div>
      {forecast.risks.length > 0 && (
        <ul className="text-xs text-[hsl(var(--muted-foreground))] list-disc pl-3 space-y-0.5">
          {forecast.risks.map((risk) => (
            <li key={risk.kind}>
              {risk.description}
              {risk.impactMinutes != null && ` (+${risk.impactMinutes}m)`}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function TimeRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-sm text-foreground font-mono tabular-nums">{value}</span>
    </div>
  );
}
