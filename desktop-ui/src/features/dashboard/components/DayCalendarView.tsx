import { useQuery } from "@shared/hooks/useQuery";
import { TZ_OFFSET_MINS, todayISO } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { useCallback, useEffect, useMemo } from "react";
import { useParams } from "react-router";
import { useEnabledLayers } from "../lib/layers";
import { DayColumnsView } from "./DayColumnsView";

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

  const dayInvalidate = {
    invalidateOn: ["entity:updated", "focus:phase_changed", "activity:switch"],
    invalidateFilter: (p: unknown) => {
      const kind = (p as { entityKind?: string })?.entityKind;
      return (
        !kind ||
        kind === "focus_session" ||
        kind === "task" ||
        kind === "note" ||
        kind === "transaction" ||
        kind === "productivity"
      );
    },
  };

  const {
    data,
    loading,
    refetch: refetchTimeline,
  } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE, dayInvalidate);

  const { data: todaySummary, refetch: refetchProdToday } = useQuery<ProductivitySummary | null>(
    "productivity_today",
    isToday ? undefined : null,
    null,
    dayInvalidate,
  );
  const rangeArgs = useMemo(
    () => (isToday ? null : { start_date: dateStr, end_date: dateStr }),
    [isToday, dateStr],
  );
  const { data: rangeSummaries, refetch: refetchProdRange } = useQuery<ProductivitySummary[]>(
    "productivity_summary_range",
    rangeArgs,
    [],
    dayInvalidate,
  );
  const productivitySummary = isToday ? todaySummary : (rangeSummaries[0] ?? null);

  const refetchAll = useCallback(() => {
    refetchTimeline();
    if (isToday) refetchProdToday();
    else refetchProdRange();
  }, [refetchTimeline, refetchProdToday, refetchProdRange, isToday]);

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
