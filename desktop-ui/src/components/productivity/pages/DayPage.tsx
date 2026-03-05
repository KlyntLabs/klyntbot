import { useParams } from 'react-router';
import { ProductivityLayout } from '../ProductivityLayout';
import { DayView } from '../DayView';
import { todayISO } from '../../../lib/dates';

export function ProductivityDayPage() {
  const { date } = useParams();
  const d = date ?? todayISO();

  return (
    <ProductivityLayout period="day" dateParam={d}>
      <DayView date={d} />
    </ProductivityLayout>
  );
}
