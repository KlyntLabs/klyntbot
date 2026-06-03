import { ChevronDown, ChevronRight } from "lucide-react";
import { useState } from "react";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
  defaultExpanded?: boolean;
}

export function ThinkingCard({ event, defaultExpanded = false }: Props) {
  const [open, setOpen] = useState(defaultExpanded);
  const text = (event.payload as { thinking?: string }).thinking ?? "";
  return (
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-amber-500">
      <button
        type="button"
        className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0 cursor-pointer transition-colors duration-100 hover:bg-[var(--color-accent)]"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(245,158,11,0.18)] text-[rgb(146,64,14)] dark:bg-[rgba(245,158,11,0.18)] dark:text-[rgb(253,224,71)]">Thinking</span>
        <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">{text.length.toLocaleString()} chars</span>
        {open ? (
          <ChevronDown size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
        ) : (
          <ChevronRight size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
        )}
      </button>
      {open && <pre className="m-0 py-2.5 px-3.5 font-code text-ui-2xs leading-[1.55] bg-[rgba(0,0,0,0.04)] dark:bg-[rgba(0,0,0,0.28)] border-t border-border-subtle max-h-[40vh] overflow-auto whitespace-pre text-text-primary">{text}</pre>}
    </div>
  );
}
