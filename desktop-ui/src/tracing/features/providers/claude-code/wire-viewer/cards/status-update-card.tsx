import { useState } from "react";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function StatusUpdateCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  const subtype = (event.payload as { subtype?: string }).subtype ?? event.type;
  const summary = renderSummary(subtype, event.payload);
  return (
    <div className="cc-card cc-card--status">
      <button type="button" onClick={() => setOpen((v) => !v)} className="cc-card__toggle">
        <span className="cc-card__chip">{subtype}</span>
        <span>{summary}</span>
      </button>
      {open && <pre className="cc-card__body">{JSON.stringify(event.payload, null, 2)}</pre>}
    </div>
  );
}

function renderSummary(subtype: string, payload: Record<string, unknown>): string {
  switch (subtype) {
    case "turn_duration": {
      const ms = (payload as { durationMs?: number }).durationMs ?? 0;
      const n = (payload as { messageCount?: number }).messageCount ?? 0;
      return `${(ms / 1000).toFixed(2)}s · ${n} messages`;
    }
    case "stop_hook_summary": {
      const c = (payload as { hookCount?: number }).hookCount ?? 0;
      const total = ((payload as { hookInfos?: { durationMs: number }[] }).hookInfos ?? []).reduce(
        (acc, h) => acc + (h.durationMs ?? 0),
        0,
      );
      return `${c} hook(s) · ${total} ms`;
    }
    default:
      return "";
  }
}
