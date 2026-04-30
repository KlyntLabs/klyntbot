import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; }

export function TurnEndCard({ event }: Props) {
  return (
    <div className="tracing-evcard tracing-evcard--turn-end">
      <span className="tracing-evcard__time">{new Date(event.occurredAt).toLocaleTimeString()}</span>
      <EventTypeChip rawKind={event.rawKind} />
      <hr className="tracing-evcard__rule" />
    </div>
  );
}
