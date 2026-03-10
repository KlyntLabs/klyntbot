import { DayColumnsView } from "@features/dashboard";
import { useEnabledLayers } from "@features/dashboard/lib/layers";
import { useEvent } from "@shared/hooks/useEvent";
import { useQuery } from "@shared/hooks/useQuery";
import { TZ_OFFSET_MINS, todayISO } from "@shared/lib/dates";
import type { ProductivitySummary } from "@shared/types";
import { EMPTY_TIMELINE_RESPONSE } from "@shared/types";
import { useCallback, useEffect, useMemo } from "react";

interface ProjectCalendarViewProps {
  date: string;
  projectId: string;
}

export function ProjectCalendarView({ date, projectId: _projectId }: ProjectCalendarViewProps) {
  const isToday = date === todayISO();
  const { enabledSources } = useEnabledLayers();

  const queryArgs = useMemo(
    () => ({
      startDate: date,
      endDate: date,
      sources: enabledSources,
      tzOffsetMins: TZ_OFFSET_MINS,
    }),
    [date, enabledSources],
  );

  const {
    data,
    loading,
    refetch: refetchTimeline,
  } = useQuery("timeline_query", queryArgs, EMPTY_TIMELINE_RESPONSE);

  // Productivity summary for the sidebar
  const { data: todaySummary, refetch: refetchProdToday } = useQuery<ProductivitySummary | null>(
    "productivity_today",
    isToday ? undefined : null,
    null,
  );
  const rangeArgs = useMemo(
    () => (isToday ? null : { start_date: date, end_date: date }),
    [isToday, date],
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

  useEvent<{ entityKind: string }>("entity:updated", (payload) => {
    const k = payload?.entityKind;
    if (k === "focus_session" || k === "task" || k === "note" || k === "productivity") {
      refetchAll();
    }
  });

  useEvent("activity:switch", () => {
    if (isToday) refetchProdToday();
  });

  // Periodic polling for today
  useEffect(() => {
    if (!isToday) return;
    const id = setInterval(refetchAll, 30_000);
    return () => clearInterval(id);
  }, [isToday, refetchAll]);

  return (
    <DayColumnsView
      date={date}
      entries={data.entries}
      summary={data.summary}
      isToday={isToday}
      loading={loading}
      productivitySummary={productivitySummary}
    />
  );
}
