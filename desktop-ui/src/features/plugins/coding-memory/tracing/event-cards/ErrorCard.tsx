import { EventTypeChip } from "../EventTypeChip";
import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
}

export function ErrorCard({ event }: Props) {
  const p = event.payload as any;
  const content = typeof p?.content === "string" ? p.content : JSON.stringify(p?.content);
  return (
    <div className="tracing-evcard tracing-evcard--error">
      <EventTypeChip rawKind="Error" />
      <span className="tracing-evcard__error-msg">{content?.slice?.(0, 240)}</span>
    </div>
  );
}
