import { useState } from "react";
import { EventTypeChip } from "../EventTypeChip";
import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
}

export function OtherEventCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  return (
    <div className="tracing-evcard tracing-evcard--other">
      <div className="tracing-evcard__row">
        <EventTypeChip rawKind={event.rawKind} />
        <button type="button" onClick={() => setOpen((v) => !v)} className="tracing-evcard__toggle">
          {open ? "▾" : "▸"} {event.rawKind}
        </button>
      </div>
      {open && (
        <pre className="tracing-detail-pane__pre">{JSON.stringify(event.payload, null, 2)}</pre>
      )}
    </div>
  );
}
