import { ChevronDown, ChevronRight } from "lucide-react";
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
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-cyan-500">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0 cursor-pointer transition-colors duration-100 hover:bg-[var(--color-accent)]"
        aria-expanded={open}
      >
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(6,182,212,0.14)] text-[rgb(14,116,144)] dark:bg-[rgba(6,182,212,0.18)] dark:text-[rgb(125,211,252)]">{subtype}</span>
        {summary && <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">{summary}</span>}
        {open ? (
          <ChevronDown size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
        ) : (
          <ChevronRight size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
        )}
      </button>
      {open && <pre className="m-0 py-2.5 px-3.5 font-code text-ui-2xs leading-[1.55] bg-[rgba(0,0,0,0.04)] dark:bg-[rgba(0,0,0,0.28)] border-t border-border-subtle max-h-[40vh] overflow-auto whitespace-pre text-text-primary">{JSON.stringify(event.payload, null, 2)}</pre>}
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
