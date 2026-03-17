import { useQuery } from "@shared/hooks/useQuery";
import type { HourlyBreakdown } from "@shared/types/productivity";

interface Props {
  startDate: string;
  endDate: string;
}

export function HourlyHeatmap({ startDate, endDate }: Props) {
  const { data } = useQuery<HourlyBreakdown[]>(
    "productivity_hourly_breakdown",
    { start_date: startDate, end_date: endDate },
    undefined,
    60_000,
  );

  if (!data || data.length === 0) return null;

  const working = data.filter((h) => h.hour >= 6 && h.hour <= 22);
  if (working.length === 0) return null;

  const maxRatio = Math.max(...working.map((h) => h.productiveRatio), 0.01);

  const peakHour = working.reduce((best, h) =>
    h.productiveRatio > best.productiveRatio ? h : best,
  );

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-xs font-medium text-foreground">
        Hourly Productivity
        {peakHour && <span className="text-muted-foreground font-normal ml-1">Peak: {peakHour.hour}:00</span>}
      </div>
      <div className="space-y-px">
        {working.map((h) => {
          const width = (h.productiveRatio / maxRatio) * 100;
          return (
            <div key={h.hour} className="flex items-center gap-1.5">
              <span className="text-[10px] text-muted-foreground w-6 text-right tabular-nums">{h.hour}</span>
              <div className="flex-1 h-2.5 rounded-sm bg-muted overflow-hidden">
                <div
                  className="h-full rounded-sm bg-accent transition-all"
                  style={{
                    width: `${width}%`,
                    opacity: 0.3 + h.productiveRatio * 0.7,
                  }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
