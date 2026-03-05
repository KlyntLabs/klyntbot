import { ChevronLeft, ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";

interface MiniCalendarProps {
  value: string | null;
  onSelect: (iso: string) => void;
  onClear?: () => void;
}

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
const MONTH_NAMES = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/** Format a Date as YYYY-MM-DD (local). */
function toISO(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Add `n` days to a date (local). */
function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

/** Get the next occurrence of a weekday (1=Mon). If today is that day, returns next week. */
function nextWeekday(from: Date, weekday: number): Date {
  const current = from.getDay() || 7; // convert Sunday 0 → 7
  const diff = weekday - current;
  return addDays(from, diff <= 0 ? diff + 7 : diff);
}

export function MiniCalendar({ value, onSelect, onClear }: MiniCalendarProps) {
  const today = useMemo(() => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return d;
  }, []);

  const todayISO = useMemo(() => toISO(today), [today]);

  // The month being viewed (year + month index)
  const [viewYear, setViewYear] = useState(() => {
    const d = value ? new Date(`${value}T00:00:00`) : today;
    return d.getFullYear();
  });
  const [viewMonth, setViewMonth] = useState(() => {
    const d = value ? new Date(`${value}T00:00:00`) : today;
    return d.getMonth();
  });

  const prevMonth = () => {
    if (viewMonth === 0) {
      setViewYear((y) => y - 1);
      setViewMonth(11);
    } else setViewMonth((m) => m - 1);
  };

  const nextMonth = () => {
    if (viewMonth === 11) {
      setViewYear((y) => y + 1);
      setViewMonth(0);
    } else setViewMonth((m) => m + 1);
  };

  // Build the 42-cell grid (6 rows × 7 cols), starting from Monday
  const cells = useMemo(() => {
    const first = new Date(viewYear, viewMonth, 1);
    // Day of week: 0=Sun → we want Mon=0, so (day+6)%7
    const startOffset = (first.getDay() + 6) % 7;
    const gridStart = new Date(viewYear, viewMonth, 1 - startOffset);

    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(gridStart);
      d.setDate(gridStart.getDate() + i);
      return d;
    });
  }, [viewYear, viewMonth]);

  // Quick-select shortcuts
  const shortcuts = useMemo(
    () => [
      { label: "Today", iso: todayISO },
      { label: "Tomorrow", iso: toISO(addDays(today, 1)) },
      { label: "Next Mon", iso: toISO(nextWeekday(today, 1)) },
    ],
    [today, todayISO],
  );

  return (
    <fieldset
      className="w-[232px] border-none p-0 m-0"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      {/* Quick-select shortcuts */}
      <div className="flex gap-1 mb-1.5 px-0.5">
        {shortcuts.map((s) => (
          <button
            type="button"
            key={s.label}
            onClick={() => onSelect(s.iso)}
            className={`px-2 py-0.5 text-[11px] font-light rounded-md transition-colors ${
              value === s.iso
                ? "bg-brand text-white"
                : "bg-surface-base text-muted hover:bg-surface-raised hover:text-secondary"
            }`}
          >
            {s.label}
          </button>
        ))}
      </div>

      {/* Month navigation header */}
      <div className="flex items-center justify-between px-0.5 mb-1">
        <button
          type="button"
          onClick={prevMonth}
          aria-label="Previous month"
          className="w-6 h-6 flex items-center justify-center rounded-md text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
        >
          <ChevronLeft className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
        <span className="text-[12px] font-light text-secondary">
          {MONTH_NAMES[viewMonth]} {viewYear}
        </span>
        <button
          type="button"
          onClick={nextMonth}
          aria-label="Next month"
          className="w-6 h-6 flex items-center justify-center rounded-md text-muted hover:text-secondary hover:bg-surface-raised transition-colors"
        >
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      </div>

      {/* Weekday labels */}
      <div className="grid grid-cols-7 mb-0.5">
        {WEEKDAYS.map((d) => (
          <div
            key={d}
            className="h-6 flex items-center justify-center text-[10px] font-light text-dim"
          >
            {d}
          </div>
        ))}
      </div>

      {/* Day grid */}
      <div className="grid grid-cols-7">
        {cells.map((d) => {
          const iso = toISO(d);
          const isCurrentMonth = d.getMonth() === viewMonth;
          const isToday = iso === todayISO;
          const isSelected = iso === value;

          return (
            <button
              type="button"
              key={iso}
              onClick={() => onSelect(iso)}
              aria-label={`${d.getDate()} ${MONTH_NAMES[d.getMonth()]} ${d.getFullYear()}`}
              className={`h-7 w-full flex items-center justify-center text-[11px] font-light rounded-md transition-colors ${
                isSelected
                  ? "bg-brand text-white"
                  : isToday
                    ? "ring-1 ring-brand text-brand"
                    : isCurrentMonth
                      ? "text-secondary hover:bg-surface-raised"
                      : "text-dim hover:bg-surface-raised"
              }`}
            >
              {d.getDate()}
            </button>
          );
        })}
      </div>

      {/* Clear date */}
      {onClear && (
        <button
          type="button"
          onClick={onClear}
          className="w-full text-left mt-1 px-2 py-1 text-[11px] font-light text-destructive hover:bg-surface-raised transition-colors"
          style={{ borderRadius: "var(--glass-radius-inner)" }}
        >
          Clear date
        </button>
      )}
    </fieldset>
  );
}
