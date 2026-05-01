import type { TraceEvent } from "../types";

interface Props { event: TraceEvent; }

export function StepInterruptedCard({ event }: Props) {
  const reason = (event.payload as any)?.reason ?? "interrupted";
  return <div className="tracing-evcard tracing-evcard--interrupted">{reason}</div>;
}
