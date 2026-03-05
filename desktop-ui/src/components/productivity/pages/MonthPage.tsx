import { useParams } from 'react-router';
import { ProductivityLayout } from '../ProductivityLayout';
import { MonthView } from '../MonthView';
import { todayISO, monthISO } from '../../../lib/dates';

export function ProductivityMonthPage() {
  const { yearMonth } = useParams();
  const ym = yearMonth ?? monthISO(todayISO());

  return (
    <ProductivityLayout period="month" dateParam={ym}>
      <MonthView yearMonth={ym} />
    </ProductivityLayout>
  );
}
