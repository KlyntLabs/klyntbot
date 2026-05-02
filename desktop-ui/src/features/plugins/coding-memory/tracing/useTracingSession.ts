import { useEffect, useState } from "react";
import { invoke } from "@/api/client";
import type { Scope, SessionDetail } from "./types";

interface State {
  detail: SessionDetail | null;
  loading: boolean;
  error: string | null;
}

export function useTracingSession(
  providerId: string,
  sessionId: string | null,
  scope: Scope,
  refreshKey: number = 0,
): State {
  const [state, setState] = useState<State>({ detail: null, loading: false, error: null });

  const scopeKey = scope.kind === "main" ? "main" : scope.agentId;

  useEffect(() => {
    if (!sessionId) {
      setState({ detail: null, loading: false, error: null });
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, loading: true, error: null }));
    invoke<SessionDetail>("tracing_load_session", { providerId, sessionId, scope })
      .then((d) => {
        if (cancelled) return;
        setState({ detail: d, loading: false, error: null });
      })
      .catch((err) => {
        if (cancelled) return;
        setState({ detail: null, loading: false, error: String(err?.message ?? err) });
      });
    return () => {
      cancelled = true;
    };
  }, [providerId, sessionId, scopeKey, refreshKey]);

  return state;
}
