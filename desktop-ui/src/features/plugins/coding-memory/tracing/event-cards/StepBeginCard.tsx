import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; deltaMs?: number; }

export function StepBeginCard({ event, deltaMs }: Props) {
  const n = (event.payload as any)?.n ?? "?";
  return (
    <div className="tracing-evcard tracing-evcard--step">
      <span className="tracing-evcard__step-label">Step {n}</span>
      {deltaMs !== undefined && <span className="tracing-evcard__delta">+{formatDelta(deltaMs)}</span>}
    </div>
  );
}

function formatDelta(ms: number) {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)}s`;
  return `${(ms / 60_000).toFixed(1)}min`;
}
