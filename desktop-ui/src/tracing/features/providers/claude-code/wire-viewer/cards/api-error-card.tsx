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
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-red-600 bg-[rgba(220,38,38,0.07)]">
      <div className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0">
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(239,68,68,0.14)] text-[rgb(153,27,27)] dark:bg-[rgba(239,68,68,0.2)] dark:text-[rgb(252,165,165)]">
          <AlertTriangle size={11} aria-hidden />
          API error
        </span>
        {p.error?.type && <span className="inline-flex items-center rounded-full bg-surface-card-muted py-px px-2 text-ui-2xs font-medium text-text-muted">{p.error.type}</span>}
        <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">
          retry {p.retryAttempt ?? 0}/{p.maxRetries ?? 0}
          {p.retryInMs != null && ` · in ${Math.round(p.retryInMs)} ms`}
        </span>
      </div>
    </div>
  );
}
