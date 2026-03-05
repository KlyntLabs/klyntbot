import { useParams } from "react-router";
import { monthISO, todayISO } from "../../../lib/dates";
import { MonthView } from "../MonthView";
import { ProductivityLayout } from "../ProductivityLayout";

export function ProductivityMonthPage() {
  const { yearMonth } = useParams();
  const ym = yearMonth ?? monthISO(todayISO());

  return (
    <ProductivityLayout period="month" dateParam={ym}>
      <MonthView yearMonth={ym} />
    </ProductivityLayout>
  );
}
