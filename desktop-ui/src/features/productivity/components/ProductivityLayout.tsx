import {
  formatFullDate,
  formatMonthLabel,
  formatWeekRange,
  monthISO,
  shiftDate,
  shiftMonth,
  todayISO,
  weekStartISO,
} from "@shared/lib/dates";
import type { ProductivityPeriod } from "@shared/types";
import type { NavigateFunction } from "react-router";
import { useLocation, useNavigate } from "react-router";
import { DateNavigator } from "./DateNavigator";

interface ProductivityLayoutProps {
  children: React.ReactNode;
  period?: ProductivityPeriod;
  dateParam?: string;
}

const periods: { key: ProductivityPeriod; label: string }[] = [
  { key: "day", label: "Day" },
  { key: "week", label: "Week" },
  { key: "month", label: "Month" },
];

function periodTodayUrl(p: ProductivityPeriod): string {
  const today = todayISO();
  if (p === "day") return `/productivity/day/${today}`;
  if (p === "week") return `/productivity/week/${weekStartISO(today)}`;
  return `/productivity/month/${monthISO(today)}`;
}

function buildDateNav(navigate: NavigateFunction, period: ProductivityPeriod, dateParam: string) {
  const handlePrev = () => {
    if (period === "day") navigate(`/productivity/day/${shiftDate(dateParam, -1)}`);
    else if (period === "week") navigate(`/productivity/week/${shiftDate(dateParam, -7)}`);
    else navigate(`/productivity/month/${shiftMonth(dateParam, -1)}`);
  };

  const handleNext = () => {
    if (period === "day") navigate(`/productivity/day/${shiftDate(dateParam, 1)}`);
    else if (period === "week") navigate(`/productivity/week/${shiftDate(dateParam, 7)}`);
    else navigate(`/productivity/month/${shiftMonth(dateParam, 1)}`);
  };

  const label =
    period === "day"
      ? formatFullDate(dateParam)
      : period === "week"
        ? formatWeekRange(dateParam)
        : formatMonthLabel(dateParam);

  return { label, handlePrev, handleNext, handleToday: () => navigate(periodTodayUrl(period)) };
}

export function ProductivityLayout({ children, period, dateParam }: ProductivityLayoutProps) {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const dateNav =
    period != null && dateParam != null ? buildDateNav(navigate, period, dateParam) : null;

  return (
    <div className="flex-1 flex flex-col gap-2 overflow-hidden">
      <div className="h-12 flex items-center px-4 gap-4 shrink-0">
        <div className="flex items-center gap-1">
          {periods.map((p) => (
            <button
              type="button"
              key={p.key}
              onClick={() => navigate(periodTodayUrl(p.key))}
              className={`px-3 py-1.5 rounded-xl text-[13px] font-light transition-all duration-200 ${
                period === p.key
                  ? "glass-button-active text-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-card"
              }`}
            >
              {p.label}
            </button>
          ))}
          <button
            type="button"
            onClick={() => navigate("/productivity/categories")}
            className={`px-3 py-1.5 rounded-xl text-[13px] font-light transition-all duration-200 ${
              pathname.includes("/categories")
                ? "glass-button-active text-foreground"
                : "text-muted-foreground hover:text-foreground hover:bg-card"
            }`}
          >
            Categories
          </button>
        </div>
        <div className="flex-1" />
        {dateNav && (
          <DateNavigator
            label={dateNav.label}
            onPrev={dateNav.handlePrev}
            onNext={dateNav.handleNext}
            onToday={dateNav.handleToday}
          />
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-4">{children}</div>
    </div>
  );
}
