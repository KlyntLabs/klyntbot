import { FlaskConical } from "lucide-react";
import type { ExperimentSummary } from "../types";

interface ExperimentTimelineProps {
  experiments: ExperimentSummary[];
  loading: boolean;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function ExperimentTimeline({ experiments, loading }: ExperimentTimelineProps) {
  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-muted-foreground flex items-center gap-2">
        <FlaskConical className="size-3.5 text-muted-foreground" />
        Experiment History
        {experiments.length > 0 && (
          <span className="text-2xs text-dim font-light ml-auto">
            {experiments.length} {experiments.length === 1 ? "experiment" : "experiments"}
          </span>
        )}
      </h2>

      {loading && <p className="text-xs font-light text-dim">Loading&hellip;</p>}

      {!loading && experiments.length === 0 && (
        <p className="text-xs font-light text-dim">No experiments yet</p>
      )}

      {!loading && experiments.length > 0 && (
        <div className="flex flex-col gap-1 max-h-64 overflow-y-auto pr-0.5">
          {experiments.map((exp, idx) => (
            <div key={exp.id} className="flex gap-3 py-1.5">
              {/* Timeline spine */}
              <div className="flex flex-col items-center flex-shrink-0">
                <div className={`version-dot mt-0.5 ${idx === 0 ? "version-dot-active" : ""}`} />
                {idx < experiments.length - 1 && <div className="version-line flex-1 mt-1" />}
              </div>

              {/* Content */}
              <div className="flex flex-col gap-0.5 pb-2 min-w-0">
                <p className="text-xs font-light text-foreground leading-snug truncate">
                  {exp.hypothesis}
                </p>
                <p className="text-2xs font-light text-dim tabular-nums">
                  {exp.variant_count}v &middot; {exp.messages_scored} scored &middot;{" "}
                  {formatDate(exp.started_at)}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
