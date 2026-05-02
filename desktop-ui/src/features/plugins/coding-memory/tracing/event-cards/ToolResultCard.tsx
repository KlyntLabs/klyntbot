import { EventTypeChip } from "../EventTypeChip";
import type { TraceEvent } from "../types";

interface Props {
  event: TraceEvent;
}

export function ToolResultCard({ event }: Props) {
  const id = (event.payload as any)?.id;
  const content = (event.payload as any)?.content;
  const preview = typeof content === "string" ? content : JSON.stringify(content);
  return (
    <div className="tracing-evcard tracing-evcard--tool-result">
      <span className="tracing-evcard__tool-id">{id?.slice?.(0, 16)}</span>
      <EventTypeChip rawKind="ToolResult" />
      <span className="tracing-evcard__preview">{preview.slice(0, 160)}</span>
    </div>
  );
}
