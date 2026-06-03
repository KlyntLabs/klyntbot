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
    <div className="rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden border-l-violet-500">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0 cursor-pointer transition-colors duration-100 hover:bg-[var(--color-accent)]"
        aria-expanded={open}
      >
        <span className="inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4] bg-[rgba(139,92,246,0.14)] text-[rgb(91,33,182)] dark:bg-[rgba(139,92,246,0.18)] dark:text-[rgb(196,181,253)]">
          <Wrench size={11} aria-hidden />
          Tool call
        </span>
        <span className="font-code font-semibold text-text-primary text-ui-xs shrink-0">{name}</span>
        <code className="font-code text-ui-2xs text-text-muted overflow-hidden text-ellipsis whitespace-nowrap min-w-0 flex-1">{preview}</code>
        {open ? (
          <ChevronDown size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
        ) : (
          <ChevronRight size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
        )}
      </button>
      {open && <pre className="m-0 py-2.5 px-3.5 font-code text-ui-2xs leading-[1.55] bg-[rgba(0,0,0,0.04)] dark:bg-[rgba(0,0,0,0.28)] border-t border-border-subtle max-h-[40vh] overflow-auto whitespace-pre text-text-primary">{pretty}</pre>}
    </div>
  );
}
