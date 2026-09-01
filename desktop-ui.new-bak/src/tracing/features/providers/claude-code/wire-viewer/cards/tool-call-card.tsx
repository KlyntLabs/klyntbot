import { ChevronDown, ChevronRight, Wrench } from "lucide-react";
import { useState } from "react";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

function previewInput(input: Record<string, unknown>): string {
  if (typeof input.command === "string") return input.command;
  if (typeof input.file_path === "string") return input.file_path;
  if (typeof input.path === "string") return input.path;
  if (typeof input.url === "string") return input.url;
  if (typeof input.query === "string") return input.query;
  if (typeof input.prompt === "string") return input.prompt;
  const flat = JSON.stringify(input);
  return flat.length > 160 ? `${flat.slice(0, 160)}…` : flat;
}

export function ToolCallCard({ event }: Props) {
  const [open, setOpen] = useState(false);
  const p = event.payload as { name?: string; input?: Record<string, unknown> };
  const name = p.name ?? "tool";
  const input = p.input ?? {};
  const preview = previewInput(input);
  const pretty = JSON.stringify(input, null, 2);
  return (
    <div className="cc-card cc-card--tool-call">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="cc-card__header cc-card__header--button"
        aria-expanded={open}
      >
        <span className="cc-card__role cc-card__role--tool-call">
          <Wrench size={11} aria-hidden />
          Tool call
        </span>
        <span className="cc-card__tool-name">{name}</span>
        <code className="cc-card__tool-preview">{preview}</code>
        {open ? (
          <ChevronDown size={14} className="cc-card__chevron" />
        ) : (
          <ChevronRight size={14} className="cc-card__chevron" />
        )}
      </button>
      {open && <pre className="cc-card__code">{pretty}</pre>}
    </div>
  );
}
