import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { ContextMessage } from "./types";

interface Props {
  providerId?: string;
  sessionId?: string;
  scope?: any;
}

export function ContextMessagesTab({ providerId, sessionId, scope }: Props) {
  const [messages, setMessages] = useState<ContextMessage[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!providerId || !sessionId) {
      setMessages([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    invoke<ContextMessage[]>("tracing_load_context", { providerId, sessionId, scope })
      .then((rows) => {
        if (!cancelled) setMessages(rows);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [providerId, sessionId, JSON.stringify(scope)]);

  if (loading) return <div className="tracing-state">Loading context…</div>;

  return (
    <div className="tracing-context">
      {messages.length === 0 && <div className="tracing-state">No context messages.</div>}
      {messages.map((m, i) => (
        <div key={i} className="tracing-context__row">
          <span className="tracing-context__role">{m.role}</span>
          <div className="tracing-context__preview">{previewOf(m.content)}</div>
        </div>
      ))}
    </div>
  );
}

function previewOf(content: unknown): string {
  if (typeof content === "string") return content.slice(0, 300);
  return JSON.stringify(content).slice(0, 300);
}
