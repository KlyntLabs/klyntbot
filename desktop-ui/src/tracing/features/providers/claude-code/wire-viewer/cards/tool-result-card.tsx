import { AlertTriangle, ChevronDown, ChevronRight, Terminal } from "lucide-react";
import { useState } from "react";
import { cn } from "@/utils/cn";
import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

const COLLAPSE_THRESHOLD = 600;
const TRUNCATE_AT = 4000;

export function ToolResultCard({ event }: Props) {
  const p = event.payload as {
    is_error?: boolean;
    content?: string | unknown[];
    tool_use_id?: string;
  };
  const fullText = typeof p.content === "string" ? p.content : JSON.stringify(p.content);
  const text = fullText?.slice(0, TRUNCATE_AT) ?? "";
  const isError = Boolean(p.is_error);
  const canCollapse = text.length > COLLAPSE_THRESHOLD;
  const [open, setOpen] = useState(!canCollapse);
  const shortId = (p.tool_use_id ?? "?").slice(-8);

  const HeaderTag = canCollapse ? "button" : "div";
  return (
    <div className={cn("rounded-lg border border-border-subtle border-l-[3px] bg-surface-card text-ui-sm overflow-hidden", "border-l-gray-500", isError && "border-l-red-500 bg-[rgba(239,68,68,0.06)]")}>
      <HeaderTag
        type={canCollapse ? "button" : undefined}
        onClick={canCollapse ? () => setOpen((v) => !v) : undefined}
        className={cn("flex items-center gap-2 w-full py-2 px-3.5 bg-transparent border-0 text-left text-inherit [font:inherit] min-w-0", canCollapse && "cursor-pointer transition-colors duration-100 hover:bg-[var(--color-accent)]")}
        aria-expanded={canCollapse ? open : undefined}
      >
        <span
          className={cn("inline-flex items-center gap-1 py-px px-[0.4375rem] rounded text-ui-2xs font-semibold tracking-[0.04em] uppercase bg-surface-card-muted text-text-muted shrink-0 leading-[1.4]", isError ? "bg-[rgba(239,68,68,0.14)] text-[rgb(153,27,27)] dark:bg-[rgba(239,68,68,0.2)] dark:text-[rgb(252,165,165)]" : "bg-[rgba(107,114,128,0.18)] text-[rgb(55,65,81)] dark:bg-[rgba(107,114,128,0.25)] dark:text-[rgb(209,213,219)]")}
        >
          {isError ? <AlertTriangle size={11} aria-hidden /> : <Terminal size={11} aria-hidden />}
          {isError ? "Error" : "Tool result"}
        </span>
        <span className="font-code text-ui-2xs text-text-muted shrink-0">→ {shortId}</span>
        <span className="ml-auto text-ui-2xs text-text-muted shrink-0 inline-flex items-center gap-2">
          {text.length.toLocaleString()} chars
          {fullText && fullText.length > TRUNCATE_AT && " (truncated)"}
        </span>
        {canCollapse &&
          (open ? (
            <ChevronDown size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
          ) : (
            <ChevronRight size={14} className="text-text-muted shrink-0 transition-transform duration-100" />
          ))}
      </HeaderTag>
      {open && <pre className="m-0 py-2.5 px-3.5 font-code text-ui-2xs leading-[1.55] bg-[rgba(0,0,0,0.04)] dark:bg-[rgba(0,0,0,0.32)] border-t border-border-subtle max-h-[50vh] overflow-auto whitespace-pre-wrap break-words text-text-primary">{text}</pre>}
    </div>
  );
}
