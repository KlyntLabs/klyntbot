import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration } from "@shared/lib/dates";
import type { WeeklyAssessment } from "@shared/types/productivity";

interface Props {
  weekStart: string;
}

export function WeeklyAssessmentCard({ weekStart }: Props) {
  const { data } = useQuery<WeeklyAssessment | null>("productivity_weekly_assessment", {
    weekStart,
  });

  if (!data) return null;

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-xs font-medium text-foreground">Weekly Assessment</div>
      <div className="text-xs text-muted space-y-0.5">
        {data.avgScore != null && <div>Avg score: {data.avgScore.toFixed(0)}</div>}
        {data.totalFocusMins != null && (
          <div>Focus: {formatHumanDuration(data.totalFocusMins * 60)}</div>
        )}
        {data.summary && <div className="italic">{data.summary}</div>}
      </div>
    </div>
  );
}
