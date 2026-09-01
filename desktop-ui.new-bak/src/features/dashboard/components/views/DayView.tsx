import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { EMPTY_TIMELINE_RESPONSE, timelineQuery } from "@/api/endpoints/dashboard";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { TZ_OFFSET_MINS, todayISO } from "@/utils/dashboardDates";
import { useDashboardState } from "../../hooks/useDashboardState";
import { useEnabledLayers } from "../../lib/layers";
import { DayColumns } from "./DayColumns";

export function DayView() {
  const { date } = useDashboardState();
  const dateStr = date || todayISO();
  const isToday = dateStr === todayISO();

  const { enabledSources } = useEnabledLayers();
  const sourcesKey = useMemo(() => enabledSources.map((s) => String(s)), [enabledSources]);

  const timelineQueryKey = qk.dashboard.timeline(dateStr, dateStr, sourcesKey);

  const queryClient = useQueryClient();
  const { data, isLoading } = useTauriQuery({
    queryKey: timelineQueryKey,
    queryFn: () => timelineQuery(dateStr, dateStr, enabledSources, true, TZ_OFFSET_MINS),
    fallback: EMPTY_TIMELINE_RESPONSE,
  });

  // Periodic poll for today — catches accumulated activity every 30s
  useEffect(() => {
    if (!isToday) return;
    const id = setInterval(() => {
      void queryClient.invalidateQueries({ queryKey: timelineQueryKey });
    }, 30_000);
    return () => clearInterval(id);
  }, [isToday, timelineQueryKey, queryClient]);

  return (
    <DayColumns
      date={dateStr}
      entries={data.entries}
      summary={data.summary}
      isToday={isToday}
      loading={isLoading}
      queryKey={timelineQueryKey}
    />
  );
}
