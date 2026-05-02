import type { TraceEvent } from "./types";

interface Props {
  event: TraceEvent;
}

export function ToolCallDetail({ event }: Props) {
  const p = event.payload as any;
  return (
    <div className="tracing-tool-detail">
      <div className="tracing-tool-detail__id">ID: {p?.id}</div>
      <div className="tracing-tool-detail__name">{p?.function?.name}()</div>
      <pre className="tracing-detail-pane__pre">
        {JSON.stringify(p?.function?.arguments ?? {}, null, 2)}
      </pre>
    </div>
  );
}
