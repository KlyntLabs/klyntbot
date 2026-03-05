import { useParams } from "react-router";
import { todayISO, weekStartISO } from "../../../lib/dates";
import { ProductivityLayout } from "../ProductivityLayout";
import { WeekView } from "../WeekView";

export function ProductivityWeekPage() {
  const { weekStart } = useParams();
  const ws = weekStart ?? weekStartISO(todayISO());

  return (
    <ProductivityLayout period="week" dateParam={ws}>
      <WeekView weekStart={ws} />
    </ProductivityLayout>
  );
}
