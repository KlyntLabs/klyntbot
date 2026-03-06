import { formatHumanDuration } from "../../lib/dates";
import type { ProjectUsage } from "../../lib/types";
import { getAppColor } from "./shared";

interface ProjectsCardProps {
  projects: ProjectUsage[];
  totalSecs: number;
}

export function ProjectsCard({ projects, totalSecs }: ProjectsCardProps) {
  if (projects.length === 0) return null;

  return (
    <div className="glass-card p-4 flex flex-col gap-3">
      <div className="flex items-baseline justify-between">
        <h2 className="text-[13px] font-medium text-secondary">Projects</h2>
        <span className="text-[10px] font-light text-dim tabular-nums">
          {projects.length} tracked
        </span>
      </div>

      <div className="flex flex-col gap-2">
        {projects.map((p) => {
          const pct = totalSecs > 0 ? Math.round((p.durationSecs / totalSecs) * 100) : 0;
          const color = p.color ?? getAppColor(p.displayName, null);
          return (
            <div key={p.projectId} className="flex items-center gap-2">
              <span className="text-[11px] tabular-nums text-dim w-8 text-right flex-shrink-0">
                {pct}%
              </span>
              <span
                className="w-2 h-2 rounded-[3px] flex-shrink-0"
                style={{ backgroundColor: color }}
              />
              <span className="text-[11px] font-light text-secondary flex-1 truncate">
                {p.displayName}
              </span>
              <div className="flex-1 h-1.5 rounded-full bg-white/[0.06] overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-500"
                  style={{ width: `${pct}%`, backgroundColor: color }}
                />
              </div>
              <span className="text-[10px] tabular-nums text-dim w-12 text-right flex-shrink-0">
                {formatHumanDuration(p.durationSecs)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
