import { useParams } from "react-router";
import { todayISO, weekStartISO } from "@shared/lib/dates";
import { ProductivityLayout } from "../components/ProductivityLayout";
import { WeekView } from "../components/WeekView";

export function ProductivityWeekPage() {
  const { weekStart } = useParams();
  const ws = weekStart ?? weekStartISO(todayISO());

  return (
    <ProductivityLayout period="week" dateParam={ws}>
      <WeekView weekStart={ws} />
    </ProductivityLayout>
  );
}
