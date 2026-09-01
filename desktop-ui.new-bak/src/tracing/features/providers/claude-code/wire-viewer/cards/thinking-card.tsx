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
    <div className="cc-card cc-card--thinking">
      <button
        type="button"
        className="cc-card__header cc-card__header--button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        <span className="cc-card__role cc-card__role--thinking">Thinking</span>
        <span className="cc-card__meta">{text.length.toLocaleString()} chars</span>
        {open ? (
          <ChevronDown size={14} className="cc-card__chevron" />
        ) : (
          <ChevronRight size={14} className="cc-card__chevron" />
        )}
      </button>
      {open && <pre className="cc-card__code">{text}</pre>}
    </div>
  );
}
