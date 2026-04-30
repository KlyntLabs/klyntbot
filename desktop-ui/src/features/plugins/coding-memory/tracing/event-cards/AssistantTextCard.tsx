import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; }

export function AssistantTextCard({ event }: Props) {
  const text = (event.payload as any)?.text ?? "";
  return (
    <div className="tracing-evcard tracing-evcard--text">
      <div className="tracing-evcard__row">
        <EventTypeChip rawKind={event.rawKind} />
      </div>
      <div className="tracing-context__preview">{text}</div>
    </div>
  );
}
