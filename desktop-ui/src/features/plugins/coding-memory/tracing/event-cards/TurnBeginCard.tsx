import { EventTypeChip } from "../EventTypeChip";
import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
  expanded: boolean;
  onToggle: () => void;
}

export function TurnBeginCard({ event, expanded, onToggle }: Props) {
  const text = extractUserText(event.payload);
  return (
    <div className="tracing-evcard tracing-evcard--turn">
      <div className="tracing-evcard__row">
        <span className="tracing-evcard__time">
          {new Date(event.occurredAt).toLocaleTimeString()}
        </span>
        <EventTypeChip rawKind={event.rawKind} />
        <button type="button" className="tracing-evcard__toggle" onClick={onToggle}>
          {expanded ? "▾" : "▸"}
        </button>
        <span className="tracing-evcard__preview">{text.slice(0, 200)}</span>
      </div>
      {expanded && <pre className="tracing-detail-pane__pre">{text}</pre>}
    </div>
  );
}

function extractUserText(payload: any): string {
  if (typeof payload?.user_input === "string") return payload.user_input;
  if (Array.isArray(payload?.user_input)) {
    return payload.user_input
      .filter((p: any) => p.type === "text")
      .map((p: any) => p.text)
      .join("\n");
  }
  return "";
}
