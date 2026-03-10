import { monthISO, todayISO } from "@shared/lib/dates";
import { useParams } from "react-router";
import { MonthView } from "../components/MonthView";
import { ProductivityLayout } from "../components/ProductivityLayout";

export function ProductivityMonthPage() {
  const { yearMonth } = useParams();
  const ym = yearMonth ?? monthISO(todayISO());

  return (
    <ProductivityLayout period="month" dateParam={ym}>
      <MonthView yearMonth={ym} />
    </ProductivityLayout>
  );
}
