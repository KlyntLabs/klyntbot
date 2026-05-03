import { productivityHourlyBreakdownQuery } from "@/api/endpoints/dashboard";
import type { HourlyBreakdownResponse } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { TZ_OFFSET_MINS } from "@/utils/dashboardDates";

interface Props {
  startDate: string;
  endDate: string;
}

function heatColor(ratio: number): string {
  const t = Math.max(0, Math.min(1, ratio));
  const stops: [number, number, number][] = [
    [0, 70, 50],
    [25, 80, 50],
    [45, 85, 50],
    [145, 65, 45],
  ];
  const seg = t * (stops.length - 1);
  const i = Math.min(Math.floor(seg), stops.length - 2);
  const f = seg - i;
  const [h, s, l] = stops[i].map((v, k) => v + (stops[i + 1][k] - v) * f);
  return `hsl(${h}, ${s}%, ${l}%)`;
}

export function HourlyHeatmap({ startDate, endDate }: Props) {
  const { data } = useTauriQuery<HourlyBreakdownResponse[]>({
    queryKey: qk.productivity.hourlyBreakdown(startDate, endDate),
    queryFn: () => productivityHourlyBreakdownQuery(startDate, endDate, TZ_OFFSET_MINS),
    fallback: [],
    staleTime: 60_000,
  });

  const working = (data ?? []).filter((h) => h.hour >= 6 && h.hour <= 22);
  if (working.length === 0) {
    return (
      <div className="dashboard__hourly dashboard__hourly--empty">
        <div className="dashboard__hourly-title">Hourly Productivity</div>
        <div className="dashboard__hourly-empty-msg">
          Hourly breakdown appears after a full day of tracking.
        </div>
      </div>
    );
  }

  const maxRatio = Math.max(...working.map((h) => h.productiveRatio), 0.01);
  const peakHour = working.reduce((best, h) =>
    h.productiveRatio > best.productiveRatio ? h : best,
  );

  return (
    <div className="dashboard__hourly">
      <div className="dashboard__hourly-title">
        Hourly Productivity
        {peakHour && <span className="dashboard__hourly-peak"> Peak: {peakHour.hour}:00</span>}
      </div>
      <div>
        {working.map((h) => {
          const width = (h.productiveRatio / maxRatio) * 100;
          return (
            <div key={h.hour} className="dashboard__hourly-row">
              <span className="dashboard__hourly-hour-label">{h.hour}</span>
              <div className="dashboard__hourly-bar-track">
                <div
                  className="dashboard__hourly-bar-fill"
                  style={{ width: `${width}%`, backgroundColor: heatColor(h.productiveRatio) }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
