import { useQuery } from "@shared/hooks/useQuery";
import { TZ_OFFSET_MINS } from "@shared/lib/dates";
import type { HourlyBreakdown } from "@shared/types/productivity";

interface Props {
  startDate: string;
  endDate: string;
}

export function HourlyHeatmap({ startDate, endDate }: Props) {
  const { data } = useQuery<HourlyBreakdown[]>(
    "productivity_hourly_breakdown",
    { start_date: startDate, end_date: endDate, tz_offset_mins: TZ_OFFSET_MINS },
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

  /** Map productive ratio (0–1) to heatmap: red → orange → yellow → green */
  const heatColor = (ratio: number): string => {
    const t = Math.max(0, Math.min(1, ratio));
    const stops: [number, number, number][] = [
      [0, 70, 50], // red    – low
      [25, 80, 50], // orange – below avg
      [45, 85, 50], // yellow – moderate
      [145, 65, 45], // green  – high
    ];
    const seg = t * (stops.length - 1);
    const i = Math.min(Math.floor(seg), stops.length - 2);
    const f = seg - i;
    const [h, s, l] = stops[i].map((v, k) => v + (stops[i + 1][k] - v) * f);
    return `hsl(${h}, ${s}%, ${l}%)`;
  };

  return (
    <div className="space-y-1 px-1 py-2">
      <div className="text-2xs font-medium text-foreground">
        Hourly Productivity
        {peakHour && <span className="text-dim font-normal ml-1">Peak: {peakHour.hour}:00</span>}
      </div>
      <div className="space-y-px">
        {working.map((h) => {
          const width = (h.productiveRatio / maxRatio) * 100;
          return (
            <div key={h.hour} className="flex items-center gap-1">
              <span className="text-2xs text-dim w-4 text-right tabular-nums">{h.hour}</span>
              <div className="flex-1 h-1 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: `${width}%`,
                    backgroundColor: heatColor(h.productiveRatio),
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
