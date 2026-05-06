import type { WireEvent } from "@/tracing/lib/api";

interface Props {
  event: WireEvent;
}

export function ToolResultCard({ event }: Props) {
  const p = event.payload as {
    is_error?: boolean;
    content?: string | unknown[];
    tool_use_id?: string;
  };
  const text = typeof p.content === "string" ? p.content : JSON.stringify(p.content);
  return (
    <div className={`cc-card cc-card--tool-result${p.is_error ? " cc-card--error" : ""}`}>
      <span className="cc-card__tool-id">→ {p.tool_use_id ?? "?"}</span>
      <pre className="cc-card__body">{text?.slice(0, 4000) ?? ""}</pre>
    </div>
  );
}
