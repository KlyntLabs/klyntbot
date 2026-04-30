import type { TraceEvent } from "../types";
import { EventTypeChip } from "../EventTypeChip";

interface Props { event: TraceEvent; onSelect: () => void; selected: boolean; }

export function ToolCallCard({ event, onSelect, selected }: Props) {
  const fn = (event.payload as any)?.function;
  const name = fn?.name ?? "(tool)";
  const args = fn?.arguments ?? "";
  return (
    <button
      type="button"
      className={"tracing-evcard tracing-evcard--tool" + (selected ? " tracing-evcard--selected" : "")}
      onClick={onSelect}
    >
      <span className="tracing-evcard__tool-id">{(event.payload as any)?.id?.slice?.(0, 16)}</span>
      <EventTypeChip rawKind="ToolCall" />
      <span className="tracing-evcard__tool-name">{name}()</span>
      <span className="tracing-evcard__tool-args">{String(args).slice(0, 120)}</span>
      <span className="tracing-evcard__detail-link">detail</span>
    </button>
  );
}
