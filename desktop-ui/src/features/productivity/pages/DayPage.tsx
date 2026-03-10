import { todayISO } from "@shared/lib/dates";
import { useParams } from "react-router";
import { DayView } from "../components/DayView";
import { ProductivityLayout } from "../components/ProductivityLayout";

export function ProductivityDayPage() {
  const { date } = useParams();
  const d = date ?? todayISO();

  return (
    <ProductivityLayout period="day" dateParam={d}>
      <DayView date={d} />
    </ProductivityLayout>
  );
}
