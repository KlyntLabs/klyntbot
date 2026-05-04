import { useMemo } from "react";
import { productivityCalendarEvents } from "@/api/endpoints/dashboard";
import type { CalendarEvent } from "@/bindings";
import { useTauriQuery } from "@/lib/query";
import { qk } from "@/lib/query/queryKeys";
import { formatHumanDuration, minutesSinceMidnight } from "@/utils/dashboardDates";
import { computeOverlapLayout } from "../../lib/timeline-utils";

interface CalendarTrackProps {
  date: string;
  hourHeight: number;
  selectedEventId: string | null;
  onSelectEvent: (event: CalendarEvent | null) => void;
}

export function CalendarTrack({
  date,
  hourHeight,
  selectedEventId,
  onSelectEvent,
}: CalendarTrackProps) {
  const pxPerMin = hourHeight / 60;

  const { data: events } = useTauriQuery<CalendarEvent[]>({
    queryKey: qk.productivity.calendarEvents(date),
    queryFn: () => productivityCalendarEvents(date),
    fallback: [],
  });

  const blocks = useMemo(() => {
    return events.map((event) => {
      const startMin = minutesSinceMidnight(event.startedAt);
      const endMin = minutesSinceMidnight(event.endedAt);
      const durationSecs = Math.round((endMin - startMin) * 60);
      return { event, startMin, endMin, durationSecs };
    });
  }, [events]);

  const layouts = useMemo(() => {
    const items = events.map((e) => ({
      id: e.id,
      startedAt: e.startedAt,
      durationSecs: Math.round(
        (new Date(e.endedAt).getTime() - new Date(e.startedAt).getTime()) / 1000,
      ),
    }));
    return computeOverlapLayout(items);
  }, [events]);

  if (events.length === 0) {
    return (
      <div
        className="dashboard__calendar-empty"
        style={{
          position: "absolute",
          inset: 0,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: "var(--fs-2xs)",
          color: "var(--ds-text-subtle)",
          padding: 16,
          textAlign: "center",
        }}
      >
        No calendar events for this day
      </div>
    );
  }

  return (
    <>
      {blocks.map(({ event, startMin, endMin, durationSecs }) => {
        const top = startMin * pxPerMin;
        const height = Math.max((endMin - startMin) * pxPerMin, 14);
        const isSelected = selectedEventId === event.id;
        const color = event.color ?? "var(--timeline-focus)";
        const layout = layouts.get(event.id);
        const hasOverlap = layout && layout.totalCols > 1;

        const posStyle: React.CSSProperties = hasOverlap
          ? {
              top,
              height,
              left: `${(layout.colIndex / layout.totalCols) * 100}%`,
              width: `${(1 / layout.totalCols) * 100}%`,
              paddingLeft: 4,
              paddingRight: 2,
            }
          : { top, height, left: 4, right: 2 };

        return (
          <button
            type="button"
            key={event.id}
            className={`dashboard__calendar-event${isSelected ? " dashboard__calendar-event--selected" : ""}`}
            style={{
              ...posStyle,
              position: "absolute",
              borderLeftWidth: 2,
              borderLeftStyle: "solid",
              borderLeftColor: color,
              backgroundColor: `color-mix(in oklch, ${color} 12%, transparent)`,
            }}
            onClick={() => onSelectEvent(isSelected ? null : event)}
            aria-label={`${event.title}, ${formatHumanDuration(durationSecs)}`}
          >
            {height > 16 && <span className="dashboard__calendar-event-title">{event.title}</span>}
            {height > 30 && (
              <span className="dashboard__calendar-event-meta">
                {formatHumanDuration(durationSecs)}
                {event.location ? ` · ${event.location}` : ""}
              </span>
            )}
          </button>
        );
      })}
    </>
  );
}
