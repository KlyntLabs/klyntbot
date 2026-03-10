/**
 * CalendarTrack — renders calendar events as blocks in the day column grid.
 * Events are fetched from the productivity_calendar_events command.
 */

import { useMemo } from "react";
import { useQuery } from "@shared/hooks/useQuery";
import { formatHumanDuration, minutesSinceMidnight } from "@shared/lib/dates";
import type { CalendarEvent } from "@shared/types";
import { cn } from "@shared/lib/utils";

interface CalendarTrackProps {
  date: string;
  hourHeight: number;
  selectedEventId: string | null;
  onSelectEvent: (event: CalendarEvent) => void;
}

export function CalendarTrack({
  date,
  hourHeight,
  selectedEventId,
  onSelectEvent,
}: CalendarTrackProps) {
  const pxPerMin = hourHeight / 60;

  const { data: events } = useQuery<CalendarEvent[]>("productivity_calendar_events", { date }, []);

  const blocks = useMemo(() => {
    return events.map((event) => {
      const startMin = minutesSinceMidnight(event.startedAt);
      const endMin = minutesSinceMidnight(event.endedAt);
      const durationSecs = Math.round((endMin - startMin) * 60);
      return { event, startMin, endMin, durationSecs };
    });
  }, [events]);

  return (
    <>
      {blocks.map(({ event, startMin, endMin, durationSecs }) => {
        const top = startMin * pxPerMin;
        const height = Math.max((endMin - startMin) * pxPerMin, 14);
        const isSelected = selectedEventId === event.id;
        const color = event.color ?? "var(--timeline-focus)";

        return (
          <button
            type="button"
            key={event.id}
            className={cn(
              "absolute left-1 right-0.5 rounded-sm cursor-pointer overflow-hidden",
              "border-l-2",
              isSelected && "ring-1 ring-brand",
            )}
            style={{
              top,
              height,
              borderLeftColor: color,
              backgroundColor: `color-mix(in oklch, ${color} 12%, transparent)`,
            }}
            onClick={() => onSelectEvent(event)}
            aria-label={`${event.title}, ${formatHumanDuration(durationSecs)}`}
          >
            {height > 16 && (
              <span className="text-[9px] text-secondary font-medium px-1.5 truncate block leading-tight mt-0.5">
                {event.title}
              </span>
            )}
            {height > 30 && (
              <span className="text-[8px] text-muted px-1.5 truncate block">
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
