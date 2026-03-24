import { HEATMAP_COLORS, type HeatmapLevel } from "../lib/heatmapColors";

const DAY_HEADERS = ["M", "T", "W", "T", "F", "S", "S"];

function formatAriaLabel(date: string, txCount: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const dateObj = new Date(y, m - 1, d);
  const formatted = new Intl.DateTimeFormat("en-US", {
    month: "long",
    day: "numeric",
  }).format(dateObj);
  return `${formatted}, ${txCount} ${txCount === 1 ? "transaction" : "transactions"}`;
}

export function SpendingHeatmap({
  year,
  month,
  levels,
  dailyCounts,
  selectedDay,
  onSelectDay,
  today,
}: {
  year: number;
  month: number; // 1-12
  levels: Map<string, HeatmapLevel>;
  dailyCounts: Map<string, number>;
  selectedDay: string | null;
  onSelectDay: (date: string | null) => void;
  today: string;
}) {
  const firstDayOfMonth = new Date(year, month - 1, 1);
  const daysInMonth = new Date(year, month, 0).getDate();

  // Monday = 0 offset (getDay: 0=Sun, 1=Mon, ..., 6=Sat)
  const rawDay = firstDayOfMonth.getDay(); // 0=Sun
  const startOffset = rawDay === 0 ? 6 : rawDay - 1;

  const cells: Array<{ date: string; day: number } | null> = [];
  for (let i = 0; i < startOffset; i++) cells.push(null);
  for (let d = 1; d <= daysInMonth; d++) {
    const mm = String(month).padStart(2, "0");
    const dd = String(d).padStart(2, "0");
    cells.push({ date: `${year}-${mm}-${dd}`, day: d });
  }

  return (
    <div>
      {/* Day headers */}
      <div className="grid grid-cols-7 mb-1">
        {DAY_HEADERS.map((h, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: static headers
          <div key={i} className="text-center text-2xs font-light text-dim py-1">
            {h}
          </div>
        ))}
      </div>

      {/* Calendar grid */}
      <div className="grid grid-cols-7 gap-0.5">
        {cells.map((cell, i) => {
          if (!cell) {
            // biome-ignore lint/suspicious/noArrayIndexKey: empty cell padding
            return <div key={`empty-${i}`} />;
          }
          const { date, day } = cell;
          const level: HeatmapLevel = levels.get(date) ?? 0;
          const txCount = dailyCounts.get(date) ?? 0;
          const isToday = date === today;
          const isSelected = date === selectedDay;

          return (
            <div
              key={date}
              role="button"
              tabIndex={0}
              aria-label={formatAriaLabel(date, txCount)}
              aria-pressed={isSelected}
              onClick={() => onSelectDay(isSelected ? null : date)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onSelectDay(isSelected ? null : date);
                }
              }}
              className="aspect-square flex items-center justify-center rounded-md text-[11px] font-light text-muted-foreground cursor-pointer transition-colors hover:brightness-125 select-none"
              style={{
                backgroundColor: HEATMAP_COLORS[level],
                outline: isSelected
                  ? "2px solid var(--info)"
                  : isToday
                    ? "1.5px solid var(--brand)"
                    : undefined,
                outlineOffset: "-1px",
              }}
            >
              {day}
            </div>
          );
        })}
      </div>

      {/* Legend */}
      <div className="flex items-center justify-end gap-1.5 mt-3">
        <span className="text-2xs text-dim font-light">Less</span>
        {HEATMAP_COLORS.map((color, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: static legend
          <div key={i} className="size-3 rounded-sm" style={{ backgroundColor: color }} />
        ))}
        <span className="text-2xs text-dim font-light">More</span>
      </div>
    </div>
  );
}
