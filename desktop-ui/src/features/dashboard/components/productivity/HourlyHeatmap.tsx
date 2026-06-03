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
    <div className="px-1 py-2 flex flex-col gap-1" role="img" aria-label="Hourly productivity breakdown">
      <div className="text-ui-2xs font-medium text-ds-text-strong">
        Hourly Productivity
        {peakHour && <span className="font-normal text-[color-mix(in_srgb,var(--ds-text-subtle)_60%,transparent)] ml-1"> Peak: {peakHour.hour}:00</span>}
      </div>
      {working.length > 0 ? (
        <div>
          {working.map((h) => {
            const width = (h.productiveRatio / maxRatio) * 100;
            return (
              <div key={h.hour} className="flex items-center gap-1">
                <span className="text-ui-2xs text-[color-mix(in_srgb,var(--ds-text-subtle)_60%,transparent)] w-4 text-right tabular-nums">{h.hour}</span>
                <div className="flex-1 h-1 rounded-full bg-surface-control overflow-hidden">
                  <div
                    className="h-full rounded-full dashboard__hourly-bar-fill"
                    style={{ width: `${width}%`, backgroundColor: heatColor(h.productiveRatio) }}
                  />
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="text-ui-2xs italic text-[color-mix(in_srgb,var(--ds-text-subtle)_70%,transparent)]">No tracked hours for this day.</div>
      )}
    </div>
  );
}
