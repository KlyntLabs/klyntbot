import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, TZ_OFFSET_MINS, todayISO } from "@shared/lib/dates";

interface WorkContextSummary {
  id: string;
  title: string;
  contextType: string;
  color: string | null;
  durationMins: number;
  confidence: number;
}

interface DashboardIntelligence {
  activeContext: WorkContextSummary | null;
  productivityScore: number;
}

export function WorkContextCard() {
  const today = todayISO();
  const { data: intel } = useQuery<DashboardIntelligence>("get_dashboard_intelligence", {
    date: today,
    tzOffsetMins: TZ_OFFSET_MINS,
  });

  const ctx = intel?.activeContext;

  return (
    <div className="glass-card rounded-xl p-5">
      <p className="text-[10px] text-muted-foreground uppercase tracking-wider mb-3">
        Work Context
      </p>

      {ctx ? (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <div
              className="w-2 h-2 rounded-full flex-shrink-0"
              style={{ backgroundColor: ctx.color ?? "var(--brand)" }}
            />
            <span className="text-sm font-medium text-foreground truncate">{ctx.title}</span>
          </div>
          <div className="flex items-center gap-3 text-[11px] text-muted-foreground">
            <span>{formatHumanDuration(ctx.durationMins * 60)}</span>
            <span className="w-px h-3 bg-border" />
            <span>{Math.round(intel?.productivityScore ?? 0)}% productive</span>
          </div>
        </div>
      ) : (
        <p className="text-[11px] text-muted-foreground">No active session</p>
      )}
    </div>
  );
}
