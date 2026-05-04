import { productivityHourlyBreakdownQuery } from "@/api/endpoints/dashboard";
import type { HourlyBreakdownResponse } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { TZ_OFFSET_MINS, todayISO } from "@/utils/dashboardDates";

interface Props {
  startDate: string;
  endDate: string;
}

const WORK_START = 6;
const WORK_END = 22;
const clampHour = (h: number) => Math.max(WORK_START, Math.min(WORK_END, h));

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

  const byHour = new Map<number, HourlyBreakdownResponse>();
  for (const h of data ?? []) byHour.set(h.hour, h);

  const isToday = endDate === todayISO();
  const nowHour = new Date().getHours();
  const dataHours = (data ?? [])
    .filter((h) => h.totalSecs > 0 && h.hour >= WORK_START && h.hour <= WORK_END)
    .map((h) => h.hour);
  const firstDataHour = dataHours.length > 0 ? Math.min(...dataHours) : null;
  const lastDataHour = dataHours.length > 0 ? Math.max(...dataHours) : null;

  // Today: from first hour with activity → current hour (so the list grows
  // through the day and motivates filling more hours). Before any activity,
  // show a single row at the current hour as a starting point.
  // Past days: span first→last hour with activity (or nothing if none).
  let startHour: number | null;
  let endHour: number | null;
  if (isToday) {
    startHour = firstDataHour ?? clampHour(nowHour);
    endHour = clampHour(Math.max(nowHour, startHour));
  } else if (firstDataHour != null && lastDataHour != null) {
    startHour = firstDataHour;
    endHour = lastDataHour;
  } else {
    startHour = null;
    endHour = null;
  }

  const working: HourlyBreakdownResponse[] = [];
  if (startHour != null && endHour != null) {
    for (let hour = startHour; hour <= endHour; hour++) {
      const existing = byHour.get(hour);
      working.push(
        existing ?? {
          hour,
          productiveSecs: 0,
          neutralSecs: 0,
          distractingSecs: 0,
          idleSecs: 0,
          totalSecs: 0,
          productiveRatio: 0,
        },
      );
    }
  }
  const hasData = working.some((h) => h.productiveRatio > 0 || h.totalSecs > 0);

  const maxRatio = Math.max(...working.map((h) => h.productiveRatio), 0.01);
  const peakHour = hasData
    ? working.reduce((best, h) => (h.productiveRatio > best.productiveRatio ? h : best))
    : null;

  return (
    <div className="dashboard__hourly">
      <div className="dashboard__hourly-title">
        Hourly Productivity
        {peakHour && <span className="dashboard__hourly-peak"> Peak: {peakHour.hour}:00</span>}
      </div>
      {working.length > 0 ? (
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
      ) : (
        <div className="dashboard__hourly-empty-msg">No tracked hours for this day.</div>
      )}
    </div>
  );
}
