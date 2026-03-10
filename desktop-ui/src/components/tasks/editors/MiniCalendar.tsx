import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toLocalISO } from "../../../lib/dates";
import { cn } from "../../../lib/utils";

interface MiniCalendarProps {
  value: string | null;
  onSelect: (iso: string) => void;
  onClear?: () => void;
  /** Show Today/Tomorrow/Next Mon shortcuts. Defaults to true. */
  showShortcuts?: boolean;
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

export function MiniCalendar({
  value,
  onSelect,
  onClear,
  showShortcuts = true,
}: MiniCalendarProps) {
  const [todayISO, setTodayISO] = useState(() => {
    const d = new Date();
    d.setHours(0, 0, 0, 0);
    return toLocalISO(d);
  });

  // biome-ignore lint/correctness/useExhaustiveDependencies: re-schedule on todayISO change after midnight
  useEffect(() => {
    const now = new Date();
    const msUntilMidnight =
      new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime() - now.getTime();
    const id = setTimeout(() => {
      const d = new Date();
      d.setHours(0, 0, 0, 0);
      setTodayISO(toLocalISO(d));
    }, msUntilMidnight + 100);
    return () => clearTimeout(id);
  }, [todayISO]);

  const today = useMemo(() => new Date(`${todayISO}T00:00:00`), [todayISO]);

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
      { label: "Tomorrow", iso: toLocalISO(addDays(today, 1)) },
      { label: "Next Mon", iso: toLocalISO(nextWeekday(today, 1)) },
    ],
    [today, todayISO],
  );

  return (
    <fieldset
      className="w-full min-w-[252px] border-none p-0 m-0"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      {/* Quick-select shortcuts */}
      {showShortcuts && (
        <div className="flex gap-1 mb-1.5 px-0.5">
          {shortcuts.map((s) => (
            <button
              type="button"
              key={s.label}
              onClick={() => onSelect(s.iso)}
              className={cn(
                "px-2 py-0.5 text-[11px] rounded-lg transition-all",
                value === s.iso ? "glass-button-active text-brand" : "glass-button text-muted",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
      )}

      {/* Month navigation header */}
      <div className="flex items-center justify-between px-0.5 mb-1">
        <button
          type="button"
          onClick={prevMonth}
          aria-label="Previous month"
          className="w-6 h-6 flex items-center justify-center glass-button rounded-lg text-muted hover:text-secondary transition-colors"
        >
          <ChevronLeft className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
        <span className="text-[12px] font-medium text-secondary">
          {MONTH_NAMES[viewMonth]} {viewYear}
        </span>
        <button
          type="button"
          onClick={nextMonth}
          aria-label="Next month"
          className="w-6 h-6 flex items-center justify-center glass-button rounded-lg text-muted hover:text-secondary transition-colors"
        >
          <ChevronRight className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
      </div>

      {/* Weekday labels */}
      <div className="grid grid-cols-7 mb-0.5">
        {WEEKDAYS.map((d) => (
          <div
            key={d}
            className="h-6 flex items-center justify-center text-[10px] font-medium text-muted"
          >
            {d}
          </div>
        ))}
      </div>

      {/* Day grid */}
      <div className="grid grid-cols-7 gap-0.5">
        {cells.map((d) => {
          const iso = toLocalISO(d);
          const isCurrentMonth = d.getMonth() === viewMonth;
          const isToday = iso === todayISO;
          const isSelected = iso === value;

          return (
            <button
              type="button"
              key={iso}
              onClick={() => onSelect(iso)}
              aria-label={`${d.getDate()} ${MONTH_NAMES[d.getMonth()]} ${d.getFullYear()}`}
              className={cn(
                "h-8 w-full flex items-center justify-center text-[11px] font-medium rounded-lg border transition-all",
                isSelected
                  ? "bg-brand text-white border-brand/40"
                  : isToday
                    ? "bg-white/[0.10] text-brand border-brand/30"
                    : isCurrentMonth
                      ? "bg-white/[0.04] text-secondary border-white/[0.06] hover:bg-white/[0.10] hover:border-white/[0.12]"
                      : "text-muted/30 border-transparent hover:bg-white/[0.06]",
              )}
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
          className="w-full text-left mt-1.5 px-2 py-1 text-[11px] text-destructive rounded-lg hover:bg-white/[0.08] transition-colors"
        >
          Clear date
        </button>
      )}
    </fieldset>
  );
}
