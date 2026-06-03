import ChevronLeft from "lucide-react/dist/esm/icons/chevron-left";
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import { useEffect, useMemo, useState } from "react";
import { cn } from "@/utils/cn";
import { LONG_MONTHS, toLocalISO } from "@/utils/dashboardDates";

interface MiniCalendarProps {
  value: string | null;
  onSelect: (iso: string) => void;
  onClear?: () => void;
  showShortcuts?: boolean;
}

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

function nextWeekday(from: Date, weekday: number): Date {
  const current = from.getDay() || 7;
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
  }, []);

  const today = useMemo(() => new Date(`${todayISO}T00:00:00`), [todayISO]);

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
      {showShortcuts && (
        <div className="flex gap-1 mb-1.5 px-0.5">
          {shortcuts.map((s) => (
            <button
              type="button"
              key={s.label}
              onClick={() => onSelect(s.iso)}
              className={cn(
                "px-2 py-0.5 text-ui-2xs rounded-lg bg-surface-hover text-text-muted border-none cursor-pointer transition-colors duration-ui-fast ease-out hover:text-text-strong",
                value === s.iso && "bg-surface-active text-border-accent",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
      )}

      <div className="flex items-center justify-between px-0.5 mb-1">
        <button
          type="button"
          onClick={prevMonth}
          aria-label="Previous month"
          className="bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active"
        >
          <ChevronLeft strokeWidth={1.5} className="w-4 h-4" />
        </button>
        <span className="text-ui-xs font-medium text-text-muted">
          {LONG_MONTHS[viewMonth]} {viewYear}
        </span>
        <button
          type="button"
          onClick={nextMonth}
          aria-label="Next month"
          className="bg-transparent border-none p-1.5 rounded-full text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out inline-flex items-center justify-center hover:text-text-strong hover:bg-surface-active"
        >
          <ChevronRight strokeWidth={1.5} className="w-4 h-4" />
        </button>
      </div>

      <div className="grid grid-cols-7 mb-0.5">
        {WEEKDAYS.map((d) => (
          <div key={d} className="h-6 flex items-center justify-center text-ui-2xs font-medium text-text-muted">
            {d}
          </div>
        ))}
      </div>

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
              aria-label={`${d.getDate()} ${LONG_MONTHS[d.getMonth()]} ${d.getFullYear()}`}
              className={cn(
                "h-8 flex items-center justify-center text-ui-2xs font-medium rounded-lg border border-transparent bg-surface-hover text-text-muted cursor-pointer transition-colors duration-ui-fast ease-out hover:bg-surface-active",
                isSelected && "bg-border-accent text-[var(--color-surface-raised,var(--surface-hover))] border-[color-mix(in_srgb,var(--border-accent)_40%,transparent)]",
                !isSelected && isToday && "bg-surface-active text-border-accent border-[color-mix(in_srgb,var(--border-accent)_30%,transparent)]",
                !isSelected && !isToday && !isCurrentMonth && "text-[color-mix(in_srgb,var(--text-muted)_30%,transparent)]",
              )}
            >
              {d.getDate()}
            </button>
          );
        })}
      </div>

      {onClear && (
        <button
          type="button"
          onClick={onClear}
          className="w-full text-left mt-1 px-2.5 py-1.5 text-ui-2xs text-text-muted bg-transparent border-none cursor-pointer rounded-md transition-colors duration-ui-fast ease-out hover:bg-surface-hover hover:text-text-strong"
          style={{ color: "var(--text-error, #d97373)" }}
        >
          Clear date
        </button>
      )}
    </fieldset>
  );
}
