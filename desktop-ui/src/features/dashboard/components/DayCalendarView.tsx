import { useCallback, useEffect, useMemo } from "react";
import { useParams } from "react-router";
import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { TZ_OFFSET_MINS, todayISO } from "@shared/lib/dates";
import type { FocusCompletedPayload, ProductivitySummary } from "@shared/types";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { DayColumnsView } from "./DayColumnsView";
import { useEnabledLayers } from "../lib/layers";

export function DayCalendarView() {
  const { date } = useParams<{ date: string }>();
  const dateStr = date || todayISO();
  const isToday = dateStr === todayISO();

  const { enabledSources } = useEnabledLayers();
  const queryArgs = useMemo(
    () => ({
      startDate: dateStr,
      endDate: dateStr,
      sources: enabledSources,
      tzOffsetMins: TZ_OFFSET_MINS,
    }),
    [dateStr, enabledSources],
  );

  const {
    data,
    loading,
    refetch: refetchTimeline,
  } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  // Fetch productivity summary for the day
  const { data: todaySummary, refetch: refetchProdToday } = useQuery<ProductivitySummary | null>(
    "productivity_today",
    isToday ? undefined : null,
    null,
  );
  const rangeArgs = useMemo(
    () => (isToday ? null : { start_date: dateStr, end_date: dateStr }),
    [isToday, dateStr],
  );
  const { data: rangeSummaries, refetch: refetchProdRange } = useQuery<ProductivitySummary[]>(
    "productivity_summary_range",
    rangeArgs,
    [],
  );
  const productivitySummary = isToday ? todaySummary : (rangeSummaries[0] ?? null);

  const refetchAll = useCallback(() => {
    refetchTimeline();
    if (isToday) refetchProdToday();
    else refetchProdRange();
  }, [refetchTimeline, refetchProdToday, refetchProdRange, isToday]);

  // Real-time: refetch when entities change (tasks, notes, finance, focus sessions)
  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    const k = payload?.entityKind;
    if (
      k === "focus_session" ||
      k === "task" ||
      k === "note" ||
      k === "transaction" ||
      k === "productivity"
    ) {
      refetchAll();
    }
  });

  // Real-time: refetch when focus session completes
  useEvent<FocusCompletedPayload>("focus:completed", () => refetchAll());

  // Real-time: refetch when user switches apps (activity data changes)
  useEvent("activity:switch", () => {
    if (isToday) refetchProdToday();
  });

  // Periodic polling for today — catches accumulated activity data every 30s
  useEffect(() => {
    if (!isToday) return;
    const id = setInterval(refetchAll, 30_000);
    return () => clearInterval(id);
  }, [isToday, refetchAll]);

  return (
    <DayColumnsView
      date={dateStr}
      entries={data.entries}
      summary={data.summary}
      isToday={isToday}
      loading={loading}
      productivitySummary={productivitySummary}
    />
  );
}
