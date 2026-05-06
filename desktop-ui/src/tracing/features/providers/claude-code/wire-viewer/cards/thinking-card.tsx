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
        className="cc-card__toggle"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        Thinking ({text.length} chars)
      </button>
      {open && <pre className="cc-card__body">{text}</pre>}
    </div>
  );
}
