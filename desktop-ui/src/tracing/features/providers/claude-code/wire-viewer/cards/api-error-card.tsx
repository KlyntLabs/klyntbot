import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function ApiErrorCard({ event }: Props) {
  const p = event.payload as {
    error?: { type?: string | null };
    retryAttempt?: number;
    maxRetries?: number;
    retryInMs?: number;
  };
  return (
    <div className="cc-card cc-card--api-error">
      <strong>API error</strong>
      <span>
        retry {p.retryAttempt ?? 0}/{p.maxRetries ?? 0}
      </span>
      {p.retryInMs != null && <span>in {Math.round(p.retryInMs)} ms</span>}
      {p.error?.type && <span className="cc-card__chip">{p.error.type}</span>}
    </div>
  );
}
