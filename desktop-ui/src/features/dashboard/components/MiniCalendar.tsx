import { ChevronLeft, ChevronRight } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
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
      className="dashboard__mini-calendar"
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => e.stopPropagation()}
    >
      {showShortcuts && (
        <div className="dashboard__mini-shortcuts">
          {shortcuts.map((s) => (
            <button
              type="button"
              key={s.label}
              onClick={() => onSelect(s.iso)}
              className={`dashboard__mini-shortcut${value === s.iso ? " dashboard__mini-shortcut--active" : ""}`}
            >
              {s.label}
            </button>
          ))}
        </div>
      )}

      <div className="dashboard__mini-month-nav">
        <button
          type="button"
          onClick={prevMonth}
          aria-label="Previous month"
          className="dashboard__icon-button"
        >
          <ChevronLeft strokeWidth={1.5} />
        </button>
        <span className="dashboard__mini-month-label">
          {LONG_MONTHS[viewMonth]} {viewYear}
        </span>
        <button
          type="button"
          onClick={nextMonth}
          aria-label="Next month"
          className="dashboard__icon-button"
        >
          <ChevronRight strokeWidth={1.5} />
        </button>
      </div>

      <div className="dashboard__mini-weekdays">
        {WEEKDAYS.map((d) => (
          <div key={d} className="dashboard__mini-weekday">
            {d}
          </div>
        ))}
      </div>

      <div className="dashboard__mini-days">
        {cells.map((d) => {
          const iso = toLocalISO(d);
          const isCurrentMonth = d.getMonth() === viewMonth;
          const isToday = iso === todayISO;
          const isSelected = iso === value;
          const cls = ["dashboard__mini-day"];
          if (isSelected) cls.push("dashboard__mini-day--selected");
          else if (isToday) cls.push("dashboard__mini-day--today");
          else if (!isCurrentMonth) cls.push("dashboard__mini-day--other-month");

          return (
            <button
              type="button"
              key={iso}
              onClick={() => onSelect(iso)}
              aria-label={`${d.getDate()} ${LONG_MONTHS[d.getMonth()]} ${d.getFullYear()}`}
              className={cls.join(" ")}
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
          className="dashboard__popover-reset"
          style={{ color: "var(--text-error, #d97373)" }}
        >
          Clear date
        </button>
      )}
    </fieldset>
  );
}
