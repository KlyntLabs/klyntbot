import { CollapsibleSection } from "@shared/components";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import { Activity } from "lucide-react";

interface ProductivitySectionProps {
  projectId: string;
  defaultOpen?: boolean;
}

interface ProjectProductivity {
  totalFocusSecs: number;
  sessionsCount: number;
  avgQuality: number;
}

export function ProductivitySection({ projectId, defaultOpen }: ProductivitySectionProps) {
  // Use productivity summary filtered to project if available
  const { data: summary } = useQuery<ProjectProductivity | null>(
    "productivity_summary_range",
    { startDate: todayISO(), endDate: todayISO(), projectId },
    null,
  );

  const focusHrs = summary ? Math.round((summary.totalFocusSecs / 3600) * 10) / 10 : 0;

  return (
    <CollapsibleSection
      title="Productivity"
      icon={<Activity className="w-3.5 h-3.5 text-brand" strokeWidth={1.5} />}
      defaultOpen={defaultOpen}
    >
      {!summary ? (
        <p className="text-[11px] text-dim font-light py-2">No data yet</p>
      ) : (
        <div className="grid grid-cols-2 gap-2">
          <div className="px-2 py-1.5">
            <p className="text-[16px] font-light text-primary tabular-nums">{focusHrs}h</p>
            <p className="text-[9px] text-dim">Focus time</p>
          </div>
          <div className="px-2 py-1.5">
            <p className="text-[16px] font-light text-primary tabular-nums">
              {summary.sessionsCount}
            </p>
            <p className="text-[9px] text-dim">Sessions</p>
          </div>
          <div className="px-2 py-1.5">
            <p className="text-[16px] font-light text-primary tabular-nums">
              {Math.round(summary.avgQuality * 100)}%
            </p>
            <p className="text-[9px] text-dim">Quality</p>
          </div>
        </div>
      )}
    </CollapsibleSection>
  );
}
