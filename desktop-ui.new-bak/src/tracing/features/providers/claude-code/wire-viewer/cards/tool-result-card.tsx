import { AlertTriangle, ChevronDown, ChevronRight, Terminal } from "lucide-react";
import { useState } from "react";
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
    <div className={`cc-card cc-card--tool-result${isError ? " cc-card--error" : ""}`}>
      <HeaderTag
        type={canCollapse ? "button" : undefined}
        onClick={canCollapse ? () => setOpen((v) => !v) : undefined}
        className={`cc-card__header${canCollapse ? " cc-card__header--button" : ""}`}
        aria-expanded={canCollapse ? open : undefined}
      >
        <span
          className={`cc-card__role ${isError ? "cc-card__role--error" : "cc-card__role--tool-result"}`}
        >
          {isError ? <AlertTriangle size={11} aria-hidden /> : <Terminal size={11} aria-hidden />}
          {isError ? "Error" : "Tool result"}
        </span>
        <span className="cc-card__tool-id">→ {shortId}</span>
        <span className="cc-card__meta">
          {text.length.toLocaleString()} chars
          {fullText && fullText.length > TRUNCATE_AT && " (truncated)"}
        </span>
        {canCollapse &&
          (open ? (
            <ChevronDown size={14} className="cc-card__chevron" />
          ) : (
            <ChevronRight size={14} className="cc-card__chevron" />
          ))}
      </HeaderTag>
      {open && <pre className="cc-card__tool-output">{text}</pre>}
    </div>
  );
}
