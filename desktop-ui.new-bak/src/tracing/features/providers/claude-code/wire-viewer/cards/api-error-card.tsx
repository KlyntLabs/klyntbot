import { AlertTriangle } from "lucide-react";
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
      <div className="cc-card__header">
        <span className="cc-card__role cc-card__role--error">
          <AlertTriangle size={11} aria-hidden />
          API error
        </span>
        {p.error?.type && <span className="cc-card__chip">{p.error.type}</span>}
        <span className="cc-card__meta">
          retry {p.retryAttempt ?? 0}/{p.maxRetries ?? 0}
          {p.retryInMs != null && ` · in ${Math.round(p.retryInMs)} ms`}
        </span>
      </div>
    </div>
  );
}
