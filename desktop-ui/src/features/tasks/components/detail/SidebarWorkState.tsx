import { formatElapsed } from "@shared/lib/dates";
import { Pause, Square } from "lucide-react";
import { useEffect, useState } from "react";
import type { DetailTask, FocusSession, TaskState } from "../../lib/mappers";
import { cn } from "../../lib/utils";
import { SectionLabel } from "./SectionLabel";

interface SidebarWorkStateProps {
  task: DetailTask;
  taskState: TaskState;
  focusSession: FocusSession | null;
  onStopFocus?: () => void;
}

export function SidebarWorkState({
  task,
  taskState,
  focusSession,
  onStopFocus,
}: SidebarWorkStateProps) {
  if (taskState === "completed") {
    return (
      <div className="px-4 py-3">
        <SectionLabel>Session Summary</SectionLabel>
        <p className="text-sm text-muted mt-1">
          Total tracked: {formatElapsed(task.totalTrackedSecs)}
        </p>
      </div>
    );
  }

  if (!focusSession || !task.focusedAt) return null;

  return (
    <div className="px-4 py-3 space-y-3">
      <SectionLabel>Work State</SectionLabel>
      <FocusTimer focusedAt={task.focusedAt} />
      {focusSession.qualityScore != null && (
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted">Quality</span>
          <span
            className={cn(
              "text-sm font-mono tabular-nums",
              focusSession.qualityScore > 0.7
                ? "text-success"
                : focusSession.qualityScore > 0.4
                  ? "text-warning"
                  : "text-destructive",
            )}
          >
            {focusSession.qualityScore.toFixed(2)}
          </span>
        </div>
      )}
      {focusSession.distractionCount != null && (
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted">Distractions</span>
          <span className="text-sm text-primary">{focusSession.distractionCount}</span>
        </div>
      )}
      {focusSession.flowState != null && (
        <div className="flex items-center justify-between">
          <span className="text-xs text-muted">Flow</span>
          <FlowBadge state={focusSession.flowState} />
        </div>
      )}
      {focusSession.qualityHistory != null && (
        <QualitySparkline values={focusSession.qualityHistory} />
      )}
      <div className="flex gap-2">
        <button
          type="button"
          disabled
          title="Coming soon"
          className="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs rounded border border-border text-primary disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <Pause className="size-3" />
          Pause
        </button>
        <button
          type="button"
          onClick={onStopFocus}
          className="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs rounded border border-border text-destructive hover:bg-destructive/10 transition-colors"
        >
          <Square className="size-3" />
          Stop
        </button>
      </div>
    </div>
  );
}

function FocusTimer({ focusedAt }: { focusedAt: string }) {
  const [elapsed, setElapsed] = useState(() =>
    Math.floor((Date.now() - new Date(focusedAt).getTime()) / 1000),
  );

  useEffect(() => {
    const origin = new Date(focusedAt).getTime();
    const interval = setInterval(() => {
      const next = Math.floor((Date.now() - origin) / 1000);
      setElapsed((prev) => (prev === next ? prev : next));
    }, 1000);
    return () => clearInterval(interval);
  }, [focusedAt]);

  return (
    <div className="text-2xl font-mono tabular-nums text-primary text-center">
      {formatElapsed(elapsed)}
    </div>
  );
}

function FlowBadge({ state }: { state: string }) {
  const color =
    state === "active"
      ? "text-success"
      : state === "building"
        ? "text-warning"
        : "text-destructive";

  return (
    <span className={cn("text-xs px-1.5 py-0.5 rounded bg-surface-raised capitalize", color)}>
      {state}
    </span>
  );
}

function QualitySparkline({ values }: { values: number[] }) {
  const max = Math.max(...values, 1);
  return (
    <div className="flex items-end gap-px h-8">
      {values.map((v, i) => {
        const height = `${(v / max) * 100}%`;
        const color = v > 0.7 ? "bg-success/70" : v > 0.4 ? "bg-warning/70" : "bg-destructive/70";
        return (
          <div key={`bar-${i}`} className={cn("flex-1 rounded-sm", color)} style={{ height }} />
        );
      })}
    </div>
  );
}
