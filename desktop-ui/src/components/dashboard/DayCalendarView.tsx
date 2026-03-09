import { useMemo } from "react";
import { useParams } from "react-router";
import { useQuery } from "../../hooks/useQuery";
import { todayISO } from "../../lib/dates";
import { EMPTY_TIMELINE_RESPONSE } from "../../lib/types";
import { DayColumnsView } from "./DayColumnsView";
import { useEnabledLayers } from "./layers";

export function DayCalendarView() {
  const { date } = useParams<{ date: string }>();
  const dateStr = date || todayISO();
  const isToday = dateStr === todayISO();

  const { enabledSources } = useEnabledLayers();
  const queryArgs = useMemo(
    () => ({ startDate: dateStr, endDate: dateStr, sources: enabledSources }),
    [dateStr, enabledSources],
  );

  const { data, loading } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  return (
    <DayColumnsView
      entries={data.entries}
      summary={data.summary}
      isToday={isToday}
      loading={loading}
    />
  );
}
