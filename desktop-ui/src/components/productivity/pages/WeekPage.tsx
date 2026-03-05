import { useParams } from 'react-router';
import { ProductivityLayout } from '../ProductivityLayout';
import { WeekView } from '../WeekView';
import { todayISO, weekStartISO } from '../../../lib/dates';

export function ProductivityWeekPage() {
  const { weekStart } = useParams();
  const ws = weekStart ?? weekStartISO(todayISO());

  return (
    <ProductivityLayout period="week" dateParam={ws}>
      <WeekView weekStart={ws} />
    </ProductivityLayout>
  );
}
