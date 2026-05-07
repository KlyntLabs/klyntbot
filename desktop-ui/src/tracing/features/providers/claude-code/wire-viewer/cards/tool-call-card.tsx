import { useState } from "react";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function ToolCallCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  const p = event.payload as { name?: string; input?: Record<string, unknown> };
  const name = p.name ?? "tool";
  const input = JSON.stringify(p.input ?? {}, null, 2);
  const preview = input.slice(0, 120);
  return (
    <div className="cc-card cc-card--tool-call">
      <button type="button" onClick={() => setOpen((v) => !v)} className="cc-card__toggle">
        <span className="cc-card__tool-name">{name}</span>
        <code className="cc-card__tool-preview">{preview}</code>
      </button>
      {open && <pre className="cc-card__body">{input}</pre>}
    </div>
  );
}
