import { useQuery } from "@shared/hooks/useQuery";
import type { ProductivityPatterns } from "@shared/types/productivity";

export function PatternsCard() {
  const { data } = useQuery<ProductivityPatterns>(
    "productivity_patterns",
    undefined,
    undefined,
    5 * 60 * 1000,
  );

  if (!data || data.daysAnalyzed < 3) return null;

  const peakLabel =
    data.peakFocusHours.length > 0
      ? data.peakFocusHours.map((h) => `${h}:00`).join(", ")
      : "\u2014";

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-xs font-medium text-foreground">Your Patterns</div>
      <div className="text-xs text-muted-foreground space-y-0.5">
        <div>Peak hours: {peakLabel}</div>
        {data.bestDayOfWeek && <div>Best day: {data.bestDayOfWeek}</div>}
        <div>Avg session: {Math.round(data.avgSessionMins)}min</div>
        <div className="text-[10px] text-muted-foreground/60">
          {data.daysAnalyzed} days analyzed
        </div>
      </div>
    </div>
  );
}
