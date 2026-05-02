import { useState } from "react";
import { EventTypeChip } from "../EventTypeChip";
import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
}

export function ThinkingCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  const text = (event.payload as any)?.think ?? "";
  return (
    <div className="tracing-evcard tracing-evcard--think">
      <div className="tracing-evcard__row">
        <EventTypeChip rawKind="ThinkPart" />
        <button type="button" onClick={() => setOpen((v) => !v)} className="tracing-evcard__toggle">
          {open ? "▾" : "▸"} thinking…
        </button>
      </div>
      {open && (
        <pre className="tracing-detail-pane__pre" style={{ fontStyle: "italic" }}>
          {text}
        </pre>
      )}
    </div>
  );
}
