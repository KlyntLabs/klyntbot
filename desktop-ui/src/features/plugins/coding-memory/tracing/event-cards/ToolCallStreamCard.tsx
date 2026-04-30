import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; }

export function ToolCallStreamCard({ event: _event }: Props) {
  return <div className="tracing-evcard tracing-evcard--tool-stream">…streaming arguments…</div>;
}
