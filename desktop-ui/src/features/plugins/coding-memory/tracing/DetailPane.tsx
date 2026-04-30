import type { TraceEvent } from "./types";
import { ToolCallDetail } from "./ToolCallDetail";

interface Props {
  selectedToolEvent?: TraceEvent;
}

export function DetailPane({ selectedToolEvent }: Props) {
  return (
    <div className="tracing-detail-pane__right">
      {selectedToolEvent ? (
        <ToolCallDetail event={selectedToolEvent} />
      ) : (
        <div className="tracing-state">Select a tool call to inspect.</div>
      )}
    </div>
  );
}
