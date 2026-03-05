import { useParams } from "react-router";
import { todayISO } from "../../../lib/dates";
import { DayView } from "../DayView";
import { ProductivityLayout } from "../ProductivityLayout";

export function ProductivityDayPage() {
  const { date } = useParams();
  const d = date ?? todayISO();

  return (
    <ProductivityLayout period="day" dateParam={d}>
      <DayView date={d} />
    </ProductivityLayout>
  );
}
