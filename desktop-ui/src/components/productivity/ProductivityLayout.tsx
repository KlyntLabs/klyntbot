import { useNavigate } from 'react-router';
import { Sidebar } from '../layout/Sidebar';
import { DateNavigator } from './DateNavigator';
import {
  todayISO, weekStartISO, monthISO,
  formatFullDate, formatWeekRange, formatMonthLabel,
  shiftDate, shiftMonth,
} from '../../lib/dates';
import type { ProductivityPeriod } from '../../lib/types';

interface ProductivityLayoutProps {
  children: React.ReactNode;
  period: ProductivityPeriod;
  dateParam: string;
}

const periods: { key: ProductivityPeriod; label: string }[] = [
  { key: 'day', label: 'Day' },
  { key: 'week', label: 'Week' },
  { key: 'month', label: 'Month' },
];

export function ProductivityLayout({ children, period, dateParam }: ProductivityLayoutProps) {
  const navigate = useNavigate();

  const handlePeriodChange = (p: ProductivityPeriod) => {
    const today = todayISO();
    if (p === 'day') navigate(`/productivity/day/${today}`);
    else if (p === 'week') navigate(`/productivity/week/${weekStartISO(today)}`);
    else navigate(`/productivity/month/${monthISO(today)}`);
  };

  const handlePrev = () => {
    if (period === 'day') navigate(`/productivity/day/${shiftDate(dateParam, -1)}`);
    else if (period === 'week') navigate(`/productivity/week/${shiftDate(dateParam, -7)}`);
    else navigate(`/productivity/month/${shiftMonth(dateParam, -1)}`);
  };

  const handleNext = () => {
    if (period === 'day') navigate(`/productivity/day/${shiftDate(dateParam, 1)}`);
    else if (period === 'week') navigate(`/productivity/week/${shiftDate(dateParam, 7)}`);
    else navigate(`/productivity/month/${shiftMonth(dateParam, 1)}`);
  };

  const handleToday = () => handlePeriodChange(period);

  const dateLabel =
    period === 'day' ? formatFullDate(dateParam) :
    period === 'week' ? formatWeekRange(dateParam) :
    formatMonthLabel(dateParam);

  return (
    <div className="h-screen w-screen bg-background text-primary flex overflow-hidden">
      <Sidebar active="Productivity" />
      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="h-14 bg-background flex items-center px-4 gap-4 flex-shrink-0">
          <div className="flex items-center gap-1">
            {periods.map((p) => (
              <button
                key={p.key}
                onClick={() => handlePeriodChange(p.key)}
                className={`px-3 py-1.5 rounded-md text-[13px] font-light transition-colors ${
                  period === p.key
                    ? 'bg-surface-highest text-white'
                    : 'bg-surface-low text-muted hover:bg-surface-base hover:text-secondary'
                }`}
              >
                {p.label}
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <DateNavigator
            label={dateLabel}
            onPrev={handlePrev}
            onNext={handleNext}
            onToday={handleToday}
          />
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          {children}
        </div>
      </div>
    </div>
  );
}
