import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { SessionState } from "./types";

interface Props {
  providerId?: string;
  sessionId?: string;
}

export function StateTab({ providerId, sessionId }: Props) {
  const [state, setState] = useState<SessionState | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!providerId || !sessionId) {
      setState(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    invoke<SessionState>("tracing_load_state", { providerId, sessionId })
      .then((s) => {
        if (!cancelled) setState(s);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [providerId, sessionId]);

  if (loading) return <div className="tracing-state">Loading state…</div>;
  if (!state) return <div className="tracing-state">No state loaded.</div>;

  return (
    <div className="tracing-state-tab">
      <section>
        <h3>Session</h3>
        <dl className="tracing-state-tab__dl">
          <dt>Title</dt>
          <dd>{state.customTitle ?? "(untitled)"}</dd>
          <dt>Plan mode</dt>
          <dd>{state.planMode ? "Yes" : "No"}</dd>
          <dt>Archived</dt>
          <dd>{state.archived ? "Yes" : "No"}</dd>
        </dl>
      </section>

      <section>
        <h3>Todos</h3>
        {state.todos?.length === 0 && <div className="tracing-state">No todos.</div>}
        <ul className="tracing-state-tab__todos">
          {state.todos?.map((t, i) => (
            <li key={i} className="tracing-state-tab__todo">
              <span
                className={
                  "tracing-state-tab__pill tracing-state-tab__pill--" + t.status.replace(" ", "_")
                }
              >
                {t.status}
              </span>
              <span className="tracing-state-tab__title">{t.title}</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
